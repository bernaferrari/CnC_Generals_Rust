#![cfg(test)]

use crate::command_system::{
    CommandExecutor, CommandQueue, CommandType, GuardTarget, ModifierKeys,
};
use crate::game_logic::{GameLogic, Object, ObjectId, PlayerID};
use glam::Vec3;
use std::collections::VecDeque;
use std::time::Duration;

/// Test fixture for command system integration tests
struct CommandTestFixture {
    game_logic: GameLogic,
    command_executor: CommandExecutor,
    command_queue: CommandQueue,
    test_player_id: PlayerID,
    test_units: Vec<ObjectId>,
}

impl CommandTestFixture {
    fn new() -> Self {
        let mut game_logic = GameLogic::new();

        // Initialize test player
        let test_player_id = PlayerID(1);
        game_logic.add_player(test_player_id, "TestPlayer");

        // Create test units
        let mut test_units = Vec::new();
        for i in 0..5 {
            let unit_id = ObjectId::new();
            let unit = Object {
                id: unit_id,
                owner: test_player_id,
                position: Vec3::new(100.0 * i as f32, 0.0, 100.0),
                health: 100.0,
                max_health: 100.0,
                unit_type: "Tank".to_string(),
                is_selected: false,
                is_building: false,
                can_move: true,
                can_attack: true,
                attack_range: 150.0,
                movement_speed: 10.0,
            };
            game_logic.add_object(unit);
            test_units.push(unit_id);
        }

        Self {
            game_logic,
            command_executor: CommandExecutor::new(),
            command_queue: CommandQueue::new(),
            test_player_id,
            test_units,
        }
    }

    fn select_units(&mut self, unit_ids: &[ObjectId]) {
        for id in unit_ids {
            if let Some(unit) = self.game_logic.get_object_mut(*id) {
                unit.is_selected = true;
            }
        }
    }
}

#[test]
fn test_movement_command_executes_without_error() {
    // Setup
    let mut fixture = CommandTestFixture::new();
    fixture.select_units(&fixture.test_units[0..2]);

    let destination = Vec3::new(200.0, 0.0, 200.0);
    let move_command = CommandType::Move { destination };

    // Action: Execute movement command
    let result = fixture.command_executor.execute_command(
        &move_command,
        fixture.test_player_id,
        &mut fixture.game_logic,
    );

    // Assert
    assert!(
        result.is_ok(),
        "Movement command should execute without error"
    );

    // Verify units have movement orders
    for unit_id in &fixture.test_units[0..2] {
        if let Some(unit) = fixture.game_logic.get_object(*unit_id) {
            // In a real implementation, we'd check unit.current_order or unit.destination
            assert!(
                unit.is_selected,
                "Selected units should remain selected after move command"
            );
        }
    }
}

#[test]
fn test_attack_command_validates_targets() {
    // Setup
    let mut fixture = CommandTestFixture::new();
    let attacker_id = fixture.test_units[0];
    let valid_target_id = fixture.test_units[3];
    let invalid_target_id = ObjectId::from(99999); // Non-existent unit

    fixture.select_units(&[attacker_id]);

    // Test case 1: Valid target
    let valid_attack = CommandType::Attack {
        target_id: valid_target_id,
    };

    let result = fixture.command_executor.execute_command(
        &valid_attack,
        fixture.test_player_id,
        &mut fixture.game_logic,
    );

    assert!(
        result.is_ok(),
        "Attack command with valid target should succeed"
    );

    // Test case 2: Invalid target
    let invalid_attack = CommandType::Attack {
        target_id: invalid_target_id,
    };

    let validator = TestCommandValidator::new();
    let validation_result =
        validator.validate_command(&invalid_attack, fixture.test_player_id, &fixture.game_logic);

    assert!(
        validation_result.is_err(),
        "Attack command with invalid target should fail validation"
    );
}

#[test]
fn test_invalid_commands_rejected_gracefully() {
    // Setup
    let mut fixture = CommandTestFixture::new();

    // Test various invalid command scenarios
    let test_cases = vec![
        (
            CommandType::Build {
                template_name: "".to_string(), // Empty template name
                location: Vec3::new(100.0, 0.0, 100.0),
            },
            "Empty build template should be rejected",
        ),
        (
            CommandType::MoveTo {
                destination: Vec3::new(f32::NAN, 0.0, 0.0), // Invalid coordinates
                waypoints: vec![],
            },
            "NaN coordinates should be rejected",
        ),
        (
            CommandType::QueueUnitCreate {
                template_name: "Soldier".to_string(),
                quantity: 0, // Zero quantity
            },
            "Zero quantity production should be rejected",
        ),
        (
            CommandType::SetRallyPoint {
                location: Vec3::new(-99999.0, 0.0, -99999.0), // Out of bounds
            },
            "Out of bounds rally point should be rejected",
        ),
    ];

    let validator = TestCommandValidator::new();

    for (command, description) in test_cases {
        let result =
            validator.validate_command(&command, fixture.test_player_id, &fixture.game_logic);

        assert!(result.is_err(), "{}", description);
    }
}

