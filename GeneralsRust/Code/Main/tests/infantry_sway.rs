//! Public fixed-tick infantry wander: authored INI -> admission -> movement.
//! Phase expectations are extracted original C++ arithmetic with a documented
//! host-seconds adapter. Gate expectations are source-derived, not execution of
//! the original dispatcher. Explicit phase inputs do not test constructor RNG.

use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use generals_main::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding;
use generals_main::game_logic::{
    GameLogic, KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate,
};
use generals_main::gameworld_shadow::{
    gameworld_movement_authority_live, shadow_coupled_tick_active,
};
use glam::Vec3;

const ORIGINAL: &str = include_str!("fixtures/infantry_sway_original.txt");
const DT: f32 = 1.0 / 30.0;

fn scalar(value: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(value, 16).unwrap())
}
fn row(name: &str) -> Vec<&'static str> {
    let row = ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|r| r.first() == Some(&"live") && r[1] == name)
        .unwrap();
    assert_eq!(row.len(), 20, "{name}");
    row
}
fn close(actual: f32, expected: f32, field: &str) {
    assert!(
        (actual - expected).abs() < 2.0e-5,
        "{field}: {actual} != {expected}"
    );
}
fn phase(object: &Object) -> (u32, u32, bool) {
    (
        object.wander_angle_offset.to_bits(),
        object.wander_offset_increment.to_bits(),
        object.wander_offset_increasing,
    )
}
fn expected_phase(row: &[&str]) -> (u32, u32, bool) {
    (
        scalar(row[17]).to_bits(),
        scalar(row[8]).to_bits(),
        row[18] == "1",
    )
}
fn active_guards(object: &Object, appearance: LocomotorAppearance) {
    assert_eq!(object.loco_appearance, appearance);
    assert!(object.is_mobile() && object.can_move() && object.is_alive());
    assert!(!object.is_disabled() && !object.is_shock_stunned());
    assert!(
        !object.shock_allow_bounce,
        "no later bounce may replace friction acceleration"
    );
    assert!(!object.worker_ai_update);
    assert!(!object.waiting_for_path && !object.is_blocked_and_stuck);
    assert!(!object.moving_backwards && !object.doing_three_point_turn);
    assert!(!object.ultra_accurate);
    assert!(object.movement.path.is_empty());
    assert_eq!(object.ground_height, 0.0);
}

fn admit(
    name: &str,
    appearance: &str,
    width: f32,
    airborne: bool,
    downhill: bool,
) -> (GameLogic, ObjectId) {
    assert!(!shadow_coupled_tick_active() && !gameworld_movement_authority_live());
    let mut game = GameLogic::new();
    assert_eq!((game.get_frame(), game.host_objects().len()), (0, 0));
    // Movement is scheduled from frame 1. An empty frame-0 tick is not coverage.
    let warm = game.update_with_dt(DT);
    assert_eq!((warm.frame, warm.steps_run), (1, 1));
    assert!(!warm.budget_hit);
    let loco = format!("InfantryWanderLoco_{name}");
    let unit = format!("InfantryWanderUnit_{name}");
    let ini = format!(
        "Locomotor {loco}\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 30\nBraking = 30\nTurnRate = 3600\nAppearance = {appearance}\nWanderWidthFactor = {width}\nWanderLengthFactor = 1\nDownhillOnly = {}\nAllowAirborneMotiveForce = {}\nZAxisBehavior = NO_Z_MOTIVE_FORCE\nEnd\n",
        if downhill { "Yes" } else { "No" },
        if airborne { "Yes" } else { "No" }
    );
    assert_eq!(load_locomotors_from_str(&ini).unwrap(), 1);
    let binding = resolve_host_locomotor_binding(&loco).unwrap();
    assert_eq!(
        get_locomotor_store()
            .find_template(&loco)
            .unwrap()
            .wander_width_factor,
        width
    );
    assert_eq!(binding.wander_width_factor, width);
    let mut template = ThingTemplate::new(&unit);
    // Worker is mobile but does not trigger Infantry/Vehicle appearance repair.
    // No WorkerAI module is installed by this synthetic authored template.
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Worker)
        .set_locomotor_name(&loco);
    game.templates.insert(unit.clone(), template);
    let id = game.create_object(&unit, Team::USA, Vec3::ZERO).unwrap();
    let object = game.host_object(id).unwrap();
    assert_eq!(game.host_objects().len(), 1);
    assert_eq!(object.loco_appearance, binding.appearance);
    assert_eq!(object.wander_width_factor, width);
    assert_eq!(object.allow_motive_force_while_airborne, airborne);
    assert_eq!(object.downhill_only, downhill);
    assert_eq!(object.movement.max_speed, binding.movement.max_speed);
    assert_eq!(object.braking, binding.braking);
    assert_eq!(object.movement.velocity, Vec3::ZERO);
    assert_eq!(object.physics_accel, Vec3::ZERO);
    active_guards(object, binding.appearance);
    (game, id)
}

