//! Shim for WOLWelcomeMenu.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wol_welcome_menu::{
    wol_welcome_menu_init, wol_welcome_menu_input, wol_welcome_menu_shutdown,
    wol_welcome_menu_system, wol_welcome_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLWelcomeMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_welcome_menu_init(layout, user_data);
}

pub fn WOLWelcomeMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_welcome_menu_update(layout, user_data);
}

pub fn WOLWelcomeMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_welcome_menu_shutdown(layout, user_data);
}

pub fn WOLWelcomeMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_welcome_menu_system(window, msg, data1, data2)
}

pub fn WOLWelcomeMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_welcome_menu_input(window, msg, data1, data2)
}
