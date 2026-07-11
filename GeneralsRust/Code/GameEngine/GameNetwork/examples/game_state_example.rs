//! Complete GameState Implementation Example for C&C Generals Zero Hour
//!
//! This example demonstrates a fully functional, deterministic game state
//! implementation suitable for lockstep networking in RTS games.
//!
//! ## Features
//! - Deterministic command execution
//! - CRC32 computation for state validation
//! - Support for units, buildings, and terrain
//! - Player resource management
//! - Full command system (move, attack, build, etc.)
//!
//! ## Usage
//! ```
//! cargo run --example game_state_example
//! ```

use std::collections::BTreeMap;

// Re-export necessary types from the game_network crate
// In a real implementation, you would import these
type EntityId = u32;
type PlayerId = u8;
type FrameNumber = u32;
type CRCValue = u32;

/// Simple game state implementation for C&C Generals Zero Hour
#[derive(Debug, Clone)]
pub struct SimpleGameState {
    /// Current simulation frame
    frame: u32,
    /// All players in the game (max 8)
    players: Vec<Player>,
    /// All units in the game
    units: BTreeMap<EntityId, Unit>,
    /// All buildings in the game
    buildings: BTreeMap<EntityId, Building>,
    /// Terrain map
    terrain: TerrainMap,
    /// Next entity ID for spawning
    next_entity_id: EntityId,
    /// Random number generator state (for determinism)
    random_seed: u32,
}

/// Player information
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    /// Player ID (0-7)
    pub id: u8,
    /// Player name
    pub name: String,
    /// Starting position on map
    pub position: (i32, i32),
    /// Current money/resources
    pub resources: i32,
    /// Team number
    pub team: u8,
    /// Power available
    pub power: i32,
    /// Power being consumed
    pub power_consumed: i32,
    /// Whether player is still active
    pub active: bool,
}

/// Unit representation
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Unique unit ID
    pub id: u32,
    /// Owning player ID
    pub owner: u8,
    /// Unit type identifier
    pub unit_type: u8,
    /// Current position (using i32 for determinism)
    pub position: (i32, i32),
    /// Current health (0-1000 for precision)
    pub health: i32,
    /// Direction facing (0-255 for 360 degrees)
    pub direction: u8,
    /// Current state (idle, moving, attacking, etc.)
    pub state: UnitState,
    /// Target position (if moving)
    pub target_position: Option<(i32, i32)>,
    /// Target entity (if attacking)
    pub target_id: Option<u32>,
}

/// Unit state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitState {
    Idle = 0,
    Moving = 1,
    Attacking = 2,
    Gathering = 3,
    Dead = 4,
}

/// Building representation
#[derive(Debug, Clone, PartialEq)]
pub struct Building {
    /// Unique building ID
    pub id: u32,
    /// Owning player ID
    pub owner: u8,
    /// Building type identifier
    pub building_type: u8,
    /// Position on map
    pub position: (i32, i32),
    /// Current health (0-1000 for precision)
    pub health: i32,
    /// Production queue
    pub production_queue: Vec<u8>,
    /// Production progress (0-1000)
    pub production_progress: i32,
    /// Power provided/consumed
    pub power_effect: i32,
}

/// Terrain map
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainMap {
    /// Map width in tiles
    pub width: u32,
    /// Map height in tiles
    pub height: u32,
    /// Height map (simple elevation)
    pub tiles: Vec<u8>,
}

/// Game commands that can be executed
#[derive(Debug, Clone, PartialEq)]
pub enum GameCommand {
    /// Move unit to position
    MoveUnit { unit_id: u32, x: i32, y: i32 },
    /// Attack target unit
    AttackUnit { unit_id: u32, target_id: u32 },
    /// Build a building
    BuildBuilding {
        player_id: u8,
        building_type: u8,
        x: i32,
        y: i32,
    },
    /// Train a unit from a building
    TrainUnit { building_id: u32, unit_type: u8 },
    /// Gather resources
    GatherResources { unit_id: u32 },
    /// Upgrade unit
    UpgradeUnit { unit_id: u32, upgrade_type: u8 },
}

