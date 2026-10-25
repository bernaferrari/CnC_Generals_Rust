#![allow(non_snake_case)]
//! Shim for SkirmishGameOptionsMenu.cpp callbacks.

use crate::gui::callbacks::skirmish_game_options_menu::{
    skirmish_game_options_menu_init, skirmish_game_options_menu_input,
    skirmish_game_options_menu_shutdown, skirmish_game_options_menu_system,
    skirmish_game_options_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn SkirmishGameOptionsMenuInit(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    skirmish_game_options_menu_init(layout, user_data, effects);
}

pub fn SkirmishGameOptionsMenuUpdate(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    skirmish_game_options_menu_update(layout, user_data, effects);
}

pub fn SkirmishGameOptionsMenuShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    skirmish_game_options_menu_shutdown(layout, user_data, effects);
}

pub fn SkirmishGameOptionsMenuSystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    skirmish_game_options_menu_system(window, msg, data1, data2, effects)
}

pub fn SkirmishGameOptionsMenuInput(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    skirmish_game_options_menu_input(window, msg, data1, data2, effects)
}
