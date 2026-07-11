#![allow(unused_crate_dependencies)]

//! Comprehensive 4-Player Local Multiplayer Integration Test
//!
//! This test suite demonstrates complete four-player game synchronization with:
//! - Full frame synchronization (lockstep networking) across 4 players
//! - Simultaneous command collection and execution from all players
//! - 4-way CRC computation and validation
//! - Desync detection and reporting across multiple players
//! - Deterministic state management for 4 concurrent players
//! - 100-frame synchronization test with all players
//! - Team-based scenarios (2v2)
//! - Free-for-all gameplay
//! - High-frequency command processing
//!
//! The test simulates four local game instances (Players 0, 1, 2, 3) running
//! a synchronized game loop for 100 frames, verifying that:
//! - All 4 players execute the same commands in identical order
//! - CRCs match across all 4 players every frame
//! - No desynchronization occurs across any player pair
//! - Final game states are identical across all instances

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

/// Simple deterministic game state for testing with 4 players
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

    /// Generate attack command targeting specific enemy player
    fn generate_attack_command(
        &mut self,
        game_state: &SimpleGameState,
        target_player: PlayerId,
    ) -> Option<GameCommandData> {
        let my_entities: Vec<_> = game_state
            .entities
            .iter()
            .filter(|e| e.owner == self.player_id)
            .collect();
        let enemy_entities: Vec<_> = game_state
            .entities
            .iter()
            .filter(|e| e.owner == target_player)
            .collect();

        if my_entities.is_empty() || enemy_entities.is_empty() {
            return None;
        }

        let attacker_idx = (self.gen_range(0, my_entities.len() as u32)) as usize;
        let target_idx = (self.gen_range(0, enemy_entities.len() as u32)) as usize;

        let attacker = my_entities[attacker_idx];
        let target = enemy_entities[target_idx];

        let mut parameters = HashMap::new();
        parameters.insert(
            "player_id".to_string(),
            CommandParameter::Int(self.player_id as i32),
        );
        parameters.insert(
            "attacker_id".to_string(),
            CommandParameter::ObjectId(attacker.id),
        );
        parameters.insert(
            "target_id".to_string(),
            CommandParameter::ObjectId(target.id),
        );

        Some(GameCommandData {
            command_type: 3, // Attack
            target_id: None,
            position: None,
            parameters,
            checksum: 0,
        })
    }
}

/// Test metrics tracking for 4-player games
#[derive(Debug, Default)]
struct TestMetrics {
    frames_executed: u32,
    commands_executed: u32,
    commands_per_player: [u32; 4],
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

