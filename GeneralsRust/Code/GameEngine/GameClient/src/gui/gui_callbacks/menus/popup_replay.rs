#![allow(non_snake_case)]
//! Shim for PopupReplay.cpp callbacks.

use crate::gui::callbacks::popup_replay::{
    popup_replay_init, popup_replay_input, popup_replay_shutdown, popup_replay_system,
    popup_replay_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn PopupReplayInit(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    popup_replay_init(layout, user_data);
}

pub fn PopupReplayUpdate(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    popup_replay_update(layout, user_data);
}

pub fn PopupReplayShutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    popup_replay_shutdown(layout, user_data);
}

pub fn PopupReplaySystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_replay_system(window, msg, data1, data2)
}

pub fn PopupReplayInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_replay_input(window, msg, data1, data2)
}
