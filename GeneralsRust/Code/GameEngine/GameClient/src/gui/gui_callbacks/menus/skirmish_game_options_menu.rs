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
) {
    skirmish_game_options_menu_init(layout, user_data);
}

pub fn SkirmishGameOptionsMenuUpdate(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    skirmish_game_options_menu_update(layout, user_data);
}

pub fn SkirmishGameOptionsMenuShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    skirmish_game_options_menu_shutdown(layout, user_data);
}

pub fn SkirmishGameOptionsMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    skirmish_game_options_menu_system(window, msg, data1, data2)
}

pub fn SkirmishGameOptionsMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    skirmish_game_options_menu_input(window, msg, data1, data2)
}