fn install_phase(object: &mut Object, row: &[&str]) {
    // Controlled prior locomotor state, installed only after normal admission.
    // Do not repair authored binding fields or RNG initialization/ownership.
    object.wander_angle_offset = scalar(row[7]);
    object.wander_offset_increment = scalar(row[8]);
    object.wander_offset_increasing = row[10] == "1";
}
fn prepare_motion(object: &mut Object, row: &[&str], position: Vec3, goal_y: f32) -> Vec3 {
    object.set_position(position);
    object.set_orientation(scalar(row[5]));
    let velocity = Vec3::new(scalar(row[2]), scalar(row[3]), scalar(row[4]));
    // Two public operations avoid rounding the combined (desired - current)
    // delta during a sign reversal; the requested input must be exact.
    object.add_velocity(-object.movement.velocity);
    object.add_velocity(velocity);
    assert_eq!(object.movement.velocity, velocity);
    let goal = scalar(row[6]);
    let target = Vec3::new(
        position.x + goal.cos() * 1000.0,
        goal_y,
        position.z - goal.sin() * 1000.0,
    );
    object.move_to(target);
    assert_eq!(object.movement.target_position, Some(target));
    assert!(object.status.moving);
    close(
        object.forward_speed_2d(),
        scalar(row[15]),
        "signed host forward speed",
    );
    target
}
fn tick(game: &mut GameLogic, expected_frame: u32) {
    let result = game.update_with_dt(DT);
    assert_eq!((result.frame, result.steps_run), (expected_frame, 1));
    assert!(!result.budget_hit);
    assert_eq!(result.accumulated_time_seconds, 0.0);
    assert!(!shadow_coupled_tick_active() && !gameworld_movement_authority_live());
    assert_eq!(game.host_objects().len(), 1);
}
fn assert_active_result(object: &Object, row: &[&str], before: Vec3, target: Vec3) {
    active_guards(object, LocomotorAppearance::LegsTwo);
    assert!(object.is_motive() && object.status.moving);
    assert!(!object.is_braking);
    assert_eq!(object.movement.target_position, Some(target));
    assert_eq!(object.num_frames_blocked, 0);
    assert_eq!(phase(object), expected_phase(row), "{} phase", row[1]);
    let desired = scalar(row[19]);
    close(object.get_orientation(), desired, "wander consumer heading");
    assert!(
        (object.get_orientation() - scalar(row[5])).abs() > 0.02,
        "positive turning witness"
    );
    // Consumer guard for the existing host impulse/integration path; this is
    // not an original C++ force oracle. The authored rates bound impulse to 1.
    let initial = Vec3::new(scalar(row[2]), scalar(row[3]), scalar(row[4]));
    // Movement integrates position before shock.rs applies friction. The public
    // previous acceleration records that later velocity increment, so subtract
    // it to observe the marched velocity without disabling/repairing physics.
    let marched_velocity = object.movement.velocity - object.previous_acceleration();
    let change = marched_velocity - initial;
    let planar_change = Vec3::new(change.x, 0.0, change.z);
    assert!(
        planar_change.length() > 0.5,
        "appearance mover applied force"
    );
    assert!(
        planar_change.length() < 1.001,
        "single authored acceleration step"
    );
    let heading = Vec3::new(desired.cos(), 0.0, -desired.sin());
    close(
        planar_change.x * heading.z - planar_change.z * heading.x,
        0.0,
        "impulse follows wander heading",
    );
    let position = object.get_position();
    assert!(
        (position - before).length() > 0.001,
        "scheduled tick really moved"
    );
    close(
        position.x - before.x,
        marched_velocity.x * DT,
        "one fixed-step X integration",
    );
    close(
        position.z - before.z,
        marched_velocity.z * DT,
        "one fixed-step Z integration",
    );
    println!(
        "{}: phase={:?} yaw={:08x} velocity={:?} later_acceleration={:?} marched_velocity={:?} position={:?}",
        row[1],
        phase(object),
        object.get_orientation().to_bits(),
        object.movement.velocity,
        object.previous_acceleration(),
        marched_velocity,
        position
    );
}
fn active_case(name: &str) {
    let row = row(name);
    let (mut game, id) = admit(name, "TWO_LEGS", scalar(row[9]), false, false);
    let object = game.host_object_mut(id).unwrap();
    install_phase(object, &row);
    let target = prepare_motion(object, &row, Vec3::ZERO, 0.0);
    assert!(!Object::height_treats_as_airborne(object.get_position().y));
    tick(&mut game, 2);
    assert_active_result(game.host_object(id).unwrap(), &row, Vec3::ZERO, target);
}

