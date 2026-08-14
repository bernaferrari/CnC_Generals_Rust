//! Frozen, client-owned physics-visual transform contract.
//!
//! C++ `Drawable::applyPhysicsXform` runs during `Drawable::draw`, after the
//! drawable's base/instance transform has been assembled and before any draw
//! module sees it.  The eventual Rust caller must collect this input once at
//! the GameClient visual-frame boundary; this module deliberately does not
//! read GameLogic, presentation, renderer, or singleton state itself.

use crate::drawable::{Matrix4, Vector3};

/// The already-calculated output of C++ `Drawable::calcPhysicsXform`.
///
/// `calcPhysicsXform` is intentionally not reproduced here.  Its locomotor
/// calculations mutate `DrawableLocoInfo` and depend on terrain, AI, physics,
/// and overlap state that must be frozen together by the future GameClient
/// visual-frame collector.  `None` means that C++ returned `false` (for
/// example, no eligible current locomotor).
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
    /// The original code clears values strictly inside `(-1e-20, 1e-20)`
    /// before returning the calculated result.  Keep this on the frozen
    /// value, rather than on the matrix, so the eventual frame collector can
    /// also persist the exact post-hotfix totals and avoid platform-specific
    /// denormal behavior.
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

/// Frozen inputs for one call to C++ `Drawable::applyPhysicsXform`.
///
/// These fields intentionally mirror only the gates from
/// `Drawable.cpp:1364-1384`.  The collector must source them from the same
/// visual-frame snapshot, rather than dereferencing live GameLogic objects
/// while WGPU submits a frame.
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
    /// tactical camera is still moving.  Do not replace this with a generic
    /// game-paused or logic-frame condition.
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
/// `Matrix3D::Translate` and all three `Matrix3D::Rotate_*` methods are
/// post-multiplies.  Therefore the sequence is intentionally *not* reordered:
/// `base * TranslateZ * RotateY(pitch) * RotateX(-roll) * RotateZ(yaw)`.
///
/// C++ source: `GameClient/Drawable.cpp:1378-1384` and
/// `WWMath/matrix3d.h:592-597, 692-870`.
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
///
/// This is a pure frame-boundary helper.  It remains unused until the
/// GameClient has a complete, authoritative `calcPhysicsXform` port and can
/// supply `calculated_xform` from the same visual snapshot as the draw.
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

#[cfg(test)]
mod tests {
    use super::{
        apply_physics_visual_xform, post_multiply_physics_visual_xform, Matrix4,
        PhysicsVisualInput, PhysicsVisualXform, Vector3,
    };

    fn enabled_input() -> PhysicsVisualInput {
        PhysicsVisualInput {
            has_object: true,
            show_client_physics: true,
            calculated_xform: Some(PhysicsVisualXform {
                total_pitch: 0.43,
                total_roll: -0.58,
                total_yaw: 0.91,
                total_z: 0.71,
            }),
            ..PhysicsVisualInput::default()
        }
    }

    fn assert_matrix_close(actual: Matrix4, expected: Matrix4) {
        for row in 0..4 {
            for column in 0..4 {
                let difference =
                    (actual.elements[row][column] - expected.elements[row][column]).abs();
                assert!(
                    difference <= 0.000_001,
                    "matrix element [{row}][{column}] differs: actual={}, expected={}",
                    actual.elements[row][column],
                    expected.elements[row][column]
                );
            }
        }
    }

    // Independent scalar transcription of WWVegas `Matrix3D` methods.  Keep
    // this in the test rather than using Matrix4's constructors so a changed
    // multiplication side/order cannot make both sides agree accidentally.
    fn cpp_translate_z(matrix: &mut Matrix4, z: f32) {
        for row in 0..3 {
            matrix.elements[row][3] += matrix.elements[row][2] * z;
        }
    }

    fn cpp_rotate_x(matrix: &mut Matrix4, theta: f32) {
        let (sine, cosine) = theta.sin_cos();
        for row in 0..3 {
            let first = matrix.elements[row][1];
            let second = matrix.elements[row][2];
            matrix.elements[row][1] = cosine * first + sine * second;
            matrix.elements[row][2] = -sine * first + cosine * second;
        }
    }

