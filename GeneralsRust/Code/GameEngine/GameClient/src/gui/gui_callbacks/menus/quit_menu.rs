#![allow(non_snake_case)]
//! Shim for QuitMenu.cpp callbacks.

use crate::gui::callbacks::quit_menu::{
    destroy_quit_menu, hide_quit_menu, quit_menu_system, toggle_quit_menu,
};
use crate::gui::{GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn QuitMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    quit_menu_system(window, msg, data1, data2)
}

pub fn ToggleQuitMenu() {
    toggle_quit_menu();
}

pub fn HideQuitMenu() {
    hide_quit_menu();
}

pub fn DestroyQuitMenu() {
    destroy_quit_menu();
}