#[test]
fn signed_forward_frame_speed() {
    active_case("forward");
}
#[test]
fn signed_backward_frame_speed() {
    active_case("backward");
}
#[test]
fn lateral_velocity_does_not_advance_phase() {
    active_case("lateral");
}
#[test]
fn vertical_velocity_does_not_advance_phase() {
    active_case("vertical");
}
#[test]
fn vertical_velocity_does_not_contaminate_forward_speed() {
    active_case("vertical_contamination");
}
#[test]
fn mixed_component_product_speed() {
    active_case("mixed");
}
#[test]
fn decreasing_phase_direction() {
    active_case("decreasing");
}
#[test]
fn strict_limit_overshoot_is_not_clamped() {
    active_case("near_limit");
}

#[test]
fn consecutive_ticks_preserve_phase_across_speed_reversal() {
    let first = row("forward");
    let second = row("reverse_second");
    let (mut game, id) = admit("sequence", "TWO_LEGS", 1.0, false, false);
    let object = game.host_object_mut(id).unwrap();
    install_phase(object, &first);
    let target = prepare_motion(object, &first, Vec3::ZERO, 0.0);
    tick(&mut game, 2);
    assert_active_result(game.host_object(id).unwrap(), &first, Vec3::ZERO, target);
    let object = game.host_object_mut(id).unwrap();
    let previous = phase(object);
    let start = object.get_position();
    let target = prepare_motion(object, &second, start, 0.0);
    assert_eq!(phase(object), previous, "motion setup must preserve phase");
    assert_eq!(previous.0, scalar(second[7]).to_bits());
    tick(&mut game, 3);
    assert_active_result(game.host_object(id).unwrap(), &second, start, target);
}

