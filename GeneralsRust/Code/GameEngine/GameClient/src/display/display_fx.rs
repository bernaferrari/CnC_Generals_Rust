//! W3DDisplay movie blit, screenshot, gamma, FPS, and movie-capture helpers.
//!
//! C++: `W3DDisplay.cpp` drawVideoBuffer / takeScreenShot / setGamma /
//! gatherDebugStats / drawDebugStats / toggleMovieCapture / setDisplayMode /
//! drawImage / renderLetterBox.

use crate::gui::display_string::DisplayStringHandle;
use crate::gui::ui_globals::with_ui_renderer_mut;
use crate::gui::ui_renderer::{UIBlendMode, UIRect};
use crate::video_buffer::{VideoBuffer, VideoBufferType};
use game_engine::common::ini::ini_game_data::get_global_data;
use glam::Vec2;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::time::Instant;

const SHADOW_COLOR_ARGB: u32 = 0x7f_a0_a0_a0;
const MIN_DISPLAY_RESOLUTION_X: u32 = 800;

/// C++ `Display::DrawImageMode` (`Display.h` 38-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawImageMode {
    Solid,
    Grayscale,
    #[default]
    Alpha,
    Additive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDeviceMode {
    pub xres: u32,
    pub yres: u32,
    pub bit_depth: u32,
    pub windowed: bool,
}

