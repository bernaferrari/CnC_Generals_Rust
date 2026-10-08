//! Public scheduled OTHER force direction from rest: loader -> admission -> tick.
//! Frozen unchanged original force-stage rows supply the expected impulse.
//! Rotation is supplied in that extraction; this is not trajectory parity.

use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use generals_main::game_logic::host_enum_table_residual::HostBodyDamageType;
use generals_main::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding;
use generals_main::game_logic::{
    GameLogic, KindOf, LocoGoalType, LocomotorAppearance, Object, Team, ThingTemplate,
};
use generals_main::gameworld_shadow::{
    gameworld_movement_authority_live, shadow_coupled_tick_active, with_active_shadow,
};
use glam::Vec3;

const ORIGINAL: &str = include_str!("fixtures/other_direction_original.txt");
const DT: f32 = 1.0 / 30.0;
const NAMES: [&str; 9] = [
    "axis_nonzero",
    "turn_90",
    "turn_30",
    "reflect_90",
    "reflect_30",
    "entry_reflect_90",
    "slide_close",
    "slide_close_flag_off",
    "slide_far",
];

fn scalar(hex: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(hex, 16).unwrap())
}
fn row(name: &str, lane: &str) -> Vec<&'static str> {
    let row = ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|row| row.first() == Some(&lane) && row[1] == name)
        .unwrap();
    assert_eq!(row.len(), 47);
    row
}
fn close(actual: f32, expected: f32, tolerance: f32, field: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{field}: {actual} ({:08x}) != {expected} ({:08x})",
        actual.to_bits(),
        expected.to_bits()
    );
}
fn bits(actual: f32, expected: &str, field: &str) {
    assert_eq!(actual.to_bits(), scalar(expected).to_bits(), "{field}");
}
fn host_vector(row: &[&str], first: usize) -> Vec3 {
    Vec3::new(
        scalar(row[first]),
        scalar(row[first + 2]),
        -scalar(row[first + 1]),
    )
}
fn host_only() {
    assert!(!shadow_coupled_tick_active());
    assert!(!gameworld_movement_authority_live());
    assert!(
        with_active_shadow(|_| ()).is_none(),
        "no authoritative-view sync may replace setup"
    );
}
fn active_guards(object: &Object, ultra: bool) {
    assert_eq!(object.loco_appearance, LocomotorAppearance::Other);
    assert!(object.is_kind_of(KindOf::Worker));
    assert!(!object.is_kind_of(KindOf::Infantry) && !object.is_kind_of(KindOf::Vehicle));
    assert!(object.is_mobile() && object.can_move() && object.is_alive());
    assert_eq!(
        (object.health.current, object.health.maximum),
        (100.0, 100.0)
    );
    assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
    assert!(!object.is_disabled() && !object.is_shock_stunned());
    assert!(!object.is_physics_held() && !object.is_freefall_disabled() && !object.is_rappelling());
    assert!(!object.allow_to_fall && !object.is_in_freefall && !object.shock_allow_bounce);
    assert!(object.contained_units().is_empty());
    assert_eq!(object.contained_items_mass, 0.0);
    assert_eq!(object.physics_get_mass(), 1.0);
    assert!(!object.worker_ai_update);
    assert!(object.dozer_task_build_target.is_none() && object.dozer_task_repair_target.is_none());
    assert!(object.preferred_dock_id.is_none());
    assert!(
        object.weapon.is_none()
            && object.secondary_weapon.is_none()
            && object.tertiary_weapon.is_none()
    );
    assert!(!object.waiting_for_path && !object.is_blocked && !object.is_blocked_and_stuck);
    assert_eq!(object.num_frames_blocked, 0);
    assert!(object.movement.path.is_empty() && object.requested_destination.is_none());
    assert_eq!(object.path_extra_distance, 0.0);
    assert_ne!(object.locomotor_goal_type, LocoGoalType::PositionExplicit);
    assert!(!object.moving_backwards && !object.doing_three_point_turn);
    assert_eq!(object.ultra_accurate, ultra);
    assert!(!object.is_braking);
    assert_eq!(object.braking_factor, 1.0);
    assert_eq!(object.turn_pivot_offset, 0.0);
    assert_eq!(object.wander_width_factor, 0.0);
    assert_eq!(object.min_speed, 0.0);
    assert_eq!(object.group_speed_factor, 1.0);
    assert!(object.bump_speed_limit >= object.movement.max_speed);
    assert_eq!(object.effective_max_speed(), object.movement.max_speed);
    assert!(object.desired_speed >= object.movement.max_speed);
    assert_eq!(
        object.effective_acceleration(),
        object.movement.acceleration
    );
    assert_eq!(object.physics_accel.x, 0.0);
    assert_eq!(object.physics_accel.z, 0.0);
    assert_eq!(object.ground_height, 0.0);
    assert!(!Object::height_treats_as_airborne(object.get_position().y));
    assert!(object.physics_current_overlap.is_none() && object.physics_previous_overlap.is_none());
    assert!(object.last_collidee.is_none());
    // Keep the ordinary physics options installed by the public ultra setter.
    assert_eq!(object.lateral_friction, 0.15);
    assert_eq!(object.forward_friction, 0.15);
    assert_eq!(object.loco_extra_2d_friction, 0.0);
    assert_eq!(object.extra_friction, if ultra { 0.5 } else { 0.0 });
    assert_eq!(
        object.get_lateral_friction(),
        0.15 + if ultra { 0.5 } else { 0.0 }
    );
    assert_eq!(
        object.apply_friction_2d_when_airborne,
        object.loco_apply_2d_friction_airborne
    );
}

