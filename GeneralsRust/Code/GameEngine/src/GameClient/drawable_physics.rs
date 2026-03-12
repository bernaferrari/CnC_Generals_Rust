// FILE: drawable_physics.rs
// Physics transform calculations for drawable visual feedback
// Ported from C++ Drawable.cpp physics transform methods
// Author: Original C++ implementation in Drawable.cpp

use crate::Common::game_type::{Real, Bool, Int};
use crate::WWMath::matrix3d::Matrix3D;
use crate::GameClient::drawable::{Drawable, DrawableLocoInfo, PhysicsXformInfo};

/// Denormalization epsilon for hotfix
const DENORM_EPSILON: Real = 1e-20;

/// Locomotor appearance types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotorAppearance {
    Wheels4,
    Motorcycle,
    Treads,
    Hover,
    Wings,
    Thrust,
}

/// Locomotor interface for physics calculations
pub trait LocomotorInterface {
    fn get_appearance(&self) -> LocomotorAppearance;
    fn get_thrust_roll(&self) -> Real { 0.0 }
    fn get_wobble_rate(&self) -> Real { 0.0 }
    fn get_max_wobble(&self) -> Real { 0.0 }
    fn get_min_wobble(&self) -> Real { 0.0 }
    fn get_forward_velocity_2d(&self) -> Real { 0.0 }
    fn get_lateral_velocity_2d(&self) -> Real { 0.0 }
    fn get_turn_rate(&self) -> Real { 0.0 }
    fn get_forward_accel_coefficient(&self) -> Real { 0.0 }
}

/// Physics transform calculation for drawables
pub struct DrawablePhysics;

impl DrawablePhysics {
    /// Apply physics transform to a matrix
    pub fn apply_physics_xform(
        drawable: &Drawable,
        mtx: &mut Matrix3D,
        locomotor: Option<&dyn LocomotorInterface>,
        time_frozen: bool,
    ) -> bool {
        if time_frozen {
            return false;
        }

        let mut info = PhysicsXformInfo::default();

        if Self::calc_physics_xform(drawable, locomotor, &mut info) {
            // Apply transforms in order: Z translation, Y rotation (pitch), X rotation (roll), Z rotation (yaw)
            mtx.translate(0.0, 0.0, info.total_z);
            mtx.rotate_y(info.total_pitch);
            mtx.rotate_x(-info.total_roll);
            mtx.rotate_z(info.total_yaw);

            true
        } else {
            false
        }
    }

    /// Calculate physics transform based on locomotor type
    pub fn calc_physics_xform(
        drawable: &Drawable,
        locomotor: Option<&dyn LocomotorInterface>,
        info: &mut PhysicsXformInfo,
    ) -> bool {
        let locomotor = match locomotor {
            Some(loco) => loco,
            None => return false,
        };

        let has_transform = match locomotor.get_appearance() {
            LocomotorAppearance::Wheels4 => {
                Self::calc_physics_xform_wheels(drawable, locomotor, info);
                true
            }
            LocomotorAppearance::Motorcycle => {
                Self::calc_physics_xform_motorcycle(drawable, locomotor, info);
                true
            }
            LocomotorAppearance::Treads => {
                Self::calc_physics_xform_treads(drawable, locomotor, info);
                true
            }
            LocomotorAppearance::Hover | LocomotorAppearance::Wings => {
                Self::calc_physics_xform_hover_or_wings(drawable, locomotor, info);
                true
            }
            LocomotorAppearance::Thrust => {
                Self::calc_physics_xform_thrust(drawable, locomotor, info);
                true
            }
        };

        if has_transform {
            // HOTFIX: Ensure that we are not passing denormalized values back to caller
            if info.total_pitch > -DENORM_EPSILON && info.total_pitch < DENORM_EPSILON {
                info.total_pitch = 0.0;
            }
            if info.total_roll > -DENORM_EPSILON && info.total_roll < DENORM_EPSILON {
                info.total_roll = 0.0;
            }
            if info.total_yaw > -DENORM_EPSILON && info.total_yaw < DENORM_EPSILON {
                info.total_yaw = 0.0;
            }
            if info.total_z > -DENORM_EPSILON && info.total_z < DENORM_EPSILON {
                info.total_z = 0.0;
            }
        }

        has_transform
    }

