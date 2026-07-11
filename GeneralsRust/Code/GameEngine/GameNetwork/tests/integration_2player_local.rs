#![allow(unused_crate_dependencies)]

//! Comprehensive 2-Player Local Multiplayer Integration Test
//!
//! This test demonstrates a complete two-player game with:
//! - Full frame synchronization (lockstep networking)
//! - Command collection and execution
//! - CRC computation and validation
//! - Desync detection and reporting
//! - Deterministic state management
//! - 100-frame synchronization test
//!
//! The test simulates two local game instances (Player 0 and Player 1) running
//! a synchronized game loop for 100 frames, verifying that:
//! - Both players execute the same commands
//! - CRCs match every frame
//! - No desynchronization occurs
//! - Final game states are identical

use game_network::commands::{CommandParameter, GameCommandData};
use game_network::error::NetworkResult;
use game_network::integration::{
    crc_validator::CRCComputer,
    desync_handler::{DesyncStatus, MultiPlayerCRCValidator},
    game_state::{
        CRCValue, EntitySnapshot, FrameNumber, GameState, GameStateCRC, PlayerId, ResourceState,
    },
};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Simple deterministic game state for testing
#[derive(Debug, Clone)]
struct SimpleGameState {
    frame: FrameNumber,
    num_players: u8,
    entities: Vec<EntitySnapshot>,
    resources: BTreeMap<PlayerId, ResourceState>,
    random_seed: u32,
    command_history: Vec<(FrameNumber, PlayerId, u32)>, // (frame, player, command_type)
    desync_detected: bool,
}