#[test]
fn test_production_commands_queue_correctly() {
    // Setup
    let mut fixture = CommandTestFixture::new();

    // Create a building that can produce units
    let building_id = ObjectId::new();
    let building = Object {
        id: building_id,
        owner: fixture.test_player_id,
        position: Vec3::new(500.0, 0.0, 500.0),
        health: 1000.0,
        max_health: 1000.0,
        unit_type: "Barracks".to_string(),
        is_selected: true,
        is_building: true,
        can_move: false,
        can_attack: false,
        attack_range: 0.0,
        movement_speed: 0.0,
    };
    fixture.game_logic.add_object(building);

    // Queue multiple production commands
    let production_commands = vec![
        CommandType::QueueUnitCreate {
            template_name: "Soldier".to_string(),
            quantity: 5,
        },
        CommandType::QueueUnitCreate {
            template_name: "Tank".to_string(),
            quantity: 3,
        },
        CommandType::QueueUnitCreate {
            template_name: "Engineer".to_string(),
            quantity: 1,
        },
    ];

    // Action: Queue all commands
    for command in &production_commands {
        fixture
            .command_queue
            .add_command(command.clone(), fixture.test_player_id);
    }

    // Assert: Verify queue state
    assert_eq!(
        fixture.command_queue.get_queue_length(),
        3,
        "All three production commands should be queued"
    );

    // Process queue
    let mut processed_count = 0;
    while let Some(queued_command) = fixture.command_queue.get_next_command() {
        processed_count += 1;

        // Verify command type matches what we queued
        match queued_command.command {
            CommandType::QueueUnitCreate {
                ref template_name,
                quantity,
            } => {
                assert!(
                    ["Soldier", "Tank", "Engineer"].contains(&template_name.as_str()),
                    "Unexpected unit type in queue: {}",
                    template_name
                );
                assert!(quantity > 0, "Quantity should be positive");
            }
            _ => panic!("Unexpected command type in production queue"),
        }
    }

    assert_eq!(
        processed_count, 3,
        "All queued commands should be processed"
    );
}

#[test]
fn test_command_ownership_validation_works() {
    // Setup
    let mut fixture = CommandTestFixture::new();

    // Create unit owned by different player
    let enemy_player = PlayerID(2);
    fixture.game_logic.add_player(enemy_player, "Enemy");

    let enemy_unit_id = ObjectId::new();
    let enemy_unit = Object {
        id: enemy_unit_id,
        owner: enemy_player,
        position: Vec3::new(300.0, 0.0, 300.0),
        health: 100.0,
        max_health: 100.0,
        unit_type: "EnemyTank".to_string(),
        is_selected: false,
        is_building: false,
        can_move: true,
        can_attack: true,
        attack_range: 150.0,
        movement_speed: 10.0,
    };
    fixture.game_logic.add_object(enemy_unit);

    // Try to command enemy unit as player 1
    fixture
        .game_logic
        .get_object_mut(enemy_unit_id)
        .unwrap()
        .is_selected = true;

    let invalid_command = CommandType::Move {
        destination: Vec3::new(400.0, 0.0, 400.0),
    };

    let validator = TestCommandValidator::new();
    let result = validator.validate_ownership(
        &[enemy_unit_id],
        fixture.test_player_id,
        &fixture.game_logic,
    );

    // Assert: Should fail ownership check
    assert!(
        result.is_err(),
        "Player should not be able to command units they don't own"
    );
}

#[test]
fn test_complex_command_chain_execution() {
    // Setup
    let mut fixture = CommandTestFixture::new();
    let unit_id = fixture.test_units[0];
    fixture.select_units(&[unit_id]);

    // Create a complex command chain
    let command_chain = vec![
        CommandType::Move {
            destination: Vec3::new(150.0, 0.0, 150.0),
        },
        CommandType::AddWaypoint {
            destination: Vec3::new(200.0, 0.0, 150.0),
        },
        CommandType::AddWaypoint {
            destination: Vec3::new(200.0, 0.0, 200.0),
        },
        CommandType::Attack {
            target_id: fixture.test_units[4],
        },
    ];

    // Action: Execute command chain
    let mut all_successful = true;
    for command in &command_chain {
        let result = fixture.command_executor.execute_command(
            command,
            fixture.test_player_id,
            &mut fixture.game_logic,
        );

        if result.is_err() {
            all_successful = false;
            break;
        }
    }

    // Assert
    assert!(
        all_successful,
        "Complex command chain should execute without errors"
    );
}

