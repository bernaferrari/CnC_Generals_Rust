#![allow(non_snake_case)]
//! Shim for GameInfoWindow.cpp callbacks.

use crate::gui::callbacks::game_info_window::{game_info_window_init, game_info_window_system};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn GameInfoWindowInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    game_info_window_init(layout, user_data);
}

pub fn GameInfoWindowSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    game_info_window_system(window, msg, data1, data2)
}
