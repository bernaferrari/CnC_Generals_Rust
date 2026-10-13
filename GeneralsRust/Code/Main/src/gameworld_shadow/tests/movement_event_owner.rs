//! Optional diagnostic movement admission keeps the driving owner's receipts.
use super::*;
use crate::game_logic::{ObjectId, Player};
use crate::presentation_frame::{PresentationEvent, PresentationFrame};

fn admitted_world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "MovementOwner", true));
    let mut template = ThingTemplate::new("MovementBoundaryUnit");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    (logic, id)
}

fn missing_mapping_retains_whole_suffix_until_a_later_session_case() {
    let (mut logic, a) = admitted_world();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let a_entity = shadow.entity_for_host(a).unwrap();
    let b = logic
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    logic.movement_events.clear();
    for (id, x) in [(a, 20.0), (b, 30.0), (a, 40.0)] {
        let (object, mut writer) = logic.host_object_and_movement_events_mut(id).unwrap();
        object.set_destination(Vec3::new(x, 0.0, 0.0), &mut writer);
    }
    // Missing observer admission cannot block the actual object command.
    assert!(shadow.entity_for_host(b).is_none());
    assert_eq!(
        logic.host_object(a).unwrap().movement.target_position,
        Some(Vec3::new(40.0, 0.0, 0.0))
    );
    assert_eq!(
        logic.host_object(b).unwrap().movement.target_position,
        Some(Vec3::new(30.0, 0.0, 0.0))
    );
    assert_eq!(shadow.world().entity(a_entity).unwrap().move_target, None);
    assert!(logic.movement_events.has_pending(a));
    assert!(logic.movement_events.has_pending(b));
    observe_completed_owner(&mut shadow, &mut logic);
    let b_entity = shadow.entity_for_host(b).unwrap();
    assert_eq!(
        shadow.world().entity(a_entity).unwrap().move_target,
        Some([40.0, 0.0, 0.0])
    );
    assert_eq!(
        shadow.world().entity(b_entity).unwrap().move_target,
        Some([30.0, 0.0, 0.0])
    );
    let receipts = logic.movement_events.snapshot_completed().to_vec();
    assert_eq!(receipts.len(), 3);
    assert!(
        receipts
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.movement_events.snapshot_completed(), receipts);
    let frame = PresentationFrame::build_from_logic(&mut logic, 0);
    let moves: Vec<_> = frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::MoveOrdered {
                unit, destination, ..
            } => Some((*unit, *destination)),
            _ => None,
        })
        .collect();
    assert_eq!(
        moves,
        [
            (a, [20.0, 0.0, 0.0]),
            (b, [30.0, 0.0, 0.0]),
            (a, [40.0, 0.0, 0.0])
        ]
    );
    assert!(logic.movement_events.take_completed().is_empty());
}

#[test]
fn missing_mapping_retains_whole_suffix_until_a_later_session() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "missing_mapping_retains_whole_suffix_until_a_later_session",
        missing_mapping_retains_whole_suffix_until_a_later_session_case,
    );
}

