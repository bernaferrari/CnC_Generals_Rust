//! Script-owned match mix, distinct from application/device slider preferences.
use super::game_audio::AudioAffect;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScriptAudioVolumes {
    pub music: f32,
    pub sound: f32,
    pub sound_3d: f32,
    pub speech: f32,
}
impl Default for ScriptAudioVolumes {
    fn default() -> Self {
        Self {
            music: 1.,
            sound: 1.,
            sound_3d: 1.,
            speech: 1.,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptAudioPolicy {
    pub volumes: ScriptAudioVolumes,
    pub named: Vec<(String, f32)>,
}
impl ScriptAudioPolicy {
    pub fn set_volume(&mut self, volume: f32, affect: AudioAffect) {
        let bits = affect.bits();
        assert_eq!(
            bits & AudioAffect::SystemSetting.bits(),
            0,
            "match policy cannot change app preferences"
        );
        if bits & 1 != 0 {
            self.volumes.music = volume;
        }
        if bits & 2 != 0 {
            self.volumes.sound = volume;
        }
        if bits & 4 != 0 {
            self.volumes.sound_3d = volume;
        }
        if bits & 8 != 0 {
            self.volumes.speech = volume;
        }
    }
    pub fn override_for(&self, name: &str) -> Option<f32> {
        self.named
            .iter()
            .find(|(entry, _)| entry == name)
            .map(|(_, volume)| *volume)
    }
    pub fn set_named_volume(&mut self, name: &str, volume: f32) {
        if name.is_empty() {
            self.named.clear();
            return;
        }
        if volume == -1. {
            self.named.retain(|(entry, _)| entry != name);
            return;
        }
        if let Some((_, current)) = self.named.iter_mut().find(|(entry, _)| entry == name) {
            *current = volume;
        } else {
            self.named.push((name.into(), volume));
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.volumes.music,
            self.volumes.sound,
            self.volumes.sound_3d,
            self.volumes.speech,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err("nonfinite script audio slider".into());
        }
        let mut names = std::collections::HashSet::new();
        if self.named.iter().any(|(name, value)| {
            name.is_empty()
                || name.contains('\0')
                || !value.is_finite()
                || *value == -1.
                || !names.insert(name)
        }) {
            return Err("invalid script audio override table".into());
        }
        Ok(())
    }
}
