//! Public scheduled TWO_LEGS force: authored loader -> admission -> one tick.
//! The unchanged original force-stage extraction supplies frozen expectations.
//! Its rotation is explicitly supplied; this is not full trajectory parity.

use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use generals_main::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding;
use generals_main::game_logic::{
    GameLogic, KindOf, LocomotorAppearance, Object, Team, ThingTemplate,
};
use generals_main::gameworld_shadow::{
    gameworld_movement_authority_live, shadow_coupled_tick_active, with_active_shadow,
};
use glam::Vec3;

const ORIGINAL: &str = include_str!("fixtures/legs_force_original.txt");
const DT: f32 = 1.0 / 30.0;
const START: Vec3 = Vec3::new(100.0, 0.0, 100.0);

fn scalar(hex: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(hex, 16).unwrap())
}
fn oracle_row(name: &str, lane: &str) -> Vec<&'static str> {
    let row = ORIGINAL
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|r| r.first() == Some(&lane) && r[1] == name)
        .unwrap();
    assert_eq!(row.len(), 30);
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
fn active_guards(object: &Object) {
    assert_eq!(object.loco_appearance, LocomotorAppearance::LegsTwo);
    assert!(object.is_kind_of(KindOf::Worker));
    assert!(!object.is_kind_of(KindOf::Infantry) && !object.is_kind_of(KindOf::Vehicle));
    assert!(object.is_mobile() && object.can_move() && object.is_alive());
    assert_eq!(
        (object.health.current, object.health.maximum),
        (100.0, 100.0)
    );
    assert!(!object.is_disabled() && !object.is_shock_stunned());
    assert!(!object.is_physics_held() && !object.is_freefall_disabled() && !object.is_rappelling());
    assert!(!object.allow_to_fall && !object.is_in_freefall && !object.shock_allow_bounce);
    assert!(object.contained_units().is_empty());
    assert_eq!(object.contained_items_mass, 0.0);
    assert_eq!(object.physics_get_mass(), 1.0);
    assert!(!object.worker_ai_update);
    assert!(
        object.weapon.is_none()
            && object.secondary_weapon.is_none()
            && object.tertiary_weapon.is_none()
    );
    assert!(!object.waiting_for_path && !object.is_blocked && !object.is_blocked_and_stuck);
    assert_eq!(object.num_frames_blocked, 0);
    assert!(object.movement.path.is_empty() && object.requested_destination.is_none());
    assert_eq!(object.path_extra_distance, 0.0);
    assert!(!object.moving_backwards && !object.doing_three_point_turn && !object.ultra_accurate);
    assert!(!object.is_braking);
    assert_eq!(object.braking_factor, 1.0);
    assert_eq!(object.turn_pivot_offset, 0.0);
    assert_eq!(object.wander_width_factor, 0.0);
    assert_eq!(object.min_speed, 0.0);
    assert_eq!(object.group_speed_factor, 1.0);
    assert!(object.bump_speed_limit >= object.movement.max_speed);
    assert_eq!(object.effective_max_speed(), object.movement.max_speed);
    assert!(object.desired_speed >= object.movement.max_speed);
    assert_eq!(object.physics_accel.x, 0.0);
    assert_eq!(object.physics_accel.z, 0.0);
    assert_eq!(object.ground_height, 0.0);
    assert!(!Object::height_treats_as_airborne(object.get_position().y));
    assert!(object.physics_current_overlap.is_none() && object.physics_previous_overlap.is_none());
    assert!(object.last_collidee.is_none());
}

