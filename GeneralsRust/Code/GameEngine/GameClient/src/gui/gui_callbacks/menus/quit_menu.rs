#![allow(non_snake_case)]
//! Shim for QuitMenu.cpp callbacks.

use crate::gui::callbacks::quit_menu::{
    destroy_quit_menu, hide_quit_menu, quit_menu_system, toggle_quit_menu,
};
use crate::gui::{GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn QuitMenuSystem(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    quit_menu_system(window, msg, data1, data2, effects)
}

pub fn ToggleQuitMenu(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    toggle_quit_menu(effects, selected_window.as_deref_mut());
}

pub fn HideQuitMenu(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    effects
        .with_manager(|manager, effects| hide_quit_menu(manager, selected_window.as_deref_mut()));
}

pub fn DestroyQuitMenu(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    destroy_quit_menu(effects);
}
