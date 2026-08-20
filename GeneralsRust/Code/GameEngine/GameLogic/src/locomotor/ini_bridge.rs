//! Copy Common `ini_locomotor` templates into the live GameLogic store,
//! applying the C++ field converters that Common still skips.
//!
//! Matches `LocomotorStore::parseLocomotorTemplateDefinition` (Locomotor.cpp:583-614)
//! plus `getFieldParse` converters (Locomotor.cpp:417-488).

use crate::locomotor::core::{
    LocomotorAppearance, LocomotorBehaviorZ, LocomotorPriority, LocomotorStore, LocomotorTemplate,
};

const LOGICFRAMES_PER_SECOND: f32 = 30.0;
const SECONDS_PER_LOGICFRAME: f32 = 1.0 / LOGICFRAMES_PER_SECOND;
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

/// C++ `INI::parseVelocityReal` — dist/sec → dist/frame.
fn parse_velocity_real(per_sec: f32) -> f32 {
    per_sec * SECONDS_PER_LOGICFRAME
}

/// C++ `parseFrictionPerSec` — friction/sec → friction/frame.
fn parse_friction_per_sec(per_sec: f32) -> f32 {
    per_sec * SECONDS_PER_LOGICFRAME
}

/// C++ `INI::parseAngleReal` — degrees → radians.
fn parse_angle_real(degrees: f32) -> f32 {
    degrees * DEG_TO_RAD
}

/// C++ `INI::parseAngularVelocityReal` — deg/sec → rad/frame.
fn parse_angular_velocity_real(deg_per_sec: f32) -> f32 {
    deg_per_sec * DEG_TO_RAD * SECONDS_PER_LOGICFRAME
}

/// C++ `INI::parseDurationReal` — msec → frames.
fn parse_duration_real(msec: f32) -> f32 {
    msec * LOGICFRAMES_PER_SECOND / 1000.0
}

fn map_appearance(src: game_engine::common::ini::LocomotorAppearance) -> LocomotorAppearance {
    use game_engine::common::ini::LocomotorAppearance as C;
    match src {
        C::LegsTWO => LocomotorAppearance::TwoLegs,
        C::WheelsFOUR => LocomotorAppearance::FourWheels,
        C::Treads => LocomotorAppearance::Treads,
        C::Hover => LocomotorAppearance::Hover,
        C::Thrust => LocomotorAppearance::Thrust,
        C::Wings => LocomotorAppearance::Wings,
        C::Climber => LocomotorAppearance::Climber,
        C::Motorcycle => LocomotorAppearance::Motorcycle,
        C::Other => LocomotorAppearance::Other,
    }
}

fn map_behavior_z(src: game_engine::common::ini::LocomotorBehaviorZ) -> LocomotorBehaviorZ {
    use game_engine::common::ini::LocomotorBehaviorZ as C;
    match src {
        C::NoZMotiveForce => LocomotorBehaviorZ::NoZMotiveForce,
        C::SeaLevel => LocomotorBehaviorZ::SeaLevel,
        C::SurfaceRelativeHeight => LocomotorBehaviorZ::SurfaceRelativeHeight,
        C::AbsoluteHeight => LocomotorBehaviorZ::AbsoluteHeight,
        C::FixedSurfaceRelativeHeight => LocomotorBehaviorZ::FixedSurfaceRelativeHeight,
        C::FixedAbsoluteHeight => LocomotorBehaviorZ::FixedAbsoluteHeight,
        C::RelativeToGroundAndBuildings => LocomotorBehaviorZ::RelativeToGroundAndBuildings,
        C::SmoothRelativeToHighestLayer => LocomotorBehaviorZ::SmoothRelativeToHighestLayer,
    }
}

fn map_priority(src: game_engine::common::ini::LocomotorPriority) -> LocomotorPriority {
    use game_engine::common::ini::LocomotorPriority as C;
    match src {
        C::MovesBack => LocomotorPriority::Back,
        C::MovesMiddle => LocomotorPriority::Middle,
        C::MovesFront => LocomotorPriority::Front,
    }
}

