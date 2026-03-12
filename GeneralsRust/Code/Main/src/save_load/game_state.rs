use crate::ai::AIManager;
use crate::command_system::{CommandSystem, GameCommand};
use crate::game_logic::*;
use crate::network::NetworkInterface;
use crate::save_load::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Central game state manager
pub struct GameStateManager {
    // Core components
    save_file_manager: Arc<Mutex<SaveFileManager>>,
    replay_manager: Arc<Mutex<ReplayManager>>,
    campaign_manager: Arc<Mutex<CampaignManager>>,

    // Game systems to snapshot
    game_logic: Option<Arc<Mutex<GameLogic>>>,
    command_system: Option<Arc<Mutex<CommandSystem>>>,
    ai_system: Option<Arc<Mutex<AIManager>>>,
    network_manager: Option<Arc<Mutex<NetworkInterface>>>,

    // Snapshot management
    post_process_snapshots: Vec<Box<dyn Snapshot + Send + Sync>>,

    // State tracking
    in_load_operation: bool,
    last_save_info: Option<SaveGameInfo>,
    auto_save_enabled: bool,
    last_auto_save_seconds: f32,
}

impl GameStateManager {
    pub fn new() -> Self {
        Self {
            save_file_manager: Arc::new(Mutex::new(SaveFileManager::new())),
            replay_manager: Arc::new(Mutex::new(ReplayManager::new())),
            campaign_manager: Arc::new(Mutex::new(CampaignManager::new())),

            game_logic: None,
            command_system: None,
            ai_system: None,
            network_manager: None,

            post_process_snapshots: Vec::new(),

            in_load_operation: false,
            last_save_info: None,
            auto_save_enabled: true,
            last_auto_save_seconds: 0.0,
        }
    }

    /// Initialize the game state manager
    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Initialize sub-managers
        {
            let mut save_manager = self.save_file_manager.lock().unwrap();
            save_manager.init()?;
        }

        {
            let mut replay_manager = self.replay_manager.lock().unwrap();
            replay_manager.init()?;
        }

        {
            let mut campaign_manager = self.campaign_manager.lock().unwrap();
            campaign_manager.init()?;
        }

