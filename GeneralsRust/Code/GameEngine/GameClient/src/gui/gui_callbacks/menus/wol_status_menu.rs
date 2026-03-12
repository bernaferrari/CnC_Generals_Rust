//! Shim for WOLStatusMenu.cpp callbacks.

use crate::gui::callbacks::wol_status_menu::{
    wol_status_menu_init, wol_status_menu_input, wol_status_menu_shutdown, wol_status_menu_system,
    wol_status_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLStatusMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_status_menu_init(layout, user_data);
}

pub fn WOLStatusMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_status_menu_update(layout, user_data);
}

pub fn WOLStatusMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_status_menu_shutdown(layout, user_data);
}

pub fn WOLStatusMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_status_menu_system(window, msg, data1, data2)
}

pub fn WOLStatusMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_status_menu_input(window, msg, data1, data2)
}