#[derive(Debug, Clone)]
pub struct CopyrightOverlay {
    pub text: String,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct LetterboxPlan {
    pub bar_height: f32,
    pub color: [f32; 4],
    pub draw_top: bool,
    pub draw_bottom: bool,
}

#[derive(Debug, Clone)]
pub struct GammaState {
    pub gamma: f32,
    pub bright: f32,
    pub contrast: f32,
}

impl Default for GammaState {
    fn default() -> Self {
        Self {
            gamma: 1.0,
            bright: 0.0,
            contrast: 1.0,
        }
    }
}

static GAMMA_STATE: Mutex<GammaState> = Mutex::new(GammaState {
    gamma: 1.0,
    bright: 0.0,
    contrast: 1.0,
});

static SCREENSHOT_SERIAL: AtomicI32 = AtomicI32::new(1);
static MOVIE_FRAME_SERIAL: AtomicI32 = AtomicI32::new(0);
static DEBUG_FRAMES: AtomicU32 = AtomicU32::new(0);
static LAST_FPS: Mutex<f32> = Mutex::new(0.0);
static LAST_FPS_INSTANT: Mutex<Option<Instant>> = Mutex::new(None);

static LAST_MOVIE_FRAME: Mutex<Option<(u32, u32, Vec<u8>)>> = Mutex::new(None);
static PENDING_SCREENSHOT: Mutex<Option<PathBuf>> = Mutex::new(None);
static PENDING_DEVICE_MODE: Mutex<Option<PendingDeviceMode>> = Mutex::new(None);
static COPYRIGHT_OVERLAY: Mutex<Option<CopyrightOverlay>> = Mutex::new(None);
static MOVIE_CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
static LETTERBOX_ENABLED: AtomicBool = AtomicBool::new(false);
static CLIP_ENABLED: AtomicBool = AtomicBool::new(false);
static CLIP_REGION: Mutex<Option<[f32; 4]>> = Mutex::new(None);
static LAST_PRESENTED_FRAME: Mutex<Option<(u32, u32, Vec<u8>)>> = Mutex::new(None);

/// Retail 4:3 modes (`W3DDisplay::getDisplayModeDescription`, ≥800×600, ≥24-bit).
const STANDARD_4_3_MODES: &[(u32, u32)] = &[
    (800, 600),
    (1024, 768),
    (1152, 864),
    (1280, 960),
    (1400, 1050),
    (1600, 1200),
    (1920, 1440),
];

pub fn set_gamma_state(gamma: f32, bright: f32, contrast: f32) {
    if let Ok(mut state) = GAMMA_STATE.lock() {
        state.gamma = gamma.clamp(0.6, 6.0);
        state.bright = bright.clamp(-0.5, 0.5);
        state.contrast = contrast.clamp(0.5, 2.0);
    }
}

/// C++ `TheDisplay->setGamma(gamma, bright, contrast, calibrate)` hook body.
/// wgpu has no DX8 gamma ramp; this stores the LUT used by draw/present.
pub fn display_set_gamma(gamma: f32, bright: f32, contrast: f32, _calibrate: bool) {
    set_gamma_state(gamma, bright, contrast);
}

/// Register `display_set_gamma` as C++ `TheDisplay->setGamma` (OnceLock).
pub fn install_default_display_gamma_hook() {
    crate::gui::options_host_bridge::set_display_gamma_hook(display_set_gamma);
}

pub fn gamma_state() -> GammaState {
    GAMMA_STATE.lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn gamma_is_identity() -> bool {
    let s = gamma_state();
    (s.gamma - 1.0).abs() < 0.001 && s.bright.abs() < 0.001 && (s.contrast - 1.0).abs() < 0.001
}

pub fn set_copyright_overlay(overlay: Option<CopyrightOverlay>) {
    if let Ok(mut g) = COPYRIGHT_OVERLAY.lock() {
        *g = overlay;
    }
}

pub fn present_copyright_overlay(screen_w: u32, screen_h: u32) -> bool {
    let Some(overlay) = COPYRIGHT_OVERLAY.lock().ok().and_then(|g| g.clone()) else {
        return false;
    };
    if overlay.text.is_empty() {
        return false;
    }
    draw_copyright_hint(
        screen_w,
        screen_h,
        &overlay.text,
        overlay.width,
        overlay.height,
    );
    true
}

pub fn set_clip_region(lo_x: f32, lo_y: f32, hi_x: f32, hi_y: f32) {
    if let Ok(mut g) = CLIP_REGION.lock() {
        *g = Some([lo_x, lo_y, hi_x, hi_y]);
    }
}

pub fn enable_clipping(enabled: bool) {
    CLIP_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn clip_region() -> Option<[f32; 4]> {
    if !CLIP_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    CLIP_REGION.lock().ok().and_then(|g| *g)
}

/// C++ `DX8Wrapper::Set_Gamma` ramp applied as a color-space transform.
pub fn apply_gamma_rgba(r: f32, g: f32, b: f32) -> [f32; 3] {
    let state = gamma_state();
    let inv = if state.gamma > 0.0001 {
        1.0 / state.gamma
    } else {
        1.0
    };
    let map = |c: f32| {
        let x = c.clamp(0.0, 1.0).powf(inv);
        (state.contrast * x + state.bright).clamp(0.0, 1.0)
    };
    [map(r), map(g), map(b)]
}

pub fn apply_gamma_to_rgba_in_place(rgba: &mut [u8]) {
    if gamma_is_identity() {
        return;
    }
    for px in rgba.chunks_exact_mut(4) {
        let mapped = apply_gamma_rgba(
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        );
        px[0] = (mapped[0] * 255.0).round() as u8;
        px[1] = (mapped[1] * 255.0).round() as u8;
        px[2] = (mapped[2] * 255.0).round() as u8;
    }
}

fn user_data_dir() -> PathBuf {
    get_global_data()
        .map(|gd| {
            let path = gd.read().get_path_user_data().trim().to_string();
            if path.is_empty() {
                std::env::temp_dir()
            } else {
                PathBuf::from(path)
            }
        })
        .unwrap_or_else(std::env::temp_dir)
}

/// C++ `W3DDisplay::takeScreenShot` — next unused `sshotNNN.bmp` in user data.
pub fn next_screenshot_path() -> PathBuf {
    let dir = user_data_dir();
    let _ = std::fs::create_dir_all(&dir);
    loop {
        let n = SCREENSHOT_SERIAL.fetch_add(1, Ordering::Relaxed);
        let leaf = format!("sshot{n:03}.bmp");
        let path = dir.join(&leaf);
        if !path.exists() {
            return path;
        }
    }
}

pub fn next_movie_frame_path() -> PathBuf {
    let dir = user_data_dir().join("Movie");
    let _ = std::fs::create_dir_all(&dir);
    let n = MOVIE_FRAME_SERIAL.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("Movie{n:05}.bmp"))
}

pub fn movie_capture_dir() -> PathBuf {
    let dir = user_data_dir().join("Movie");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn reset_movie_capture_counter() {
    MOVIE_FRAME_SERIAL.store(0, Ordering::Relaxed);
}

/// Write a 24-bit BMP (C++ `CreateBMPFile`).
pub fn write_bmp_bgr(path: &std::path::Path, width: u32, height: u32, bgr: &[u8]) -> bool {
    let row_stride = ((width * 3 + 3) / 4) * 4;
    let image_size = row_stride * height;
    let file_size = 14 + 40 + image_size;
    let mut out = Vec::with_capacity(file_size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&image_size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    let src_stride = width as usize * 3;
    let pad = vec![0u8; (row_stride - width * 3) as usize];
    for y in (0..height as usize).rev() {
        let start = y * src_stride;
        let end = start + src_stride;
        if end > bgr.len() {
            return false;
        }
        out.extend_from_slice(&bgr[start..end]);
        out.extend_from_slice(&pad);
    }
    std::fs::write(path, out).is_ok()
}

pub fn rgba_to_bgr(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut bgr = vec![0u8; width as usize * height as usize * 3];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let s = (y * width as usize + x) * 4;
            let d = (y * width as usize + x) * 3;
            if s + 3 >= rgba.len() {
                break;
            }
            bgr[d] = rgba[s + 2];
            bgr[d + 1] = rgba[s + 1];
            bgr[d + 2] = rgba[s];
        }
    }
    bgr
}

pub fn video_buffer_rgba(buffer: &dyn VideoBuffer) -> Option<(u32, u32, Vec<u8>)> {
    if !buffer.valid() {
        return None;
    }
    let width = buffer.width();
    let height = buffer.height();
    let pitch = buffer.pitch();
    if width == 0 || height == 0 || pitch == 0 {
        return None;
    }
    let byte_len = (pitch as usize).saturating_mul(height as usize);
    let _ = (width, height, byte_len);
    None
}

pub fn video_buffer_rgba_mut(buffer: &mut dyn VideoBuffer) -> Option<(u32, u32, Vec<u8>)> {
    if !buffer.valid() {
        return None;
    }
    let width = buffer.width();
    let height = buffer.height();
    let pitch = buffer.pitch();
    if width == 0 || height == 0 || pitch == 0 {
        return None;
    }
    let byte_len = (pitch as usize).saturating_mul(height as usize);
    let ptr = buffer.lock();
    if ptr.is_null() || byte_len == 0 {
        buffer.unlock();
        return None;
    }
    // SAFETY: ptr came from buffer.lock() (non-null checked above) and points at
    // SAFETY: the locked buffer's backing store; byte_len = pitch*height bounds the
    // SAFETY: readable region. The slice is consumed before buffer.unlock() below.
    let src = unsafe { std::slice::from_raw_parts(ptr, byte_len) };
    let data = match buffer.format() {
        VideoBufferType::X8R8G8B8 => convert_x8r8g8b8(src, width, height, pitch),
        VideoBufferType::R8G8B8 => convert_r8g8b8(src, width, height, pitch),
        _ => None,
    };
    buffer.unlock();
    data.map(|data| (width, height, data))
}

fn convert_x8r8g8b8(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = width as usize * 4;
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height as usize {
        let src_row = y * pitch;
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let s = x * 4;
            let d = (y * width as usize + x) * 4;
            out[d] = row[s + 2];
            out[d + 1] = row[s + 1];
            out[d + 2] = row[s];
            out[d + 3] = 255;
        }
    }
    Some(out)
}

fn convert_r8g8b8(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = width as usize * 3;
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height as usize {
        let src_row = y * pitch;
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let s = x * 3;
            let d = (y * width as usize + x) * 4;
            out[d] = row[s + 2];
            out[d + 1] = row[s + 1];
            out[d + 2] = row[s];
            out[d + 3] = 255;
        }
    }
    Some(out)
}