    fn cpp_rotate_y(matrix: &mut Matrix4, theta: f32) {
        let (sine, cosine) = theta.sin_cos();
        for row in 0..3 {
            let first = matrix.elements[row][0];
            let second = matrix.elements[row][2];
            matrix.elements[row][0] = cosine * first - sine * second;
            matrix.elements[row][2] = sine * first + cosine * second;
        }
    }

    fn cpp_rotate_z(matrix: &mut Matrix4, theta: f32) {
        let (sine, cosine) = theta.sin_cos();
        for row in 0..3 {
            let first = matrix.elements[row][0];
            let second = matrix.elements[row][1];
            matrix.elements[row][0] = cosine * first + sine * second;
            matrix.elements[row][1] = -sine * first + cosine * second;
        }
    }

    #[test]
    fn physics_visual_gate_matches_drawable_apply_physics_xform() {
        let enabled = enabled_input();
        assert!(enabled.permits_application());

        let cases = [
            (
                PhysicsVisualInput {
                    has_object: false,
                    ..enabled
                },
                false,
            ),
            (
                PhysicsVisualInput {
                    object_disabled_held: true,
                    ..enabled
                },
                false,
            ),
            (
                PhysicsVisualInput {
                    show_client_physics: false,
                    ..enabled
                },
                false,
            ),
            (
                PhysicsVisualInput {
                    tactical_view_time_frozen: true,
                    camera_movement_finished: false,
                    ..enabled
                },
                false,
            ),
            (
                PhysicsVisualInput {
                    tactical_view_time_frozen: true,
                    camera_movement_finished: true,
                    ..enabled
                },
                true,
            ),
            (
                PhysicsVisualInput {
                    script_time_frozen_debug: true,
                    ..enabled
                },
                false,
            ),
            (
                PhysicsVisualInput {
                    script_time_frozen_script: true,
                    ..enabled
                },
                false,
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(input.permits_application(), expected, "input={input:?}");
        }
    }

    #[test]
    fn physics_visual_preserves_base_without_calculation_or_when_gated() {
        let base =
            Matrix4::translation(Vector3::new(3.0, -5.0, 7.0)).mul(&Matrix4::rotation_z(0.37));

        let no_calculation = PhysicsVisualInput {
            calculated_xform: None,
            ..enabled_input()
        };
        assert_eq!(apply_physics_visual_xform(base, no_calculation), base);

        let disabled = PhysicsVisualInput {
            object_disabled_held: true,
            ..enabled_input()
        };
        assert_eq!(apply_physics_visual_xform(base, disabled), base);
    }

    #[test]
    fn physics_visual_uses_cpp_postmultiply_order_and_roll_sign() {
        let base = Matrix4::translation(Vector3::new(3.0, -5.0, 7.0))
            .mul(&Matrix4::rotation_z(0.37))
            .mul(&Matrix4::rotation_x(-0.22));
        let input = enabled_input();
        let xform = input
            .calculated_xform
            .expect("enabled test input has xform");

        let mut expected = base;
        cpp_translate_z(&mut expected, xform.total_z);
        cpp_rotate_y(&mut expected, xform.total_pitch);
        cpp_rotate_x(&mut expected, -xform.total_roll);
        cpp_rotate_z(&mut expected, xform.total_yaw);

        assert_matrix_close(post_multiply_physics_visual_xform(base, xform), expected);
        assert_matrix_close(apply_physics_visual_xform(base, input), expected);
    }

    #[test]
    fn physics_visual_applies_cpp_denormal_hotfix_before_transform() {
        let base = Matrix4::translation(Vector3::new(3.0, -5.0, 7.0));
        let input = PhysicsVisualInput {
            calculated_xform: Some(PhysicsVisualXform {
                total_pitch: 0.5e-20,
                total_roll: -0.5e-20,
                total_yaw: 0.5e-20,
                total_z: -0.5e-20,
            }),
            ..enabled_input()
        };

        assert_eq!(
            input
                .calculated_xform
                .expect("test input has xform")
                .without_denormals(),
            PhysicsVisualXform::default()
        );
        assert_eq!(apply_physics_visual_xform(base, input), base);
    }
}
