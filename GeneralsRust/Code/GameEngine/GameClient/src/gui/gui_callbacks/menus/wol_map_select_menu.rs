//! Shim for WOLMapSelectMenu.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wol_map_select_menu::{
    wol_map_select_menu_init, wol_map_select_menu_input, wol_map_select_menu_shutdown,
    wol_map_select_menu_system, wol_map_select_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLMapSelectMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_map_select_menu_init(layout, user_data);
}

pub fn WOLMapSelectMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_map_select_menu_update(layout, user_data);
}

pub fn WOLMapSelectMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_map_select_menu_shutdown(layout, user_data);
}

pub fn WOLMapSelectMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_map_select_menu_system(window, msg, data1, data2)
}

pub fn WOLMapSelectMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_map_select_menu_input(window, msg, data1, data2)
}