/// C++ `drawVideoBuffer` fullscreen textured quad.
pub fn blit_video_rgba(width: u32, height: u32, rgba: &[u8], screen_w: u32, screen_h: u32) {
    let mut pixels = rgba.to_vec();
    apply_gamma_to_rgba_in_place(&mut pixels);
    let _ = with_ui_renderer_mut(|renderer| {
        let view = renderer.create_texture_from_rgba(width, height, &pixels);
        renderer.draw_textured_rect(
            UIRect::new(0.0, 0.0, screen_w as f32, screen_h as f32),
            view,
            [1.0, 1.0, 1.0, 1.0],
            None,
            9_500.0,
        );
    });
}

/// C++ `W3DDisplay::draw` copyright: black, `((width-dX)/2, height-dY-20)`.
pub fn draw_copyright_hint(screen_w: u32, screen_h: u32, text: &str, text_w: f32, text_h: f32) {
    let x = (screen_w as f32 - text_w) * 0.5;
    let y = screen_h as f32 - text_h - 20.0;
    let _ = with_ui_renderer_mut(|renderer| {
        let _ = renderer.draw_text_simple(
            text,
            Vec2::new(x, y),
            text_h.max(12.0),
            [0.0, 0.0, 0.0, 1.0],
        );
    });
}

pub fn copyright_text(handle: &Option<DisplayStringHandle>) -> String {
    if handle.is_some() {
        crate::game_text::GameText::fetch("GUI:EACopyright")
    } else {
        String::new()
    }
}

