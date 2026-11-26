//! Move orders publish once in the issuing world's order and retain save continuity.
use crate::game_logic::game_logic::pose_owner_tests::isolated_at;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use crate::presentation_frame::{PresentationEvent, PresentationFrame};
use glam::Vec3;

fn admitted_world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    // C++ loads Object INI before NewGame; the unit is authored content.
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("MovementBoundaryUnit")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
        ],
    );
    logic.add_player(Player::new(0, Team::USA, "MovementOwner", true));
    let id = logic
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    (logic, id)
}

/// A world whose unit is admitted content carrying the AIUpdateInterface that
/// owns its movement machine: a saved game records that module's state
/// (AIUpdateInterface::xfer), so a hand-built unit with active control has no
/// native machine to capture.
fn admitted_ai_world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("MovementBoundaryUnit")
                .kind_of("INFANTRY")
                .health(100.0)
                .module("Behavior = AIUpdateInterface MovementAI\nEnd"),
        ],
    );
    logic.add_player(Player::new(0, Team::USA, "MovementOwner", true));
    let id = logic
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    (logic, id)
}

fn order_move(logic: &mut GameLogic, id: ObjectId, dest: Vec3) {
    let frame = logic.get_frame();
    let (object, mut writer) = logic.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(
        dest,
        frame,
        &mut writer,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
}

fn moves(frame: &PresentationFrame) -> Vec<(ObjectId, [f32; 3])> {
    frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::MoveOrdered {
                unit, destination, ..
            } => Some((*unit, *destination)),
            _ => None,
        })
        .collect()
}

fn move_receipts_include_orders_for_newly_admitted_actors_case() {
    let (mut logic, a) = admitted_world();
    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let b = logic
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    logic.movement_events.clear();
    for (id, x) in [(a, 20.0), (b, 30.0), (a, 40.0)] {
        order_move(&mut logic, id, Vec3::new(x, 0.0, 0.0));
    }
    assert!(before.objects.iter().all(|row| row.id != b));
    assert_eq!(
        before
            .objects
            .iter()
            .find(|row| row.id == a)
            .unwrap()
            .move_destination,
        None
    );
    assert_eq!(
        logic.host_authoritative_move_dest(a),
        Some([40.0, 0.0, 0.0])
    );
    assert_eq!(
        logic.host_authoritative_move_dest(b),
        Some([30.0, 0.0, 0.0])
    );
    assert!(logic.movement_events.has_pending(a));
    assert!(logic.movement_events.has_pending(b));

    logic.complete_presentation_observations();
    let receipts = logic.movement_events.snapshot_completed().to_vec();
    assert_eq!(receipts.len(), 3);
    assert!(
        receipts
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    logic.complete_presentation_observations();
    assert_eq!(logic.movement_events.snapshot_completed(), receipts);

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    for (id, x) in [(a, 40.0), (b, 30.0)] {
        let actor = frame.objects.iter().find(|row| row.id == id).unwrap();
        assert_eq!(actor.move_destination, Some(Vec3::new(x, 0.0, 0.0)));
    }
    assert_eq!(
        moves(&frame),
        [
            (a, [20.0, 0.0, 0.0]),
            (b, [30.0, 0.0, 0.0]),
            (a, [40.0, 0.0, 0.0])
        ]
    );
    assert!(logic.movement_events.take_completed().is_empty());
}

#[test]
fn move_receipts_include_orders_for_newly_admitted_actors() {
    isolated_at(
        module_path!(),
        "move_receipts_include_orders_for_newly_admitted_actors",
        move_receipts_include_orders_for_newly_admitted_actors_case,
    );
}

fn move_receipts_stay_ordered_across_boundaries_and_same_id_worlds_case() {
    let (mut owner, a) = admitted_world();
    let b = owner
        .create_object("MovementBoundaryUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let (mut peer, peer_a) = admitted_world();
    assert_eq!(a, peer_a);
    let owner_session = owner.movement_events.session().clone();
    let peer_session = peer.movement_events.session().clone();
    assert_ne!(owner_session, peer_session);
    owner.movement_events.clear();
    peer.movement_events.clear();

    order_move(&mut owner, a, Vec3::new(20.0, 0.0, 0.0));
    owner.complete_presentation_observations();
    // Path following updates the local destination without issuing a new order.
    owner.host_object_mut(a).unwrap().movement.target_position = Some(Vec3::new(99.0, 0.0, 0.0));
    order_move(&mut owner, b, Vec3::new(30.0, 0.0, 0.0));
    order_move(&mut peer, peer_a, Vec3::new(80.0, 0.0, 0.0));
    peer.complete_presentation_observations();
    peer.complete_presentation_observations();
    assert!(owner.movement_events.has_pending(b));
    owner.complete_presentation_observations();

    let receipts = owner.movement_events.snapshot_completed();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].unit, a);
    assert_eq!(receipts[1].unit, b);
    assert!(receipts.iter().all(|event| event.session == owner_session));
    let peer_receipts = peer.movement_events.snapshot_completed();
    assert_eq!(peer_receipts.len(), 1);
    assert_eq!(peer_receipts[0].session, peer_session);

    let frame = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    assert_eq!(
        moves(&frame),
        [(a, [20.0, 0.0, 0.0]), (b, [30.0, 0.0, 0.0])]
    );
    for (id, x) in [(a, 99.0), (b, 30.0)] {
        let actor = frame.objects.iter().find(|row| row.id == id).unwrap();
        assert_eq!(actor.move_destination, Some(Vec3::new(x, 0.0, 0.0)));
    }
    assert!(owner.movement_events.take_completed().is_empty());
    assert_eq!(peer.movement_events.snapshot_completed().len(), 1);
    let foreign = PresentationFrame::try_build_from_logic(&mut peer, 0).unwrap();
    assert_eq!(moves(&foreign), [(peer_a, [80.0, 0.0, 0.0])]);
    assert_eq!(
        foreign
            .objects
            .iter()
            .find(|row| row.id == peer_a)
            .unwrap()
            .move_destination,
        Some(Vec3::new(80.0, 0.0, 0.0))
    );
}

