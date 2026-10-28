use game_engine::common::system::DataChunkOutput;
use gamelogic::common::Relationship;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::system::detection_manager::get_detection_manager;
use gamelogic::system::stealth_manager::{StealthStatus, get_stealth_manager};
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn minimal_map(root: &std::path::Path) -> std::path::PathBuf {
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        output.write_int(value);
    }
    for _ in 0..1681 {
        output.write_byte(0);
    }
    output.close_data_chunk();
    output.open_data_chunk("ObjectsList", 3);
    output.close_data_chunk();
    let path = root.join("StealthVisibilityOwner.map");
    std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "StealthVisibilityOwner.ini".into(),
            "Object VisibilityTarget\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
             Object VisibilityDetector\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Local", true).with_script_identity("Local"));
    world.add_player(Player::new(2, Team::China, "China", false).with_script_identity("China"));
    world
}

fn reveal_all(world: &mut GameLogic, side: &str) {
    let mut action = ScriptAction::new(ScriptActionType::MapRevealAllPerm);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, side.to_owned()))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "StealthVisibilityOwner".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
}

struct GlobalStealthCleanup(u32);

impl Drop for GlobalStealthCleanup {
    fn drop(&mut self) {
        let _ = get_stealth_manager()
            .lock()
            .unwrap()
            .unregister_object(self.0);
        let _ = get_detection_manager()
            .lock()
            .unwrap()
            .unregister_object(self.0);
    }
}

#[test]
fn live_visibility_uses_receiving_object_status_not_core_singletons() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = minimal_map(owner_root.path());
    let peer_map = minimal_map(peer_root.path());
    let mut owner = world(&owner_map);
    let mut peer = world(&peer_map);

    let owner_target = owner
        .create_object_for_player("VisibilityTarget", 2, glam::Vec3::ZERO)
        .unwrap();
    let peer_target = peer
        .create_object_for_player("VisibilityTarget", 2, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(
        owner_target, peer_target,
        "sessions reuse the target ObjectId"
    );
    let id = owner_target.0;

    // Keep the local player active in the owner match without granting it a
    // detector. Victory processing otherwise eliminates its empty force.
    owner
        .create_object_for_player("VisibilityDetector", 1, glam::Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    // Give the peer a real receiving detector; Main's detector scan writes the
    // peer target's canonical Object status without a parallel registry.
    let detector = peer
        .create_object_for_player("VisibilityDetector", 1, glam::Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    peer.host_object_mut(detector)
        .unwrap()
        .set_detector_state(true, 500.0, 1);
    reveal_all(&mut owner, "<Local Player>");
    reveal_all(&mut peer, "<Local Player>");
    owner.update_with_dt(1.0 / 30.0);
    peer.update_with_dt(1.0 / 30.0);
    for world in [&mut owner, &mut peer] {
        world
            .host_object_mut(owner_target)
            .unwrap()
            .set_status_stealthed(true);
        world
            .host_object_mut(owner_target)
            .unwrap()
            .set_status_detected(false);
    }
    let frame = peer.get_frame();
    peer.host_object_mut(detector)
        .unwrap()
        .next_detection_scan_frame = frame;
    peer.update_stealth_and_detection();
    assert!(peer.host_object(peer_target).unwrap().status.detected);

    // Seed the legacy Core singleton with a conflicting same-ID invisible
    // entry. Before the migration, its process-global answer hid both worlds.
    {
        let mut detection = get_detection_manager().lock().unwrap();
        let _ = detection.unregister_object(id);
    }
    {
        let mut stealth = get_stealth_manager().lock().unwrap();
        let _ = stealth.unregister_object(id);
        stealth.register_object(id).unwrap();
        stealth
            .set_stealth_status(id, 1, StealthStatus::Invisible)
            .unwrap();
        stealth
            .set_stealth_strength(
                id,
                gamelogic::system::stealth_manager::StealthStrength::strong_stealth(),
            )
            .unwrap();
    }
    let _cleanup = GlobalStealthCleanup(id);

    assert!(
        owner.get_player(1).unwrap().is_alive,
        "viewer must still be active"
    );
    assert_ne!(owner.player_relationship(2, 1), Relationship::Allies);
    let owner_object = owner.host_object(owner_target).unwrap();
    assert!(owner_object.status.stealthed);
    assert!(!owner_object.status.detected);
    assert!(!owner_object.status.disguised);
    let owner_visible = owner.get_visible_objects(1);
    let peer_visible = peer.get_visible_objects(1);
    assert!(!owner_visible.contains(&owner_target));
    assert!(peer_visible.contains(&peer_target));
}

#[test]
fn same_faction_visibility_uses_exact_player_relationships() {
    let root = tempfile::tempdir().unwrap();
    let map = minimal_map(root.path());
    let mut world = world(&map);

    // Player 1 and the target owner are distinct players on the same faction.
    // Player 3 is an ally of the target owner; explicit map relationships take
    // precedence over faction equality for stealth appearance.
    world.add_player(Player::new(3, Team::USA, "Ally", false).with_script_identity("Ally"));
    world
        .get_player_mut(1)
        .unwrap()
        .set_map_relationship(2, Relationship::Enemies);
    world.get_player_mut(2).unwrap().team = Team::USA;
    world
        .get_player_mut(2)
        .unwrap()
        .set_map_relationship(1, Relationship::Enemies);
    world
        .get_player_mut(2)
        .unwrap()
        .set_map_relationship(3, Relationship::Allies);
    world
        .get_player_mut(3)
        .unwrap()
        .set_map_relationship(2, Relationship::Allies);

    let target = world
        .create_object_for_player("VisibilityTarget", 2, glam::Vec3::ZERO)
        .unwrap();
    reveal_all(&mut world, "<Local Player>");
    reveal_all(&mut world, "Ally");
    world.update_with_dt(1.0 / 30.0);
    let target_object = world.host_object_mut(target).unwrap();
    target_object.set_status_stealthed(true);
    target_object.set_status_detected(false);

    assert_eq!(world.get_player(1).unwrap().team, Team::USA);
    assert_eq!(world.get_player(2).unwrap().team, Team::USA);
    assert_ne!(
        world.get_player(1).unwrap().id,
        world.get_player(2).unwrap().id
    );
    assert_eq!(world.player_relationship(2, 1), Relationship::Enemies);
    assert_eq!(world.player_relationship(2, 3), Relationship::Allies);
    assert!(!world.get_visible_objects(1).contains(&target));
    assert!(world.get_visible_objects(3).contains(&target));
}