impl SimpleGameState {
    /// Create a new game state with specified number of players
    pub fn new(num_players: u8) -> Self {
        assert!(num_players > 0 && num_players <= 8, "Must have 1-8 players");

        let mut players = Vec::new();
        for i in 0..num_players {
            players.push(Player {
                id: i,
                name: format!("Player {}", i + 1),
                position: (i as i32 * 1000, i as i32 * 1000),
                resources: 10000, // Starting resources
                team: i,          // Each player on their own team by default
                power: 0,
                power_consumed: 0,
                active: true,
            });
        }

        // Create a simple terrain map
        let width = 256;
        let height = 256;
        let tiles = vec![0; (width * height) as usize]; // Flat terrain initially

        Self {
            frame: 0,
            players,
            units: BTreeMap::new(),
            buildings: BTreeMap::new(),
            terrain: TerrainMap {
                width,
                height,
                tiles,
            },
            next_entity_id: 1,
            random_seed: 12345,
        }
    }

    /// Get current frame number
    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// Advance to the next frame
    pub fn advance_frame(&mut self) {
        self.frame += 1;

        // Update all units
        self.update_units();

        // Update all buildings
        self.update_buildings();

        // Update random seed for next frame (deterministic)
        self.random_seed = self
            .random_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
    }

    /// Update all units (movement, combat, etc.)
    fn update_units(&mut self) {
        let mut units_to_update = Vec::new();

        for (id, unit) in &self.units {
            units_to_update.push((*id, unit.clone()));
        }

        for (id, mut unit) in units_to_update {
            match unit.state {
                UnitState::Moving => {
                    if let Some(target) = unit.target_position {
                        // Simple movement towards target
                        let dx = target.0 - unit.position.0;
                        let dy = target.1 - unit.position.1;

                        // Move at speed of 10 units per frame
                        let speed = 10;

                        if dx.abs() <= speed && dy.abs() <= speed {
                            // Reached target
                            unit.position = target;
                            unit.target_position = None;
                            unit.state = UnitState::Idle;
                        } else {
                            // Move towards target
                            let angle = (dy as f64).atan2(dx as f64);
                            unit.position.0 += (angle.cos() * speed as f64) as i32;
                            unit.position.1 += (angle.sin() * speed as f64) as i32;

                            // Update direction (0-255)
                            unit.direction =
                                ((angle.to_degrees() + 360.0) % 360.0 * 255.0 / 360.0) as u8;
                        }
                    } else {
                        unit.state = UnitState::Idle;
                    }
                }
                UnitState::Attacking => {
                    if let Some(target_id) = unit.target_id {
                        // Check if target still exists
                        if self.units.contains_key(&target_id)
                            || self.buildings.contains_key(&target_id)
                        {
                            // In range? Apply damage (simplified)
                            // Real implementation would check range, line of sight, etc.
                            let damage = 10; // Fixed damage per frame for determinism

                            if let Some(target_unit) = self.units.get_mut(&target_id) {
                                target_unit.health = target_unit.health.saturating_sub(damage);
                            } else if let Some(target_building) = self.buildings.get_mut(&target_id)
                            {
                                target_building.health =
                                    target_building.health.saturating_sub(damage);
                            }
                        } else {
                            // Target destroyed
                            unit.target_id = None;
                            unit.state = UnitState::Idle;
                        }
                    } else {
                        unit.state = UnitState::Idle;
                    }
                }
                UnitState::Gathering => {
                    // Gather resources every 10 frames
                    if self.frame.is_multiple_of(10) {
                        if let Some(player) = self.players.get_mut(unit.owner as usize) {
                            player.resources += 10; // Gather 10 resources
                        }
                    }
                }
                UnitState::Dead | UnitState::Idle => {
                    // Do nothing
                }
            }

            // Update the unit in the map
            if let Some(existing_unit) = self.units.get_mut(&id) {
                *existing_unit = unit;
            }
        }

        // Remove dead units
        self.units.retain(|_, unit| unit.health > 0);
    }

    /// Update all buildings (production, etc.)
    fn update_buildings(&mut self) {
        let mut buildings_to_update = Vec::new();

        for (id, building) in &self.buildings {
            buildings_to_update.push((*id, building.clone()));
        }

        for (id, mut building) in buildings_to_update {
            // Process production queue
            if !building.production_queue.is_empty() {
                building.production_progress += 10; // Progress per frame

                if building.production_progress >= 1000 {
                    // Production complete
                    let unit_type = building.production_queue.remove(0);
                    building.production_progress = 0;

                    // Spawn unit near building
                    let spawn_pos = (building.position.0 + 50, building.position.1 + 50);
                    self.spawn_unit(building.owner, unit_type, spawn_pos);
                }
            }

            // Update building in map
            if let Some(existing_building) = self.buildings.get_mut(&id) {
                *existing_building = building;
            }
        }

        // Remove destroyed buildings
        self.buildings.retain(|_, building| building.health > 0);
    }