#[test]
fn move_receipts_stay_ordered_across_boundaries_and_same_id_worlds() {
    isolated_at(
        module_path!(),
        "move_receipts_stay_ordered_across_boundaries_and_same_id_worlds",
        move_receipts_stay_ordered_across_boundaries_and_same_id_worlds_case,
    );
}

fn restoring_a_snapshot_baselines_only_the_receiving_world_output_case() {
    let (mut owner, id) = admitted_ai_world();
    let (object, mut writer) = owner.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(
        Vec3::new(20.0, 0.0, 30.0),
        0,
        &mut writer,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    owner.complete_presentation_observations();
    let completed = owner.movement_events.snapshot_completed().to_vec();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut saved = builder.create_world_snapshot(&owner).unwrap();
    let (mut receiver, receiver_id) = admitted_ai_world();
    assert_eq!(receiver_id, id);
    let (object, mut writer) = receiver.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(
        Vec3::new(80.0, 0.0, 90.0),
        0,
        &mut writer,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
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
    let frame = receiver.get_frame();
    let (object, mut writer) = receiver.host_object_and_movement_events_mut(id).unwrap();
    object.set_destination(
        Vec3::new(40.0, 0.0, 50.0),
        frame,
        &mut writer,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    receiver.complete_presentation_observations();
    assert_eq!(receiver.movement_events.snapshot_completed().len(), 1);
    assert_eq!(
        receiver.movement_events.snapshot_completed()[0].session,
        restored_session
    );
    assert_eq!(owner.movement_events.snapshot_completed(), completed);
}

#[test]
fn restoring_a_snapshot_baselines_only_the_receiving_world_output() {
    isolated_at(
        module_path!(),
        "restoring_a_snapshot_baselines_only_the_receiving_world_output",
        restoring_a_snapshot_baselines_only_the_receiving_world_output_case,
    );
}