pub fn note_frame_for_fps() -> f32 {
    let frames = DEBUG_FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    let mut last = LAST_FPS_INSTANT.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    match *last {
        None => {
            *last = Some(now);
            0.0
        }
        Some(prev) => {
            let elapsed = now.duration_since(prev).as_secs_f32();
            if elapsed >= 2.0 {
                let fps = frames as f32 / elapsed.max(0.1);
                DEBUG_FRAMES.store(0, Ordering::Relaxed);
                *last = Some(now);
                if let Ok(mut stored) = LAST_FPS.lock() {
                    *stored = fps;
                }
                fps
            } else {
                LAST_FPS.lock().map(|g| *g).unwrap_or(0.0)
            }
        }
    }
}

pub fn draw_debug_overlay(screen_w: u32, fps: f32, frame: u32, particle_count: u32) {
    let _ = screen_w;
    let _ = with_ui_renderer_mut(|renderer| {
        let ms = if fps > 0.1 { 1000.0 / fps } else { 0.0 };
        let line0 = format!("FPS: {fps:.2}, {ms:.2}ms");
        let line1 = format!("Frame: {frame}");
        let line2 = format!("Particles: {particle_count}");
        let _ = renderer.draw_text_simple(&line0, Vec2::new(3.0, 3.0), 13.0, [1.0, 1.0, 1.0, 1.0]);
        let _ = renderer.draw_text_simple(&line1, Vec2::new(3.0, 16.0), 13.0, [1.0, 1.0, 1.0, 1.0]);
        let _ = renderer.draw_text_simple(&line2, Vec2::new(3.0, 29.0), 13.0, [1.0, 1.0, 1.0, 1.0]);
    });
}

pub fn draw_framerate_bar(screen_w: u32, fps: f32, fps_limit: f32) {
    let perc = if fps_limit > 0.0 {
        (fps / fps_limit).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let width = perc * screen_w as f32;
    let color = [1.0 - perc, perc, 0.0, 0.5];
    let _ = with_ui_renderer_mut(|renderer| {
        renderer.draw_rect(UIRect::new(1.0, 1.0, width, 15.0), color, 9_000.0);
    });
}

/// C++ `W3DDisplay::setGamma` / `DX8Wrapper::Set_Gamma` analog.
///
/// Hardware LUT is a wgpu color remap of the last movie/backbuffer sample.
/// Called from leftover `Display::draw` and host present.
pub fn apply_gamma_pass(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    target: &wgpu::TextureView,
    width: u32,
    height: u32,
) {
    let _ = (encoder, device, format, target, width, height);
    if gamma_is_identity() {
        return;
    }
    if let Ok(mut frame) = LAST_MOVIE_FRAME.lock() {
        if let Some((_, _, rgba)) = frame.as_mut() {
            apply_gamma_to_rgba_in_place(rgba);
        }
    }
    if let Ok(mut frame) = LAST_PRESENTED_FRAME.lock() {
        if let Some((_, _, rgba)) = frame.as_mut() {
            apply_gamma_to_rgba_in_place(rgba);
        }
    }
}

pub fn gamma_uniform() -> [f32; 4] {
    let s = gamma_state();
    [s.gamma, s.bright, s.contrast, SHADOW_COLOR_ARGB as f32]
}

// ---------------------------------------------------------------------------
// Live-path host present hooks (C++ W3DDisplay::draw / setDisplayMode)
// ---------------------------------------------------------------------------

pub fn store_movie_frame(width: u32, height: u32, rgba: Vec<u8>) {
    if let Ok(mut frame) = LAST_MOVIE_FRAME.lock() {
        *frame = Some((width, height, rgba));
    }
}

pub fn current_movie_frame() -> Option<(u32, u32, Vec<u8>)> {
    LAST_MOVIE_FRAME.lock().ok().and_then(|g| g.clone())
}

pub fn clear_movie_frame() {
    if let Ok(mut frame) = LAST_MOVIE_FRAME.lock() {
        *frame = None;
    }
}

/// Host present: C++ `drawVideoBuffer(m_videoBuffer, 0, 0, getWidth(), getHeight())`.
pub fn present_movie_overlay(screen_w: u32, screen_h: u32) -> bool {
    let Some((w, h, rgba)) = current_movie_frame() else {
        return false;
    };
    // While a Display movie covers the frame, that buffer *is* the backbuffer.
    note_presented_frame(w, h, rgba.clone());
    blit_video_rgba(w, h, &rgba, screen_w, screen_h);
    true
}

/// C++ W3DDisplay::draw movie + copyright + letterbox + capture flush.
pub fn present_host_overlays(
    screen_w: u32,
    screen_h: u32,
    letterbox_fade: f32,
    letterbox_enabled: bool,
) {
    if present_movie_overlay(screen_w, screen_h) {
        let _ = present_copyright_overlay(screen_w, screen_h);
    }
    queue_letterbox_bars(
        screen_w as f32,
        screen_h as f32,
        letterbox_fade,
        letterbox_enabled,
    );
    flush_pending_captures();
}

pub fn flush_pending_captures() {
    if is_movie_capture_enabled() {
        let _ = write_backbuffer_movie_frame();
    }
    if let Some(path) = take_pending_screenshot() {
        if !write_backbuffer_screenshot(&path) {
            queue_screenshot(path);
        }
    }
}

pub fn queue_screenshot(path: PathBuf) {
    if let Ok(mut pending) = PENDING_SCREENSHOT.lock() {
        *pending = Some(path);
    }
}

pub fn take_pending_screenshot() -> Option<PathBuf> {
    PENDING_SCREENSHOT.lock().ok().and_then(|mut g| g.take())
}

pub fn note_presented_frame(width: u32, height: u32, rgba: Vec<u8>) {
    if let Ok(mut frame) = LAST_PRESENTED_FRAME.lock() {
        *frame = Some((width, height, rgba));
    }
}

pub fn current_presented_frame() -> Option<(u32, u32, Vec<u8>)> {
    LAST_PRESENTED_FRAME.lock().ok().and_then(|g| g.clone())
}

/// Write a screenshot from the presented backbuffer (never the stale movie buffer
/// unless that movie is the current fullscreen blit and no backbuffer exists).
pub fn write_backbuffer_screenshot(path: &std::path::Path) -> bool {
    if let Some((w, h, mut rgba)) = current_presented_frame() {
        apply_gamma_to_rgba_in_place(&mut rgba);
        let bgr = rgba_to_bgr(&rgba, w, h);
        return write_bmp_bgr(path, w, h, &bgr);
    }
    false
}

pub fn write_backbuffer_movie_frame() -> bool {
    let path = next_movie_frame_path();
    write_backbuffer_screenshot(&path)
}

pub fn set_movie_capture_enabled(enabled: bool) {
    MOVIE_CAPTURE_ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        reset_movie_capture_counter();
    }
}

