use crate::config::{BackgroundMode, Config, Language, OuterFillMode, ScaleMode};
use crate::i18n::{strings, Strings};
use eframe::egui;
use std::fs;
use std::sync::Arc;
use std::time::Instant;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowLongW, SetWindowPos, SystemParametersInfoW, GWL_STYLE, SPI_GETWORKAREA,
    SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    WS_CAPTION, WS_SYSMENU,
};

static ICON_RGBA_64: &[u8] = include_bytes!("../assets/black_apple_64.rgba");

#[derive(Clone, Copy, PartialEq, Eq)]
struct ThemePalette {
    is_dark: bool,
    accent_base: egui::Color32,
    accent_hover: egui::Color32,
    accent_pressed: egui::Color32,
    accent_light: egui::Color32,
    accent_label: egui::Color32,
    accent_text: egui::Color32,
    sidebar_bg: egui::Color32,
    main_bg: egui::Color32,
    card_bg: egui::Color32,
    card_border: egui::Color32,
    sub_card_bg: egui::Color32,
    slider_track: egui::Color32,
    btn_bg: egui::Color32,
    btn_hover_bg: egui::Color32,
    btn_pressed_bg: egui::Color32,
    btn_border: egui::Color32,
    btn_hover_border: egui::Color32,
    btn_pressed_border: egui::Color32,
    nav_selected_bg: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    text_muted: egui::Color32,
    status_ok: egui::Color32,
    diagram_wing_on: egui::Color32,
    diagram_wing_off: egui::Color32,
    diagram_video_bg: egui::Color32,
}

impl ThemePalette {
    fn from_system(is_dark: bool, accent: egui::Color32) -> Self {
        let lum = 0.2126 * (accent.r() as f32)
            + 0.7152 * (accent.g() as f32)
            + 0.0722 * (accent.b() as f32);
        let accent_text = if lum > 165.0 {
            egui::Color32::from_rgb(0x14, 0x14, 0x18)
        } else {
            egui::Color32::WHITE
        };

        let accent_hover = if is_dark {
            lerp_color(accent, egui::Color32::WHITE, 0.14)
        } else {
            lerp_color(accent, egui::Color32::BLACK, 0.10)
        };
        let accent_pressed = if is_dark {
            lerp_color(accent, egui::Color32::BLACK, 0.18)
        } else {
            lerp_color(accent, egui::Color32::BLACK, 0.22)
        };
        let accent_light = if is_dark {
            lerp_color(accent, egui::Color32::WHITE, 0.36)
        } else {
            lerp_color(accent, egui::Color32::WHITE, 0.22)
        };
        let accent_label = if is_dark {
            accent_light
        } else {
            lerp_color(accent, egui::Color32::BLACK, 0.18)
        };

        if is_dark {
            Self {
                is_dark,
                accent_base: accent,
                accent_hover,
                accent_pressed,
                accent_light,
                accent_label,
                accent_text,
                sidebar_bg: egui::Color32::from_rgba_unmultiplied(0x20, 0x20, 0x23, 130),
                main_bg: egui::Color32::from_rgb(0x16, 0x16, 0x18),
                card_bg: egui::Color32::from_rgb(0x22, 0x22, 0x26),
                card_border: egui::Color32::from_rgb(0x32, 0x32, 0x38),
                sub_card_bg: egui::Color32::from_rgb(0x1A, 0x1A, 0x1D),
                slider_track: egui::Color32::from_rgb(0x38, 0x38, 0x40),
                btn_bg: egui::Color32::from_rgb(0x2B, 0x2B, 0x30),
                btn_hover_bg: egui::Color32::from_rgb(0x37, 0x37, 0x3E),
                btn_pressed_bg: egui::Color32::from_rgb(0x21, 0x21, 0x25),
                btn_border: egui::Color32::from_rgb(0x3F, 0x3F, 0x47),
                btn_hover_border: egui::Color32::from_rgb(0x55, 0x55, 0x60),
                btn_pressed_border: egui::Color32::from_rgb(0x32, 0x32, 0x38),
                nav_selected_bg: egui::Color32::from_rgb(0x2B, 0x2B, 0x30),
                text_primary: egui::Color32::from_rgb(0xF3, 0xF3, 0xF6),
                text_secondary: egui::Color32::from_rgb(0xD0, 0xD0, 0xDA),
                text_muted: egui::Color32::from_rgb(0x9A, 0x9A, 0xA8),
                status_ok: egui::Color32::from_rgb(0x5F, 0xCB, 0x6C),
                diagram_wing_on: egui::Color32::from_rgba_unmultiplied(
                    accent.r(),
                    accent.g(),
                    accent.b(),
                    48,
                ),
                diagram_wing_off: egui::Color32::from_rgba_unmultiplied(90, 96, 110, 22),
                diagram_video_bg: egui::Color32::from_rgb(26, 30, 38),
            }
        } else {
            Self {
                is_dark,
                accent_base: accent,
                accent_hover,
                accent_pressed,
                accent_light,
                accent_label,
                accent_text,
                sidebar_bg: egui::Color32::from_rgba_unmultiplied(0xF0, 0xF0, 0xF4, 150),
                main_bg: egui::Color32::from_rgb(0xE6, 0xE6, 0xEC),
                card_bg: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
                card_border: egui::Color32::from_rgb(0xD4, 0xD4, 0xDC),
                sub_card_bg: egui::Color32::from_rgb(0xF3, 0xF3, 0xF7),
                slider_track: egui::Color32::from_rgb(0xC8, 0xC8, 0xD2),
                btn_bg: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
                btn_hover_bg: egui::Color32::from_rgb(0xF5, 0xF5, 0xF8),
                btn_pressed_bg: egui::Color32::from_rgb(0xE8, 0xE8, 0xEE),
                btn_border: egui::Color32::from_rgb(0xCC, 0xCC, 0xD6),
                btn_hover_border: egui::Color32::from_rgb(0xB0, 0xB0, 0xBC),
                btn_pressed_border: egui::Color32::from_rgb(0xC0, 0xC0, 0xCA),
                nav_selected_bg: egui::Color32::from_rgb(0xE2, 0xE2, 0xE8),
                text_primary: egui::Color32::from_rgb(0x18, 0x18, 0x1C),
                text_secondary: egui::Color32::from_rgb(0x44, 0x44, 0x4E),
                text_muted: egui::Color32::from_rgb(0x6E, 0x6E, 0x7A),
                status_ok: egui::Color32::from_rgb(0x10, 0x7C, 0x10),
                diagram_wing_on: egui::Color32::from_rgba_unmultiplied(
                    accent.r(),
                    accent.g(),
                    accent.b(),
                    56,
                ),
                diagram_wing_off: egui::Color32::from_rgba_unmultiplied(150, 155, 168, 35),
                diagram_video_bg: egui::Color32::from_rgb(225, 228, 235),
            }
        }
    }
}

fn read_registry_dword(subkey: windows::core::PCWSTR, value: windows::core::PCWSTR) -> Option<u32> {
    let mut data: u32 = 0;
    let mut size: u32 = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    if status.is_ok() {
        Some(data)
    } else {
        None
    }
}

fn query_system_theme() -> (bool, egui::Color32) {
    let is_dark = read_registry_dword(
        w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
        w!("AppsUseLightTheme"),
    )
    .map(|v| v == 0)
    .unwrap_or(true);

    let accent = read_registry_dword(w!(r"Software\Microsoft\Windows\DWM"), w!("AccentColor"))
        .map(|abgr| {
            let r = (abgr & 0xFF) as u8;
            let g = ((abgr >> 8) & 0xFF) as u8;
            let b = ((abgr >> 16) & 0xFF) as u8;
            egui::Color32::from_rgb(r, g, b)
        })
        .unwrap_or_else(|| egui::Color32::from_rgb(0x00, 0x78, 0xD4));

    (is_dark, accent)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    General,
    About,
}

fn centered_window_pos(win_w: f32, win_h: f32) -> egui::Pos2 {
    unsafe {
        let mut rc = RECT::default();
        let dpi_scale = (GetDpiForSystem() as f32 / 96.0).max(1.0);
        if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rc as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            let work_w = (rc.right - rc.left) as f32 / dpi_scale;
            let work_h = (rc.bottom - rc.top) as f32 / dpi_scale;
            let x = (rc.left as f32 / dpi_scale) + ((work_w - win_w) * 0.5).max(0.0);
            let y = (rc.top as f32 / dpi_scale) + ((work_h - win_h) * 0.5).max(0.0);
            return egui::pos2(x, y);
        }
    }
    egui::pos2(160.0, 80.0)
}

unsafe extern "system" fn find_own_window_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == GetCurrentProcessId() && IsWindowVisible(hwnd).as_bool() {
        let mut title = [0u16; 128];
        let len = GetWindowTextW(hwnd, &mut title);
        if len > 0 && String::from_utf16_lossy(&title[..len as usize]) == "Bad Apple!! rs" {
            let out = &mut *(lparam.0 as *mut Option<HWND>);
            *out = Some(hwnd);
            return BOOL(0);
        }
    }
    BOOL(1)
}

fn ensure_borderless_window_style() {
    unsafe {
        let mut found: Option<HWND> = None;
        let _ = EnumWindows(
            Some(find_own_window_cb),
            LPARAM(&mut found as *mut Option<HWND> as isize),
        );
        if let Some(hwnd) = found {
            if !hwnd.is_invalid() {
                let style = GetWindowLongW(hwnd, GWL_STYLE);
                let clean_style = style & !((WS_CAPTION.0 | WS_SYSMENU.0) as i32);
                if clean_style != style {
                    SetWindowLongW(hwnd, GWL_STYLE, clean_style);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                    );
                }
            }
        }
    }
}