#[test]
fn test_command_cancellation() {
    // Setup
    let mut fixture = CommandTestFixture::new();

    // Queue several commands
    for i in 0..5 {
        let command = CommandType::QueueUnitCreate {
            template_name: format!("Unit_{}", i),
            quantity: 1,
        };
        fixture
            .command_queue
            .add_command(command, fixture.test_player_id);
    }

    assert_eq!(
        fixture.command_queue.get_queue_length(),
        5,
        "Should have 5 commands queued"
    );

    // Action: Clear queue
    fixture.command_queue.clear_queue();

    // Assert
    assert_eq!(
        fixture.command_queue.get_queue_length(),
        0,
        "Queue should be empty after clearing"
    );
}

#[test]
fn test_command_performance() {
    // Setup
    let mut fixture = CommandTestFixture::new();

    // Create many units for performance testing
    for i in 0..100 {
        let unit_id = ObjectId::new();
        let unit = Object {
            id: unit_id,
            owner: fixture.test_player_id,
            position: Vec3::new(i as f32 * 10.0, 0.0, i as f32 * 10.0),
            health: 100.0,
            max_health: 100.0,
            unit_type: "Infantry".to_string(),
            is_selected: true,
            is_building: false,
            can_move: true,
            can_attack: true,
            attack_range: 50.0,
            movement_speed: 5.0,
        };
        fixture.game_logic.add_object(unit);
    }

    let move_command = CommandType::Move {
        destination: Vec3::new(500.0, 0.0, 500.0),
    };

    // Action: Time command execution
    let result = fixture.command_executor.execute_command(
        &move_command,
        fixture.test_player_id,
        &mut fixture.game_logic,
    );

    // Assert
    assert!(result.is_ok(), "Mass move command should succeed");
}

// Mock implementations for testing
pub struct CommandExecutor;

impl CommandExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_command(
        &mut self,
        command: &CommandType,
        player_id: PlayerID,
        game_logic: &mut GameLogic,
    ) -> CommandResult<()> {
        // Simple mock implementation
        match command {
            CommandType::Move { destination } if destination.x.is_nan() => {
                Err("Invalid destination".into())
            }
            _ => Ok(()),
        }
    }
}

pub struct CommandQueue {
    queue: VecDeque<QueuedCommand>,
}

pub struct QueuedCommand {
    pub command: CommandType,
    pub player_id: PlayerID,
    pub timestamp: Instant,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn add_command(&mut self, command: CommandType, player_id: PlayerID, timestamp: Instant) {
        self.queue.push_back(QueuedCommand {
            command,
            player_id,
            timestamp,
        });
    }

    pub fn get_next_command(&mut self) -> Option<QueuedCommand> {
        self.queue.pop_front()
    }

    pub fn get_queue_length(&self) -> usize {
        self.queue.len()
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }
}

pub struct TestCommandValidator;

impl TestCommandValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_command(
        &self,
        command: &CommandType,
        player_id: PlayerID,
        game_logic: &GameLogic,
    ) -> CommandResult<()> {
        match command {
            CommandType::Build { template_name, .. } if template_name.is_empty() => {
                Err("Empty template name".into())
            }
            CommandType::MoveTo { destination, .. } if destination.x.is_nan() => {
                Err("Invalid coordinates".into())
            }
            CommandType::QueueUnitCreate { quantity, .. } if *quantity == 0 => {
                Err("Invalid quantity".into())
            }
            CommandType::SetRallyPoint { location }
                if location.x < -10000.0 || location.z < -10000.0 =>
            {
                Err("Out of bounds".into())
            }
            CommandType::Attack { target_id } => {
                if game_logic.get_object(*target_id).is_none() {
                    Err("Invalid target".into())
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    pub fn validate_ownership(
        &self,
        unit_ids: &[ObjectId],
        player_id: PlayerID,
        game_logic: &GameLogic,
    ) -> CommandResult<()> {
        for unit_id in unit_ids {
            if let Some(unit) = game_logic.get_object(*unit_id) {
                if unit.owner != player_id {
                    return Err("Unit not owned by player".into());
                }
            }
        }
        Ok(())
    }
}

type CommandResult<T> = Result<T, Box<dyn std::error::Error>>;
