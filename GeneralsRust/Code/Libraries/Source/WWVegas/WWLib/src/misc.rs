//! Miscellaneous helpers and globals (ported from WWLib misc.h).

use crate::palette::PaletteClass;

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

/// Instance-owned WWLib misc video/display state (C++ file-statics).
#[derive(Debug, Clone)]
pub struct MiscVideoState {
    pub current_palette: [u8; 768],
    pub debug_windowed: bool,
    pub surfaces_restored: bool,
    pub audio_focus_loss_function: Option<fn()>,
}

impl Default for MiscVideoState {
    fn default() -> Self {
        Self {
            current_palette: [0u8; 768],
            debug_windowed: false,
            surfaces_restored: false,
            audio_focus_loss_function: None,
        }
    }
}

impl MiscVideoState {
    pub fn current_palette(&self) -> [u8; 768] {
        self.current_palette
    }

    pub fn set_current_palette(&mut self, palette: [u8; 768]) {
        self.current_palette = palette;
    }

    pub fn debug_windowed(&self) -> bool {
        self.debug_windowed
    }

    pub fn set_debug_windowed(&mut self, value: bool) {
        self.debug_windowed = value;
    }

    pub fn surfaces_restored(&self) -> bool {
        self.surfaces_restored
    }

    pub fn set_surfaces_restored(&mut self, value: bool) {
        self.surfaces_restored = value;
    }

    pub fn set_audio_focus_loss_function(&mut self, func: Option<fn()>) {
        self.audio_focus_loss_function = func;
    }

    pub fn trigger_audio_focus_loss(&self) {
        if let Some(func) = self.audio_focus_loss_function {
            func();
        }
    }

    pub fn set_palette(&mut self, pal: &PaletteClass, time: i32, callback: Option<fn()>) {
        let bytes = pal.as_bytes();
        let mut buffer = [0u8; 768];
        let len = bytes.len().min(buffer.len());
        buffer[..len].copy_from_slice(&bytes[..len]);
        self.set_current_palette(buffer);

        if time > 0 {
            delay(time);
        }
        if let Some(cb) = callback {
            cb();
        }
    }

    pub fn set_palette_raw(&mut self, palette: &[u8]) {
        let mut buffer = [0u8; 768];
        let len = palette.len().min(buffer.len());
        buffer[..len].copy_from_slice(&palette[..len]);
        self.set_current_palette(buffer);
    }
}

/// Compatibility free helpers — prefer [`MiscVideoState`] on the owning application.
pub fn current_palette(state: &MiscVideoState) -> [u8; 768] {
    state.current_palette()
}

pub fn set_current_palette(state: &mut MiscVideoState, palette: [u8; 768]) {
    state.set_current_palette(palette);
}

pub fn debug_windowed(state: &MiscVideoState) -> bool {
    state.debug_windowed()
}

pub fn set_debug_windowed(state: &mut MiscVideoState, value: bool) {
    state.set_debug_windowed(value);
}

pub fn surfaces_restored(state: &MiscVideoState) -> bool {
    state.surfaces_restored()
}

pub fn set_surfaces_restored(state: &mut MiscVideoState, value: bool) {
    state.set_surfaces_restored(value);
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

pub fn set_palette(
    state: &mut MiscVideoState,
    pal: &PaletteClass,
    time: i32,
    callback: Option<fn()>,
) {
    state.set_palette(pal, time, callback);
}

pub fn set_palette_raw(state: &mut MiscVideoState, palette: &[u8]) {
    state.set_palette_raw(palette);
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

pub fn trigger_audio_focus_loss(state: &MiscVideoState) {
    state.trigger_audio_focus_loss();
}
