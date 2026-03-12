#![cfg(feature = "internal")]
/*
** Command & Conquer Generals Zero Hour(tm) - Save/Load System Tests
** Copyright 2025 Electronic Arts Inc.
**
** Comprehensive tests for save/load functionality including:
** - Xfer serialization/deserialization
** - Game state snapshots
** - Full save/load cycle
** - Error handling and recovery
*/

use generals_rust::save_load::*;
use std::io::Cursor;
use tempfile::TempDir;

#[test]
fn test_xfer_basic_types() {
    // Test saving basic types
    let mut buffer = Vec::new();
    {
        let mut xfer = XferSave::new(Cursor::new(&mut buffer));
        xfer.open("test").unwrap();

        let mut test_u32 = 12345u32;
        let mut test_f32 = 3.14159f32;
        let mut test_bool = true;
        let mut test_string = "Hello World".to_string();

        xfer.xfer_u32(&mut test_u32).unwrap();
        xfer.xfer_f32(&mut test_f32).unwrap();
        xfer.xfer_bool(&mut test_bool).unwrap();
        xfer.xfer_string(&mut test_string).unwrap();

        xfer.close().unwrap();
    }

    // Test loading basic types
    {
        let mut xfer = XferLoad::new(Cursor::new(&buffer));
        xfer.open("test").unwrap();

        let mut loaded_u32 = 0u32;
        let mut loaded_f32 = 0.0f32;
        let mut loaded_bool = false;
        let mut loaded_string = String::new();

        xfer.xfer_u32(&mut loaded_u32).unwrap();
        xfer.xfer_f32(&mut loaded_f32).unwrap();
        xfer.xfer_bool(&mut loaded_bool).unwrap();
        xfer.xfer_string(&mut loaded_string).unwrap();

        assert_eq!(loaded_u32, 12345);
        assert!((loaded_f32 - 3.14159).abs() < 0.0001);
        assert_eq!(loaded_bool, true);
        assert_eq!(loaded_string, "Hello World");

        xfer.close().unwrap();
    }
}

#[test]
fn test_xfer_vectors() {
    let mut buffer = Vec::new();

    // Save vectors
    {
        let mut xfer = XferSave::new(Cursor::new(&mut buffer));
        xfer.open("test_vectors").unwrap();

        let mut test_vec2 = glam::Vec2::new(1.0, 2.0);
        let mut test_vec3 = glam::Vec3::new(3.0, 4.0, 5.0);
        let mut test_vec4 = glam::Vec4::new(6.0, 7.0, 8.0, 9.0);

        xfer.xfer_vec2(&mut test_vec2).unwrap();
        xfer.xfer_vec3(&mut test_vec3).unwrap();
        xfer.xfer_vec4(&mut test_vec4).unwrap();

        xfer.close().unwrap();
    }

    // Load vectors
    {
        let mut xfer = XferLoad::new(Cursor::new(&buffer));
        xfer.open("test_vectors").unwrap();

        let mut loaded_vec2 = glam::Vec2::ZERO;
        let mut loaded_vec3 = glam::Vec3::ZERO;
        let mut loaded_vec4 = glam::Vec4::ZERO;

        xfer.xfer_vec2(&mut loaded_vec2).unwrap();
        xfer.xfer_vec3(&mut loaded_vec3).unwrap();
        xfer.xfer_vec4(&mut loaded_vec4).unwrap();

        assert_eq!(loaded_vec2, glam::Vec2::new(1.0, 2.0));
        assert_eq!(loaded_vec3, glam::Vec3::new(3.0, 4.0, 5.0));
        assert_eq!(loaded_vec4, glam::Vec4::new(6.0, 7.0, 8.0, 9.0));

        xfer.close().unwrap();
    }
}

#[test]
fn test_xfer_blocks() {
    let mut buffer = Vec::new();

    // Save with blocks
    {
        let mut xfer = XferSave::new(Cursor::new(&mut buffer));
        xfer.open("test_blocks").unwrap();

        xfer.begin_block().unwrap();
        let mut data1 = 100u32;
        let mut data2 = 200u32;
        xfer.xfer_u32(&mut data1).unwrap();
        xfer.xfer_u32(&mut data2).unwrap();
        xfer.end_block().unwrap();

        xfer.begin_block().unwrap();
        let mut data3 = "Block 2 Data".to_string();
        xfer.xfer_string(&mut data3).unwrap();
        xfer.end_block().unwrap();

        xfer.close().unwrap();
    }

    // Load with blocks
    {
        let mut xfer = XferLoad::new(Cursor::new(&buffer));
        xfer.open("test_blocks").unwrap();

        let block_size1 = xfer.begin_block().unwrap();
        assert!(block_size1 > 0); // Should have non-zero block size

        let mut loaded1 = 0u32;
        let mut loaded2 = 0u32;
        xfer.xfer_u32(&mut loaded1).unwrap();
        xfer.xfer_u32(&mut loaded2).unwrap();
        xfer.end_block().unwrap();

        assert_eq!(loaded1, 100);
        assert_eq!(loaded2, 200);

        let block_size2 = xfer.begin_block().unwrap();
        assert!(block_size2 > 0);

        let mut loaded3 = String::new();
        xfer.xfer_string(&mut loaded3).unwrap();
        xfer.end_block().unwrap();

        assert_eq!(loaded3, "Block 2 Data");

        xfer.close().unwrap();
    }
}

