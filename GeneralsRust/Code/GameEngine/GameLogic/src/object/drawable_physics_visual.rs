//! GameLogic `Drawable::draw` physics-visual apply seam.
//!
//! C++ `applyPhysicsXform` runs here (after instance, before draw modules).
//! The exact `calcPhysicsXform` port lives in `game-client-rust` and cannot
//! be imported (GameClient depends on GameLogic). This module applies the
//! C++ gate on `show_client_physics` and leaves the matrix unchanged when
//! no frozen calc result is available. The Main host present path runs the
//! full calc.

use crate::common::Matrix3D;
use game_engine::common::ini::get_global_data;
use glam::{Mat4, Vec3};

/// Apply C++ `applyPhysicsXform` gates. Without a GameClient calc result the
/// matrix is unchanged (fail-closed, not an approximation from saved loco).
#[must_use]
pub fn apply_if_gated(base: Matrix3D) -> Matrix3D {
    if !show_client_physics() {
        return base;
    }
    base
}

/// Post-multiply C++ `TranslateZ * Ry(pitch) * Rx(-roll) * Rz(yaw)` in the
/// GameLogic glam Z-up convention. Used by tests and future frozen-calc wiring.
#[must_use]
pub fn post_multiply_zup(base: Matrix3D, pitch: f32, roll: f32, yaw: f32, z: f32) -> Matrix3D {
    base * Mat4::from_translation(Vec3::new(0.0, 0.0, z))
        * Mat4::from_rotation_y(pitch)
        * Mat4::from_rotation_x(-roll)
        * Mat4::from_rotation_z(yaw)
}

fn show_client_physics() -> bool {
    get_global_data()
        .map(|data| data.read().show_client_physics)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::post_multiply_zup;
    use glam::{Mat4, Vec3};

    #[test]
    fn logic_post_multiply_is_translate_then_pitch_then_neg_roll_then_yaw() {
        let base = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let actual = post_multiply_zup(base, 0.2, 0.3, 0.4, 5.0);
        let expected = base
            * Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0))
            * Mat4::from_rotation_y(0.2)
            * Mat4::from_rotation_x(-0.3)
            * Mat4::from_rotation_z(0.4);
        let a = actual.to_cols_array();
        let e = expected.to_cols_array();
        for i in 0..16 {
            assert!((a[i] - e[i]).abs() < 1e-5);
        }
    }
}
