use generals_main::command_system::{CommandType, GameCommand, ModifierKeys, SpecialPowerType};
use generals_main::game_logic::{GameLogic, ObjectId};
use generals_main::save_load::snapshot::SnapshotBuilder;
use std::time::SystemTime;

fn queue_command(world: &mut GameLogic) {
    world.queue_command(GameCommand {
        command_type: CommandType::Sell {
            object_id: ObjectId(7),
        },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::UNIX_EPOCH,
        selected_units: vec![ObjectId(7)],
        modifier_keys: ModifierKeys::default(),
    });
}

fn triggered_sources(world: &GameLogic) -> Vec<u32> {
    SnapshotBuilder::new()
        .create_world_snapshot(world)
        .expect("capture receiving session")
        .persist_v18
        .script_engine_tail
        .expect("session has its own ScriptEngine")
        .triggered_special_powers
        .into_iter()
        .flatten()
        .map(|(_, source)| source)
        .collect()
}

#[test]
fn reset_clears_only_the_receiving_session_commands_and_script_events() {
    let mut first = GameLogic::new();
    let mut peer = GameLogic::new();
    queue_command(&mut first);
    queue_command(&mut peer);
    first.notify_script_engine_special_power_event(
        ObjectId(7),
        &SpecialPowerType::Airstrike,
        true,
        false,
    );
    peer.notify_script_engine_special_power_event(
        ObjectId(7),
        &SpecialPowerType::Artillery,
        true,
        false,
    );
    assert_eq!(triggered_sources(&first), vec![7]);
    assert_eq!(triggered_sources(&peer), vec![7]);

    first.reset();

    assert!(!first.has_pending_commands());
    assert!(triggered_sources(&first).is_empty());
    assert!(peer.has_pending_commands());
    assert_eq!(triggered_sources(&peer), vec![7]);
}
