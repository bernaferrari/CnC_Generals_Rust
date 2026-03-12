//! Shim for DownloadMenu.cpp callbacks.

use crate::gui::callbacks::download_menu::{
    download_menu_init, download_menu_input, download_menu_shutdown, download_menu_system,
    download_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn DownloadMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    download_menu_init(layout, user_data);
}

pub fn DownloadMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    download_menu_update(layout, user_data);
}

pub fn DownloadMenuShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    download_menu_shutdown(layout, user_data);
}

pub fn DownloadMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    download_menu_system(window, msg, data1, data2)
}

pub fn DownloadMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    download_menu_input(window, msg, data1, data2)
}
