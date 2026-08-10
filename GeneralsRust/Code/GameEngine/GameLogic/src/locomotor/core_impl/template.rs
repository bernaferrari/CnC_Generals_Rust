// ============================================================================
// LOCOMOTOR TEMPLATE
// ============================================================================

/// Locomotor template - defines movement characteristics
/// Matches C++ LocomotorTemplate
#[derive(Debug, Clone)]
pub struct LocomotorTemplate {
    /// Template name
    pub name: String,

    /// Legal surface types (bitmask)
    pub surfaces: LocomotorSurfaceTypeMask,

    /// Maximum speed (dist/frame)
    pub max_speed: Real,
    /// Maximum speed when damaged
    pub max_speed_damaged: Real,
    /// Minimum speed (for slowing down)
    pub min_speed: Real,

    /// Maximum turn rate (radians/frame)
    pub max_turn_rate: Real,
    /// Maximum turn rate when damaged
    pub max_turn_rate_damaged: Real,

    /// Acceleration (dist/frame^2)
    pub acceleration: Real,
    /// Acceleration when damaged
    pub acceleration_damaged: Real,

    /// Lift force (for aircraft)
    pub lift: Real,
    /// Lift when damaged
    pub lift_damaged: Real,

    /// Braking deceleration
    pub braking: Real,

    /// Minimum speed required to turn
    pub min_turn_speed: Real,

    /// Preferred flight height
    pub preferred_height: Real,
    /// Height damping factor (1.0 = aggressive, 0.1 = gradual)
    pub preferred_height_damping: Real,

    /// Circling radius for aircraft (0 = smallest possible)
    pub circling_radius: Real,

    /// Altitude change threshold for circling behavior.
    /// When > 0 and the Z delta to goal exceeds this, the aircraft circles
    /// to gain/lose altitude before resuming course.
    /// Matches C++ Locomotor::m_circleThresh (CIRCLE_FOR_LANDING, disabled by default).
    pub circle_thresh: Real,

    /// Maximum Z-axis speed
    pub speed_limit_z: Real,

    /// Extra 2D friction
    pub extra_2d_friction: Real,

    /// Maximum thrust angle (THRUST locos only)
    pub max_thrust_angle: Real,

    /// Z-axis behavior
    pub behavior_z: LocomotorBehaviorZ,

    /// Visual appearance/type
    pub appearance: LocomotorAppearance,

    /// Group movement priority
    pub move_priority: LocomotorPriority,

    // Suspension and pitch/roll parameters
    pub accel_pitch_limit: Real,
    pub decel_pitch_limit: Real,
    pub bounce_kick: Real,
    pub pitch_stiffness: Real,
    pub roll_stiffness: Real,
    pub pitch_damping: Real,
    pub roll_damping: Real,
    pub pitch_by_z_vel_coef: Real,
    pub thrust_roll: Real,
    pub wobble_rate: Real,
    pub min_wobble: Real,
    pub max_wobble: Real,
    pub forward_vel_coef: Real,
    pub lateral_vel_coef: Real,
    pub forward_accel_coef: Real,
    pub lateral_accel_coef: Real,
    pub uniform_axial_damping: Real,
    pub turn_pivot_offset: Real,

    /// Height at which unit becomes airborne target
    pub airborne_targeting_height: Int,

    /// Close enough distance to destination
    pub close_enough_dist: Real,
    /// Is close enough distance 3D?
    pub is_close_enough_dist_3d: Bool,

    /// Ultra-accurate slide factor
    pub ultra_accurate_slide_factor: Real,

    // Boolean flags
    pub locomotor_works_when_dead: Bool,
    pub allow_motive_force_while_airborne: Bool,
    pub apply_2d_friction_when_airborne: Bool,
    pub downhill_only: Bool,
    pub stick_to_ground: Bool,
    pub can_move_backward: Bool,

    // Suspension parameters
    pub has_suspension: Bool,
    pub maximum_wheel_extension: Real,
    pub maximum_wheel_compression: Real,
    pub wheel_turn_angle: Real,

    // Wander parameters
    pub wander_width_factor: Real,
    pub wander_length_factor: Real,
    pub wander_about_point_radius: Real,

    // Flight control parameters
    pub rudder_correction_degree: Real,
    pub rudder_correction_rate: Real,
    pub elevator_correction_degree: Real,
    pub elevator_correction_rate: Real,
}

