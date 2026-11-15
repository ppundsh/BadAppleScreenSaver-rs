use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Media::MediaFoundation::{
    IMFSourceReader, MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL,
    MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_NV12, MF_MT_DEFAULT_STRIDE,
    MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MFSTARTUP_NOSOCKET, MF_VERSION,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED,
};

static EMBEDDED_MP4_4_3: &[u8] = include_bytes!("../assets/bad_apple_4_3.mp4");
static EMBEDDED_MP4_16_9: &[u8] = include_bytes!("../assets/bad_apple_16_9.mp4");

struct DecodedFrame {
    pts_secs: f64,
    y_plane: Vec<u8>,
}

pub struct VideoDecoder {
    pub width: usize,
    pub height: usize,
    reader: Option<IMFSourceReader>,
    stride: usize,
    queue: VecDeque<DecodedFrame>,
    loop_base_secs: f64,
    last_pts_secs: f64,
    com_init: bool,
    mf_init: bool,
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        // Release all COM / MF objects BEFORE calling MFShutdown & CoUninitialize!
        self.reader = None;
        unsafe {
            if self.mf_init {
                let _ = MFShutdown();
            }
            if self.com_init {
                CoUninitialize();
            }
        }
    }
}

fn fnv1a_64_bytes(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn file_matches_bytes(path: &std::path::Path, expected: &[u8]) -> bool {
    use std::io::Read;
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() != expected.len() as u64 {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let mut reader = std::io::BufReader::with_capacity(65536, file);
    let mut buf = [0u8; 65536];
    let mut h: u64 = 0xcbf29ce484222325;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                for &b in &buf[..n] {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
            Err(_) => return false,
        }
    }
    h == fnv1a_64_bytes(expected)
}

fn ensure_default_mp4_path(use_16_9: bool) -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Local"))
        })
        .ok_or_else(|| "LOCALAPPDATA / USERPROFILE not set".to_string())?;
    let dir = base.join("BadAppleScreenSaver-rs");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache dir: {e}"))?;
    let (filename, bytes) = if use_16_9 {
        ("bad_apple_16_9.mp4", EMBEDDED_MP4_16_9)
    } else {
        ("bad_apple_4_3.mp4", EMBEDDED_MP4_4_3)
    };
    let path = dir.join(filename);
    if !file_matches_bytes(&path, bytes) {
        let tmp_path = dir.join(format!("{filename}.{}.tmp", std::process::id()));
        fs::write(&tmp_path, bytes).map_err(|e| format!("Failed to extract mp4: {e}"))?;
        if fs::rename(&tmp_path, &path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            fs::write(&path, bytes).map_err(|e| format!("Failed to extract mp4: {e}"))?;
        }
    }
    Ok(path)
}

