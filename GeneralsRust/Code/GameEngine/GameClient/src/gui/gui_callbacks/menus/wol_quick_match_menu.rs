//! Shim for WOLQuickMatchMenu.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wol_quick_match_menu::{
    wol_quick_match_menu_init, wol_quick_match_menu_input, wol_quick_match_menu_shutdown,
    wol_quick_match_menu_system, wol_quick_match_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLQuickMatchMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_quick_match_menu_init(layout, user_data);
}

pub fn WOLQuickMatchMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_quick_match_menu_update(layout, user_data);
}

pub fn WOLQuickMatchMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_quick_match_menu_shutdown(layout, user_data);
}

pub fn WOLQuickMatchMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_quick_match_menu_system(window, msg, data1, data2)
}

pub fn WOLQuickMatchMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_quick_match_menu_input(window, msg, data1, data2)
}
