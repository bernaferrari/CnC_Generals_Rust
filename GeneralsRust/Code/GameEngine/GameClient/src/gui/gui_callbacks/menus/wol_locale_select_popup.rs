//! Shim for WOLLocaleSelectPopup.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::wol_locale_select_popup::{
    wol_locale_select_init, wol_locale_select_input, wol_locale_select_shutdown,
    wol_locale_select_system, wol_locale_select_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLLocaleSelectInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_locale_select_init(layout, user_data);
}

pub fn WOLLocaleSelectUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_locale_select_update(layout, user_data);
}

pub fn WOLLocaleSelectShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_locale_select_shutdown(layout, user_data);
}

pub fn WOLLocaleSelectSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_locale_select_system(window, msg, data1, data2)
}

pub fn WOLLocaleSelectInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_locale_select_input(window, msg, data1, data2)
}
