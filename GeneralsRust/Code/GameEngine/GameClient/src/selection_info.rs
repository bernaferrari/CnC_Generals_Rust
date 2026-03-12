//! Selection info helpers for UI/context command decisions.

use crate::helpers::TheInGameUI;
use crate::message_stream::player_state::get_local_player_id;
use game_engine::common::ini::ini_game_data::get_global_data;
use gamelogic::commands::selection::{
    get_selection_manager, SelectionContextOptions, SelectionInfo,
};
use gamelogic::common::{Coord3D, ObjectID};

fn build_context_options() -> SelectionContextOptions {
    let use_alternate_mouse = get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false);
    SelectionContextOptions {
        force_attack_mode: TheInGameUI::is_in_force_attack_mode(),
        force_move_mode: TheInGameUI::is_in_force_move_to_mode(),
        use_alternate_mouse,
        prefer_selection_mode: TheInGameUI::is_in_prefer_selection_mode(),
    }
}

/// Fetch selection info for the local player.
pub fn get_selection_info() -> Option<SelectionInfo> {
    let player_id = get_local_player_id();
    if player_id < 0 {
        return None;
    }
    let manager = get_selection_manager();
    let manager = manager.read().ok()?;
    let selection = manager.get_player_selection_ref(player_id)?;
    Some(selection.get_selection_info())
}

/// Compute selection info for a new selection set.
pub fn get_selection_info_for_new_selection(newly_selected: &[ObjectID]) -> Option<SelectionInfo> {
    let player_id = get_local_player_id();
    if player_id < 0 {
        return None;
    }
    let manager = get_selection_manager();
    let manager = manager.read().ok()?;
    let selection = manager.get_player_selection_ref(player_id)?;
    Some(selection.get_selection_info_for_new_selection(newly_selected))
}

/// Decide whether a context command should fire instead of selection change.
pub fn context_command_for_new_selection<E>(
    newly_selected: &[ObjectID],
    selection_is_point: bool,
    evaluate_context_command: E,
) -> bool
where
    E: Fn(ObjectID, Coord3D) -> bool,
{
    let player_id = get_local_player_id();
    if player_id < 0 {
        return false;
    }
    let manager = get_selection_manager();
    let manager = match manager.read() {
        Ok(manager) => manager,
        Err(_) => return false,
    };
    let Some(selection) = manager.get_player_selection_ref(player_id) else {
        return false;
    };
    let options = build_context_options();
    selection.context_command_for_new_selection(
        newly_selected,
        selection_is_point,
        options,
        evaluate_context_command,
    )
}
