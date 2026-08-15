//! Frozen calc inputs for C++ `Drawable::calcPhysicsXform*`.
//!
//! Vectors are C++ Z-up (`xy` ground, `z` vertical). Host Y-up callers remap
//! at the presentation boundary before constructing these records.

/// C++ `PI` as used by Drawable.cpp overlap / suspension math.
pub const CPP_PI: f32 = 3.14159265359;

/// C++ `LocomotorAppearance` cases that `calcPhysicsXform` dispatches on.
///
/// C++ source: `Locomotor.h` appearance enum; `Drawable.cpp:1400-1422`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsVisualAppearance {
    LegsTwo,
    WheelsFour,
    Treads,
    Hover,
    Thrust,
    Wings,
    Climber,
    Other,
    Motorcycle,
}

impl PhysicsVisualAppearance {
    /// Whether C++ enters a `calcPhysicsXform*` handler for this appearance.
    #[must_use]
    pub const fn has_physics_xform(self) -> bool {
        matches!(
            self,
            Self::WheelsFour
                | Self::Motorcycle
                | Self::Treads
                | Self::Hover
                | Self::Wings
                | Self::Thrust
        )
    }
}

/// Authored locomotor visual constants (`Locomotor.h:227-270`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocomotorVisualParams {
    pub accel_pitch_limit: f32,
    pub decel_pitch_limit: f32,
    pub bounce_kick: f32,
    pub pitch_stiffness: f32,
    pub roll_stiffness: f32,
    pub pitch_damping: f32,
    pub roll_damping: f32,
    pub pitch_by_z_vel_coef: f32,
    pub thrust_roll: f32,
    pub wobble_rate: f32,
    pub min_wobble: f32,
    pub max_wobble: f32,
    pub forward_vel_coef: f32,
    pub lateral_vel_coef: f32,
    pub forward_accel_coef: f32,
    pub lateral_accel_coef: f32,
    pub uniform_axial_damping: f32,
    pub has_suspension: bool,
    pub max_wheel_extension: f32,
    pub wheel_turn_angle: f32,
    pub rudder_correction_degree: f32,
    pub rudder_correction_rate: f32,
    pub elevator_correction_degree: f32,
    pub elevator_correction_rate: f32,
}

impl Default for LocomotorVisualParams {
    fn default() -> Self {
        // C++ Locomotor.cpp:284-332 defaults.
        Self {
            accel_pitch_limit: 0.0,
            decel_pitch_limit: 0.0,
            bounce_kick: 0.0,
            pitch_stiffness: 0.1,
            roll_stiffness: 0.1,
            pitch_damping: 0.9,
            roll_damping: 0.9,
            pitch_by_z_vel_coef: 0.0,
            thrust_roll: 0.0,
            wobble_rate: 0.0,
            min_wobble: 0.0,
            max_wobble: 0.0,
            forward_vel_coef: 0.0,
            lateral_vel_coef: 0.0,
            forward_accel_coef: 0.0,
            lateral_accel_coef: 0.0,
            uniform_axial_damping: 1.0,
            has_suspension: false,
            max_wheel_extension: 0.0,
            wheel_turn_angle: 0.0,
            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
        }
    }
}

/// Overlap target facts for treads (`Drawable.cpp:1691-1770`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlapVisualTarget {
    pub is_shrubbery: bool,
    pub is_low_overlappable: bool,
    pub is_infantry: bool,
    pub front_crushed: bool,
    pub back_crushed: bool,
    /// C++ overlap position XY.
    pub pos_x: f32,
    pub pos_y: f32,
    pub bounding_circle_radius: f32,
    pub max_height_above_position: f32,
}

/// Per-draw body / physics / terrain snapshot. C++ Z-up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsVisualBody {
    pub has_object: bool,
    pub has_ai: bool,
    pub has_physics: bool,
    /// C++ `getUnitDirectionVector2D` (`cos(orient)`, `sin(orient)`).
    pub dir_x: f32,
    pub dir_y: f32,
    /// C++ `getVelocity()` (Z-up).
    pub vel_x: f32,
    pub vel_y: f32,
    pub vel_z: f32,
    /// C++ `getAcceleration()` = previous-frame accel (Z-up).
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub velocity_magnitude: f32,
    pub forward_speed_2d: f32,
    pub is_motive: bool,
    /// C++ `PhysicsTurningType`: -1 / 0 / +1.
    pub turning: i8,
    pub cur_locomotor_speed: f32,
    /// C++ object position (Z-up).
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub terrain_height: f32,
    pub terrain_normal_x: f32,
    pub terrain_normal_y: f32,
    pub terrain_normal_z: f32,
    pub significantly_above_terrain: bool,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub bounding_circle_radius: f32,
    pub current_overlap: Option<OverlapVisualTarget>,
    pub previous_overlap_valid: bool,
}

impl Default for PhysicsVisualBody {
    fn default() -> Self {
        Self {
            has_object: true,
            has_ai: true,
            has_physics: true,
            dir_x: 1.0,
            dir_y: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: 0.0,
            velocity_magnitude: 0.0,
            forward_speed_2d: 0.0,
            is_motive: false,
            turning: 0,
            cur_locomotor_speed: 0.0,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            terrain_height: 0.0,
            terrain_normal_x: 0.0,
            terrain_normal_y: 0.0,
            terrain_normal_z: 1.0,
            significantly_above_terrain: false,
            major_radius: 1.0,
            minor_radius: 1.0,
            bounding_circle_radius: 1.0,
            current_overlap: None,
            previous_overlap_valid: false,
        }
    }
}
