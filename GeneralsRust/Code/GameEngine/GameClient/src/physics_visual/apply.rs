//! Frozen apply contract for C++ `Drawable::applyPhysicsXform`.
//!
//! C++ source: `GameClient/Drawable.cpp:1364-1384` and
//! `WWMath/matrix3d.h:592-597, 692-870`.

use crate::drawable::{Matrix4, Vector3};

use super::PhysicsVisualXform;

/// Frozen inputs for one call to C++ `Drawable::applyPhysicsXform`.
///
/// These fields intentionally mirror only the gates from
/// `Drawable.cpp:1364-1384`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhysicsVisualInput {
    /// C++ `getObject() != NULL`.
    pub has_object: bool,
    /// C++ `obj->isDisabledByType(DISABLED_HELD)`.
    pub object_disabled_held: bool,
    /// C++ `TheGlobalData->m_showClientPhysics`.
    pub show_client_physics: bool,
    /// C++ `TheTacticalView->isTimeFrozen()`.
    pub tactical_view_time_frozen: bool,
    /// C++ `TheTacticalView->isCameraMovementFinished()`.
    pub camera_movement_finished: bool,
    /// C++ `TheScriptEngine->isTimeFrozenDebug()`.
    pub script_time_frozen_debug: bool,
    /// C++ `TheScriptEngine->isTimeFrozenScript()`.
    pub script_time_frozen_script: bool,
    /// The final `calcPhysicsXform` result from this visual frame.
    pub calculated_xform: Option<PhysicsVisualXform>,
}

impl PhysicsVisualInput {
    /// The exact C++ visual-freeze condition.
    ///
    /// C++ deliberately allows physics visuals to continue while a frozen
    /// tactical camera is still moving.
    #[must_use]
    pub const fn is_frozen_for_physics_visuals(self) -> bool {
        (self.tactical_view_time_frozen && !self.camera_movement_finished)
            || self.script_time_frozen_debug
            || self.script_time_frozen_script
    }

    /// Whether C++ reaches `calcPhysicsXform` for this frozen input.
    #[must_use]
    pub const fn permits_application(self) -> bool {
        self.has_object
            && !self.object_disabled_held
            && self.show_client_physics
            && !self.is_frozen_for_physics_visuals()
    }
}

/// Post-multiply the exact local transform assembled by C++
/// `Drawable::applyPhysicsXform`.
///
/// Sequence: `base * TranslateZ * RotateY(pitch) * RotateX(-roll) * RotateZ(yaw)`.
#[must_use]
pub fn post_multiply_physics_visual_xform(
    base_transform: Matrix4,
    xform: PhysicsVisualXform,
) -> Matrix4 {
    let local = Matrix4::translation(Vector3::new(0.0, 0.0, xform.total_z))
        .mul(&Matrix4::rotation_y(xform.total_pitch))
        .mul(&Matrix4::rotation_x(-xform.total_roll))
        .mul(&Matrix4::rotation_z(xform.total_yaw));
    base_transform.mul(&local)
}

/// Apply a frozen C++ physics visual correction, or preserve the base exactly.
#[must_use]
pub fn apply_physics_visual_xform(base_transform: Matrix4, input: PhysicsVisualInput) -> Matrix4 {
    if !input.permits_application() {
        return base_transform;
    }

    input
        .calculated_xform
        .map(|xform| post_multiply_physics_visual_xform(base_transform, xform.without_denormals()))
        .unwrap_or(base_transform)
}