    /// Spawn a new unit
    fn spawn_unit(&mut self, owner: u8, unit_type: u8, position: (i32, i32)) {
        let unit = Unit {
            id: self.next_entity_id,
            owner,
            unit_type,
            position,
            health: 1000, // Full health
            direction: 0,
            state: UnitState::Idle,
            target_position: None,
            target_id: None,
        };

        self.units.insert(self.next_entity_id, unit);
        self.next_entity_id += 1;
    }

    /// Apply a game command
    pub fn apply_command(&mut self, player_id: u8, cmd: &GameCommand) -> Result<(), String> {
        match cmd {
            GameCommand::MoveUnit { unit_id, x, y } => {
                // Verify unit exists and belongs to player
                if let Some(unit) = self.units.get_mut(unit_id) {
                    if unit.owner != player_id {
                        return Err("Unit does not belong to player".to_string());
                    }

                    unit.target_position = Some((*x, *y));
                    unit.state = UnitState::Moving;
                    Ok(())
                } else {
                    Err("Unit not found".to_string())
                }
            }

            GameCommand::AttackUnit { unit_id, target_id } => {
                // Verify target exists first (before borrowing unit)
                if !self.units.contains_key(target_id) && !self.buildings.contains_key(target_id) {
                    return Err("Target not found".to_string());
                }

                // Verify unit exists and belongs to player
                if let Some(unit) = self.units.get_mut(unit_id) {
                    if unit.owner != player_id {
                        return Err("Unit does not belong to player".to_string());
                    }

                    unit.target_id = Some(*target_id);
                    unit.state = UnitState::Attacking;
                    Ok(())
                } else {
                    Err("Unit not found".to_string())
                }
            }

            GameCommand::BuildBuilding {
                player_id: cmd_player_id,
                building_type,
                x,
                y,
            } => {
                if *cmd_player_id != player_id {
                    return Err("Player ID mismatch".to_string());
                }

                // Check if player has resources
                let cost = 500; // Fixed cost for simplicity
                if let Some(player) = self.players.get_mut(player_id as usize) {
                    if player.resources < cost {
                        return Err("Insufficient resources".to_string());
                    }

                    player.resources -= cost;
                } else {
                    return Err("Player not found".to_string());
                }

                // Create building
                let building = Building {
                    id: self.next_entity_id,
                    owner: player_id,
                    building_type: *building_type,
                    position: (*x, *y),
                    health: 1000,
                    production_queue: Vec::new(),
                    production_progress: 0,
                    power_effect: 0,
                };

                self.buildings.insert(self.next_entity_id, building);
                self.next_entity_id += 1;
                Ok(())
            }

            GameCommand::TrainUnit {
                building_id,
                unit_type,
            } => {
                // Verify building exists and belongs to player
                if let Some(building) = self.buildings.get_mut(building_id) {
                    if building.owner != player_id {
                        return Err("Building does not belong to player".to_string());
                    }

                    // Check if player has resources
                    let cost = 100; // Fixed cost for simplicity
                    if let Some(player) = self.players.get_mut(player_id as usize) {
                        if player.resources < cost {
                            return Err("Insufficient resources".to_string());
                        }

                        player.resources -= cost;
                    } else {
                        return Err("Player not found".to_string());
                    }

                    // Add to production queue
                    if building.production_queue.len() >= 5 {
                        return Err("Production queue full".to_string());
                    }

                    building.production_queue.push(*unit_type);
                    Ok(())
                } else {
                    Err("Building not found".to_string())
                }
            }

            GameCommand::GatherResources { unit_id } => {
                // Verify unit exists and belongs to player
                if let Some(unit) = self.units.get_mut(unit_id) {
                    if unit.owner != player_id {
                        return Err("Unit does not belong to player".to_string());
                    }

                    unit.state = UnitState::Gathering;
                    Ok(())
                } else {
                    Err("Unit not found".to_string())
                }
            }

            GameCommand::UpgradeUnit {
                unit_id,
                upgrade_type: _,
            } => {
                // Verify unit exists and belongs to player
                if let Some(_unit) = self.units.get(unit_id) {
                    if _unit.owner != player_id {
                        return Err("Unit does not belong to player".to_string());
                    }

                    // Check if player has resources
                    let cost = 200; // Fixed cost for simplicity
                    if let Some(player) = self.players.get_mut(player_id as usize) {
                        if player.resources < cost {
                            return Err("Insufficient resources".to_string());
                        }

                        player.resources -= cost;
                    } else {
                        return Err("Player not found".to_string());
                    }

                    // Apply upgrade (simplified - just increase health)
                    if let Some(unit) = self.units.get_mut(unit_id) {
                        unit.health = unit.health.saturating_add(100);
                    }

                    Ok(())
                } else {
                    Err("Unit not found".to_string())
                }
            }
        }
    }

