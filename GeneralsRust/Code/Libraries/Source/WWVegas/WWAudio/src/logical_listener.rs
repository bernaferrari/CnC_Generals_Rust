//! Logical listener representation mirroring WWAudio's `LogicalListenerClass`.

use crate::{
    math::{Matrix3D, Vector3},
    sound_scene_obj::{SoundObjectId, SoundSceneObject},
    sound_types::SoundClassId,
};

/// Process-wide logical-listener clock/scale moved onto [`crate::sound_scene::SoundScene`].

#[derive(Debug, Clone)]
pub struct LogicalListener {
    pub base: SoundSceneObject,
    pub type_mask: u32,
    pub scale: f32,
    pub timestamp: u32,
}

impl LogicalListener {
    pub fn new(id: SoundObjectId) -> Self {
        Self {
            base: SoundSceneObject::new(id, SoundClassId::Logical),
            type_mask: 0,
            scale: 1.0,
            timestamp: 0,
        }
    }

    pub fn set_type_mask(&mut self, mask: u32) {
        self.type_mask = mask;
    }

    pub fn type_mask(&self) -> u32 {
        self.type_mask
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.0);
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn effective_scale(&self, global_scale: f32) -> f32 {
        self.scale * global_scale
    }

    pub fn set_position(&mut self, position: Vector3) {
        self.base.set_position(position);
    }

    pub fn position(&self) -> Vector3 {
        self.base.position()
    }

    pub fn set_transform(&mut self, transform: Matrix3D) {
        self.base.set_transform(transform);
    }

    pub fn transform(&self) -> Matrix3D {
        self.base.transform()
    }

    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    pub fn set_timestamp(&mut self, timestamp: u32) {
        self.timestamp = timestamp;
    }

}
