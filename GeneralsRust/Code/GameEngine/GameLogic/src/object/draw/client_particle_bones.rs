//! Borrowed native particle and bone operations for one receiving Drawable.
use crate::common::{Coord3D, Matrix3D};

/// Keeps particle existence, bone queries, and writes in their original order.
/// Implementations borrow the receiving manager and Drawable pose; they must
/// not select a global object, particle manager, or another active session.
pub trait ClientParticleBoneRuntime {
    fn particle_system_exists(&mut self, id: u32) -> bool;
    fn bone_transform(&mut self, model: &str, bone_index: i32, name: &str) -> Option<Matrix3D>;
    /// Apply position, local Z rotation, world transform, then skip-parent=true.
    fn set_particle_bone_transform(
        &mut self,
        id: u32,
        position: &Coord3D,
        z_rotation: f32,
        transform: &Matrix3D,
    );
}

/// Preparation is separate from malformed factory/admission errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeStateParticlePreparation {
    Prepared(bool),
    /// This valid ObjectDraw interface has no native preparation implementation.
    UnsupportedModule,
    /// Authored behavior requires receiving services that are not supplied yet.
    Unsupported {
        animation_pose: bool,
        terrain_tracks: bool,
    },
}

/// Inputs admitted at the actual receiving Draw phase, distinct from logic time.
#[derive(Debug, Clone, Copy)]
pub struct NativeStateParticleContext {
    pub conditions: crate::common::ModelConditionFlags,
    pub instance_scale: f32,
    pub state_particles: bool,
    pub drawable_id: u32,
    pub hidden: bool,
    pub fully_obscured: bool,
    pub creation_frame: u32,
}

/// Selected static-model state/emitter operations on explicit receiving owners.
pub trait ClientParticleStateRuntime {
    fn standard_public_bones(&self) -> Vec<String>;
    fn pristine_bone(
        &mut self,
        model: &str,
        scale: f32,
        pose_frame: i32,
        name: &str,
    ) -> Option<(i32, Matrix3D)>;
    fn sub_object_names(&mut self, model: &str) -> Vec<String>;
    fn destroy_particle_system(&mut self, id: u32);
    fn set_particle_system_running(&mut self, id: u32, running: bool);
    fn create_particle_system(
        &mut self,
        template: &str,
        creation_frame: u32,
    ) -> Result<Option<u32>, String>;
    fn model_space_bone(&mut self, model: &str, bone: &str) -> Option<(i32, Matrix3D)>;
    /// Position/rotation, attach to Drawable, saveable=false, then hidden stop.
    fn configure_particle_system(
        &mut self,
        id: u32,
        drawable_id: u32,
        bone_index: i32,
        local: &Matrix3D,
        hidden: bool,
    );
}
