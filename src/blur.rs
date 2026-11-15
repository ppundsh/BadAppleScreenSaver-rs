use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, SRCCOPY,
};

/// Captures a desktop rectangle (x, y, w, h) as a tightly-packed top-down BGRA8 buffer.
pub fn capture_desktop_rect(x: i32, y: i32, w: i32, h: i32) -> Vec<u8> {
    let len = (w.max(1) as usize) * (h.max(1) as usize) * 4;
    let mut out = vec![0u8; len];
    if w <= 0 || h <= 0 {
        return out;
    }

    unsafe {
        let screen_dc = GetDC(HWND::default());
        if screen_dc.is_invalid() {
            return out;
        }
        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_invalid() {
            ReleaseDC(HWND::default(), screen_dc);
            return out;
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        if let Ok(hbmp) = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0) {
            if !hbmp.is_invalid() && !bits_ptr.is_null() {
                let old = SelectObject(mem_dc, hbmp);
                let _ = BitBlt(mem_dc, 0, 0, w, h, screen_dc, x, y, SRCCOPY | CAPTUREBLT);
                std::ptr::copy_nonoverlapping(bits_ptr as *const u8, out.as_mut_ptr(), len);
                SelectObject(mem_dc, old);
                let _ = DeleteObject(hbmp);
            }
        }

        let _ = DeleteDC(mem_dc);
        ReleaseDC(HWND::default(), screen_dc);
    }

    // Ensure opaque alpha
    for px in out.chunks_exact_mut(4) {
        px[3] = 255;
    }
    out
}

pub fn solid_background(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
    let mut buf = vec![0u8; w * h * 4];
    for px in buf.chunks_exact_mut(4) {
        px[0] = rgb[2]; // B
        px[1] = rgb[1]; // G
        px[2] = rgb[0]; // R
        px[3] = 255;
    }
    buf
}