    fn record_player_command(&mut self, player_id: PlayerId) {
        if (player_id as usize) < 4 {
            self.commands_per_player[player_id as usize] += 1;
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
        println!("\n=== 4-Player Test Metrics Summary ===");
        println!("Frames executed: {}", self.frames_executed);
        println!("Total commands executed: {}", self.commands_executed);
        println!("Commands per player:");
        for (i, count) in self.commands_per_player.iter().enumerate() {
            println!("  Player {}: {} commands", i, count);
        }
        println!("CRC matches: {}", self.crc_matches);
        println!("CRC mismatches: {}", self.crc_mismatches);
        println!("CRC match rate: {:.2}%", self.crc_match_rate());
        println!("Average frame time: {:?}", self.average_frame_time());
        println!("Min frame time: {:?}", self.min_frame_time);
        println!("Max frame time: {:?}", self.max_frame_time);
        println!("======================================\n");
    }
}

/// Main test: 100-frame 4-player synchronization
#[tokio::test]
async fn test_four_player_100_frame_sync() {
    println!("\n### Starting 4-Player 100-Frame Synchronization Test ###\n");

    // Initialize game states for all 4 players
    let mut players = vec![
        SimpleGameState::new(4), // Player 0
        SimpleGameState::new(4), // Player 1
        SimpleGameState::new(4), // Player 2
        SimpleGameState::new(4), // Player 3
    ];

    // Verify initial states match across all players
    let initial_serialized = players[0].serialize();
    for (i, player) in players.iter().enumerate() {
        assert_eq!(
            player.serialize(),
            initial_serialized,
            "Player {} initial state must match Player 0",
            i
        );
    }

    let initial_crc = players[0].compute_crc32();
    for (i, player) in players.iter().enumerate() {
        let player_crc = player.compute_crc32();
        assert_eq!(
            player_crc, initial_crc,
            "Player {} initial CRC {:08x} must match {:08x}",
            i, player_crc, initial_crc
        );
    }

    println!("Initial state CRC: {:08x}", initial_crc);
    println!(
        "Initial entities per player: {}",
        players[0].entities.len() / 4
    );
    println!("Initial resources:");
    for player_id in 0..4 {
        println!(
            "  Player {}: ${}",
            player_id,
            players[0].resources.get(&player_id).unwrap().money
        );
    }
    println!();

    // Create CRC validator for 4 players
    let mut crc_validator = MultiPlayerCRCValidator::new(vec![0u8, 1u8, 2u8, 3u8]);

    // Create command generators with different seeds for variety
    let mut cmd_generators = [
        CommandGenerator::new(0, 42),
        CommandGenerator::new(1, 43),
        CommandGenerator::new(2, 44),
        CommandGenerator::new(3, 45),
    ];

    // Metrics
    let mut metrics = TestMetrics::new();

    // Run 100 frames
    for frame in 0..100 {
        let frame_start = Instant::now();

        // 1. Collect commands from all 4 players (deterministic)
        let commands: Vec<_> = (0..4)
            .map(|pid| cmd_generators[pid as usize].generate_random_command(&players[0]))
            .collect();

        let commands_this_frame = commands.iter().filter(|c| c.is_some()).count() as u32;

        // 2. Execute commands on all 4 players in deterministic order (by player ID)
        for (player_id, cmd_opt) in commands.iter().enumerate() {
            if let Some(cmd) = cmd_opt {
                // Execute this command on all 4 game instances
                for player_state in &mut players {
                    player_state.execute_command(cmd).unwrap_or_else(|_| {
                        panic!(
                            "Frame {} Player {} command execution failed",
                            frame, player_id
                        )
                    });
                }
                metrics.record_player_command(player_id as u8);
            }
        }

        // 3. Compute CRC for all 4 players
        let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();

        // 4. Validate all CRCs match
        let all_match = crcs.windows(2).all(|w| w[0] == w[1]);

        if !all_match {
            eprintln!("DESYNC DETECTED at frame {}!", frame);
            for (i, crc) in crcs.iter().enumerate() {
                eprintln!(
                    "  Player {} CRC: {:08x}, entities: {}",
                    i,
                    crc,
                    players[i].entities.len()
                );
            }
            panic!("Frame {} desynchronized across 4 players", frame);
        }

        // Record in CRC validator
        for (player_id, crc) in crcs.iter().enumerate() {
            crc_validator.add_crc(frame, player_id as u8, *crc);
        }

        // Validate frame CRCs
        if let Some(status) = crc_validator.validate_frame(frame) {
            assert_eq!(
                status,
                DesyncStatus::Synchronized,
                "CRC validator detected mismatch at frame {}",
                frame
            );
        }

        // 5. Advance all 4 players to next frame
        for player in &mut players {
            player.advance_frame();
        }

        // Record metrics
        let frame_time = frame_start.elapsed();
        metrics.record_frame(frame_time, all_match, commands_this_frame);

        // Periodic progress report
        if (frame + 1) % 25 == 0 {
            println!(
                "Frame {}: CRC {:08x}, {} commands, entities: P0={} P1={} P2={} P3={}",
                frame,
                crcs[0],
                commands_this_frame,
                players[0].entities.len(),
                players[1].entities.len(),
                players[2].entities.len(),
                players[3].entities.len()
            );
        }
    }

    // Verify final states match
    println!("\nVerifying final state across all 4 players...");

    for (i, player) in players.iter().enumerate() {
        assert_eq!(
            player.current_frame(),
            100,
            "Player {} should be at frame 100",
            i
        );
    }

    let final_serialized = players[0].serialize();
    for (i, player) in players.iter().enumerate().skip(1) {
        assert_eq!(
            player.serialize(),
            final_serialized,
            "Player {} final state must match Player 0",
            i
        );
    }

    let final_crc = players[0].compute_crc32();
    for (i, player) in players.iter().enumerate() {
        let player_crc = player.compute_crc32();
        assert_eq!(
            player_crc, final_crc,
            "Player {} final CRC must match: {:08x} != {:08x}",
            i, player_crc, final_crc
        );
    }

    println!("Final CRC (all players): {:08x}", final_crc);
    println!("Final entities:");
    for (i, player) in players.iter().enumerate() {
        println!("  Player {}: {} entities", i, player.entities.len());
    }
    println!("Final resources:");
    for player_id in 0..4 {
        println!(
            "  Player {}: ${}",
            player_id,
            players[0].resources.get(&player_id).unwrap().money
        );
    }

    // Verify no desyncs
    for (i, player) in players.iter().enumerate() {
        assert!(!player.desync_detected, "Player {} detected desync", i);
    }

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

    println!("### 4-Player 100-Frame Synchronization Test PASSED ###\n");
}

/// Test: 4-player teams (2v2) with coordinated attacks
#[tokio::test]
async fn test_four_player_teams() {
    println!("\n### Testing 4-Player Teams (2v2 Scenario) ###\n");

    let mut players = vec![
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
    ];

    // Team A: Players 0 and 1
    // Team B: Players 2 and 3

    let mut cmd_generators = [
        CommandGenerator::new(0, 100),
        CommandGenerator::new(1, 101),
        CommandGenerator::new(2, 102),
        CommandGenerator::new(3, 103),
    ];

    let mut metrics = TestMetrics::new();

    // Run 60 frames with team-based attacks
    for frame in 0..60 {
        let frame_start = Instant::now();

        let mut commands = Vec::new();

        // Team A attacks Team B
        for attacker in 0..2 {
            let target = 2 + (frame % 2); // Alternate between players 2 and 3
            let cmd = if frame % 3 == 0 {
                cmd_generators[attacker].generate_attack_command(&players[0], target as u8)
            } else {
                cmd_generators[attacker].generate_random_command(&players[0])
            };
            commands.push(cmd);
        }

        // Team B attacks Team A
        for attacker in 2..4 {
            let target = frame % 2; // Alternate between players 0 and 1
            let cmd = if frame % 3 == 0 {
                cmd_generators[attacker].generate_attack_command(&players[0], target as u8)
            } else {
                cmd_generators[attacker].generate_random_command(&players[0])
            };
            commands.push(cmd);
        }

        let commands_this_frame = commands.iter().filter(|c| c.is_some()).count() as u32;

        // Execute on all players
        for (player_id, cmd_opt) in commands.iter().enumerate() {
            if let Some(cmd) = cmd_opt {
                for player_state in &mut players {
                    player_state.execute_command(cmd).unwrap();
                }
                metrics.record_player_command(player_id as u8);
            }
        }

        // Verify CRCs
        let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();
        let all_match = crcs.windows(2).all(|w| w[0] == w[1]);

        assert!(
            all_match,
            "Frame {} CRCs don't match in 2v2: {:08x?}",
            frame, crcs
        );

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(frame_start.elapsed(), all_match, commands_this_frame);

        if (frame + 1) % 20 == 0 {
            println!(
                "Frame {}: CRC {:08x}, Team A entities: P0={} P1={}, Team B entities: P2={} P3={}",
                frame,
                crcs[0],
                players[0].entities.iter().filter(|e| e.owner == 0).count(),
                players[0].entities.iter().filter(|e| e.owner == 1).count(),
                players[0].entities.iter().filter(|e| e.owner == 2).count(),
                players[0].entities.iter().filter(|e| e.owner == 3).count()
            );
        }
    }

    println!("\nFinal team composition:");
    println!("Team A:");
    println!(
        "  Player 0: {} entities",
        players[0].entities.iter().filter(|e| e.owner == 0).count()
    );
    println!(
        "  Player 1: {} entities",
        players[0].entities.iter().filter(|e| e.owner == 1).count()
    );
    println!("Team B:");
    println!(
        "  Player 2: {} entities",
        players[0].entities.iter().filter(|e| e.owner == 2).count()
    );
    println!(
        "  Player 3: {} entities",
        players[0].entities.iter().filter(|e| e.owner == 3).count()
    );

    metrics.print_summary();

    assert_eq!(
        metrics.crc_mismatches, 0,
        "2v2 should have 0 CRC mismatches"
    );
    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "2v2 CRC match rate should be 100%"
    );

