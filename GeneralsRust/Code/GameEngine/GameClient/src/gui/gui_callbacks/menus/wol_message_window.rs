//! Shim for WOLMessageWindow.cpp callbacks.

use crate::gui::callbacks::wol_message_window::{
    wol_message_window_init, wol_message_window_input, wol_message_window_shutdown,
    wol_message_window_system, wol_message_window_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLMessageWindowInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_message_window_init(layout, user_data);
}

pub fn WOLMessageWindowUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_message_window_update(layout, user_data);
}

pub fn WOLMessageWindowShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_message_window_shutdown(layout, user_data);
}

pub fn WOLMessageWindowSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_message_window_system(window, msg, data1, data2)
}

pub fn WOLMessageWindowInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_message_window_input(window, msg, data1, data2)
}
