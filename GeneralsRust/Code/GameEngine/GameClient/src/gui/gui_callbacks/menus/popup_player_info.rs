//! Shim for PopupPlayerInfo.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::popup_player_info::{
    popup_player_info_init, popup_player_info_input, popup_player_info_shutdown,
    popup_player_info_system, popup_player_info_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn GameSpyPlayerInfoOverlayInit(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    popup_player_info_init(layout, user_data);
}

pub fn GameSpyPlayerInfoOverlayUpdate(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
) {
    popup_player_info_update(layout, user_data);
}

pub fn GameSpyPlayerInfoOverlayShutdown(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
) {
    popup_player_info_shutdown(layout, user_data);
}

pub fn GameSpyPlayerInfoOverlaySystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    popup_player_info_system(window, msg, data1, data2, effects)
}

pub fn GameSpyPlayerInfoOverlayInput(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    popup_player_info_input(window, msg, data1, data2, effects)
}
