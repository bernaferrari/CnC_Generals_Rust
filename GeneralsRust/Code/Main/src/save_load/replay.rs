use crate::command_system::{CommandSystem, CommandType, GameCommand};
use crate::game_logic::*;
use crate::save_load::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Replay file magic number
const REPLAY_MAGIC: [u8; 4] = *b"GZRP";

/// Replay recording modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderMode {
    None,
    Recording,
    Playback,
}

/// Replay file header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub flags: u32,
    pub start_time: u64,
    pub end_time: u64,
    pub frame_duration: u32,
    pub total_frames: u64,
    pub checksum: u32,

    // Game information
    pub game_version: String,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub map_name: String,
    pub game_mode: GameMode,
    pub difficulty: GameDifficulty,

    // Player information
    pub players: Vec<ReplayPlayerInfo>,
    pub teams: Vec<ReplayTeamInfo>,
    pub game_options: String,

    // Replay metadata
    pub replay_name: String,
    pub description: String,
    pub quit_early: bool,
    pub desync_occurred: bool,
    pub disconnect_info: Vec<DisconnectInfo>,
}

impl Default for ReplayHeader {
    fn default() -> Self {
        Self {
            magic: REPLAY_MAGIC,
            version: SAVE_FILE_VERSION,
            flags: 0,
            start_time: 0,
            end_time: 0,
            frame_duration: 16, // 60 FPS = 16ms per frame
            total_frames: 0,
            checksum: 0,
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            exe_crc: 0,
            ini_crc: 0,
            map_name: String::new(),
            game_mode: GameMode::Skirmish,
            difficulty: GameDifficulty::Medium,
            players: Vec::new(),
            teams: Vec::new(),
            game_options: String::new(),
            replay_name: String::new(),
            description: String::new(),
            quit_early: false,
            desync_occurred: false,
            disconnect_info: Vec::new(),
        }
    }
}

/// Game modes for replay tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    Campaign,
    Skirmish,
    Multiplayer,
    Challenge,
}

/// Player information in replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlayerInfo {
    pub player_id: u32,
    pub player_name: String,
    pub team: Team,
    pub is_human: bool,
    pub is_observer: bool,
    pub faction: String,
    pub color: [f32; 4],
    pub start_position: glam::Vec3,
}

/// Team information in replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayTeamInfo {
    pub team_id: u32,
    pub team_name: String,
    pub players: Vec<u32>,
    pub allied_teams: Vec<u32>,
}

/// Player disconnect information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectInfo {
    pub player_id: u32,
    pub player_name: String,
    pub disconnect_frame: u64,
    pub reason: String,
}

/// Recorded game event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEvent {
    pub frame: u64,
    pub player_id: u32,
    pub event_type: ReplayEventType,
    pub data: Vec<u8>,
}

/// Types of events recorded in replays
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayEventType {
    // Player commands
    MoveCommand,
    AttackCommand,
    BuildCommand,
    SelectCommand,
    StopCommand,
    GuardCommand,
    PatrolCommand,

    // Special abilities
    SpecialPower,
    Upgrade,

    // System events
    GameStart,
    GameEnd,
    PlayerDisconnect,
    Pause,
    Resume,
    SpeedChange,

    // Network events
    Sync,
    Desync,
    CrcMismatch,

    // Radar/Beacon UI events
    RadarEvent,
    BeaconEvent,

    // Debug events
    DebugCommand,
    CheatCommand,
    SellCommand,
}

fn replay_event_type_for_command(command_type: &CommandType) -> ReplayEventType {
    match command_type {
        CommandType::Move { .. }
        | CommandType::MoveTo { .. }
        | CommandType::AttackMoveTo { .. }
        | CommandType::ForceMoveTo { .. } => ReplayEventType::MoveCommand,
        CommandType::Attack { .. }
        | CommandType::AttackObject { .. }
        | CommandType::ForceAttackObject { .. }
        | CommandType::ForceAttackGround { .. } => ReplayEventType::AttackCommand,
        CommandType::Build { .. }
        | CommandType::DozerConstruct { .. }
        | CommandType::DozerConstructLine { .. }
        | CommandType::DozerCancelConstruct { .. }
        | CommandType::ResumeConstruction { .. } => ReplayEventType::BuildCommand,
        CommandType::Stop => ReplayEventType::StopCommand,
        CommandType::Guard { .. } => ReplayEventType::GuardCommand,
        CommandType::CreateSelectedGroup { .. }
        | CommandType::DestroySelectedGroup { .. }
        | CommandType::RemoveFromSelectedGroup { .. } => ReplayEventType::SelectCommand,
        CommandType::PurchaseScience { .. }
        | CommandType::QueueUpgrade { .. }
        | CommandType::CancelUpgrade { .. } => ReplayEventType::Upgrade,
        CommandType::Sell { .. } => ReplayEventType::SellCommand,
        _ => ReplayEventType::MoveCommand,
    }
}

