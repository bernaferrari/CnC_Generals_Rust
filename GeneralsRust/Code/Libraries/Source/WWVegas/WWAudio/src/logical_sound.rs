//! Logical sound representation used for gameplay notification events.

use crate::{
    math::{Matrix3D, Vector3},
    sound_scene_obj::{SoundObjectId, SoundSceneObject},
    sound_types::SoundClassId,
};

#[derive(Debug, Clone)]
pub struct LogicalSound {
    pub base: SoundSceneObject,
    pub type_mask: u32,
    pub is_single_shot: bool,
    pub notify_delay_ms: u32,
    pub last_notification_ms: u32,
    pub dropoff_radius: f32,
    pub listener_timestamp: u32,
    pub max_listeners: usize,
    pub position: Vector3,
}

impl LogicalSound {
    pub fn new(id: SoundObjectId) -> Self {
        let mut base = SoundSceneObject::new(id, SoundClassId::Logical);
        base.set_priority(0.0);
        Self {
            base,
            type_mask: 0,
            is_single_shot: false,
            notify_delay_ms: 2000,
            last_notification_ms: 0,
            dropoff_radius: 1.0,
            listener_timestamp: 0,
            max_listeners: 0,
            position: Vector3::ZERO,
        }
    }

    pub fn set_position(&mut self, position: Vector3) {
        self.position = position;
        self.base.set_position(position);
    }

    pub fn position(&self) -> Vector3 {
        self.position
    }

    pub fn set_transform(&mut self, transform: Matrix3D) {
        self.position = transform.get_translation();
        self.base.set_transform(transform);
    }

    pub fn allow_notify(&mut self, current_time_ms: u32) -> bool {
        if self.notify_delay_ms == 0 {
            self.last_notification_ms = current_time_ms;
            return true;
        }
        let elapsed = current_time_ms.saturating_sub(self.last_notification_ms);
        if elapsed >= self.notify_delay_ms {
            self.last_notification_ms = current_time_ms;
            true
        } else {
            false
        }
    }

    pub fn set_type_mask(&mut self, mask: u32) {
        self.type_mask = mask;
    }

    pub fn type_mask(&self) -> u32 {
        self.type_mask
    }

    pub fn set_single_shot(&mut self, single_shot: bool) {
        self.is_single_shot = single_shot;
    }

    pub fn is_single_shot(&self) -> bool {
        self.is_single_shot
    }

    pub fn set_dropoff_radius(&mut self, radius: f32) {
        self.dropoff_radius = radius.max(0.0);
    }

    pub fn dropoff_radius(&self) -> f32 {
        self.dropoff_radius
    }

    pub fn set_notify_delay(&mut self, delay_ms: u32) {
        self.notify_delay_ms = delay_ms;
    }

    pub fn notify_delay(&self) -> u32 {
        self.notify_delay_ms
    }

    pub fn set_last_notification(&mut self, timestamp: u32) {
        self.last_notification_ms = timestamp;
    }

    pub fn last_notification(&self) -> u32 {
        self.last_notification_ms
    }

    pub fn set_listener_timestamp(&mut self, timestamp: u32) {
        self.listener_timestamp = timestamp;
    }

    pub fn listener_timestamp(&self) -> u32 {
        self.listener_timestamp
    }

    pub fn set_max_listeners(&mut self, count: usize) {
        self.max_listeners = count;
    }

    pub fn max_listeners(&self) -> usize {
        self.max_listeners
    }
}