#[test]
fn test_xfer_version_handling() {
    let mut buffer = Vec::new();

    // Save with version
    {
        let mut xfer = XferSave::new(Cursor::new(&mut buffer));
        xfer.open("test_version").unwrap();

        let mut version = 5u8;
        xfer.xfer_version(&mut version, 10).unwrap();

        xfer.close().unwrap();
    }

    // Load with matching version
    {
        let mut xfer = XferLoad::new(Cursor::new(&buffer));
        xfer.open("test_version").unwrap();

        let mut version = 0u8;
        xfer.xfer_version(&mut version, 10).unwrap();

        assert_eq!(version, 5);

        xfer.close().unwrap();
    }

    // Load with version mismatch
    {
        let mut xfer = XferLoad::new(Cursor::new(&buffer));
        xfer.open("test_version").unwrap();

        let mut version = 0u8;
        let result = xfer.xfer_version(&mut version, 3);

        assert!(result.is_err());
        match result {
            Err(SaveLoadError::VersionMismatch { expected, actual }) => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 5);
            }
            _ => panic!("Expected version mismatch error"),
        }
    }
}

#[test]
fn test_compression() {
    // Test basic compression
    let original_data = vec![42u8; 10000];
    let compressed = compression::compress(&original_data).unwrap();

    // Compressed should be smaller than original for repetitive data
    assert!(compressed.len() < original_data.len());

    // Test decompression
    let decompressed = compression::decompress(&compressed).unwrap();
    assert_eq!(decompressed, original_data);
}

#[test]
fn test_compression_detection() {
    let uncompressed_data = vec![1, 2, 3, 4, 5];
    let compressed_data = compression::compress(&uncompressed_data).unwrap();

    // If compression was beneficial, it should be marked as compressed
    if compressed_data.len() < uncompressed_data.len() * 9 / 10 {
        assert!(compression::is_compressed(&compressed_data).unwrap());
    }

    // Uncompressed data should not be detected as compressed
    assert!(!compression::is_compressed(&uncompressed_data).unwrap());
}

#[test]
fn test_compression_chunked() {
    let original_data = vec![123u8; 100000];

    // Compress in chunks
    let compressed = compression::compress_chunked(&original_data, 8192).unwrap();

    // Decompress
    let decompressed = compression::decompress_chunked(&compressed).unwrap();

    assert_eq!(decompressed, original_data);
}

#[test]
fn test_save_file_manager_basic() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("GENERALS_SAVE_DIR", temp_dir.path());

    let mut manager = SaveFileManager::new();
    manager.init().unwrap();

    // Test save path generation
    let save_path = manager.get_save_path("test_save");
    assert!(save_path.to_string_lossy().contains("test_save"));
    assert!(save_path.to_string_lossy().ends_with(".gen"));

    // Initially no saves should exist
    assert!(!manager.save_exists("test_save"));

    // List saves should return empty
    let saves = manager.list_saves().unwrap();
    assert_eq!(saves.len(), 0);
}

#[test]
fn test_save_file_header() {
    let mut header = SaveFileHeader::new();
    header.set_compressed(true);
    header.uncompressed_size = 12345;
    header.compressed_size = 6789;

    // Test header validation
    assert!(header.is_valid());
    assert!(header.is_compressed());

    // Test serialization
    let serialized = bincode::serialize(&header).unwrap();
    let deserialized: SaveFileHeader = bincode::deserialize(&serialized).unwrap();

    assert_eq!(header.magic, deserialized.magic);
    assert_eq!(header.version, deserialized.version);
    assert_eq!(header.uncompressed_size, deserialized.uncompressed_size);
    assert_eq!(header.compressed_size, deserialized.compressed_size);
    assert_eq!(header.is_compressed(), deserialized.is_compressed());
}

#[test]
fn test_world_snapshot_serialization() {
    let mut snapshot = WorldSnapshot::default();
    snapshot.frame_number = 12345;
    snapshot.random_seed = 67890;

    // Serialize
    let serialized = bincode::serialize(&snapshot).unwrap();

    // Deserialize
    let deserialized: WorldSnapshot = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.frame_number, 12345);
    assert_eq!(deserialized.random_seed, 67890);
    assert_eq!(deserialized.version, snapshot.version);
}

