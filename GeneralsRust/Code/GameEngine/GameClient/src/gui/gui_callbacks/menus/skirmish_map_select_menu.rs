#![allow(non_snake_case)]
//! Shim for SkirmishMapSelectMenu.cpp callbacks.

use crate::gui::callbacks::skirmish_map_select_menu::{
    draw_map_preview, skirmish_map_select_menu_init, skirmish_map_select_menu_input,
    skirmish_map_select_menu_shutdown, skirmish_map_select_menu_system,
    skirmish_map_select_menu_update,
};
use crate::gui::{
    GameWindow, WindowInstanceData, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
};

pub fn SkirmishMapSelectMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    skirmish_map_select_menu_init(layout, user_data);
}

pub fn SkirmishMapSelectMenuUpdate(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    skirmish_map_select_menu_update(layout, user_data);
}

pub fn SkirmishMapSelectMenuShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    skirmish_map_select_menu_shutdown(layout, user_data);
}

pub fn SkirmishMapSelectMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    skirmish_map_select_menu_system(window, msg, data1, data2)
}

pub fn SkirmishMapSelectMenuInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    skirmish_map_select_menu_input(window, msg, data1, data2)
}

pub fn W3DDrawMapPreview(window: &GameWindow, inst: &WindowInstanceData) {
    draw_map_preview(window, inst);
}