fn enable_win11_backdrop_blur(is_dark: bool) -> bool {
    unsafe {
        let mut found: Option<HWND> = None;
        let _ = EnumWindows(
            Some(find_own_window_cb),
            LPARAM(&mut found as *mut Option<HWND> as isize),
        );
        if let Some(hwnd) = found {
            if !hwnd.is_invalid() {
                // Ensure WS_CAPTION / WS_SYSMENU are cleared so DWM does not paint duplicate native caption buttons
                let style = GetWindowLongW(hwnd, GWL_STYLE);
                let clean_style = style & !((WS_CAPTION.0 | WS_SYSMENU.0) as i32);
                if clean_style != style {
                    SetWindowLongW(hwnd, GWL_STYLE, clean_style);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                    );
                }

                let dark: i32 = if is_dark { 1 } else { 0 };
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                    &dark as *const i32 as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
                let margins = MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                };
                let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
                // 3 = DWMSBT_TRANSIENTWINDOW (Acrylic translucent blur for sidebar & titlebar)
                let backdrop: i32 = 3;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE,
                    &backdrop as *const i32 as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
                // 2 = DWMWCP_ROUND (keep native Windows 11 rounded corners when borderless)
                let corner: i32 = 2;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &corner as *const i32 as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
                return true;
            }
        }
    }
    false
}

pub fn run_settings() -> Result<(), String> {
    let cfg = Config::load();
    let (is_dark, accent) = query_system_theme();
    let pal = ThemePalette::from_system(is_dark, accent);

    let icon = egui::IconData {
        rgba: ICON_RGBA_64.to_vec(),
        width: 64,
        height: 64,
    };
    let win_w = 840.0;
    let win_h = 870.0;
    let pos = centered_window_pos(win_w, win_h);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([win_w, win_h])
            .with_min_inner_size([740.0, 560.0])
            .with_position(pos)
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_maximize_button(true)
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        "Bad Apple!! rs",
        options,
        Box::new(move |cc| {
            setup_cjk_font(&cc.egui_ctx);
            apply_fluent_theme(&cc.egui_ctx, &pal);
            enable_win11_backdrop_blur(pal.is_dark);
            Ok(Box::new(SettingsApp {
                cfg,
                tab: ActiveTab::General,
                nav_prev_tab: ActiveTab::General,
                nav_switch_time: None,
                content_prev_tab: ActiveTab::General,
                content_switch_time: None,
                status_msg: String::new(),
                backdrop_applied: false,
                style_check_frames: 0,
                pal,
                last_theme_check: Instant::now(),
                title_icon: None,
            }))
        }),
    )
    .map_err(|e| format!("Failed to open settings UI: {e}"))
}

fn setup_cjk_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let tc_candidates = [
        r"C:\Windows\Fonts\msjh.ttc",
        r"C:\Windows\Fonts\msjhbd.ttc",
        r"C:\Windows\Fonts\kaiu.ttf",
        r"C:\Windows\Fonts\mingliu.ttc",
    ];
    let jp_candidates = [
        r"C:\Windows\Fonts\YuGothR.ttc",
        r"C:\Windows\Fonts\meiryo.ttc",
        r"C:\Windows\Fonts\msgothic.ttc",
    ];
    let mut loaded_any = false;

    for path in tc_candidates {
        if let Ok(data) = fs::read(path) {
            fonts
                .font_data
                .insert("cjk_tc".to_owned(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk_tc".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk_tc".to_owned());
            loaded_any = true;
            break;
        }
    }

    for path in jp_candidates {
        if let Ok(data) = fs::read(path) {
            fonts
                .font_data
                .insert("cjk_jp".to_owned(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("cjk_jp".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk_jp".to_owned());
            loaded_any = true;
            break;
        }
    }

    if loaded_any {
        ctx.set_fonts(fonts);
    }
}

fn apply_fluent_theme(ctx: &egui::Context, pal: &ThemePalette) {
    let mut style = (*ctx.style()).clone();
    style.animation_time = 0.14;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.visuals = if pal.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.panel_fill = egui::Color32::TRANSPARENT;
    style.visuals.window_fill = egui::Color32::TRANSPARENT;
    style.visuals.override_text_color = Some(pal.text_primary);
    style.visuals.widgets.noninteractive.bg_fill = pal.card_bg;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, pal.text_secondary);
    style.visuals.widgets.inactive.bg_fill = pal.btn_bg;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, pal.text_primary);
    style.visuals.widgets.hovered.bg_fill = pal.btn_hover_bg;
    style.visuals.widgets.active.bg_fill = pal.accent_base;
    style.visuals.selection.bg_fill = pal.accent_base;
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, pal.accent_light);
    style.visuals.hyperlink_color = pal.accent_label;
    ctx.set_style(style);
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| -> u8 { ((x as f32) + ((y as f32) - (x as f32)) * t).round() as u8 };
    egui::Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

fn fluent_card(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .fill(pal.card_bg)
        .stroke(egui::Stroke::new(1.0_f32, pal.card_border))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(16.0, 12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .size(14.0)
                    .color(pal.text_primary),
            );
            ui.add_space(5.0);
            add_contents(ui);
        });
}

fn draw_gear_icon(painter: &egui::Painter, center: egui::Pos2, r: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.4_f32, color);
    painter.circle_stroke(center, r * 0.42, stroke);
    for i in 0..8 {
        let ang = (i as f32) * std::f32::consts::FRAC_PI_4;
        let (sin_a, cos_a) = ang.sin_cos();
        let p0 = egui::pos2(center.x + cos_a * r * 0.62, center.y + sin_a * r * 0.62);
        let p1 = egui::pos2(center.x + cos_a * r * 0.96, center.y + sin_a * r * 0.96);
        painter.line_segment([p0, p1], egui::Stroke::new(1.8_f32, color));
    }
    painter.circle_stroke(center, r * 0.70, egui::Stroke::new(1.1_f32, color));
}

fn draw_info_icon(painter: &egui::Painter, center: egui::Pos2, r: f32, color: egui::Color32) {
    painter.circle_stroke(center, r * 0.88, egui::Stroke::new(1.4_f32, color));
    painter.circle_filled(
        egui::pos2(center.x, center.y - r * 0.36),
        1.15,
        color,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - r * 0.08),
            egui::pos2(center.x, center.y + r * 0.46),
        ],
        egui::Stroke::new(1.6_f32, color),
    );
}

fn draw_globe_icon(painter: &egui::Painter, center: egui::Pos2, r: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.2_f32, color);
    painter.circle_stroke(center, r, stroke);
    // Equator
    painter.line_segment(
        [
            egui::pos2(center.x - r, center.y),
            egui::pos2(center.x + r, center.y),
        ],
        stroke,
    );
    // Upper and lower latitude lines
    let lat_y = r * 0.48;
    let lat_x = (r * r - lat_y * lat_y).sqrt();
    painter.line_segment(
        [
            egui::pos2(center.x - lat_x, center.y - lat_y),
            egui::pos2(center.x + lat_x, center.y - lat_y),
        ],
        egui::Stroke::new(1.0_f32, color),
    );
    painter.line_segment(
        [
            egui::pos2(center.x - lat_x, center.y + lat_y),
            egui::pos2(center.x + lat_x, center.y + lat_y),
        ],
        egui::Stroke::new(1.0_f32, color),
    );
    // Vertical meridian ellipse (left & right arcs)
    let mut left_arc = Vec::with_capacity(9);
    let mut right_arc = Vec::with_capacity(9);
    for i in 0..=8 {
        let t = -std::f32::consts::FRAC_PI_2 + (i as f32 / 8.0) * std::f32::consts::PI;
        let y = center.y + t.sin() * r;
        let dx = t.cos() * r * 0.48;
        left_arc.push(egui::pos2(center.x - dx, y));
        right_arc.push(egui::pos2(center.x + dx, y));
    }
    painter.add(egui::Shape::line(left_arc, stroke));
    painter.add(egui::Shape::line(right_arc, stroke));
}

fn fluent_slider(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    unit: &str,
) -> bool {
    const LABEL_W: f32 = 148.0;
    const VAL_W: f32 = 52.0;

    let min = *range.start();
    let max = *range.end();
    let span = (max - min).max(1e-5);
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(LABEL_W, 24.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(egui::RichText::new(label).color(pal.text_secondary));
            },
        );

        let track_w = (ui.available_width() - VAL_W - 10.0).max(90.0);
        let (rect, mut response) =
            ui.allocate_exact_size(egui::vec2(track_w, 24.0), egui::Sense::click_and_drag());
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        let pad_x = 8.0;
        let left_x = rect.left() + pad_x;
        let right_x = rect.right() - pad_x;
        let usable_w = (right_x - left_x).max(1.0);

        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let norm = ((pos.x - left_x) / usable_w).clamp(0.0, 1.0);
                let next = (min + norm * span).round().clamp(min, max);
                if (next - *value).abs() > 0.01 {
                    *value = next;
                    changed = true;
                }
            }
        }

        if ui.is_rect_visible(rect) {
            let norm = ((*value - min) / span).clamp(0.0, 1.0);
            let thumb_x = left_x + norm * usable_w;
            let cy = rect.center().y;
            let track_h = 4.0;

            let full_track = egui::Rect::from_min_max(
                egui::pos2(left_x, cy - track_h * 0.5),
                egui::pos2(right_x, cy + track_h * 0.5),
            );
            ui.painter().rect_filled(full_track, 2.0, pal.slider_track);

            let active_track = egui::Rect::from_min_max(
                egui::pos2(left_x, cy - track_h * 0.5),
                egui::pos2(thumb_x, cy + track_h * 0.5),
            );
            ui.painter()
                .rect_filled(active_track, 2.0, pal.accent_base);

            let hov_t = ui
                .ctx()
                .animate_bool_with_time(response.id.with("sl_hov"), response.hovered(), 0.12);
            let drag_t = ui
                .ctx()
                .animate_bool_with_time(response.id.with("sl_drg"), response.dragged(), 0.09);

            let outer_r = 7.5 + 1.0 * hov_t - 0.5 * drag_t;
            let inner_r = 3.8 + 0.8 * hov_t - 0.8 * drag_t;
            let outer_col = if pal.is_dark {
                egui::Color32::from_rgb(0x45, 0x45, 0x4E)
            } else {
                egui::Color32::from_rgb(0xFF, 0xFF, 0xFF)
            };
            ui.painter().circle(
                egui::pos2(thumb_x, cy),
                outer_r,
                outer_col,
                egui::Stroke::new(1.0_f32, pal.btn_hover_border),
            );
            let inner_col = lerp_color(pal.accent_base, pal.accent_hover, hov_t);
            ui.painter()
                .circle_filled(egui::pos2(thumb_x, cy), inner_r, inner_col);
        }

        ui.allocate_ui_with_layout(
            egui::vec2(VAL_W, 24.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let text = format!("{}{}", value.round() as i32, unit);
                let frame_rect = ui.available_rect_before_wrap();
                ui.painter().rect(
                    egui::Rect::from_center_size(frame_rect.center(), egui::vec2(VAL_W, 22.0)),
                    4.0,
                    pal.sub_card_bg,
                    egui::Stroke::new(1.0_f32, pal.card_border),
                );
                let galley = ui.painter().layout_no_wrap(
                    text,
                    egui::FontId::monospace(12.0),
                    pal.text_primary,
                );
                ui.painter().galley(
                    frame_rect.center() - galley.size() * 0.5,
                    galley,
                    pal.text_primary,
                );
            },
        );
    });

    changed
}

