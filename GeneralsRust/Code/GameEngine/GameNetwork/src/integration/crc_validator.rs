//! Game State CRC Validation
//!
//! This module implements CRC computation and validation for detecting desynchronization
//! in multiplayer games. It uses a bit-rotation CRC algorithm that matches the C++
//! implementation exactly.

use super::game_state::{CRCValue, EntitySnapshot, FrameNumber, GameState, ResourceState};
use crate::error::{NetworkError, NetworkResult};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

/// CRC computer using bit-rotation algorithm
///
/// This implementation matches the C++ CRC::addCRC() exactly to ensure
/// compatibility between Rust and C++ clients.
#[derive(Debug, Clone)]
pub struct CRCComputer {
    crc: u32,
}

impl CRCComputer {
    /// Create a new CRC computer with initial value of 0
    pub fn new() -> Self {
        Self { crc: 0 }
    }

    /// Create a CRC computer with a specific initial value
    pub fn with_initial(initial: u32) -> Self {
        Self { crc: initial }
    }

    /// Add a single byte to the CRC
    ///
    /// Uses bit-rotation algorithm matching C++ implementation:
    /// ```c++
    /// UnsignedInt hibit = (crc & 0x80000000) ? 1 : 0;
    /// crc <<= 1;
    /// crc += val;
    /// crc += hibit;
    /// ```
    pub fn add_byte(&mut self, val: u8) {
        let hibit = if self.crc & 0x80000000 != 0 { 1 } else { 0 };

        self.crc = self.crc.wrapping_shl(1);
        self.crc = self.crc.wrapping_add(val as u32);
        self.crc = self.crc.wrapping_add(hibit);
    }

    /// Add multiple bytes to the CRC
    pub fn add_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.add_byte(byte);
        }
    }

    /// Add a u32 value to the CRC (little-endian)
    pub fn add_u32(&mut self, val: u32) {
        self.add_bytes(&val.to_le_bytes());
    }

    /// Add a i32 value to the CRC (little-endian)
    pub fn add_i32(&mut self, val: i32) {
        self.add_bytes(&val.to_le_bytes());
    }

    /// Add a f32 value to the CRC (using to_bits() for determinism)
    pub fn add_f32(&mut self, val: f32) {
        self.add_bytes(&val.to_bits().to_le_bytes());
    }

    /// Get the current CRC value
    pub fn get(&self) -> u32 {
        self.crc
    }

    /// Reset the CRC to 0
    pub fn reset(&mut self) {
        self.crc = 0;
    }
}

impl Default for CRCComputer {
    fn default() -> Self {
        Self::new()
    }
}

/// Game state CRC validator
///
/// Computes and validates CRCs for game state to detect desynchronization.
pub struct GameStateCRCValidator<G: GameState> {
    game_state: Arc<Mutex<G>>,
    crc_history: BTreeMap<FrameNumber, CRCValue>,
    max_history: usize,
}

impl<G: GameState> GameStateCRCValidator<G> {
    /// Create a new CRC validator
    pub fn new(game_state: Arc<Mutex<G>>) -> Self {
        Self {
            game_state,
            crc_history: BTreeMap::new(),
            max_history: 100, // Keep last 100 frames
        }
    }

    /// Create a CRC validator with custom history size
    pub fn with_history_size(game_state: Arc<Mutex<G>>, max_history: usize) -> Self {
        Self {
            game_state,
            crc_history: BTreeMap::new(),
            max_history,
        }
    }

    /// Compute CRC for the current game state
    ///
    /// This computes a CRC over all mutable game state in deterministic order:
    /// 1. Frame number
    /// 2. All entities (sorted by ID)
    /// 3. All resources (sorted by player ID)
    /// 4. Random seed
    pub fn compute_crc(&self, frame: FrameNumber) -> NetworkResult<CRCValue> {
        let game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        let mut crc = CRCComputer::new();

        // Add frame number
        crc.add_u32(frame);

        // Add all entities in deterministic order (sorted by ID)
        let mut entities = game.get_entities();
        entities.sort_by_key(|e| e.id);

        for entity in entities {
            crc.add_bytes(&entity.to_bytes());
        }

        debug!("Frame {}: CRC after entities = {:08x}", frame, crc.get());

        // Add all resources in deterministic order (sorted by player ID)
        let resources = game.get_resources();
        for (player_id, resource_state) in resources {
            crc.add_byte(player_id);
            crc.add_bytes(&resource_state.to_bytes());
        }

        debug!("Frame {}: CRC after resources = {:08x}", frame, crc.get());

        // Add random seed
        crc.add_u32(game.get_random_seed());

        let final_crc = crc.get();
        debug!("Frame {}: Final CRC = {:08x}", frame, final_crc);

        Ok(final_crc)
    }

