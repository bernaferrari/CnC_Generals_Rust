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

pub fn SkirmishMapSelectMenuInit(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    skirmish_map_select_menu_init(layout, user_data, effects);
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
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    skirmish_map_select_menu_shutdown(layout, user_data, effects);
}

pub fn SkirmishMapSelectMenuSystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_manager(|manager, effects| {
        skirmish_map_select_menu_system(window, msg, data1, data2, manager, effects)
    })
}

pub fn SkirmishMapSelectMenuInput(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    skirmish_map_select_menu_input(window, msg, data1, data2, effects)
}

pub fn W3DDrawMapPreview(
    window: &GameWindow,
    inst: &WindowInstanceData,
    draw_context: &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
) {
    draw_map_preview(window, inst, draw_context);
}
