//! Listener representation mirroring `Listener3DClass` behaviour.

use crate::{
    math::{Matrix3D, Vector3},
    sound_scene_obj::{SoundObjectId, SoundSceneObject},
    sound_types::{SoundClassId, SoundState},
};

#[derive(Debug, Clone)]
pub struct Listener3D {
    pub base: SoundSceneObject,
    pub attached_object: Option<SoundObjectId>,
    pub bone_index: Option<i32>,
    pub velocity: Vector3,
}

impl Listener3D {
    pub fn new(id: SoundObjectId) -> Self {
        Self {
            base: SoundSceneObject::new(id, SoundClassId::Listener),
            attached_object: None,
            bone_index: None,
            velocity: Vector3::ZERO,
        }
    }

    pub fn attach_to_object(&mut self, object_id: SoundObjectId, bone_index: Option<i32>) {
        self.attached_object = Some(object_id);
        self.bone_index = bone_index;
    }

    pub fn detach(&mut self) {
        self.attached_object = None;
        self.bone_index = None;
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

    pub fn set_velocity(&mut self, velocity: Vector3) {
        self.velocity = velocity;
    }

    pub fn velocity(&self) -> Vector3 {
        self.velocity
    }

    pub fn is_active(&self) -> bool {
        self.base.state() != SoundState::Stopped
    }
}