    /// Calculate physics transform for thrust locomotor (missiles, etc.)
    fn calc_physics_xform_thrust(
        drawable: &Drawable,
        locomotor: &dyn LocomotorInterface,
        info: &mut PhysicsXformInfo,
    ) {
        let thrust_roll = locomotor.get_thrust_roll();
        let wobble_rate = locomotor.get_wobble_rate();
        let max_wobble = locomotor.get_max_wobble();
        let min_wobble = locomotor.get_min_wobble();

        // Quick thrust implementation for scud missiles to wobble
        // Adjust pitch, yaw, and roll slightly

        if wobble_rate > 0.0 {
            // Would access drawable's loco_info here
            // This is a simplified version without mutable access

            // Wobbling logic would go here, oscillating between min and max wobble
            // Based on the wobble state and rate
        }

        // Set roll based on thrust roll parameter
        info.total_roll = thrust_roll;
    }

    /// Calculate physics transform for hover/wings locomotor
    fn calc_physics_xform_hover_or_wings(
        drawable: &Drawable,
        locomotor: &dyn LocomotorInterface,
        info: &mut PhysicsXformInfo,
    ) {
        // Hover and wings use similar physics
        let forward_vel = locomotor.get_forward_velocity_2d();
        let lateral_vel = locomotor.get_lateral_velocity_2d();
        let turn_rate = locomotor.get_turn_rate();

        // Calculate banking based on turn rate
        let bank_scalar = 0.3; // Could be configurable
        info.total_roll = -turn_rate * bank_scalar;

        // Calculate pitch based on forward velocity
        let pitch_scalar = 0.1;
        info.total_pitch = forward_vel * pitch_scalar;

        // Add some bobbing/hovering motion
        // This would use frame counters and sin/cos for smooth oscillation
    }

    /// Calculate physics transform for treaded vehicles
    fn calc_physics_xform_treads(
        drawable: &Drawable,
        locomotor: &dyn LocomotorInterface,
        info: &mut PhysicsXformInfo,
    ) {
        let forward_vel = locomotor.get_forward_velocity_2d();
        let lateral_vel = locomotor.get_lateral_velocity_2d();
        let turn_rate = locomotor.get_turn_rate();

        // Treads lean into turns
        let lean_scalar = 0.15;
        info.total_roll = -turn_rate * lean_scalar;

        // Pitch forward/backward based on acceleration
        let accel_coeff = locomotor.get_forward_accel_coefficient();
        let pitch_scalar = 0.08;
        info.total_pitch = -accel_coeff * pitch_scalar;

        // Clamp values
        info.total_roll = info.total_roll.max(-0.3).min(0.3);
        info.total_pitch = info.total_pitch.max(-0.2).min(0.2);
    }

    /// Calculate physics transform for wheeled vehicles
    fn calc_physics_xform_wheels(
        drawable: &Drawable,
        locomotor: &dyn LocomotorInterface,
        info: &mut PhysicsXformInfo,
    ) {
        let forward_vel = locomotor.get_forward_velocity_2d();
        let lateral_vel = locomotor.get_lateral_velocity_2d();
        let turn_rate = locomotor.get_turn_rate();

        // Wheeled vehicles lean into turns more than treads
        let lean_scalar = 0.25;
        info.total_roll = -turn_rate * lean_scalar;

        // Pitch based on acceleration
        let accel_coeff = locomotor.get_forward_accel_coefficient();
        let pitch_scalar = 0.12;
        info.total_pitch = -accel_coeff * pitch_scalar;

        // Suspension effects would be calculated here
        // Using wheel info for individual wheel height offsets

        // Clamp values
        info.total_roll = info.total_roll.max(-0.4).min(0.4);
        info.total_pitch = info.total_pitch.max(-0.25).min(0.25);
    }

    /// Calculate physics transform for motorcycles
    fn calc_physics_xform_motorcycle(
        drawable: &Drawable,
        locomotor: &dyn LocomotorInterface,
        info: &mut PhysicsXformInfo,
    ) {
        let forward_vel = locomotor.get_forward_velocity_2d();
        let lateral_vel = locomotor.get_lateral_velocity_2d();
        let turn_rate = locomotor.get_turn_rate();

        // Motorcycles lean heavily into turns
        let lean_scalar = 0.5;
        info.total_roll = -turn_rate * lean_scalar;

        // Strong pitch response to acceleration
        let accel_coeff = locomotor.get_forward_accel_coefficient();
        let pitch_scalar = 0.2;
        info.total_pitch = -accel_coeff * pitch_scalar;

        // Motorcycles can lean much more than other vehicles
        info.total_roll = info.total_roll.max(-0.7).min(0.7);
        info.total_pitch = info.total_pitch.max(-0.3).min(0.3);
    }
}

