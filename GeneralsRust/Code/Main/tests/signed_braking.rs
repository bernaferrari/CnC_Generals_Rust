//! Ordinary public fixed tick: loader -> template -> admission -> movement -> physics.
//! Expected live values are the explicitly adapted column of the extracted C++
//! force oracle. Raw original frame values are retained separately in the fixture.

use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use generals_main::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding;
use generals_main::game_logic::{
    GameLogic, KindOf, LocoGoalType, LocomotorAppearance, Object, Team, ThingTemplate,
};
use generals_main::gameworld_shadow::{
    gameworld_movement_authority_live, shadow_coupled_tick_active,
};
use glam::Vec3;

const ORIGINAL: &str = include_str!("fixtures/signed_braking_original.txt");
const DT: f32 = 1.0 / 30.0;
const TARGET: Vec3 = Vec3::new(1000.0, 0.0, 0.0);

fn bits(value: &str) -> u32 {
    u32::from_str_radix(value, 16).unwrap()
}
fn scalar(value: &str) -> f32 {
    f32::from_bits(bits(value))
}
fn rows(family: &str) -> Vec<Vec<&str>> {
    ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|row| row.first() == Some(&family))
        .collect()
}
fn eq_bits(actual: f32, expected: &str, context: &str) {
    assert_eq!(
        actual.to_bits(),
        bits(expected),
        "{context}: {actual} vs {}",
        scalar(expected)
    );
}
fn assert_isolated(object: &Object) {
    assert_eq!(object.loco_appearance, LocomotorAppearance::Other);
    assert_eq!(object.get_orientation(), 0.0);
    assert!(!object.is_braking);
    assert_eq!(object.braking_factor, 1.0);
    assert!(!object.ultra_accurate);
    assert!(!object.moving_backwards);
    assert!(!object.doing_three_point_turn);
    assert!(!object.waiting_for_path);
    assert!(!object.is_blocked);
    assert!(!object.is_blocked_and_stuck);
    assert_eq!(object.num_frames_blocked, 0);
    assert!(object.movement.path.is_empty());
    assert_eq!(object.path_extra_distance, 0.0);
    assert_eq!(object.group_speed_factor, 1.0);
    assert!(object.bump_speed_limit >= object.movement.max_speed);
    assert_eq!(object.effective_max_speed(), object.movement.max_speed);
    assert_ne!(object.locomotor_goal_type, LocoGoalType::PositionExplicit);
    assert!(object.desired_speed >= object.movement.max_speed);
    assert_eq!(object.wander_width_factor, 0.0);
    assert_eq!(object.contained_items_mass, 0.0);
    assert!(!object.worker_ai_update);
    assert!(object.dozer_task_build_target.is_none());
    assert!(object.dozer_task_repair_target.is_none());
    assert!(object.preferred_dock_id.is_none());
    assert_eq!(object.physics_get_mass(), 1.0);
    assert_eq!(object.physics_accel.x, 0.0);
    assert_eq!(object.physics_accel.z, 0.0);
    assert_eq!(object.movement.velocity.z, 0.0);
    assert_eq!(object.ground_height, 0.0);
    assert!(!Object::height_treats_as_airborne(object.get_position().y));
    assert!(object.physics_current_overlap.is_none());
    assert!(object.physics_previous_overlap.is_none());
    assert!(object.last_collidee.is_none());
}