fn labeled_row(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    enabled: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    const LABEL_W: f32 = 90.0;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(LABEL_W, 28.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let col = if enabled {
                    pal.text_primary
                } else {
                    pal.text_muted
                };
                ui.label(egui::RichText::new(label).color(col));
            },
        );
        add_contents(ui);
    });
}

fn sidebar_nav_item(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    current: &mut ActiveTab,
    value: ActiveTab,
    label: &str,
) -> (bool, egui::Rect, f32) {
    let selected = *current == value;
    let desired = egui::vec2(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let mut changed = false;
    if response.clicked() && !selected {
        *current = value;
        changed = true;
    }

    let press_t = ui.ctx().animate_bool_with_time(
        response.id.with("nav_prs"),
        response.is_pointer_button_down_on(),
        0.09,
    );
    let draw_rect = rect.shrink(press_t * 0.5);

    if ui.is_rect_visible(rect) {
        let sel_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("nav_sel"), selected, 0.14);
        let hov_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("nav_hov"), response.hovered(), 0.12);

        let rest_bg = lerp_color(egui::Color32::TRANSPARENT, pal.nav_selected_bg, sel_t);
        let hov_bg = lerp_color(rest_bg, pal.btn_hover_bg, hov_t * 0.65);
        let fill = lerp_color(hov_bg, pal.btn_pressed_bg, press_t);

        ui.painter().rect_filled(draw_rect, 6.0, fill);

        let base_text = lerp_color(pal.text_secondary, pal.text_primary, sel_t.max(hov_t));
        let text_col = lerp_color(base_text, pal.text_muted, press_t * 0.45);
        let icon_center = egui::pos2(draw_rect.left() + 21.0, draw_rect.center().y);
        match value {
            ActiveTab::General => draw_gear_icon(ui.painter(), icon_center, 6.5, text_col),
            ActiveTab::About => draw_info_icon(ui.painter(), icon_center, 6.8, text_col),
        }

        let galley = ui.painter().layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(14.0),
            text_col,
        );
        let text_pos = egui::pos2(
            draw_rect.left() + 36.0,
            draw_rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().galley(text_pos, galley, text_col);
    }
    (changed, draw_rect, press_t)
}

fn fluent_checkbox(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    checked: &mut bool,
    label: &str,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let box_size = egui::vec2(17.0, 17.0);
        let (rect, response) = ui.allocate_exact_size(box_size, egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        let galley = ui.painter().layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(13.0),
            pal.text_primary,
        );
        let text_resp = ui
            .add(egui::Label::new(galley).sense(egui::Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        if response.clicked() || text_resp.clicked() {
            *checked = !*checked;
            changed = true;
        }

        if ui.is_rect_visible(rect) {
            let chk_t = ui
                .ctx()
                .animate_bool_with_time(response.id.with("chk"), *checked, 0.18);
            let hov_t = ui.ctx().animate_bool_with_time(
                response.id.with("chk_hov"),
                response.hovered() || text_resp.hovered(),
                0.12,
            );
            let press_t = ui.ctx().animate_bool_with_time(
                response.id.with("chk_prs"),
                response.is_pointer_button_down_on() || text_resp.is_pointer_button_down_on(),
                0.09,
            );

            let off_bg = lerp_color(pal.sub_card_bg, pal.btn_hover_bg, hov_t);
            let off_bg = lerp_color(off_bg, pal.btn_pressed_bg, press_t);
            let on_bg = lerp_color(pal.accent_base, pal.accent_hover, hov_t);
            let on_bg = lerp_color(on_bg, pal.accent_pressed, press_t);
            let fill = lerp_color(off_bg, on_bg, chk_t);

            let off_border = lerp_color(pal.btn_border, pal.btn_hover_border, hov_t);
            let off_border = lerp_color(off_border, pal.btn_pressed_border, press_t);
            let on_border = lerp_color(pal.accent_light, pal.accent_pressed, press_t);
            let border = lerp_color(off_border, on_border, chk_t);

            let draw_rect = rect.shrink(press_t * 0.5);
            ui.painter()
                .rect(draw_rect, 4.0, fill, egui::Stroke::new(1.2_f32, border));

            if chk_t > 0.02 {
                let alpha = if *checked {
                    ((chk_t * 3.0).min(1.0) * 255.0).round() as u8
                } else {
                    (chk_t * 255.0).round() as u8
                };
                let tick_col = egui::Color32::from_rgba_unmultiplied(
                    pal.accent_text.r(),
                    pal.accent_text.g(),
                    pal.accent_text.b(),
                    alpha,
                );
                let p1 = egui::pos2(draw_rect.left() + 3.8, draw_rect.center().y + 0.5);
                let p2 = egui::pos2(draw_rect.left() + 7.2, draw_rect.bottom() - 4.0);
                let p3 = egui::pos2(draw_rect.right() - 3.6, draw_rect.top() + 4.0);
                let stroke = egui::Stroke::new(2.0_f32, tick_col);

                if *checked {
                    // Draw checkmark progressively from left (p1) -> vertex (p2) -> right (p3)
                    let t1 = (chk_t / 0.35).clamp(0.0, 1.0);
                    let mid = p1 + (p2 - p1) * t1;
                    ui.painter().line_segment([p1, mid], stroke);

                    if chk_t > 0.35 {
                        let t2 = ((chk_t - 0.35) / 0.65).clamp(0.0, 1.0);
                        let end = p2 + (p3 - p2) * t2;
                        ui.painter().line_segment([p2, end], stroke);
                    }
                } else {
                    // Fade out full checkmark directly when unchecking
                    ui.painter().line_segment([p1, p2], stroke);
                    ui.painter().line_segment([p2, p3], stroke);
                }
            }
        }
    });
    changed
}

fn fluent_pill<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    current: &mut T,
    value: T,
    label: &str,
    min_width: f32,
) -> bool {
    fluent_pill_enabled(ui, pal, current, value, label, min_width, true)
}

fn fluent_pill_enabled<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    current: &mut T,
    value: T,
    label: &str,
    min_width: f32,
    enabled: bool,
) -> bool {
    let selected = *current == value;
    let desired = egui::vec2(min_width, 26.0);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired, sense);
    let response = if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    let mut changed = false;
    if enabled && response.clicked() && !selected {
        *current = value;
        changed = true;
    }

    if ui.is_rect_visible(rect) {
        let sel_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("sel"), selected, 0.14);
        let hov_t = ui.ctx().animate_bool_with_time(
            response.id.with("hov"),
            enabled && response.hovered(),
            0.12,
        );
        let press_t = ui.ctx().animate_bool_with_time(
            response.id.with("prs"),
            enabled && response.is_pointer_button_down_on(),
            0.09,
        );

        let (fill, border, text_col) = if enabled {
            let unsel_bg = lerp_color(pal.btn_bg, pal.btn_hover_bg, hov_t);
            let unsel_bg = lerp_color(unsel_bg, pal.btn_pressed_bg, press_t);
            let sel_bg = lerp_color(pal.accent_base, pal.accent_hover, hov_t);
            let sel_bg = lerp_color(sel_bg, pal.accent_pressed, press_t);
            let fill = lerp_color(unsel_bg, sel_bg, sel_t);

            let unsel_border = lerp_color(pal.btn_border, pal.btn_hover_border, hov_t);
            let unsel_border = lerp_color(unsel_border, pal.btn_pressed_border, press_t);
            let sel_border = lerp_color(pal.accent_light, pal.accent_pressed, press_t);
            let border = lerp_color(unsel_border, sel_border, sel_t);

            let unsel_text = lerp_color(pal.text_secondary, pal.text_primary, hov_t);
            let text_col = lerp_color(unsel_text, pal.accent_text, sel_t);
            let text_col = lerp_color(text_col, pal.text_muted, press_t * 0.35);
            (fill, border, text_col)
        } else {
            let mut f = pal.sub_card_bg;
            if selected {
                f = lerp_color(f, pal.btn_bg, 0.5);
            }
            let b = pal.card_border;
            let t = pal.text_muted;
            (f, b, t)
        };

        let draw_rect = rect.shrink(press_t * 0.5);
        ui.painter()
            .rect(draw_rect, 6.0, fill, egui::Stroke::new(1.0_f32, border));

        let galley = ui.painter().layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(13.0),
            text_col,
        );
        let text_pos = draw_rect.center() - galley.size() * 0.5;
        ui.painter().galley(text_pos, galley, text_col);
    }
    changed
}

fn fluent_button(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    primary: bool,
    width: f32,
) -> bool {
    let desired = egui::vec2(width, 30.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if ui.is_rect_visible(rect) {
        let hov_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("btn_hov"), response.hovered(), 0.12);
        let press_t = ui.ctx().animate_bool_with_time(
            response.id.with("btn_prs"),
            response.is_pointer_button_down_on(),
            0.09,
        );

        let (fill, border, text_col) = if primary {
            let f = lerp_color(pal.accent_base, pal.accent_hover, hov_t);
            let f = lerp_color(f, pal.accent_pressed, press_t);
            let b = lerp_color(pal.accent_light, pal.accent_pressed, press_t);
            let t = lerp_color(pal.accent_text, pal.text_muted, press_t * 0.25);
            (f, b, t)
        } else {
            let f = lerp_color(pal.btn_bg, pal.btn_hover_bg, hov_t);
            let f = lerp_color(f, pal.btn_pressed_bg, press_t);
            let b = lerp_color(pal.btn_border, pal.btn_hover_border, hov_t);
            let b = lerp_color(b, pal.btn_pressed_border, press_t);
            let t = lerp_color(pal.text_primary, pal.text_muted, press_t * 0.35);
            (f, b, t)
        };

        let draw_rect = rect.shrink(press_t * 0.5);
        ui.painter()
            .rect(draw_rect, 6.0, fill, egui::Stroke::new(1.0_f32, border));

        let galley = ui.painter().layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(13.0),
            text_col,
        );
        ui.painter()
            .galley(draw_rect.center() - galley.size() * 0.5, galley, text_col);
    }

    response.clicked()
}

