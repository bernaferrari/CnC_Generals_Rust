//! Long AI state belongs to its receiving world and survives a real Xfer round trip.
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::{
    AIUpdateModuleSnapshot, ModuleSnapshot, Snapshot, SnapshotBuilder, WorldSnapshot, Xfer,
    XferData, XferLoad, XferSave,
};
use glam::Vec3;
use std::collections::HashMap;
use std::io::Cursor;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor OwnerWalk\n Surfaces = GROUND\n Speed = 60\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world.admit_object_ini_sources([("AiOwner.ini".into(), "Object OwnerUnit\n KindOf = INFANTRY SELECTABLE\n Locomotor = SET_NORMAL OwnerWalk\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into())]).unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(7, Team::USA, "Owner", true));
    world
}

#[test]
fn live_ai_continuation_round_trips_without_touching_a_peer() {
    let mut source = world();
    let mut peer = world();
    let actor = source
        .create_object_for_player("OwnerUnit", 7, Vec3::ZERO)
        .unwrap();
    let peer_actor = peer
        .create_object_for_player("OwnerUnit", 7, Vec3::ZERO)
        .unwrap();
    assert_eq!(actor, peer_actor);
    assert!(source.unit_command_move_to(actor, Vec3::new(80., 0., 20.)));
    assert!(peer.unit_command_move_to(peer_actor, Vec3::new(20., 0., 80.)));
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let peer_before = builder.create_world_snapshot(&peer).unwrap();
    let modules = &snapshot.objects.get(&actor).unwrap().modules;
    let native = modules
        .values()
        .find_map(|module| match module {
            ModuleSnapshot::BaseAi(value) => Some(serde_json::to_string(value).unwrap()),
            _ => None,
        })
        .expect("live AI source has a native continuation");
    assert!(
        native.len() > 255,
        "this witness exercises the former one-byte limit"
    );
    let mut bytes = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();
    let mut restored = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(actor).unwrap().ai_state(),
        source.host_object(actor).unwrap().ai_state()
    );
    let restored_snapshot = builder.create_world_snapshot(&restored).unwrap();
    let restored_native = restored_snapshot.objects[&actor]
        .modules
        .values()
        .find_map(|module| match module {
            ModuleSnapshot::BaseAi(value) => Some(serde_json::to_string(value).unwrap()),
            _ => None,
        })
        .unwrap();
    assert_eq!(restored_native, native);
    let peer_after = builder.create_world_snapshot(&peer).unwrap();
    let peer_native = |snapshot: &WorldSnapshot| {
        snapshot.objects[&peer_actor]
            .modules
            .values()
            .find_map(|module| match module {
                ModuleSnapshot::BaseAi(value) => Some(serde_json::to_string(value).unwrap()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(peer_native(&peer_after), peer_native(&peer_before));

    // Continue the actual sessions, interleaving the unrelated peer on the
    // same thread. The restored actor must follow the same live transition.
    peer.update_with_dt(1.0 / 30.0);
    source.update_with_dt(1.0 / 30.0);
    restored.update_with_dt(1.0 / 30.0);
    let source_actor = source.host_object(actor).expect("source actor continues");
    let restored_actor = restored
        .host_object(actor)
        .expect("restored actor continues");
    assert_eq!(restored_actor.ai_state(), source_actor.ai_state());
    assert_eq!(restored_actor.get_position(), source_actor.get_position());
}

#[test]
fn state_projection_accepts_long_values_and_rejects_oversized_framing() {
    let value = "state payload ".repeat(100);
    let mut saved = ModuleSnapshot::AIUpdate(AIUpdateModuleSnapshot {
        current_state: "Guard".into(),
        state_machine_data: HashMap::from([("Guard".into(), value.clone())]),
        target_object: Some(ObjectId(19)),
        current_task: None,
        task_queue: vec![],
    });
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        saved.xfer(&mut writer).unwrap();
        writer.xfer_u32(&mut 0xA3B4_C5D6).unwrap();
    }
    let mut restored = saved.clone();
    let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
    restored.xfer(&mut reader).unwrap();
    let mut sentinel = 0;
    reader.xfer_u32(&mut sentinel).unwrap();
    assert_eq!(sentinel, 0xA3B4_C5D6);
    let ModuleSnapshot::AIUpdate(state) = restored else {
        panic!("AI state")
    };
    assert_eq!(state.state_machine_data["Guard"], value);

    let mut malicious = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut malicious);
        writer.xfer_u32(&mut 15).unwrap();
        writer.xfer_string(&mut "Guard".into()).unwrap();
        writer.xfer_u32(&mut 1).unwrap();
        writer.xfer_string(&mut "Guard".into()).unwrap();
        let mut oversized_length = u32::MAX;
        writer.xfer_u32(&mut oversized_length).unwrap();
    }
    let before = serde_json::to_string(&saved).unwrap();
    assert!(
        saved
            .xfer(&mut XferLoad::new(Cursor::new(malicious.into_inner())))
            .is_err()
    );
    assert_eq!(
        serde_json::to_string(&saved).unwrap(),
        before,
        "rejected input does not replace the receiving state"
    );
}
