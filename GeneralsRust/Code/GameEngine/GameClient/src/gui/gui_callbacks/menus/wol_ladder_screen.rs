//! Shim for WOLLadderScreen.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wol_ladder_screen::{
    wol_ladder_screen_init, wol_ladder_screen_input, wol_ladder_screen_shutdown,
    wol_ladder_screen_system, wol_ladder_screen_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLLadderScreenInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_ladder_screen_init(layout, user_data);
}

pub fn WOLLadderScreenUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_ladder_screen_update(layout, user_data);
}

pub fn WOLLadderScreenShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_ladder_screen_shutdown(layout, user_data);
}

pub fn WOLLadderScreenSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_ladder_screen_system(window, msg, data1, data2)
}

pub fn WOLLadderScreenInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_ladder_screen_input(window, msg, data1, data2)
}
