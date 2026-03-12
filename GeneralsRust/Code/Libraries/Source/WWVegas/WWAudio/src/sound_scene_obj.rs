//! Base representation of sound objects inserted into the virtual audio scene.

use crate::{
    math::{Matrix3D, Vector3},
    sound_types::{SoundClassId, SoundFlags, SoundState},
    Priority,
};

/// Unique identifier for scene objects.
pub type SoundObjectId = u32;

/// Core data shared by all audible scene objects.
#[derive(Debug, Clone)]
pub struct SoundSceneObject {
    pub id: SoundObjectId,
    pub class_id: SoundClassId,
    pub transform: Matrix3D,
    pub velocity: Vector3,
    pub priority: f32,
    pub flags: SoundFlags,
    pub state: SoundState,
    pub user_priority: Priority,
    pub user_data: u32,
    pub last_update_ms: u64,
}

impl SoundSceneObject {
    pub fn new(id: SoundObjectId, class_id: SoundClassId) -> Self {
        Self {
            id,
            class_id,
            transform: Matrix3D::default(),
            velocity: Vector3::default(),
            priority: 1.0,
            flags: SoundFlags::default(),
            state: SoundState::Stopped,
            user_priority: Priority::Normal,
            user_data: 0,
            last_update_ms: 0,
        }
    }

    pub fn position(&self) -> Vector3 {
        self.transform.get_translation()
    }

    pub fn set_position(&mut self, position: Vector3) {
        self.transform.set_translation(position);
    }

    pub fn set_transform(&mut self, transform: Matrix3D) {
        self.transform = transform;
    }

    pub fn transform(&self) -> Matrix3D {
        self.transform
    }

    pub fn set_velocity(&mut self, velocity: Vector3) {
        self.velocity = velocity;
    }

    pub fn velocity(&self) -> Vector3 {
        self.velocity
    }

    pub fn set_priority(&mut self, priority: f32) {
        self.priority = priority;
    }

    pub fn priority(&self) -> f32 {
        self.priority
    }

    pub fn mark_static(&mut self, is_static: bool) {
        self.flags.is_static = is_static;
    }

    pub fn is_static(&self) -> bool {
        self.flags.is_static
    }

    pub fn mark_culled(&mut self, culled: bool) {
        self.flags.is_culled = culled;
    }

    pub fn is_culled(&self) -> bool {
        self.flags.is_culled
    }

    pub fn set_state(&mut self, state: SoundState) {
        self.state = state;
    }

    pub fn state(&self) -> SoundState {
        self.state
    }

    pub fn set_user_priority(&mut self, priority: Priority) {
        self.user_priority = priority;
    }

    pub fn user_priority(&self) -> Priority {
        self.user_priority
    }

    pub fn set_user_data(&mut self, value: u32) {
        self.user_data = value;
    }

    pub fn user_data(&self) -> u32 {
        self.user_data
    }

    pub fn update_timestamp(&mut self, timestamp: u64) {
        self.last_update_ms = timestamp;
    }

    pub fn last_update(&self) -> u64 {
        self.last_update_ms
    }
}