    println!("### 4-Player Teams Test PASSED ###\n");
}

/// Test: 4-player free-for-all with asymmetric commands
#[tokio::test]
async fn test_four_player_free_for_all() {
    println!("\n### Testing 4-Player Free-For-All ###\n");

    let mut players = vec![
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
    ];

    let mut cmd_generators = [
        CommandGenerator::new(0, 200),
        CommandGenerator::new(1, 201),
        CommandGenerator::new(2, 202),
        CommandGenerator::new(3, 203),
    ];

    let mut metrics = TestMetrics::new();

    // Run 80 frames with everyone attacking everyone
    for frame in 0..80 {
        let frame_start = Instant::now();

        let mut commands = Vec::new();

        // Each player attacks a different random target
        for attacker in 0..4usize {
            // Pick a target different from self
            let possible_targets: Vec<u8> = (0..4).filter(|&t| t != attacker as u8).collect();
            let target = possible_targets[(frame as usize + attacker) % possible_targets.len()];

            let cmd = if frame % 4 == attacker as u32 {
                // Occasionally everyone attacks
                cmd_generators[attacker].generate_attack_command(&players[0], target)
            } else if frame % 5 == 0 {
                // Build phase
                Some(GameCommandData {
                    command_type: 2,
                    target_id: None,
                    position: None,
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert(
                            "player_id".to_string(),
                            CommandParameter::Int(attacker as i32),
                        );
                        params.insert("entity_type".to_string(), CommandParameter::Int(2));
                        params
                    },
                    checksum: 0,
                })
            } else {
                cmd_generators[attacker].generate_random_command(&players[0])
            };

            commands.push(cmd);
        }

        let commands_this_frame = commands.iter().filter(|c| c.is_some()).count() as u32;

        // Execute on all players
        for (player_id, cmd_opt) in commands.iter().enumerate() {
            if let Some(cmd) = cmd_opt {
                for player_state in &mut players {
                    player_state.execute_command(cmd).unwrap();
                }
                metrics.record_player_command(player_id as u8);
            }
        }

        // Verify CRCs
        let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();
        let all_match = crcs.windows(2).all(|w| w[0] == w[1]);

        assert!(
            all_match,
            "Frame {} CRCs don't match in FFA: {:08x?}",
            frame, crcs
        );

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(frame_start.elapsed(), all_match, commands_this_frame);

        if (frame + 1) % 20 == 0 {
            println!(
                "Frame {}: CRC {:08x}, entities: P0={} P1={} P2={} P3={}",
                frame,
                crcs[0],
                players[0].entities.iter().filter(|e| e.owner == 0).count(),
                players[0].entities.iter().filter(|e| e.owner == 1).count(),
                players[0].entities.iter().filter(|e| e.owner == 2).count(),
                players[0].entities.iter().filter(|e| e.owner == 3).count()
            );
        }
    }

    println!("\nFinal free-for-all standings:");
    for player_id in 0..4 {
        let entity_count = players[0]
            .entities
            .iter()
            .filter(|e| e.owner == player_id)
            .count();
        let money = players[0].resources.get(&player_id).unwrap().money;
        println!(
            "  Player {}: {} entities, ${}",
            player_id, entity_count, money
        );
    }

    metrics.print_summary();

    assert_eq!(
        metrics.crc_mismatches, 0,
        "FFA should have 0 CRC mismatches"
    );
    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "FFA CRC match rate should be 100%"
    );

    println!("### 4-Player Free-For-All Test PASSED ###\n");
}