impl SimpleGameState {
    fn new(num_players: u8) -> Self {
        let mut resources = BTreeMap::new();
        for player_id in 0..num_players {
            resources.insert(
                player_id,
                ResourceState {
                    money: 10000,
                    power: 100,
                    power_consumed: 0,
                },
            );
        }

        // Start with a few entities for each player
        let mut entities = Vec::new();
        for player_id in 0..num_players {
            // Each player starts with a command center
            entities.push(EntitySnapshot {
                id: (player_id as u32) * 1000,
                position: (100.0 * player_id as f32, 100.0 * player_id as f32, 0.0),
                health: 1000,
                owner: player_id,
                entity_type: 1, // Command Center
                state: 0,
            });

            // And 3 workers
            for i in 0..3 {
                entities.push(EntitySnapshot {
                    id: (player_id as u32) * 1000 + i + 1,
                    position: (
                        100.0 * player_id as f32 + 10.0 * i as f32,
                        100.0 * player_id as f32 + 10.0,
                        0.0,
                    ),
                    health: 100,
                    owner: player_id,
                    entity_type: 2, // Worker
                    state: 0,
                });
            }
        }

        let random_seed = 12345;

        Self {
            frame: 0,
            num_players,
            entities,
            resources,
            random_seed,
            command_history: Vec::new(),
            desync_detected: false,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Frame number
        bytes.extend_from_slice(&self.frame.to_le_bytes());

        // Entities (sorted by ID for determinism)
        let mut sorted_entities = self.entities.clone();
        sorted_entities.sort_by_key(|e| e.id);
        bytes.extend_from_slice(&(sorted_entities.len() as u32).to_le_bytes());
        for entity in sorted_entities {
            bytes.extend_from_slice(&entity.to_bytes());
        }

        // Resources
        bytes.extend_from_slice(&(self.resources.len() as u32).to_le_bytes());
        for (player_id, resource) in &self.resources {
            bytes.push(*player_id);
            bytes.extend_from_slice(&resource.to_bytes());
        }

        // Random seed
        bytes.extend_from_slice(&self.random_seed.to_le_bytes());

        bytes
    }

    fn compute_crc32(&self) -> CRCValue {
        let mut crc = CRCComputer::new();

        // Add frame
        crc.add_u32(self.frame);

        // Add entities in sorted order
        let mut sorted_entities = self.entities.clone();
        sorted_entities.sort_by_key(|e| e.id);
        for entity in sorted_entities {
            crc.add_bytes(&entity.to_bytes());
        }

        // Add resources
        for (player_id, resource) in &self.resources {
            crc.add_byte(*player_id);
            crc.add_bytes(&resource.to_bytes());
        }

        // Add random seed
        crc.add_u32(self.random_seed);

        crc.get()
    }

    fn apply_command(&mut self, command: &GameCommandData) -> NetworkResult<()> {
        // Record command in history
        let player_id = command
            .parameters
            .get("player_id")
            .and_then(|p| {
                if let CommandParameter::Int(id) = p {
                    Some(*id as u8)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        self.command_history
            .push((self.frame, player_id, command.command_type));

        // Process command based on type
        match command.command_type {
            1 => self.apply_move_command(command),
            2 => self.apply_build_command(command),
            3 => self.apply_attack_command(command),
            4 => self.apply_gather_command(command),
            _ => Ok(()),
        }
    }

    fn apply_move_command(&mut self, command: &GameCommandData) -> NetworkResult<()> {
        if let (
            Some(CommandParameter::ObjectId(entity_id)),
            Some(CommandParameter::Position(x, y, z)),
        ) = (
            command.parameters.get("entity_id"),
            command.parameters.get("target_pos"),
        ) {
            if let Some(entity) = self.entities.iter_mut().find(|e| e.id == *entity_id) {
                // Move entity towards target (simplified)
                entity.position = (*x, *y, *z);
            }
        }
        Ok(())
    }

    fn apply_build_command(&mut self, command: &GameCommandData) -> NetworkResult<()> {
        if let (Some(CommandParameter::Int(player_id)), Some(CommandParameter::Int(entity_type))) = (
            command.parameters.get("player_id"),
            command.parameters.get("entity_type"),
        ) {
            let player_id = *player_id as u8;

            // Check resources
            if let Some(resources) = self.resources.get_mut(&player_id) {
                let cost = 100;
                if resources.money >= cost {
                    resources.money -= cost;

                    // Create new entity
                    let new_id = self.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
                    let base_pos = self
                        .entities
                        .iter()
                        .find(|e| e.owner == player_id && e.entity_type == 1)
                        .map(|e| e.position)
                        .unwrap_or((0.0, 0.0, 0.0));

                    self.entities.push(EntitySnapshot {
                        id: new_id,
                        position: (base_pos.0 + 20.0, base_pos.1 + 20.0, 0.0),
                        health: 100,
                        owner: player_id,
                        entity_type: *entity_type as u16,
                        state: 0,
                    });
                }
            }
        }
        Ok(())
    }

    fn apply_attack_command(&mut self, command: &GameCommandData) -> NetworkResult<()> {
        if let (
            Some(CommandParameter::ObjectId(_attacker_id)),
            Some(CommandParameter::ObjectId(target_id)),
        ) = (
            command.parameters.get("attacker_id"),
            command.parameters.get("target_id"),
        ) {
            // Simplified combat: reduce target health
            if let Some(target) = self.entities.iter_mut().find(|e| e.id == *target_id) {
                target.health = target.health.saturating_sub(10);
            }

            // Remove dead entities
            self.entities.retain(|e| e.health > 0);
        }
        Ok(())
    }

    fn apply_gather_command(&mut self, command: &GameCommandData) -> NetworkResult<()> {
        if let Some(CommandParameter::Int(player_id)) = command.parameters.get("player_id") {
            let player_id = *player_id as u8;

            // Add resources
            if let Some(resources) = self.resources.get_mut(&player_id) {
                resources.money += 10;
            }
        }
        Ok(())
    }
}

impl GameState for SimpleGameState {
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
        command: &GameCommandData,
    ) -> game_network::integration::game_state::GameStateResult<()> {
        self.apply_command(command).map_err(|_| {
            game_network::integration::game_state::GameStateError::ExecutionFailed(
                "Command failed".to_string(),
            )
        })
    }

    fn current_frame(&self) -> FrameNumber {
        self.frame
    }

    fn advance_frame(&mut self) {
        self.frame += 1;

        // Update random seed deterministically
        self.random_seed = self
            .random_seed
            .wrapping_mul(1103515245)
            .wrapping_add(12345);
    }

    fn get_entities(&self) -> Vec<EntitySnapshot> {
        let mut entities = self.entities.clone();
        entities.sort_by_key(|e| e.id);
        entities
    }

    fn get_resources(&self) -> BTreeMap<PlayerId, ResourceState> {
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

    fn get_entity_owner(&self, entity_id: u32) -> Option<PlayerId> {
        self.entities
            .iter()
            .find(|e| e.id == entity_id)
            .map(|e| e.owner)
    }

    fn handle_desync(&mut self, frame: FrameNumber, local_crc: CRCValue, remote_crc: CRCValue) {
        eprintln!(
            "DESYNC at frame {}: local {:08x} != remote {:08x}",
            frame, local_crc, remote_crc
        );
        self.desync_detected = true;
    }
}

/// Command generator for deterministic testing
struct CommandGenerator {
    seed: u64,
    player_id: PlayerId,
    call_count: u64,
}

impl CommandGenerator {
    fn new(player_id: PlayerId, seed: u64) -> Self {
        Self {
            seed,
            player_id,
            call_count: 0,
        }
    }

    fn next_u32(&mut self) -> u32 {
        self.call_count += 1;
        let mut x = self.seed.wrapping_add(self.call_count);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x as u32
    }

    fn gen_range(&mut self, min: u32, max: u32) -> u32 {
        min + (self.next_u32() % (max - min))
    }

    fn gen_bool(&mut self, probability: f64) -> bool {
        let threshold = (probability * (u32::MAX as f64)) as u32;
        self.next_u32() < threshold
    }

    fn generate_random_command(&mut self, game_state: &SimpleGameState) -> Option<GameCommandData> {
        // 30% chance of no command
        if self.gen_bool(0.3) {
            return None;
        }

        let command_type = self.gen_range(1, 5); // 1=move, 2=build, 3=attack, 4=gather

        let mut parameters = HashMap::new();
        parameters.insert(
            "player_id".to_string(),
            CommandParameter::Int(self.player_id as i32),
        );

        match command_type {
            1 => {
                // Move command
                let entities: Vec<_> = game_state
                    .entities
                    .iter()
                    .filter(|e| e.owner == self.player_id)
                    .collect();

                if !entities.is_empty() {
                    let idx = (self.gen_range(0, entities.len() as u32)) as usize;
                    let entity = entities[idx];
                    parameters.insert(
                        "entity_id".to_string(),
                        CommandParameter::ObjectId(entity.id),
                    );

                    let x = entity.position.0 + (self.next_u32() as f32 / u32::MAX as f32) * 100.0
                        - 50.0;
                    let y = entity.position.1 + (self.next_u32() as f32 / u32::MAX as f32) * 100.0
                        - 50.0;
                    parameters.insert(
                        "target_pos".to_string(),
                        CommandParameter::Position(x, y, 0.0),
                    );
                }
            }
            2 => {
                // Build command
                let entity_type = self.gen_range(2, 6) as i32;
                parameters.insert(
                    "entity_type".to_string(),
                    CommandParameter::Int(entity_type),
                );
            }
            3 => {
                // Attack command
                let my_entities: Vec<_> = game_state
                    .entities
                    .iter()
                    .filter(|e| e.owner == self.player_id)
                    .collect();
                let enemy_entities: Vec<_> = game_state
                    .entities
                    .iter()
                    .filter(|e| e.owner != self.player_id)
                    .collect();

                if !my_entities.is_empty() && !enemy_entities.is_empty() {
                    let attacker_idx = (self.gen_range(0, my_entities.len() as u32)) as usize;
                    let target_idx = (self.gen_range(0, enemy_entities.len() as u32)) as usize;

                    let attacker = my_entities[attacker_idx];
                    let target = enemy_entities[target_idx];

                    parameters.insert(
                        "attacker_id".to_string(),
                        CommandParameter::ObjectId(attacker.id),
                    );
                    parameters.insert(
                        "target_id".to_string(),
                        CommandParameter::ObjectId(target.id),
                    );
                }
            }
            4 => {
                // Gather command (just adds resources)
                // No extra parameters needed
            }
            _ => {}
        }

        Some(GameCommandData {
            command_type,
            target_id: None,
            position: None,
            parameters,
            checksum: 0,
        })
    }
}

/// Test metrics tracking
#[derive(Debug, Default)]
struct TestMetrics {
    frames_executed: u32,
    commands_executed: u32,
    crc_matches: u32,
    crc_mismatches: u32,
    total_frame_time: Duration,
    min_frame_time: Duration,
    max_frame_time: Duration,
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            min_frame_time: Duration::from_secs(1000),
            max_frame_time: Duration::ZERO,
            ..Default::default()
        }
    }

    fn record_frame(&mut self, duration: Duration, crc_match: bool, commands: u32) {
        self.frames_executed += 1;
        self.commands_executed += commands;
        self.total_frame_time += duration;

        if duration < self.min_frame_time {
            self.min_frame_time = duration;
        }
        if duration > self.max_frame_time {
            self.max_frame_time = duration;
        }

        if crc_match {
            self.crc_matches += 1;
        } else {
            self.crc_mismatches += 1;
        }
    }

    fn average_frame_time(&self) -> Duration {
        if self.frames_executed > 0 {
            self.total_frame_time / self.frames_executed
        } else {
            Duration::ZERO
        }
    }

    fn crc_match_rate(&self) -> f64 {
        if self.frames_executed > 0 {
            (self.crc_matches as f64 / self.frames_executed as f64) * 100.0
        } else {
            0.0
        }
    }

    fn print_summary(&self) {
        println!("\n=== Test Metrics Summary ===");
        println!("Frames executed: {}", self.frames_executed);
        println!("Commands executed: {}", self.commands_executed);
        println!("CRC matches: {}", self.crc_matches);
        println!("CRC mismatches: {}", self.crc_mismatches);
        println!("CRC match rate: {:.2}%", self.crc_match_rate());
        println!("Average frame time: {:?}", self.average_frame_time());
        println!("Min frame time: {:?}", self.min_frame_time);
        println!("Max frame time: {:?}", self.max_frame_time);
        println!("===========================\n");
    }
}

/// Main test: 100-frame 2-player synchronization
#[tokio::test]
async fn test_two_player_100_frame_sync() {
    println!("\n### Starting 2-Player 100-Frame Synchronization Test ###\n");

    // Initialize game states for both players
    let mut player1 = SimpleGameState::new(2);
    let mut player2 = SimpleGameState::new(2);

    // Verify initial states match
    assert_eq!(
        player1.serialize(),
        player2.serialize(),
        "Initial states must match"
    );

    let initial_crc1 = player1.compute_crc32();
    let initial_crc2 = player2.compute_crc32();
    assert_eq!(
        initial_crc1, initial_crc2,
        "Initial CRCs must match: {:08x} vs {:08x}",
        initial_crc1, initial_crc2
    );

    println!("Initial state CRC: {:08x}", initial_crc1);
    println!("Initial entities: {}", player1.entities.len());
    println!(
        "Initial resources P0: ${}, P1: ${}\n",
        player1.resources.get(&0).unwrap().money,
        player1.resources.get(&1).unwrap().money
    );

    // Create CRC validator for multi-player
    let mut crc_validator = MultiPlayerCRCValidator::new(vec![0u8, 1u8]);

    // Create command generators with same seeds for determinism
    let mut p1_cmd_gen = CommandGenerator::new(0, 42);
    let mut p2_cmd_gen = CommandGenerator::new(1, 43);

    // Metrics
    let mut metrics = TestMetrics::new();

    // Run 100 frames
    for frame in 0..100 {
        let frame_start = Instant::now();

        // 1. Collect commands from both players (deterministic)
        let p1_cmd = p1_cmd_gen.generate_random_command(&player1);
        let p2_cmd = p2_cmd_gen.generate_random_command(&player2);

        let mut commands_this_frame = 0;

        // 2. Execute commands on both players in deterministic order (by player ID)
        if let Some(cmd) = &p1_cmd {
            player1
                .execute_command(cmd)
                .expect("P1 command execution failed");
            player2
                .execute_command(cmd)
                .expect("P2 command execution failed");
            commands_this_frame += 1;
        }

        if let Some(cmd) = &p2_cmd {
            player1
                .execute_command(cmd)
                .expect("P1 command execution failed");
            player2
                .execute_command(cmd)
                .expect("P2 command execution failed");
            commands_this_frame += 1;
        }

        // 3. Compute CRC for both
        let p1_crc = player1.compute_crc32();
        let p2_crc = player2.compute_crc32();

        // 4. Validate CRCs match
        let crc_match = p1_crc == p2_crc;

        if !crc_match {
            eprintln!("DESYNC DETECTED at frame {}!", frame);
            eprintln!("  Player 1 CRC: {:08x}", p1_crc);
            eprintln!("  Player 2 CRC: {:08x}", p2_crc);
            eprintln!("  Player 1 entities: {}", player1.entities.len());
            eprintln!("  Player 2 entities: {}", player2.entities.len());
            panic!(
                "Frame {} desynchronized: {:08x} != {:08x}",
                frame, p1_crc, p2_crc
            );
        }

        // Record in CRC validator
        crc_validator.add_crc(frame, 0, p1_crc);
        crc_validator.add_crc(frame, 1, p2_crc);

        // Validate frame CRCs
        if let Some(status) = crc_validator.validate_frame(frame) {
            assert_eq!(
                status,
                DesyncStatus::Synchronized,
                "CRC validator detected mismatch at frame {}",
                frame
            );
        }

        // 5. Advance both to next frame
        player1.advance_frame();
        player2.advance_frame();

        // Record metrics
        let frame_time = frame_start.elapsed();
        metrics.record_frame(frame_time, crc_match, commands_this_frame);

        // Periodic progress report
        if (frame + 1) % 25 == 0 {
            println!(
                "Frame {}: CRC {:08x}, {} commands, {} entities P1, {} entities P2",
                frame,
                p1_crc,
                commands_this_frame,
                player1.entities.len(),
                player2.entities.len()
            );
        }
    }

    // Verify final states match
    println!("\nVerifying final state...");

    assert_eq!(
        player1.current_frame(),
        100,
        "Player 1 should be at frame 100"
    );
    assert_eq!(
        player2.current_frame(),
        100,
        "Player 2 should be at frame 100"
    );

    let final_serialized1 = player1.serialize();
    let final_serialized2 = player2.serialize();
    assert_eq!(
        final_serialized1,
        final_serialized2,
        "Final serialized states must match. P1 len: {}, P2 len: {}",
        final_serialized1.len(),
        final_serialized2.len()
    );

    let final_crc1 = player1.compute_crc32();
    let final_crc2 = player2.compute_crc32();
    assert_eq!(final_crc1, final_crc2, "Final CRCs must match");

    println!("Final CRC: {:08x}", final_crc1);
    println!("Final entities P1: {}", player1.entities.len());
    println!("Final entities P2: {}", player2.entities.len());
    println!(
        "Final resources P0: ${}, P1: ${}",
        player1.resources.get(&0).unwrap().money,
        player1.resources.get(&1).unwrap().money
    );

    // Verify no desyncs
    assert!(!player1.desync_detected, "Player 1 detected desync");
    assert!(!player2.desync_detected, "Player 2 detected desync");

    // Print metrics
    metrics.print_summary();

    // Final assertions
    assert_eq!(metrics.crc_mismatches, 0, "Should have 0 CRC mismatches");
    assert_eq!(
        metrics.frames_executed, 100,
        "Should execute exactly 100 frames"
    );
    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "CRC match rate should be 100%"
    );

    println!("### 2-Player 100-Frame Synchronization Test PASSED ###\n");
}

/// Test: CRC changes when state changes
#[tokio::test]
async fn test_crc_changes_on_state_change() {
    println!("\n### Testing CRC Changes on State Modification ###\n");

    let mut state = SimpleGameState::new(2);
    let initial_crc = state.compute_crc32();

    println!("Initial CRC: {:08x}", initial_crc);

    // Modify state - add entity
    state.entities.push(EntitySnapshot {
        id: 9999,
        position: (50.0, 50.0, 0.0),
        health: 100,
        owner: 0,
        entity_type: 3,
        state: 0,
    });

    let modified_crc = state.compute_crc32();
    println!("CRC after adding entity: {:08x}", modified_crc);

    assert_ne!(
        initial_crc, modified_crc,
        "CRC must change when entity is added"
    );

    // Modify state - change resources
    state.resources.get_mut(&0).unwrap().money = 5000;
    let resource_modified_crc = state.compute_crc32();
    println!(
        "CRC after changing resources: {:08x}",
        resource_modified_crc
    );

    assert_ne!(
        modified_crc, resource_modified_crc,
        "CRC must change when resources change"
    );

    println!("### CRC Change Test PASSED ###\n");
}

/// Test: CRC stays same when no changes
#[tokio::test]
async fn test_crc_stable_no_changes() {
    println!("\n### Testing CRC Stability Without Changes ###\n");

    let state = SimpleGameState::new(2);

    let crc1 = state.compute_crc32();
    let crc2 = state.compute_crc32();
    let crc3 = state.compute_crc32();

    println!("CRC1: {:08x}", crc1);
    println!("CRC2: {:08x}", crc2);
    println!("CRC3: {:08x}", crc3);

    assert_eq!(crc1, crc2, "CRC must be stable with no changes");
    assert_eq!(
        crc2, crc3,
        "CRC must be stable across multiple computations"
    );

    println!("### CRC Stability Test PASSED ###\n");
}

/// Test: Both players issuing commands every frame
#[tokio::test]
async fn test_high_command_frequency() {
    println!("\n### Testing High Command Frequency (Every Frame) ###\n");

    let mut player1 = SimpleGameState::new(2);
    let mut player2 = SimpleGameState::new(2);

    let mut total_commands = 0;

    for frame in 0..50 {
        // Force generate commands every frame (no random skip)
        let p1_cmd = GameCommandData {
            command_type: 4, // Gather (safe command)
            target_id: None,
            position: None,
            parameters: {
                let mut params = HashMap::new();
                params.insert("player_id".to_string(), CommandParameter::Int(0));
                params
            },
            checksum: 0,
        };

        let p2_cmd = GameCommandData {
            command_type: 4, // Gather
            target_id: None,
            position: None,
            parameters: {
                let mut params = HashMap::new();
                params.insert("player_id".to_string(), CommandParameter::Int(1));
                params
            },
            checksum: 0,
        };

        player1.execute_command(&p1_cmd).unwrap();
        player2.execute_command(&p1_cmd).unwrap();

        player1.execute_command(&p2_cmd).unwrap();
        player2.execute_command(&p2_cmd).unwrap();

        total_commands += 2;

        let p1_crc = player1.compute_crc32();
        let p2_crc = player2.compute_crc32();

        assert_eq!(
            p1_crc, p2_crc,
            "Frame {} CRC mismatch: {:08x} != {:08x}",
            frame, p1_crc, p2_crc
        );

        player1.advance_frame();
        player2.advance_frame();
    }

    println!("Executed {} commands across 50 frames", total_commands);
    println!(
        "Final P0 money: ${}",
        player1.resources.get(&0).unwrap().money
    );
    println!(
        "Final P1 money: ${}",
        player1.resources.get(&1).unwrap().money
    );

    assert_eq!(
        total_commands, 100,
        "Should execute 100 commands (2 per frame)"
    );

    println!("### High Command Frequency Test PASSED ###\n");
}

/// Test: Desync detection works
#[tokio::test]
async fn test_desync_detection() {
    println!("\n### Testing Desync Detection ###\n");

    let mut player1 = SimpleGameState::new(2);
    let mut player2 = SimpleGameState::new(2);

    // Run a few frames in sync
    for _ in 0..5 {
        player1.advance_frame();
        player2.advance_frame();
    }

    let crc1 = player1.compute_crc32();
    let crc2 = player2.compute_crc32();
    assert_eq!(crc1, crc2, "Should be in sync after 5 frames");

    // Intentionally desync player2 by modifying state
    player2.resources.get_mut(&0).unwrap().money = 9999;

    let desynced_crc1 = player1.compute_crc32();
    let desynced_crc2 = player2.compute_crc32();

    println!("Player 1 CRC: {:08x}", desynced_crc1);
    println!("Player 2 CRC (desynced): {:08x}", desynced_crc2);

    assert_ne!(
        desynced_crc1, desynced_crc2,
        "CRCs should differ after intentional desync"
    );

    // Test CRC validator detection
    let mut crc_validator = MultiPlayerCRCValidator::new(vec![0u8, 1u8]);
    crc_validator.add_crc(5, 0, desynced_crc1);
    crc_validator.add_crc(5, 1, desynced_crc2);

    let status = crc_validator.validate_frame(5).unwrap();

    match status {
        DesyncStatus::Desynchronized {
            frame,
            desynced_players,
        } => {
            println!(
                "Desync detected at frame {} for players: {:?}",
                frame, desynced_players
            );
            assert_eq!(frame, 5);
            // Either player 0 or 1 could be detected as desynced depending on reference CRC
            assert!(
                !desynced_players.is_empty(),
                "Should have at least one desynced player"
            );
        }
        DesyncStatus::Synchronized => {
            panic!("CRC validator failed to detect desync!");
        }
    }

    println!("### Desync Detection Test PASSED ###\n");
}

/// Test: Determinism across multiple runs
#[tokio::test]
async fn test_determinism_multiple_runs() {
    println!("\n### Testing Determinism Across Multiple Runs ###\n");

    let run_simulation = || {
        let mut state = SimpleGameState::new(2);
        let mut cmd_gen = CommandGenerator::new(0, 12345); // Same seed

        for _ in 0..20 {
            if let Some(cmd) = cmd_gen.generate_random_command(&state) {
                state.execute_command(&cmd).unwrap();
            }
            state.advance_frame();
        }

        state.compute_crc32()
    };

    let crc_run1 = run_simulation();
    let crc_run2 = run_simulation();
    let crc_run3 = run_simulation();

    println!("Run 1 CRC: {:08x}", crc_run1);
    println!("Run 2 CRC: {:08x}", crc_run2);
    println!("Run 3 CRC: {:08x}", crc_run3);

    assert_eq!(
        crc_run1, crc_run2,
        "Runs with same seed must produce identical CRCs"
    );
    assert_eq!(
        crc_run2, crc_run3,
        "Determinism must hold across multiple runs"
    );

    println!("### Determinism Test PASSED ###\n");
}
