//! Actual current named-chunk save routes.

use super::*;
use crate::game_logic::{
    HackerDisableChannelPhase, HackerDisableChannelState, KindOf, ObjectId, Player,
    SupplyTruckState, Team, ThingTemplate,
};
use crate::save_load::snapshot::CollectorRuntimeSnapshot;
use glam::Vec3;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_save_info() -> SaveGameInfo {
    SaveGameInfo {
        filename: "legacy_fixture".to_string(),
        display_name: "Legacy production fixture".to_string(),
        description: "v1 bincode production payload".to_string(),
        map_name: "LegacyMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH,
        game_version: "test".to_string(),
        play_time: std::time::Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn unique_fixture_directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "generalsrust-legacy-production-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn test_save_file_paths() {
    let manager = SaveFileManager::new();

    let save_path = manager.get_save_path("test_save");
    assert!(save_path.to_string_lossy().contains("test_save"));
    assert!(
        save_path
            .to_string_lossy()
            .ends_with(&format!(".{}", SAVE_EXTENSION))
    );

    let temp_path = manager.get_temp_path("test_temp");
    assert!(temp_path.to_string_lossy().contains("test_temp"));
    assert!(temp_path.to_string_lossy().ends_with(".tmp"));
}

#[test]
fn default_save_directory_is_user_data_save_like_popup() {
    let host = SaveLoadManager::default_save_directory();
    let popup = crate::subsystem_manager::resolve_save_directory();
    assert_eq!(
        host, popup,
        "host SaveFileManager and Popup TheGameState must share UserData/Save"
    );
    assert_eq!(host.file_name().and_then(|s| s.to_str()), Some("Save"));
    let manager = SaveFileManager::new();
    assert_eq!(manager.save_directory(), host.as_path());
}

#[test]
fn host_pause_save_writes_cpp_17_named_chunks_and_v2_header() {
    // C++ GameState::init (GameState.cpp:289-305) + xferSaveData
    // (GameState.cpp:1313-1381) writes 17 CHUNK_* tokens then SG_EOF.
    // Pre-fix host wrote only GameState/GameLogic/GhostObject with a
    // Rust-invented CommonGameState schema (version 1).
    let snapshot = WorldSnapshot::default();
    let save_info = fixture_save_info();
    let bytes = SaveFileManager::write_common_sav_chunks(&snapshot, &save_info)
        .expect("write 17-block sav");
    let text = String::from_utf8_lossy(&bytes);
    for name in SAVELOAD_BLOCK_NAMES {
        assert!(
            text.contains(name),
            "host writer must emit C++ block token {name}"
        );
    }
    assert!(text.contains(SAVE_FILE_EOF));

    let blocks = walk_named_chunks(&bytes).expect("walk host chunks");
    assert_eq!(blocks.len(), SAVELOAD_BLOCK_NAMES.len());
    assert_eq!(
        blocks
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        SAVELOAD_BLOCK_NAMES.to_vec()
    );

    let header = parse_cpp_game_state_header(&blocks[0].1).expect("C++ v2 header");
    assert_eq!(header.description, save_info.description);
    assert_eq!(header.map_name, "LegacyMap");
    assert_eq!(header.save_type, SaveFileType::Normal);

    let listed = SaveFileManager::read_named_chunk_save_info(&bytes).expect("list");
    assert_eq!(listed.description, save_info.description);
    assert_eq!(listed.map_name, "LegacyMap");
}

#[test]
fn host_pause_save_persists_terrain_scorches_and_particle_systems() {
    game_client::terrain::clear_terrain_scorches();
    assert!(game_client::terrain::add_terrain_scorch(
        [88.0, 16.0, 4.0],
        22.0,
        1
    ));

    let snapshot = WorldSnapshot::default();
    let save_info = fixture_save_info();
    let bytes = SaveFileManager::write_common_sav_chunks(&snapshot, &save_info)
        .expect("write sav with FX chunks");
    let blocks = walk_named_chunks(&bytes).expect("walk host chunks");

    let terrain = blocks
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(CHUNK_TERRAIN_VISUAL))
        .expect("CHUNK_TerrainVisual");
    assert_eq!(terrain.1.first().copied(), Some(3));
    assert!(
        terrain.1.len() > 1,
        "CHUNK_TerrainVisual must not be NullSnapshot v1"
    );

    let particles = blocks
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(CHUNK_PARTICLE_SYSTEM))
        .expect("CHUNK_ParticleSystem");
    assert!(
        !particles.1.is_empty(),
        "CHUNK_ParticleSystem must write manager xfer"
    );

    game_client::terrain::clear_terrain_scorches();
    assert!(game_client::terrain::terrain_scorch_marks().is_empty());
    restore_terrain_visual_from_xfer_bytes(&terrain.1).expect("restore scorches from chunk");
    let marks = game_client::terrain::terrain_scorch_marks();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].location, [88.0, 16.0, 4.0]);
    assert_eq!(marks[0].radius, 22.0);
    assert_eq!(marks[0].scorch_type, 1);
    game_client::terrain::clear_terrain_scorches();
}