/// Applies dimming and optional fast multi-pass Gaussian-approximated box blur to BGRA8 buffer.
pub fn blur_and_dim_bgra(buf: &mut [u8], w: usize, h: usize, radius: u32, dim_opacity: f32) {
    if w == 0 || h == 0 || buf.len() < w * h * 4 {
        return;
    }

    if radius > 0 {
        // Downsample for large radii to keep startup instant (<20ms even on 4K)
        let scale = if radius >= 16 {
            4
        } else if radius >= 6 {
            2
        } else {
            1
        };

        if scale > 1 && w / scale >= 16 && h / scale >= 16 {
            let sw = w / scale;
            let sh = h / scale;
            let mut small = downsample_box(buf, w, h, sw, sh, scale);
            let sr = ((radius as f32) / (scale as f32)).round().max(1.0) as usize;
            box_blur_3pass(&mut small, sw, sh, sr);
            upsample_bilinear(&small, sw, sh, buf, w, h);
        } else {
            box_blur_3pass(buf, w, h, radius as usize);
        }
    }

    let keep = (1.0 - dim_opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    if keep < 0.999 {
        let keep_q = (keep * 256.0) as u32;
        for px in buf.chunks_exact_mut(4) {
            px[0] = ((px[0] as u32 * keep_q) >> 8) as u8;
            px[1] = ((px[1] as u32 * keep_q) >> 8) as u8;
            px[2] = ((px[2] as u32 * keep_q) >> 8) as u8;
            px[3] = 255;
        }
    }
}

fn downsample_box(src: &[u8], w: usize, _h: usize, sw: usize, sh: usize, scale: usize) -> Vec<u8> {
    let mut dst = vec![255u8; sw * sh * 4];
    let area = (scale * scale) as u32;
    for sy in 0..sh {
        for sx in 0..sw {
            let mut b = 0u32;
            let mut g = 0u32;
            let mut r = 0u32;
            let y0 = sy * scale;
            let x0 = sx * scale;
            for dy in 0..scale {
                let row = (y0 + dy) * w;
                for dx in 0..scale {
                    let idx = (row + x0 + dx) * 4;
                    b += src[idx] as u32;
                    g += src[idx + 1] as u32;
                    r += src[idx + 2] as u32;
                }
            }
            let out_idx = (sy * sw + sx) * 4;
            dst[out_idx] = (b / area) as u8;
            dst[out_idx + 1] = (g / area) as u8;
            dst[out_idx + 2] = (r / area) as u8;
        }
    }
    dst
}

fn upsample_bilinear(src: &[u8], sw: usize, sh: usize, dst: &mut [u8], w: usize, h: usize) {
    let x_ratio = (sw as f32) / (w as f32);
    let y_ratio = (sh as f32) / (h as f32);

    for y in 0..h {
        let fy = (y as f32 + 0.5) * y_ratio - 0.5;
        let y0 = (fy.floor().max(0.0) as usize).min(sh - 1);
        let y1 = (y0 + 1).min(sh - 1);
        let wy = (fy - y0 as f32).clamp(0.0, 1.0);
        let wy0 = ((1.0 - wy) * 256.0) as u32;
        let wy1 = 256 - wy0;

        for x in 0..w {
            let fx = (x as f32 + 0.5) * x_ratio - 0.5;
            let x0 = (fx.floor().max(0.0) as usize).min(sw - 1);
            let x1 = (x0 + 1).min(sw - 1);
            let wx = (fx - x0 as f32).clamp(0.0, 1.0);
            let wx0 = ((1.0 - wx) * 256.0) as u32;
            let wx1 = 256 - wx0;

            let i00 = (y0 * sw + x0) * 4;
            let i10 = (y0 * sw + x1) * 4;
            let i01 = (y1 * sw + x0) * 4;
            let i11 = (y1 * sw + x1) * 4;
            let out_idx = (y * w + x) * 4;

            for c in 0..3 {
                let top = src[i00 + c] as u32 * wx0 + src[i10 + c] as u32 * wx1;
                let bot = src[i01 + c] as u32 * wx0 + src[i11 + c] as u32 * wx1;
                dst[out_idx + c] = ((top * wy0 + bot * wy1) >> 16) as u8;
            }
            dst[out_idx + 3] = 255;
        }
    }
}

fn box_blur_3pass(buf: &mut [u8], w: usize, h: usize, r: usize) {
    let mut tmp = vec![255u8; w * h * 4];
    for _ in 0..3 {
        box_blur_h(buf, &mut tmp, w, h, r);
        box_blur_v(&tmp, buf, w, h, r);
    }
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let div = (r * 2 + 1) as u32;
    for y in 0..h {
        let row = y * w;
        let mut b = (src[row * 4] as u32) * (r as u32 + 1);
        let mut g = (src[row * 4 + 1] as u32) * (r as u32 + 1);
        let mut red = (src[row * 4 + 2] as u32) * (r as u32 + 1);
        for i in 1..=r {
            let xi = i.min(w - 1);
            let idx = (row + xi) * 4;
            b += src[idx] as u32;
            g += src[idx + 1] as u32;
            red += src[idx + 2] as u32;
        }
        for x in 0..w {
            let out_idx = (row + x) * 4;
            dst[out_idx] = (b / div) as u8;
            dst[out_idx + 1] = (g / div) as u8;
            dst[out_idx + 2] = (red / div) as u8;

            let add_x = (x + r + 1).min(w - 1);
            let sub_x = x.saturating_sub(r);
            let add_i = (row + add_x) * 4;
            let sub_i = (row + sub_x) * 4;
            b = b + src[add_i] as u32 - src[sub_i] as u32;
            g = g + src[add_i + 1] as u32 - src[sub_i + 1] as u32;
            red = red + src[add_i + 2] as u32 - src[sub_i + 2] as u32;
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let div = (r * 2 + 1) as u32;
    for x in 0..w {
        let mut b = (src[x * 4] as u32) * (r as u32 + 1);
        let mut g = (src[x * 4 + 1] as u32) * (r as u32 + 1);
        let mut red = (src[x * 4 + 2] as u32) * (r as u32 + 1);
        for i in 1..=r {
            let yi = i.min(h - 1);
            let idx = (yi * w + x) * 4;
            b += src[idx] as u32;
            g += src[idx + 1] as u32;
            red += src[idx + 2] as u32;
        }
        for y in 0..h {
            let out_idx = (y * w + x) * 4;
            dst[out_idx] = (b / div) as u8;
            dst[out_idx + 1] = (g / div) as u8;
            dst[out_idx + 2] = (red / div) as u8;

            let add_y = (y + r + 1).min(h - 1);
            let sub_y = y.saturating_sub(r);
            let add_i = (add_y * w + x) * 4;
            let sub_i = (sub_y * w + x) * 4;
            b = b + src[add_i] as u32 - src[sub_i] as u32;
            g = g + src[add_i + 1] as u32 - src[sub_i + 1] as u32;
            red = red + src[add_i + 2] as u32 - src[sub_i + 2] as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_and_dim_works() {
        let mut buf = vec![0u8; 64 * 64 * 4];
        // Put a bright white square in the center
        for y in 24..40 {
            for x in 24..40 {
                let i = (y * 64 + x) * 4;
                buf[i] = 255;
                buf[i + 1] = 255;
                buf[i + 2] = 255;
                buf[i + 3] = 255;
            }
        }
        blur_and_dim_bgra(&mut buf, 64, 64, 8, 0.2);
        // Edge of center should have diffused > 0 and center dimmed < 255
        let edge = (20 * 64 + 20) * 4;
        let center = (32 * 64 + 32) * 4;
        assert!(buf[edge] > 0);
        assert!(buf[center] < 255);
    }
}
