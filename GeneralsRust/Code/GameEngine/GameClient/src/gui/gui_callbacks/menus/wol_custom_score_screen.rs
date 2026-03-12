//! Shim for WOLCustomScoreScreen.cpp callbacks.

use crate::gui::callbacks::wol_custom_score_screen::{
    wol_custom_score_screen_init, wol_custom_score_screen_input, wol_custom_score_screen_shutdown,
    wol_custom_score_screen_system, wol_custom_score_screen_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLCustomScoreScreenInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_custom_score_screen_init(layout, user_data);
}

pub fn WOLCustomScoreScreenUpdate(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    wol_custom_score_screen_update(layout, user_data);
}

pub fn WOLCustomScoreScreenShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    wol_custom_score_screen_shutdown(layout, user_data);
}

pub fn WOLCustomScoreScreenSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_custom_score_screen_system(window, msg, data1, data2)
}

pub fn WOLCustomScoreScreenInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_custom_score_screen_input(window, msg, data1, data2)
}
