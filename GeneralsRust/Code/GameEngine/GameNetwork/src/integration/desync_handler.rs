//! Desynchronization Detection and Handling
//!
//! This module implements desynchronization detection and response mechanisms.
//! When game states diverge between clients, this system detects it and initiates
//! appropriate recovery or disconnect procedures.

use super::game_state::{CRCValue, FrameNumber, GameState, PlayerId};
use crate::error::{NetworkError, NetworkResult};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Desync detection result
#[derive(Debug, Clone, PartialEq)]
pub enum DesyncStatus {
    /// All CRCs match - no desync
    Synchronized,
    /// CRC mismatch detected
    Desynchronized {
        frame: FrameNumber,
        desynced_players: Vec<PlayerId>,
    },
}

/// Desync handling strategy
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DesyncStrategy {
    /// Disconnect desynced players immediately
    DisconnectImmediate,
    /// Initiate vote to disconnect desynced players
    DisconnectVote,
    /// Pause game and wait for manual resolution
    PauseGame,
    /// Log but continue (for debugging)
    LogOnly,
}

/// Record of a desync event
#[derive(Debug, Clone)]
pub struct DesyncRecord {
    pub frame: FrameNumber,
    pub timestamp: u64,
    pub local_crc: CRCValue,
    pub remote_crcs: HashMap<PlayerId, CRCValue>,
    pub desynced_players: Vec<PlayerId>,
}

impl DesyncRecord {
    /// Create a new desync record
    pub fn new(
        frame: FrameNumber,
        local_crc: CRCValue,
        remote_crcs: HashMap<PlayerId, CRCValue>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Find players with mismatching CRCs
        let desynced_players: Vec<PlayerId> = remote_crcs
            .iter()
            .filter(|(_, &crc)| crc != local_crc)
            .map(|(&player_id, _)| player_id)
            .collect();

        Self {
            frame,
            timestamp,
            local_crc,
            remote_crcs,
            desynced_players,
        }
    }

    /// Format desync record as string for logging
    pub fn to_log_string(&self) -> String {
        let mut log = format!(
            "Desync at frame {}: local CRC {:08x}\n",
            self.frame, self.local_crc
        );

        for (player_id, crc) in &self.remote_crcs {
            let status = if self.desynced_players.contains(player_id) {
                "MISMATCH"
            } else {
                "OK"
            };
            log.push_str(&format!(
                "  Player {}: {:08x} [{}]\n",
                player_id, crc, status
            ));
        }

        log
    }
}

/// Desync handler that detects and responds to desynchronization
pub struct DesyncHandler<G: GameState> {
    game_state: Arc<Mutex<G>>,
    strategy: DesyncStrategy,
    desync_history: Vec<DesyncRecord>,
    max_history: usize,
    dump_directory: Option<PathBuf>,
}

impl<G: GameState> DesyncHandler<G> {
    /// Create a new desync handler with default settings
    pub fn new(game_state: Arc<Mutex<G>>) -> Self {
        Self {
            game_state,
            strategy: DesyncStrategy::DisconnectVote,
            desync_history: Vec::new(),
            max_history: 10,
            dump_directory: None,
        }
    }

    /// Create a desync handler with custom settings
    pub fn with_settings(
        game_state: Arc<Mutex<G>>,
        strategy: DesyncStrategy,
        dump_directory: Option<PathBuf>,
    ) -> Self {
        Self {
            game_state,
            strategy,
            desync_history: Vec::new(),
            max_history: 10,
            dump_directory,
        }
    }

    /// Detect desynchronization by comparing CRCs
    ///
    /// # Arguments
    /// * `frame` - Frame number to check
    /// * `local_crc` - CRC computed locally
    /// * `remote_crcs` - Map of player ID to their CRC values
    ///
    /// # Returns
    /// * `DesyncStatus` indicating whether desync was detected
    pub fn detect_desync(
        &mut self,
        frame: FrameNumber,
        local_crc: CRCValue,
        remote_crcs: HashMap<PlayerId, CRCValue>,
    ) -> DesyncStatus {
        // Check if any remote CRC differs from local
        let desynced_players: Vec<PlayerId> = remote_crcs
            .iter()
            .filter(|(_, &crc)| crc != local_crc)
            .map(|(&player_id, _)| player_id)
            .collect();

        if desynced_players.is_empty() {
            return DesyncStatus::Synchronized;
        }

        // Create desync record
        let record = DesyncRecord::new(frame, local_crc, remote_crcs);

        warn!("Desynchronization detected!");
        warn!("{}", record.to_log_string());

        // Store in history
        self.store_desync_record(record);

        DesyncStatus::Desynchronized {
            frame,
            desynced_players,
        }
    }

