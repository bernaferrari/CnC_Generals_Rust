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

use generals_main::game_logic::*;
use generals_main::save_load::GameMode as ReplayGameMode;
use generals_main::save_load::*;
use std::collections::HashMap;
use std::io::Cursor;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
fn test_xfer_hashmap_round_trip() {
    let mut buffer = Vec::new();
    let mut saved: HashMap<String, u32> = HashMap::from([
        ("AmericaTankCrusader".to_string(), 1200),
        ("ChinaTankOverlord".to_string(), 2000),
        ("GLAVehicleScorpion".to_string(), 600),
    ]);

    {
        let mut xfer = XferSave::new(Cursor::new(&mut buffer));
        xfer.open("test_hashmap").unwrap();
        xfer.xfer_hashmap(&mut saved).unwrap();
        xfer.close().unwrap();
    }

    let mut loaded: HashMap<String, u32> = HashMap::new();
    {
        let mut xfer = XferLoad::new(Cursor::new(&buffer));
        xfer.open("test_hashmap").unwrap();
        xfer.xfer_hashmap(&mut loaded).unwrap();
        xfer.close().unwrap();
    }

    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded["AmericaTankCrusader"], 1200);
    assert_eq!(loaded["ChinaTankOverlord"], 2000);
    assert_eq!(loaded["GLAVehicleScorpion"], 600);
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
    let snapshot = ObjectSnapshot {
        id: ObjectId(123),
        template_name: "TestUnit".to_string(),
        team: Team::USA,
        player_id: 1,
        geometry: GeometryInfo::default(),
        status: ObjectStatusSnapshot::default(),
        health: Health::new(100.0),
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
    header.game_mode = ReplayGameMode::Multiplayer;
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

    assert_eq!(deserialized.magic, *b"GZRP");
    assert_eq!(deserialized.map_name, "Tournament Desert");
    assert_eq!(deserialized.game_mode, ReplayGameMode::Multiplayer);
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
    use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
    use generals_main::game_logic::GameMode;
    use glam::Vec3;

    fn command(
        command_id: u32,
        command_type: CommandType,
        selected_units: Vec<ObjectId>,
    ) -> GameCommand {
        GameCommand {
            command_type,
            player_id: 0,
            command_id,
            timestamp: UNIX_EPOCH + Duration::from_secs(command_id as u64),
            selected_units,
            modifier_keys: ModifierKeys::default(),
        }
    }

    fn template(
        name: &str,
        kind_of: &[KindOf],
        health: f32,
        supplies: u32,
        build_time: f32,
    ) -> ThingTemplate {
        let mut template = ThingTemplate::new(name);
        template.set_health(health);
        template.set_cost(supplies, 0);
        template.build_time = build_time;
        for kind in kind_of {
            template.add_kind_of(*kind);
        }
        template
    }

    fn install_fixture_templates(game_logic: &mut GameLogic) {
        let templates = [
            template(
                "SaveTestCommandCenter",
                &[KindOf::Structure, KindOf::Selectable, KindOf::CommandCenter],
                2000.0,
                2000,
                0.1,
            ),
            template(
                "SaveTestDozer",
                &[KindOf::Vehicle, KindOf::Worker, KindOf::Selectable],
                300.0,
                1000,
                0.1,
            ),
            template(
                "SaveTestBarracks",
                &[KindOf::Structure, KindOf::Selectable],
                1000.0,
                500,
                0.1,
            ),
            template(
                "SaveTestRanger",
                &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                120.0,
                100,
                0.05,
            ),
        ];

        for template in templates {
            game_logic.templates.insert(template.name.clone(), template);
        }
    }

    fn save_info(filename: &str, save_type: SaveFileType) -> SaveGameInfo {
        SaveGameInfo {
            filename: filename.to_string(),
            display_name: "Save Fixture".to_string(),
            description: "Deterministic save/load fixture".to_string(),
            map_name: "SaveFixtureMap".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: Duration::from_secs(42),
            difficulty: GameDifficulty::Medium,
            save_type,
        }
    }

    fn fixture_game() -> GameLogic {
        let mut game_logic = GameLogic::new();
        game_logic.start_new_game(GameMode::Skirmish);
        game_logic.clear_all_players();
        install_fixture_templates(&mut game_logic);
        game_logic.add_player(Player::new(0, Team::USA, "USA", true));
        game_logic.add_player(Player::new(1, Team::China, "China", false));
        game_logic
    }

    fn save_and_load(filename: &str, game_logic: &GameLogic) -> (GameLogic, SaveGameInfo) {
        let save_dir = TempDir::new().expect("save temp dir should be created");
        let mut manager = SaveFileManager::with_save_directory(save_dir.path());
        manager.init().expect("save manager should initialize");
        let info = save_info(filename, SaveFileType::Normal);
        manager
            .save_game(filename, game_logic, &info)
            .expect("fixture should save");

        let mut loaded = GameLogic::new();
        install_fixture_templates(&mut loaded);
        let loaded_info = manager
            .load_game(filename, &mut loaded)
            .expect("fixture should load");
        (loaded, loaded_info)
    }

    #[test]
    fn test_full_save_load_cycle() {
        let mut game_logic = fixture_game();
        let command_center = game_logic
            .create_object("SaveTestCommandCenter", Team::USA, Vec3::ZERO)
            .expect("command center should spawn");
        let enemy_command_center = game_logic
            .create_object(
                "SaveTestCommandCenter",
                Team::China,
                Vec3::new(80.0, 0.0, 0.0),
            )
            .expect("enemy command center should spawn");
        let ranger = game_logic
            .create_object("SaveTestRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("ranger should spawn");

        let ranger_object = game_logic
            .get_object_mut(ranger)
            .expect("ranger should exist");
        ranger_object.weapon = Some(Weapon {
            damage: 60.0,
            range: 200.0,
            reload_time: 0.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        });

        let (mut loaded, loaded_info) = save_and_load("full_cycle", &game_logic);
        assert_eq!(loaded_info.map_name, "SaveFixtureMap");
        assert!(loaded.get_object(command_center).is_some());
        assert!(loaded.get_object(ranger).is_some());
        assert!(loaded.get_object(enemy_command_center).is_some());

        loaded.queue_command(command(
            1,
            CommandType::AttackObject {
                target_id: enemy_command_center,
            },
            vec![ranger],
        ));
        loaded.update();
        assert!(
            loaded
                .get_object(enemy_command_center)
                .expect("enemy command center should survive one shot")
                .health
                .current
                < 2000.0
        );
    }

    #[test]
    fn test_save_with_active_units() {
        let mut game_logic = fixture_game();
        let dozer = game_logic
            .create_object("SaveTestDozer", Team::USA, Vec3::ZERO)
            .expect("dozer should spawn");

        game_logic.queue_command(command(
            2,
            CommandType::MoveTo {
                destination: Vec3::new(64.0, 0.0, 0.0),
                waypoints: vec![Vec3::new(32.0, 0.0, 0.0)],
            },
            vec![dozer],
        ));
        game_logic.process_commands();

        let (loaded, _) = save_and_load("active_units", &game_logic);
        let loaded_dozer = loaded.get_object(dozer).expect("moving dozer should load");
        assert_eq!(loaded_dozer.ai_state, AIState::Moving);
        assert!(loaded_dozer.status.moving);
        assert_eq!(
            loaded_dozer.movement.target_position,
            Some(Vec3::new(64.0, 0.0, 0.0))
        );
    }

    #[test]
    fn test_save_with_active_combat() {
        let mut game_logic = fixture_game();
        let attacker = game_logic
            .create_object("SaveTestRanger", Team::USA, Vec3::ZERO)
            .expect("attacker should spawn");
        let target = game_logic
            .create_object(
                "SaveTestCommandCenter",
                Team::China,
                Vec3::new(60.0, 0.0, 0.0),
            )
            .expect("target should spawn");
        {
            let attacker = game_logic
                .get_object_mut(attacker)
                .expect("attacker should exist");
            attacker.weapon = Some(Weapon {
                damage: 75.0,
                range: 200.0,
                reload_time: 0.0,
                projectile_speed: 0.0,
                ..Weapon::default()
            });
            attacker.attack_target(target);
        }

        let (mut loaded, _) = save_and_load("active_combat", &game_logic);
        let loaded_attacker = loaded.get_object(attacker).expect("attacker should load");
        assert_eq!(loaded_attacker.ai_state, AIState::Attacking);
        assert_eq!(loaded_attacker.target, Some(target));
        assert!(loaded_attacker.weapon.is_some());

        loaded.update();
        assert!(
            loaded
                .get_object(target)
                .expect("target should still exist")
                .health
                .current
                < 2000.0
        );
    }

    #[test]
    fn test_save_with_production_queue() {
        let mut game_logic = fixture_game();
        let barracks = game_logic
            .create_object("SaveTestBarracks", Team::USA, Vec3::ZERO)
            .expect("barracks should spawn");
        assert!(game_logic.enqueue_production(barracks, "SaveTestRanger".to_string()));
        {
            let building = game_logic
                .get_object_mut(barracks)
                .and_then(|object| object.building_data.as_mut())
                .expect("barracks should have building data");
            building.production_queue[0].progress = 0.02;
            building.rally_point = Some(Vec3::new(30.0, 0.0, 10.0));
        }

        let (loaded, _) = save_and_load("production_queue", &game_logic);
        let building = loaded
            .get_object(barracks)
            .and_then(|object| object.building_data.as_ref())
            .expect("loaded barracks should keep building data");
        assert_eq!(building.production_queue.len(), 1);
        assert_eq!(building.production_queue[0].template_name, "SaveTestRanger");
        assert_eq!(building.production_queue[0].cost.supplies, 100);
        assert!((building.production_queue[0].progress - 0.02).abs() < 0.001);
        assert_eq!(building.rally_point, Some(Vec3::new(30.0, 0.0, 10.0)));
    }

    #[test]
    fn test_autosave_functionality() {
        let mut game_logic = fixture_game();
        let command_center = game_logic
            .create_object("SaveTestCommandCenter", Team::USA, Vec3::ZERO)
            .expect("command center should spawn");
        let save_dir = TempDir::new().expect("save temp dir should be created");
        let mut manager = SaveFileManager::with_save_directory(save_dir.path());
        manager.init().expect("save manager should initialize");

        manager
            .auto_save(&game_logic)
            .expect("autosave should complete");
        let saves = manager.list_saves().expect("autosave should be listed");
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].save_info.save_type, SaveFileType::AutoSave);

        let mut loaded = GameLogic::new();
        install_fixture_templates(&mut loaded);
        let loaded_info = manager
            .load_game(&saves[0].filename, &mut loaded)
            .expect("autosave should load");
        assert_eq!(loaded_info.save_type, SaveFileType::AutoSave);
        assert!(loaded.get_object(command_center).is_some());
    }
}