fn excluded_appearance(name: &str, appearance: &str, expected: LocomotorAppearance, width: f32) {
    let input = row("near_limit");
    let (mut game, id) = admit(name, appearance, width, false, false);
    let object = game.host_object_mut(id).unwrap();
    install_phase(object, &input);
    let target = prepare_motion(object, &input, Vec3::ZERO, 0.0);
    let initial = phase(object);
    tick(&mut game, 2);
    let object = game.host_object(id).unwrap();
    assert_eq!(
        object.loco_appearance, expected,
        "appearance survived dispatch"
    );
    assert_eq!(object.wander_width_factor, width);
    assert!(object.status.moving && object.is_motive());
    assert_eq!(object.movement.target_position, Some(target));
    assert!(
        object.get_position().x > 0.1,
        "negative control still moved"
    );
    assert_eq!(
        phase(object),
        initial,
        "{appearance} must not execute infantry wander"
    );
    close(object.get_orientation(), 0.0, "no infantry heading bias");
}
#[test]
fn climber_excludes_wander() {
    excluded_appearance("climber", "CLIMBER", LocomotorAppearance::Climber, 1.0);
}
#[test]
fn treads_excludes_wander() {
    excluded_appearance("treads", "TREADS", LocomotorAppearance::Treads, 1.0);
}
#[test]
fn four_wheels_excludes_wander() {
    excluded_appearance(
        "wheels",
        "FOUR_WHEELS",
        LocomotorAppearance::WheelsFour,
        1.0,
    );
}
#[test]
fn motorcycle_excludes_wander() {
    excluded_appearance(
        "motorcycle",
        "MOTORCYCLE",
        LocomotorAppearance::Motorcycle,
        1.0,
    );
}
#[test]
fn hover_excludes_wander() {
    excluded_appearance("hover", "HOVER", LocomotorAppearance::Hover, 1.0);
}
#[test]
fn wings_excludes_wander() {
    excluded_appearance("wings", "WINGS", LocomotorAppearance::Wings, 1.0);
}
#[test]
fn thrust_excludes_wander() {
    excluded_appearance("thrust", "THRUST", LocomotorAppearance::Thrust, 1.0);
}
#[test]
fn other_excludes_wander() {
    excluded_appearance("other", "OTHER", LocomotorAppearance::Other, 1.0);
}
#[test]
fn zero_width_preserves_phase() {
    excluded_appearance("zero_width", "TWO_LEGS", LocomotorAppearance::LegsTwo, 0.0);
}

fn skip_then_resume(gate: &str) {
    let input = row("near_limit");
    let (mut game, id) = admit(gate, "TWO_LEGS", 1.0, false, gate == "downhill");
    let object = game.host_object_mut(id).unwrap();
    install_phase(object, &input);
    let position = if gate == "airborne" {
        Vec3::new(0.0, 100.0, 0.0)
    } else {
        Vec3::ZERO
    };
    let goal_y = if gate == "downhill" {
        100.0
    } else {
        position.y
    };
    let target = prepare_motion(object, &input, position, goal_y);
    if gate == "blocked" {
        // Public prior collision state, not a change to the template/binding.
        object.is_blocked = true;
        object.cur_max_blocked_speed = 0.0;
        assert!(object.effective_max_speed() > object.cur_max_blocked_speed);
    } else if gate == "downhill" {
        assert!(object.downhill_only_blocks_goal(position.y, target.y));
    } else {
        assert!(Object::height_treats_as_airborne(position.y));
        assert!(!object.allow_motive_force_while_airborne);
    }
    active_guards(object, LocomotorAppearance::LegsTwo);
    let initial = phase(object);
    tick(&mut game, 2);
    let object = game.host_object(id).unwrap();
    assert_eq!(object.movement.target_position, Some(target));
    assert!(object.is_motive() && object.status.moving);
    close(
        object.get_orientation(),
        0.0,
        "gate suppressed appearance turn",
    );
    if gate == "blocked" {
        assert!(
            object.num_frames_blocked > 0,
            "blocked survived unclamping tests"
        );
        close(
            object.movement.velocity.x,
            0.0,
            "blocked branch scrubbed forward velocity",
        );
    } else if gate == "downhill" {
        assert!(object.downhill_only_blocks_goal(position.y, target.y));
    } else {
        assert!(Object::height_treats_as_airborne(object.get_position().y));
    }
    assert_eq!(
        phase(object),
        initial,
        "{gate} skip must preserve offset and direction"
    );
    let object = game.host_object_mut(id).unwrap();
    object.is_blocked = false;
    let target = prepare_motion(object, &input, Vec3::ZERO, 0.0);
    assert_eq!(phase(object), initial, "resume setup never rewrites phase");
    assert!(!object.downhill_only_blocks_goal(0.0, 0.0));
    assert!(!Object::height_treats_as_airborne(0.0));
    tick(&mut game, 3);
    let object = game.host_object(id).unwrap();
    assert_eq!(
        phase(object),
        expected_phase(&input),
        "{gate} resumed from pre-skip state"
    );
    close(
        object.get_orientation(),
        scalar(input[19]),
        "resumed heading",
    );
    assert!(object.get_position().x > 0.1 && object.is_motive());
    assert_eq!(object.movement.target_position, Some(target));
    assert_eq!(object.num_frames_blocked, 0);
}
#[test]
fn blocked_skip_then_resume_preserves_phase() {
    skip_then_resume("blocked");
}
#[test]
fn downhill_skip_then_resume_preserves_phase() {
    skip_then_resume("downhill");
}
#[test]
fn airborne_skip_then_resume_preserves_phase() {
    skip_then_resume("airborne");
}

