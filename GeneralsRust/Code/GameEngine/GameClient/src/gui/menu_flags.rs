//! Shared menu flags used by multiple shell/menu callbacks.

use std::cell::Cell;

// THREAD: main thread only — shell menu latches read and written by GUI-thread
// callbacks, so plain cells replace the lock-wrapped static.
thread_local! {
    static DONT_SHOW_MAIN_MENU: Cell<bool> = Cell::new(false);
    static REPLAY_WAS_PRESSED: Cell<bool> = Cell::new(false);
}

pub fn get_dont_show_main_menu() -> bool {
    DONT_SHOW_MAIN_MENU.get()
}

pub fn set_dont_show_main_menu(value: bool) {
    DONT_SHOW_MAIN_MENU.set(value);
}

pub fn get_replay_was_pressed() -> bool {
    REPLAY_WAS_PRESSED.get()
}

pub fn set_replay_was_pressed(value: bool) {
    REPLAY_WAS_PRESSED.set(value);
}