/// Extension methods for Drawable physics
pub trait DrawablePhysicsExt {
    /// Apply physics transform to a matrix
    fn apply_physics_transform(
        &self,
        mtx: &mut Matrix3D,
        locomotor: Option<&dyn LocomotorInterface>,
        time_frozen: bool,
    ) -> bool;
}

impl DrawablePhysicsExt for Drawable {
    fn apply_physics_transform(
        &self,
        mtx: &mut Matrix3D,
        locomotor: Option<&dyn LocomotorInterface>,
        time_frozen: bool,
    ) -> bool {
        DrawablePhysics::apply_physics_xform(self, mtx, locomotor, time_frozen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLocomotor {
        appearance: LocomotorAppearance,
        forward_vel: Real,
        turn_rate: Real,
    }

    impl LocomotorInterface for MockLocomotor {
        fn get_appearance(&self) -> LocomotorAppearance {
            self.appearance
        }

        fn get_forward_velocity_2d(&self) -> Real {
            self.forward_vel
        }

        fn get_turn_rate(&self) -> Real {
            self.turn_rate
        }

        fn get_forward_accel_coefficient(&self) -> Real {
            0.5
        }
    }

    #[test]
    fn test_physics_xform_denorm_fix() {
        let drawable = Drawable::default();
        let locomotor = MockLocomotor {
            appearance: LocomotorAppearance::Wheels4,
            forward_vel: 0.0,
            turn_rate: 0.0,
        };

        let mut info = PhysicsXformInfo::default();
        DrawablePhysics::calc_physics_xform(&drawable, Some(&locomotor), &mut info);

        // Values should be exactly 0.0, not denormalized
        assert_eq!(info.total_pitch, 0.0);
        assert_eq!(info.total_roll, 0.0);
        assert_eq!(info.total_yaw, 0.0);
        assert_eq!(info.total_z, 0.0);
    }

    #[test]
    fn test_wheels_lean_into_turn() {
        let drawable = Drawable::default();
        let locomotor = MockLocomotor {
            appearance: LocomotorAppearance::Wheels4,
            forward_vel: 10.0,
            turn_rate: 1.0,
        };

        let mut info = PhysicsXformInfo::default();
        DrawablePhysics::calc_physics_xform(&drawable, Some(&locomotor), &mut info);

        // Should have negative roll when turning (leaning into turn)
        assert!(info.total_roll < 0.0);
        assert!(info.total_roll >= -0.4); // Within clamp range
    }

    #[test]
    fn test_motorcycle_lean_more_than_wheels() {
        let drawable = Drawable::default();

        let wheels_loco = MockLocomotor {
            appearance: LocomotorAppearance::Wheels4,
            forward_vel: 10.0,
            turn_rate: 1.0,
        };

        let motorcycle_loco = MockLocomotor {
            appearance: LocomotorAppearance::Motorcycle,
            forward_vel: 10.0,
            turn_rate: 1.0,
        };

        let mut wheels_info = PhysicsXformInfo::default();
        let mut motorcycle_info = PhysicsXformInfo::default();

        DrawablePhysics::calc_physics_xform(&drawable, Some(&wheels_loco), &mut wheels_info);
        DrawablePhysics::calc_physics_xform(&drawable, Some(&motorcycle_loco), &mut motorcycle_info);

        // Motorcycle should lean more than wheeled vehicle
        assert!(motorcycle_info.total_roll.abs() > wheels_info.total_roll.abs());
    }

    #[test]
    fn test_thrust_appearance() {
        let drawable = Drawable::default();
        let locomotor = MockLocomotor {
            appearance: LocomotorAppearance::Thrust,
            forward_vel: 50.0,
            turn_rate: 0.0,
        };

        let mut info = PhysicsXformInfo::default();
        let result = DrawablePhysics::calc_physics_xform(&drawable, Some(&locomotor), &mut info);

        assert!(result); // Should have physics transform
    }
}