/// Replay recorder/player
pub struct ReplayManager {
    mode: RecorderMode,
    current_frame: u64,
    replay_directory: PathBuf,

    // Recording state
    recording_file: Option<BufWriter<File>>,
    recording_header: ReplayHeader,
    recorded_events: Vec<ReplayEvent>,
    last_sync_frame: u64,

    // Playback state
    playback_file: Option<BufReader<File>>,
    playback_header: ReplayHeader,
    event_queue: Vec<ReplayEvent>,
    next_event_index: usize,
    playback_speed: f32,

    // State validation
    frame_checksums: HashMap<u64, u32>,
    desync_detected: bool,
}

impl ReplayManager {
    pub fn new() -> Self {
        let replay_dir = SaveLoadManager::default_save_directory().join("Replays");

        Self {
            mode: RecorderMode::None,
            current_frame: 0,
            replay_directory: replay_dir,

            recording_file: None,
            recording_header: ReplayHeader::default(),
            recorded_events: Vec::new(),
            last_sync_frame: 0,

            playback_file: None,
            playback_header: ReplayHeader::default(),
            event_queue: Vec::new(),
            next_event_index: 0,
            playback_speed: 1.0,

            frame_checksums: HashMap::new(),
            desync_detected: false,
        }
    }

    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Create replay directory
        std::fs::create_dir_all(&self.replay_directory)?;

        // Clean up incomplete replay files
        self.cleanup_incomplete_replays()?;

