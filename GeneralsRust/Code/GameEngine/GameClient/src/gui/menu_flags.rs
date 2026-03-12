//! Shared menu flags used by multiple shell/menu callbacks.

use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct MenuFlags {
    dont_show_main_menu: bool,
    replay_was_pressed: bool,
}

static MENU_FLAGS: OnceLock<Mutex<MenuFlags>> = OnceLock::new();

fn flags() -> &'static Mutex<MenuFlags> {
    MENU_FLAGS.get_or_init(|| Mutex::new(MenuFlags::default()))
}

pub fn get_dont_show_main_menu() -> bool {
    flags()
        .lock()
        .map(|flags| flags.dont_show_main_menu)
        .unwrap_or(false)
}

pub fn set_dont_show_main_menu(value: bool) {
    if let Ok(mut flags) = flags().lock() {
        flags.dont_show_main_menu = value;
    }
}

pub fn get_replay_was_pressed() -> bool {
    flags()
        .lock()
        .map(|flags| flags.replay_was_pressed)
        .unwrap_or(false)
}

pub fn set_replay_was_pressed(value: bool) {
    if let Ok(mut flags) = flags().lock() {
        flags.replay_was_pressed = value;
    }
}
