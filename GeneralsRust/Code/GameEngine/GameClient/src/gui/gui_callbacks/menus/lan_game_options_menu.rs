#![allow(non_snake_case)]
//! Shim for LanGameOptionsMenu.cpp callbacks.

use crate::gui::callbacks::lan_game_options_menu::{
    lan_game_options_menu_init, lan_game_options_menu_input, lan_game_options_menu_shutdown,
    lan_game_options_menu_system, lan_game_options_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn LanGameOptionsMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    lan_game_options_menu_init(layout, user_data);
}

pub fn LanGameOptionsMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    lan_game_options_menu_update(layout, user_data);
}

pub fn LanGameOptionsMenuShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    lan_game_options_menu_shutdown(layout, user_data);
}

pub fn LanGameOptionsMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    lan_game_options_menu_system(window, msg, data1, data2)
}

pub fn LanGameOptionsMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    lan_game_options_menu_input(window, msg, data1, data2)
}
