//! Shim for WOLGameSetupMenu.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wol_game_setup_menu::{
    wol_game_setup_menu_init, wol_game_setup_menu_input, wol_game_setup_menu_shutdown,
    wol_game_setup_menu_system, wol_game_setup_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLGameSetupMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_game_setup_menu_init(layout, user_data);
}

pub fn WOLGameSetupMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_game_setup_menu_update(layout, user_data);
}

pub fn WOLGameSetupMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_game_setup_menu_shutdown(layout, user_data);
}

pub fn WOLGameSetupMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_game_setup_menu_system(window, msg, data1, data2)
}

pub fn WOLGameSetupMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_game_setup_menu_input(window, msg, data1, data2)
}