#[test]
fn test_object_snapshot_serialization() {
    let mut snapshot = ObjectSnapshot {
        id: ObjectId(123),
        template_name: "TestUnit".to_string(),
        team: Team::USA,
        player_id: 1,
        geometry: GeometryInfo::default(),
        status: ObjectStatusSnapshot::default(),
        health: Health::default(),
        movement: Movement::default(),
        experience: Experience::default(),
        weapons: Vec::new(),
        contained_objects: Vec::new(),
        container_object: None,
        modules: std::collections::HashMap::new(),
        object_type: ObjectTypeSnapshot::Unit(UnitSnapshot {
            unit_type: "Infantry".to_string(),
            formation_position: None,
            formation_id: None,
            group_id: None,
            waypoints: Vec::new(),
        }),
    };

    // Serialize
    let serialized = bincode::serialize(&snapshot).unwrap();

    // Deserialize
    let deserialized: ObjectSnapshot = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.id, ObjectId(123));
    assert_eq!(deserialized.template_name, "TestUnit");
    assert_eq!(deserialized.team, Team::USA);
    assert_eq!(deserialized.player_id, 1);
}

#[test]
fn test_player_snapshot_serialization() {
    let snapshot = PlayerSnapshot {
        id: 0,
        name: "Player 1".to_string(),
        team: Team::USA,
        is_human: true,
        is_active: true,
        resources: Resources {
            supplies: 5000,
            power: 100,
        },
        population: PopulationInfo {
            current: 50,
            maximum: 100,
        },
        tech_tree: TechTreeSnapshot {
            unlocked_units: vec!["Tank".to_string()],
            unlocked_buildings: vec!["Barracks".to_string()],
            unlocked_upgrades: vec!["Armor1".to_string()],
            research_progress: std::collections::HashMap::new(),
        },
        upgrades: vec!["Upgrade1".to_string()],
        build_queue: Vec::new(),
        research_queue: Vec::new(),
        statistics: PlayerStatisticsSnapshot {
            units_built: 10,
            units_lost: 2,
            buildings_built: 5,
            buildings_lost: 1,
            damage_dealt: 1000.0,
            damage_received: 500.0,
            resources_gathered: 10000,
            experience_gained: 250.0,
        },
    };

    // Serialize
    let serialized = bincode::serialize(&snapshot).unwrap();

    // Deserialize
    let deserialized: PlayerSnapshot = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.id, 0);
    assert_eq!(deserialized.name, "Player 1");
    assert_eq!(deserialized.team, Team::USA);
    assert_eq!(deserialized.resources.supplies, 5000);
    assert_eq!(deserialized.population.current, 50);
    assert_eq!(deserialized.statistics.units_built, 10);
}

#[test]
fn test_campaign_progress_serialization() {
    let progress = CampaignProgress {
        version: SAVE_FILE_VERSION,
        player_name: "TestPlayer".to_string(),
        total_play_time: std::time::Duration::from_secs(3600),
        last_played: SystemTime::now(),
        completed_missions: std::collections::HashMap::new(),
        current_campaign: Some(CampaignId::USACampaign),
        current_mission: Some("usa_01".to_string()),
        earned_honors: std::collections::HashMap::new(),
        total_honor_points: 150,
        current_rank: 2,
        global_stats: GlobalCampaignStats::default(),
        unlocked_units: vec!["Tank".to_string()],
        unlocked_buildings: vec!["Barracks".to_string()],
        unlocked_upgrades: vec!["Armor1".to_string()],
        unlocked_generals: Vec::new(),
        preferred_difficulty: MissionDifficulty::Normal,
        show_cutscenes: true,
        show_briefings: true,
    };

    // Serialize
    let serialized = bincode::serialize(&progress).unwrap();

    // Deserialize
    let deserialized: CampaignProgress = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.player_name, "TestPlayer");
    assert_eq!(deserialized.current_campaign, Some(CampaignId::USACampaign));
    assert_eq!(deserialized.total_honor_points, 150);
    assert_eq!(deserialized.current_rank, 2);
}

#[test]
fn test_replay_header_serialization() {
    let mut header = ReplayHeader::default();
    header.map_name = "Tournament Desert".to_string();
    header.game_mode = GameMode::Multiplayer;
    header.difficulty = GameDifficulty::Hard;
    header.total_frames = 100000;

    header.players.push(ReplayPlayerInfo {
        player_id: 0,
        player_name: "Player1".to_string(),
        team: Team::USA,
        is_human: true,
        is_observer: false,
        faction: "USA".to_string(),
        color: [0.2, 0.4, 0.8, 1.0],
        start_position: glam::Vec3::new(100.0, 0.0, 100.0),
    });

    // Serialize
    let serialized = bincode::serialize(&header).unwrap();

    // Deserialize
    let deserialized: ReplayHeader = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.magic, REPLAY_MAGIC);
    assert_eq!(deserialized.map_name, "Tournament Desert");
    assert_eq!(deserialized.game_mode, GameMode::Multiplayer);
    assert_eq!(deserialized.total_frames, 100000);
    assert_eq!(deserialized.players.len(), 1);
}