fn language_display_name(lang: Language) -> &'static str {
    match lang {
        Language::ZhTw => "繁體中文",
        Language::En => "English",
        Language::Ja => "日本語",
    }
}

fn fluent_language_dropdown(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    current: &mut Language,
    width: f32,
) {
    let popup_id = ui.make_persistent_id("lang_dropdown_popup");
    let is_open = ui.memory(|m| m.is_popup_open(popup_id));

    let desired = egui::vec2(width, 30.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    if ui.is_rect_visible(rect) {
        let hov_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("cbx_hov"), response.hovered(), 0.12);
        let press_t = ui.ctx().animate_bool_with_time(
            response.id.with("cbx_prs"),
            response.is_pointer_button_down_on(),
            0.09,
        );
        let open_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("cbx_open"), is_open, 0.14);

        let f = lerp_color(pal.btn_bg, pal.btn_hover_bg, hov_t);
        let fill = lerp_color(f, pal.btn_pressed_bg, press_t);

        let b = lerp_color(pal.btn_border, pal.btn_hover_border, hov_t);
        let b = lerp_color(b, pal.btn_pressed_border, press_t);
        let border = lerp_color(b, pal.accent_base, open_t * 0.75);

        let text_col = lerp_color(pal.text_primary, pal.text_muted, press_t * 0.35);

        let draw_rect = rect.shrink(press_t * 0.5);
        ui.painter()
            .rect(draw_rect, 6.0, fill, egui::Stroke::new(1.0_f32, border));

        let globe_col = lerp_color(pal.text_secondary, pal.text_primary, hov_t);
        draw_globe_icon(
            ui.painter(),
            egui::pos2(draw_rect.left() + 17.0, draw_rect.center().y),
            6.5,
            globe_col,
        );

        let galley = ui.painter().layout_no_wrap(
            language_display_name(*current).to_string(),
            egui::FontId::proportional(13.0),
            text_col,
        );
        let text_pos = egui::pos2(
            draw_rect.left() + 31.0,
            draw_rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().galley(text_pos, galley, text_col);

        // Animated chevron indicator
        let cx = draw_rect.right() - 15.0;
        let cy = draw_rect.center().y;
        let flip = 1.0 - 2.0 * open_t; // +1 down chevron, -1 up chevron
        let p1 = egui::pos2(cx - 4.0, cy - 1.8 * flip);
        let p2 = egui::pos2(cx, cy + 2.2 * flip);
        let p3 = egui::pos2(cx + 4.0, cy - 1.8 * flip);
        let chev_col = lerp_color(pal.text_secondary, pal.accent_label, open_t);
        ui.painter()
            .line_segment([p1, p2], egui::Stroke::new(1.5_f32, chev_col));
        ui.painter()
            .line_segment([p2, p3], egui::Stroke::new(1.5_f32, chev_col));
    }

    if is_open {
        let options = [Language::ZhTw, Language::En, Language::Ja];
        let item_h = 30.0;
        let pad = 4.0;
        let spacing_y = 2.0;
        let menu_h =
            (options.len() as f32) * item_h + ((options.len() - 1) as f32) * spacing_y + pad * 2.0;
        // Align bottom of flyout with bottom of trigger button so the open menu covers the original button
        let popup_pos = egui::pos2(rect.left(), (rect.bottom() - menu_h).max(8.0));

        let flyout_bg = if pal.is_dark {
            egui::Color32::from_rgb(0x2B, 0x2B, 0x33)
        } else {
            egui::Color32::from_rgb(0xFC, 0xFC, 0xFE)
        };
        let flyout_hover = if pal.is_dark {
            egui::Color32::from_rgb(0x3B, 0x3B, 0x46)
        } else {
            egui::Color32::from_rgb(0xF0, 0xF0, 0xF6)
        };
        let flyout_pressed = if pal.is_dark {
            egui::Color32::from_rgb(0x24, 0x24, 0x2B)
        } else {
            egui::Color32::from_rgb(0xE4, 0xE4, 0xEE)
        };
        let flyout_selected = if pal.is_dark {
            egui::Color32::from_rgb(0x34, 0x34, 0x3E)
        } else {
            egui::Color32::from_rgb(0xEA, 0xEA, 0xF2)
        };

        let mut any_item_clicked = false;
        let area_resp = egui::Area::new(popup_id)
            .order(egui::Order::Tooltip)
            .fixed_pos(popup_pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::none()
                    .fill(flyout_bg)
                    .stroke(egui::Stroke::new(1.0_f32, pal.btn_hover_border))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(pad))
                    .shadow(egui::Shadow {
                        offset: egui::vec2(0.0, 4.0),
                        blur: 14.0,
                        spread: 0.0,
                        color: egui::Color32::from_black_alpha(if pal.is_dark { 110 } else { 36 }),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - pad * 2.0);
                        ui.spacing_mut().item_spacing.y = spacing_y;

                        for opt in options {
                            let selected = *current == opt;
                            let item_desired = egui::vec2(ui.available_width(), item_h);
                            let (item_rect, item_resp) =
                                ui.allocate_exact_size(item_desired, egui::Sense::click());
                            let item_resp =
                                item_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                            if item_resp.clicked() {
                                *current = opt;
                                any_item_clicked = true;
                            }

                            if ui.is_rect_visible(item_rect) {
                                let sel_t = if selected { 1.0 } else { 0.0 };
                                let hov_t = ui.ctx().animate_bool_with_time(
                                    item_resp.id.with("opt_hov"),
                                    item_resp.hovered(),
                                    0.12,
                                );
                                let press_t = ui.ctx().animate_bool_with_time(
                                    item_resp.id.with("opt_prs"),
                                    item_resp.is_pointer_button_down_on(),
                                    0.09,
                                );

                                let rest_bg = lerp_color(flyout_bg, flyout_selected, sel_t);
                                let hov_bg = lerp_color(rest_bg, flyout_hover, hov_t);
                                let bg = lerp_color(hov_bg, flyout_pressed, press_t);

                                let draw_r = item_rect.shrink(press_t * 0.4);
                                ui.painter().rect_filled(draw_r, 5.0, bg);

                                if selected {
                                    let pill_h = 15.0 * (1.0 - 0.25 * press_t);
                                    let pill_rect = egui::Rect::from_center_size(
                                        egui::pos2(draw_r.left() + 4.5, draw_r.center().y),
                                        egui::vec2(3.0, pill_h),
                                    );
                                    let pill_col =
                                        lerp_color(pal.accent_base, pal.accent_pressed, press_t);
                                    ui.painter().rect_filled(pill_rect, 1.5, pill_col);
                                }

                                let text_col = lerp_color(
                                    pal.text_secondary,
                                    pal.text_primary,
                                    sel_t.max(hov_t),
                                );
                                let text_col = lerp_color(text_col, pal.text_muted, press_t * 0.4);
                                let galley = ui.painter().layout_no_wrap(
                                    language_display_name(opt).to_string(),
                                    egui::FontId::proportional(13.0),
                                    text_col,
                                );
                                let text_pos = egui::pos2(
                                    draw_r.left() + 14.0,
                                    draw_r.center().y - galley.size().y * 0.5,
                                );
                                ui.painter().galley(text_pos, galley, text_col);
                            }
                        }
                    });
            });

        if any_item_clicked {
            ui.memory_mut(|m| m.close_popup());
        } else if !response.clicked() && ui.input(|i| i.pointer.any_pressed()) {
            let inside = ui
                .input(|i| i.pointer.interact_pos())
                .map(|p| area_resp.response.rect.contains(p) || rect.contains(p))
                .unwrap_or(false);
            if !inside {
                ui.memory_mut(|m| m.close_popup());
            }
        }
    }
}

fn read_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let mut result = None;
        if let Ok(handle) = GetClipboardData(13) {
            // CF_UNICODETEXT = 13
            let hmem = windows::Win32::Foundation::HGLOBAL(handle.0);
            let ptr = GlobalLock(hmem) as *const u16;
            if !ptr.is_null() {
                let mut len = 0usize;
                while len < 256 && *ptr.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                result = Some(String::from_utf16_lossy(slice));
                let _ = GlobalUnlock(hmem);
            }
        }
        let _ = CloseClipboard();
        result
    }
}

fn parse_color_text(text: &str) -> Option<[u8; 3]> {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix('#').or(Some(t)) {
        if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b]);
        }
        if hex.len() == 3 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some([r, g, b]);
        }
    }
    let parts: Vec<&str> = t
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() == 3 {
        let r = parts[0].parse::<u8>().ok()?;
        let g = parts[1].parse::<u8>().ok()?;
        let b = parts[2].parse::<u8>().ok()?;
        return Some([r, g, b]);
    }
    None
}