pub fn is_movie_capture_enabled() -> bool {
    MOVIE_CAPTURE_ENABLED.load(Ordering::Relaxed)
}

pub fn queue_device_mode(mode: PendingDeviceMode) {
    if let Ok(mut pending) = PENDING_DEVICE_MODE.lock() {
        *pending = Some(mode);
    }
}

pub fn take_pending_device_mode() -> Option<PendingDeviceMode> {
    PENDING_DEVICE_MODE.lock().ok().and_then(|mut g| g.take())
}

pub fn peek_pending_device_mode() -> Option<PendingDeviceMode> {
    PENDING_DEVICE_MODE.lock().ok().and_then(|g| *g)
}

/// C++ `W3DDisplay::getDisplayModeCount` — 4:3, ≥800×600, ≥24-bit.
pub fn display_mode_count() -> i32 {
    STANDARD_4_3_MODES.len() as i32
}

/// C++ `W3DDisplay::getDisplayModeDescription`.
pub fn display_mode_description(mode_index: i32) -> Option<(u32, u32, u32)> {
    let idx = usize::try_from(mode_index).ok()?;
    let &(w, h) = STANDARD_4_3_MODES.get(idx)?;
    if w < MIN_DISPLAY_RESOLUTION_X || !is_four_by_three(w, h) {
        return None;
    }
    Some((w, h, 32))
}

pub fn is_four_by_three(width: u32, height: u32) -> bool {
    if height == 0 {
        return false;
    }
    let aspect = width as f32 / height as f32;
    (aspect - 4.0 / 3.0).abs() < 0.03
}

pub fn is_valid_device_mode(xres: u32, yres: u32, bit_depth: u32) -> bool {
    xres >= MIN_DISPLAY_RESOLUTION_X && yres >= 600 && bit_depth >= 24 && xres > 0 && yres > 0
}

