//! Logical definitions and factories mirroring Miles definitions.

use crate::sound_scene_obj::SoundObjectId;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct LogicalTypeDefinition {
    pub id: i32,
    pub display_name: String,
}

#[derive(Debug, Default)]
pub struct LogicalDefinitionManager {
    types: HashMap<i32, LogicalTypeDefinition>,
}

impl LogicalDefinitionManager {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    pub fn add_type(&mut self, id: i32, name: impl Into<String>) {
        self.types.insert(
            id,
            LogicalTypeDefinition {
                id,
                display_name: name.into(),
            },
        );
    }

    pub fn get(&self, id: i32) -> Option<&LogicalTypeDefinition> {
        self.types.get(&id)
    }

    pub fn clear(&mut self) {
        self.types.clear();
    }
}

#[derive(Debug, Clone)]
pub struct LogicalSoundFactoryEntry {
    pub sound_id: SoundObjectId,
    pub type_mask: u32,
    pub display: Option<String>,
}

#[derive(Debug, Default)]
pub struct LogicalSoundFactory {
    entries: HashMap<SoundObjectId, LogicalSoundFactoryEntry>,
}

impl LogicalSoundFactory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn register(&mut self, sound_id: SoundObjectId, type_mask: u32, display: Option<String>) {
        self.entries.insert(
            sound_id,
            LogicalSoundFactoryEntry {
                sound_id,
                type_mask,
                display,
            },
        );
    }

    pub fn lookup(&self, sound_id: SoundObjectId) -> Option<&LogicalSoundFactoryEntry> {
        self.entries.get(&sound_id)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub mod list;
