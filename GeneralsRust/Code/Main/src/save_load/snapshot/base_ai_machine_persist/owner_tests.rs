use super::*;
use crate::game_logic::{AIState, GameLogic, ObjectId, Player, Team};
use crate::save_load::{XferLoad, XferSave};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("base-ai-native.ini".into(), "Object BaseAiNative\nKindOf = VEHICLE SELECTABLE\nLocomotor = SET_NORMAL AmericaVehicleDozerLocomotor\nBody = ActiveBody Body\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = AIUpdateInterface ActualFirstAI\nEnd\nEnd\n".into()),
    ]).unwrap()).unwrap();
    world.add_player(Player::new(1, Team::USA, "NativeOwner", true));
    let actor = world
        .create_object_for_player("BaseAiNative", 1, Vec3::ZERO)
        .unwrap();
    (world, actor)
}

fn owner_bytes(world: &GameLogic, actor: ObjectId) -> Vec<u8> {
    serde_json::to_vec(
        &world
            .host_object(actor)
            .unwrap()
            .capture_base_ai_continuation(),
    )
    .unwrap()
}

fn encoded(snapshot: &BaseAiObjectSnapshot) -> Vec<u8> {
    let mut snapshot = snapshot.clone();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    bytes.into_inner()
}

#[test]
fn actual_move_native_continuation_is_inert_and_does_not_touch_same_id_peer() {
    let (mut source, actor) = world();
    let (mut peer, peer_actor) = world();
    assert_eq!(actor, peer_actor);
    assert!(source.unit_command_move_to(actor, Vec3::new(80., 0., 20.)));
    assert!(peer.unit_command_move_to(peer_actor, Vec3::new(20., 0., 80.)));
    let source_before = owner_bytes(&source, actor);
    let peer_before = owner_bytes(&peer, peer_actor);
    assert_ne!(source_before, peer_before);
    let snapshot = BaseAiObjectSnapshot::capture(source.host_object(actor).unwrap()).unwrap();
    let bytes = encoded(&snapshot);
    let mut decoded = BaseAiObjectSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes)))
        .unwrap();
    let (mut receiving, receiving_actor) = world();
    assert_eq!(actor, receiving_actor);
    decoded
        .apply_after_admission(receiving.host_object_mut(receiving_actor).unwrap())
        .unwrap();
    assert_eq!(
        owner_bytes(&receiving, receiving_actor),
        source_before,
        "restore copies retained control without entry effects"
    );
    assert_eq!(owner_bytes(&source, actor), source_before);
    assert_eq!(owner_bytes(&peer, peer_actor), peer_before);
    assert_eq!(
        receiving.host_object(actor).unwrap().ai_state(),
        AIState::Moving
    );
}

#[test]
fn foreign_source_rejects_before_changing_admitted_control() {
    let (mut source, actor) = world();
    assert!(source.unit_command_guard_position(actor, Vec3::new(40., 0., 10.)));
    let mut snapshot = BaseAiObjectSnapshot::capture(source.host_object(actor).unwrap()).unwrap();
    snapshot.source.as_mut().unwrap().1 = "DifferentSourceTag".into();
    let (mut receiving, receiving_actor) = world();
    let before = owner_bytes(&receiving, receiving_actor);
    assert!(
        snapshot
            .validate_for_template(
                &receiving
                    .host_object(receiving_actor)
                    .unwrap()
                    .thing()
                    .template
            )
            .is_err()
    );
    assert!(
        snapshot
            .apply_after_admission(receiving.host_object_mut(receiving_actor).unwrap())
            .is_err()
    );
    assert_eq!(owner_bytes(&receiving, receiving_actor), before);
}