        log::info!("Game state manager initialized");
        Ok(())
    }

    /// Register game systems for save/load operations
    pub fn register_systems(
        &mut self,
        game_logic: Arc<Mutex<GameLogic>>,
        command_system: Arc<Mutex<CommandSystem>>,
        ai_system: Arc<Mutex<AIManager>>,
        network_manager: Option<Arc<Mutex<NetworkInterface>>>,
    ) {
        self.game_logic = Some(game_logic);
        self.command_system = Some(command_system);
        self.ai_system = Some(ai_system);
        self.network_manager = network_manager;
    }

    /// Add a snapshot component for post-processing
    pub fn add_post_process_snapshot(&mut self, snapshot: Box<dyn Snapshot + Send + Sync>) {
        self.post_process_snapshots.push(snapshot);
    }

    /// Save game to named slot
    pub fn save_game(
        &mut self,
        slot_name: &str,
        description: &str,
        save_type: SaveFileType,
    ) -> SaveLoadResult<()> {
        let Some(game_logic) = &self.game_logic else {
            return Err(SaveLoadError::InvalidFormat);
        };

        let game_logic = game_logic.lock().unwrap();

        // Create save info
        let save_info = SaveGameInfo {
            filename: slot_name.to_string(),
            display_name: slot_name.to_string(),
            description: description.to_string(),
            map_name: game_logic.get_current_map_name().to_string(),
            campaign_side: self.get_campaign_side(),
            mission_number: self.get_mission_number(),
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs_f32(game_logic.get_total_play_time()),
            difficulty: match game_logic.get_difficulty() {
                crate::ai::AIDifficulty::Easy => GameDifficulty::Easy,
                crate::ai::AIDifficulty::Medium => GameDifficulty::Medium,
                crate::ai::AIDifficulty::Hard => GameDifficulty::Hard,
                crate::ai::AIDifficulty::Brutal => GameDifficulty::Hard, // Map Brutal to Hard
            },
            save_type,
        };

        // Perform save operation
        {
            let mut save_manager = self.save_file_manager.lock().unwrap();
            save_manager.save_game(slot_name, &game_logic, &save_info)?;
        }

        // Record in replay if recording
        if save_type != SaveFileType::AutoSave {
            let mut replay_manager = self.replay_manager.lock().unwrap();
            if replay_manager.is_recording() {
                // Record save event in replay for synchronization
                let _ = replay_manager.record_event_with_player(
                    ReplayEventType::DebugCommand,
                    0,
                    format!("save_game:{}", slot_name).as_bytes(),
                );
            }
        }

        self.last_save_info = Some(save_info);

        log::info!("Game saved to slot: {}", slot_name);
        Ok(())
    }

    /// Load game from named slot
    pub fn load_game(&mut self, slot_name: &str) -> SaveLoadResult<()> {
        self.in_load_operation = true;

        let result = self.perform_load_game(slot_name);

        self.in_load_operation = false;

        result
    }

    /// Quick save to slot 0
    pub fn quick_save(&mut self) -> SaveLoadResult<()> {
        self.save_game("quicksave", "Quick Save", SaveFileType::QuickSave)
    }

    /// Quick load from slot 0
    pub fn quick_load(&mut self) -> SaveLoadResult<()> {
        self.load_game("quicksave")
    }

    /// Auto save if conditions are met
    pub fn try_auto_save(&mut self) -> SaveLoadResult<bool> {
        if !self.auto_save_enabled {
            return Ok(false);
        }

        let Some(game_logic) = &self.game_logic else {
            return Ok(false);
        };

        let game_logic = game_logic.lock().unwrap();

        // Don't auto-save during certain conditions
        let current_play_time = game_logic.get_total_play_time();

        if self.in_load_operation
            || game_logic.is_paused()
            || game_logic.is_in_battle()
            || (current_play_time - self.last_auto_save_seconds) < 300.0
        {
            // 5 minute minimum interval
            return Ok(false);
        }

        drop(game_logic); // Release lock before save

        // Generate unique auto-save name
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let auto_save_name = format!("autosave_{}", timestamp);

        self.save_game(&auto_save_name, "Auto Save", SaveFileType::AutoSave)?;
        self.last_auto_save_seconds = current_play_time;

        // Clean up old auto-saves
        self.cleanup_auto_saves()?;

        Ok(true)
    }

    /// Delete a save file
    pub fn delete_save(&mut self, slot_name: &str) -> SaveLoadResult<()> {
        let save_manager = self.save_file_manager.lock().unwrap();
        save_manager.delete_save(slot_name)
    }

    /// Check if save exists
    pub fn save_exists(&self, slot_name: &str) -> bool {
        let save_manager = self.save_file_manager.lock().unwrap();
        save_manager.save_exists(slot_name)
    }

    /// Get save file information
    pub fn get_save_info(&self, slot_name: &str) -> SaveLoadResult<SaveGameInfo> {
        let save_manager = self.save_file_manager.lock().unwrap();
        save_manager.get_save_info(slot_name)
    }

    /// List all available saves
    pub fn list_saves(&self) -> SaveLoadResult<Vec<AvailableGameInfo>> {
        let save_manager = self.save_file_manager.lock().unwrap();
        save_manager.list_saves()
    }

    /// Start recording a replay
    pub fn start_replay_recording(
        &mut self,
        map_name: &str,
        game_mode: crate::game_logic::GameMode,
        difficulty: GameDifficulty,
        players: &[ReplayPlayerInfo],
        teams: &[ReplayTeamInfo],
    ) -> SaveLoadResult<()> {
        let mut replay_manager = self.replay_manager.lock().unwrap();
        let replay_game_mode = match game_mode {
            crate::game_logic::GameMode::Skirmish => crate::save_load::replay::GameMode::Skirmish,
            crate::game_logic::GameMode::SinglePlayer => {
                crate::save_load::replay::GameMode::Campaign
            }
            crate::game_logic::GameMode::Multiplayer => {
                crate::save_load::replay::GameMode::Multiplayer
            }
            _ => crate::save_load::replay::GameMode::Skirmish, // Default to Skirmish
        };
        replay_manager.start_recording(map_name, replay_game_mode, difficulty, players, teams)
    }

    /// Stop recording replay
    pub fn stop_replay_recording(&mut self) -> SaveLoadResult<()> {
        let mut replay_manager = self.replay_manager.lock().unwrap();
        replay_manager.stop_recording()
    }

    /// Start replay playback
    pub fn start_replay_playback(&mut self, filename: &str) -> SaveLoadResult<ReplayHeader> {
        let mut replay_manager = self.replay_manager.lock().unwrap();
        replay_manager.start_playback(filename)
    }

    /// Stop replay playback
    pub fn stop_replay_playback(&mut self) -> SaveLoadResult<()> {
        let mut replay_manager = self.replay_manager.lock().unwrap();
        replay_manager.stop_playback()
    }

    /// Update replay system
    pub fn update_replay(&mut self) -> SaveLoadResult<()> {
        let Some(command_system) = &self.command_system else {
            return Ok(());
        };
        let Some(game_logic) = &self.game_logic else {
            return Ok(());
        };

        let mut command_system = command_system.lock().unwrap();
        let mut replay_manager = self.replay_manager.lock().unwrap();
        let mut game_logic = game_logic.lock().unwrap();

        replay_manager.update(&mut command_system, &mut game_logic)
    }

    /// Record a command in replay
    pub fn record_command(&mut self, command: &GameCommand) -> SaveLoadResult<()> {
        let mut replay_manager = self.replay_manager.lock().unwrap();
        replay_manager.record_command(command)
    }

    /// Get campaign manager
    pub fn get_campaign_manager(&self) -> Arc<Mutex<CampaignManager>> {
        self.campaign_manager.clone()
    }

    /// Start new campaign
    pub fn start_campaign(
        &mut self,
        campaign_id: CampaignId,
        player_name: &str,
    ) -> SaveLoadResult<()> {
        let mut campaign_manager = self.campaign_manager.lock().unwrap();
        campaign_manager.start_campaign(campaign_id, player_name)
    }

    /// Complete current mission
    pub fn complete_mission(
        &mut self,
        mission_id: &str,
        difficulty: MissionDifficulty,
        completion_data: MissionCompletionData,
    ) -> SaveLoadResult<()> {
        let mut campaign_manager = self.campaign_manager.lock().unwrap();
        campaign_manager.complete_mission(mission_id, difficulty, completion_data)
    }

    /// Save mission state for campaign
    pub fn save_mission_state(&mut self, save_state: MissionSaveState) -> SaveLoadResult<()> {
        let mut campaign_manager = self.campaign_manager.lock().unwrap();
        campaign_manager.save_mission_state(save_state)
    }

    /// Load mission state for campaign
    pub fn load_mission_state(&mut self) -> SaveLoadResult<Option<MissionSaveState>> {
        let mut campaign_manager = self.campaign_manager.lock().unwrap();
        campaign_manager.load_mission_state()
    }

    /// Check if currently in load operation
    pub fn is_in_load_operation(&self) -> bool {
        self.in_load_operation
    }

    /// Set auto-save enabled state
    pub fn set_auto_save_enabled(&mut self, enabled: bool) {
        self.auto_save_enabled = enabled;
    }

    /// Get last save information
    pub fn get_last_save_info(&self) -> Option<&SaveGameInfo> {
        self.last_save_info.as_ref()
    }

    /// Create a complete CRC of game state for validation
    pub fn calculate_game_state_crc(&self) -> SaveLoadResult<u32> {
        let Some(game_logic) = &self.game_logic else {
            return Ok(0);
        };

        let game_logic = game_logic.lock().unwrap();

        // Create a snapshot and calculate CRC
        let snapshot_builder = SnapshotBuilder::new();
        let world_snapshot = snapshot_builder.create_world_snapshot(&game_logic)?;

        let serialized = bincode::serialize(&world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(crc32fast::hash(&serialized))
    }

    /// Validate save file integrity
    pub fn validate_save_file(&self, slot_name: &str) -> SaveLoadResult<bool> {
        let save_manager = self.save_file_manager.lock().unwrap();

        // Try to read save info - if this fails, file is corrupted
        match save_manager.get_save_info(slot_name) {
            Ok(_) => Ok(true),
            Err(SaveLoadError::Corrupted(_)) => Ok(false),
            Err(SaveLoadError::InvalidFormat) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Export all save data for backup
    pub fn export_all_saves(&self) -> SaveLoadResult<HashMap<String, Vec<u8>>> {
        let mut exported = HashMap::new();

        let saves = self.list_saves()?;
        let save_manager = self.save_file_manager.lock().unwrap();

        for save_info in saves {
            let save_path = save_manager.get_save_path(&save_info.filename);
            if save_path.exists() {
                let data = std::fs::read(&save_path)?;
                exported.insert(save_info.filename, data);
            }
        }

        Ok(exported)
    }

    /// Import save data from backup
    pub fn import_save_data(
        &mut self,
        save_data: HashMap<String, Vec<u8>>,
    ) -> SaveLoadResult<usize> {
        let save_manager = self.save_file_manager.lock().unwrap();
        let mut imported_count = 0;

        for (filename, data) in save_data {
            let save_path = save_manager.get_save_path(&filename);

            // Validate data before writing
            if data.len() >= 8 && &data[0..4] == b"GZHS" {
                std::fs::write(&save_path, data)?;
                imported_count += 1;
            }
        }

        Ok(imported_count)
    }

    // Private implementation methods

    fn perform_load_game(&mut self, slot_name: &str) -> SaveLoadResult<()> {
        // Stop any ongoing replay first
        if self.replay_manager.lock().unwrap().is_recording() {
            self.stop_replay_recording()?;
        }

        let Some(game_logic) = &self.game_logic else {
            return Err(SaveLoadError::InvalidFormat);
        };

        // Load from file
        let save_info = {
            let mut game_logic = game_logic.lock().unwrap();
            let mut save_manager = self.save_file_manager.lock().unwrap();
            save_manager.load_game(slot_name, &mut game_logic)?
        };

        // Run post-processing on all registered snapshots
        self.run_post_process_load()?;

        // Update AI system if needed
        if let Some(ai_system) = &self.ai_system {
            let mut ai = ai_system.lock().unwrap();
            ai.on_game_loaded();
        }

        // Update command system
        if let Some(command_system) = &self.command_system {
            let mut commands = command_system.lock().unwrap();
            commands.clear_queue();
        }

        self.last_save_info = Some(save_info);

        log::info!("Game loaded from slot: {}", slot_name);
        Ok(())
    }

    fn run_post_process_load(&mut self) -> SaveLoadResult<()> {
        for snapshot in &mut self.post_process_snapshots {
            snapshot.load_post_process()?;
        }
        Ok(())
    }

    fn get_campaign_side(&self) -> Option<String> {
        self.campaign_manager
            .try_lock()
            .ok()
            .and_then(|campaign_manager| {
                campaign_manager
                    .current_campaign_side_name()
                    .map(str::to_string)
            })
    }

    fn get_mission_number(&self) -> Option<u32> {
        self.campaign_manager
            .try_lock()
            .ok()
            .and_then(|campaign_manager| campaign_manager.current_mission_number())
    }

    fn cleanup_auto_saves(&mut self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        let mut auto_saves: Vec<_> = saves
            .into_iter()
            .filter(|s| s.save_info.save_type == SaveFileType::AutoSave)
            .collect();

        // Sort by date, newest first
        auto_saves.sort_by(|a, b| b.save_info.save_date.cmp(&a.save_info.save_date));

        // Keep only the 5 most recent auto-saves
        for old_save in auto_saves.iter().skip(5) {
            if let Err(e) = self.delete_save(&old_save.filename) {
                log::warn!(
                    "Failed to delete old auto-save {}: {}",
                    old_save.filename,
                    e
                );
            }
        }

        Ok(())
    }
}

impl Default for GameStateManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global game state manager instance
lazy_static::lazy_static! {
    pub static ref GAME_STATE_MANAGER: Mutex<GameStateManager> =
        Mutex::new(GameStateManager::new());
}

pub fn global_campaign_manager() -> Result<Arc<Mutex<CampaignManager>>, &'static str> {
    GAME_STATE_MANAGER
        .try_lock()
        .map(|manager| manager.get_campaign_manager())
        .map_err(|_| "Campaign manager unavailable")
}

/// Initialize the global game state system
pub fn init_game_state_system() -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.init()
}

/// Register game systems with the state manager
pub fn register_game_systems(
    game_logic: Arc<Mutex<GameLogic>>,
    command_system: Arc<Mutex<CommandSystem>>,
    ai_system: Arc<Mutex<AIManager>>,
    network_manager: Option<Arc<Mutex<NetworkInterface>>>,
) {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.register_systems(game_logic, command_system, ai_system, network_manager);
}

/// Convenience functions for common operations

pub fn save_game(
    slot_name: &str,
    description: &str,
    save_type: SaveFileType,
) -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.save_game(slot_name, description, save_type)
}

pub fn load_game(slot_name: &str) -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.load_game(slot_name)
}

pub fn quick_save() -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.quick_save()
}

pub fn quick_load() -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.quick_load()
}

pub fn try_auto_save() -> SaveLoadResult<bool> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.try_auto_save()
}

pub fn list_available_saves() -> SaveLoadResult<Vec<AvailableGameInfo>> {
    let manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.list_saves()
}

pub fn record_replay_command(command: &GameCommand) -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.record_command(command)
}

pub fn update_replay_system() -> SaveLoadResult<()> {
    let mut manager = GAME_STATE_MANAGER.lock().unwrap();
    manager.update_replay()
}
