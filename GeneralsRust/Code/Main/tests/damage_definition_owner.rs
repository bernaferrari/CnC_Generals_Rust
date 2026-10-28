use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{
    GameLogic, GameMode, HostHealthEvents, Team, audio_dispatch_impl::UnitVoiceSlot,
};

fn map_with_damage_fx(root: &std::path::Path, name: &str, effect: &str) -> std::path::PathBuf {
    let directory = root.join(name);
    std::fs::create_dir(&directory).expect("create map directory");

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

    let map_path = directory.join("DamageOwner.map");
    std::fs::write(&map_path, map.into_ckmp_bytes()).expect("write map chunks");
    std::fs::write(
        directory.join("Map.ini"),
        format!(
            "Object SharedVictim\n ReplaceModule DamageTag\n Behavior = TransitionDamageFX MapDamageTag\n DamagedFXList1 = Loc X:0 Y:0 Z:0 FXList:{effect}\n ReallyDamagedFXList1 = Loc X:0 Y:0 Z:0 FXList:{effect}Really\n End\n End\nEnd\n"
        ),
    )
    .expect("write Map.ini damage override");
    map_path
}

fn admitted_world(sound: &str, map: &std::path::Path) -> GameLogic {
    let source = format!(
        "Object SharedVictim\n KindOf = VEHICLE\n SoundOnDamaged = {sound}\n SoundOnReallyDamaged = {sound}Really\n VoiceSelect = {sound}Voice\n VoiceMove = {sound}MoveBase\n\
         UnitSpecificSounds\n VoiceMove = {sound}MoveSpecific\n End\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
         Behavior = TransitionDamageFX DamageTag\n DamagedFXList1 = Loc X:0 Y:0 Z:0 FXList:BaseDamageFx\n ReallyDamagedFXList1 = Loc X:0 Y:0 Z:0 FXList:BaseDamageFxReally\n End\nEnd\n"
    );
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world
        .admit_object_ini_sources([("Object.ini".into(), source)])
        .unwrap();
    assert!(world.load_map(map.to_str().expect("map path is UTF-8")));
    world
}

fn world(sound: &str, map: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = admitted_world(sound, map);
    let id = world
        .create_object("SharedVictim", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}

#[test]
fn same_named_objects_keep_their_own_damage_transition_content() {
    let _ = env_logger::builder().is_test(true).try_init();
    let root = tempfile::tempdir().unwrap();
    let first_map = map_with_damage_fx(root.path(), "first", "FirstMapFx");
    let peer_map = map_with_damage_fx(root.path(), "peer", "PeerMapFx");
    let previous_services = gamelogic::system::engine_stores::active_services();
    let (mut first, first_id) = world("FirstDamage", &first_map);
    let (mut peer, peer_id) = world("PeerDamage", &peer_map);
    assert_eq!(first_id, peer_id);

    peer.queue_picked_unit_voice(&[peer_id], UnitVoiceSlot::Select);
    assert!(
        peer.queued_audio_events
            .drain_requests()
            .iter()
            .any(|event| event.event_type == "PeerDamageVoice")
    );
    peer.queue_picked_unit_voice(&[peer_id], UnitVoiceSlot::Move);
    assert!(
        peer.queued_audio_events
            .drain_requests()
            .iter()
            .any(|event| event.event_type == "PeerDamageMoveSpecific")
    );
    assert!(first.queued_audio_events.is_empty());

    let exercise = |world: &mut GameLogic, id, sound: &str, effect: &str| {
        let object = world.host_object_mut(id).unwrap();
        // Rebuild on demand to exercise the same lazy path used after restoration.
        object.transition_damage_fx = None;
        object.ensure_transition_damage_fx();
        let data = object.transition_damage_fx.as_ref().unwrap();
        assert_eq!(data.audio_for_state[1].as_deref(), Some(sound));
        assert_eq!(data.fx_for_state[1].as_deref(), Some(effect));
        object.take_damage(60.0, 1, &mut HostHealthEvents::default());
        assert!(object.health.current < object.health.maximum);
        assert!(
            object.pending_transition_damage_fx.iter().any(|event| {
                event.audio_name.as_deref() == Some(sound) && event.new_state == 1
            }),
            "damage transition events: {:?}, health: {:?}",
            object.pending_transition_damage_fx,
            object.health,
        );
    };
    exercise(&mut peer, peer_id, "PeerDamage", "PeerMapFx");
    peer.reset();
    assert!(std::sync::Arc::ptr_eq(
        &previous_services,
        &gamelogic::system::engine_stores::active_services(),
    ));
    first.queue_picked_unit_voice(&[first_id], UnitVoiceSlot::Select);
    assert!(
        first
            .queued_audio_events
            .drain_requests()
            .iter()
            .any(|event| event.event_type == "FirstDamageVoice")
    );
    exercise(&mut first, first_id, "FirstDamage", "FirstMapFx");
    drop(peer);
    let object = first.host_object_mut(first_id).unwrap();
    object.transition_damage_fx = None;
    object.ensure_transition_damage_fx();
    assert_eq!(
        object
            .transition_damage_fx
            .as_ref()
            .unwrap()
            .audio_for_state[1]
            .as_deref(),
        Some("FirstDamage")
    );

    // Restoration must rebind the object to this session's admitted content,
    // then continue from its saved health without selecting a peer catalog.
    let save_root = tempfile::tempdir().unwrap();
    let mut saves =
        generals_main::save_load::SaveFileManager::with_save_directory(save_root.path());
    saves.init().unwrap();
    saves.quick_save(&first).expect("encode the live world");
    let decoded = saves
        .load_game_snapshot("quicksave")
        .expect("decode the saved archive");
    let mut restored = admitted_world("FirstDamage", &first_map);
    saves
        .restore_game_snapshot(&decoded.snapshot, &mut restored)
        .expect("restore decoded snapshot into map-admitted owner");
    for owner in [&mut first, &mut restored] {
        let object = owner.host_object_mut(first_id).unwrap();
        assert_eq!(object.health.current, 40.0);
        object.transition_damage_fx = None;
        object.ensure_transition_damage_fx();
        let data = object.transition_damage_fx.as_ref().unwrap();
        assert_eq!(data.fx_for_state[2].as_deref(), Some("FirstMapFxReally"));
        object.take_damage(30.0, 2, &mut HostHealthEvents::default());
        assert_eq!(object.health.current, 10.0);
        assert!(object.pending_transition_damage_fx.iter().any(|event| {
            event.new_state == 2 && event.audio_name.as_deref() == Some("FirstDamageReally")
        }));
    }
}