        Ok(())
    }

    /// Start recording a new replay
    pub fn start_recording(
        &mut self,
        map_name: &str,
        game_mode: GameMode,
        difficulty: GameDifficulty,
        players: &[ReplayPlayerInfo],
        teams: &[ReplayTeamInfo],
    ) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::None {
            return Err(SaveLoadError::InvalidFormat);
        }

        // Generate unique filename
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("replay_{}.{}", timestamp, REPLAY_EXTENSION);
        let filepath = self.replay_directory.join(&filename);

        // Create recording file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath)?;
        let writer = BufWriter::new(file);

        // Initialize header
        let mut header = ReplayHeader::default();
        header.start_time = timestamp;
        header.map_name = map_name.to_string();
        header.game_mode = game_mode;
        header.difficulty = difficulty;
        header.players = players.to_vec();
        header.teams = teams.to_vec();
        header.replay_name = format!("Replay {}", timestamp);

        self.recording_file = Some(writer);
        self.recording_header = header;
        self.recorded_events.clear();
        self.current_frame = 0;
        self.mode = RecorderMode::Recording;

        // Record game start event
        self.record_event(ReplayEventType::GameStart, &[])?;

        log::info!("Started recording replay: {}", filepath.display());
        Ok(())
    }

    /// Stop recording and finalize replay file
    pub fn stop_recording(&mut self) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Recording {
            return Ok(());
        }

        // Record game end event
        self.record_event(ReplayEventType::GameEnd, &[])?;

        // Update header with final information
        self.recording_header.end_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.recording_header.total_frames = self.current_frame;

        // Write final header and events to file
        if let Some(mut writer) = self.recording_file.take() {
            self.write_replay_file(&mut writer)?;
        }

        self.mode = RecorderMode::None;
        log::info!("Stopped recording replay at frame {}", self.current_frame);
        Ok(())
    }

    /// Start playback of a replay file
    pub fn start_playback(&mut self, filename: &str) -> SaveLoadResult<ReplayHeader> {
        if self.mode != RecorderMode::None {
            return Err(SaveLoadError::InvalidFormat);
        }

        let filepath = self.get_replay_path(filename);
        if !filepath.exists() {
            return Err(SaveLoadError::FileNotFound(filename.to_string()));
        }

        // Open replay file
        let file = File::open(&filepath)?;
        let mut reader = BufReader::new(file);

        // Read and validate header
        let header = self.read_replay_header(&mut reader)?;
        if header.magic != REPLAY_MAGIC {
            return Err(SaveLoadError::InvalidFormat);
        }

        if header.version > SAVE_FILE_VERSION {
            return Err(SaveLoadError::VersionMismatch {
                expected: SAVE_FILE_VERSION,
                actual: header.version,
            });
        }

        // Read all events
        let events = self.read_replay_events(&mut reader)?;

        self.playback_file = Some(reader);
        self.playback_header = header.clone();
        self.event_queue = events;
        self.next_event_index = 0;
        self.current_frame = 0;
        self.playback_speed = 1.0;
        self.mode = RecorderMode::Playback;

        log::info!(
            "Started playback of replay: {} ({} frames, {} events)",
            filepath.display(),
            header.total_frames,
            self.event_queue.len()
        );

        Ok(header)
    }

    /// Stop replay playback
    pub fn stop_playback(&mut self) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Playback {
            return Ok(());
        }

        self.playback_file = None;
        self.event_queue.clear();
        self.next_event_index = 0;
        self.mode = RecorderMode::None;

        log::info!("Stopped replay playback");
        Ok(())
    }

    /// Update replay system (called every frame)
    /// NOTE: This works alongside the low-level recorder in GameEngine/Common
    /// This is the high-level interface, recorder.rs handles .rep file format
    pub fn update(
        &mut self,
        command_system: &mut CommandSystem,
        game_logic: &mut crate::game_logic::GameLogic,
    ) -> SaveLoadResult<()> {
        match self.mode {
            RecorderMode::Recording => {
                self.update_recording()?;
            }
            RecorderMode::Playback => {
                self.update_playback(command_system, game_logic)?;
            }
            RecorderMode::None => {}
        }

        self.current_frame += 1;
        Ok(())
    }

    /// Record a player command
    pub fn record_command(&mut self, command: &GameCommand) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Recording {
            return Ok(());
        }

        let event_type = replay_event_type_for_command(&command.command_type);

        // Serialize command data
        let data =
            bincode::serialize(command).map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        self.record_event_with_player(event_type, command.player_id, &data)?;

        Ok(())
    }

    /// Record a special power activation
    pub fn record_special_power(
        &mut self,
        player_id: u32,
        power_name: &str,
        target: glam::Vec3,
    ) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Recording {
            return Ok(());
        }

        #[derive(Serialize)]
        struct SpecialPowerData {
            power_name: String,
            target: glam::Vec3,
        }

        let power_data = SpecialPowerData {
            power_name: power_name.to_string(),
            target,
        };

        let data = bincode::serialize(&power_data)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        self.record_event_with_player(ReplayEventType::SpecialPower, player_id, &data)?;

        Ok(())
    }

    /// Record player disconnect
    pub fn record_player_disconnect(
        &mut self,
        player_id: u32,
        player_name: &str,
        reason: &str,
    ) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Recording {
            return Ok(());
        }

        let disconnect = DisconnectInfo {
            player_id,
            player_name: player_name.to_string(),
            disconnect_frame: self.current_frame,
            reason: reason.to_string(),
        };

        self.recording_header
            .disconnect_info
            .push(disconnect.clone());

        let data = bincode::serialize(&disconnect)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        self.record_event_with_player(ReplayEventType::PlayerDisconnect, player_id, &data)?;

        Ok(())
    }

    /// Record CRC mismatch (desync detection)
    pub fn record_crc_mismatch(
        &mut self,
        expected: u32,
        actual: u32,
        frame: u64,
    ) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Recording {
            return Ok(());
        }

        #[derive(Serialize)]
        struct CrcMismatchData {
            expected: u32,
            actual: u32,
            frame: u64,
        }

        let mismatch_data = CrcMismatchData {
            expected,
            actual,
            frame,
        };
        let data = bincode::serialize(&mismatch_data)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        self.recording_header.desync_occurred = true;
        self.desync_detected = true;

        self.record_event(ReplayEventType::CrcMismatch, &data)?;

        log::error!(
            "CRC mismatch recorded at frame {}: expected 0x{:08X}, got 0x{:08X}",
            frame,
            expected,
            actual
        );

        Ok(())
    }

    /// Set playback speed (1.0 = normal, 2.0 = 2x, etc.)
    pub fn set_playback_speed(&mut self, speed: f32) {
        self.playback_speed = speed.max(0.1).min(10.0);
        log::debug!("Playback speed set to {}x", self.playback_speed);
    }

    /// Get current playback position as percentage
    pub fn get_playback_progress(&self) -> f32 {
        if self.mode != RecorderMode::Playback || self.playback_header.total_frames == 0 {
            return 0.0;
        }

        self.current_frame as f32 / self.playback_header.total_frames as f32
    }

    /// Seek to specific frame in playback (limited support)
    pub fn seek_to_frame(&mut self, target_frame: u64) -> SaveLoadResult<()> {
        if self.mode != RecorderMode::Playback {
            return Err(SaveLoadError::InvalidFormat);
        }

        // Simple implementation - can only seek forward
        if target_frame < self.current_frame {
            return Err(SaveLoadError::InvalidFormat);
        }

        // Fast-forward to target frame
        while self.current_frame < target_frame && self.next_event_index < self.event_queue.len() {
            if self.event_queue[self.next_event_index].frame <= target_frame {
                self.next_event_index += 1;
            } else {
                break;
            }
            self.current_frame += 1;
        }

        Ok(())
    }

    /// Get replay information without starting playback
    pub fn get_replay_info(&self, filename: &str) -> SaveLoadResult<ReplayHeader> {
        let filepath = self.get_replay_path(filename);
        let file = File::open(&filepath)?;
        let mut reader = BufReader::new(file);

        self.read_replay_header(&mut reader)
    }

    /// List available replay files
    pub fn list_replays(&self) -> SaveLoadResult<Vec<String>> {
        let mut replays = Vec::new();

        if !self.replay_directory.exists() {
            return Ok(replays);
        }

        let entries = std::fs::read_dir(&self.replay_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == REPLAY_EXTENSION {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        replays.push(filename.to_string());
                    }
                }
            }
        }

        replays.sort();
        Ok(replays)
    }

    /// Get current mode
    pub fn get_mode(&self) -> RecorderMode {
        self.mode
    }

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.mode == RecorderMode::Recording
    }

    /// Check if playing back
    pub fn is_playing(&self) -> bool {
        self.mode == RecorderMode::Playback
    }

    // Private implementation methods

    fn record_event(&mut self, event_type: ReplayEventType, data: &[u8]) -> SaveLoadResult<()> {
        self.record_event_with_player(event_type, 0, data)
    }

    pub fn record_event_with_player(
        &mut self,
        event_type: ReplayEventType,
        player_id: u32,
        data: &[u8],
    ) -> SaveLoadResult<()> {
        let event = ReplayEvent {
            frame: self.current_frame,
            player_id,
            event_type,
            data: data.to_vec(),
        };

        self.recorded_events.push(event);

        // Periodic sync events for validation
        if self.current_frame - self.last_sync_frame >= 1800 {
            // Every 30 seconds at 60fps
            self.record_sync_event()?;
            self.last_sync_frame = self.current_frame;
        }

        Ok(())
    }

    fn record_sync_event(&mut self) -> SaveLoadResult<()> {
        // Generate frame checksum for validation
        let checksum = self.calculate_frame_checksum();
        self.frame_checksums.insert(self.current_frame, checksum);

        let data = checksum.to_le_bytes();
        self.record_event(ReplayEventType::Sync, &data)?;

        Ok(())
    }

    fn calculate_frame_checksum(&self) -> u32 {
        // In a real implementation, this would calculate a checksum of the entire game state
        // For now, just use the frame number
        crc32fast::hash(&self.current_frame.to_le_bytes())
    }

    fn update_recording(&mut self) -> SaveLoadResult<()> {
        // Recording update logic - mainly just frame tracking
        // Commands are recorded when they occur via record_command()
        Ok(())
    }

    fn update_playback(
        &mut self,
        command_system: &mut CommandSystem,
        game_logic: &mut crate::game_logic::GameLogic,
    ) -> SaveLoadResult<()> {
        // Process events for current frame
        while self.next_event_index < self.event_queue.len() {
            if self.event_queue[self.next_event_index].frame > self.current_frame {
                break; // Wait for this frame
            }

            let event = self.event_queue[self.next_event_index].clone();
            self.process_playback_event(&event, command_system, game_logic)?;
            self.next_event_index += 1;
        }

        Ok(())
    }

    fn process_playback_event(
        &mut self,
        event: &ReplayEvent,
        command_system: &mut CommandSystem,
        game_logic: &mut crate::game_logic::GameLogic,
    ) -> SaveLoadResult<()> {
        match event.event_type {
            ReplayEventType::MoveCommand
            | ReplayEventType::AttackCommand
            | ReplayEventType::BuildCommand
            | ReplayEventType::SellCommand
            | ReplayEventType::SelectCommand
            | ReplayEventType::Upgrade
            | ReplayEventType::StopCommand
            | ReplayEventType::GuardCommand
            | ReplayEventType::PatrolCommand => {
                // Deserialize and execute command
                let command: GameCommand = bincode::deserialize(&event.data)
                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

                let _result = command_system.execute_command(&command, game_logic);
            }

            ReplayEventType::SpecialPower => {
                // Handle special power activation
                log::debug!("Replaying special power at frame {}", event.frame);
            }

            ReplayEventType::RadarEvent | ReplayEventType::BeaconEvent => {
                #[derive(serde::Deserialize)]
                struct RadarReplayPayload {
                    text: String,
                    position: Option<[f32; 3]>,
                    kind: u8,
                }
                if let Ok(payload) = bincode::deserialize::<RadarReplayPayload>(&event.data) {
                    let pos = payload.position.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
                    let kind = match payload.kind {
                        1 => crate::game_logic::radar_notifications::RadarKind::Attack,
                        2 => crate::game_logic::radar_notifications::RadarKind::Ally,
                        _ => crate::game_logic::radar_notifications::RadarKind::Generic,
                    };
                    log::debug!(
                        "Replaying radar/beacon event {:?} @ {:?} (kind {:?})",
                        payload.text,
                        pos,
                        kind
                    );
                    let world_pos = pos.unwrap_or(glam::Vec3::ZERO);
                    match event.event_type {
                        ReplayEventType::BeaconEvent => {
                            game_logic.note_beacon_placed(world_pos);
                            game_logic.queue_radar_message_at(payload.text, world_pos, kind);
                        }
                        _ => {
                            game_logic.queue_radar_message_at(payload.text, world_pos, kind);
                        }
                    }
                }
            }

            ReplayEventType::PlayerDisconnect => {
                let disconnect: DisconnectInfo = bincode::deserialize(&event.data)
                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

                log::info!(
                    "Player {} disconnected at frame {}: {}",
                    disconnect.player_name,
                    disconnect.disconnect_frame,
                    disconnect.reason
                );
            }

            ReplayEventType::Sync => {
                // Validate sync checksum
                if event.data.len() >= 4 {
                    let recorded_checksum = u32::from_le_bytes([
                        event.data[0],
                        event.data[1],
                        event.data[2],
                        event.data[3],
                    ]);

                    let current_checksum = self.calculate_frame_checksum();

                    if recorded_checksum != current_checksum {
                        log::warn!(
                            "Sync mismatch at frame {}: recorded 0x{:08X}, current 0x{:08X}",
                            event.frame,
                            recorded_checksum,
                            current_checksum
                        );
                        self.desync_detected = true;
                    }
                }
            }

            ReplayEventType::GameStart => {
                log::info!("Replay game started at frame {}", event.frame);
            }

            ReplayEventType::GameEnd => {
                log::info!("Replay game ended at frame {}", event.frame);
            }

            _ => {
                log::debug!("Unhandled replay event type: {:?}", event.event_type);
            }
        }

        Ok(())
    }

    fn write_replay_file(&self, writer: &mut BufWriter<File>) -> SaveLoadResult<()> {
        // Write header
        let header_data = bincode::serialize(&self.recording_header)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        writer.write_all(&(header_data.len() as u32).to_le_bytes())?;
        writer.write_all(&header_data)?;

        // Write events
        writer.write_all(&(self.recorded_events.len() as u32).to_le_bytes())?;

        for event in &self.recorded_events {
            let event_data = bincode::serialize(event)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

            writer.write_all(&(event_data.len() as u32).to_le_bytes())?;
            writer.write_all(&event_data)?;
        }

        writer.flush()?;
        Ok(())
    }

    fn read_replay_header(&self, reader: &mut BufReader<File>) -> SaveLoadResult<ReplayHeader> {
        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        let header_size = u32::from_le_bytes(size_bytes) as usize;

        let mut header_data = vec![0u8; header_size];
        reader.read_exact(&mut header_data)?;

        let header: ReplayHeader = bincode::deserialize(&header_data)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(header)
    }

    fn read_replay_events(&self, reader: &mut BufReader<File>) -> SaveLoadResult<Vec<ReplayEvent>> {
        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        let event_count = u32::from_le_bytes(size_bytes) as usize;

        let mut events = Vec::with_capacity(event_count);

        for _ in 0..event_count {
            reader.read_exact(&mut size_bytes)?;
            let event_size = u32::from_le_bytes(size_bytes) as usize;

            let mut event_data = vec![0u8; event_size];
            reader.read_exact(&mut event_data)?;

            let event: ReplayEvent = bincode::deserialize(&event_data)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

            events.push(event);
        }

        Ok(events)
    }

    fn get_replay_path(&self, filename: &str) -> PathBuf {
        let mut path = self.replay_directory.clone();
        if filename.ends_with(&format!(".{}", REPLAY_EXTENSION)) {
            path.push(filename);
        } else {
            path.push(format!("{}.{}", filename, REPLAY_EXTENSION));
        }
        path
    }

    fn cleanup_incomplete_replays(&self) -> SaveLoadResult<()> {
        // Remove any .tmp files or corrupted replays
        if !self.replay_directory.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.replay_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == "tmp" {
                    log::debug!("Removing incomplete replay file: {}", path.display());
                    let _ = std::fs::remove_file(&path);
                }
            }
        }

        Ok(())
    }
}

