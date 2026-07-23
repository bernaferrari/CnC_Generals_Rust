#![cfg(test)]

use crate::command_system::{
    CommandResult, CommandSystem, CommandType, GameCommand, GuardTarget, ModifierKeys,
};
use crate::game_logic::{GameLogic, ObjectId};
use glam::Vec3;
use std::time::SystemTime;

#[test]
fn queue_and_process_invalid_command_returns_invalid_result() {
    let mut command_system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    command_system.queue_command(GameCommand {
        command_type: CommandType::Invalid,
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(1)],
        modifier_keys: ModifierKeys::default(),
    });

    let results = command_system.process_commands(&mut game_logic);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], CommandResult::InvalidCommand);
}

#[test]
fn queue_immediate_move_command_processes_successfully_with_empty_selection() {
    let mut command_system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    command_system.queue_immediate_command(
        CommandType::Move {
            destination: Vec3::new(200.0, 0.0, 240.0),
        },
        &[],
        0,
        ModifierKeys::default(),
    );

    let results = command_system.process_commands(&mut game_logic);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], CommandResult::Success);
}

#[test]
fn guard_command_payload_round_trips_through_game_command() {
    let target_position = Vec3::new(512.0, 0.0, 1024.0);
    let command = GameCommand {
        command_type: CommandType::Guard {
            target: GuardTarget::Position(target_position),
            mode: crate::game_logic::GuardMode::Normal,
        },
        player_id: 3,
        command_id: 42,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(11), ObjectId(12)],
        modifier_keys: ModifierKeys {
            shift: true,
            ctrl: false,
            alt: true,
        },
    };

    match &command.command_type {
        CommandType::Guard {
            target: GuardTarget::Position(position),
            mode: crate::game_logic::GuardMode::Normal,
        } => assert_eq!(*position, target_position),
        other => panic!("expected guard command, got {other:?}"),
    }

    assert_eq!(command.player_id, 3);
    assert_eq!(command.selected_units, vec![ObjectId(11), ObjectId(12)]);
    assert!(command.modifier_keys.shift);
    assert!(command.modifier_keys.alt);
    assert!(!command.modifier_keys.ctrl);
}
