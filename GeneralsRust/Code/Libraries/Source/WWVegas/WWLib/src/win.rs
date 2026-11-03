//! Windows platform globals and helpers (ported from WWLib win.h).

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND};

#[cfg(target_os = "windows")]
struct ProgramInstanceSlot(AtomicIsize);
impl std::ops::Deref for ProgramInstanceSlot {
    type Target = AtomicIsize;
    fn deref(&self) -> &Self::Target { &self.0 }
}
static PROGRAM_INSTANCE: ProgramInstanceSlot = ProgramInstanceSlot(AtomicIsize::new(0));

#[cfg(target_os = "windows")]
struct MainWindowSlot(AtomicIsize);
impl std::ops::Deref for MainWindowSlot {
    type Target = AtomicIsize;
    fn deref(&self) -> &Self::Target { &self.0 }
}
static MAIN_WINDOW: MainWindowSlot = MainWindowSlot(AtomicIsize::new(0));

#[cfg(target_os = "windows")]
struct GameInFocusSlot(AtomicBool);
impl std::ops::Deref for GameInFocusSlot {
    type Target = AtomicBool;
    fn deref(&self) -> &Self::Target { &self.0 }
}
static GAME_IN_FOCUS: GameInFocusSlot = GameInFocusSlot(AtomicBool::new(false));

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
pub fn set_game_in_focus(_value: bool) {}

#[cfg(not(target_os = "windows"))]
pub fn is_game_in_focus() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn print_win32_error(_win32_error: u32) {}