/// Test: 4-player with asymmetric command patterns
#[tokio::test]
async fn test_four_player_asymmetric_commands() {
    println!("\n### Testing 4-Player Asymmetric Command Patterns ###\n");

    let mut players = vec![
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
    ];

    let mut cmd_generators = [
        CommandGenerator::new(0, 300),
        CommandGenerator::new(1, 301),
        CommandGenerator::new(2, 302),
        CommandGenerator::new(3, 303),
    ];

    let mut metrics = TestMetrics::new();

    // Each player has a different command pattern
    for frame in 0..60 {
        let frame_start = Instant::now();

        let mut commands = Vec::new();

        // Player 0: Aggressive (attack often)
        let cmd0 = if frame % 2 == 0 {
            cmd_generators[0]
                .generate_attack_command(&players[0], 1)
                .or_else(|| cmd_generators[0].generate_random_command(&players[0]))
        } else {
            cmd_generators[0].generate_random_command(&players[0])
        };
        commands.push(cmd0);

        // Player 1: Builder (build often)
        let cmd1 = if frame % 3 == 0 {
            Some(GameCommandData {
                command_type: 2,
                target_id: None,
                position: None,
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("player_id".to_string(), CommandParameter::Int(1));
                    params.insert("entity_type".to_string(), CommandParameter::Int(2));
                    params
                },
                checksum: 0,
            })
        } else {
            cmd_generators[1].generate_random_command(&players[0])
        };
        commands.push(cmd1);

        // Player 2: Resource gatherer (gather often)
        let cmd2 = if frame % 2 == 1 {
            Some(GameCommandData {
                command_type: 4,
                target_id: None,
                position: None,
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("player_id".to_string(), CommandParameter::Int(2));
                    params
                },
                checksum: 0,
            })
        } else {
            cmd_generators[2].generate_random_command(&players[0])
        };
        commands.push(cmd2);

        // Player 3: Balanced (random)
        let cmd3 = cmd_generators[3].generate_random_command(&players[0]);
        commands.push(cmd3);

        let commands_this_frame = commands.iter().filter(|c| c.is_some()).count() as u32;

        // Execute on all players
        for (player_id, cmd_opt) in commands.iter().enumerate() {
            if let Some(cmd) = cmd_opt {
                for player_state in &mut players {
                    player_state.execute_command(cmd).unwrap();
                }
                metrics.record_player_command(player_id as u8);
            }
        }

        // Verify CRCs
        let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();
        let all_match = crcs.windows(2).all(|w| w[0] == w[1]);

        assert!(
            all_match,
            "Frame {} CRCs don't match with asymmetric commands: {:08x?}",
            frame, crcs
        );

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(frame_start.elapsed(), all_match, commands_this_frame);
    }

    println!("\nFinal asymmetric results:");
    println!(
        "Player 0 (Aggressive): {} entities, ${} money",
        players[0].entities.iter().filter(|e| e.owner == 0).count(),
        players[0].resources.get(&0).unwrap().money
    );
    println!(
        "Player 1 (Builder): {} entities, ${} money",
        players[0].entities.iter().filter(|e| e.owner == 1).count(),
        players[0].resources.get(&1).unwrap().money
    );
    println!(
        "Player 2 (Gatherer): {} entities, ${} money",
        players[0].entities.iter().filter(|e| e.owner == 2).count(),
        players[0].resources.get(&2).unwrap().money
    );
    println!(
        "Player 3 (Balanced): {} entities, ${} money",
        players[0].entities.iter().filter(|e| e.owner == 3).count(),
        players[0].resources.get(&3).unwrap().money
    );

    metrics.print_summary();

    assert_eq!(
        metrics.crc_mismatches, 0,
        "Asymmetric should have 0 CRC mismatches"
    );
    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "Asymmetric CRC match rate should be 100%"
    );

    println!("### 4-Player Asymmetric Command Test PASSED ###\n");
}

