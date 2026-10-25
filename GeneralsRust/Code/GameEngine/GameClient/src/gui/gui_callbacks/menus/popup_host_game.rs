//! Shim for PopupHostGame.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::popup_host_game::{
    popup_host_game_init, popup_host_game_input, popup_host_game_system, popup_host_game_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn PopupHostGameInit(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    popup_host_game_init(layout, user_data);
}

pub fn PopupHostGameUpdate(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    popup_host_game_update(layout, user_data, effects);
}

pub fn PopupHostGameSystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_host_game_system(window, msg, data1, data2)
}

pub fn PopupHostGameInput(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    popup_host_game_input(window, msg, data1, data2, effects)
}
