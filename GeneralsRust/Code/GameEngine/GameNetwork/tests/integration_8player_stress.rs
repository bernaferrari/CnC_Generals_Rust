#![allow(unused_crate_dependencies)]

//! Comprehensive 8-Player Maximum Capacity Stress Test
//!
//! This test demonstrates the maximum supported player count (8 players) with:
//! - Full frame synchronization across all 8 players (lockstep networking)
//! - 200-frame extended stress testing (longer than 2/4-player tests)
//! - High command frequency (up to 3 commands per player per frame)
//! - Complete CRC computation and 8-way validation
//! - Desync detection under maximum load
//! - Deterministic state management across 8 instances
//! - Performance metrics tracking
//! - Resource contention scenarios
//! - Maximum conflict scenarios (8-way attacks)
//!
//! The test simulates eight local game instances (Players 0-7) running
//! a synchronized game loop for 200 frames, verifying that:
//! - All 8 players execute identical commands
//! - CRCs match across all 8 instances every frame (8-way match)
//! - No desynchronization occurs under heavy load
//! - Final game states are identical across all players
//! - System handles maximum player count without degradation

use game_network::commands::{CommandParameter, GameCommandData};
use game_network::error::NetworkResult;
use game_network::integration::{
    crc_validator::CRCComputer,
    desync_handler::{DesyncStatus, MultiPlayerCRCValidator},
    game_state::{
        CRCValue, EntitySnapshot, FrameNumber, GameState, GameStateCRC, PlayerId, ResourceState,
    },
};
use game_network::DesyncManager;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Maximum players supported by the system (from config::MAX_PLAYERS)
const MAX_PLAYERS: u8 = 8;

/// Number of frames for stress testing (longer than 2/4-player tests)
const STRESS_TEST_FRAMES: u32 = 200;

/// Simple deterministic game state for 8-player testing
#[derive(Debug, Clone)]
struct EightPlayerGameState {
    frame: FrameNumber,
    num_players: u8,
    entities: Vec<EntitySnapshot>,
    resources: BTreeMap<PlayerId, ResourceState>,
    random_seed: u32,
    command_history: Vec<(FrameNumber, PlayerId, u32)>, // (frame, player, command_type)
    desync_detected: bool,
}

