//! Windows platform globals and helpers (ported from WWLib win.h).

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND};

#[cfg(target_os = "windows")]
pub static mut PROGRAM_INSTANCE: HINSTANCE = HINSTANCE(0);

#[cfg(target_os = "windows")]
pub static mut MAIN_WINDOW: HWND = HWND(0);

#[cfg(target_os = "windows")]
pub static GAME_IN_FOCUS: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
pub fn set_game_in_focus(value: bool) {
    GAME_IN_FOCUS.store(value, Ordering::Relaxed);
}

#[cfg(target_os = "windows")]
pub fn is_game_in_focus() -> bool {
    GAME_IN_FOCUS.load(Ordering::Relaxed)
}

#[cfg(target_os = "windows")]
pub fn print_win32_error(win32_error: u32) {
    let last_error = unsafe { GetLastError().0 };
    if cfg!(debug_assertions) {
        eprintln!(
            "Win32 error: supplied=0x{:08X}, last=0x{:08X}",
            win32_error, last_error
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_game_in_focus(_value: bool) {}

#[cfg(not(target_os = "windows"))]
pub fn is_game_in_focus() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn print_win32_error(_win32_error: u32) {}
