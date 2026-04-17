//! Shim for WOLQMScoreScreen.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wolqm_score_screen::{
    wol_qm_score_screen_init, wol_qm_score_screen_input, wol_qm_score_screen_shutdown,
    wol_qm_score_screen_system, wol_qm_score_screen_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLQMScoreScreenInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_qm_score_screen_init(layout, user_data);
}

pub fn WOLQMScoreScreenUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_qm_score_screen_update(layout, user_data);
}

pub fn WOLQMScoreScreenShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_qm_score_screen_shutdown(layout, user_data);
}

pub fn WOLQMScoreScreenSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_qm_score_screen_system(window, msg, data1, data2)
}

pub fn WOLQMScoreScreenInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_qm_score_screen_input(window, msg, data1, data2)
}
