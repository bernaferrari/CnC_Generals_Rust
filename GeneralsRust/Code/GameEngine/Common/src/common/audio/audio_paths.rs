//! AudioEventRTS.cpp:746-777 reads paths from the driving AudioManager.
use super::audio_event_rts::AudioType;
use super::game_audio::AudioSettings;

/// A synchronous configuration value; no manager handle escapes generation.
#[derive(Debug, Clone)]
pub(crate) struct AudioPathSettings {
    audio_root: String,
    sounds_folder: String,
    music_folder: String,
    streaming_folder: String,
    sounds_extension: String,
}

impl AudioPathSettings {
    pub(super) fn from_settings(settings: &AudioSettings) -> Self {
        Self {
            audio_root: settings.audio_root.clone(),
            sounds_folder: settings.sounds_folder.clone(),
            music_folder: settings.music_folder.clone(),
            streaming_folder: settings.streaming_folder.clone(),
            sounds_extension: settings.sounds_extension.clone(),
        }
    }

    pub(super) fn prefix(&self, sound_type: AudioType, language: Option<&str>) -> String {
        let mut prefix = self.audio_root.clone();
        prefix.push('\\');
        prefix.push_str(match sound_type {
            AudioType::Music => &self.music_folder,
            AudioType::Streaming => &self.streaming_folder,
            _ => &self.sounds_folder,
        });
        prefix.push('\\');
        if let Some(language) = language {
            prefix.push_str(language);
            prefix.push('\\');
        }
        prefix
    }

    pub(super) fn extension(&self, sound_type: AudioType) -> String {
        if sound_type == AudioType::Music {
            return String::new();
        }
        let extension = self.sounds_extension.trim();
        if extension.starts_with('.') {
            extension.to_owned()
        } else {
            format!(".{extension}")
        }
    }
}

impl Default for AudioPathSettings {
    fn default() -> Self {
        Self {
            audio_root: "Data\\Audio".to_owned(),
            sounds_folder: "Sounds".to_owned(),
            music_folder: "Music".to_owned(),
            streaming_folder: "Speech".to_owned(),
            sounds_extension: "wav".to_owned(),
        }
    }
}

/// Existing standalone/async adapter. Manager operations pass their own value
/// directly; they never reacquire this process handle or take its fallback.
pub(super) fn current_audio_path_settings() -> AudioPathSettings {
    let Some(manager) = super::game_audio::get_global_audio_manager() else {
        return AudioPathSettings::default();
    };
    let guard = match manager.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return AudioPathSettings::default(),
    };
    AudioPathSettings::from_settings(guard.get_audio_settings())
}