fn run_case(name: &str) {
    let cases = rows("live");
    assert_eq!(cases.len(), 9);
    let row = cases.iter().find(|r| r[1] == name).unwrap();
    assert_eq!(row.len(), 20);
    assert!(!shadow_coupled_tick_active());
    assert!(!gameworld_movement_authority_live());
    let mut game = GameLogic::new();
    assert_eq!(game.get_frame(), 0);
    assert_eq!(game.host_objects().len(), 0);
    // Movement first wakes at frame 1. A frame-0-only tick is not evidence.
    let warm = game.update_with_dt(DT);
    assert_eq!((warm.frame, warm.steps_run), (1, 1));
    assert!(!warm.budget_hit);
    assert_eq!(warm.accumulated_time_seconds, 0.0);
    let loco_name = format!("SignedForceLocomotor_{name}");
    let unit_name = format!("SignedForceUnit_{name}");
    // Zero braking gives infinite approach distance. Keep its minimum equal to
    // the goal so approach cannot replace the tested speed gap.
    let minimum = if name.ends_with("zero") { 60.0 } else { 0.0 };
    let ini = format!(
        "Locomotor {loco_name}\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 30\nBraking = {}\nAppearance = OTHER\nMinSpeed = {minimum}\nWanderWidthFactor = 0\nZAxisBehavior = NO_Z_MOTIVE_FORCE\nEnd\n",
        row[2]
    );
    assert_eq!(load_locomotors_from_str(&ini).unwrap(), 1);
    {
        let store = get_locomotor_store();
        let parsed = store.find_template(&loco_name).unwrap();
        eq_bits(parsed.braking, row[8], "parsed original frame braking");
        eq_bits(parsed.max_speed, row[7], "parsed original frame goal");
    }
    let binding = resolve_host_locomotor_binding(&loco_name).unwrap();
    eq_bits(binding.braking, row[5], "bound host braking");
    eq_bits(binding.movement.max_speed, row[4], "bound host goal");
    let mut template = ThingTemplate::new(&unit_name);
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Worker)
        .set_locomotor_name(&loco_name);
    game.templates.insert(unit_name.clone(), template);
    let id = game
        .create_object(&unit_name, Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(game.host_objects().len(), 1);
    {
        let object = game.host_object_mut(id).unwrap();
        eq_bits(
            object.braking,
            row[5],
            "admitted braking without field repair",
        );
        eq_bits(
            object.movement.max_speed,
            row[4],
            "admitted goal without field repair",
        );
        assert_eq!(object.min_speed, minimum);
        assert_eq!(object.loco_appearance, binding.appearance);
        assert!(object.is_mobile());
        assert!(object.can_move());
        assert_eq!(object.movement.velocity, Vec3::ZERO);
        assert_eq!(object.physics_accel, Vec3::ZERO);
        object.set_orientation(0.0);
        object.add_velocity(Vec3::new(scalar(row[3]), 0.0, 0.0));
        object.move_to(TARGET);
        assert_eq!(object.movement.target_position, Some(TARGET));
        assert!(object.status.moving);
        assert!(object.is_alive());
        assert_isolated(object);
        eq_bits(object.braking * DT, row[17], "signed host step");
        eq_bits(
            object.movement.max_speed - object.movement.velocity.x,
            row[18],
            "host gap",
        );
        if name.ends_with("equal") {
            assert_eq!(
                (object.braking * DT).abs().to_bits(),
                (object.movement.max_speed - object.movement.velocity.x)
                    .abs()
                    .to_bits(),
                "strict equality must be real after parsing/binding"
            );
        }
        if name == "zero_gap" {
            assert_eq!(object.movement.velocity.x, object.movement.max_speed);
        }
    }
    let tick = game.update_with_dt(DT);
    assert_eq!((tick.frame, tick.steps_run), (2, 1));
    assert!(!tick.budget_hit);
    assert_eq!(tick.accumulated_time_seconds, 0.0);
    assert!(!shadow_coupled_tick_active());
    assert!(!gameworld_movement_authority_live());
    assert_eq!(game.host_objects().len(), 1);
    let object = game.host_object(id).unwrap();
    assert_isolated(object);
    assert!(object.is_alive());
    assert!(object.is_motive());
    assert!(object.status.moving);
    assert_eq!(object.movement.target_position, Some(TARGET));
    assert!(object.get_position().x < TARGET.x - 100.0);
    println!(
        "{name}: frame={} speed={:08x} x={:08x} brake={:08x} goal={:08x}",
        tick.frame,
        object.movement.velocity.x.to_bits(),
        object.get_position().x.to_bits(),
        object.braking.to_bits(),
        object.movement.max_speed.to_bits()
    );
    eq_bits(object.movement.velocity.x, row[15], name);
    eq_bits(
        object.get_position().x,
        row[16],
        "one host fixed-frame displacement",
    );
}

#[test]
fn negative_large_clamps_to_signed_gap() {
    run_case("negative_large");
}
#[test]
fn negative_below_preserves_accelerating_sign() {
    run_case("negative_below");
}
#[test]
fn negative_equal_preserves_strict_comparison() {
    run_case("negative_equal");
}
#[test]
fn positive_large_clamps() {
    run_case("positive_large");
}
#[test]
fn positive_below_brakes() {
    run_case("positive_below");
}
#[test]
fn positive_equal_brakes() {
    run_case("positive_equal");
}
#[test]
fn positive_zero_rate_keeps_velocity() {
    run_case("positive_zero");
}
#[test]
fn negative_zero_rate_keeps_velocity() {
    run_case("negative_zero");
}
#[test]
fn zero_gap_skips_force() {
    run_case("zero_gap");
}

#[test]
fn original_scalar_controls_and_adapter_distinction_are_retained() {
    let controls = rows("scalar");
    assert_eq!(controls.len(), 9);
    assert_eq!(controls[0][3], "bf800000"); // -2 braking clamps to negative gap.
    assert_eq!(controls[1][3], "3f000000"); // Smaller negative braking pushes away.
    assert_eq!(controls[2][3], "3f800000"); // Equality deliberately does not clamp.
    assert_eq!(controls[6][3], "80000000"); // Original force retains negative zero.
    assert_eq!(controls[7][3], "00000000");
    assert_eq!(controls[8][5], "0"); // Zero gap never applies force.
    assert!(rows("live").iter().any(|r| r[9] != r[12]));
    assert!(rows("live").iter().any(|r| r[10] != r[13]));
}