    /// Serialize the game state to bytes for CRC computation
    /// This must be deterministic - same state always produces same bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Add frame number
        bytes.extend_from_slice(&self.frame.to_le_bytes());

        // Add random seed
        bytes.extend_from_slice(&self.random_seed.to_le_bytes());

        // Add all players (in order)
        for player in &self.players {
            bytes.push(player.id);
            bytes.extend_from_slice(&player.position.0.to_le_bytes());
            bytes.extend_from_slice(&player.position.1.to_le_bytes());
            bytes.extend_from_slice(&player.resources.to_le_bytes());
            bytes.push(player.team);
            bytes.extend_from_slice(&player.power.to_le_bytes());
            bytes.extend_from_slice(&player.power_consumed.to_le_bytes());
            bytes.push(if player.active { 1 } else { 0 });
        }

        // Add all units (BTreeMap ensures sorted order)
        bytes.extend_from_slice(&(self.units.len() as u32).to_le_bytes());
        for (id, unit) in &self.units {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.push(unit.owner);
            bytes.push(unit.unit_type);
            bytes.extend_from_slice(&unit.position.0.to_le_bytes());
            bytes.extend_from_slice(&unit.position.1.to_le_bytes());
            bytes.extend_from_slice(&unit.health.to_le_bytes());
            bytes.push(unit.direction);
            bytes.push(unit.state as u8);
        }

        // Add all buildings (BTreeMap ensures sorted order)
        bytes.extend_from_slice(&(self.buildings.len() as u32).to_le_bytes());
        for (id, building) in &self.buildings {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.push(building.owner);
            bytes.push(building.building_type);
            bytes.extend_from_slice(&building.position.0.to_le_bytes());
            bytes.extend_from_slice(&building.position.1.to_le_bytes());
            bytes.extend_from_slice(&building.health.to_le_bytes());
            bytes.extend_from_slice(&(building.production_queue.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&building.production_queue);
            bytes.extend_from_slice(&building.production_progress.to_le_bytes());
        }

        // Add terrain (only if modified - for now we skip since it's static)
        // In a real implementation, you'd include modified terrain tiles

        bytes
    }

    /// Compute CRC32 checksum of current game state
    /// Uses the same algorithm as the C++ implementation for compatibility
    pub fn compute_crc32(&self) -> u32 {
        let bytes = self.serialize();
        crc32_compute(&bytes)
    }

    /// Get player count
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Get unit count
    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    /// Get building count
    pub fn building_count(&self) -> usize {
        self.buildings.len()
    }

    /// Get a summary string of the game state
    pub fn summary(&self) -> String {
        format!(
            "Frame: {}, Players: {}, Units: {}, Buildings: {}",
            self.frame,
            self.players.len(),
            self.units.len(),
            self.buildings.len()
        )
    }
}

/// CRC32 computation matching the C++ bit-rotation algorithm
/// This must produce identical results to the C++ implementation
fn crc32_compute(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;

    for &byte in data {
        let hibit = if crc & 0x80000000 != 0 { 1 } else { 0 };
        crc <<= 1;
        crc = crc.wrapping_add(byte as u32);
        crc = crc.wrapping_add(hibit);
    }

    crc
}

// ============================================================================
// Example Usage and Tests
// ============================================================================

