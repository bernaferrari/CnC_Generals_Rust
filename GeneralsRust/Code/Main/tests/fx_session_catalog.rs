//! FXList.cpp clears a redefined list; definitions belong to the receiving game.
use generals_main::game_logic::GameLogic;

#[test]
fn same_named_fx_lists_are_owned_and_redefinitions_are_atomic() {
    let source = |name: &str| format!("FXList Shared\n Sound\n Name = {name}\n End\nEnd\n");
    let mut left = GameLogic::new();
    let mut right = GameLogic::new();
    assert_eq!(left.admit_fx_source(&source("LeftSound")).unwrap(), 1);
    assert_eq!(right.admit_fx_source(&source("RightSound")).unwrap(), 1);
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftSound"]);
    assert!(left.fx_list_sound_names("shared").is_empty());
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
    assert_eq!(
        left.resolve_owned_audio_event_names("FX:Shared"),
        ["LeftSound"]
    );
    assert_eq!(
        right.resolve_owned_audio_event_names("FX:Shared"),
        ["RightSound"]
    );
    assert!(
        left.resolve_owned_audio_event_names("FX:Missing")
            .is_empty()
    );
    assert_eq!(
        left.resolve_owned_audio_event_names("OrdinarySound"),
        ["OrdinarySound"]
    );

    // A mixed map source must skip a nested Object before the FXList block.
    let mixed = format!(
        "Object Unrelated\n Body = ActiveBody Tag\n MaxHealth = 100\n End\nEnd\n{}",
        source("LeftReplacement")
    );
    assert_eq!(left.admit_fx_source(&mixed).unwrap(), 1);
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftReplacement"]);
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
    assert_eq!(
        left.resolve_owned_audio_event_names("FX:Shared"),
        ["LeftReplacement"]
    );
    assert_eq!(
        right.resolve_owned_audio_event_names("FX:Shared"),
        ["RightSound"]
    );
    let bad = format!(
        "{}FXList Broken\n NotAnFxNugget\n End\nEnd\n",
        source("Uncommitted")
    );
    assert!(left.admit_fx_source(&bad).is_err());
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftReplacement"]);
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
    left.reset();
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftReplacement"]);
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
}

#[test]
fn retail_fx_list_is_admitted_when_extracted_content_is_available() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../windows_game/extracted_big_files_v2/INI/FXList.ini");
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let mut world = GameLogic::new();
    assert!(world.admit_fx_source(&source).unwrap() > 0);
    assert_eq!(
        world.fx_list_sound_names("FX_CarOverlappedByCrusher"),
        ["CarMount"]
    );
}

#[test]
fn presentation_freezes_each_sessions_fx_sounds() {
    use generals_main::game_logic::combat_particles::CombatParticleKind;
    use generals_main::presentation_frame::PresentationFrame;
    let source =
        |sound: &str| format!("FXList WeaponFX_Shared\n Sound\n Name = {sound}\n End\nEnd\n");
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    first.admit_fx_source(&source("FirstSound")).unwrap();
    second.admit_fx_source(&source("SecondSound")).unwrap();
    for world in [&mut first, &mut second] {
        world.combat_particles_mut().spawn_named(
            CombatParticleKind::DeathExplosion,
            "WeaponFX_Shared",
            glam::Vec3::ZERO,
            0,
            None,
            None,
            &mut (),
        );
    }
    let first_frame = PresentationFrame::build_from_logic(&mut first, 0);
    let second_frame = PresentationFrame::build_from_logic(&mut second, 0);
    first.admit_fx_source(&source("ReplacementSound")).unwrap();
    let names = |frame: &PresentationFrame| {
        frame
            .collect_audio_events()
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&first_frame), ["FirstSound"]);
    assert_eq!(names(&second_frame), ["SecondSound"]);
    let bytes = bincode_legacy::serialize(&first_frame).unwrap();
    let restored: PresentationFrame = bincode_legacy::deserialize(&bytes).unwrap();
    assert_eq!(names(&restored), ["FirstSound"]);
}

#[test]
fn map_fx_overrides_and_failed_peer_load_are_session_owned() {
    use game_engine::common::system::DataChunkOutput;
    use generals_main::game_logic::GameMode;

    fn map(root: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
        let directory = root.join(name);
        std::fs::create_dir(&directory).unwrap();
        let mut chunks = DataChunkOutput::new();
        chunks.open_data_chunk("HeightMapData", 3);
        for value in [21, 21, 1, 441] {
            chunks.write_int(value);
        }
        for _ in 0..441 {
            chunks.write_byte(8);
        }
        chunks.close_data_chunk();
        chunks.open_data_chunk("ObjectsList", 3);
        chunks.close_data_chunk();
        let path = directory.join("FxOwner.map");
        std::fs::write(&path, chunks.into_ckmp_bytes()).unwrap();
        std::fs::write(directory.join("Map.ini"), source).unwrap();
        path
    }
    let definition = |sound: &str| format!("FXList Shared\n Sound\n Name = {sound}\n End\nEnd\n");
    let directory = tempfile::tempdir().unwrap();
    let first_map = map(directory.path(), "first", &definition("FirstMap"));
    let second_map = map(directory.path(), "second", &definition("SecondMap"));
    let bad_map = map(
        directory.path(),
        "bad",
        "FXList Shared\n UnknownNugget\n End\nEnd\n",
    );
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    for (world, sound) in [(&mut first, "FirstBase"), (&mut second, "SecondBase")] {
        world.start_new_game(GameMode::SinglePlayer);
        world.admit_fx_source(&definition(sound)).unwrap();
    }
    assert!(first.load_map(first_map.to_str().unwrap()));
    assert!(second.load_map(second_map.to_str().unwrap()));
    assert_eq!(first.fx_list_sound_names("Shared"), ["FirstMap"]);
    assert_eq!(second.fx_list_sound_names("Shared"), ["SecondMap"]);
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&first).unwrap();
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded = bincode_legacy::deserialize(&encoded).unwrap();
    let mut loaded = GameLogic::new();
    loaded.start_new_game(GameMode::SinglePlayer);
    loaded.admit_fx_source(&definition("FirstBase")).unwrap();
    assert!(loaded.load_map(first_map.to_str().unwrap()));
    builder
        .restore_from_snapshot(&decoded, &mut loaded)
        .unwrap();
    assert_eq!(loaded.fx_list_sound_names("Shared"), ["FirstMap"]);
    let mut invalid = builder.create_world_snapshot(&first).unwrap();
    invalid.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&invalid, &mut loaded)
            .is_err()
    );
    assert_eq!(loaded.fx_list_sound_names("Shared"), ["FirstMap"]);
    first.reset();
    assert_eq!(first.fx_list_sound_names("Shared"), ["FirstBase"]);
    assert_eq!(second.fx_list_sound_names("Shared"), ["SecondMap"]);
    assert!(first.load_map(first_map.to_str().unwrap()));
    assert!(!second.load_map(bad_map.to_str().unwrap()));
    assert_eq!(first.fx_list_sound_names("Shared"), ["FirstMap"]);
    assert_eq!(loaded.fx_list_sound_names("Shared"), ["FirstMap"]);
    second.reset();
    assert_eq!(second.fx_list_sound_names("Shared"), ["SecondBase"]);
}