impl LocomotorTemplate {
    /// Create new locomotor template with defaults
    pub fn new(name: String) -> Self {
        Self {
            name,
            surfaces: SURFACE_GROUND,
            max_speed: 10.0,
            max_speed_damaged: 5.0,
            min_speed: 0.0,
            max_turn_rate: 0.1,
            max_turn_rate_damaged: 0.05,
            acceleration: 2.0,
            acceleration_damaged: 1.0,
            lift: 0.0,
            lift_damaged: 0.0,
            braking: 3.0,
            min_turn_speed: 0.0,
            preferred_height: 0.0,
            preferred_height_damping: 0.5,
            circling_radius: 0.0,
            circle_thresh: 0.0,
            speed_limit_z: 5.0,
            extra_2d_friction: 0.0,
            max_thrust_angle: 0.0,
            behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            appearance: LocomotorAppearance::Other,
            move_priority: LocomotorPriority::Middle,
            accel_pitch_limit: 0.0,
            decel_pitch_limit: 0.0,
            bounce_kick: 0.0,
            pitch_stiffness: 0.0,
            roll_stiffness: 0.0,
            pitch_damping: 0.0,
            roll_damping: 0.0,
            pitch_by_z_vel_coef: 0.0,
            thrust_roll: 0.0,
            wobble_rate: 0.0,
            min_wobble: 0.0,
            max_wobble: 0.0,
            forward_vel_coef: 0.0,
            lateral_vel_coef: 0.0,
            forward_accel_coef: 0.0,
            lateral_accel_coef: 0.0,
            uniform_axial_damping: 0.0,
            turn_pivot_offset: 0.0,
            airborne_targeting_height: 0,
            close_enough_dist: 5.0,
            is_close_enough_dist_3d: false,
            ultra_accurate_slide_factor: 1.0,
            locomotor_works_when_dead: false,
            allow_motive_force_while_airborne: false,
            apply_2d_friction_when_airborne: false,
            downhill_only: false,
            stick_to_ground: false,
            can_move_backward: false,
            has_suspension: false,
            maximum_wheel_extension: 0.0,
            maximum_wheel_compression: 0.0,
            wheel_turn_angle: 0.0,
            wander_width_factor: 0.0,
            wander_length_factor: 0.0,
            wander_about_point_radius: 0.0,
            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
        }
    }

    /// Create infantry locomotor template
    pub fn new_infantry(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::TwoLegs;
        template.surfaces = SURFACE_GROUND;
        template.max_speed = 8.0;
        template.max_speed_damaged = 4.0;
        template.acceleration = 3.0;
        template.max_turn_rate = 0.15;
        template.braking = 4.0;
        template.stick_to_ground = true;
        template.can_move_backward = true;
        template.close_enough_dist = 3.0;
        template
    }

    /// Create wheeled vehicle template
    pub fn new_wheeled(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::FourWheels;
        template.surfaces = SURFACE_GROUND;
        template.max_speed = 15.0;
        template.max_speed_damaged = 8.0;
        template.acceleration = 5.0;
        template.max_turn_rate = 0.08;
        template.braking = 6.0;
        template.stick_to_ground = true;
        template.has_suspension = true;
        template.can_move_backward = true;
        template.close_enough_dist = 5.0;
        template
    }

    /// Create tracked vehicle template
    pub fn new_tracked(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Treads;
        template.surfaces = SURFACE_GROUND | SURFACE_RUBBLE;
        template.max_speed = 12.0;
        template.max_speed_damaged = 7.0;
        template.acceleration = 4.0;
        template.max_turn_rate = 0.1;
        template.braking = 5.0;
        template.stick_to_ground = true;
        template.can_move_backward = true;
        template.close_enough_dist = 5.0;
        template
    }

    /// Create hover vehicle template
    pub fn new_hover(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Hover;
        template.surfaces = SURFACE_GROUND | SURFACE_WATER;
        template.max_speed = 14.0;
        template.max_speed_damaged = 9.0;
        template.acceleration = 4.5;
        template.max_turn_rate = 0.12;
        template.braking = 5.5;
        template.preferred_height = 3.0;
        template.preferred_height_damping = 0.8;
        template.behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        template.allow_motive_force_while_airborne = true;
        template.close_enough_dist = 5.0;
        template
    }

    /// Create thrust aircraft template (helicopters)
    pub fn new_thrust(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Thrust;
        template.surfaces = SURFACE_AIR;
        template.max_speed = 20.0;
        template.max_speed_damaged = 12.0;
        template.acceleration = 3.0;
        template.lift = 15.0;
        template.max_turn_rate = 0.1;
        template.braking = 4.0;
        template.preferred_height = 50.0;
        template.preferred_height_damping = 0.5;
        template.behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        template.allow_motive_force_while_airborne = true;
        template.airborne_targeting_height = 25;
        template.close_enough_dist = 10.0;
        template
    }

    /// Create fixed-wing aircraft template
    pub fn new_wings(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Wings;
        template.surfaces = SURFACE_AIR;
        template.max_speed = 35.0;
        template.max_speed_damaged = 20.0;
        template.acceleration = 2.0;
        template.lift = 20.0;
        template.max_turn_rate = 0.05;
        template.braking = 2.0;
        template.min_turn_speed = 10.0;
        template.preferred_height = 80.0;
        template.preferred_height_damping = 0.3;
        template.circling_radius = 50.0;
        template.behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        template.allow_motive_force_while_airborne = true;
        template.airborne_targeting_height = 40;
        template.close_enough_dist = 15.0;
        template
    }

    /// Create climber template
    pub fn new_climber(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Climber;
        template.surfaces = SURFACE_GROUND | SURFACE_CLIFF;
        template.max_speed = 6.0;
        template.max_speed_damaged = 3.0;
        template.acceleration = 2.5;
        template.max_turn_rate = 0.12;
        template.braking = 3.5;
        template.stick_to_ground = true;
        template.can_move_backward = true;
        template.close_enough_dist = 3.0;
        template
    }
}