#[test]
fn actual_guard_world_native_round_trip_and_reset_keep_peer_owner_unchanged() {
    let (mut source, actor) = world();
    let (mut peer, peer_actor) = world();
    assert_eq!(actor, peer_actor);
    assert!(source.unit_command_guard_position(actor, Vec3::new(50., 0., 15.)));
    assert!(peer.unit_command_move_to(peer_actor, Vec3::new(15., 0., 50.)));
    for _ in 0..10 {
        source.update_with_dt(1. / 30.);
        if source
            .host_object(actor)
            .unwrap()
            .unit_ai_runtime
            .guard_phase()
            .is_some()
        {
            break;
        }
    }
    let guard = &source.host_object(actor).unwrap().unit_ai_runtime;
    let guard_phase = guard.guard_phase();
    let guard_deadline = guard.guard_scan_deadline();
    let guard_anchor = guard.guard_anchor();
    let guard_updated = guard.guard_updated_frame();
    assert!(
        guard_phase.is_some(),
        "actual Guard callback enters its nested machine"
    );
    let before = owner_bytes(&source, actor);
    let peer_before = owner_bytes(&peer, peer_actor);
    let factory = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = factory.create_world_snapshot(&source).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut receiving, receiving_actor) = world();
    assert_eq!(actor, receiving_actor);
    factory
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(
        owner_bytes(&receiving, receiving_actor),
        before,
        "nested control survives actual receiving admission and native tails"
    );
    assert_eq!(owner_bytes(&peer, peer_actor), peer_before);
    let restored_guard = &receiving
        .host_object(receiving_actor)
        .unwrap()
        .unit_ai_runtime;
    assert_eq!(restored_guard.guard_phase(), guard_phase);
    assert_eq!(restored_guard.guard_scan_deadline(), guard_deadline);
    assert_eq!(restored_guard.guard_anchor(), guard_anchor);
    assert_eq!(restored_guard.guard_updated_frame(), guard_updated);
    source.reset();
    assert!(source.host_object(actor).is_none());
    assert_eq!(
        owner_bytes(&peer, peer_actor),
        peer_before,
        "reset retires only the driving owner"
    );
    receiving.update_with_dt(1. / 30.);
    assert_eq!(owner_bytes(&peer, peer_actor), peer_before);
}

#[test]
fn actual_hunt_command_preserves_parent_and_scan_in_receiving_native_owner() {
    let (mut source, actor) = world();
    let (mut peer, peer_actor) = world();
    assert_eq!(actor, peer_actor);
    assert!(source.unit_command_patrol(actor)); // actual Main aiHunt producer
    assert!(source.host_object(actor).unwrap().is_hunting());
    source.update_with_dt(1. / 30.);
    let scan = source
        .host_object(actor)
        .unwrap()
        .unit_ai_runtime
        .hunt_scan_deadline();
    assert!(
        scan.is_some(),
        "actual Hunt entry installs its source scan clock"
    );
    let before = owner_bytes(&source, actor);
    let peer_before = owner_bytes(&peer, peer_actor);
    let factory = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = factory.create_world_snapshot(&source).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut receiving, receiving_actor) = world();
    factory
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(owner_bytes(&receiving, receiving_actor), before);
    assert!(receiving.host_object(receiving_actor).unwrap().is_hunting());
    assert_eq!(
        receiving
            .host_object(receiving_actor)
            .unwrap()
            .unit_ai_runtime
            .hunt_scan_deadline(),
        scan
    );
    assert_eq!(owner_bytes(&peer, peer_actor), peer_before);
    receiving.update_with_dt(1. / 30.);
    assert!(
        receiving.host_object(receiving_actor).unwrap().is_hunting(),
        "saved root survives a nested Idle/scan callback"
    );
    assert_eq!(owner_bytes(&peer, peer_actor), peer_before);
}

