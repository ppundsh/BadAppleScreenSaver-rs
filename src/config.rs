use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundMode {
    Blur,
    LiquidGlass,
    Transparent,
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    ZhTw,
    En,
    Ja,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OuterFillMode {
    Auto,
    Solid,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleMode {
    Fit,
    Fill,
    Stretch,
    Original,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub background_mode: BackgroundMode,
    pub blur_radius: u32,
    pub glass_strength: f32,
    pub bg_dim_opacity: f32,
    pub invert_silhouette: bool,
    pub foreground_color: [u8; 3],
    pub solid_bg_color: [u8; 3],
    pub scale_mode: ScaleMode,
    pub use_16_9: bool,
    pub outer_fill_mode: OuterFillMode,
    pub outer_fill_color: [u8; 3],
    pub all_monitors: bool,
    pub show_fps: bool,
    pub audio_enabled: bool,
    pub audio_volume: f32,
    pub custom_audio_path: Option<String>,
    pub custom_video_path: Option<String>,
    pub language: Language,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            background_mode: BackgroundMode::Blur,
            blur_radius: 24,
            glass_strength: 1.0,
            bg_dim_opacity: 0.25,
            invert_silhouette: false,
            foreground_color: [0, 0, 0],
            solid_bg_color: [255, 255, 255],
            scale_mode: ScaleMode::Fit,
            use_16_9: true,
            outer_fill_mode: OuterFillMode::Auto,
            outer_fill_color: [0, 0, 0],
            all_monitors: true,
            show_fps: false,
            audio_enabled: false,
            audio_volume: 0.8,
            custom_audio_path: None,
            custom_video_path: None,
            language: Language::ZhTw,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("BadAppleScreenSaver-rs").join("config.json")
    }

    pub fn sanitize(&mut self) {
        self.blur_radius = self.blur_radius.min(60);
        self.glass_strength = if self.glass_strength.is_finite() {
            self.glass_strength.clamp(0.0, 2.0)
        } else {
            1.0
        };
        self.bg_dim_opacity = if self.bg_dim_opacity.is_finite() {
            self.bg_dim_opacity.clamp(0.0, 0.95)
        } else {
            0.25
        };
        self.audio_volume = if self.audio_volume.is_finite() {
            self.audio_volume.clamp(0.0, 1.0)
        } else {
            0.8
        };
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<Config>(&data) {
                cfg.sanitize();
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let mut clean = self.clone();
        clean.sanitize();
        let json = serde_json::to_string_pretty(&clean)
            .map_err(|e| format!("Failed to serialize: {e}"))?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &json).map_err(|e| format!("Failed to write temp config: {e}"))?;
        if fs::rename(&tmp_path, &path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            fs::write(&path, &json).map_err(|e| format!("Failed to write config: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.blur_radius, cfg.blur_radius);
        assert_eq!(parsed.background_mode, BackgroundMode::Blur);
    }
}
