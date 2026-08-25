//! Frozen, client-owned physics-visual transform contract.
//!
//! C++ `Drawable::applyPhysicsXform` runs during `Drawable::draw`, after the
//! drawable's base/instance transform has been assembled and before any draw
//! module sees it.

mod apply;
mod calc;
mod hover;
mod loco_state;
mod motorcycle;
mod rng;
mod spring;
mod thrust;
mod treads;
mod types;
mod wheels;
mod wheels_suspension;

pub use apply::{
    PhysicsVisualInput, apply_physics_visual_xform, post_multiply_physics_visual_xform,
};
pub use calc::calc_physics_visual_xform;
pub use loco_state::PhysicsVisualLocoState;
pub use rng::{ClientVisualRng, LiveClientRng, ScriptedClientRng};
pub use types::{
    CPP_PI, LocomotorVisualParams, OverlapVisualTarget, PhysicsVisualAppearance, PhysicsVisualBody,
};

/// The already-calculated output of C++ `Drawable::calcPhysicsXform`.
///
/// C++ source: `GameClient/Drawable.h:589-596`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhysicsVisualXform {
    /// C++ `PhysicsXformInfo::m_totalPitch`.
    pub total_pitch: f32,
    /// C++ `PhysicsXformInfo::m_totalRoll`.
    pub total_roll: f32,
    /// C++ `PhysicsXformInfo::m_totalYaw`.
    pub total_yaw: f32,
    /// C++ `PhysicsXformInfo::m_totalZ`.
    pub total_z: f32,
}

impl PhysicsVisualXform {
    /// Apply the denormal hotfix from C++ `Drawable::calcPhysicsXform`.
    ///
    /// The original code clears values strictly inside `(-1e-20, 1e-20)`.
    #[must_use]
    pub fn without_denormals(self) -> Self {
        fn clear(value: f32) -> f32 {
            if value > -1.0e-20 && value < 1.0e-20 {
                0.0
            } else {
                value
            }
        }

        Self {
            total_pitch: clear(self.total_pitch),
            total_roll: clear(self.total_roll),
            total_yaw: clear(self.total_yaw),
            total_z: clear(self.total_z),
        }
    }
}

/// Y-up local physics multiply for the Main host mesh path.
///
/// Calc totals are C++ Z-up. Host local frame is Y-up with +X forward at
/// orientation 0, so `T(0,0,Z)*Ry(pitch)*Rx(-roll)*Rz(yaw)` remaps to
/// `T(0,Z,0)*Rz(pitch)*Rx(-roll)*Ry(yaw)`.
#[must_use]
pub fn glam_yup_physics_visual_local(xform: PhysicsVisualXform) -> glam::Mat4 {
    let cleaned = xform.without_denormals();
    glam::Mat4::from_translation(glam::Vec3::new(0.0, cleaned.total_z, 0.0))
        * glam::Mat4::from_rotation_z(cleaned.total_pitch)
        * glam::Mat4::from_rotation_x(-cleaned.total_roll)
        * glam::Mat4::from_rotation_y(cleaned.total_yaw)
}

/// Z-up glam multiply matching GameLogic `Matrix3D` (= `glam::Mat4`) usage.
#[must_use]
pub fn glam_zup_physics_visual_local(xform: PhysicsVisualXform) -> glam::Mat4 {
    let cleaned = xform.without_denormals();
    glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, cleaned.total_z))
        * glam::Mat4::from_rotation_y(cleaned.total_pitch)
        * glam::Mat4::from_rotation_x(-cleaned.total_roll)
        * glam::Mat4::from_rotation_z(cleaned.total_yaw)
}

#[cfg(test)]
mod tests;