fn rgb_to_hsv(rgb: [u8; 3]) -> [f32; 3] {
    let r = rgb[0] as f32 / 255.0;
    let g = rgb[1] as f32 / 255.0;
    let b = rgb[2] as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if (max - g).abs() < 1e-6 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max < 1e-6 { 0.0 } else { d / max };
    [h, s, max]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    [
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

fn fluent_color_picker(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    s: &Strings,
    id_salt: &str,
    rgb: &mut [u8; 3],
    enabled: bool,
) {
    let popup_id = ui.make_persistent_id(id_salt).with("color_popup");
    let is_open = enabled && ui.memory(|m| m.is_popup_open(popup_id));

    let desired = egui::vec2(46.0, 24.0);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired, sense);
    let response = if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };

    if enabled && response.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    if ui.is_rect_visible(rect) {
        let hov_t = ui.ctx().animate_bool_with_time(
            response.id.with("cp_hov"),
            enabled && response.hovered(),
            0.12,
        );
        let press_t = ui.ctx().animate_bool_with_time(
            response.id.with("cp_prs"),
            enabled && response.is_pointer_button_down_on(),
            0.09,
        );
        let open_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("cp_open"), is_open, 0.14);

        let draw_rect = rect.shrink(press_t * 0.5);
        let outer_b = if enabled {
            let b = lerp_color(pal.btn_border, pal.btn_hover_border, hov_t);
            let b = lerp_color(b, pal.btn_pressed_border, press_t);
            lerp_color(b, pal.accent_base, open_t * 0.8)
        } else {
            pal.card_border
        };
        let frame_bg = if enabled {
            let bg = lerp_color(pal.btn_bg, pal.btn_hover_bg, hov_t);
            lerp_color(bg, pal.btn_pressed_bg, press_t)
        } else {
            pal.sub_card_bg
        };

        ui.painter()
            .rect(draw_rect, 6.0, frame_bg, egui::Stroke::new(1.0_f32, outer_b));

        let swatch_col = if enabled {
            egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
        } else {
            egui::Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 95)
        };
        ui.painter()
            .rect_filled(draw_rect.shrink(3.0), 3.5, swatch_col);
    }

    if is_open {
        let hsv_id = popup_id.with("hsv");
        let mut hsv = ui
            .ctx()
            .data_mut(|d| d.get_temp::<[f32; 3]>(hsv_id))
            .unwrap_or_else(|| rgb_to_hsv(*rgb));
        if hsv_to_rgb(hsv[0], hsv[1], hsv[2]) != *rgb {
            let from_rgb = rgb_to_hsv(*rgb);
            // Preserve previous hue if current RGB is grayscale
            if from_rgb[1] < 0.01 {
                hsv[1] = from_rgb[1];
                hsv[2] = from_rgb[2];
            } else {
                hsv = from_rgb;
            }
        }

        let flyout_bg = if pal.is_dark {
            egui::Color32::from_rgb(0x2B, 0x2B, 0x33)
        } else {
            egui::Color32::from_rgb(0xFC, 0xFC, 0xFE)
        };

        let popup_w = 216.0;
        let screen_r = ui.ctx().screen_rect();
        let px = rect.left().min(screen_r.right() - popup_w - 24.0).max(8.0);
        let py = if rect.bottom() + 270.0 > screen_r.bottom() {
            (rect.top() - 266.0).max(8.0)
        } else {
            rect.bottom() + 4.0
        };

        let area_resp = egui::Area::new(popup_id)
            .order(egui::Order::Tooltip)
            .fixed_pos(egui::pos2(px, py))
            .show(ui.ctx(), |ui| {
                egui::Frame::none()
                    .fill(flyout_bg)
                    .stroke(egui::Stroke::new(1.0_f32, pal.btn_hover_border))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(10.0))
                    .shadow(egui::Shadow {
                        offset: egui::vec2(0.0, 4.0),
                        blur: 14.0,
                        spread: 0.0,
                        color: egui::Color32::from_black_alpha(if pal.is_dark { 115 } else { 38 }),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_w);

                        // Top bar: color preview + Hex + Copy + Paste
                        let hex_str = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                        ui.horizontal(|ui| {
                            let (chip_rect, _) =
                                ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                            ui.painter().rect(
                                chip_rect,
                                4.0,
                                egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                                egui::Stroke::new(1.0_f32, pal.btn_hover_border),
                            );
                            ui.label(
                                egui::RichText::new(&hex_str)
                                    .monospace()
                                    .strong()
                                    .color(pal.text_primary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if fluent_button(ui, pal, s.btn_paste, false, 52.0) {
                                        if let Some(clip) = read_clipboard_text() {
                                            if let Some(parsed) = parse_color_text(&clip) {
                                                *rgb = parsed;
                                                hsv = rgb_to_hsv(*rgb);
                                            }
                                        }
                                    }
                                    if fluent_button(ui, pal, s.btn_copy, false, 52.0) {
                                        ui.ctx().copy_text(hex_str.clone());
                                    }
                                },
                            );
                        });

                        ui.add_space(6.0);

                        // 2D Saturation / Value gradient square
                        let sv_size = egui::vec2(popup_w, 168.0);
                        let (sv_rect, sv_resp) =
                            ui.allocate_exact_size(sv_size, egui::Sense::click_and_drag());
                        if sv_resp.dragged() || sv_resp.clicked() {
                            if let Some(pos) = sv_resp.interact_pointer_pos() {
                                hsv[1] = ((pos.x - sv_rect.left()) / sv_rect.width()).clamp(0.0, 1.0);
                                hsv[2] =
                                    1.0 - ((pos.y - sv_rect.top()) / sv_rect.height()).clamp(0.0, 1.0);
                                *rgb = hsv_to_rgb(hsv[0], hsv[1], hsv[2]);
                            }
                        }
                        if ui.is_rect_visible(sv_rect) {
                            let mut mesh = egui::Mesh::default();
                            let cols = 12usize;
                            let rows = 12usize;
                            for r in 0..=rows {
                                let v_frac = 1.0 - (r as f32 / rows as f32);
                                let y = egui::lerp(sv_rect.top()..=sv_rect.bottom(), r as f32 / rows as f32);
                                for c in 0..=cols {
                                    let s_frac = c as f32 / cols as f32;
                                    let x = egui::lerp(sv_rect.left()..=sv_rect.right(), s_frac);
                                    let crgb = hsv_to_rgb(hsv[0], s_frac, v_frac);
                                    mesh.colored_vertex(
                                        egui::pos2(x, y),
                                        egui::Color32::from_rgb(crgb[0], crgb[1], crgb[2]),
                                    );
                                }
                            }
                            for r in 0..rows {
                                for c in 0..cols {
                                    let i = (r * (cols + 1) + c) as u32;
                                    let stride = (cols + 1) as u32;
                                    mesh.add_triangle(i, i + 1, i + stride);
                                    mesh.add_triangle(i + 1, i + stride + 1, i + stride);
                                }
                            }
                            ui.painter().add(egui::Shape::mesh(mesh));
                            ui.painter().rect_stroke(
                                sv_rect,
                                2.0,
                                egui::Stroke::new(1.0_f32, pal.btn_border),
                            );

                            let kx = egui::lerp(sv_rect.left()..=sv_rect.right(), hsv[1]);
                            let ky = egui::lerp(sv_rect.top()..=sv_rect.bottom(), 1.0 - hsv[2]);
                            ui.painter().circle_stroke(
                                egui::pos2(kx, ky),
                                6.0,
                                egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
                            );
                            ui.painter().circle_stroke(
                                egui::pos2(kx, ky),
                                7.2,
                                egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(150)),
                            );
                        }

                        ui.add_space(6.0);

                        // 1D Hue bar
                        let hue_size = egui::vec2(popup_w, 16.0);
                        let (hue_rect, hue_resp) =
                            ui.allocate_exact_size(hue_size, egui::Sense::click_and_drag());
                        if hue_resp.dragged() || hue_resp.clicked() {
                            if let Some(pos) = hue_resp.interact_pointer_pos() {
                                hsv[0] = ((pos.x - hue_rect.left()) / hue_rect.width()).clamp(0.0, 1.0);
                                *rgb = hsv_to_rgb(hsv[0], hsv[1], hsv[2]);
                            }
                        }
                        if ui.is_rect_visible(hue_rect) {
                            let mut mesh = egui::Mesh::default();
                            let stops = 6usize;
                            for i in 0..=stops {
                                let t = i as f32 / stops as f32;
                                let x = egui::lerp(hue_rect.left()..=hue_rect.right(), t);
                                let hrgb = hsv_to_rgb(t, 1.0, 1.0);
                                let col = egui::Color32::from_rgb(hrgb[0], hrgb[1], hrgb[2]);
                                mesh.colored_vertex(egui::pos2(x, hue_rect.top()), col);
                                mesh.colored_vertex(egui::pos2(x, hue_rect.bottom()), col);
                            }
                            for i in 0..stops {
                                let base = (i * 2) as u32;
                                mesh.add_triangle(base, base + 2, base + 1);
                                mesh.add_triangle(base + 2, base + 3, base + 1);
                            }
                            ui.painter().add(egui::Shape::mesh(mesh));
                            ui.painter().rect_stroke(
                                hue_rect,
                                2.0,
                                egui::Stroke::new(1.0_f32, pal.btn_border),
                            );

                            let hx = egui::lerp(hue_rect.left()..=hue_rect.right(), hsv[0]);
                            let h_handle = egui::Rect::from_center_size(
                                egui::pos2(hx, hue_rect.center().y),
                                egui::vec2(6.0, hue_rect.height() + 2.0),
                            );
                            ui.painter().rect(
                                h_handle,
                                2.0,
                                egui::Color32::WHITE,
                                egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(150)),
                            );
                        }

                        ui.add_space(6.0);

                        // RGB numeric readout
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("R {}", rgb[0]))
                                    .small()
                                    .monospace()
                                    .color(pal.text_secondary),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(format!("G {}", rgb[1]))
                                    .small()
                                    .monospace()
                                    .color(pal.text_secondary),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(format!("B {}", rgb[2]))
                                    .small()
                                    .monospace()
                                    .color(pal.text_secondary),
                            );
                        });
                    });
            });

        ui.ctx().data_mut(|d| d.insert_temp(hsv_id, hsv));

        if !response.clicked() && ui.input(|i| i.pointer.any_pressed()) {
            let inside = ui
                .input(|i| i.pointer.interact_pos())
                .map(|p| area_resp.response.rect.contains(p) || rect.contains(p))
                .unwrap_or(false);
            if !inside {
                ui.memory_mut(|m| m.close_popup());
            }
        }
    }
}