/// Build a GameLogic template from the Common INI store, converting omitted-field
/// defaults to the C++ ctor and applying skipped unit converters.
pub fn from_common_ini_template(
    src: &game_engine::common::ini::LocomotorTemplate,
) -> LocomotorTemplate {
    let mut dest = LocomotorTemplate::new(src.name.to_string());

    dest.surfaces = src.surfaces.0;
    dest.max_speed = src.max_speed;
    dest.max_speed_damaged = src.max_speed_damaged;
    dest.min_speed = parse_velocity_real(src.min_speed);
    dest.max_turn_rate = src.max_turn_rate;
    dest.max_turn_rate_damaged = src.max_turn_rate_damaged;
    dest.acceleration = src.acceleration;
    dest.acceleration_damaged = src.acceleration_damaged;
    dest.lift = src.lift;
    dest.lift_damaged = src.lift_damaged;
    dest.braking = if src.braking == 0.0 {
        dest.braking
    } else {
        src.braking
    };
    dest.min_turn_speed = if src.min_turn_speed == 0.0 {
        dest.min_turn_speed
    } else {
        parse_velocity_real(src.min_turn_speed)
    };
    dest.preferred_height = src.preferred_height;
    dest.preferred_height_damping = src.preferred_height_damping;
    dest.circling_radius = src.circling_radius;
    dest.speed_limit_z = if src.speed_limit_z >= 1_000_000.0 {
        dest.speed_limit_z
    } else {
        parse_velocity_real(src.speed_limit_z)
    };
    dest.extra_2d_friction = parse_friction_per_sec(src.extra_2d_friction);
    dest.max_thrust_angle = parse_angle_real(src.max_thrust_angle);
    dest.behavior_z = map_behavior_z(src.behavior_z);
    dest.appearance = map_appearance(src.appearance);
    dest.move_priority = map_priority(src.move_priority);
    dest.accel_pitch_limit = parse_angle_real(src.accel_pitch_limit);
    dest.decel_pitch_limit = parse_angle_real(src.decel_pitch_limit);
    dest.bounce_kick = parse_angular_velocity_real(src.bounce_kick);
    dest.pitch_stiffness = if src.pitch_stiffness == 0.0 {
        dest.pitch_stiffness
    } else {
        src.pitch_stiffness
    };
    dest.roll_stiffness = if src.roll_stiffness == 0.0 {
        dest.roll_stiffness
    } else {
        src.roll_stiffness
    };
    dest.pitch_damping = if src.pitch_damping == 0.0 {
        dest.pitch_damping
    } else {
        src.pitch_damping
    };
    dest.roll_damping = if src.roll_damping == 0.0 {
        dest.roll_damping
    } else {
        src.roll_damping
    };
    dest.pitch_by_z_vel_coef = src.pitch_by_z_vel_coef;
    dest.thrust_roll = src.thrust_roll;
    dest.wobble_rate = src.wobble_rate;
    dest.min_wobble = src.min_wobble;
    dest.max_wobble = src.max_wobble;
    dest.forward_vel_coef = src.forward_vel_coef;
    dest.lateral_vel_coef = src.lateral_vel_coef;
    dest.forward_accel_coef = src.forward_accel_coef;
    dest.lateral_accel_coef = src.lateral_accel_coef;
    dest.uniform_axial_damping = if src.uniform_axial_damping == 0.0 {
        dest.uniform_axial_damping
    } else {
        src.uniform_axial_damping
    };
    dest.turn_pivot_offset = src.turn_pivot_offset;
    dest.airborne_targeting_height = if src.airborne_targeting_height == 0 {
        dest.airborne_targeting_height
    } else {
        src.airborne_targeting_height
    };
    dest.close_enough_dist = src.close_enough_dist;
    dest.is_close_enough_dist_3d = src.is_close_enough_dist_3d;
    dest.ultra_accurate_slide_factor =
        parse_duration_real(src.ultra_accurate_slide_into_place_factor);
    dest.locomotor_works_when_dead = src.locomotor_works_when_dead;
    dest.allow_motive_force_while_airborne = src.allow_motive_force_while_airborne;
    dest.apply_2d_friction_when_airborne = src.apply_2d_friction_when_airborne;
    dest.downhill_only = src.downhill_only;
    dest.stick_to_ground = src.stick_to_ground;
    dest.can_move_backward = src.can_move_backward;
    dest.has_suspension = src.has_suspension;
    dest.maximum_wheel_extension = src.maximum_wheel_extension;
    dest.maximum_wheel_compression = src.maximum_wheel_compression;
    dest.wheel_turn_angle = parse_angle_real(src.wheel_turn_angle);
    dest.wander_width_factor = src.wander_width_factor;
    dest.wander_length_factor = if src.wander_length_factor == 0.0 {
        dest.wander_length_factor
    } else {
        src.wander_length_factor
    };
    dest.wander_about_point_radius = src.wander_about_point_radius;
    dest.rudder_correction_degree = src.rudder_correction_degree;
    dest.rudder_correction_rate = src.rudder_correction_rate;
    dest.elevator_correction_degree = src.elevator_correction_degree;
    dest.elevator_correction_rate = src.elevator_correction_rate;

    dest.validate();
    dest
}

/// Register every Common INI locomotor into the live GameLogic store.
pub fn sync_common_store_into(store: &LocomotorStore) {
    let common = game_engine::common::ini::get_locomotor_store();
    for name in common.get_template_names() {
        let key = name.to_string();
        if let Some(src) = common.find_template(&key) {
            store.register_template(from_common_ini_template(src));
        }
    }
}

/// Convert a single Common template by retail name.
pub fn convert_named(name: &str) -> Option<LocomotorTemplate> {
    let common = game_engine::common::ini::get_locomotor_store();
    common.find_template(name).map(from_common_ini_template)
}