fn eager_prefix_late_suffix_and_foreign_session_remain_once_and_ordered_case() {
    let (mut owner, a) = admitted_world();
    let b = owner
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let (mut other, other_a) = admitted_world();
    assert_eq!(a, other_a);
    let owner_session = owner.movement_events.session().clone();
    let other_session = other.movement_events.session().clone();
    let mut shadow = GameWorldShadow::new(64);
    let mut other_shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&owner);
    other_shadow.sync_from_host(&other);
    let a_entity = shadow.entity_for_host(a).unwrap();
    let b_entity = shadow.entity_for_host(b).unwrap();
    let other_entity = other_shadow.entity_for_host(other_a).unwrap();
    owner.movement_events.clear();
    other.movement_events.clear();
    {
        let (object, mut writer) = owner.host_object_and_movement_events_mut(a).unwrap();
        object.set_destination(Vec3::new(20.0, 0.0, 0.0), &mut writer);
    }

    observe_completed_owner(&mut shadow, &mut owner);
    // An admitted prefix must never be replayed over later diagnostic state.
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(a_entity)
        .unwrap()
        .move_target = Some([99.0, 0.0, 0.0]);
    // Path following also changes current local targets without replaying the
    // accepted goal observation. Full session sync must see this newer value.
    owner.host_object_mut(a).unwrap().movement.target_position = Some(Vec3::new(99.0, 0.0, 0.0));
    let (object, mut writer) = owner.host_object_and_movement_events_mut(b).unwrap();
    object.set_destination(Vec3::new(30.0, 0.0, 0.0), &mut writer);
    let (object, mut writer) = other.host_object_and_movement_events_mut(other_a).unwrap();
    object.set_destination(Vec3::new(80.0, 0.0, 0.0), &mut writer);
    observe_completed_owner(&mut other_shadow, &mut other);
    let _ = observe_completed_owner(&mut other_shadow, &mut other);
    assert_eq!(
        other_shadow
            .world()
            .entity(other_entity)
            .unwrap()
            .move_target,
        Some([80.0, 0.0, 0.0])
    );
    assert!(owner.movement_events.has_pending(b));
    let _ = observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(
        shadow.world().entity(a_entity).unwrap().move_target,
        Some([99.0, 0.0, 0.0])
    );
    assert_eq!(
        shadow.world().entity(b_entity).unwrap().move_target,
        Some([30.0, 0.0, 0.0])
    );
    let receipts = owner.movement_events.snapshot_completed();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].unit, a);
    assert_eq!(receipts[1].unit, b);
    assert!(receipts.iter().all(|event| event.session == owner_session));
    assert_eq!(other.movement_events.snapshot_completed().len(), 1);
    assert_eq!(
        other.movement_events.snapshot_completed()[0].session,
        other_session
    );
    let frame = PresentationFrame::build_from_logic(&mut owner, 0);
    assert_eq!(
        frame
            .events
            .iter()
            .filter(|event| matches!(event, PresentationEvent::MoveOrdered { .. }))
            .count(),
        2
    );
    assert!(owner.movement_events.take_completed().is_empty());
    assert_eq!(other.movement_events.snapshot_completed().len(), 1);
}

#[test]
fn eager_prefix_late_suffix_and_foreign_session_remain_once_and_ordered() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "eager_prefix_late_suffix_and_foreign_session_remain_once_and_ordered",
        eager_prefix_late_suffix_and_foreign_session_remain_once_and_ordered_case,
    );
}

fn restoring_a_snapshot_baselines_only_the_receiving_world_output_case() {
    let (mut owner, id) = admitted_world();
    let (object, mut writer) = owner.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(Vec3::new(20.0, 0.0, 30.0), &mut writer);
    run_owned_host_boundary(None, &mut owner);
    let completed = owner.movement_events.snapshot_completed().to_vec();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut saved = builder.create_world_snapshot(&owner).unwrap();
    let (mut receiver, receiver_id) = admitted_world();
    assert_eq!(receiver_id, id);
    let (object, mut writer) = receiver.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(Vec3::new(80.0, 0.0, 90.0), &mut writer);
    let pending = receiver.movement_events.snapshot_pending().to_vec();
    let receiver_session = receiver.movement_events.session().clone();
    let version = saved.version;
    saved.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&saved, &mut receiver)
            .is_err()
    );
    assert_eq!(receiver.movement_events.snapshot_pending(), pending);
    assert_eq!(receiver.movement_events.session(), &receiver_session);
    saved.version = version;
    builder
        .restore_from_snapshot(&saved, &mut receiver)
        .unwrap();
    assert_eq!(owner.movement_events.snapshot_completed(), completed);
    assert_eq!(
        receiver.host_object(id).unwrap().movement.target_position,
        Some(Vec3::new(20.0, 0.0, 30.0))
    );
    assert_ne!(receiver.movement_events.session(), &receiver_session);
    assert!(receiver.movement_events.snapshot_pending().is_empty());
    assert!(receiver.movement_events.snapshot_completed().is_empty());
    let restored_session = receiver.movement_events.session().clone();
    let (object, mut writer) = receiver.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(Vec3::new(40.0, 0.0, 50.0), &mut writer);
    run_owned_host_boundary(None, &mut receiver);
    assert_eq!(receiver.movement_events.snapshot_completed().len(), 1);
    assert_eq!(
        receiver.movement_events.snapshot_completed()[0].session,
        restored_session
    );
    assert_eq!(owner.movement_events.snapshot_completed(), completed);
}

#[test]
fn restoring_a_snapshot_baselines_only_the_receiving_world_output() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "restoring_a_snapshot_baselines_only_the_receiving_world_output",
        restoring_a_snapshot_baselines_only_the_receiving_world_output_case,
    );
}