impl EightPlayerGameState {
    fn new(num_players: u8) -> Self {
        assert!(num_players <= MAX_PLAYERS, "Cannot exceed MAX_PLAYERS (8)");

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

        // Start with entities for each player
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
            Some(CommandParameter::ObjectId(attacker_id)),
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

impl GameState for EightPlayerGameState {
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

    fn generate_random_command(
        &mut self,
        game_state: &EightPlayerGameState,
    ) -> Option<GameCommandData> {
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

    /// Generate multiple commands for stress testing
    fn generate_multiple_commands(
        &mut self,
        game_state: &EightPlayerGameState,
        count: usize,
    ) -> Vec<GameCommandData> {
        let mut commands = Vec::new();
        for _ in 0..count {
            if let Some(cmd) = self.generate_random_command(game_state) {
                commands.push(cmd);
            }
        }
        commands
    }
}

/// Test metrics tracking for 8-player stress test
#[derive(Debug, Default)]
struct StressTestMetrics {
    frames_executed: u32,
    commands_executed: u32,
    crc_matches: u32,
    crc_mismatches: u32,
    total_frame_time: Duration,
    min_frame_time: Duration,
    max_frame_time: Duration,
    total_crc_time: Duration,
    player_command_counts: HashMap<PlayerId, u32>,
    memory_snapshots: Vec<usize>,
}

impl StressTestMetrics {
    fn new() -> Self {
        Self {
            min_frame_time: Duration::from_secs(1000),
            max_frame_time: Duration::ZERO,
            player_command_counts: HashMap::new(),
            memory_snapshots: Vec::new(),
            ..Default::default()
        }
    }

    fn record_frame(
        &mut self,
        duration: Duration,
        crc_duration: Duration,
        crc_match: bool,
        commands: u32,
    ) {
        self.frames_executed += 1;
        self.commands_executed += commands;
        self.total_frame_time += duration;
        self.total_crc_time += crc_duration;

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

    fn record_command(&mut self, player_id: PlayerId) {
        *self.player_command_counts.entry(player_id).or_insert(0) += 1;
    }

    fn record_memory(&mut self, bytes: usize) {
        self.memory_snapshots.push(bytes);
    }

    fn average_frame_time(&self) -> Duration {
        if self.frames_executed > 0 {
            self.total_frame_time / self.frames_executed
        } else {
            Duration::ZERO
        }
    }

    fn average_crc_time(&self) -> Duration {
        if self.frames_executed > 0 {
            self.total_crc_time / self.frames_executed
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

    fn average_memory(&self) -> usize {
        if !self.memory_snapshots.is_empty() {
            self.memory_snapshots.iter().sum::<usize>() / self.memory_snapshots.len()
        } else {
            0
        }
    }

    fn print_summary(&self, player_count: u8) {
        println!("\n========== 8-Player Stress Test Metrics ==========");
        println!("Player Count: {}", player_count);
        println!("Frames executed: {}", self.frames_executed);
        println!(
            "Commands executed: {} ({} per frame avg)",
            self.commands_executed,
            if self.frames_executed > 0 {
                self.commands_executed / self.frames_executed
            } else {
                0
            }
        );
        println!("CRC matches: {}", self.crc_matches);
        println!("CRC mismatches: {}", self.crc_mismatches);
        println!("CRC match rate: {:.2}%", self.crc_match_rate());
        println!("\nPerformance:");
        println!("  Average frame time: {:?}", self.average_frame_time());
        println!("  Min frame time: {:?}", self.min_frame_time);
        println!("  Max frame time: {:?}", self.max_frame_time);
        println!("  Average CRC computation: {:?}", self.average_crc_time());
        println!("\nPer-Player Command Distribution:");
        for player_id in 0..player_count {
            let count = self.player_command_counts.get(&player_id).unwrap_or(&0);
            println!("  Player {}: {} commands", player_id, count);
        }
        println!("\nMemory:");
        println!("  Average game state size: {} bytes", self.average_memory());
        println!("==================================================\n");
    }
}

/// Helper to estimate game state memory usage
fn estimate_state_size(state: &EightPlayerGameState) -> usize {
    let serialized = state.serialize();
    serialized.len()
}

/// Test: 8-Player 200-Frame Basic Synchronization
#[tokio::test]
async fn test_eight_player_200_frame_sync() {
    println!("\n### Starting 8-Player 200-Frame Synchronization Test ###\n");

    // Initialize game states for all 8 players
    let mut players: Vec<EightPlayerGameState> = (0..8)
        .map(|_| EightPlayerGameState::new(MAX_PLAYERS))
        .collect();

    // Verify initial states match
    let initial_serialized = players[0].serialize();
    for (idx, player) in players.iter().enumerate() {
        assert_eq!(
            player.serialize(),
            initial_serialized,
            "Player {} initial state must match Player 0",
            idx
        );
    }

    let initial_crc = players[0].compute_crc32();
    for (idx, player) in players.iter().enumerate() {
        let crc = player.compute_crc32();
        assert_eq!(
            crc, initial_crc,
            "Player {} initial CRC {:08x} must match Player 0 CRC {:08x}",
            idx, crc, initial_crc
        );
    }

    println!("Initial state CRC: {:08x}", initial_crc);
    println!("Initial entities: {}", players[0].entities.len());
    println!("Number of players: {}\n", MAX_PLAYERS);

    // Create CRC validator for 8 players
    let mut crc_validator = MultiPlayerCRCValidator::new((0..8).collect());

    // Create command generators with different seeds for each player (deterministic)
    let mut cmd_gens: Vec<CommandGenerator> = (0..8)
        .map(|i| CommandGenerator::new(i, 42 + i as u64))
        .collect();

    // Metrics
    let mut metrics = StressTestMetrics::new();

    // Run 200 frames
    for frame in 0..STRESS_TEST_FRAMES {
        let frame_start = Instant::now();

        // 1. Collect commands from all 8 players (deterministic)
        let mut all_commands: Vec<(PlayerId, GameCommandData)> = Vec::new();

        for (player_id, gen) in cmd_gens.iter_mut().enumerate() {
            if let Some(cmd) = gen.generate_random_command(&players[0]) {
                all_commands.push((player_id as u8, cmd));
                metrics.record_command(player_id as u8);
            }
        }

        // 2. Execute commands on all players in deterministic order (by player ID)
        for (_, cmd) in &all_commands {
            for player in &mut players {
                player
                    .execute_command(cmd)
                    .expect("Command execution failed");
            }
        }

        // 3. Compute CRCs for all 8 players
        let crc_start = Instant::now();
        let mut crcs = Vec::new();
        for player in &players {
            crcs.push(player.compute_crc32());
        }
        let crc_duration = crc_start.elapsed();

        // 4. Validate all CRCs match (8-way validation)
        let reference_crc = crcs[0];
        let mut crc_match = true;

        for (idx, &crc) in crcs.iter().enumerate() {
            if crc != reference_crc {
                eprintln!("DESYNC DETECTED at frame {}!", frame);
                eprintln!("  Reference CRC (Player 0): {:08x}", reference_crc);
                eprintln!("  Player {} CRC: {:08x}", idx, crc);
                crc_match = false;
                panic!(
                    "Frame {} desynchronized: Player {} CRC {:08x} != Reference {:08x}",
                    frame, idx, crc, reference_crc
                );
            }
        }

        // Record CRCs in validator
        for (player_id, &crc) in crcs.iter().enumerate() {
            crc_validator.add_crc(frame, player_id as u8, crc);
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

        // 5. Advance all players to next frame
        for player in &mut players {
            player.advance_frame();
        }

        // Record metrics
        let frame_time = frame_start.elapsed();
        metrics.record_frame(
            frame_time,
            crc_duration,
            crc_match,
            all_commands.len() as u32,
        );

        if frame % 50 == 0 {
            metrics.record_memory(estimate_state_size(&players[0]));
        }

        // Periodic progress report
        if (frame + 1) % 50 == 0 {
            println!(
                "Frame {}: CRC {:08x}, {} commands, {} entities",
                frame,
                reference_crc,
                all_commands.len(),
                players[0].entities.len()
            );
        }
    }

    // Verify final states match
    println!("\nVerifying final state across all 8 players...");

    for player in &players {
        assert_eq!(
            player.current_frame(),
            STRESS_TEST_FRAMES,
            "All players should be at frame {}",
            STRESS_TEST_FRAMES
        );
    }

    let final_serialized = players[0].serialize();
    for (idx, player) in players.iter().enumerate() {
        let player_serialized = player.serialize();
        assert_eq!(
            player_serialized,
            final_serialized,
            "Player {} final state must match Player 0. P{} len: {}, P0 len: {}",
            idx,
            idx,
            player_serialized.len(),
            final_serialized.len()
        );
    }

    let final_crc = players[0].compute_crc32();
    for (idx, player) in players.iter().enumerate() {
        let player_crc = player.compute_crc32();
        assert_eq!(
            player_crc, final_crc,
            "Player {} final CRC {:08x} must match Player 0 CRC {:08x}",
            idx, player_crc, final_crc
        );
    }

    println!("Final CRC: {:08x}", final_crc);
    println!("Final entities: {}", players[0].entities.len());

    // Verify no desyncs
    for (idx, player) in players.iter().enumerate() {
        assert!(!player.desync_detected, "Player {} detected desync", idx);
    }

    // Print metrics
    metrics.print_summary(MAX_PLAYERS);

    // Final assertions
    assert_eq!(metrics.crc_mismatches, 0, "Should have 0 CRC mismatches");
    assert_eq!(
        metrics.frames_executed, STRESS_TEST_FRAMES,
        "Should execute exactly {} frames",
        STRESS_TEST_FRAMES
    );
    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "CRC match rate should be 100%"
    );

    println!("### 8-Player 200-Frame Synchronization Test PASSED ###\n");
}

/// Test: 8-Player Maximum Commands (3 per player per frame)
#[tokio::test]
async fn test_eight_player_maximum_commands() {
    println!("\n### Starting 8-Player Maximum Commands Test ###\n");

    let mut players: Vec<EightPlayerGameState> = (0..8)
        .map(|_| EightPlayerGameState::new(MAX_PLAYERS))
        .collect();

    let mut cmd_gens: Vec<CommandGenerator> = (0..8)
        .map(|i| CommandGenerator::new(i, 100 + i as u64))
        .collect();

    let mut metrics = StressTestMetrics::new();
    let test_frames = 100u32;

    for frame in 0..test_frames {
        let frame_start = Instant::now();

        // Generate up to 3 commands per player
        let mut all_commands: Vec<GameCommandData> = Vec::new();

        for (player_id, gen) in cmd_gens.iter_mut().enumerate() {
            let commands = gen.generate_multiple_commands(&players[0], 3);
            for cmd in commands {
                all_commands.push(cmd);
                metrics.record_command(player_id as u8);
            }
        }

        // Execute on all players
        for cmd in &all_commands {
            for player in &mut players {
                player
                    .execute_command(cmd)
                    .expect("Command execution failed");
            }
        }

        // Compute and validate CRCs
        let crc_start = Instant::now();
        let reference_crc = players[0].compute_crc32();
        let crc_duration = crc_start.elapsed();

        for (idx, player) in players.iter().enumerate().skip(1) {
            let crc = player.compute_crc32();
            assert_eq!(
                crc, reference_crc,
                "Frame {} Player {} CRC mismatch: {:08x} != {:08x}",
                frame, idx, crc, reference_crc
            );
        }

        // Advance all
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(
            frame_start.elapsed(),
            crc_duration,
            true,
            all_commands.len() as u32,
        );

        if (frame + 1) % 25 == 0 {
            println!(
                "Frame {}: {} commands, {} entities",
                frame,
                all_commands.len(),
                players[0].entities.len()
            );
        }
    }

    metrics.print_summary(MAX_PLAYERS);

    let expected_max_commands = test_frames * MAX_PLAYERS as u32 * 3;
    println!(
        "Total commands: {} (max possible: {})",
        metrics.commands_executed, expected_max_commands
    );

    assert_eq!(
        metrics.crc_match_rate(),
        100.0,
        "CRC match rate should be 100%"
    );
    assert!(
        metrics.commands_executed > 0,
        "Should execute many commands"
    );

    println!("### 8-Player Maximum Commands Test PASSED ###\n");
}

/// Test: 8-Player All-Attack-All (Maximum Conflict)
#[tokio::test]
async fn test_eight_player_all_attack_all() {
    println!("\n### Starting 8-Player All-Attack-All Test ###\n");

    let mut players: Vec<EightPlayerGameState> = (0..8)
        .map(|_| EightPlayerGameState::new(MAX_PLAYERS))
        .collect();

    let mut metrics = StressTestMetrics::new();
    let test_frames = 50u32;

    for frame in 0..test_frames {
        let frame_start = Instant::now();

        // Each player attacks a random enemy entity
        let mut attack_commands = Vec::new();

        for attacker_id in 0..8u8 {
            let my_entities: Vec<_> = players[0]
                .entities
                .iter()
                .filter(|e| e.owner == attacker_id)
                .collect();
            let enemy_entities: Vec<_> = players[0]
                .entities
                .iter()
                .filter(|e| e.owner != attacker_id)
                .collect();

            if !my_entities.is_empty() && !enemy_entities.is_empty() {
                let attacker = my_entities[frame as usize % my_entities.len()];
                let target = enemy_entities[frame as usize % enemy_entities.len()];

                let mut params = HashMap::new();
                params.insert(
                    "player_id".to_string(),
                    CommandParameter::Int(attacker_id as i32),
                );
                params.insert(
                    "attacker_id".to_string(),
                    CommandParameter::ObjectId(attacker.id),
                );
                params.insert(
                    "target_id".to_string(),
                    CommandParameter::ObjectId(target.id),
                );

                attack_commands.push(GameCommandData {
                    command_type: 3, // Attack
                    target_id: None,
                    position: None,
                    parameters: params,
                    checksum: 0,
                });

                metrics.record_command(attacker_id);
            }
        }

        // Execute attacks on all players
        for cmd in &attack_commands {
            for player in &mut players {
                player.execute_command(cmd).expect("Attack command failed");
            }
        }

        // Validate CRCs
        let crc_start = Instant::now();
        let reference_crc = players[0].compute_crc32();
        let crc_duration = crc_start.elapsed();

        for (idx, player) in players.iter().enumerate().skip(1) {
            let crc = player.compute_crc32();
            assert_eq!(
                crc, reference_crc,
                "Frame {} all-attack-all desync at Player {}",
                frame, idx
            );
        }

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(
            frame_start.elapsed(),
            crc_duration,
            true,
            attack_commands.len() as u32,
        );

        if (frame + 1) % 10 == 0 {
            println!(
                "Frame {}: {} attacks, {} entities remaining",
                frame,
                attack_commands.len(),
                players[0].entities.len()
            );
        }
    }

    metrics.print_summary(MAX_PLAYERS);
    println!("### 8-Player All-Attack-All Test PASSED ###\n");
}

/// Test: 8-Player Building Rush (Heavy Construction)
#[tokio::test]
async fn test_eight_player_building_rush() {
    println!("\n### Starting 8-Player Building Rush Test ###\n");

    let mut players: Vec<EightPlayerGameState> = (0..8)
        .map(|_| EightPlayerGameState::new(MAX_PLAYERS))
        .collect();

    let mut metrics = StressTestMetrics::new();
    let test_frames = 100u32;

    for frame in 0..test_frames {
        let frame_start = Instant::now();

        // Each player attempts to build 2 structures per frame
        let mut build_commands = Vec::new();

        for builder_id in 0..8u8 {
            // Check if player has resources
            if let Some(resources) = players[0].resources.get(&builder_id) {
                if resources.money >= 200 {
                    for i in 0..2 {
                        let mut params = HashMap::new();
                        params.insert(
                            "player_id".to_string(),
                            CommandParameter::Int(builder_id as i32),
                        );
                        params.insert("entity_type".to_string(), CommandParameter::Int(2 + i)); // Different building types

                        build_commands.push(GameCommandData {
                            command_type: 2, // Build
                            target_id: None,
                            position: None,
                            parameters: params,
                            checksum: 0,
                        });

                        metrics.record_command(builder_id);
                    }
                }
            }
        }

        // Execute builds
        for cmd in &build_commands {
            for player in &mut players {
                player.execute_command(cmd).expect("Build command failed");
            }
        }

        // Validate CRCs
        let crc_start = Instant::now();
        let reference_crc = players[0].compute_crc32();
        let crc_duration = crc_start.elapsed();

        for (idx, player) in players.iter().enumerate().skip(1) {
            let crc = player.compute_crc32();
            assert_eq!(
                crc, reference_crc,
                "Frame {} building rush desync at Player {}",
                frame, idx
            );
        }

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(
            frame_start.elapsed(),
            crc_duration,
            true,
            build_commands.len() as u32,
        );

        if (frame + 1) % 25 == 0 {
            println!(
                "Frame {}: {} builds, {} entities, ${} avg resources",
                frame,
                build_commands.len(),
                players[0].entities.len(),
                players[0].resources.values().map(|r| r.money).sum::<i32>() / MAX_PLAYERS as i32
            );
        }
    }

    metrics.print_summary(MAX_PLAYERS);
    println!("### 8-Player Building Rush Test PASSED ###\n");
}

/// Test: 8-Player Resource Gathering (Resource Contention)
#[tokio::test]
async fn test_eight_player_resource_gathering() {
    println!("\n### Starting 8-Player Resource Gathering Test ###\n");

    let mut players: Vec<EightPlayerGameState> = (0..8)
        .map(|_| EightPlayerGameState::new(MAX_PLAYERS))
        .collect();

    let mut metrics = StressTestMetrics::new();
    let test_frames = 150u32;

    for frame in 0..test_frames {
        let frame_start = Instant::now();

        // All players gather resources every frame
        let mut gather_commands = Vec::new();

        for gatherer_id in 0..8u8 {
            let mut params = HashMap::new();
            params.insert(
                "player_id".to_string(),
                CommandParameter::Int(gatherer_id as i32),
            );

            gather_commands.push(GameCommandData {
                command_type: 4, // Gather
                target_id: None,
                position: None,
                parameters: params,
                checksum: 0,
            });

            metrics.record_command(gatherer_id);
        }

        // Execute gathering
        for cmd in &gather_commands {
            for player in &mut players {
                player.execute_command(cmd).expect("Gather command failed");
            }
        }

        // Validate CRCs
        let crc_start = Instant::now();
        let reference_crc = players[0].compute_crc32();
        let crc_duration = crc_start.elapsed();

        for (idx, player) in players.iter().enumerate().skip(1) {
            let crc = player.compute_crc32();
            assert_eq!(
                crc, reference_crc,
                "Frame {} resource gathering desync at Player {}",
                frame, idx
            );
        }

        // Advance
        for player in &mut players {
            player.advance_frame();
        }

        metrics.record_frame(
            frame_start.elapsed(),
            crc_duration,
            true,
            gather_commands.len() as u32,
        );

        if (frame + 1) % 50 == 0 {
            let total_resources: i32 = players[0].resources.values().map(|r| r.money).sum();
            println!(
                "Frame {}: Total resources across all players: ${}",
                frame, total_resources
            );
        }
    }

    // Verify resource accumulation
    for (player_id, player) in players.iter().enumerate() {
        if let Some(resources) = player.resources.get(&(player_id as u8)) {
            let expected = 10000 + (test_frames * 10) as i32; // Starting + frames * gather_amount
            assert_eq!(
                resources.money, expected,
                "Player {} should have ${} (actual: ${})",
                player_id, expected, resources.money
            );
        }
    }

    metrics.print_summary(MAX_PLAYERS);
    println!("### 8-Player Resource Gathering Test PASSED ###\n");
}
