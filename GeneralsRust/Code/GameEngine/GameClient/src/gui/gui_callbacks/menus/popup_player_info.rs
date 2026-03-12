//! Shim for PopupPlayerInfo.cpp callbacks.

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
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_player_info_system(window, msg, data1, data2)
}

pub fn GameSpyPlayerInfoOverlayInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_player_info_input(window, msg, data1, data2)
}