    /// Validate CRC against a remote CRC
    ///
    /// Returns Ok(true) if CRCs match, Ok(false) if they don't match.
    /// Returns Err if CRC computation fails.
    pub fn validate(&mut self, frame: FrameNumber, remote_crc: CRCValue) -> NetworkResult<bool> {
        let local_crc = self.compute_crc(frame)?;

        // Store in history
        self.store_crc(frame, local_crc);

        if local_crc != remote_crc {
            warn!(
                "CRC mismatch at frame {}: local {:08x} != remote {:08x}",
                frame, local_crc, remote_crc
            );

            // Handle desync in game state
            let mut game = self
                .game_state
                .lock()
                .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

            game.handle_desync(frame, local_crc, remote_crc);

            Ok(false)
        } else {
            debug!("CRC match at frame {}: {:08x}", frame, local_crc);
            Ok(true)
        }
    }

    /// Store CRC in history and prune old entries
    fn store_crc(&mut self, frame: FrameNumber, crc: CRCValue) {
        self.crc_history.insert(frame, crc);

        // Prune history if too large
        while self.crc_history.len() > self.max_history {
            if let Some(oldest_frame) = self.crc_history.keys().next().copied() {
                self.crc_history.remove(&oldest_frame);
            }
        }
    }

    /// Get CRC for a specific frame from history
    pub fn get_crc(&self, frame: FrameNumber) -> Option<CRCValue> {
        self.crc_history.get(&frame).copied()
    }

    /// Get all CRCs in history
    pub fn get_crc_history(&self) -> &BTreeMap<FrameNumber, CRCValue> {
        &self.crc_history
    }

    /// Clear CRC history
    pub fn clear_history(&mut self) {
        self.crc_history.clear();
    }

    /// Get number of CRCs stored in history
    pub fn history_size(&self) -> usize {
        self.crc_history.len()
    }
}

/// Helper to compute CRC for a list of entities
pub fn compute_entities_crc(entities: &[EntitySnapshot]) -> CRCValue {
    let mut crc = CRCComputer::new();

    for entity in entities {
        crc.add_bytes(&entity.to_bytes());
    }

    crc.get()
}