fn toggle_switch(ui: &mut egui::Ui, pal: &ThemePalette, on: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(42.0, 21.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let how_on = ui
            .ctx()
            .animate_bool_with_time(response.id, *on, 0.14);
        let hov_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("sw_hov"), response.hovered(), 0.12);
        let press_t = ui.ctx().animate_bool_with_time(
            response.id.with("sw_prs"),
            response.is_pointer_button_down_on(),
            0.09,
        );
        let radius = 0.5 * rect.height();

        let off_fill = lerp_color(pal.btn_bg, pal.btn_hover_bg, hov_t);
        let off_fill = lerp_color(off_fill, pal.btn_pressed_bg, press_t);
        let on_fill = lerp_color(pal.accent_base, pal.accent_hover, hov_t);
        let on_fill = lerp_color(on_fill, pal.accent_pressed, press_t);
        let bg_fill = lerp_color(off_fill, on_fill, how_on);

        let off_border = lerp_color(pal.btn_hover_border, pal.btn_pressed_border, press_t);
        let on_border = lerp_color(pal.accent_light, pal.accent_pressed, press_t);
        let border_col = lerp_color(off_border, on_border, how_on);

        ui.painter()
            .rect(rect, radius, bg_fill, egui::Stroke::new(1.0_f32, border_col));
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let knob_r = (0.35 + 0.04 * hov_t + 0.02 * press_t) * rect.height();
        let knob_stretch = press_t * 2.5;
        let knob_rect = egui::Rect::from_center_size(
            egui::pos2(circle_x, rect.center().y),
            egui::vec2(knob_r * 2.0 + knob_stretch, knob_r * 2.0),
        );
        let knob_col = lerp_color(pal.text_secondary, pal.accent_text, how_on);
        ui.painter().rect_filled(knob_rect, knob_r, knob_col);
    }
    response
}

fn draw_scale_mode_diagram(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    mode: ScaleMode,
    fg_rgb: [u8; 3],
    outer_fill_mode: OuterFillMode,
    outer_fill_color: [u8; 3],
    use_16_9: bool,
) {
    let screen_size = egui::vec2(176.0, 99.0); // 16:9 monitor preview
    let (rect, _) = ui.allocate_exact_size(screen_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 5.0, pal.sub_card_bg);
    let effective_fill = if mode == ScaleMode::Stretch {
        OuterFillMode::None
    } else {
        outer_fill_mode
    };
    let wing_color = match effective_fill {
        OuterFillMode::Auto => pal.diagram_wing_on,
        OuterFillMode::Solid => egui::Color32::from_rgb(
            outer_fill_color[0],
            outer_fill_color[1],
            outer_fill_color[2],
        ),
        OuterFillMode::None => pal.diagram_wing_off,
    };
    painter.rect_filled(rect.shrink(3.0), 3.0, wing_color);

    let video_rect = if use_16_9 {
        match mode {
            ScaleMode::Fit | ScaleMode::Fill | ScaleMode::Stretch => rect,
            ScaleMode::Original => {
                egui::Rect::from_center_size(rect.center(), egui::vec2(106.0, 59.6))
            }
        }
    } else {
        match mode {
            ScaleMode::Fit => {
                let w = rect.height() * (4.0 / 3.0);
                egui::Rect::from_center_size(rect.center(), egui::vec2(w, rect.height()))
            }
            ScaleMode::Fill => {
                let h = rect.width() * (3.0 / 4.0);
                egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), h))
            }
            ScaleMode::Stretch => rect,
            ScaleMode::Original => {
                egui::Rect::from_center_size(rect.center(), egui::vec2(68.0, 51.0))
            }
        }
    };

    painter.rect_filled(video_rect, 0.0, pal.diagram_video_bg);
    painter.rect_stroke(video_rect, 0.0, egui::Stroke::new(1.2_f32, pal.accent_base));

    let fg = egui::Color32::from_rgb(fg_rgb[0], fg_rgb[1], fg_rgb[2]);
    let rx = video_rect.width() * 0.16;
    let ry = video_rect.height() * 0.22;
    let c = video_rect.center();
    if (rx - ry).abs() < 2.0 {
        painter.circle_filled(c, rx, fg);
    } else {
        painter.circle_filled(c - egui::vec2(rx * 0.22, 0.0), ry.min(rx * 0.9), fg);
        painter.circle_filled(c + egui::vec2(rx * 0.22, 0.0), ry.min(rx * 0.9), fg);
    }

    painter.rect_stroke(rect, 5.0, egui::Stroke::new(1.5_f32, pal.card_border));
}

struct SettingsApp {
    cfg: Config,
    tab: ActiveTab,
    nav_prev_tab: ActiveTab,
    nav_switch_time: Option<Instant>,
    content_prev_tab: ActiveTab,
    content_switch_time: Option<Instant>,
    status_msg: String,
    backdrop_applied: bool,
    style_check_frames: u8,
    pal: ThemePalette,
    last_theme_check: Instant,
    title_icon: Option<egui::TextureHandle>,
}

impl SettingsApp {
    fn draw_general_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        let pal = self.pal;

        fluent_card(ui, &pal, s.card_bg_title, |ui| {
            let pill_gap = ui.spacing().item_spacing.x;
            let pill_w = ((ui.available_width() - pill_gap * 3.0) / 4.0).max(96.0);
            ui.horizontal(|ui| {
                fluent_pill(
                    ui,
                    &pal,
                    &mut self.cfg.background_mode,
                    BackgroundMode::Blur,
                    s.bg_blur,
                    pill_w,
                );
                fluent_pill(
                    ui,
                    &pal,
                    &mut self.cfg.background_mode,
                    BackgroundMode::LiquidGlass,
                    s.bg_liquid_glass,
                    pill_w,
                );
                fluent_pill(
                    ui,
                    &pal,
                    &mut self.cfg.background_mode,
                    BackgroundMode::Transparent,
                    s.bg_clear,
                    pill_w,
                );
                fluent_pill(
                    ui,
                    &pal,
                    &mut self.cfg.background_mode,
                    BackgroundMode::Solid,
                    s.bg_solid,
                    pill_w,
                );
            });

            ui.add_space(6.0);

            if matches!(
                self.cfg.background_mode,
                BackgroundMode::Blur | BackgroundMode::LiquidGlass
            ) {
                let mut blur_f = self.cfg.blur_radius as f32;
                if fluent_slider(ui, &pal, s.blur_radius, &mut blur_f, 0.0..=60.0, "") {
                    self.cfg.blur_radius = blur_f.round() as u32;
                }
            }

            if self.cfg.background_mode == BackgroundMode::LiquidGlass {
                let mut glass_pct = self.cfg.glass_strength * 100.0;
                if fluent_slider(
                    ui,
                    &pal,
                    s.glass_strength,
                    &mut glass_pct,
                    0.0..=200.0,
                    "%",
                ) {
                    self.cfg.glass_strength = glass_pct / 100.0;
                }
            }

            if self.cfg.background_mode != BackgroundMode::Solid {
                let mut dim_pct = self.cfg.bg_dim_opacity * 100.0;
                if fluent_slider(ui, &pal, s.bg_dim, &mut dim_pct, 0.0..=90.0, "%") {
                    self.cfg.bg_dim_opacity = dim_pct / 100.0;
                }
            } else {
                labeled_row(ui, &pal, s.bg_color, true, |ui| {
                    fluent_color_picker(
                        ui,
                        &pal,
                        s,
                        "solid_bg_color",
                        &mut self.cfg.solid_bg_color,
                        true,
                    );
                });
            }
        });

        ui.add_space(10.0);