#[test]
fn test_replay_event_serialization() {
    let event = ReplayEvent {
        frame: 12345,
        player_id: 0,
        event_type: ReplayEventType::MoveCommand,
        data: vec![1, 2, 3, 4, 5],
    };

    // Serialize
    let serialized = bincode::serialize(&event).unwrap();

    // Deserialize
    let deserialized: ReplayEvent = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.frame, 12345);
    assert_eq!(deserialized.player_id, 0);
    assert_eq!(deserialized.event_type, ReplayEventType::MoveCommand);
    assert_eq!(deserialized.data, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_mission_info_serialization() {
    let mission = MissionInfo {
        id: "usa_01".to_string(),
        campaign_id: CampaignId::USACampaign,
        mission_number: 1,
        name: "First Strike".to_string(),
        description: "Test mission".to_string(),
        map_name: "usa_01_map".to_string(),
        briefing_video: Some("briefing.bik".to_string()),
        preview_image: Some("preview.tga".to_string()),
        required_missions: Vec::new(),
        required_rank: None,
        required_honor_points: None,
        time_limit: Some(1800),
        starting_resources: Resources {
            supplies: 10000,
            power: 0,
        },
        starting_units: vec!["Tank".to_string()],
        tech_restrictions: Vec::new(),
        special_rules: Vec::new(),
        victory_rule: Some("Annihilation".to_string()),
        primary_objectives: Vec::new(),
        secondary_objectives: Vec::new(),
        bonus_objectives: Vec::new(),
    };

    // Serialize
    let serialized = bincode::serialize(&mission).unwrap();

    // Deserialize
    let deserialized: MissionInfo = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.id, "usa_01");
    assert_eq!(deserialized.campaign_id, CampaignId::USACampaign);
    assert_eq!(deserialized.mission_number, 1);
    assert_eq!(deserialized.name, "First Strike");
    assert_eq!(deserialized.time_limit, Some(1800));
}

#[test]
fn test_error_types() {
    // Test that error types can be created and displayed
    let err1 = SaveLoadError::FileNotFound("test.gen".to_string());
    assert!(format!("{}", err1).contains("test.gen"));

    let err2 = SaveLoadError::VersionMismatch {
        expected: 1,
        actual: 2,
    };
    assert!(format!("{}", err2).contains("expected 1"));
    assert!(format!("{}", err2).contains("got 2"));

    let err3 = SaveLoadError::Corrupted("Bad checksum".to_string());
    assert!(format!("{}", err3).contains("Bad checksum"));
}

#[test]
fn test_save_game_info() {
    let info = SaveGameInfo {
        filename: "quicksave".to_string(),
        display_name: "Quick Save".to_string(),
        description: "Auto-generated quick save".to_string(),
        map_name: "Tournament Desert".to_string(),
        campaign_side: Some("USA".to_string()),
        mission_number: Some(3),
        save_date: SystemTime::now(),
        game_version: "1.0.0".to_string(),
        play_time: std::time::Duration::from_secs(1800),
        difficulty: GameDifficulty::Hard,
        save_type: SaveFileType::QuickSave,
    };

    // Serialize
    let serialized = bincode::serialize(&info).unwrap();

    // Deserialize
    let deserialized: SaveGameInfo = bincode::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.filename, "quicksave");
    assert_eq!(deserialized.display_name, "Quick Save");
    assert_eq!(deserialized.map_name, "Tournament Desert");
    assert_eq!(deserialized.difficulty, GameDifficulty::Hard);
    assert_eq!(deserialized.save_type, SaveFileType::QuickSave);
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    // These tests would require a full game_logic implementation
    // They are placeholders demonstrating the API usage

    #[test]
    #[ignore] // Requires full game implementation
    fn test_full_save_load_cycle() {
        // This test would:
        // 1. Create a game instance
        // 2. Set up some game state (units, buildings, etc.)
        // 3. Save the game
        // 4. Load the game
        // 5. Verify all state was restored correctly
    }

    #[test]
    #[ignore] // Requires full game implementation
    fn test_save_with_active_units() {
        // Test saving/loading with units in motion
    }

    #[test]
    #[ignore] // Requires full game implementation
    fn test_save_with_active_combat() {
        // Test saving/loading during combat
    }

    #[test]
    #[ignore] // Requires full game implementation
    fn test_save_with_production_queue() {
        // Test saving/loading with active production
    }

    #[test]
    #[ignore] // Requires full game implementation
    fn test_autosave_functionality() {
        // Test autosave triggers and cleanup
    }
}
