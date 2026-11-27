//! Authored object creation lists for the simulation (C++ ObjectCreationList.cpp).

mod catalog;
mod delivery;
mod generic;
mod orders;
mod parser;

pub use catalog::{ObjectCreationListDefinitions, ObjectCreationLists};
pub use delivery::{
    DeliverPayloadData, DeliverPayloadFormationPose, DeliverPayloadNugget, DeliveryNugget, Payload,
};
pub use generic::{AnimSet, GenericObjectCreationNugget};
pub use orders::{ApplyRandomForceNugget, AttackNugget, FireWeaponNugget};
pub use parser::{parse_object_creation_lists_from_ini, parse_object_creation_lists_from_str};

use glam::Vec3;
use std::f32::consts::PI;
use std::sync::Arc;

pub const INVALID_ANGLE: f32 = -999999.0;

#[derive(Clone, Debug)]
pub enum ObjectCreationNugget {
    Generic(GenericObjectCreationNugget),
    DeliverPayload(DeliverPayloadNugget),
    FireWeapon(FireWeaponNugget),
    Attack(AttackNugget),
    ApplyRandomForce(ApplyRandomForceNugget),
}

#[derive(Clone, Debug, Default)]
pub struct ObjectCreationList {
    nuggets: Arc<[ObjectCreationNugget]>,
}

impl ObjectCreationList {
    pub fn new(nuggets: Vec<ObjectCreationNugget>) -> Self {
        Self {
            nuggets: nuggets.into(),
        }
    }

    pub fn nuggets(&self) -> &[ObjectCreationNugget] {
        &self.nuggets
    }
}

/// Matrix result of C++ `calcRandomForce`:
/// `Scale(mag) * Rotate_Z(angle) * Rotate_Y(-pitch)` then `Get_X_Vector()`.
pub fn calc_random_force_xyz(angle: f32, pitch: f32, mag: f32) -> Vec3 {
    let horiz = mag * pitch.cos();
    let vert = mag * pitch.sin();
    Vec3::new(horiz * angle.cos(), horiz * angle.sin(), vert)
}

/// C++ ObjectCreationList.cpp:575-591 draws angle, pitch, and magnitude.
fn calc_random_force_with(
    mut draw: impl FnMut(f32, f32) -> f32,
    min_mag: f32,
    max_mag: f32,
    min_pitch: f32,
    max_pitch: f32,
) -> Vec3 {
    let angle = draw(0.0, 2.0 * PI);
    let pitch = draw(min_pitch, max_pitch);
    let mag = draw(min_mag, max_mag);
    calc_random_force_xyz(angle, pitch, mag)
}