        fluent_card(ui, &pal, s.card_display_title, |ui| {
            labeled_row(ui, &pal, s.fg_color, true, |ui| {
                fluent_color_picker(
                    ui,
                    &pal,
                    s,
                    "fg_color",
                    &mut self.cfg.foreground_color,
                    true,
                );
                ui.add_space(18.0);
                fluent_checkbox(
                    ui,
                    &pal,
                    &mut self.cfg.invert_silhouette,
                    s.invert_silhouette,
                );
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(s.scale_mode_label).color(pal.text_secondary));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let mode_w = 84.0;
                        fluent_pill(
                            ui,
                            &pal,
                            &mut self.cfg.scale_mode,
                            ScaleMode::Fit,
                            s.scale_fit,
                            mode_w,
                        );
                        fluent_pill(
                            ui,
                            &pal,
                            &mut self.cfg.scale_mode,
                            ScaleMode::Fill,
                            s.scale_fill,
                            mode_w,
                        );
                        fluent_pill(
                            ui,
                            &pal,
                            &mut self.cfg.scale_mode,
                            ScaleMode::Stretch,
                            s.scale_stretch,
                            mode_w,
                        );
                        fluent_pill(
                            ui,
                            &pal,
                            &mut self.cfg.scale_mode,
                            ScaleMode::Original,
                            s.scale_original,
                            mode_w,
                        );
                    });
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    draw_scale_mode_diagram(
                        ui,
                        &pal,
                        self.cfg.scale_mode,
                        self.cfg.foreground_color,
                        self.cfg.outer_fill_mode,
                        self.cfg.outer_fill_color,
                        self.cfg.use_16_9,
                    );
                });
            });

            ui.add_space(8.0);
            labeled_row(ui, &pal, s.video_aspect_label, true, |ui| {
                toggle_switch(ui, &pal, &mut self.cfg.use_16_9);
                let fmt_text = if self.cfg.use_16_9 {
                    s.video_16_9
                } else {
                    s.video_4_3
                };
                ui.label(egui::RichText::new(fmt_text).color(if self.cfg.use_16_9 {
                    pal.accent_label
                } else {
                    pal.text_primary
                }));
            });

            ui.add_space(4.0);
            let fill_enabled = self.cfg.scale_mode != ScaleMode::Stretch;
            labeled_row(ui, &pal, s.fill_mode_label, fill_enabled, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                fluent_pill_enabled(
                    ui,
                    &pal,
                    &mut self.cfg.outer_fill_mode,
                    OuterFillMode::Auto,
                    s.fill_auto,
                    86.0,
                    fill_enabled,
                );
                fluent_pill_enabled(
                    ui,
                    &pal,
                    &mut self.cfg.outer_fill_mode,
                    OuterFillMode::Solid,
                    s.fill_solid,
                    86.0,
                    fill_enabled,
                );
                fluent_pill_enabled(
                    ui,
                    &pal,
                    &mut self.cfg.outer_fill_mode,
                    OuterFillMode::None,
                    s.fill_none,
                    86.0,
                    fill_enabled,
                );
                if self.cfg.outer_fill_mode == OuterFillMode::Solid {
                    ui.add_space(4.0);
                    fluent_color_picker(
                        ui,
                        &pal,
                        s,
                        "outer_fill_color",
                        &mut self.cfg.outer_fill_color,
                        fill_enabled,
                    );
                }
            });

            ui.add_space(4.0);
            labeled_row(ui, &pal, s.monitors_label, true, |ui| {
                toggle_switch(ui, &pal, &mut self.cfg.all_monitors);
                let mode_text = if self.cfg.all_monitors {
                    s.monitors_all
                } else {
                    s.monitors_primary
                };
                ui.label(
                    egui::RichText::new(mode_text).color(if self.cfg.all_monitors {
                        pal.accent_label
                    } else {
                        pal.text_primary
                    }),
                );
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if fluent_button(ui, &pal, s.btn_custom_video, false, 152.0) {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_ascii_lowercase());
                        if matches!(ext.as_deref(), Some("mp4" | "m4v" | "mov")) {
                            self.cfg.custom_video_path = Some(path.to_string_lossy().into_owned());
                        } else {
                            rfd::MessageDialog::new()
                                .set_level(rfd::MessageLevel::Warning)
                                .set_title(s.warn_video_title)
                                .set_description(s.warn_video_body)
                                .set_buttons(rfd::MessageButtons::Ok)
                                .show();
                        }
                    }
                }
                if self.cfg.custom_video_path.is_some()
                    && fluent_button(ui, &pal, s.btn_restore_video, false, 118.0)
                {
                    self.cfg.custom_video_path = None;
                }
                if let Some(ref current_video) = self.cfg.custom_video_path {
                    ui.label(
                        egui::RichText::new(current_video)
                            .small()
                            .color(pal.text_muted),
                    );
                }
            });
        });

        ui.add_space(10.0);

        fluent_card(ui, &pal, s.card_audio_title, |ui| {
            labeled_row(ui, &pal, s.bg_music_label, true, |ui| {
                toggle_switch(ui, &pal, &mut self.cfg.audio_enabled);
                ui.label(
                    egui::RichText::new(if self.cfg.audio_enabled {
                        s.audio_on
                    } else {
                        s.audio_off
                    })
                    .color(if self.cfg.audio_enabled {
                        pal.accent_label
                    } else {
                        pal.text_muted
                    }),
                );
            });

            if self.cfg.audio_enabled {
                ui.add_space(4.0);
                let mut vol_pct = self.cfg.audio_volume * 100.0;
                if fluent_slider(ui, &pal, s.volume_label, &mut vol_pct, 0.0..=100.0, "%") {
                    self.cfg.audio_volume = vol_pct / 100.0;
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if fluent_button(ui, &pal, s.btn_custom_audio, false, 148.0) {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(s.dialog_audio_filter, &["mp3", "wav", "ogg"])
                            .pick_file()
                        {
                            self.cfg.custom_audio_path = Some(path.to_string_lossy().into_owned());
                        }
                    }
                    if self.cfg.custom_audio_path.is_some()
                        && fluent_button(ui, &pal, s.btn_restore_audio, false, 118.0)
                    {
                        self.cfg.custom_audio_path = None;
                    }
                    if let Some(ref current_audio) = self.cfg.custom_audio_path {
                        ui.label(
                            egui::RichText::new(current_audio)
                                .small()
                                .color(pal.text_muted),
                        );
                    }
                });
            }
        });
    }

    fn draw_bottom_actions(&mut self, ui: &mut egui::Ui, s: &Strings) {
        let pal = self.pal;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if fluent_button(ui, &pal, s.btn_preview, false, 96.0) {
                if let Err(e) = self.cfg.save() {
                    self.status_msg = format!("{}: {e}", s.status_save_err);
                } else if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("/s").spawn();
                }
            }

            if fluent_button(ui, &pal, s.btn_reset, false, 108.0) {
                let keep_lang = self.cfg.language;
                self.cfg = Config::default();
                self.cfg.language = keep_lang;
                self.status_msg = s.status_reset.to_string();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if fluent_button(ui, &pal, s.btn_save_close, true, 112.0) {
                    match self.cfg.save() {
                        Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(e) => self.status_msg = format!("{}: {e}", s.status_save_err),
                    }
                }
                if fluent_button(ui, &pal, s.btn_apply, false, 76.0) {
                    match self.cfg.save() {
                        Ok(()) => self.status_msg = s.status_saved.to_string(),
                        Err(e) => self.status_msg = format!("{}: {e}", s.status_save_err),
                    }
                }
            });
        });

        if !self.status_msg.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(&self.status_msg)
                    .small()
                    .color(pal.status_ok),
            );
        }
    }

    fn draw_about_tab(&self, ui: &mut egui::Ui, s: &Strings) {
        let pal = self.pal;

        fluent_card(ui, &pal, "Bad Apple!! rs", |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .strong()
                        .color(pal.accent_label),
                );
                ui.label(
                    egui::RichText::new("• Windows 11 Screensaver (.scr)")
                        .color(pal.text_secondary),
                );
            });
        });

        ui.add_space(10.0);

        fluent_card(ui, &pal, s.about_arch_title, |ui| {
            ui.label(egui::RichText::new(s.about_arch_1).color(pal.text_secondary));
            ui.label(egui::RichText::new(s.about_arch_2).color(pal.text_secondary));
            ui.label(egui::RichText::new(s.about_arch_3).color(pal.text_secondary));
        });

        ui.add_space(10.0);

        fluent_card(ui, &pal, s.about_feat_title, |ui| {
            ui.label(egui::RichText::new(s.about_feat_1).color(pal.text_secondary));
            ui.label(egui::RichText::new(s.about_feat_2).color(pal.text_secondary));
            ui.label(egui::RichText::new(s.about_feat_3).color(pal.text_secondary));
        });

        ui.add_space(10.0);

        fluent_card(ui, &pal, s.about_credits_title, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(s.about_pv_label).strong());
                ui.hyperlink_to(
                    s.about_pv_title,
                    "https://www.nicovideo.jp/watch/sm8628149",
                );
            });
            ui.label(egui::RichText::new(s.about_authors).color(pal.text_secondary));
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(s.about_src_4_3).strong());
                ui.hyperlink_to(
                    "https://www.bilibili.com/video/BV1x5411o7Kn/",
                    "https://www.bilibili.com/video/BV1x5411o7Kn/",
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(s.about_src_16_9).strong());
                ui.hyperlink_to(
                    "https://www.youtube.com/watch?v=DeYs_ZTzMzA",
                    "https://www.youtube.com/watch?v=DeYs_ZTzMzA",
                );
            });
        });

        ui.add_space(10.0);

        fluent_card(ui, &pal, s.about_license_title, |ui| {
            ui.label(egui::RichText::new(s.about_license_code).color(pal.text_secondary));
            ui.hyperlink_to(
                "GNU General Public License v3.0 (https://www.gnu.org/licenses/gpl-3.0.html)",
                "https://www.gnu.org/licenses/gpl-3.0.html",
            );
            ui.add_space(2.0);
            ui.label(egui::RichText::new(s.about_license_media).color(pal.text_secondary));
            ui.label(egui::RichText::new(s.about_license_warranty).color(pal.text_muted));
        });
    }

    fn draw_tab_panel(
        &mut self,
        ui: &mut egui::Ui,
        tab: ActiveTab,
        s: &Strings,
        y_offset: f32,
    ) {
        let pal = self.pal;
        if y_offset > 0.1 {
            ui.add_space(y_offset);
        }
        let heading = match tab {
            ActiveTab::General => s.nav_settings,
            ActiveTab::About => s.nav_about,
        };
        ui.horizontal(|ui| {
            let (icon_rect, _) =
                ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
            match tab {
                ActiveTab::General => {
                    draw_gear_icon(ui.painter(), icon_rect.center(), 8.5, pal.text_primary);
                }
                ActiveTab::About => {
                    draw_info_icon(ui.painter(), icon_rect.center(), 9.0, pal.text_primary);
                }
            }
            ui.label(
                egui::RichText::new(heading)
                    .strong()
                    .size(19.0)
                    .color(pal.text_primary),
            );
        });
        ui.add_space(10.0);

        let reserve_bottom = if tab == ActiveTab::General {
            if self.status_msg.is_empty() { 44.0 } else { 66.0 }
        } else {
            0.0
        };
        let scroll_h = (ui.available_height() - reserve_bottom).max(160.0);
        egui::ScrollArea::vertical()
            .id_salt(match tab {
                ActiveTab::General => "scroll_general",
                ActiveTab::About => "scroll_about",
            })
            .max_height(scroll_h)
            .show(ui, |ui| match tab {
                ActiveTab::General => self.draw_general_tab(ui, s),
                ActiveTab::About => self.draw_about_tab(ui, s),
            });

        if tab == ActiveTab::General {
            self.draw_bottom_actions(ui, s);
        }
    }

    fn draw_titlebar(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if self.title_icon.is_none() {
            let img = egui::ColorImage::from_rgba_unmultiplied([64, 64], ICON_RGBA_64);
            self.title_icon =
                Some(ctx.load_texture("title_icon", img, egui::TextureOptions::LINEAR));
        }

        let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        egui::TopBottomPanel::top("custom_titlebar")
            .exact_height(32.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .fill(pal.sidebar_bg)
                    .inner_margin(egui::Margin::symmetric(0.0, 0.0)),
            )
            .show(ctx, |ui| {
                let full_rect = ui.max_rect();
                let btn_w = 46.0;
                let controls_w = btn_w * 3.0;
                let drag_rect = egui::Rect::from_min_max(
                    full_rect.min,
                    egui::pos2(full_rect.right() - controls_w, full_rect.bottom()),
                );
                let drag_resp = ui.interact(
                    drag_rect,
                    ui.make_persistent_id("titlebar_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                }

                // App icon + title on left (no divider line so titlebar & sidebar are seamless)
                if let Some(tex) = &self.title_icon {
                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(full_rect.left() + 20.0, full_rect.center().y),
                        egui::vec2(16.0, 16.0),
                    );
                    ui.painter().image(
                        tex.id(),
                        icon_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                let title_galley = ui.painter().layout_no_wrap(
                    "Bad Apple!! rs".to_string(),
                    egui::FontId::proportional(12.5),
                    pal.text_secondary,
                );
                ui.painter().galley(
                    egui::pos2(
                        full_rect.left() + 34.0,
                        full_rect.center().y - title_galley.size().y * 0.5,
                    ),
                    title_galley,
                    pal.text_secondary,
                );

                // Caption buttons on right: Minimize, Maximize/Restore, Close
                let draw_cap_btn = |ui: &mut egui::Ui, idx: usize, is_close: bool| -> egui::Response {
                    let r = egui::Rect::from_min_size(
                        egui::pos2(full_rect.right() - btn_w * ((3 - idx) as f32), full_rect.top()),
                        egui::vec2(btn_w, full_rect.height()),
                    );
                    let resp = ui.interact(
                        r,
                        ui.make_persistent_id(("cap_btn", idx)),
                        egui::Sense::click(),
                    );
                    let hov = resp.hovered();
                    let prs = resp.is_pointer_button_down_on();
                    let bg = if is_close {
                        if prs {
                            egui::Color32::from_rgb(0xA6, 0x24, 0x17)
                        } else if hov {
                            egui::Color32::from_rgb(0xC4, 0x2B, 0x1C)
                        } else {
                            egui::Color32::TRANSPARENT
                        }
                    } else if prs {
                        if pal.is_dark {
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 28)
                        }
                    } else if hov {
                        if pal.is_dark {
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 26)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 16)
                        }
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    if bg != egui::Color32::TRANSPARENT {
                        ui.painter().rect_filled(r, 0.0, bg);
                    }
                    let fg = if is_close && (hov || prs) {
                        egui::Color32::WHITE
                    } else {
                        pal.text_secondary
                    };
                    let c = r.center();
                    let stroke = egui::Stroke::new(1.2_f32, fg);
                    match idx {
                        0 => {
                            ui.painter().line_segment(
                                [egui::pos2(c.x - 5.0, c.y), egui::pos2(c.x + 5.0, c.y)],
                                stroke,
                            );
                        }
                        1 => {
                            let sq = egui::Rect::from_center_size(c, egui::vec2(9.0, 9.0));
                            ui.painter().rect_stroke(sq, 0.0, stroke);
                        }
                        _ => {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(c.x - 4.5, c.y - 4.5),
                                    egui::pos2(c.x + 4.5, c.y + 4.5),
                                ],
                                stroke,
                            );
                            ui.painter().line_segment(
                                [
                                    egui::pos2(c.x + 4.5, c.y - 4.5),
                                    egui::pos2(c.x - 4.5, c.y + 4.5),
                                ],
                                stroke,
                            );
                        }
                    }
                    resp
                };

                if draw_cap_btn(ui, 0, false).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                if draw_cap_btn(ui, 1, false).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                }
                if draw_cap_btn(ui, 2, true).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }
}

