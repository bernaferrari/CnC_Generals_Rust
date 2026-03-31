//! Miscellaneous helpers and globals (ported from WWLib misc.h).

use crate::palette::PaletteClass;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use windows::core::HRESULT;

#[cfg(not(target_os = "windows"))]
type HRESULT = i32;

pub const VIDEO_BLITTER: u32 = 1;
pub const VIDEO_BLITTER_ASYNC: u32 = 2;
pub const VIDEO_SYNC_PALETTE: u32 = 4;
pub const VIDEO_BANK_SWITCHED: u32 = 8;
pub const VIDEO_COLOR_FILL: u32 = 16;
pub const VIDEO_NO_HARDWARE_ASSIST: u32 = 32;

static CURRENT_PALETTE: OnceLock<Mutex<[u8; 768]>> = OnceLock::new();
static DEBUG_WINDOWED: AtomicBool = AtomicBool::new(false);
static SURFACES_RESTORED: AtomicBool = AtomicBool::new(false);

pub fn current_palette() -> [u8; 768] {
    *CURRENT_PALETTE
        .get_or_init(|| Mutex::new([0u8; 768]))
        .lock()
        .unwrap()
}

pub fn set_current_palette(palette: [u8; 768]) {
    *CURRENT_PALETTE
        .get_or_init(|| Mutex::new([0u8; 768]))
        .lock()
        .unwrap() = palette;
}

pub fn debug_windowed() -> bool {
    DEBUG_WINDOWED.load(Ordering::Relaxed)
}

pub fn set_debug_windowed(value: bool) {
    DEBUG_WINDOWED.store(value, Ordering::Relaxed);
}

pub fn surfaces_restored() -> bool {
    SURFACES_RESTORED.load(Ordering::Relaxed)
}

pub fn set_surfaces_restored(value: bool) {
    SURFACES_RESTORED.store(value, Ordering::Relaxed);
}

pub fn prep_direct_draw() {}

pub fn process_dd_result(_result: HRESULT, _display_ok_msg: i32) {}

pub fn set_video_mode(_hwnd: usize, _w: i32, _h: i32, _bits_per_pixel: i32) -> bool {
    true
}

pub fn reset_video_mode() {}

pub fn get_free_video_memory() -> u32 {
    0
}

pub fn wait_blit() {}

pub fn get_video_hardware_capabilities() -> u32 {
    VIDEO_NO_HARDWARE_ASSIST
}

pub fn wait_vert_blank() {}

pub fn set_palette(pal: &PaletteClass, time: i32, callback: Option<fn()>) {
    let bytes = pal.as_bytes();
    let mut buffer = [0u8; 768];
    let len = bytes.len().min(buffer.len());
    buffer[..len].copy_from_slice(&bytes[..len]);
    set_current_palette(buffer);

    if time > 0 {
        delay(time);
    }
    if let Some(cb) = callback {
        cb();
    }
}

pub fn set_palette_raw(palette: &[u8]) {
    let mut buffer = [0u8; 768];
    let len = palette.len().min(buffer.len());
    buffer[..len].copy_from_slice(&palette[..len]);
    set_current_palette(buffer);
}

pub fn delay(duration: i32) {
    if duration <= 0 {
        return;
    }
    std::thread::sleep(std::time::Duration::from_millis(duration as u64));
}

pub fn vsync() {
    wait_vert_blank();
}

pub static AUDIO_FOCUS_LOSS_FUNCTION: Mutex<Option<fn()>> = Mutex::new(None);

pub fn trigger_audio_focus_loss() {
    if let Some(func) = *AUDIO_FOCUS_LOSS_FUNCTION.lock().unwrap() {
        func();
    }
}
