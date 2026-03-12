//! Control-bar observer helpers.
//!
//! Ported from `ControlBarObserver.cpp`.

use super::control_bar::ControlBar;
use super::ControlBarContext;
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use gamelogic::control_bar::get_control_bar_bridge;

/// Populate observer-specific commands.
///
/// The legacy C++ observer UI primarily drives a player-list window; command buttons are sourced
/// from an observer command set when present.
pub(super) fn populate_observer_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(control_bar) = get_control_bar_bridge() else {
        return Ok(());
    };
    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    let observer_set_names = [
        "Observer",
        "OBSERVER",
        "MultiPlayerObserver",
        "MULTIPLAYEROBSERVER",
    ];

    for set_name in observer_set_names {
        let Some(set) = control_bar.find_command_set_by_name(set_name) else {
            continue;
        };

        for button in set.buttons.iter().flatten() {
            if let Some(common_button) = common_bar.find_command_button_resolved(button.get_name())
            {
                ControlBar::push_command_if_missing(
                    context,
                    ControlBar::command_from_definition(common_button),
                );
            } else {
                ControlBar::push_command_if_missing(
                    context,
                    ControlBar::command_from_logic_button(button),
                );
            }
        }
        break;
    }

    Ok(())
}
