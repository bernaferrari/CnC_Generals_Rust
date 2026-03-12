//! Logical sound registration mirroring Miles logical factories.

use crate::sound_scene_obj::SoundObjectId;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct LogicalSoundRegistration {
    pub sound_id: SoundObjectId,
    pub type_mask: u32,
    pub display: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct LogicalSoundRegistry {
    entries: HashMap<SoundObjectId, LogicalSoundRegistration>,
}

impl LogicalSoundRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, sound_id: SoundObjectId, type_mask: u32, display: Option<String>) {
        self.entries.insert(
            sound_id,
            LogicalSoundRegistration {
                sound_id,
                type_mask,
                display,
            },
        );
    }

    pub fn lookup(&self, sound_id: SoundObjectId) -> Option<&LogicalSoundRegistration> {
        self.entries.get(&sound_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &LogicalSoundRegistration> {
        self.entries.values()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
