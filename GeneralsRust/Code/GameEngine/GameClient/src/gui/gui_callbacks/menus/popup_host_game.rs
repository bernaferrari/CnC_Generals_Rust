//! Shim for PopupHostGame.cpp callbacks.

use crate::gui::callbacks::popup_host_game::{
    popup_host_game_init, popup_host_game_input, popup_host_game_system, popup_host_game_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn PopupHostGameInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    popup_host_game_init(layout, user_data);
}

pub fn PopupHostGameUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    popup_host_game_update(layout, user_data);
}

pub fn PopupHostGameSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_host_game_system(window, msg, data1, data2)
}

pub fn PopupHostGameInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_host_game_input(window, msg, data1, data2)
}