impl eframe::App for SettingsApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparent framebuffer so Windows 11 DWM Acrylic blur shows through sidebar & titlebar
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.backdrop_applied {
            self.backdrop_applied = enable_win11_backdrop_blur(self.pal.is_dark);
        }
        if self.style_check_frames < 12 {
            self.style_check_frames += 1;
            ensure_borderless_window_style();
            if self.style_check_frames < 4 {
                ctx.request_repaint();
            }
        }

        if self.last_theme_check.elapsed().as_secs_f32() >= 0.8 {
            self.last_theme_check = Instant::now();
            let (is_dark, accent) = query_system_theme();
            if is_dark != self.pal.is_dark || accent != self.pal.accent_base {
                self.pal = ThemePalette::from_system(is_dark, accent);
                apply_fluent_theme(ctx, &self.pal);
                enable_win11_backdrop_blur(self.pal.is_dark);
            }
        }

        let pal = self.pal;
        let s = strings(self.cfg.language);

        // Borderless edge resize support
        let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if !is_max {
            let screen_rect = ctx.screen_rect();
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let m = 5.0;
                let left = pos.x <= screen_rect.left() + m;
                let right = pos.x >= screen_rect.right() - m;
                let top = pos.y <= screen_rect.top() + m;
                let bottom = pos.y >= screen_rect.bottom() - m;
                let dir = match (left, right, top, bottom) {
                    (true, _, true, _) => Some((egui::ResizeDirection::NorthWest, egui::CursorIcon::ResizeNwSe)),
                    (_, true, true, _) => Some((egui::ResizeDirection::NorthEast, egui::CursorIcon::ResizeNeSw)),
                    (true, _, _, true) => Some((egui::ResizeDirection::SouthWest, egui::CursorIcon::ResizeNeSw)),
                    (_, true, _, true) => Some((egui::ResizeDirection::SouthEast, egui::CursorIcon::ResizeNwSe)),
                    (true, _, _, _) => Some((egui::ResizeDirection::West, egui::CursorIcon::ResizeHorizontal)),
                    (_, true, _, _) => Some((egui::ResizeDirection::East, egui::CursorIcon::ResizeHorizontal)),
                    (_, _, true, _) => Some((egui::ResizeDirection::North, egui::CursorIcon::ResizeVertical)),
                    (_, _, _, true) => Some((egui::ResizeDirection::South, egui::CursorIcon::ResizeVertical)),
                    _ => None,
                };
                if let Some((resize_dir, cursor)) = dir {
                    ctx.set_cursor_icon(cursor);
                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(resize_dir));
                    }
                }
            }
        }

        self.draw_titlebar(ctx, &pal);

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(196.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .fill(pal.sidebar_bg)
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Bad Apple!! rs")
                        .strong()
                        .size(17.0)
                        .color(pal.text_primary),
                );
                ui.add_space(14.0);

                let prev_tab_before = self.tab;
                let (_, rect_gen, prs_gen) =
                    sidebar_nav_item(ui, &pal, &mut self.tab, ActiveTab::General, s.nav_settings);
                let (_, rect_abt, prs_abt) =
                    sidebar_nav_item(ui, &pal, &mut self.tab, ActiveTab::About, s.nav_about);

                if self.tab != prev_tab_before {
                    self.nav_prev_tab = prev_tab_before;
                    self.nav_switch_time = Some(Instant::now());
                    self.content_prev_tab = prev_tab_before;
                    self.content_switch_time = Some(Instant::now());
                }

                let y_for = |t: ActiveTab| match t {
                    ActiveTab::General => rect_gen.center().y,
                    ActiveTab::About => rect_abt.center().y,
                };
                let y_to = y_for(self.tab);
                let active_prs = match self.tab {
                    ActiveTab::General => prs_gen,
                    ActiveTab::About => prs_abt,
                };

                let ease_io = |x: f32| -> f32 {
                    let x = x.clamp(0.0, 1.0);
                    x * x * (3.0 - 2.0 * x)
                };

                let (pill_y, pill_h) = if let Some(t0) = self.nav_switch_time {
                    let raw = (t0.elapsed().as_secs_f32() / 0.26).clamp(0.0, 1.0);
                    if raw < 1.0 {
                        ctx.request_repaint();
                        let y_from = y_for(self.nav_prev_tab);
                        if raw < 0.25 {
                            let k = ease_io(raw / 0.25);
                            (y_from, 16.0 - 11.0 * k)
                        } else if raw < 0.75 {
                            let k = ease_io((raw - 0.25) / 0.50);
                            (y_from + (y_to - y_from) * k, 5.0)
                        } else {
                            let k = ease_io((raw - 0.75) / 0.25);
                            (y_to, 5.0 + 11.0 * k)
                        }
                    } else {
                        self.nav_switch_time = None;
                        (y_to, 16.0 * (1.0 - 0.25 * active_prs))
                    }
                } else {
                    (y_to, 16.0 * (1.0 - 0.25 * active_prs))
                };

                let pill_rect = egui::Rect::from_center_size(
                    egui::pos2(rect_gen.left() + 4.5, pill_y),
                    egui::vec2(3.0, pill_h),
                );
                let pill_col = lerp_color(pal.accent_base, pal.accent_pressed, active_prs);
                ui.painter().rect_filled(pill_rect, 1.5, pill_col);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    let btn_w = ui.available_width();
                    fluent_language_dropdown(ui, &pal, &mut self.cfg.language, btn_w);
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(pal.sidebar_bg)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                let outer_rect = ui.max_rect();
                ui.set_clip_rect(outer_rect);

                // Rounded top-left corner transition into the deeper main content panel
                let corner_r = 10.0;
                let bg_rect = egui::Rect::from_min_max(
                    outer_rect.min,
                    outer_rect.max + egui::vec2(2.0, 2.0),
                );
                ui.painter().rect(
                    bg_rect,
                    egui::Rounding {
                        nw: corner_r,
                        ne: 0.0,
                        sw: 0.0,
                        se: 0.0,
                    },
                    pal.main_bg,
                    egui::Stroke::new(1.0_f32, pal.card_border),
                );

                let content_rect = outer_rect.shrink2(egui::vec2(18.0, 16.0));
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                    let panel_rect = ui.max_rect();
                    ui.set_clip_rect(panel_rect);

                let (old_alpha, new_alpha, slide_y, old_tab) =
                    if let Some(t0) = self.content_switch_time {
                        let raw = (t0.elapsed().as_secs_f32() / 0.28).clamp(0.0, 1.0);
                        if raw < 1.0 {
                            ctx.request_repaint();
                            let out_t = (raw / 0.38).clamp(0.0, 1.0);
                            let old_a = 1.0 - out_t * out_t;
                            let in_t = ((raw - 0.22) / 0.78).clamp(0.0, 1.0);
                            let ease_in = 1.0 - (1.0 - in_t).powi(3);
                            (old_a, ease_in, (1.0 - ease_in) * 18.0, Some(self.content_prev_tab))
                        } else {
                            self.content_switch_time = None;
                            (0.0, 1.0, 0.0, None)
                        }
                    } else {
                        (0.0, 1.0, 0.0, None)
                    };

                if new_alpha > 0.01 {
                    ui.scope(|ui| {
                        ui.set_opacity(new_alpha);
                        let cur_tab = self.tab;
                        self.draw_tab_panel(ui, cur_tab, s, slide_y);
                    });
                }

                if let (Some(prev), true) = (old_tab, old_alpha > 0.01) {
                    egui::Area::new(egui::Id::new("tab_fade_out_overlay"))
                        .fixed_pos(panel_rect.min)
                        .order(egui::Order::Foreground)
                        .interactable(false)
                        .show(ctx, |ui| {
                            ui.set_clip_rect(panel_rect);
                            ui.allocate_ui_with_layout(
                                panel_rect.size(),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.disable();
                                    ui.set_opacity(old_alpha);
                                    self.draw_tab_panel(ui, prev, s, 0.0);
                                },
                            );
                        });
                }
                });
            });
    }
}
