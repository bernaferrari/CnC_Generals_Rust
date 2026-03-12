#[cfg(not(feature = "dev-tools"))]
fn main() {
    eprintln!("Enable the 'dev-tools' feature to build and run save_load_demo.");
}

#[cfg(feature = "dev-tools")]
use generals_main::ai::AIManager;
#[cfg(feature = "dev-tools")]
use generals_main::command_system::{CommandSystem, CommandType, GameCommand, ModifierKeys};
#[cfg(feature = "dev-tools")]
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Team};
#[cfg(feature = "dev-tools")]
use generals_main::save_load::{
    init_campaign_system, init_game_state_system, init_replay_system, init_save_load_system,
    list_available_saves, load_game, quick_save, record_replay_command, register_game_systems,
    save_game, try_auto_save, update_replay_system, CampaignId, GameDifficulty,
    MissionCompletionData, MissionDifficulty, ReplayPlayerInfo, ReplayTeamInfo, SaveFileType,
    GAME_STATE_MANAGER,
};
#[cfg(feature = "dev-tools")]
use glam::Vec3;
#[cfg(feature = "dev-tools")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "dev-tools")]
use std::time::{Duration, SystemTime};

#[cfg(feature = "dev-tools")]
fn build_demo_state() {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let _ = logic.load_map("demo_map");
    let _ = logic.create_object("USA_Ranger", Team::USA, Vec3::new(-20.0, 0.0, -10.0));
    let _ = logic.create_object("USA_Humvee", Team::USA, Vec3::new(-10.0, 0.0, -8.0));
    let _ = logic.create_object("GLA_Soldier", Team::GLA, Vec3::new(25.0, 0.0, 25.0));

    let game_logic = Arc::new(Mutex::new(logic));
    let command_system = Arc::new(Mutex::new(CommandSystem::new()));
    let ai_system = Arc::new(Mutex::new(AIManager::new()));

    register_game_systems(game_logic, command_system, ai_system, None);
}

#[cfg(feature = "dev-tools")]
fn demo_save_load() -> anyhow::Result<()> {
    save_game(
        "demo_slot",
        "Save/load API smoke test",
        SaveFileType::Normal,
    )?;

    quick_save()?;
    let _ = try_auto_save()?;
    let saves = list_available_saves()?;
    println!("Available saves: {}", saves.len());

    load_game("demo_slot")?;
    println!("Save/load round-trip succeeded");
    Ok(())
}

#[cfg(feature = "dev-tools")]
fn demo_replay() -> anyhow::Result<()> {
    let players = vec![
        ReplayPlayerInfo {
            player_id: 0,
            player_name: "Player 1".to_string(),
            team: Team::USA,
            is_human: true,
            is_observer: false,
            faction: "USA".to_string(),
            color: [0.2, 0.4, 0.8, 1.0],
            start_position: Vec3::new(-40.0, 0.0, -40.0),
        },
        ReplayPlayerInfo {
            player_id: 1,
            player_name: "AI Opponent".to_string(),
            team: Team::GLA,
            is_human: false,
            is_observer: false,
            faction: "GLA".to_string(),
            color: [0.8, 0.2, 0.2, 1.0],
            start_position: Vec3::new(40.0, 0.0, 40.0),
        },
    ];

    let teams = vec![
        ReplayTeamInfo {
            team_id: 0,
            team_name: "USA Team".to_string(),
            players: vec![0],
            allied_teams: vec![],
        },
        ReplayTeamInfo {
            team_id: 1,
            team_name: "GLA Team".to_string(),
            players: vec![1],
            allied_teams: vec![],
        },
    ];

    {
        let mut manager = GAME_STATE_MANAGER.lock().unwrap();
        manager.start_replay_recording(
            "demo_map",
            GameMode::Skirmish,
            GameDifficulty::Medium,
            &players,
            &teams,
        )?;
    }

    let command = GameCommand {
        command_type: CommandType::Move {
            destination: Vec3::new(10.0, 0.0, 10.0),
        },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(1)],
        modifier_keys: ModifierKeys::default(),
    };

    record_replay_command(&command)?;
    update_replay_system()?;

    {
        let mut manager = GAME_STATE_MANAGER.lock().unwrap();
        manager.stop_replay_recording()?;
    }

    println!("Replay record/update/stop cycle succeeded");
    Ok(())
}

#[cfg(feature = "dev-tools")]
fn demo_campaign() -> anyhow::Result<()> {
    {
        let mut manager = GAME_STATE_MANAGER.lock().unwrap();
        manager.start_campaign(CampaignId::USACampaign, "Demo Player")?;

        let completion = MissionCompletionData {
            play_duration: Duration::from_secs(900),
            score: 10_000,
            completed_primary: vec!["destroy_gla_base".to_string()],
            completed_secondary: vec![],
            completed_bonus: vec![],
            units_built: 20,
            units_lost: 2,
            enemies_destroyed: 15,
            resources_gathered: 6000,
            buildings_constructed: 8,
            special_powers_used: 1,
            perfect_completion: false,
            under_time_limit: true,
            no_losses: false,
            stealth_completion: false,
        };

        // Keep behavior non-fatal if mission id differs in local data.
        let _ = manager.complete_mission("usa_01", MissionDifficulty::Normal, completion);
    }

    println!("Campaign start/complete path exercised");
    Ok(())
}

#[cfg(feature = "dev-tools")]
fn main() -> anyhow::Result<()> {
    env_logger::init();

    init_save_load_system()?;
    init_replay_system()?;
    init_campaign_system()?;
    init_game_state_system()?;

    build_demo_state();
    demo_save_load()?;
    demo_replay()?;
    demo_campaign()?;

    println!("save_load_demo completed");
    Ok(())
}
