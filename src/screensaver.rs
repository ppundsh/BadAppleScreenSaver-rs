use crate::audio::AudioPlayer;
use crate::blur::{blur_and_dim_bgra, capture_desktop_rect, solid_background};
use crate::config::{BackgroundMode, Config, OuterFillMode, ScaleMode};
use crate::media::VideoDecoder;
use std::cell::RefCell;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, EnumDisplayMonitors, GetDC, GetMonitorInfoW, MonitorFromPoint,
    ReleaseDC, SetDIBitsToDevice, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC,
    HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos,
    LoadCursorW, PeekMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, ShowCursor,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, IDC_ARROW, MSG, PM_REMOVE, WM_DESTROY, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
    WNDCLASSW, WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

#[derive(Clone, Copy)]
struct MonitorRect {
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    is_primary: bool,
}

#[derive(Clone, Copy)]
struct BilinearSample {
    i0: usize,
    i1: usize,
    w0: u32,
    w1: u32,
}

struct ScreenSurface {
    hwnd: HWND,
    w: usize,
    h: usize,
    black_only: bool,
    bg_buf: Vec<u8>,
    frame_buf: Vec<u8>,
    dst_x: usize,
    dst_y: usize,
    dst_w: usize,
    dst_h: usize,
    x_map: Vec<BilinearSample>,
    y_map: Vec<BilinearSample>,
    smoothed_left_alpha: f32,
    smoothed_right_alpha: f32,
}

thread_local! {
    static SHOULD_EXIT: RefCell<bool> = const { RefCell::new(false) };
    static IS_PREVIEW: RefCell<bool> = const { RefCell::new(false) };
    static START_TIME: RefCell<Option<Instant>> = const { RefCell::new(None) };
    static INIT_MOUSE: RefCell<Option<POINT>> = const { RefCell::new(None) };
}

fn request_exit() {
    SHOULD_EXIT.with(|f| *f.borrow_mut() = true);
}

fn build_edge_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    for (i, slot) in lut.iter_mut().enumerate() {
        // Expand TV-range Y (16..235) to full 0..255 and apply smooth anti-aliased S-curve
        let t = ((i as f32 - 26.0) / (224.0 - 26.0)).clamp(0.0, 1.0);
        let c = ((t - 0.5) * 1.35 + 0.5).clamp(0.0, 1.0);
        let smooth = c * c * (3.0 - 2.0 * c);
        *slot = (smooth * 255.0).round() as u8;
    }
    lut
}

