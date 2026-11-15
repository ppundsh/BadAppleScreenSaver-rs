#![windows_subsystem = "windows"]

mod audio;
mod blur;
mod config;
mod i18n;
mod media;
mod screensaver;
mod settings_ui;

use config::Config;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

enum Mode {
    Screensaver,
    Config,
    Preview(isize),
}

fn parse_args() -> Mode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Mode::Config;
    }

    let first = args[0].trim().to_ascii_lowercase();
    if first == "/s" || first == "-s" {
        Mode::Screensaver
    } else if first.starts_with("/c") || first.starts_with("-c") {
        Mode::Config
    } else if first.starts_with("/p") || first.starts_with("-p") {
        let hwnd_str = if let Some(rest) = first.strip_prefix("/p:").or_else(|| first.strip_prefix("-p:")) {
            rest.to_string()
        } else {
            args.get(1).cloned().unwrap_or_default()
        };
        if let Ok(val) = hwnd_str.trim().parse::<isize>() {
            Mode::Preview(val)
        } else {
            Mode::Config
        }
    } else {
        Mode::Config
    }
}

fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let cfg = Config::load();
    let res = match parse_args() {
        Mode::Screensaver => screensaver::run_fullscreen(&cfg),
        Mode::Config => settings_ui::run_settings(),
        Mode::Preview(hwnd) => screensaver::run_preview(hwnd, &cfg),
    };

    if let Err(e) = res {
        eprintln!("Error: {e}");
    }
}
