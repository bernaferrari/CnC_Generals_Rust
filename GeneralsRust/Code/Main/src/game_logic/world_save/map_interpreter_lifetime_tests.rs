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

#[test]
fn interleaved_real_maps_and_companions_use_the_receiving_filesystem() {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    fn retail_map(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|root| root.join("windows_game/extracted_big_files/MapsZH/Maps").join(relative))
            .find(|path| path.is_file())
            .expect("extracted retail maps required for the two-root map regression")
    }
    fn owner(root: &Path) -> Arc<Mutex<crate::assets::AssetManager>> {
        let mut manager = crate::assets::AssetManager::new().unwrap();
        manager.archive_file_system_mut().set_local_search_roots([root.to_path_buf()]);
        Arc::new(Mutex::new(manager))
    }
    fn copy_map(root: &Path, source: &Path, cash: u32) {
        let directory = root.join("Maps/Shared");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::copy(source, directory.join("Shared.map")).unwrap();
        std::fs::write(directory.join("map.ini"), "GameData\n DefaultStartingCash = 111\nEnd\n").unwrap();
        std::fs::write(directory.join("solo.ini"), format!("GameData\n DefaultStartingCash = {cash}\nEnd\n")).unwrap();
    }
    let first_root = tempfile::tempdir().unwrap();
    let second_root = tempfile::tempdir().unwrap();
    let first_map = retail_map("Lone Eagle/Lone Eagle.map");
    let second_map = retail_map("ShellMapMD/ShellMapMD.map");
    copy_map(first_root.path(), &first_map, 1234);
    copy_map(second_root.path(), &second_map, 5678);
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    first.bind_application_assets(owner(first_root.path()));
    second.bind_application_assets(owner(second_root.path()));
    let name = "Maps/Shared/Shared.map";
    assert!(first.load_map(name));
    let first_bounds = first.world_bounds();
    assert_eq!(first.skirmish_rules.starting_cash_default, Some(1234));
    assert!(second.load_map(name));
    assert_eq!(second.skirmish_rules.starting_cash_default, Some(5678));
    assert_ne!(first_bounds, second.world_bounds(), "distinct retail geometry must not resolve to one owner");
    assert_eq!(first.world_bounds(), first_bounds);
    assert_eq!(first.skirmish_rules.starting_cash_default, Some(1234));
    assert!(first.load_map(name));
    assert_eq!(first.world_bounds(), first_bounds);
    assert_eq!(first.skirmish_rules.starting_cash_default, Some(1234));
    assert_eq!(second.skirmish_rules.starting_cash_default, Some(5678));
}