/// Test: 4-player high-frequency commands (maximum throughput)
#[tokio::test]
async fn test_four_player_high_frequency() {
    println!("\n### Testing 4-Player High-Frequency Commands ###\n");

    let mut players = vec![
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
    ];

    let mut metrics = TestMetrics::new();

    // Every player issues a command every frame (100% command rate)
    for frame in 0..50 {
        let frame_start = Instant::now();

        // All 4 players issue gather commands (safe, fast)
        for player_id in 0..4 {
            let cmd = GameCommandData {
                command_type: 4, // Gather
                target_id: None,
                position: None,
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("player_id".to_string(), CommandParameter::Int(player_id));
                    params
                },
                checksum: 0,
            };

            // Execute on all game instances
            for player_state in &mut players {
                player_state.execute_command(&cmd).unwrap();
            }

            metrics.record_player_command(player_id as u8);
        }

        // Verify CRCs
        let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();
        let all_match = crcs.windows(2).all(|w| w[0] == w[1]);

        assert!(
            all_match,
            "Frame {} high-frequency CRCs don't match: {:08x?}",
            frame, crcs
        );

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(frame_start.elapsed(), all_match, 4);
    }

    println!("\nHigh-frequency results:");
    println!("Total frames: 50");
    println!("Commands per frame: 4 (one from each player)");
    println!("Total commands: {}", metrics.commands_executed);
    println!("Final resources (all players gathered every frame):");
    for player_id in 0..4 {
        println!(
            "  Player {}: ${}",
            player_id,
            players[0].resources.get(&player_id).unwrap().money
        );
    }

    metrics.print_summary();

    assert_eq!(
        metrics.commands_executed, 200,
        "Should execute 200 commands (4 players * 50 frames)"
    );
    assert_eq!(
        metrics.crc_mismatches, 0,
        "High-frequency should have 0 CRC mismatches"
    );
    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "High-frequency CRC match rate should be 100%"
    );

    // Verify resource gains are identical across all instances
    let expected_money = 10000 + (50 * 10); // Initial + (frames * gather_amount)
    for player_id in 0..4 {
        assert_eq!(
            players[0].resources.get(&player_id).unwrap().money,
            expected_money,
            "Player {} money should be {}",
            player_id,
            expected_money
        );
    }

    println!("### 4-Player High-Frequency Test PASSED ###\n");
}