impl Default for ReplayManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global replay manager instance
lazy_static::lazy_static! {
    pub static ref REPLAY_MANAGER: std::sync::Mutex<ReplayManager> =
        std::sync::Mutex::new(ReplayManager::new());
}

/// Initialize the global replay system
pub fn init_replay_system() -> SaveLoadResult<()> {
    let mut manager = REPLAY_MANAGER.lock().unwrap_or_else(|e| e.into_inner());
    manager.init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sell_command_maps_to_sell_event_type() {
        let event_type = replay_event_type_for_command(&CommandType::Sell {
            object_id: ObjectId(42),
        });
        assert_eq!(event_type, ReplayEventType::SellCommand);
    }

    #[test]
    fn build_command_mapping_is_unchanged() {
        let event_type = replay_event_type_for_command(&CommandType::Build {
            template_name: "USA_PowerPlant".to_string(),
            location: glam::Vec3::new(10.0, 0.0, 20.0),
        });
        assert_eq!(event_type, ReplayEventType::BuildCommand);
    }

    #[test]
    fn queue_upgrade_maps_to_upgrade_event_type() {
        let event_type = replay_event_type_for_command(&CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        });
        assert_eq!(event_type, ReplayEventType::Upgrade);
    }

    #[test]
    fn selection_commands_map_to_select_event_type() {
        let event_type = replay_event_type_for_command(&CommandType::CreateSelectedGroup {
            create_new: true,
            units: vec![ObjectId(7), ObjectId(8)],
        });
        assert_eq!(event_type, ReplayEventType::SelectCommand);
    }

    #[test]
    fn dozer_construct_maps_to_build_event_type() {
        let event_type = replay_event_type_for_command(&CommandType::DozerConstruct {
            template_name: "USA_Barracks".to_string(),
            location: glam::Vec3::new(0.0, 0.0, 0.0),
            orientation: 0.0,
        });
        assert_eq!(event_type, ReplayEventType::BuildCommand);
    }
}