/// Simulates a 3D curved bas-relief height map (128x72) of the Touhou character silhouette
/// so Liquid Glass refracts across the blurred background around the character.
fn build_glass_height_map(mask: &[u8], src_w: usize, src_h: usize, invert: bool) -> (Vec<u8>, usize, usize) {
    let gw = 128usize;
    let gh = ((src_h * gw) / src_w.max(1)).clamp(32, 128);
    let mut hmap = vec![0u8; gw * gh];
    let step_x = (src_w / gw).max(1);
    let step_y = (src_h / gh).max(1);

    for gy in 0..gh {
        let sy = (gy * src_h / gh).min(src_h - 1);
        for gx in 0..gw {
            let sx = (gx * src_w / gw).min(src_w - 1);
            let mut sum = 0u32;
            let mut cnt = 0u32;
            for dy in (0..step_y).step_by(2) {
                let row = ((sy + dy).min(src_h - 1)) * src_w;
                for dx in (0..step_x).step_by(2) {
                    let mut v = mask[row + (sx + dx).min(src_w - 1)];
                    if invert {
                        v = 255 - v;
                    }
                    sum += v as u32;
                    cnt += 1;
                }
            }
            hmap[gy * gw + gx] = (sum / cnt.max(1)) as u8;
        }
    }

    // 2-pass separable box blur (r=5) on 128x96 so the smooth 3D lens field
    // extends outward into the surrounding blurred desktop around the Touhou character.
    let mut tmp = vec![0u8; gw * gh];
    for _ in 0..2 {
        for y in 0..gh {
            for x in 0..gw {
                let mut acc = 0u32;
                let mut cnt = 0u32;
                let x0 = x.saturating_sub(5);
                let x1 = (x + 5).min(gw - 1);
                for xi in x0..=x1 {
                    acc += hmap[y * gw + xi] as u32;
                    cnt += 1;
                }
                tmp[y * gw + x] = (acc / cnt) as u8;
            }
        }
        for x in 0..gw {
            for y in 0..gh {
                let mut acc = 0u32;
                let mut cnt = 0u32;
                let y0 = y.saturating_sub(5);
                let y1 = (y + 5).min(gh - 1);
                for yi in y0..=y1 {
                    acc += tmp[yi * gw + x] as u32;
                    cnt += 1;
                }
                hmap[y * gw + x] = (acc / cnt) as u8;
            }
        }
    }

    (hmap, gw, gh)
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let preview = IS_PREVIEW.with(|p| *p.borrow());
    match msg {
        WM_DESTROY => {
            request_exit();
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &mut ps);
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
            if !preview =>
        {
            let ready = START_TIME.with(|t| {
                t.borrow()
                    .map(|s| s.elapsed() > Duration::from_millis(250))
                    .unwrap_or(false)
            });
            if ready {
                request_exit();
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE if !preview => {
            let ready = START_TIME.with(|t| {
                t.borrow()
                    .map(|s| s.elapsed() > Duration::from_millis(300))
                    .unwrap_or(false)
            });
            if ready {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    INIT_MOUSE.with(|m| {
                        let mut m = m.borrow_mut();
                        if let Some(orig) = *m {
                            let dx = pt.x - orig.x;
                            let dy = pt.y - orig.y;
                            if dx * dx + dy * dy > 144 {
                                request_exit();
                            }
                        } else {
                            *m = Some(pt);
                        }
                    });
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn enum_monitor_cb(
    hmon: HMONITOR,
    _hdc: HDC,
    rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    if !rect.is_null() {
        let r = *rect;
        let list = &mut *(lparam.0 as *mut Vec<MonitorRect>);
        let w = (r.right - r.left).max(1) as usize;
        let h = (r.bottom - r.top).max(1) as usize;
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let is_primary = if GetMonitorInfoW(hmon, &mut mi).as_bool() {
            (mi.dwFlags & 1) != 0
        } else {
            list.is_empty()
        };
        list.push(MonitorRect {
            x: r.left,
            y: r.top,
            w,
            h,
            is_primary,
        });
    }
    BOOL(1)
}

fn collect_all_monitors() -> Vec<MonitorRect> {
    let mut list = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitor_cb),
            LPARAM(&mut list as *mut Vec<MonitorRect> as isize),
        );
        if list.is_empty() {
            let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut mi).as_bool() {
                let r = mi.rcMonitor;
                list.push(MonitorRect {
                    x: r.left,
                    y: r.top,
                    w: (r.right - r.left).max(1) as usize,
                    h: (r.bottom - r.top).max(1) as usize,
                    is_primary: true,
                });
            }
        }
    }
    list
}

fn build_bilinear_map(
    vis_start: usize,
    vis_end: usize,
    off: isize,
    draw_len: usize,
    src_len: usize,
) -> Vec<BilinearSample> {
    let mut map = Vec::with_capacity(vis_end.saturating_sub(vis_start));
    let max_idx = src_len.saturating_sub(1);
    let scale = (src_len as f64) / (draw_len.max(1) as f64);

    for d in vis_start..vis_end {
        let rel = (d as isize - off) as f64;
        let f = ((rel + 0.5) * scale - 0.5).clamp(0.0, max_idx as f64);
        let i0 = (f.floor() as usize).min(max_idx);
        let i1 = (i0 + 1).min(max_idx);
        let frac = f - (i0 as f64);
        let w1 = (frac * 256.0).round() as u32;
        let w0 = 256 - w1;
        map.push(BilinearSample { i0, i1, w0, w1 });
    }
    map
}

fn compute_layout(
    src_w: usize,
    src_h: usize,
    dst_total_w: usize,
    dst_total_h: usize,
    mode: ScaleMode,
) -> (
    usize,
    usize,
    usize,
    usize,
    Vec<BilinearSample>,
    Vec<BilinearSample>,
) {
    let (draw_w, draw_h, off_x, off_y) = match mode {
        ScaleMode::Stretch => (dst_total_w, dst_total_h, 0isize, 0isize),
        ScaleMode::Original => (
            src_w,
            src_h,
            (dst_total_w as isize - src_w as isize) / 2,
            (dst_total_h as isize - src_h as isize) / 2,
        ),
        ScaleMode::Fit => {
            let scale = (dst_total_w as f64 / src_w as f64).min(dst_total_h as f64 / src_h as f64);
            let w = ((src_w as f64 * scale).round() as usize).max(1);
            let h = ((src_h as f64 * scale).round() as usize).max(1);
            (
                w,
                h,
                (dst_total_w as isize - w as isize) / 2,
                (dst_total_h as isize - h as isize) / 2,
            )
        }
        ScaleMode::Fill => {
            let scale = (dst_total_w as f64 / src_w as f64).max(dst_total_h as f64 / src_h as f64);
            let w = ((src_w as f64 * scale).round() as usize).max(1);
            let h = ((src_h as f64 * scale).round() as usize).max(1);
            (
                w,
                h,
                (dst_total_w as isize - w as isize) / 2,
                (dst_total_h as isize - h as isize) / 2,
            )
        }
    };

    let vis_x0 = off_x.max(0) as usize;
    let vis_y0 = off_y.max(0) as usize;
    let vis_x1 = ((off_x + draw_w as isize).min(dst_total_w as isize)).max(0) as usize;
    let vis_y1 = ((off_y + draw_h as isize).min(dst_total_h as isize)).max(0) as usize;

    let vis_w = vis_x1.saturating_sub(vis_x0);
    let vis_h = vis_y1.saturating_sub(vis_y0);

    let x_map = build_bilinear_map(vis_x0, vis_x1, off_x, draw_w, src_w);
    let y_map = build_bilinear_map(vis_y0, vis_y1, off_y, draw_h, src_h);

    (vis_x0, vis_y0, vis_w, vis_h, x_map, y_map)
}

fn smooth_half_alpha(prev: &mut f32, lookahead_avg: f32) -> u8 {
    // Hysteresis around 1/2 screen brightness (128) using current + future frames
    let target = if lookahead_avg >= 142.0 {
        255.0
    } else if lookahead_avg <= 114.0 {
        0.0
    } else if *prev >= 128.0 {
        255.0
    } else {
        0.0
    };
    *prev = *prev * 0.78 + target * 0.22;
    if *prev < 1.5 {
        *prev = 0.0;
    } else if *prev > 253.5 {
        *prev = 255.0;
    }
    (*prev).round().clamp(0.0, 255.0) as u8
}

fn present_buffer(hwnd: HWND, w: usize, h: usize, buf: &[u8]) {
    unsafe {
        let hdc = GetDC(hwnd);
        if !hdc.is_invalid() {
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            SetDIBitsToDevice(
                hdc,
                0,
                0,
                w as u32,
                h as u32,
                0,
                0,
                0,
                h as u32,
                buf.as_ptr() as *const _,
                &bmi,
                DIB_RGB_COLORS,
            );
            ReleaseDC(hwnd, hdc);
        }
    }
}

fn fill_outer_rect_lr(
    surface: &mut ScreenSurface,
    left_alpha: u8,
    right_alpha: u8,
    fg_b: u8,
    fg_g: u8,
    fg_r: u8,
) {
    let total_w = surface.w;
    let total_h = surface.h;
    let x0 = surface.dst_x;
    let y0 = surface.dst_y;
    let x1 = (surface.dst_x + surface.dst_w).min(total_w);
    let y1 = (surface.dst_y + surface.dst_h).min(total_h);

    if x0 == 0 && y0 == 0 && x1 == total_w && y1 == total_h {
        return;
    }

    let fg_b32 = fg_b as u32;
    let fg_g32 = fg_g as u32;
    let fg_r32 = fg_r as u32;

    let fill_row_segment = |frame: &mut [u8], bg: &[u8], start_px: usize, end_px: usize, alpha: u8| {
        let byte_start = start_px * 4;
        let byte_end = end_px * 4;
        if alpha == 0 {
            frame[byte_start..byte_end].copy_from_slice(&bg[byte_start..byte_end]);
        } else if alpha == 255 {
            for px in frame[byte_start..byte_end].chunks_exact_mut(4) {
                px[0] = fg_b;
                px[1] = fg_g;
                px[2] = fg_r;
                px[3] = 255;
            }
        } else {
            let a32 = alpha as u32;
            let inv_a = 255 - a32;
            for (f_px, b_px) in frame[byte_start..byte_end]
                .chunks_exact_mut(4)
                .zip(bg[byte_start..byte_end].chunks_exact(4))
            {
                f_px[0] = ((fg_b32 * a32 + b_px[0] as u32 * inv_a) / 255) as u8;
                f_px[1] = ((fg_g32 * a32 + b_px[1] as u32 * inv_a) / 255) as u8;
                f_px[2] = ((fg_r32 * a32 + b_px[2] as u32 * inv_a) / 255) as u8;
                f_px[3] = 255;
            }
        }
    };

    let mid_alpha = ((left_alpha as u16 + right_alpha as u16) / 2) as u8;
    for y in 0..y0 {
        let row = y * total_w;
        fill_row_segment(&mut surface.frame_buf, &surface.bg_buf, row, row + total_w, mid_alpha);
    }
    for y in y0..y1 {
        let row = y * total_w;
        if x0 > 0 {
            fill_row_segment(&mut surface.frame_buf, &surface.bg_buf, row, row + x0, left_alpha);
        }
        if x1 < total_w {
            fill_row_segment(&mut surface.frame_buf, &surface.bg_buf, row + x1, row + total_w, right_alpha);
        }
    }
    for y in y1..total_h {
        let row = y * total_w;
        fill_row_segment(&mut surface.frame_buf, &surface.bg_buf, row, row + total_w, mid_alpha);
    }
}

fn composite_and_present(
    surface: &mut ScreenSurface,
    mask: &[u8],
    src_w: usize,
    src_h: usize,
    cfg: &Config,
    edge_lut: &[u8; 256],
    glass_hmap: Option<&(Vec<u8>, usize, usize)>,
    lookahead_lr: (f32, f32),
    elapsed_secs: f64,
) {
    if surface.black_only {
        return;
    }

    let fg_b = cfg.foreground_color[2];
    let fg_g = cfg.foreground_color[1];
    let fg_r = cfg.foreground_color[0];
    let fg_b32 = fg_b as u32;
    let fg_g32 = fg_g as u32;
    let fg_r32 = fg_r as u32;
    let invert = cfg.invert_silhouette;

    let effective_fill = if cfg.scale_mode == ScaleMode::Stretch {
        OuterFillMode::None
    } else {
        cfg.outer_fill_mode
    };
    let (left_alpha, right_alpha, fill_b, fill_g, fill_r) = match effective_fill {
        OuterFillMode::Auto => (
            smooth_half_alpha(&mut surface.smoothed_left_alpha, lookahead_lr.0),
            smooth_half_alpha(&mut surface.smoothed_right_alpha, lookahead_lr.1),
            fg_b,
            fg_g,
            fg_r,
        ),
        OuterFillMode::Solid => (
            255,
            255,
            cfg.outer_fill_color[2],
            cfg.outer_fill_color[1],
            cfg.outer_fill_color[0],
        ),
        OuterFillMode::None => (0, 0, fg_b, fg_g, fg_r),
    };
    fill_outer_rect_lr(surface, left_alpha, right_alpha, fill_b, fill_g, fill_r);

    let total_w = surface.w;
    let total_h = surface.h;
    let max_x = total_w as isize - 1;
    let max_y = total_h as isize - 1;
    let wave_phase = (elapsed_secs * 2.2) as f32;
    let glass_strength = cfg.glass_strength.clamp(0.0, 2.0);
    let glass_scale_fp = (glass_strength * 256.0).round() as isize;

    for (row_idx, sy) in surface.y_map.iter().enumerate() {
        let dy = surface.dst_y + row_idx;
        let row0 = &mask[sy.i0 * src_w..];
        let row1 = &mask[sy.i1 * src_w..];
        let wy0 = sy.w0;
        let wy1 = sy.w1;
        let row_byte_off = (dy * total_w + surface.dst_x) * 4;
        let wave_x = if glass_hmap.is_some() {
            ((wave_phase + dy as f32 * 0.014).sin() * 3.0 * glass_strength).round() as isize
        } else {
            0
        };

        for (col_idx, sx) in surface.x_map.iter().enumerate() {
            let v00 = row0[sx.i0] as u32;
            let v10 = row0[sx.i1] as u32;
            let v01 = row1[sx.i0] as u32;
            let v11 = row1[sx.i1] as u32;

            let top = v00 * sx.w0 + v10 * sx.w1;
            let bot = v01 * sx.w0 + v11 * sx.w1;
            let interp = ((top * wy0 + bot * wy1 + 32768) >> 16).min(255) as usize;
            let mut alpha = edge_lut[interp];
            if invert {
                alpha = 255 - alpha;
            }
            let px_off = row_byte_off + col_idx * 4;

            if alpha == 255 {
                // Silhouette stays crisp and solid (the main focus)
                surface.frame_buf[px_off] = fg_b;
                surface.frame_buf[px_off + 1] = fg_g;
                surface.frame_buf[px_off + 2] = fg_r;
            } else if let Some((hmap, gw, gh)) = glass_hmap {
                // Liquid Glass only processes the blurred background around the character
                let hx = ((sx.i0 * gw) / src_w).min(gw - 1);
                let hy = ((sy.i0 * gh) / src_h).min(gh - 1);
                let h_l = hmap[hy * gw + hx.saturating_sub(1)] as isize;
                let h_r = hmap[hy * gw + (hx + 1).min(gw - 1)] as isize;
                let h_u = hmap[hy.saturating_sub(1) * gw + hx] as isize;
                let h_d = hmap[(hy + 1).min(gh - 1) * gw + hx] as isize;

                let nx = ((h_l - h_r) * glass_scale_fp) >> 9;
                let ny = ((h_u - h_d) * glass_scale_fp) >> 9;
                let dx = surface.dst_x as isize + col_idx as isize;
                let dy_i = dy as isize;

                // Pure refractive lens distortion + subtle dispersion (no milky tint or halo)
                let bx = (dx + wave_x + (nx * 8) / 10).clamp(0, max_x) as usize;
                let by = (dy_i + (ny * 8) / 10).clamp(0, max_y) as usize;
                let gx_p = (dx + wave_x + nx).clamp(0, max_x) as usize;
                let gy_p = (dy_i + ny).clamp(0, max_y) as usize;
                let rx = (dx + wave_x + (nx * 12) / 10).clamp(0, max_x) as usize;
                let ry = (dy_i + (ny * 12) / 10).clamp(0, max_y) as usize;

                let bg_b = surface.bg_buf[(by * total_w + bx) * 4] as u32;
                let bg_g = surface.bg_buf[(gy_p * total_w + gx_p) * 4 + 1] as u32;
                let bg_r = surface.bg_buf[(ry * total_w + rx) * 4 + 2] as u32;

                if alpha == 0 {
                    surface.frame_buf[px_off] = bg_b as u8;
                    surface.frame_buf[px_off + 1] = bg_g as u8;
                    surface.frame_buf[px_off + 2] = bg_r as u8;
                } else {
                    let a = alpha as u32;
                    let inv_a = 255 - a;
                    surface.frame_buf[px_off] = ((fg_b32 * a + bg_b * inv_a) / 255) as u8;
                    surface.frame_buf[px_off + 1] = ((fg_g32 * a + bg_g * inv_a) / 255) as u8;
                    surface.frame_buf[px_off + 2] = ((fg_r32 * a + bg_r * inv_a) / 255) as u8;
                }
            } else if alpha == 0 {
                surface.frame_buf[px_off] = surface.bg_buf[px_off];
                surface.frame_buf[px_off + 1] = surface.bg_buf[px_off + 1];
                surface.frame_buf[px_off + 2] = surface.bg_buf[px_off + 2];
            } else {
                let a = alpha as u32;
                let inv_a = 255 - a;
                let bg_b = surface.bg_buf[px_off] as u32;
                let bg_g = surface.bg_buf[px_off + 1] as u32;
                let bg_r = surface.bg_buf[px_off + 2] as u32;
                surface.frame_buf[px_off] = ((fg_b32 * a + bg_b * inv_a) / 255) as u8;
                surface.frame_buf[px_off + 1] = ((fg_g32 * a + bg_g * inv_a) / 255) as u8;
                surface.frame_buf[px_off + 2] = ((fg_r32 * a + bg_r * inv_a) / 255) as u8;
            }
        }
    }

    present_buffer(surface.hwnd, surface.w, surface.h, &surface.frame_buf);
}

pub fn run_fullscreen(cfg: &Config) -> Result<(), String> {
    let mut decoder = VideoDecoder::open(cfg.custom_video_path.as_deref(), cfg.use_16_9)?;
    let src_w = decoder.width;
    let src_h = decoder.height;
    let monitors = collect_all_monitors();
    if monitors.is_empty() {
        return Err("No monitors detected".into());
    }
    let edge_lut = build_edge_lut();

    let mut prepared_bgs = Vec::with_capacity(monitors.len());
    for m in &monitors {
        let black_only = !cfg.all_monitors && !m.is_primary;
        let bg = if black_only {
            solid_background(m.w, m.h, [0, 0, 0])
        } else {
            match cfg.background_mode {
                BackgroundMode::Solid => solid_background(m.w, m.h, cfg.solid_bg_color),
                BackgroundMode::Transparent => {
                    let mut raw = capture_desktop_rect(m.x, m.y, m.w as i32, m.h as i32);
                    blur_and_dim_bgra(&mut raw, m.w, m.h, 0, cfg.bg_dim_opacity);
                    raw
                }
                BackgroundMode::Blur => {
                    let mut raw = capture_desktop_rect(m.x, m.y, m.w as i32, m.h as i32);
                    blur_and_dim_bgra(&mut raw, m.w, m.h, cfg.blur_radius, cfg.bg_dim_opacity);
                    raw
                }
                BackgroundMode::LiquidGlass => {
                    let mut raw = capture_desktop_rect(m.x, m.y, m.w as i32, m.h as i32);
                    blur_and_dim_bgra(
                        &mut raw,
                        m.w,
                        m.h,
                        cfg.blur_radius.max(12),
                        cfg.bg_dim_opacity,
                    );
                    raw
                }
            }
        };
        prepared_bgs.push((bg, black_only));
    }

    IS_PREVIEW.with(|p| *p.borrow_mut() = false);
    SHOULD_EXIT.with(|f| *f.borrow_mut() = false);
    START_TIME.with(|t| *t.borrow_mut() = Some(Instant::now()));

    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_ok() {
            INIT_MOUSE.with(|m| *m.borrow_mut() = Some(pt));
        }

        let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {e}"))?;
        let class_name = w!("BadAppleScreenSaverWnd");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let mut surfaces = Vec::with_capacity(monitors.len());
        for (i, m) in monitors.iter().enumerate() {
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class_name,
                w!("Bad Apple!! rs"),
                WS_POPUP | WS_VISIBLE,
                m.x,
                m.y,
                m.w as i32,
                m.h as i32,
                None,
                None,
                hinstance,
                None,
            )
            .map_err(|e| format!("CreateWindowExW: {e}"))?;

            if m.is_primary {
                let _ = SetForegroundWindow(hwnd);
            }

            let (bg_buf, black_only) = std::mem::take(&mut prepared_bgs[i]);
            let frame_buf = bg_buf.clone();
            if black_only {
                present_buffer(hwnd, m.w, m.h, &frame_buf);
            }

            let (dst_x, dst_y, dst_w, dst_h, x_map, y_map) =
                compute_layout(src_w, src_h, m.w, m.h, cfg.scale_mode);

            surfaces.push(ScreenSurface {
                hwnd,
                w: m.w,
                h: m.h,
                black_only,
                bg_buf,
                frame_buf,
                dst_x,
                dst_y,
                dst_w,
                dst_h,
                x_map,
                y_map,
                smoothed_left_alpha: 0.0,
                smoothed_right_alpha: 0.0,
            });
        }

        ShowCursor(false);
        let _audio = AudioPlayer::start(cfg);

        let play_start = Instant::now();
        let mut last_tick = usize::MAX;
        let mut msg = MSG::default();

        loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if play_start.elapsed() > Duration::from_millis(250) {
                if (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_MBUTTON.0 as i32) as u16 & 0x8000) != 0
                {
                    request_exit();
                }
            }

            if SHOULD_EXIT.with(|f| *f.borrow()) {
                break;
            }

            let elapsed = play_start.elapsed().as_secs_f64();
            let tick = (elapsed * 30.0) as usize;
            if tick != last_tick {
                last_tick = tick;
                let (mask, look_lr) = decoder.frame_at(elapsed, cfg.invert_silhouette);
                if mask.len() >= src_w * src_h {
                    let glass_hmap = if cfg.background_mode == BackgroundMode::LiquidGlass
                        && cfg.glass_strength > 0.001
                    {
                        Some(build_glass_height_map(mask, src_w, src_h, cfg.invert_silhouette))
                    } else {
                        None
                    };
                    for s in &mut surfaces {
                        composite_and_present(
                            s,
                            mask,
                            src_w,
                            src_h,
                            cfg,
                            &edge_lut,
                            glass_hmap.as_ref(),
                            look_lr,
                            elapsed,
                        );
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        ShowCursor(true);
        for s in surfaces {
            let _ = DestroyWindow(s.hwnd);
        }
    }

    Ok(())
}

pub fn run_preview(parent_hwnd_val: isize, cfg: &Config) -> Result<(), String> {
    if parent_hwnd_val <= 0 {
        return Err("Invalid preview parent HWND".to_string());
    }
    let parent = HWND(parent_hwnd_val as *mut _);
    if !unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(parent).as_bool() } {
        return Err("Preview parent HWND is not a valid window".to_string());
    }
    let mut decoder = VideoDecoder::open(cfg.custom_video_path.as_deref(), cfg.use_16_9)?;
    let src_w = decoder.width;
    let src_h = decoder.height;
    let edge_lut = build_edge_lut();

    IS_PREVIEW.with(|p| *p.borrow_mut() = true);
    SHOULD_EXIT.with(|f| *f.borrow_mut() = false);

    unsafe {
        let mut rc = RECT::default();
        GetClientRect(parent, &mut rc).map_err(|e| format!("GetClientRect: {e}"))?;
        let w = (rc.right - rc.left).max(1) as usize;
        let h = (rc.bottom - rc.top).max(1) as usize;

        let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {e}"))?;
        let class_name = w!("BadApplePreviewWnd");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            w as i32,
            h as i32,
            parent,
            None,
            hinstance,
            None,
        )
        .map_err(|e| format!("CreateWindowExW preview: {e}"))?;

        let bg_buf = solid_background(w, h, cfg.solid_bg_color);
        let frame_buf = bg_buf.clone();
        let (dst_x, dst_y, dst_w, dst_h, x_map, y_map) =
            compute_layout(src_w, src_h, w, h, cfg.scale_mode);

        let mut surface = ScreenSurface {
            hwnd,
            w,
            h,
            black_only: false,
            bg_buf,
            frame_buf,
            dst_x,
            dst_y,
            dst_w,
            dst_h,
            x_map,
            y_map,
            smoothed_left_alpha: 0.0,
            smoothed_right_alpha: 0.0,
        };

        let play_start = Instant::now();
        let mut last_tick = usize::MAX;
        let mut msg = MSG::default();

        loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if SHOULD_EXIT.with(|f| *f.borrow()) {
                break;
            }
            let elapsed = play_start.elapsed().as_secs_f64();
            let tick = (elapsed * 30.0) as usize;
            if tick != last_tick {
                last_tick = tick;
                let (mask, look_lr) = decoder.frame_at(elapsed, cfg.invert_silhouette);
                if mask.len() >= src_w * src_h {
                    composite_and_present(
                        &mut surface,
                        mask,
                        src_w,
                        src_h,
                        cfg,
                        &edge_lut,
                        None,
                        look_lr,
                        elapsed,
                    );
                }
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    Ok(())
}
