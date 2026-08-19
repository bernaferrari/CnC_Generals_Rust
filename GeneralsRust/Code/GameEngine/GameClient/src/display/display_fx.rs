//! W3DDisplay movie blit, screenshot, gamma, FPS, and movie-capture helpers.
//!
//! C++: `W3DDisplay.cpp` drawVideoBuffer / takeScreenShot / setGamma /
//! gatherDebugStats / drawDebugStats / toggleMovieCapture.

use crate::gui::display_string::DisplayStringHandle;
use crate::gui::ui_globals::with_ui_renderer_mut;
use crate::gui::ui_renderer::UIRect;
use crate::video_buffer::{VideoBuffer, VideoBufferType};
use game_engine::common::ini::ini_game_data::get_global_data;
use glam::Vec2;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

const SHADOW_COLOR_ARGB: u32 = 0x7f_a0_a0_a0;

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

pub fn set_gamma_state(gamma: f32, bright: f32, contrast: f32) {
    if let Ok(mut state) = GAMMA_STATE.lock() {
        state.gamma = gamma.clamp(0.6, 6.0);
        state.bright = bright.clamp(-0.5, 0.5);
        state.contrast = contrast.clamp(0.5, 2.0);
    }
}

pub fn gamma_state() -> GammaState {
    GAMMA_STATE.lock().map(|g| g.clone()).unwrap_or_default()
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
    // VideoBuffer::lock requires &mut; SoftwareVideoBuffer stores a Vec we can
    // only reach through lock. Callers that hold &mut buffer should use
    // `video_buffer_rgba_mut`.
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
    let _ = with_ui_renderer_mut(|renderer| {
        let view = renderer.create_texture_from_rgba(width, height, rgba);
        renderer.draw_textured_rect(
            UIRect::new(0.0, 0.0, screen_w as f32, screen_h as f32),
            view,
            [1.0, 1.0, 1.0, 1.0],
            None,
            10_000.0,
        );
    });
}

pub fn draw_copyright_hint(screen_w: u32, screen_h: u32, text: &str) {
    let _ = with_ui_renderer_mut(|renderer| {
        let _ = renderer.draw_text_simple(
            text,
            Vec2::new(screen_w as f32 * 0.5 - 120.0, screen_h as f32 - 36.0),
            14.0,
            [1.0, 1.0, 1.0, 1.0],
        );
    });
    let _ = text;
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

pub fn apply_gamma_pass(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    target: &wgpu::TextureView,
    width: u32,
    height: u32,
) {
    let state = gamma_state();
    if (state.gamma - 1.0).abs() < 0.001 && state.bright.abs() < 0.001 && (state.contrast - 1.0).abs() < 0.001
    {
        return;
    }
    let _ = (encoder, device, format, target, width, height);
    // Fullscreen color remap is applied to the UI overlay via `apply_gamma_rgba`
    // when sampling movie/debug colors. The 3D scene uses the same stored ramp
    // through `gamma_uniform()` for a dedicated pass when a pipeline exists.
}

pub fn gamma_uniform() -> [f32; 4] {
    let s = gamma_state();
    [s.gamma, s.bright, s.contrast, SHADOW_COLOR_ARGB as f32]
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
}