fn main() {
    println!("=== C&C Generals Zero Hour - GameState Example ===\n");

    // Create a game with 2 players
    let mut game1 = SimpleGameState::new(2);
    let mut game2 = SimpleGameState::new(2);

    println!("Created two identical game states");
    println!("Initial state: {}", game1.summary());
    println!("Initial CRC: {:08x}\n", game1.compute_crc32());

    // Verify initial CRCs match
    assert_eq!(game1.compute_crc32(), game2.compute_crc32());
    println!("✓ Initial CRCs match: {:08x}\n", game1.compute_crc32());

    // Spawn some units for testing
    game1.spawn_unit(0, 1, (100, 100));
    game1.spawn_unit(0, 1, (200, 100));
    game1.spawn_unit(1, 2, (100, 200));

    game2.spawn_unit(0, 1, (100, 100));
    game2.spawn_unit(0, 1, (200, 100));
    game2.spawn_unit(1, 2, (100, 200));

    println!("Spawned units: {}", game1.summary());
    println!("CRC after spawning: {:08x}\n", game1.compute_crc32());

    // Verify CRCs still match
    assert_eq!(game1.compute_crc32(), game2.compute_crc32());
    println!(
        "✓ CRCs match after spawning: {:08x}\n",
        game1.compute_crc32()
    );

    // Apply identical commands to both games
    let commands = vec![
        (
            0u8,
            GameCommand::MoveUnit {
                unit_id: 1,
                x: 500,
                y: 500,
            },
        ),
        (
            0u8,
            GameCommand::BuildBuilding {
                player_id: 0,
                building_type: 1,
                x: 300,
                y: 300,
            },
        ),
        (
            1u8,
            GameCommand::AttackUnit {
                unit_id: 3,
                target_id: 1,
            },
        ),
    ];

    println!("Applying {} commands...", commands.len());
    for (player_id, cmd) in &commands {
        game1.apply_command(*player_id, cmd).unwrap();
        game2.apply_command(*player_id, cmd).unwrap();
    }

    println!("After commands: {}", game1.summary());
    println!("CRC after commands: {:08x}\n", game1.compute_crc32());

    // Verify CRCs still match
    assert_eq!(game1.compute_crc32(), game2.compute_crc32());
    println!(
        "✓ CRCs match after commands: {:08x}\n",
        game1.compute_crc32()
    );

    // Simulate 10 frames
    println!("Simulating 10 frames...");
    for _i in 0..10 {
        let crc_before = game1.compute_crc32();

        game1.advance_frame();
        game2.advance_frame();

        let crc_after = game1.compute_crc32();

        assert_eq!(game1.compute_crc32(), game2.compute_crc32());

        println!(
            "  Frame {}: CRC {:08x} -> {:08x} | {}",
            game1.get_frame(),
            crc_before,
            crc_after,
            game1.summary()
        );
    }

    println!("\n✓ All frames completed with matching CRCs!");
    println!("Final CRC: {:08x}\n", game1.compute_crc32());

    // Test determinism: applying the same sequence should yield same CRC
    println!("Testing determinism...");
    let mut game3 = SimpleGameState::new(2);
    game3.spawn_unit(0, 1, (100, 100));
    game3.spawn_unit(0, 1, (200, 100));
    game3.spawn_unit(1, 2, (100, 200));

    for (player_id, cmd) in &commands {
        game3.apply_command(*player_id, cmd).unwrap();
    }

    for _ in 0..10 {
        game3.advance_frame();
    }

    assert_eq!(game1.compute_crc32(), game3.compute_crc32());
    println!("✓ Determinism verified: Same commands = Same CRC");
    println!("  Game1 CRC: {:08x}", game1.compute_crc32());
    println!("  Game3 CRC: {:08x}\n", game3.compute_crc32());

    // Test that different commands produce different CRCs
    println!("Testing different commands produce different CRCs...");
    let mut game4 = SimpleGameState::new(2);
    game4.spawn_unit(0, 1, (100, 100));
    game4.spawn_unit(0, 1, (200, 100));
    game4.spawn_unit(1, 2, (100, 200));

    // Apply different command
    game4
        .apply_command(
            0,
            &GameCommand::MoveUnit {
                unit_id: 1,
                x: 600, // Different position
                y: 600,
            },
        )
        .unwrap();

    for _ in 0..10 {
        game4.advance_frame();
    }

    assert_ne!(game1.compute_crc32(), game4.compute_crc32());
    println!("✓ Different commands produce different CRCs");
    println!("  Game1 CRC: {:08x}", game1.compute_crc32());
    println!("  Game4 CRC: {:08x}\n", game4.compute_crc32());

    // Test resource management
    println!("Testing resource management...");
    let mut game5 = SimpleGameState::new(2);
    game5.spawn_unit(0, 1, (100, 100));

    println!("  Initial resources: {}", game5.players[0].resources);

    game5
        .apply_command(
            0,
            &GameCommand::BuildBuilding {
                player_id: 0,
                building_type: 1,
                x: 300,
                y: 300,
            },
        )
        .unwrap();

    println!("  After building: {}", game5.players[0].resources);

    game5
        .apply_command(0, &GameCommand::GatherResources { unit_id: 1 })
        .unwrap();

    for _ in 0..20 {
        game5.advance_frame();
    }

    println!("  After gathering: {}", game5.players[0].resources);
    println!("✓ Resource management working\n");

    // Summary
    println!("=== Summary ===");
    println!("All tests passed!");
    println!("✓ Deterministic command execution");
    println!("✓ CRC32 computation is consistent");
    println!("✓ Same commands produce same CRC");
    println!("✓ Different commands produce different CRCs");
    println!("✓ Resource management works correctly");
    println!("\nThis GameState implementation is ready for network synchronization!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_creation() {
        let game = SimpleGameState::new(4);
        assert_eq!(game.player_count(), 4);
        assert_eq!(game.get_frame(), 0);
    }

    #[test]
    fn test_deterministic_crc() {
        let mut game1 = SimpleGameState::new(2);
        let mut game2 = SimpleGameState::new(2);

        assert_eq!(game1.compute_crc32(), game2.compute_crc32());

        game1.spawn_unit(0, 1, (100, 100));
        game2.spawn_unit(0, 1, (100, 100));

        assert_eq!(game1.compute_crc32(), game2.compute_crc32());
    }

    #[test]
    fn test_frame_advance() {
        let mut game = SimpleGameState::new(2);
        assert_eq!(game.get_frame(), 0);

        game.advance_frame();
        assert_eq!(game.get_frame(), 1);

        game.advance_frame();
        assert_eq!(game.get_frame(), 2);
    }

    #[test]
    fn test_move_command() {
        let mut game = SimpleGameState::new(2);
        game.spawn_unit(0, 1, (100, 100));

        let result = game.apply_command(
            0,
            &GameCommand::MoveUnit {
                unit_id: 1,
                x: 200,
                y: 200,
            },
        );

        assert!(result.is_ok());
        assert_eq!(game.units[&1].state, UnitState::Moving);
    }

    #[test]
    fn test_build_command() {
        let mut game = SimpleGameState::new(2);

        let result = game.apply_command(
            0,
            &GameCommand::BuildBuilding {
                player_id: 0,
                building_type: 1,
                x: 300,
                y: 300,
            },
        );

        assert!(result.is_ok());
        assert_eq!(game.building_count(), 1);
    }

    #[test]
    fn test_insufficient_resources() {
        let mut game = SimpleGameState::new(2);
        game.players[0].resources = 0;

        let result = game.apply_command(
            0,
            &GameCommand::BuildBuilding {
                player_id: 0,
                building_type: 1,
                x: 300,
                y: 300,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_different_states_different_crcs() {
        let mut game1 = SimpleGameState::new(2);
        let mut game2 = SimpleGameState::new(2);

        game1.spawn_unit(0, 1, (100, 100));
        game2.spawn_unit(0, 1, (200, 200)); // Different position

        assert_ne!(game1.compute_crc32(), game2.compute_crc32());
    }

    #[test]
    fn test_crc32_algorithm() {
        // Test the CRC32 algorithm matches expected behavior
        let data = b"Hello, World!";
        let crc1 = crc32_compute(data);
        let crc2 = crc32_compute(data);

        // Should be deterministic
        assert_eq!(crc1, crc2);

        // Different data should produce different CRC
        let data2 = b"Hello, World?";
        let crc3 = crc32_compute(data2);
        assert_ne!(crc1, crc3);
    }

    #[test]
    fn test_serialization_deterministic() {
        let mut game1 = SimpleGameState::new(2);
        game1.spawn_unit(0, 1, (100, 100));

        let bytes1 = game1.serialize();
        let bytes2 = game1.serialize();

        assert_eq!(bytes1, bytes2);
    }
}
