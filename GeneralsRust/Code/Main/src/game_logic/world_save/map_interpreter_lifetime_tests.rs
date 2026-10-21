use super::*;

#[test]
fn failed_map_admission_keeps_receiving_interpreter_for_display_and_logic() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("Missing.map");
    let malformed = directory.path().join("Malformed.map");
    std::fs::write(&malformed, b"not a Chunky map").unwrap();
    let mut world = GameLogic::new();
    let mut peer = GameLogic::new();
    world
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_global_difficulty(gamelogic::player::GameDifficulty::Hard);
    world
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    peer.owned_script_engine
        .as_mut()
        .unwrap()
        .set_global_difficulty(gamelogic::player::GameDifficulty::Easy);
    peer.owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    for map in [&missing, &malformed] {
        assert!(!world.load_map(map.to_str().unwrap()));
        assert!(!world.map_loaded);
        assert!(world.owned_script_engine.is_some());
        assert_eq!(
            world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            gamelogic::player::GameDifficulty::Hard
        );
        assert!(world.script_debug_frozen(None).unwrap());
        assert!(world.simulation_frozen_with_script_engine(None).unwrap());
        assert!(peer.script_debug_frozen(None).unwrap());
        assert_eq!(
            peer.owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            gamelogic::player::GameDifficulty::Easy
        );
    }
    world.force_script_debug_continue(None).unwrap();
    world.game_mode = GameMode::Shell;
    let before = world.frame;
    world.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(
        world.frame,
        before.wrapping_add(1),
        "ordinary Menu/Shell logic must retain its real interpreter after failed map admission"
    );
    world
        .advance_receiving_camera_view(true, false, false, false, 33, None)
        .unwrap();
    assert!(!world.script_debug_frozen(None).unwrap());
    assert!(peer.script_debug_frozen(None).unwrap());
}

#[test]
fn map_admission_rejects_unreturned_interpreter_before_mutating_world() {
    let mut world = GameLogic::new();
    world.map_name = "Existing map".into();
    world.map_loaded = true;
    let engine = world.owned_script_engine.take().unwrap();
    let before = world.frame;
    assert!(!world.load_map("unavailable-owner.map"));
    assert_eq!(world.map_name, "Existing map");
    assert!(world.map_loaded);
    assert_eq!(world.frame, before);
    assert!(
        world.owned_script_engine.is_none(),
        "admission cannot fabricate a replacement for an outstanding loan"
    );
    world.install_owned_script_engine(engine);
    assert!(world.script_debug_frozen(None).is_ok());
}
