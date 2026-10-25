#![allow(non_snake_case)]
//! Shim for GameInfoWindow.cpp callbacks.

use crate::gui::callbacks::game_info_window::{game_info_window_init, game_info_window_system};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn GameInfoWindowInit(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.with_manager(|manager, effects| game_info_window_init(layout, user_data, manager));
}

pub fn GameInfoWindowSystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    game_info_window_system(window, msg, data1, data2)
}
