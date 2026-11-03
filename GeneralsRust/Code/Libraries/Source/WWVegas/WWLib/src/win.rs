//! Windows platform globals and helpers (ported from WWLib win.h).

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND};

#[cfg(target_os = "windows")]
pub fn set_game_in_focus(runtime: &crate::process_state::WwLibRuntime, value: bool) {
    runtime.game_in_focus.store(value, Ordering::Relaxed);
}

#[cfg(target_os = "windows")]
pub fn is_game_in_focus(runtime: &crate::process_state::WwLibRuntime) -> bool {
    runtime.game_in_focus.load(Ordering::Relaxed)
}

#[cfg(target_os = "windows")]
pub fn print_win32_error(win32_error: u32) {
    // SAFETY: [Category 8 — FFI] trivial Win32 call reading the last-error slot; no memory effects.
    let last_error = unsafe { GetLastError().0 };
    if cfg!(debug_assertions) {
        eprintln!(
            "Win32 error: supplied=0x{:08X}, last=0x{:08X}",
            win32_error, last_error
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_game_in_focus(_runtime: &crate::process_state::WwLibRuntime, _value: bool) {}

#[cfg(not(target_os = "windows"))]
pub fn is_game_in_focus(_runtime: &crate::process_state::WwLibRuntime) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn print_win32_error(_win32_error: u32) {}
