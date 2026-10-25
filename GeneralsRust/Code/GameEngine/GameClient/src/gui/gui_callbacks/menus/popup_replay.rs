#![allow(non_snake_case)]
//! Shim for PopupReplay.cpp callbacks.

use crate::gui::callbacks::popup_replay::{
    popup_replay_init, popup_replay_input, popup_replay_shutdown, popup_replay_system,
    popup_replay_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn PopupReplayInit(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.with_manager(|manager, effects| popup_replay_init(layout, user_data, manager, effects));
}

pub fn PopupReplayUpdate(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.with_manager(|manager, effects| popup_replay_update(layout, user_data, manager));
}

pub fn PopupReplayShutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    popup_replay_shutdown(layout, user_data);
}

pub fn PopupReplaySystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_manager(|manager, effects| {
        popup_replay_system(window, msg, data1, data2, manager, effects)
    })
}

pub fn PopupReplayInput(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    popup_replay_input(window, msg, data1, data2, effects)
}