pub fn set_letterbox_enabled(enabled: bool) {
    LETTERBOX_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn letterbox_enabled() -> bool {
    LETTERBOX_ENABLED.load(Ordering::Relaxed)
}

/// C++ `W3DDisplay::renderLetterBox` (non-SLIDE): constant 16:9 height, alpha fade.
/// Fade-out (`!enabled`) draws only the top bar — bottom `drawFillRect` is commented out.
pub fn letterbox_plan(width: f32, height: f32, fade: f32, enabled: bool) -> LetterboxPlan {
    let fade = fade.clamp(0.0, 1.0);
    let bar_height = ((height - (9.0 / 16.0) * width) * 0.5).max(0.0);
    let visible = fade > 0.0 && bar_height > 0.5;
    LetterboxPlan {
        bar_height,
        color: [0.0, 0.0, 0.0, fade],
        draw_top: visible,
        draw_bottom: visible && enabled,
    }
}

pub fn queue_letterbox_bars(width: f32, height: f32, fade: f32, enabled: bool) {
    let plan = letterbox_plan(width, height, fade, enabled);
    if !plan.draw_top {
        return;
    }
    let _ = with_ui_renderer_mut(|renderer| {
        renderer.draw_rect(
            UIRect::new(0.0, 0.0, width, plan.bar_height),
            plan.color,
            10_000.0,
        );
        if plan.draw_bottom {
            renderer.draw_rect(
                UIRect::new(0.0, height - plan.bar_height, width, plan.bar_height),
                plan.color,
                10_000.0,
            );
        }
    });
}

/// Queue a W3D `drawImage` textured mesh (clip + rotate + mode).
pub fn queue_draw_image_mesh(
    texture: std::sync::Arc<wgpu::TextureView>,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    uv_left: f32,
    uv_top: f32,
    uv_right: f32,
    uv_bottom: f32,
    color: [f32; 4],
    mode: DrawImageMode,
    rotated_90: bool,
) -> bool {
    with_ui_renderer_mut(|renderer| {
        queue_draw_image_mesh_on(
            renderer, texture, start_x, start_y, end_x, end_y, uv_left, uv_top, uv_right,
            uv_bottom, color, mode, rotated_90,
        )
    })
    .unwrap_or(false)
}

pub fn queue_draw_image_mesh_on(
    renderer: &mut crate::gui::ui_renderer::UIRenderer,
    texture: std::sync::Arc<wgpu::TextureView>,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    uv_left: f32,
    uv_top: f32,
    uv_right: f32,
    uv_bottom: f32,
    color: [f32; 4],
    mode: DrawImageMode,
    rotated_90: bool,
) -> bool {
    let Some((sx0, sy0, sx1, sy1, u0, v0, u1, v1)) = clip_image_quad(
        start_x, start_y, end_x, end_y, uv_left, uv_top, uv_right, uv_bottom, rotated_90,
    ) else {
        return false;
    };
    let (color, blend) = apply_draw_image_color(color, mode);
    if rotated_90 {
        let verts = rotated_90_vertices(sx0, sy0, sx1, sy1, u0, v0, u1, v1);
        let positions: Vec<[f32; 2]> = verts.iter().map(|(p, _)| *p).collect();
        let uvs: Vec<[f32; 2]> = verts.iter().map(|(_, uv)| *uv).collect();
        renderer.draw_textured_mesh(
            &positions,
            &uvs,
            &[0, 1, 2, 3, 4, 5],
            texture,
            color,
            blend,
            0.0,
        );
    } else {
        renderer.draw_textured_rect_ex(
            UIRect::new(sx0, sy0, sx1 - sx0, sy1 - sy0),
            texture,
            color,
            Some(UIRect::new(u0, v0, u1 - u0, v1 - v0)),
            blend,
            0.0,
        );
    }
    true
}

/// C++ `W3DDisplay::drawImage` clip remap (normal or `IMAGE_STATUS_ROTATED_90_CLOCKWISE`).
pub fn clip_image_quad(
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    uv_left: f32,
    uv_top: f32,
    uv_right: f32,
    uv_bottom: f32,
    rotated_90: bool,
) -> Option<(f32, f32, f32, f32, f32, f32, f32, f32)> {
    let Some([clip_lo_x, clip_lo_y, clip_hi_x, clip_hi_y]) = clip_region() else {
        return Some((
            start_x, start_y, end_x, end_y, uv_left, uv_top, uv_right, uv_bottom,
        ));
    };
    if end_x <= clip_lo_x || end_y <= clip_lo_y {
        return None;
    }
    let screen_w = end_x - start_x;
    let screen_h = end_y - start_y;
    if screen_w.abs() < 0.001 || screen_h.abs() < 0.001 {
        return None;
    }
    let clipped_left = start_x.max(clip_lo_x);
    let clipped_right = end_x.min(clip_hi_x);
    let clipped_top = start_y.max(clip_lo_y);
    let clipped_bottom = end_y.min(clip_hi_y);
    if clipped_right <= clipped_left || clipped_bottom <= clipped_top {
        return None;
    }
    let uv_w = uv_right - uv_left;
    let uv_h = uv_bottom - uv_top;
    let (cu_l, cu_t, cu_r, cu_b) = if rotated_90 {
        let p_left = (clipped_left - start_x) / screen_w;
        let p_right = (clipped_right - start_x) / screen_w;
        let p_top = (clipped_top - start_y) / screen_h;
        let p_bottom = (clipped_bottom - start_y) / screen_h;
        let uv_top_c = uv_top + uv_h * p_left;
        let uv_bottom_c = uv_top + uv_h * p_right;
        let uv_right_c = uv_right - uv_w * p_top;
        let uv_left_c = uv_right - uv_w * p_bottom;
        (uv_left_c, uv_top_c, uv_right_c, uv_bottom_c)
    } else {
        let p_left = (clipped_left - start_x) / screen_w;
        let p_right = (clipped_right - start_x) / screen_w;
        let p_top = (clipped_top - start_y) / screen_h;
        let p_bottom = (clipped_bottom - start_y) / screen_h;
        (
            uv_left + uv_w * p_left,
            uv_top + uv_h * p_top,
            uv_left + uv_w * p_right,
            uv_top + uv_h * p_bottom,
        )
    };
    Some((
        clipped_left,
        clipped_top,
        clipped_right,
        clipped_bottom,
        cu_l,
        cu_t,
        cu_r,
        cu_b,
    ))
}

/// C++ `W3DDisplay::drawImage` two-tri UV swap for `IMAGE_STATUS_ROTATED_90_CLOCKWISE`.
pub fn rotated_90_vertices(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    uv_left: f32,
    uv_top: f32,
    uv_right: f32,
    uv_bottom: f32,
) -> [([f32; 2], [f32; 2]); 6] {
    [
        ([left, top], [uv_right, uv_top]),
        ([left, bottom], [uv_left, uv_top]),
        ([right, top], [uv_right, uv_bottom]),
        ([right, bottom], [uv_left, uv_bottom]),
        ([right, top], [uv_right, uv_bottom]),
        ([left, bottom], [uv_left, uv_top]),
    ]
}

pub fn color_u32_to_rgba(color: u32) -> [f32; 4] {
    let a = ((color >> 24) & 0xff) as f32 / 255.0;
    let r = ((color >> 16) & 0xff) as f32 / 255.0;
    let g = ((color >> 8) & 0xff) as f32 / 255.0;
    let b = (color & 0xff) as f32 / 255.0;
    [r, g, b, a]
}

pub fn apply_draw_image_color(color: [f32; 4], mode: DrawImageMode) -> ([f32; 4], UIBlendMode) {
    // C++ DX8Wrapper::Set_Gamma is a hardware LUT after all draws.
    // wgpu analog: remap every W3D drawImage tint that reaches the renderer.
    let (mut color, blend) = match mode {
        DrawImageMode::Alpha => (color, UIBlendMode::Alpha),
        DrawImageMode::Additive => (color, UIBlendMode::Additive),
        DrawImageMode::Solid => ([color[0], color[1], color[2], 1.0], UIBlendMode::None),
        DrawImageMode::Grayscale => {
            let gray = 0.299 * color[0] + 0.587 * color[1] + 0.114 * color[2];
            ([gray, gray, gray, color[3]], UIBlendMode::Grayscale)
        }
    };
    let mapped = apply_gamma_rgba(color[0], color[1], color[2]);
    color[0] = mapped[0];
    color[1] = mapped[1];
    color[2] = mapped[2];
    (color, blend)
}

/// C++ `W3DDisplay::setGamma` windowed early-out analog for host present.
pub fn present_gamma_if_fullscreen(windowed: bool, screen_w: u32, screen_h: u32) {
    if windowed || gamma_is_identity() {
        return;
    }
    let _ = (screen_w, screen_h);
    // blit_video_rgba already remaps a copy. Remap the presented backbuffer
    // sample used for screenshots / host consume of LAST_PRESENTED_FRAME.
    if let Ok(mut frame) = LAST_PRESENTED_FRAME.lock() {
        if let Some((_, _, rgba)) = frame.as_mut() {
            apply_gamma_to_rgba_in_place(rgba);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_identity_at_defaults() {
        set_gamma_state(1.0, 0.0, 1.0);
        let mapped = apply_gamma_rgba(0.5, 0.25, 0.75);
        assert!((mapped[0] - 0.5).abs() < 0.01);
        assert!((mapped[1] - 0.25).abs() < 0.01);
        assert!((mapped[2] - 0.75).abs() < 0.01);
    }

    #[test]
    fn bmp_header_is_valid() {
        let dir = std::env::temp_dir();
        let path = dir.join("sshot_test_fx.bmp");
        let pixels = vec![0u8, 0, 255, 0, 255, 0, 255, 0, 0, 128, 128, 128];
        assert!(write_bmp_bgr(&path, 2, 2, &pixels));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..2], b"BM");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn letterbox_fade_out_drops_bottom_bar() {
        // C++ W3DDisplay.cpp:2021-2022 — bottom drawFillRect commented out while fading out.
        let fade_in = letterbox_plan(1920.0, 1080.0, 0.5, true);
        assert!(fade_in.draw_top && fade_in.draw_bottom);
        assert!((fade_in.bar_height - ((1080.0 - 9.0 / 16.0 * 1920.0) * 0.5)).abs() < 0.01);
        let fade_out = letterbox_plan(1920.0, 1080.0, 0.5, false);
        assert!(fade_out.draw_top && !fade_out.draw_bottom);
        assert!((fade_out.bar_height - fade_in.bar_height).abs() < 0.01);
    }

    #[test]
    fn display_modes_are_four_by_three() {
        assert!(display_mode_count() > 0);
        for i in 0..display_mode_count() {
            let (w, h, depth) = display_mode_description(i).expect("mode");
            assert!(is_four_by_three(w, h));
            assert!(w >= 800 && depth >= 24);
        }
        assert!(!is_valid_device_mode(0, 0, 32));
        assert!(is_valid_device_mode(800, 600, 32));
    }

    #[test]
    fn clip_image_quad_remaps_uv() {
        enable_clipping(true);
        set_clip_region(10.0, 10.0, 50.0, 50.0);
        let clipped =
            clip_image_quad(0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 1.0, 1.0, false).expect("visible");
        assert!((clipped.0 - 10.0).abs() < 0.01);
        assert!((clipped.4 - 0.1).abs() < 0.01);
        enable_clipping(false);
    }

    #[test]
    fn grayscale_mode_desaturates_tint() {
        set_gamma_state(1.0, 0.0, 1.0);
        let (c, blend) = apply_draw_image_color([1.0, 0.0, 0.0, 1.0], DrawImageMode::Grayscale);
        assert!((c[0] - c[1]).abs() < 0.001 && (c[1] - c[2]).abs() < 0.001);
        assert_eq!(blend, UIBlendMode::Grayscale);
        let (_, add) = apply_draw_image_color([1.0, 1.0, 1.0, 0.5], DrawImageMode::Additive);
        assert_eq!(add, UIBlendMode::Additive);
    }

    #[test]
    fn draw_image_color_consumes_display_gamma_like_dx8_set_gamma() {
        // C++ W3DDisplay.cpp:519-524 TheDisplay->setGamma / DX8Wrapper::Set_Gamma.
        set_gamma_state(2.0, 0.0, 1.0);
        let (c, _) = apply_draw_image_color([0.25, 0.25, 0.25, 1.0], DrawImageMode::Alpha);
        let expected = apply_gamma_rgba(0.25, 0.25, 0.25);
        assert!((c[0] - expected[0]).abs() < 0.01);
        assert!((c[0] - 0.25).abs() > 0.05);
        set_gamma_state(1.0, 0.0, 1.0);
    }

    #[test]
    fn present_gamma_remaps_presented_backbuffer() {
        set_gamma_state(2.0, 0.0, 1.0);
        note_presented_frame(1, 1, vec![64, 0, 0, 255]);
        present_gamma_if_fullscreen(false, 8, 8);
        let (_, _, rgba) = current_presented_frame().expect("presented");
        assert_ne!(rgba[0], 64);
        present_gamma_if_fullscreen(true, 8, 8);
        set_gamma_state(1.0, 0.0, 1.0);
        if let Ok(mut frame) = LAST_PRESENTED_FRAME.lock() {
            *frame = None;
        }
    }

    #[test]
    fn screenshot_does_not_use_stale_movie_without_backbuffer() {
        clear_movie_frame();
        if let Ok(mut frame) = LAST_PRESENTED_FRAME.lock() {
            *frame = None;
        }
        let path = std::env::temp_dir().join("sshot_no_backbuffer.bmp");
        assert!(!write_backbuffer_screenshot(&path));
        note_presented_frame(1, 1, vec![10, 20, 30, 255]);
        assert!(write_backbuffer_screenshot(&path));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn present_movie_notes_backbuffer_for_screenshot() {
        clear_movie_frame();
        if let Ok(mut frame) = LAST_PRESENTED_FRAME.lock() {
            *frame = None;
        }
        store_movie_frame(1, 1, vec![255, 0, 0, 255]);
        assert!(present_movie_overlay(8, 8));
        let path = std::env::temp_dir().join("sshot_movie_as_backbuffer.bmp");
        assert!(write_backbuffer_screenshot(&path));
        let _ = std::fs::remove_file(path);
        clear_movie_frame();
    }
}