impl VideoDecoder {
    pub fn open(custom_path: Option<&str>, use_16_9: bool) -> Result<Self, String> {
        let path = if let Some(p) = custom_path {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                pb
            } else {
                ensure_default_mp4_path(use_16_9)?
            }
        } else {
            ensure_default_mp4_path(use_16_9)?
        };

        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let com_ok = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)
                .map_err(|e| format!("MFStartup failed: {e}"))?;

            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 2).map_err(|e| format!("MFCreateAttributes: {e}"))?;
            let attrs = attrs.unwrap();
            let _ = attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
            let _ = attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1);

            let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attrs)
                .map_err(|e| format!("MFCreateSourceReaderFromURL: {e}"))?;

            let stream_idx = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let media_type = MFCreateMediaType().map_err(|e| format!("MFCreateMediaType: {e}"))?;
            media_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| format!("SetGUID major: {e}"))?;
            media_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
                .map_err(|e| format!("SetGUID NV12: {e}"))?;
            reader
                .SetCurrentMediaType(stream_idx, None, &media_type)
                .map_err(|e| format!("SetCurrentMediaType NV12: {e}"))?;

            let out_type = reader
                .GetCurrentMediaType(stream_idx)
                .map_err(|e| format!("GetCurrentMediaType: {e}"))?;
            let frame_size = out_type
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .map_err(|e| format!("GetUINT64 frame size: {e}"))?;
            let width = (frame_size >> 32) as usize;
            let height = (frame_size & 0xFFFF_FFFF) as usize;
            let stride = out_type
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(|s| s as usize)
                .unwrap_or(width)
                .max(width);

            let mut dec = Self {
                width,
                height,
                reader: Some(reader),
                stride,
                queue: VecDeque::with_capacity(24),
                loop_base_secs: 0.0,
                last_pts_secs: 0.0,
                com_init: com_ok,
                mf_init: true,
            };
            dec.fill_lookahead(16);
            Ok(dec)
        }
    }

    fn read_next_frame(&mut self) -> bool {
        let Some(ref reader) = self.reader else {
            return false;
        };
        let stream_idx = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        unsafe {
            for _ in 0..8 {
                let mut actual_stream = 0u32;
                let mut flags = 0u32;
                let mut timestamp_100ns = 0i64;
                let mut sample = None;

                if reader
                    .ReadSample(
                        stream_idx,
                        0,
                        Some(&mut actual_stream),
                        Some(&mut flags),
                        Some(&mut timestamp_100ns),
                        Some(&mut sample),
                    )
                    .is_err()
                {
                    return false;
                }

                if (flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                    // Loop back to beginning
                    let zero_pos = windows::core::PROPVARIANT::from(0i64);
                    let _ = reader.SetCurrentPosition(&windows::core::GUID::zeroed(), &zero_pos);
                    self.loop_base_secs += self.last_pts_secs + (1.0 / 30.0);
                    self.last_pts_secs = 0.0;
                    continue;
                }

                if let Some(sample) = sample {
                    if let Ok(buf) = sample.ConvertToContiguousBuffer() {
                        let mut ptr: *mut u8 = std::ptr::null_mut();
                        let mut max_len = 0u32;
                        let mut cur_len = 0u32;
                        if buf.Lock(&mut ptr, Some(&mut max_len), Some(&mut cur_len)).is_ok() {
                            let needed = self.stride * self.height;
                            let mut y_plane = vec![0u8; self.width * self.height];
                            if !ptr.is_null() && (cur_len as usize) >= needed {
                                for row in 0..self.height {
                                    let src = std::slice::from_raw_parts(
                                        ptr.add(row * self.stride),
                                        self.width,
                                    );
                                    y_plane[row * self.width..(row + 1) * self.width]
                                        .copy_from_slice(src);
                                }
                            }
                            let _ = buf.Unlock();
                            let pts = (timestamp_100ns.max(0) as f64) / 10_000_000.0;
                            self.last_pts_secs = pts;
                            self.queue.push_back(DecodedFrame {
                                pts_secs: self.loop_base_secs + pts,
                                y_plane,
                            });
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn fill_lookahead(&mut self, target_len: usize) {
        while self.queue.len() < target_len {
            if !self.read_next_frame() {
                break;
            }
        }
    }

    /// Advances to `elapsed_secs` and returns `(current_y_plane, (lookahead_left_avg, lookahead_right_avg))`.
    pub fn frame_at(&mut self, elapsed_secs: f64, invert: bool) -> (&[u8], (f32, f32)) {
        self.fill_lookahead(16);
        while self.queue.len() > 1 && self.queue[1].pts_secs <= elapsed_secs {
            self.queue.pop_front();
            self.fill_lookahead(16);
        }

        let (l_avg, r_avg) = self.lookahead_lr_brightness(invert);
        let y_plane = if let Some(front) = self.queue.front() {
            front.y_plane.as_slice()
        } else {
            &[]
        };
        (y_plane, (l_avg, r_avg))
    }

    fn lookahead_lr_brightness(&self, invert: bool) -> (f32, f32) {
        if self.queue.is_empty() {
            return (0.0, 0.0);
        }
        let w = self.width;
        let h = self.height;
        let half_w = w / 2;
        let step_x = (w / 64).max(1);
        let step_y = (h / 48).max(1);

        let mut sum_l = 0u64;
        let mut sum_r = 0u64;
        let mut cnt_l = 0u64;
        let mut cnt_r = 0u64;

        for idx in [0usize, 4, 8, 12] {
            let f_idx = idx.min(self.queue.len() - 1);
            let f = &self.queue[f_idx].y_plane;
            for y in (0..h).step_by(step_y) {
                let row = y * w;
                for x in (0..half_w).step_by(step_x) {
                    let v = if invert { 255 - f[row + x] } else { f[row + x] };
                    sum_l += v as u64;
                    cnt_l += 1;
                }
                for x in (half_w..w).step_by(step_x) {
                    let v = if invert { 255 - f[row + x] } else { f[row + x] };
                    sum_r += v as u64;
                    cnt_r += 1;
                }
            }
        }

        (
            sum_l as f32 / cnt_l.max(1) as f32,
            sum_r as f32 / cnt_r.max(1) as f32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_decodes_mp4() {
        let mut dec43 = VideoDecoder::open(None, false).expect("4:3 MP4 decoder should open");
        assert_eq!(dec43.width, 1440);
        assert_eq!(dec43.height, 1080);
        let (frame43, _) = dec43.frame_at(10.0, false);
        assert_eq!(frame43.len(), 1440 * 1080);
        drop(dec43);

        let mut dec169 = VideoDecoder::open(None, true).expect("16:9 MP4 decoder should open");
        assert_eq!(dec169.width, 1920);
        assert_eq!(dec169.height, 1080);
        let (frame169, (l, r)) = dec169.frame_at(10.0, false);
        assert_eq!(frame169.len(), 1920 * 1080);
        assert!((0.0..=255.0).contains(&l));
        assert!((0.0..=255.0).contains(&r));
    }
}
