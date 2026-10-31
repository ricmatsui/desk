use std::collections::HashMap;

use raylib::core::audio::{RaylibAudio, Sound, Wave};

/// Clips embedded in the binary, by the name messages refer to them with
const CLIPS: &[(&str, &[u8])] = &[
    ("meow", include_bytes!("../assets/meow.mp3")),
    ("double-meow", include_bytes!("../assets/double-meow.mp3")),
];

/// The audio device and the decoded clips, owned by the raylib thread. Sounds play on miniaudio's
/// own thread, so playing never blocks drawing.
pub struct Sounds {
    // Declared before `audio` so the clips unload before the device closes
    sounds: HashMap<&'static str, Sound>,
    audio: RaylibAudio,
}

impl Sounds {
    pub fn load() -> Self {
        let audio = RaylibAudio::init_audio_device();

        if !audio.is_audio_device_ready() {
            tracing::warn!("audio device not ready, sounds disabled");

            return Self {
                sounds: HashMap::new(),
                audio,
            };
        }

        let sounds = CLIPS
            .iter()
            .filter_map(|(name, data)| match load_mp3(data) {
                Ok(sound) => Some((*name, sound)),
                Err(error) => {
                    tracing::error!("failed to load sound {}: {}", name, error);
                    None
                }
            })
            .collect();

        Self { sounds, audio }
    }

    pub fn play(&mut self, name: &str) {
        match self.sounds.get(name) {
            Some(sound) => self.audio.play_sound(sound),
            None => tracing::warn!("unknown sound {}", name),
        }
    }
}

fn load_mp3(data: &[u8]) -> Result<Sound, String> {
    let wave = Wave::load_wave_from_memory(".mp3", data)?;
    Sound::load_sound_from_wave(&wave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_decode() {
        for (name, data) in CLIPS {
            let wave = Wave::load_wave_from_memory(".mp3", data).unwrap();
            assert!(wave.sample_count() > 0, "{} has no samples", name);
        }
    }
}
