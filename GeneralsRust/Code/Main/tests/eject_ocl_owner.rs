use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};

fn eject_ocl(milliseconds: u32) -> String {
    format!(
        "ObjectCreationList OCL_EjectPilotOnGround\n\
         CreateObject\n ObjectNames = AmericaInfantryPilot\n Count = 1\n\
         IgnorePrimaryObstacle = Yes\n InheritsVeterancy = Yes\n\
         Disposition = RANDOM_FORCE\n MinForceMagnitude = 2\n MaxForceMagnitude = 3\n\
         MinForcePitch = 50\n MaxForcePitch = 60\n SpinRate = 0\n\
         InvulnerableTime = {milliseconds}ms\n RequiresLivePlayer = Yes\n End\nEnd\n"
    )
}

fn world(map: &std::path::Path, sound: &str) -> (GameLogic, ObjectId) {
    let source = format!(
        "Object SharedCarrier\n KindOf = VEHICLE SELECTABLE\n\
                  UnitSpecificSounds\n VoiceEject = {sound}Voice\n SoundEject = {sound}Sound\n End\n\
                  Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
                  Behavior = EjectPilotDie EjectTag\n GroundCreationList = OCL_EjectPilotOnGround\n End\nEnd\n\
                  Object AmericaInfantryPilot\n KindOf = INFANTRY SELECTABLE\n\
                  Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Object.ini".into(), source)])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(42, Team::USA, "Owner", true));
    let id = world
        .create_object_for_player("SharedCarrier", 42, glam::Vec3::new(50., 0., 50.))
        .unwrap();
    (world, id)
}

fn pilot_deadline(world: &GameLogic) -> Option<u32> {
    world
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "AmericaInfantryPilot")
        .map(|object| object.status.eject_invulnerable_until_frame)
}

#[test]
fn same_named_eject_lists_use_their_own_world_and_never_a_foreign_fallback() {
    // A foreign Core registry is a negative control for the removed fallback.
    let foreign = gamelogic::object_creation_list::store::parse_object_creation_lists_from_ini(
        &eject_ocl(5000),
    )
    .unwrap();
    for (name, list) in foreign {
        gamelogic::helpers::TheObjectCreationListStore::register_object_creation_list(&name, list);
    }
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let bytes = map.into_ckmp_bytes();
    let root = tempfile::tempdir().unwrap();
    let make_map = |name: &str, contents: Option<String>| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        if let Some(contents) = contents {
            std::fs::write(dir.join("Map.ini"), contents).unwrap();
        }
        path
    };
    let first_map = make_map("first", Some(eject_ocl(2000)));
    let peer_map = make_map("peer", Some(eject_ocl(4000)));
    let missing_map = make_map("missing", None);
    let (mut first, first_id) = world(&first_map, "FirstEject");
    let (mut peer, peer_id) = world(&peer_map, "PeerEject");
    let (mut missing, missing_id) = world(&missing_map, "MissingEject");
    assert_eq!(first_id, peer_id);
    assert_eq!(first_id, missing_id);

    first.apply_host_damage(first_id, 1000.0);
    let first_deadline = pilot_deadline(&first).expect("first world ejects its own pilot");
    let first_sounds = first.queued_audio_events.drain_requests();
    assert!(
        first_sounds
            .iter()
            .any(|event| event.event_type == "FirstEjectVoice")
    );
    assert!(
        first_sounds
            .iter()
            .any(|event| event.event_type == "FirstEjectSound")
    );
    assert_eq!(pilot_deadline(&peer), None);
    peer.apply_host_damage(peer_id, 1000.0);
    let peer_deadline = pilot_deadline(&peer).expect("peer ejects its own pilot");
    let peer_sounds = peer.queued_audio_events.drain_requests();
    assert!(
        peer_sounds
            .iter()
            .any(|event| event.event_type == "PeerEjectVoice")
    );
    assert!(
        peer_sounds
            .iter()
            .any(|event| event.event_type == "PeerEjectSound")
    );
    assert_eq!(peer_deadline - first_deadline, 60);
    missing.apply_host_damage(missing_id, 1000.0);
    assert_eq!(pilot_deadline(&missing), None);
    peer.reset();
    assert_eq!(pilot_deadline(&first), Some(first_deadline));
}
