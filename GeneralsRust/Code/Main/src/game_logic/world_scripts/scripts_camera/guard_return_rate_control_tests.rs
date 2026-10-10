//! C++ AIGuard.cpp:602-735: Return completion selects Idle, not guard radius.
use super::named_command_test_support::{execute, world};
use super::*;
use game_engine::common::ini::AIData;
use gamelogic::scripting::core::ScriptActionType;

pub(super) fn authored_rates(world: &mut GameLogic, return_rate: u32, idle_rate: u32) {
    world.set_ai_definition_base(AIData {
        guard_enemy_return_scan_rate: return_rate,
        guard_enemy_scan_rate: idle_rate,
        guard_inner_modifier_human: 4.0,
        ..AIData::default()
    });
}

pub(super) fn unfinished_return() -> (GameLogic, ObjectId) {
    let (mut world, id, _) = world();
    authored_rates(&mut world, 60, 7);
    world.host_object_mut(id).unwrap().vision_range = 100.0;
    assert_eq!(
        execute(
            &mut world,
            &[(ScriptActionType::NamedGuard, "NamedUnit", "")]
        ),
        1
    );
    let post = world.host_object(id).unwrap().guard_position.unwrap();
    world
        .host_object_mut(id)
        .unwrap()
        .set_position(post + Vec3::new(20.0, 0.0, 0.0));
    world.host_object_mut(id).unwrap().close_enough_dist = Some(1.0);
    world.return_guard_to_post(id);
    let unit = world.host_object(id).unwrap();
    let inner = world.host_std_guard_ranges(id).0;
    assert!(
        inner > 20.0,
        "witness must start inside the inner guard radius"
    );
    assert!(
        !unit.movement.path.is_empty(),
        "return must admit a real path"
    );
    assert!(
        !unit.waiting_for_path,
        "witness requires the admitted route, not a pending request"
    );
    let last = *unit.movement.path.last().unwrap();
    assert!(unit.host_locomotor_distance_to_goal(unit.get_position(), last) > 1.0);
    world
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(100));
    world.frame = 100;
    (world, id)
}

#[test]
fn actual_guard_order_and_unfinished_path_calibration() {
    let (owner, id) = unfinished_return();
    assert_eq!(
        owner
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(100)
    );
    assert_eq!(
        owner.host_object(id).unwrap().ai_state,
        AIState::GuardingArea
    );
}

#[test]
fn unfinished_return_keeps_original_return_rate_at_support_boundary() {
    let (mut owner, id) = unfinished_return();
    owner.update_support_states(&[id], 1.0 / 30.0);
    assert_eq!(
        owner
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(160)
    );
}