#[test]
fn host_lists_cpp_game_state_version_2_without_rejecting() {
    // C++ GameState::xfer (GameState.cpp:1543-1559) writes version=2.
    // Pre-fix CommonGameState::xfer currentVersion=1 rejected it.
    let mut payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        let info = SaveGameInfo {
            filename: "retail".into(),
            display_name: "Retail Save".into(),
            description: "C++ listed".into(),
            map_name: "Maps\\Alpine Assault.map".into(),
            campaign_side: Some("America".into()),
            mission_number: Some(3),
            save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
            game_version: "1.04".into(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Mission,
        };
        write_cpp_game_state_header(&mut xfer, &info).expect("encode v2");
    }
    assert_eq!(payload.first().copied(), Some(2), "C++ currentVersion is 2");

    let mut bytes = Vec::new();
    bytes.push(CHUNK_GAME_STATE.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

    let info =
        SaveFileManager::read_named_chunk_save_info(&bytes).expect("version 2 header must list");
    assert_eq!(info.description, "C++ listed");
    assert_eq!(info.save_type, SaveFileType::Mission);
    assert_eq!(info.campaign_side.as_deref(), Some("America"));
    assert_eq!(info.mission_number, Some(3));
    assert_eq!(info.map_name, "Maps\\Alpine Assault.map");
    // C++ mission files have no world restore payload. Host lists them
    // and loadGame restarts the mission instead of decoding GameLogic.
    let (snapshot, listed) = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
        .expect("mission header-only is a thin restart file");
    assert_eq!(listed.save_type, SaveFileType::Mission);
    assert!(
        snapshot.objects.is_empty(),
        "mission save must not invent a mid-world snapshot"
    );
}

#[test]
fn game_state_header_writes_empty_campaign_and_invalid_mission() {
    // C++ GameState.cpp:1632-1638: no current campaign → empty side + -1.
    let mut payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        write_cpp_game_state_header(
            &mut xfer,
            &SaveGameInfo {
                filename: "skirmish".into(),
                display_name: "Skirmish".into(),
                description: "Skirmish".into(),
                map_name: "Maps\\Alpine Assault.map".into(),
                campaign_side: None,
                mission_number: None,
                save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
                game_version: "1.04".into(),
                play_time: std::time::Duration::from_secs(0),
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            },
        )
        .expect("encode empty campaign header");
    }

    let mut bytes = Vec::new();
    bytes.push(CHUNK_GAME_STATE.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

    let info = SaveFileManager::read_named_chunk_save_info(&bytes)
        .expect("empty campaign header must list");
    assert_eq!(info.campaign_side.as_deref(), None);
    assert_eq!(info.mission_number, None);
}

fn cpp_game_logic_xfer_with_objects() -> Vec<u8> {
    // Minimal C++ GameLogic::xfer (GameLogic.cpp:4666-4696): version 10,
    // frame, object TOC with one template, objectCount=1. Host bincode
    // cannot consume this as WorldSnapshot.
    let mut payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        let mut version = 10u8;
        xfer.xfer_version(&mut version, 10)
            .expect("C++ GameLogic version");
        let mut frame = 42u32;
        xfer.xfer_unsigned_int(&mut frame).expect("frame");
        let mut toc_version = 1u8;
        xfer.xfer_version(&mut toc_version, 1).expect("TOC version");
        let mut toc_count = 1u32;
        xfer.xfer_unsigned_int(&mut toc_count).expect("TOC count");
        write_ascii(&mut xfer, "AmericaRanger").expect("TOC name");
        let mut toc_id = 1u16;
        xfer.xfer_unsigned_short(&mut toc_id).expect("TOC id");
        let mut object_count = 1u32;
        xfer.xfer_unsigned_int(&mut object_count)
            .expect("objectCount");
        xfer.xfer_unsigned_short(&mut toc_id)
            .expect("object TOC id");
        xfer.begin_block().expect("object block");
        let mut dummy = [0u8, 1, 2, 3];
        // SAFETY: dummy is a stack array of exactly its own length;
        // test-only save round-trip.
        unsafe {
            xfer.xfer_user(dummy.as_mut_ptr(), dummy.len())
                .expect("object bytes");
        }
        xfer.end_block().expect("end object block");
    }
    payload
}

