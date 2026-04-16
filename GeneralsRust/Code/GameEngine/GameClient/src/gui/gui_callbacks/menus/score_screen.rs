#![allow(non_snake_case)]
//! Shim for ScoreScreen.cpp callbacks.

use crate::gui::callbacks::score_screen::{
    score_screen_init, score_screen_input, score_screen_shutdown, score_screen_system,
    score_screen_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn ScoreScreenInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    score_screen_init(layout, user_data);
}

pub fn ScoreScreenUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    score_screen_update(layout, user_data);
}

pub fn ScoreScreenShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    score_screen_shutdown(layout, user_data);
}

pub fn ScoreScreenSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    score_screen_system(window, msg, data1, data2)
}

pub fn ScoreScreenInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    score_screen_input(window, msg, data1, data2)
}