/// Helper to compute CRC for resource states
pub fn compute_resources_crc(resources: &BTreeMap<u8, ResourceState>) -> CRCValue {
    let mut crc = CRCComputer::new();

    for (player_id, resource_state) in resources {
        crc.add_byte(*player_id);
        crc.add_bytes(&resource_state.to_bytes());
    }

    crc.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::game_state::GameStateCRC;

    // Mock game state for testing
    struct MockGameState {
        frame: u32,
        entities: Vec<EntitySnapshot>,
        resources: BTreeMap<u8, ResourceState>,
        random_seed: u32,
    }

    impl MockGameState {
        fn new() -> Self {
            Self {
                frame: 0,
                entities: Vec::new(),
                resources: BTreeMap::new(),
                random_seed: 12345,
            }
        }
    }

    impl GameState for MockGameState {
        fn get_state_for_crc(&self) -> GameStateCRC {
            GameStateCRC {
                frame: self.frame,
                entities: self.entities.clone(),
                resources: self.resources.clone(),
                random_seed: self.random_seed,
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
            self.entities.clone()
        }

        fn get_resources(&self) -> BTreeMap<u8, ResourceState> {
            self.resources.clone()
        }

        fn get_random_seed(&self) -> u32 {
            self.random_seed
        }

        fn set_random_seed(&mut self, seed: u32) {
            self.random_seed = seed;
        }

        fn entity_exists(&self, entity_id: u32) -> bool {
            self.entities.iter().any(|e| e.id == entity_id)
        }

        fn get_entity_owner(&self, entity_id: u32) -> Option<u8> {
            self.entities
                .iter()
                .find(|e| e.id == entity_id)
                .map(|e| e.owner)
        }

        fn handle_desync(&mut self, _frame: u32, _local_crc: u32, _remote_crc: u32) {}
    }

    #[test]
    fn test_crc_computer_add_byte() {
        let mut crc = CRCComputer::new();

        crc.add_byte(42);
        assert_ne!(crc.get(), 0);

        let first_crc = crc.get();

        crc.add_byte(43);
        assert_ne!(crc.get(), first_crc);
    }

    #[test]
    fn test_crc_deterministic() {
        let mut crc1 = CRCComputer::new();
        crc1.add_byte(1);
        crc1.add_byte(2);
        crc1.add_byte(3);

        let mut crc2 = CRCComputer::new();
        crc2.add_byte(1);
        crc2.add_byte(2);
        crc2.add_byte(3);

        assert_eq!(crc1.get(), crc2.get());
    }

    #[test]
    fn test_crc_order_sensitive() {
        let mut crc1 = CRCComputer::new();
        crc1.add_byte(1);
        crc1.add_byte(2);

        let mut crc2 = CRCComputer::new();
        crc2.add_byte(2);
        crc2.add_byte(1);

        assert_ne!(crc1.get(), crc2.get());
    }

    #[test]
    fn test_crc_validator_empty_state() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let validator = GameStateCRCValidator::new(game_state);

        let crc = validator.compute_crc(0).unwrap();
        assert_ne!(crc, 0); // Should have non-zero CRC due to frame number and random seed
    }

    #[test]
    fn test_crc_validator_with_entities() {
        let mut mock = MockGameState::new();
        mock.entities.push(EntitySnapshot {
            id: 1,
            position: (100.0, 200.0, 0.0),
            health: 100,
            owner: 0,
            entity_type: 5,
            state: 1,
        });

        let game_state = Arc::new(Mutex::new(mock));
        let validator = GameStateCRCValidator::new(game_state);

        let crc = validator.compute_crc(0).unwrap();
        assert_ne!(crc, 0);
    }

    #[test]
    fn test_crc_validator_deterministic() {
        let mut mock = MockGameState::new();
        mock.entities.push(EntitySnapshot {
            id: 1,
            position: (100.0, 200.0, 0.0),
            health: 100,
            owner: 0,
            entity_type: 5,
            state: 1,
        });

        let game_state = Arc::new(Mutex::new(mock));
        let validator = GameStateCRCValidator::new(game_state);

        let crc1 = validator.compute_crc(0).unwrap();
        let crc2 = validator.compute_crc(0).unwrap();

        assert_eq!(crc1, crc2);
    }

    #[test]
    fn test_crc_validation_match() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut validator = GameStateCRCValidator::new(game_state.clone());

        let local_crc = validator.compute_crc(0).unwrap();
        let result = validator.validate(0, local_crc).unwrap();

        assert!(result);
    }

    #[test]
    fn test_crc_validation_mismatch() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut validator = GameStateCRCValidator::new(game_state);

        let local_crc = validator.compute_crc(0).unwrap();
        let wrong_crc = local_crc.wrapping_add(1);

        let result = validator.validate(0, wrong_crc).unwrap();

        assert!(!result);
    }

    #[test]
    fn test_crc_history() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut validator = GameStateCRCValidator::with_history_size(game_state, 5);

        // Compute CRCs for frames 0-9
        for frame in 0..10 {
            let local_crc = validator.compute_crc(frame).unwrap();
            validator.validate(frame, local_crc).unwrap();
        }

        // Should only keep last 5 frames
        assert_eq!(validator.history_size(), 5);

        // Oldest frames should be pruned
        assert!(validator.get_crc(0).is_none());
        assert!(validator.get_crc(4).is_none());

        // Recent frames should be available
        assert!(validator.get_crc(5).is_some());
        assert!(validator.get_crc(9).is_some());
    }

    #[test]
    fn test_compute_entities_crc() {
        let entities = vec![
            EntitySnapshot {
                id: 1,
                position: (100.0, 200.0, 0.0),
                health: 100,
                owner: 0,
                entity_type: 5,
                state: 1,
            },
            EntitySnapshot {
                id: 2,
                position: (150.0, 250.0, 0.0),
                health: 100,
                owner: 1,
                entity_type: 5,
                state: 1,
            },
        ];

        let crc = compute_entities_crc(&entities);
        assert_ne!(crc, 0);
    }

    #[test]
    fn test_compute_resources_crc() {
        let mut resources = BTreeMap::new();
        resources.insert(
            0,
            ResourceState {
                money: 1000,
                power: 50,
                power_consumed: 30,
            },
        );

        let crc = compute_resources_crc(&resources);
        assert_ne!(crc, 0);
    }
}