/// Test: CRC validation across 4 players
#[tokio::test]
async fn test_four_player_crc_validation() {
    println!("\n### Testing 4-Player CRC Validation ###\n");

    let players = [
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
    ];

    // All should have identical CRCs
    let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();

    println!("Initial CRCs:");
    for (i, crc) in crcs.iter().enumerate() {
        println!("  Player {}: {:08x}", i, crc);
    }

    // Verify all match
    for (i, crc) in crcs.iter().enumerate() {
        assert_eq!(
            *crc, crcs[0],
            "Player {} CRC {:08x} doesn't match Player 0 CRC {:08x}",
            i, crc, crcs[0]
        );
    }

    // Test CRC validator
    let mut validator = MultiPlayerCRCValidator::new(vec![0, 1, 2, 3]);

    for (player_id, crc) in crcs.iter().enumerate() {
        validator.add_crc(0, player_id as u8, *crc);
    }

    let status = validator.validate_frame(0).unwrap();
    assert_eq!(
        status,
        DesyncStatus::Synchronized,
        "Initial frame should be synchronized"
    );

    println!("CRC validation: All 4 players synchronized");

    println!("### 4-Player CRC Validation Test PASSED ###\n");
}

/// Test: Resource distribution across 4 players
#[tokio::test]
async fn test_four_player_resource_distribution() {
    println!("\n### Testing 4-Player Resource Distribution ###\n");

    let mut players = vec![
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
        SimpleGameState::new(4),
    ];

    // Each player gathers different amounts
    for frame in 0..30 {
        // Player 0: 1 gather per frame
        // Player 1: 2 gathers per frame
        // Player 2: 3 gathers per frame
        // Player 3: 4 gathers per frame

        for player_id in 0..4 {
            let gathers = player_id + 1;

            for _ in 0..gathers {
                let cmd = GameCommandData {
                    command_type: 4,
                    target_id: None,
                    position: None,
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("player_id".to_string(), CommandParameter::Int(player_id));
                        params
                    },
                    checksum: 0,
                };

                for player_state in &mut players {
                    player_state.execute_command(&cmd).unwrap();
                }
            }
        }

        // Verify sync
        let crcs: Vec<_> = players.iter().map(|p| p.compute_crc32()).collect();
        assert!(
            crcs.windows(2).all(|w| w[0] == w[1]),
            "Frame {} resource distribution desync",
            frame
        );

        for player in &mut players {
            player.advance_frame();
        }
    }

    println!("\nFinal resource distribution (30 frames):");
    for player_id in 0..4u8 {
        let expected = 10000 + (30 * (player_id as i32 + 1) * 10);
        let actual = players[0].resources.get(&player_id).unwrap().money;
        println!(
            "  Player {}: ${} (expected: ${})",
            player_id, actual, expected
        );
        assert_eq!(
            actual, expected,
            "Player {} resource calculation incorrect",
            player_id
        );
    }

    // Verify all player instances have same state
    for i in 1..4 {
        for player_id in 0..4 {
            assert_eq!(
                players[i].resources.get(&player_id).unwrap().money,
                players[0].resources.get(&player_id).unwrap().money,
                "Player instance {} doesn't match instance 0 for player_id {}",
                i,
                player_id
            );
        }
    }

    println!("### 4-Player Resource Distribution Test PASSED ###\n");
}