fn run_case(name: &str, assert_original_force: bool) {
    host_only();
    let expected = row(name, "host-impulse");
    let raw = row(name, "raw-frame");
    let start = host_vector(&expected, 5);
    let target = host_vector(&expected, 8);
    let ultra = expected[41] == "1";
    let slide = name == "slide_close";
    let slide_msec = if name.starts_with("slide_") { 1000 } else { 0 };
    let mut game = GameLogic::new();
    assert_eq!((game.get_frame(), game.host_objects().len()), (0, 0));
    // Movement first wakes at frame 1, so the observed tick must follow this.
    let warm = game.update_with_dt(DT);
    assert_eq!((warm.frame, warm.steps_run), (1, 1));
    assert!(!warm.budget_hit);
    assert_eq!(warm.accumulated_time_seconds, 0.0);
    let loco = format!("OtherDirectionLoco_{name}");
    let unit = format!("OtherDirectionUnit_{name}");
    let ini = format!(
        "Locomotor {loco}\nSurfaces = GROUND\nSpeed = 36\nAcceleration = 90\nBraking = 90\nTurnRate = 3600\nAppearance = OTHER\nMinSpeed = 0\nWanderWidthFactor = 0\nZAxisBehavior = NO_Z_MOTIVE_FORCE\nSlideIntoPlaceTime = {slide_msec}\nEnd\n"
    );
    assert_eq!(load_locomotors_from_str(&ini).unwrap(), 1);
    {
        let store = get_locomotor_store();
        let parsed = store.find_template(&loco).unwrap();
        bits(parsed.max_speed, raw[19], "parsed frame speed");
        bits(parsed.acceleration, raw[20], "parsed frame acceleration");
        bits(parsed.braking, raw[21], "parsed frame braking");
    }
    let binding = resolve_host_locomotor_binding(&loco).unwrap();
    bits(binding.movement.max_speed, expected[19], "bound host speed");
    bits(
        binding.movement.acceleration * DT,
        expected[20],
        "bound host acceleration step",
    );
    bits(
        binding.braking * DT,
        expected[21],
        "bound host braking step",
    );
    close(
        binding.ultra_accurate_slide_factor,
        scalar(raw[25]),
        2.0e-6,
        "bound slide factor in frames",
    );
    let mut template = ThingTemplate::new(&unit);
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Worker)
        .set_locomotor_name(&loco);
    game.templates.insert(unit.clone(), template);
    let id = game.create_object(&unit, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(game.host_objects().len(), 1);
    {
        let object = game.host_object_mut(id).unwrap();
        assert_eq!(object.movement.velocity, Vec3::ZERO);
        assert_eq!(object.physics_accel, Vec3::ZERO);
        assert_eq!(object.previous_acceleration(), Vec3::ZERO);
        assert_eq!(object.movement.max_speed, binding.movement.max_speed);
        assert_eq!(object.movement.acceleration, binding.movement.acceleration);
        assert_eq!(object.braking, binding.braking);
        assert_eq!(object.movement.turn_rate, binding.movement.turn_rate);
        assert_eq!(
            object.ultra_accurate_slide_factor,
            binding.ultra_accurate_slide_factor
        );
        object.set_position(start);
        object.set_orientation(scalar(expected[11]));
        object.set_ultra_accurate(ultra);
        object.move_to(target);
        assert_eq!(
            object.movement.velocity,
            Vec3::ZERO,
            "entry remains exactly at rest"
        );
        assert_eq!(object.movement.target_position, Some(target));
        assert!(object.status.moving);
        active_guards(object, ultra);
        close(
            object.get_orientation(),
            scalar(expected[11]),
            2.0e-6,
            "entry yaw",
        );
        let direction = object.unit_direction_vector_2d();
        close(
            direction.x,
            scalar(expected[13]),
            2.0e-6,
            "entry direction X",
        );
        close(
            direction.y,
            -scalar(expected[14]),
            2.0e-6,
            "entry direction Z",
        );
        assert_eq!(
            object.forward_speed_2d(),
            0.0,
            "zero isolates direction from the known OTHER speed metric gap"
        );
        let requested =
            (-(target - start).normalize_or_zero().z).atan2((target - start).normalize_or_zero().x);
        if !slide {
            close(
                requested,
                scalar(expected[12]),
                2.0e-6,
                "independent requested heading",
            );
            assert!(
                (requested - object.get_orientation()).abs() <= object.effective_turn_rate() * DT
            );
        }
        let threshold = (object.movement.max_speed / 30.0) * object.ultra_accurate_slide_factor;
        let admitted_slide = ultra
            && object.ultra_accurate_slide_factor > 0.0
            && (target.x - start.x).abs() <= threshold
            && (target.z - start.z).abs() <= threshold;
        assert_eq!(
            admitted_slide, slide,
            "authored flag and distance select the intended slide branch"
        );
        assert_eq!(object.close_enough_dist, Some(1.0));
        assert!((target - start).length() > 1.0);
        println!(
            "ENTRY {name} yaw={:08x} direction={:08x},{:08x} position={:08x},{:08x} velocity={:08x},{:08x} target={:08x},{:08x}",
            object.get_orientation().to_bits(),
            direction.x.to_bits(),
            direction.y.to_bits(),
            start.x.to_bits(),
            start.z.to_bits(),
            object.movement.velocity.x.to_bits(),
            object.movement.velocity.z.to_bits(),
            target.x.to_bits(),
            target.z.to_bits()
        );
        println!(
            "REQUEST {name} angle={:08x} goal={:08x} step={:08x} forward={:08x} slide_factor={:08x} threshold={:08x} ultra={} sliding={}",
            requested.to_bits(),
            object.movement.max_speed.to_bits(),
            (object.effective_acceleration() * DT).to_bits(),
            object.forward_speed_2d().to_bits(),
            object.ultra_accurate_slide_factor.to_bits(),
            threshold.to_bits(),
            ultra,
            admitted_slide
        );
    }
    host_only();
    let tick = game.update_with_dt(DT);
    assert_eq!((tick.frame, tick.steps_run), (2, 1));
    assert!(!tick.budget_hit);
    assert_eq!(tick.accumulated_time_seconds, 0.0);
    host_only();
    assert_eq!(game.host_objects().len(), 1);
    let object = game.host_object(id).unwrap();
    active_guards(object, ultra);
    assert_eq!(object.cur_locomotor_name.as_deref(), Some(loco.as_str()));
    assert_eq!(object.movement.max_speed, binding.movement.max_speed);
    assert_eq!(object.movement.acceleration, binding.movement.acceleration);
    assert_eq!(object.braking, binding.braking);
    assert_eq!(object.movement.turn_rate, binding.movement.turn_rate);
    assert_eq!(
        object.ultra_accurate_slide_factor,
        binding.ultra_accurate_slide_factor
    );
    assert_eq!(object.movement.target_position, Some(target));
    assert!(object.status.moving && object.is_motive());
    let yaw = object.get_orientation();
    let direction = object.unit_direction_vector_2d();
    let friction = object.previous_acceleration();
    // This snapshot is the later apply_frictional_forces/integrate_physics_accel
    // contribution. Recover XZ only; gravity/clamping can still affect host Y.
    let marched = object.movement.velocity - friction;
    let position = object.get_position();
    println!(
        "RESULT {name} yaw={:08x} direction={:08x},{:08x} marched={:08x},{:08x} final={:08x},{:08x} friction={:08x},{:08x} position={:08x},{:08x}",
        yaw.to_bits(),
        direction.x.to_bits(),
        direction.y.to_bits(),
        marched.x.to_bits(),
        marched.z.to_bits(),
        object.movement.velocity.x.to_bits(),
        object.movement.velocity.z.to_bits(),
        friction.x.to_bits(),
        friction.z.to_bits(),
        position.x.to_bits(),
        position.z.to_bits()
    );
    // Freeze these independent baseline headings before editing production.
    // Expected force is never derived from candidate direction or motion.
    close(
        yaw,
        scalar(expected[28]),
        2.0e-6,
        "frozen supplied post-turn yaw",
    );
    close(
        direction.x,
        scalar(expected[29]),
        2.0e-6,
        "frozen supplied post-turn direction X",
    );
    close(
        direction.y,
        -scalar(expected[30]),
        2.0e-6,
        "frozen supplied post-turn direction Z",
    );
    if slide || name == "axis_nonzero" {
        close(
            yaw,
            scalar(expected[11]),
            2.0e-6,
            "unchanged axis/slide yaw",
        );
    } else {
        assert!(
            (yaw - scalar(expected[11])).abs() > 0.25,
            "actual turn precedes force"
        );
    }
    assert!(
        marched.x.hypot(marched.z) > 1.0,
        "scheduled force was nonzero"
    );
    assert!(
        (position - start).length() > 0.05,
        "one scheduled position step was nonzero"
    );
    close(
        position.x - start.x,
        marched.x * DT,
        2.0e-5,
        "one X position integration",
    );
    close(
        position.z - start.z,
        marched.z * DT,
        2.0e-5,
        "one Z position integration",
    );
    let lateral = marched.x * -direction.y + marched.z * direction.x;
    let coefficient = object.get_lateral_friction();
    assert!(coefficient > 0.0);
    close(
        friction.x,
        -coefficient * lateral * -direction.y,
        4.0e-5,
        "later lateral friction X",
    );
    close(
        friction.z,
        -coefficient * lateral * direction.x,
        4.0e-5,
        "later lateral friction Z",
    );
    if slide {
        assert!(
            friction.x.hypot(friction.z) > 0.01,
            "slide friction is independently observable"
        );
    }
    if assert_original_force {
        let expected_velocity = host_vector(&expected, 35);
        // Mat4/trig, f32 accumulation and friction reconstruction can differ by
        // a few ULPs. Each intended baseline failure exceeds 0.4 host units.
        close(
            marched.x,
            expected_velocity.x,
            3.0e-5,
            "original-adapted marched X",
        );
        close(
            marched.z,
            expected_velocity.z,
            3.0e-5,
            "original-adapted marched Z",
        );
        let expected_impulse = host_vector(&expected, 32);
        assert!(marched.x * expected_impulse.x + marched.z * expected_impulse.z > 0.0);
        if name != "axis_nonzero" {
            assert!(
                friction.x.hypot(friction.z) > 0.01,
                "corrected force retains distinct later lateral friction"
            );
        }
    }
}

#[test]
fn axis_positive_force_control() {
    run_case("axis_nonzero", true);
}
#[test]
fn ninety_degree_turn_keeps_entry_force_direction() {
    run_case("turn_90", true);
}
#[test]
fn thirty_degree_turn_keeps_entry_force_direction() {
    run_case("turn_30", true);
}
#[test]
fn reflected_ninety_degree_target_keeps_entry_force_direction() {
    run_case("reflect_90", true);
}
#[test]
fn reflected_thirty_degree_target_keeps_entry_force_direction() {
    run_case("reflect_30", true);
}
#[test]
fn different_entry_direction_is_preserved() {
    run_case("entry_reflect_90", true);
}
#[test]
fn close_ultra_accurate_slide_overrides_entry_direction() {
    run_case("slide_close", true);
}
#[test]
fn nearby_target_without_ultra_accurate_uses_entry_direction() {
    run_case("slide_close_flag_off", true);
}
#[test]
fn distant_ultra_accurate_target_uses_entry_direction() {
    run_case("slide_far", true);
}
#[test]
fn independently_frozen_heading_and_slide_admission_controls() {
    for name in NAMES {
        run_case(name, false);
    }
}
