mod combat_targeting;
mod command_audio_build;
mod group_path;
pub(super) use super::CommandExecutor;

mod movement_formation;

fn dispatch_test_command(
    command_type: crate::command_system::CommandType,
    player_id: u32,
    selected: Vec<crate::game_logic::ObjectId>,
) -> crate::command_system::GameCommand {
    crate::command_system::GameCommand {
        command_type,
        player_id,
        command_id: 0,
        timestamp: std::time::SystemTime::now(),
        selected_units: selected,
        modifier_keys: crate::command_system::ModifierKeys::default(),
    }
}