    /// Handle desynchronization according to configured strategy
    pub fn handle_desync(
        &mut self,
        frame: FrameNumber,
        local_crc: CRCValue,
        remote_crc: CRCValue,
        desynced_player: PlayerId,
    ) -> NetworkResult<()> {
        error!(
            "Desynchronization at frame {}: local {:08x}, player {} {:08x}",
            frame, local_crc, desynced_player, remote_crc
        );

        // Save state dump if directory configured
        if let Some(ref dir) = self.dump_directory {
            self.save_state_dump(frame, dir)?;
        }

        // Notify game state
        let mut game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        game.handle_desync(frame, local_crc, remote_crc);
        drop(game);

        // Apply strategy
        match self.strategy {
            DesyncStrategy::DisconnectImmediate => {
                info!("Disconnecting player {} immediately", desynced_player);
                // Caller should disconnect the player
                Ok(())
            }
            DesyncStrategy::DisconnectVote => {
                info!("Initiating disconnect vote for player {}", desynced_player);
                // Caller should initiate vote
                Ok(())
            }
            DesyncStrategy::PauseGame => {
                info!("Pausing game due to desync");
                // Caller should pause the game
                Ok(())
            }
            DesyncStrategy::LogOnly => {
                warn!("Desync logged, continuing game (debug mode)");
                Ok(())
            }
        }
    }

    /// Save game state dump for debugging
    fn save_state_dump(&self, frame: FrameNumber, directory: &PathBuf) -> NetworkResult<()> {
        let game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let filename = format!("desync_frame_{}_time_{}.txt", frame, timestamp);
        let filepath = directory.join(filename);

        let mut file = File::create(&filepath)
            .map_err(|e| NetworkError::generic(format!("Failed to create dump file: {}", e)))?;

        writeln!(file, "=== DESYNC STATE DUMP ===")
            .map_err(|e| NetworkError::generic(format!("Failed to write dump: {}", e)))?;

        writeln!(file, "Frame: {}", frame)
            .map_err(|e| NetworkError::generic(format!("Failed to write dump: {}", e)))?;

        writeln!(file, "Timestamp: {}", timestamp)
            .map_err(|e| NetworkError::generic(format!("Failed to write dump: {}", e)))?;

        writeln!(file, "\n{}", game.get_state_dump())
            .map_err(|e| NetworkError::generic(format!("Failed to write dump: {}", e)))?;

        info!("State dump saved to: {:?}", filepath);

        Ok(())
    }

    /// Store desync record in history
    fn store_desync_record(&mut self, record: DesyncRecord) {
        self.desync_history.push(record);

        // Prune old records if history too large
        while self.desync_history.len() > self.max_history {
            self.desync_history.remove(0);
        }
    }

    /// Get all desync records
    pub fn get_desync_history(&self) -> &[DesyncRecord] {
        &self.desync_history
    }

    /// Get number of desyncs recorded
    pub fn desync_count(&self) -> usize {
        self.desync_history.len()
    }

    /// Clear desync history
    pub fn clear_history(&mut self) {
        self.desync_history.clear();
    }

    /// Set desync handling strategy
    pub fn set_strategy(&mut self, strategy: DesyncStrategy) {
        self.strategy = strategy;
    }

    /// Get current strategy
    pub fn get_strategy(&self) -> DesyncStrategy {
        self.strategy
    }

    /// Set directory for state dumps
    pub fn set_dump_directory(&mut self, directory: PathBuf) {
        self.dump_directory = Some(directory);
    }
}

/// Multi-player CRC validator
///
/// Collects CRCs from all players and validates consistency.
pub struct MultiPlayerCRCValidator {
    expected_players: Vec<PlayerId>,
    crcs_per_frame: BTreeMap<FrameNumber, HashMap<PlayerId, CRCValue>>,
    max_frames: usize,
}

impl MultiPlayerCRCValidator {
    /// Create a new multi-player CRC validator
    pub fn new(expected_players: Vec<PlayerId>) -> Self {
        Self {
            expected_players,
            crcs_per_frame: BTreeMap::new(),
            max_frames: 100,
        }
    }

    /// Add a CRC from a player for a frame
    pub fn add_crc(&mut self, frame: FrameNumber, player_id: PlayerId, crc: CRCValue) {
        self.crcs_per_frame
            .entry(frame)
            .or_insert_with(HashMap::new)
            .insert(player_id, crc);

        // Prune old frames
        while self.crcs_per_frame.len() > self.max_frames {
            if let Some(oldest_frame) = self.crcs_per_frame.keys().next().copied() {
                self.crcs_per_frame.remove(&oldest_frame);
            }
        }
    }

    /// Check if all players have submitted CRCs for a frame
    pub fn is_frame_complete(&self, frame: FrameNumber) -> bool {
        if let Some(crcs) = self.crcs_per_frame.get(&frame) {
            self.expected_players.iter().all(|p| crcs.contains_key(p))
        } else {
            false
        }
    }