#[test]
fn invalid_retained_parent_is_rejected_during_detached_native_decode() {
    let (mut source, actor) = world();
    assert!(source.unit_command_move_to(actor, Vec3::new(60., 0., 12.)));
    let captured = BaseAiObjectSnapshot::capture(source.host_object(actor).unwrap()).unwrap();
    let (mut ordinal, mut tag, mut class) = captured.source.clone().unwrap();
    let mut invalid = serde_json::to_value(captured.continuation.as_ref().unwrap()).unwrap();
    invalid["parent_state"] = serde_json::to_value(AIState::SpecialAbility).unwrap();
    let mut body = serde_json::to_string(&invalid).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        writer.xfer_u8(&mut 1).unwrap();
        writer.xfer_bool(&mut true).unwrap();
        writer.xfer_u32(&mut ordinal).unwrap();
        writer.xfer_string(&mut tag).unwrap();
        writer.xfer_string(&mut class).unwrap();
        writer.xfer_string(&mut body).unwrap();
    }
    let before = owner_bytes(&source, actor);
    let mut decoded = BaseAiObjectSnapshot::default();
    assert!(
        decoded
            .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
            .is_err()
    );
    assert!(decoded.source.is_none());
    assert!(decoded.continuation.is_none());
    assert_eq!(owner_bytes(&source, actor), before);
}

#[test]
fn foreign_native_source_preflights_before_frame_or_object_replacement() {
    use crate::save_load::snapshot::ModuleSnapshot;
    let (mut source, actor) = world();
    source.set_current_frame(11);
    assert!(source.unit_command_move_to(actor, Vec3::new(70., 0., 9.)));
    let factory = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = factory.create_world_snapshot(&source).unwrap();
    let record = snapshot
        .objects
        .iter_mut()
        .find(|object| object.id == actor)
        .unwrap();
    let payload = record
        .modules
        .values_mut()
        .find_map(|module| {
            if let ModuleSnapshot::BaseAi(payload) = module {
                Some(payload)
            } else {
                None
            }
        })
        .expect("actual source native14 record");
    payload.source.as_mut().unwrap().1 = "ForeignNativeSource".into();
    let (mut receiving, receiving_actor) = world();
    assert_eq!(receiving_actor, actor);
    receiving.set_current_frame(77);
    let frame = receiving.get_frame();
    let before = owner_bytes(&receiving, receiving_actor);
    let birth = std::sync::Arc::clone(
        &receiving
            .host_object(receiving_actor)
            .unwrap()
            .visual_draw_identity_token,
    );
    assert!(
        factory
            .restore_from_snapshot(&snapshot, &mut receiving)
            .is_err()
    );
    assert_eq!(receiving.get_frame(), frame);
    assert_eq!(owner_bytes(&receiving, receiving_actor), before);
    assert!(std::sync::Arc::ptr_eq(
        &birth,
        &receiving
            .host_object(receiving_actor)
            .unwrap()
            .visual_draw_identity_token
    ));
}

#[test]
fn actual_source_without_ai_is_inert_and_unsupported_control_is_not_silently_omitted() {
    let mut receiving = GameLogic::new();
    receiving.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("no-ai-native.ini".into(), "Object NoAiNative\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into()),
    ]).unwrap()).unwrap();
    receiving.add_player(Player::new(1, Team::USA, "NoMachine", true));
    let actor = receiving
        .create_object_for_player("NoAiNative", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        receiving
            .host_object(actor)
            .unwrap()
            .base_ai_control_is_inert()
    );
    let none = BaseAiObjectSnapshot::capture(receiving.host_object(actor).unwrap()).unwrap();
    assert!(none.source.is_none());
    assert!(none.continuation.is_none());
    assert!(
        !receiving.unit_command_move_to(actor, Vec3::new(30., 0., 10.)),
        "no virtual AI interface may receive Move"
    );
    assert!(
        receiving
            .host_object(actor)
            .unwrap()
            .base_ai_control_is_inert()
    );
    // Explicit malformed decoder/test input, not a fabricated positive source.
    receiving
        .host_object_mut(actor)
        .unwrap()
        .restore_ai_state(AIState::Moving);
    assert!(BaseAiObjectSnapshot::capture(receiving.host_object(actor).unwrap()).is_err());
    assert!(none.validate_ai_state_projection(&AIState::Moving).is_err());
    assert!(
        none.apply_after_admission(receiving.host_object_mut(actor).unwrap())
            .is_err()
    );
}
