//! Logical listener representation mirroring WWAudio's `LogicalListenerClass`.

use crate::{
    math::{Matrix3D, Vector3},
    sound_scene_obj::{SoundObjectId, SoundSceneObject},
    sound_types::SoundClassId,
};
use std::sync::atomic::{AtomicU32, Ordering};

static GLOBAL_SCALE_BITS: AtomicU32 = AtomicU32::new(1.0f32.to_bits());
static OLDEST_TIMESTAMP: AtomicU32 = AtomicU32::new(0);
static NEWEST_TIMESTAMP: AtomicU32 = AtomicU32::new(1);

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

    pub fn effective_scale(&self) -> f32 {
        self.scale * Self::global_scale()
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

    pub fn new_timestamp() -> u32 {
        NEWEST_TIMESTAMP.fetch_add(1, Ordering::SeqCst)
    }

    pub fn newest_timestamp() -> u32 {
        NEWEST_TIMESTAMP.load(Ordering::SeqCst).saturating_sub(1)
    }

    pub fn oldest_timestamp() -> u32 {
        OLDEST_TIMESTAMP.load(Ordering::SeqCst)
    }

    pub fn set_oldest_timestamp(timestamp: u32) {
        let mut current = OLDEST_TIMESTAMP.load(Ordering::SeqCst);
        while timestamp > current {
            match OLDEST_TIMESTAMP.compare_exchange(
                current,
                timestamp,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    pub fn global_scale() -> f32 {
        f32::from_bits(GLOBAL_SCALE_BITS.load(Ordering::Relaxed))
    }

    pub fn set_global_scale(scale: f32) {
        let clamped = scale.max(0.0);
        GLOBAL_SCALE_BITS.store(clamped.to_bits(), Ordering::Relaxed);
    }
}
