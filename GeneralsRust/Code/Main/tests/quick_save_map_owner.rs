//! Quick and auto saves embed the receiving world's map rather than "Unknown".
use game_engine::common::{
    ini::INILoadType, rts::player_template::PlayerTemplateStore, system::DataChunkOutput,
};
use generals_main::{
    game_logic::{GameLogic, GameMode},
    save_load::SaveFileManager,
};

fn map_bytes(height: u8) -> Vec<u8> {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(height);
    }
    map.close_data_chunk();
    map.open_data_chunk("SidesList", 2);
    map.write_int(0);
    map.write_int(0);
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    map.into_ckmp_bytes()
}

#[test]
fn quick_and_auto_save_embed_the_receiving_map() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("Original.map");
    let bytes = map_bytes(8);
    std::fs::write(&original, &bytes).unwrap();
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions
        .apply_source(
            "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut world = GameLogic::new();
    world
        .admit_player_template_definitions(definitions)
        .unwrap();
    world.admit_object_ini_sources(std::iter::empty()).unwrap();
    world.start_new_game(GameMode::SinglePlayer).unwrap();
    assert!(world.load_map(original.to_str().unwrap()));

    let save_root = root.path().join("saves");
    let mut saves = SaveFileManager::with_save_directory(&save_root);
    saves.init().unwrap();
    saves.quick_save(&world).unwrap();
    saves.auto_save(&world).unwrap();
    let mut names = vec!["quicksave".to_string()];
    names.extend(std::fs::read_dir(&save_root).unwrap().filter_map(|entry| {
        let path = entry.ok()?.path();
        let name = path.file_stem()?.to_str()?;
        (name.starts_with("autosave_") && path.extension()?.to_str()? == "sav")
            .then(|| name.to_owned())
    }));
    assert_eq!(names.len(), 2);
    for name in names {
        let decoded = saves.load_game_snapshot(&name).unwrap();
        let extracted = std::path::Path::new(&decoded.save_info.map_name);
        assert_eq!(
            std::fs::read(extracted).unwrap(),
            bytes,
            "{name} must embed the admitted map"
        );
    }

    // A second instance may use an identically named map in another directory.
    // Its quick-save must embed that world's bytes rather than the first map.
    let second_dir = root.path().join("second");
    std::fs::create_dir(&second_dir).unwrap();
    let second_map = second_dir.join("Original.map");
    let second_bytes = map_bytes(12);
    std::fs::write(&second_map, &second_bytes).unwrap();
    let mut second = GameLogic::new();
    second.admit_object_ini_sources(std::iter::empty()).unwrap();
    second.start_new_game(GameMode::SinglePlayer).unwrap();
    assert!(second.load_map(second_map.to_str().unwrap()));
    saves.quick_save(&second).unwrap();
    let decoded = saves.load_game_snapshot("quicksave").unwrap();
    assert_eq!(
        std::fs::read(&decoded.save_info.map_name).unwrap(),
        second_bytes
    );
    assert_eq!(std::fs::read(&original).unwrap(), bytes);
}
