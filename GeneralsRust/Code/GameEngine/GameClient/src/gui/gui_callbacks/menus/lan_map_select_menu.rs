#![allow(non_snake_case)]
//! Shim for LanMapSelectMenu.cpp callbacks.

use crate::gui::callbacks::lan_map_select_menu::{
    lan_map_select_menu_init, lan_map_select_menu_input, lan_map_select_menu_shutdown,
    lan_map_select_menu_system, lan_map_select_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn LanMapSelectMenuInit(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    lan_map_select_menu_init(layout, user_data, effects);
}

pub fn LanMapSelectMenuUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    lan_map_select_menu_update(layout, user_data);
}

pub fn LanMapSelectMenuShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    lan_map_select_menu_shutdown(layout, user_data, effects);
}

pub fn LanMapSelectMenuSystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_manager(|manager, effects| {
        lan_map_select_menu_system(window, msg, data1, data2, manager, effects)
    })
}

pub fn LanMapSelectMenuInput(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    lan_map_select_menu_input(window, msg, data1, data2, effects)
}