fn run_case(name: &str, assert_original_force: bool) {
    host_only();
    let row = oracle_row(name, "host-impulse");
    let raw = oracle_row(name, "raw-frame");
    let (speed, acceleration, braking, turn_rate) = match name {
        "component_cancellation" => (24, 90, 90, 3600),
        "reverse_clamp" => (6, 1800, 90, 3600),
        "turn_snapshot" => (60, 90, 900, 3600),
        "turn_limited" => (60, 90, 900, 450),
        _ => (36, 90, 90, 3600),
    };
    let mut game = GameLogic::new();
    assert_eq!((game.get_frame(), game.host_objects().len()), (0, 0));
    // The first measured movement must be due: movement wakes from frame 1.
    let warm = game.update_with_dt(DT);
    assert_eq!((warm.frame, warm.steps_run), (1, 1));
    assert!(!warm.budget_hit);
    assert_eq!(warm.accumulated_time_seconds, 0.0);
    let loco = format!("LegsForceLoco_{name}");
    let unit = format!("LegsForceUnit_{name}");
    let ini = format!(
        "Locomotor {loco}\nSurfaces = GROUND\nSpeed = {speed}\nAcceleration = {acceleration}\nBraking = {braking}\nTurnRate = {turn_rate}\nAppearance = TWO_LEGS\nMinSpeed = 0\nWanderWidthFactor = 0\nZAxisBehavior = NO_Z_MOTIVE_FORCE\nEnd\n"
    );
    assert_eq!(load_locomotors_from_str(&ini).unwrap(), 1);
    {
        let store = get_locomotor_store();
        let parsed = store.find_template(&loco).unwrap();
        bits(parsed.max_speed, raw[14], "parsed frame speed");
        bits(parsed.acceleration, raw[15], "parsed frame acceleration");
        bits(parsed.braking, raw[16], "parsed frame braking");
    }
    let binding = resolve_host_locomotor_binding(&loco).unwrap();
    bits(binding.movement.max_speed, row[14], "bound host speed");
    bits(
        binding.movement.acceleration * DT,
        row[15],
        "bound host acceleration step",
    );
    bits(binding.braking * DT, row[16], "bound host braking step");
    let mut template = ThingTemplate::new(&unit);
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Worker)
        .set_locomotor_name(&loco);
    game.templates.insert(unit.clone(), template);
    let id = game.create_object(&unit, Team::USA, Vec3::ZERO).unwrap();
    let initial = host_vector(&row, 2);
    let goal = scalar(row[6]);
    let target = Vec3::new(
        START.x + goal.cos() * 1000.0,
        0.0,
        START.z - goal.sin() * 1000.0,
    );
    {
        let object = game.host_object_mut(id).unwrap();
        assert_eq!(object.movement.velocity, Vec3::ZERO);
        assert_eq!(object.physics_accel, Vec3::ZERO);
        assert_eq!(object.movement.max_speed, binding.movement.max_speed);
        assert_eq!(object.movement.acceleration, binding.movement.acceleration);
        assert_eq!(object.braking, binding.braking);
        assert_eq!(object.movement.turn_rate, binding.movement.turn_rate);
        object.set_position(START);
        object.set_orientation(scalar(row[5]));
        object.add_velocity(-object.movement.velocity);
        assert_eq!(object.movement.velocity, Vec3::ZERO);
        object.add_velocity(initial);
        assert_eq!(object.movement.velocity, initial);
        object.move_to(target);
        assert_eq!(
            object.movement.velocity, initial,
            "move entry must not scrub velocity"
        );
        assert_eq!(object.movement.target_position, Some(target));
        assert!(object.status.moving);
        active_guards(object);
        close(
            object.get_orientation(),
            scalar(row[5]),
            2.0e-6,
            "entry yaw",
        );
        let direction = object.unit_direction_vector_2d();
        close(direction.x, scalar(row[8]), 2.0e-6, "entry direction X");
        close(direction.y, -scalar(row[9]), 2.0e-6, "entry direction Z");
        close(
            object.forward_speed_2d(),
            scalar(row[18]),
            1.0e-5,
            "entry signed component-product speed",
        );
        let sign_sum = initial.x * direction.x + initial.z * direction.y;
        if name == "component_cancellation" {
            assert_eq!(
                sign_sum, 0.0,
                "opposing component products must cancel exactly"
            );
            assert!(
                object.forward_speed_2d() > 29.9,
                "zero sign sum chooses positive magnitude"
            );
        } else {
            assert_eq!(sign_sum.is_sign_negative(), name == "reverse_clamp");
        }
        // Source-shaped observation of the unchanged target normalization
        // boundary; this is not used to generate any force expectation.
        let normalized_goal = (target - START).normalize_or_zero();
        let requested = (-normalized_goal.z).atan2(normalized_goal.x);
        close(
            requested,
            scalar(row[20]),
            2.0e-6,
            "original requested-angle boundary",
        );
        let angle_coefficient =
            (requested - object.get_orientation()).abs() / std::f32::consts::FRAC_PI_4;
        println!(
            "REQUEST {name} normalized_angle={:08x} angle_coefficient={:08x}",
            requested.to_bits(),
            angle_coefficient.min(1.0).to_bits()
        );
        println!(
            "ENTRY {name} yaw={:08x} direction={:08x},{:08x} forward={:08x}",
            object.get_orientation().to_bits(),
            direction.x.to_bits(),
            direction.y.to_bits(),
            object.forward_speed_2d().to_bits()
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
    active_guards(object);
    assert_eq!(object.cur_locomotor_name.as_deref(), Some(loco.as_str()));
    assert_eq!(object.movement.max_speed, binding.movement.max_speed);
    assert_eq!(object.movement.acceleration, binding.movement.acceleration);
    assert_eq!(object.braking, binding.braking);
    assert_eq!(object.movement.target_position, Some(target));
    assert!(object.status.moving && object.is_motive());
    let yaw = object.get_orientation();
    let direction = object.unit_direction_vector_2d();
    let friction = object.previous_acceleration();
    // Only XZ is reconstructed: later gravity/clamping can still modify Y.
    let marched = object.movement.velocity - friction;
    let impulse = marched - initial;
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
    // These independent heading checks execute before the force comparison on
    // both baseline and candidate. Force never supplies an expected direction.
    close(yaw, scalar(row[7]), 2.0e-6, "frozen supplied post-turn yaw");
    close(
        direction.x,
        scalar(row[11]),
        2.0e-6,
        "frozen supplied post-turn direction X",
    );
    close(
        direction.y,
        -scalar(row[12]),
        2.0e-6,
        "frozen supplied post-turn direction Z",
    );
    if name.starts_with("turn_") {
        assert!(
            (yaw - scalar(row[5])).abs() > 0.25,
            "actual turn precedes force"
        );
    }
    assert!(
        impulse.x.hypot(impulse.z) > 1.0,
        "scheduled force was nonzero"
    );
    assert!(
        (position - START).length() > 0.1,
        "live scheduled position moved"
    );
    close(
        position.x - START.x,
        marched.x * DT,
        2.0e-5,
        "one X position integration",
    );
    close(
        position.z - START.z,
        marched.z * DT,
        2.0e-5,
        "one Z position integration",
    );
    // The original PhysicsUpdate lateral dot decomposition is intentionally
    // retained. Check it separately from the frozen force-stage expectation.
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
    if name.starts_with("turn_") || name == "component_cancellation" {
        assert!(
            friction.x.hypot(friction.z) > 0.01,
            "later friction is independently observable"
        );
    }
    if assert_original_force {
        let expected_velocity = host_vector(&row, 24);
        let expected_impulse = host_vector(&row, 21);
        // Accumulation, friction reconstruction and Mat4/trig representations
        // can differ by a few f32 ULPs. Distinguishing failures exceed 1 unit.
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
        assert!(
            impulse.x * expected_impulse.x + impulse.z * expected_impulse.z > 0.0,
            "original acceleration/braking sign"
        );
        close(
            impulse.x * direction.y - impulse.z * direction.x,
            0.0,
            3.0e-5,
            "force follows independently frozen post-turn heading",
        );
    }
}

#[test]
fn axis_positive_force_control() {
    run_case("axis_accel", true);
}
#[test]
fn diagonal_component_speed_changes_force_sign() {
    run_case("diagonal_sign", true);
}
#[test]
fn reflected_diagonal_preserves_original_force() {
    run_case("diagonal_reflect", true);
}
#[test]
fn cancelling_products_keep_positive_speed() {
    run_case("component_cancellation", true);
}
#[test]
fn negative_speed_clamps_original_gap() {
    run_case("reverse_clamp", true);
}
#[test]
fn turn_uses_entry_speed_for_clamped_force() {
    run_case("turn_snapshot", true);
}
#[test]
fn limited_turn_uses_entry_speed_for_clamped_force() {
    run_case("turn_limited", true);
}
#[test]
fn independently_frozen_turn_outputs() {
    run_case("turn_snapshot", false);
    run_case("turn_limited", false);
}