    /// Validate CRCs for a frame
    ///
    /// Returns `DesyncStatus` indicating whether all CRCs match.
    pub fn validate_frame(&self, frame: FrameNumber) -> Option<DesyncStatus> {
        let crcs = self.crcs_per_frame.get(&frame)?;

        // Get reference CRC (from first expected player for deterministic behavior)
        let reference_player = *self.expected_players.first()?;
        let reference_crc = crcs.get(&reference_player).copied()?;

        // Find players with mismatching CRCs
        let desynced_players: Vec<PlayerId> = crcs
            .iter()
            .filter(|(_, &crc)| crc != reference_crc)
            .map(|(&player_id, _)| player_id)
            .collect();

        if desynced_players.is_empty() {
            Some(DesyncStatus::Synchronized)
        } else {
            Some(DesyncStatus::Desynchronized {
                frame,
                desynced_players,
            })
        }
    }

    /// Get CRCs for a specific frame
    pub fn get_frame_crcs(&self, frame: FrameNumber) -> Option<&HashMap<PlayerId, CRCValue>> {
        self.crcs_per_frame.get(&frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::game_state::{EntitySnapshot, GameStateCRC, ResourceState};
    use std::collections::BTreeMap;

    // Mock game state for testing
    struct MockGameState {
        frame: u32,
        desync_handled: bool,
    }

    impl MockGameState {
        fn new() -> Self {
            Self {
                frame: 0,
                desync_handled: false,
            }
        }
    }

    impl GameState for MockGameState {
        fn get_state_for_crc(&self) -> GameStateCRC {
            GameStateCRC {
                frame: self.frame,
                entities: Vec::new(),
                resources: BTreeMap::new(),
                random_seed: 12345,
            }
        }

        fn execute_command(
            &mut self,
            _command: &crate::commands::GameCommandData,
        ) -> super::super::game_state::GameStateResult<()> {
            Ok(())
        }

        fn current_frame(&self) -> u32 {
            self.frame
        }

        fn advance_frame(&mut self) {
            self.frame += 1;
        }

        fn get_entities(&self) -> Vec<EntitySnapshot> {
            Vec::new()
        }

        fn get_resources(&self) -> BTreeMap<u8, ResourceState> {
            BTreeMap::new()
        }

        fn get_random_seed(&self) -> u32 {
            12345
        }

        fn set_random_seed(&mut self, _seed: u32) {}

        fn entity_exists(&self, _entity_id: u32) -> bool {
            false
        }

        fn get_entity_owner(&self, _entity_id: u32) -> Option<u8> {
            None
        }

        fn handle_desync(&mut self, _frame: u32, _local_crc: u32, _remote_crc: u32) {
            self.desync_handled = true;
        }
    }

    #[test]
    fn test_detect_desync_synchronized() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut handler = DesyncHandler::new(game_state);

        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(0, 0x12345678);
        remote_crcs.insert(1, 0x12345678);

        let status = handler.detect_desync(0, 0x12345678, remote_crcs);

        assert_eq!(status, DesyncStatus::Synchronized);
    }

    #[test]
    fn test_detect_desync_desynchronized() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut handler = DesyncHandler::new(game_state);

        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(0, 0x12345678);
        remote_crcs.insert(1, 0x87654321); // Different CRC

        let status = handler.detect_desync(0, 0x12345678, remote_crcs);

        match status {
            DesyncStatus::Desynchronized {
                frame,
                desynced_players,
            } => {
                assert_eq!(frame, 0);
                assert_eq!(desynced_players, vec![1]);
            }
            _ => panic!("Expected desync status"),
        }
    }

    #[test]
    fn test_desync_handler_stores_history() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut handler = DesyncHandler::new(game_state);

        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(1, 0x87654321);

        handler.detect_desync(0, 0x12345678, remote_crcs);

        assert_eq!(handler.desync_count(), 1);
        assert_eq!(handler.get_desync_history().len(), 1);
    }

    #[test]
    fn test_multi_player_validator_add_crc() {
        let mut validator = MultiPlayerCRCValidator::new(vec![0, 1]);

        validator.add_crc(0, 0, 0x12345678);
        validator.add_crc(0, 1, 0x12345678);

        assert!(validator.is_frame_complete(0));
    }

    #[test]
    fn test_multi_player_validator_incomplete_frame() {
        let mut validator = MultiPlayerCRCValidator::new(vec![0, 1]);

        validator.add_crc(0, 0, 0x12345678);
        // Player 1 hasn't submitted yet

        assert!(!validator.is_frame_complete(0));
    }

    #[test]
    fn test_multi_player_validator_synchronized() {
        let mut validator = MultiPlayerCRCValidator::new(vec![0, 1]);

        validator.add_crc(0, 0, 0x12345678);
        validator.add_crc(0, 1, 0x12345678);

        let status = validator.validate_frame(0).unwrap();
        assert_eq!(status, DesyncStatus::Synchronized);
    }

    #[test]
    fn test_multi_player_validator_desynchronized() {
        let mut validator = MultiPlayerCRCValidator::new(vec![0, 1]);

        validator.add_crc(0, 0, 0x12345678);
        validator.add_crc(0, 1, 0x87654321);

        let status = validator.validate_frame(0).unwrap();

        match status {
            DesyncStatus::Desynchronized {
                desynced_players, ..
            } => {
                assert_eq!(desynced_players, vec![1]);
            }
            _ => panic!("Expected desync status"),
        }
    }
}
