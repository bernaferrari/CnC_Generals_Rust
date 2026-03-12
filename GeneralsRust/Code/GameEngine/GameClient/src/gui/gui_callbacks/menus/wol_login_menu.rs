//! Shim for WOLLoginMenu.cpp callbacks.

use crate::gui::callbacks::wol_login_menu::{
    wol_login_menu_init, wol_login_menu_input, wol_login_menu_shutdown, wol_login_menu_system,
    wol_login_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLLoginMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_login_menu_init(layout, user_data);
}

pub fn WOLLoginMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_login_menu_update(layout, user_data);
}

pub fn WOLLoginMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_login_menu_shutdown(layout, user_data);
}

pub fn WOLLoginMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_login_menu_system(window, msg, data1, data2)
}

pub fn WOLLoginMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_login_menu_input(window, msg, data1, data2)
}