#[test]
fn cpp_chunk_game_logic_does_not_report_successful_empty_world() {
    // C++ GameState::xferSaveData (GameState.cpp:1313-1381) writes
    // CHUNK_GameLogic via GameLogic::xfer (GameLogic.cpp:4666). Pre-fix
    // host decoded that as WorldSnapshot::default() and load reported
    // success with objects stripped.
    let mut header = Vec::new();
    {
        let mut cursor = Cursor::new(&mut header);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        write_cpp_game_state_header(
            &mut xfer,
            &SaveGameInfo {
                filename: "retail".into(),
                display_name: "Retail Save".into(),
                description: "C++ listed".into(),
                map_name: "Maps\\Alpine Assault.map".into(),
                campaign_side: Some("America".into()),
                mission_number: Some(3),
                save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
                game_version: "1.04".into(),
                play_time: std::time::Duration::from_secs(0),
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Mission,
            },
        )
        .expect("encode v2 header");
    }
    let logic = cpp_game_logic_xfer_with_objects();
    assert_eq!(
        logic.first().copied(),
        Some(10),
        "C++ GameLogic currentVersion is 10"
    );

    let mut bytes = Vec::new();
    bytes.push(CHUNK_GAME_STATE.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
    bytes.extend_from_slice(&(header.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.push(CHUNK_GAME_LOGIC.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_LOGIC.as_bytes());
    bytes.extend_from_slice(&(logic.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&logic);
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

    let listed =
        SaveFileManager::read_named_chunk_save_info(&bytes).expect("C++ header must still list");
    assert_eq!(listed.description, "C++ listed");

    let err = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
        .expect_err("C++ GameLogic::xfer must not succeed as an empty host world");
    let err = err.to_string();
    assert!(
        err.contains("GameLogic::xfer") || err.contains("not a host WorldSnapshot"),
        "error must say C++ objects were not restored, got {err}"
    );

    let fixture_directory = unique_fixture_directory();
    std::fs::create_dir_all(&fixture_directory).expect("create fixture directory");
    let path = fixture_directory.join("retail_cpp.sav");
    std::fs::write(&path, &bytes).expect("write C++-shaped save");
    let mut manager = SaveFileManager::with_save_directory(&fixture_directory);
    let mut world = GameLogic::new();
    let load_err = manager
        .load_game("retail_cpp", &mut world)
        .expect_err("live load must refuse unrestored C++ CHUNK_GameLogic");
    assert!(
        world.host_objects().is_empty(),
        "fail-closed load must not populate a stripped world"
    );
    let load_err = load_err.to_string();
    assert!(
        load_err.contains("GameLogic::xfer") || load_err.contains("not a host WorldSnapshot"),
        "live load error must name the unrestored C++ stream, got {load_err}"
    );
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(fixture_directory);
}

#[test]
fn mission_save_writes_only_game_state_and_campaign_chunks() {
    let snapshot = WorldSnapshot::default();
    let mut save_info = fixture_save_info();
    save_info.save_type = SaveFileType::Mission;
    save_info.map_name = "Maps\\Alpine Assault.map".into();
    save_info.description = "MissionSave".into();
    let bytes =
        SaveFileManager::write_common_sav_chunks(&snapshot, &save_info).expect("write mission sav");
    let blocks = walk_named_chunks(&bytes).expect("walk mission chunks");
    let names: Vec<&str> = blocks.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec![CHUNK_GAME_STATE, CHUNK_CAMPAIGN]);
    let listed = SaveFileManager::read_named_chunk_save_info(&bytes).expect("list mission");
    assert_eq!(listed.save_type, SaveFileType::Mission);
    assert_eq!(listed.map_name, "Maps\\Alpine Assault.map");
    let (world, info) =
        SaveFileManager::read_common_sav_chunks(&bytes, Path::new("")).expect("read mission");
    assert_eq!(info.save_type, SaveFileType::Mission);
    assert!(world.objects.is_empty());
}

#[test]
fn campaign_block_writes_runtime_difficulty_and_challenge() {
    use std::sync::Arc;
    game_engine::System::register_campaign_manager_runtime_hooks(
        Some(Arc::new(|| game_engine::System::CampaignManagerXferState {
            campaign: "GLA".into(),
            mission: "GLA02".into(),
            rank_points: 0,
            difficulty: 2,
            is_challenge: true,
            challenge_info: Some(game_engine::System::ChallengeGameInfoXfer::default()),
            generals_template: 4,
        })),
        None,
    );
    let snapshot = WorldSnapshot::default();
    let mut save_info = fixture_save_info();
    save_info.save_type = SaveFileType::Mission;
    save_info.map_name = "Maps\\GLA02.map".into();
    let bytes =
        SaveFileManager::write_common_sav_chunks(&snapshot, &save_info).expect("write mission");
    let listed = SaveFileManager::read_named_chunk_save_info(&bytes).expect("list");
    assert_eq!(listed.difficulty, GameDifficulty::Hard);
    let blocks = walk_named_chunks(&bytes).expect("walk");
    let campaign = blocks
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(CHUNK_CAMPAIGN))
        .map(|(_, payload)| parse_campaign_block(payload).expect("parse campaign"));
    let state = campaign.expect("CHUNK_Campaign");
    assert_eq!(state.campaign, "GLA");
    assert_eq!(state.mission, "GLA02");
    assert_eq!(state.difficulty, 2);
    assert!(state.is_challenge);
    assert_eq!(state.generals_template, 4);
    // Listing must surface CHUNK_Campaign difficulty (C++ loadGame then
    // MSG_NEW_GAME uses TheCampaignManager->getGameDifficulty()).
    assert_eq!(campaign_difficulty(&state), GameDifficulty::Hard);
}

#[test]
fn load_prefers_embedded_scratch_over_installed_same_named_map() {
    // C++ GameStateMap::xfer always plays the extracted Save-dir copy.
    // Pre-fix live kept the header map when find_map_file hit retail.
    let root = unique_fixture_directory();
    let installed_dir = root.join("installed");
    let save_dir = root.join("Save");
    std::fs::create_dir_all(&installed_dir).expect("create installed dir");
    std::fs::create_dir_all(&save_dir).expect("create save dir");
    let leaf = "Hq6q2b5ScratchPrefer.map";
    let installed = installed_dir.join(leaf);
    std::fs::write(&installed, b"RETAIL-INSTALLED").expect("write installed map");

    let mut save_info = fixture_save_info();
    save_info.save_type = SaveFileType::Mission;
    save_info.map_name = installed.to_string_lossy().into_owned();
    assert!(
        crate::game_logic::script_loader::find_map_file(&save_info.map_name).is_some(),
        "fixture must make find_map_file hit the installed same-named map"
    );

    let mut header = Vec::new();
    {
        let mut cursor = Cursor::new(&mut header);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        write_cpp_game_state_header(&mut xfer, &save_info).expect("encode header");
    }

    let mut map_payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut map_payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        let mut version = 2u8;
        xfer.xfer_version(&mut version, 2).expect("map version");
        write_ascii(&mut xfer, &format!("Save\\{leaf}")).expect("save path");
        write_ascii(&mut xfer, &format!("Maps\\{leaf}")).expect("pristine path");
        let mut game_mode = 0i32;
        xfer.xfer_int(&mut game_mode).expect("game mode");
        xfer.begin_block().expect("begin embed");
        let mut map_bytes = b"SCRATCH-CUSTOM".to_vec();
        // SAFETY: map_bytes is an owned Vec; xfer_user reads exactly
        // its len for this embedded-map fixture.
        unsafe {
            xfer.xfer_user(map_bytes.as_mut_ptr(), map_bytes.len())
                .expect("embed scratch");
        }
        xfer.end_block().expect("end embed");
        let mut object_id = 1u32;
        let mut drawable_id = 1u32;
        xfer.xfer_unsigned_int(&mut object_id).expect("object id");
        xfer.xfer_unsigned_int(&mut drawable_id)
            .expect("drawable id");
    }

    let mut bytes = Vec::new();
    bytes.push(CHUNK_GAME_STATE.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
    bytes.extend_from_slice(&(header.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.push(CHUNK_GAME_STATE_MAP.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_STATE_MAP.as_bytes());
    bytes.extend_from_slice(&(map_payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&map_payload);
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

    let (_, listed) = SaveFileManager::read_common_sav_chunks(&bytes, &save_dir)
        .expect("mission + GameStateMap must load");
    let extracted = save_dir.join(leaf);
    assert_eq!(
        listed.map_name,
        extracted.to_string_lossy().into_owned(),
        "load must play the extracted Save-dir scratch, not the installed map"
    );
    assert_eq!(
        std::fs::read(&extracted).expect("read extracted scratch"),
        b"SCRATCH-CUSTOM"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn game_state_map_round_trips_live_game_mode() {
    set_pending_save_game_mode(Some(CPP_GAME_SKIRMISH));
    let mut payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        let info = SaveGameInfo {
            filename: "mode".into(),
            display_name: "Mode".into(),
            description: "mode".into(),
            map_name: String::new(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH,
            game_version: "test".into(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        write_game_state_map_block(&mut xfer, &info).expect("write map");
    }
    set_pending_save_game_mode(None);
    store_loaded_game_state_map_mode(None);
    let dest = unique_fixture_directory();
    let _ = extract_embedded_map(&payload, &dest);
    assert_eq!(
        take_loaded_game_state_map_mode(),
        Some(CPP_GAME_SKIRMISH),
        "CHUNK_GameStateMap v2 must persist TheGameLogic game mode"
    );
    let _ = std::fs::remove_dir_all(dest);
}

#[test]
fn failed_load_does_not_apply_chunk_campaign_to_live_match() {
    // C++ GameState::loadGame only keeps CHUNK_Campaign after the whole
    // xfer succeeds; failure calls clearGameData. Live decode must stash
    // campaign and leave the still-playable match's identity/rank/difficulty.
    use std::sync::Arc;

    let prior = capture_live_campaign_state();
    let live = game_engine::System::CampaignManagerXferState {
        campaign: "USA".into(),
        mission: "USA01".into(),
        rank_points: 11,
        difficulty: 0,
        is_challenge: false,
        challenge_info: None,
        generals_template: 0,
    };
    apply_campaign_manager_state(live.clone());

    game_engine::System::register_campaign_manager_runtime_hooks(
        Some(Arc::new(|| game_engine::System::CampaignManagerXferState {
            campaign: "GLA".into(),
            mission: "GLA02".into(),
            rank_points: 99,
            difficulty: 2,
            is_challenge: false,
            challenge_info: None,
            generals_template: 4,
        })),
        None,
    );

    let mut campaign_payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut campaign_payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        write_campaign_block(&mut xfer).expect("write campaign");
    }

    let mut header = Vec::new();
    {
        let mut cursor = Cursor::new(&mut header);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        write_cpp_game_state_header(
            &mut xfer,
            &SaveGameInfo {
                filename: "bad_campaign".into(),
                display_name: "Bad Campaign".into(),
                description: "failed load".into(),
                map_name: "Maps\\Alpine Assault.map".into(),
                campaign_side: Some("GLA".into()),
                mission_number: Some(2),
                save_date: UNIX_EPOCH,
                game_version: "test".into(),
                play_time: std::time::Duration::from_secs(0),
                difficulty: GameDifficulty::Hard,
                save_type: SaveFileType::Normal,
            },
        )
        .expect("encode header");
    }

    let logic = cpp_game_logic_xfer_with_objects();
    let mut bytes = Vec::new();
    bytes.push(CHUNK_GAME_STATE.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
    bytes.extend_from_slice(&(header.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.push(CHUNK_CAMPAIGN.len() as u8);
    bytes.extend_from_slice(CHUNK_CAMPAIGN.as_bytes());
    bytes.extend_from_slice(&(campaign_payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&campaign_payload);
    bytes.push(CHUNK_GAME_LOGIC.len() as u8);
    bytes.extend_from_slice(CHUNK_GAME_LOGIC.as_bytes());
    bytes.extend_from_slice(&(logic.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&logic);
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

    let decode_err = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
        .expect_err("C++ GameLogic must fail closed");
    let decode_err = decode_err.to_string();
    assert!(
        decode_err.contains("GameLogic::xfer") || decode_err.contains("not a host WorldSnapshot"),
        "unexpected decode error: {decode_err}"
    );
    let after_decode = capture_live_campaign_state();
    assert_eq!(after_decode.rank_points, 11);
    assert_eq!(after_decode.difficulty, 0);

    let fixture_directory = unique_fixture_directory();
    std::fs::create_dir_all(&fixture_directory).expect("create fixture directory");
    let path = fixture_directory.join("bad_campaign.sav");
    std::fs::write(&path, &bytes).expect("write failed-load save");
    let mut manager = SaveFileManager::with_save_directory(&fixture_directory);
    let mut world = GameLogic::new();
    manager
        .load_game("bad_campaign", &mut world)
        .expect_err("failed load must not succeed");
    let after_load = capture_live_campaign_state();
    assert_eq!(
        after_load.rank_points, 11,
        "failed load must not keep save rank"
    );
    assert_eq!(
        after_load.difficulty, 0,
        "failed load must not keep save difficulty"
    );

    let snapshot = WorldSnapshot::default();
    let mut save_info = fixture_save_info();
    save_info.save_type = SaveFileType::Mission;
    let mission_bytes =
        SaveFileManager::write_common_sav_chunks(&snapshot, &save_info).expect("write mission sav");
    let _ = SaveFileManager::read_common_sav_chunks(&mission_bytes, Path::new(""))
        .expect("mission decode");
    let after_stash = capture_live_campaign_state();
    assert_eq!(
        after_stash.rank_points, 11,
        "successful decode must stash CHUNK_Campaign, not apply it"
    );
    commit_stashed_campaign_state();
    let after_commit = capture_live_campaign_state();
    assert_eq!(after_commit.rank_points, 99);
    assert_eq!(after_commit.difficulty, 2);

    apply_campaign_manager_state(prior);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(fixture_directory);
}
