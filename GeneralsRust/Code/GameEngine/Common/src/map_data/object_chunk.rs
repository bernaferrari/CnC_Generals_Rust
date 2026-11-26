//! Authored Object rows inside the map's ObjectsList chunk.

use crate::common::dict::{Dict, DictType};
use crate::common::system::DataChunkInput;
use glam::Vec3;

/// The Object chunk's position, identity and authored properties.
pub struct MapObjectChunk {
    pub position: Vec3,
    pub flags: u32,
    pub name: String,
    pub properties: Dict,
}

impl MapObjectChunk {
    pub fn read(input: &mut DataChunkInput<'_>, version: u16) -> Self {
        let mut position = Vec3::new(input.read_real(), input.read_real(), input.read_real());
        if version <= 2 {
            position.z = 0.0;
        }

        let _angle = input.read_real();
        let flags = input.read_int() as u32;
        let name = input.read_ascii_string();
        let properties = if version >= 2 {
            input.read_dict()
        } else {
            Dict::new()
        };
        Self {
            position,
            flags,
            name,
            properties,
        }
    }

    /// Return the authored waypoint ID and name as resolved by the map loader.
    pub fn waypoint(&self, input: &mut DataChunkInput<'_>) -> Option<(i32, String)> {
        let id = input.intern_name("waypointID");
        if self.properties.get_type(id) != Some(DictType::Int) {
            return None;
        }

        let name_key = input.intern_name("waypointName");
        let name = match self.properties.get_type(name_key) {
            Some(DictType::AsciiString) => self.properties.get_ascii_string(name_key),
            Some(DictType::UnicodeString) => self.properties.get_unicode_string(name_key),
            _ => String::new(),
        };
        let name = if name.is_empty() {
            self.name.clone()
        } else {
            name
        };
        Some((self.properties.get_int(id), name))
    }
}