#[test]
fn authored_airborne_motive_permission_admits_wander() {
    let input = row("near_limit");
    let (mut game, id) = admit("airborne_allowed", "TWO_LEGS", 1.0, true, false);
    let object = game.host_object_mut(id).unwrap();
    install_phase(object, &input);
    let start = Vec3::new(0.0, 100.0, 0.0);
    let target = prepare_motion(object, &input, start, start.y);
    assert!(Object::height_treats_as_airborne(start.y));
    tick(&mut game, 2);
    assert_active_result(game.host_object(id).unwrap(), &input, start, target);
}

#[test]
fn stationary_phase_still_biases_heading() {
    active_case("stationary");
}
#[test]
fn negative_heading_uses_component_products() {
    active_case("negative_heading");
}
#[test]
fn diagonal_reverse_preserves_signed_speed() {
    active_case("diagonal_reverse");
}
#[test]
fn negative_width_keeps_original_strict_threshold() {
    active_case("negative_width");
}
#[test]
fn negative_increment_is_preserved() {
    active_case("negative_increment");
}

#[test]
fn scalar_phase_boundaries_match_extracted_original() {
    // Supporting arithmetic coverage only. Public ticks above establish reachability.
    let mut checked = 0;
    for line in ORIGINAL.lines() {
        let r = line.split_whitespace().collect::<Vec<_>>();
        if r.first() != Some(&"scalar") {
            continue;
        }
        assert_eq!(r.len(), 12);
        let inputs = (2..7).map(|i| scalar(r[i])).collect::<Vec<_>>();
        // Zero-increment fallback and exceptional NaN normalization remain
        // explicit residuals. Never claim those original controls match Main.
        if inputs.iter().any(|v| !v.is_finite()) || scalar(r[5]) == 0.0 {
            continue;
        }
        let mut object = Object::new(ThingTemplate::new("WanderScalar"), ObjectId(901), Team::USA);
        object.wander_width_factor = scalar(r[6]);
        object.wander_angle_offset = scalar(r[4]);
        object.wander_offset_increment = scalar(r[5]);
        object.wander_offset_increasing = r[7] == "1";
        object.tick_wander_angle_offset(scalar(r[2]));
        assert_eq!(
            phase(&object),
            (scalar(r[8]).to_bits(), scalar(r[9]).to_bits(), r[10] == "1"),
            "{}",
            r[1]
        );
        checked += 1;
    }
    assert_eq!(checked, 17);
}

#[test]
fn original_and_host_adapter_rounding_are_distinct() {
    let mixed = row("mixed");
    assert_ne!(
        mixed[11], mixed[16],
        "component-first and speed-first frame adaptation differ"
    );
    assert_ne!(
        mixed[12], mixed[17],
        "raw and host-adapted offset outputs remain distinct"
    );
    let zero_increment = row("zero_increment");
    assert_eq!(
        zero_increment[7], zero_increment[17],
        "original does not replace zero increment"
    );
}
