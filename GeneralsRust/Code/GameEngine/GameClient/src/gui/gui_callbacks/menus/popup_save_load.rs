//! Shim for PopupSaveLoad.cpp callbacks.

use crate::gui::callbacks::popup_save_load::{
    save_load_menu_full_screen_init, save_load_menu_init, save_load_menu_input,
    save_load_menu_shutdown, save_load_menu_system, save_load_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn SaveLoadMenuInit(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    save_load_menu_init(layout, user_data);
}

pub fn SaveLoadMenuFullScreenInit(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    save_load_menu_full_screen_init(layout, user_data);
}

pub fn SaveLoadMenuUpdate(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    save_load_menu_update(layout, user_data);
}

pub fn SaveLoadMenuShutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    save_load_menu_shutdown(layout, user_data);
}

pub fn SaveLoadMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    save_load_menu_system(window, msg, data1, data2)
}

pub fn SaveLoadMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    save_load_menu_input(window, msg, data1, data2)
}
