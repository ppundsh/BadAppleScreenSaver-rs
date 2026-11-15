use crate::config::Config;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::{BufReader, Cursor};

static DEFAULT_AUDIO: &[u8] = include_bytes!("../assets/bad_apple.mp3");

pub struct AudioPlayer {
    _stream: OutputStream,
    _sink: Sink,
}

impl AudioPlayer {
    pub fn start(cfg: &Config) -> Option<Self> {
        if !cfg.audio_enabled {
            return None;
        }
        let (_stream, stream_handle) = OutputStream::try_default().ok()?;
        let sink = Sink::try_new(&stream_handle).ok()?;
        sink.set_volume(cfg.audio_volume.clamp(0.0, 1.0));

        let loaded = if let Some(ref path) = cfg.custom_audio_path {
            if std::path::Path::new(path).is_file() {
                if let Ok(file) = File::open(path) {
                    if let Ok(dec) = Decoder::new(BufReader::new(file)) {
                        sink.append(dec.repeat_infinite());
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if !loaded {
            let cursor = Cursor::new(DEFAULT_AUDIO);
            if let Ok(dec) = Decoder::new(cursor) {
                sink.append(dec.repeat_infinite());
            } else {
                return None;
            }
        }

        sink.play();
        Some(Self {
            _stream,
            _sink: sink,
        })
    }
}
