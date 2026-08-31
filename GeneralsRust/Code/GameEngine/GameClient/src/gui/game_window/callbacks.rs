//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use std::sync::Arc;

use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};
use crate::game_text::GameText;
use crate::gui::UIRect;
use crate::gui::gadgets::{InputEvent, KeyCode, KeyModifiers};
use crate::gui::with_ui_renderer_mut;
use crate::video_buffer::{VideoBufferHandle, VideoBufferType};

use super::font::{Color, WindowInstanceData, WindowState};
use super::messages::{WIN_COLOR_UNDEFINED, WindowMessage, WindowMsgHandled, WindowStatus};
use super::payload::{
    KEY_STATE_DOWN, KEY_STATE_LALT, KEY_STATE_LCONTROL, KEY_STATE_LSHIFT, KEY_STATE_RALT,
    KEY_STATE_RCONTROL, KEY_STATE_RSHIFT, KEY_STATE_UP, WindowMsgData,
};
use super::window_struct::GameWindow;

/// C++ W3DGameWinDefaultDraw ops (W3DGameWindow.cpp 278–323).
///
/// `WIN_STATUS_IMAGE` draws the mapped image only (neutral tint). Color fill
/// and border are the else branch. Missing image → no ops (not a black rect).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3dDefaultDrawOp {
    Image,
    ColorFill,
    ColorBorder,
}

/// Plan the C++ IMAGE vs color-fill branch. Tests and [`default_draw_callback`]
/// share this so IMAGE + defined color cannot emit `draw_rect` fill.
pub fn plan_w3d_game_win_default_draw(
    image_status: bool,
    color_defined: bool,
    border_defined: bool,
    has_image: bool,
) -> Vec<W3dDefaultDrawOp> {
    if image_status {
        if has_image {
            vec![W3dDefaultDrawOp::Image]
        } else {
            Vec::new()
        }
    } else {
        let mut ops = Vec::new();
        if color_defined {
            ops.push(W3dDefaultDrawOp::ColorFill);
        }
        if border_defined {
            ops.push(W3dDefaultDrawOp::ColorBorder);
        }
        ops
    }
}

// Default callback implementations
pub fn legacy_default_draw_callback(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ parity: GameWinDefaultDraw is a no-op. USER/[None]/W3DNoDraw windows
    // should not fall through into a Rust-only generic image draw path.
}

pub fn default_draw_callback(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    let video_frame = _inst_data.video_buffer.as_ref().and_then(read_video_frame);
    let _ = with_ui_renderer_mut(|renderer| {
        let (x, y) = _window.get_screen_position();
        let (width, height) = _window.get_size();
        let offset = _inst_data.image_offset;
        let mut rect = UIRect::new(
            (x + offset.x) as f32,
            (y + offset.y) as f32,
            width as f32,
            height as f32,
        );
        let scale = _window.get_press_scale();
        if (scale - 1.0).abs() > f32::EPSILON {
            let cx = rect.x + rect.width * 0.5;
            let cy = rect.y + rect.height * 0.5;
            let scaled_width = rect.width * scale;
            let scaled_height = rect.height * scale;
            rect = UIRect::new(
                cx - scaled_width * 0.5,
                cy - scaled_height * 0.5,
                scaled_width,
                scaled_height,
            );
        }

        let (draw_data, text_colors) =
            if _inst_data.state.contains(WindowState::DISABLED) || !_window.is_enabled() {
                (&_inst_data.disabled_draw_data, &_inst_data.disabled_text)
            } else if _inst_data.state.contains(WindowState::HILITED) {
                (&_inst_data.hilite_draw_data, &_inst_data.hilite_text)
            } else {
                (&_inst_data.enabled_draw_data, &_inst_data.enabled_text)
            };

        let entry = draw_data.first();
        let color_defined = entry.is_some_and(|e| e.color != WIN_COLOR_UNDEFINED);
        let border_defined = entry.is_some_and(|e| e.border_color != WIN_COLOR_UNDEFINED);
        let has_image = entry.is_some_and(|e| e.image.is_some());
        let image_status = _window.get_status().contains(WindowStatus::IMAGE);
        let ops =
            plan_w3d_game_win_default_draw(image_status, color_defined, border_defined, has_image);

        if ops.contains(&W3dDefaultDrawOp::ColorFill) {
            if let Some(entry) = entry {
                renderer.draw_rect(rect, color_to_rgba(entry.color), 0.0);
            }
        }
        if ops.contains(&W3dDefaultDrawOp::ColorBorder) {
            if let Some(entry) = entry {
                renderer.draw_rect_outline(rect, 1.0, color_to_rgba(entry.border_color), 0.1);
            }
        }
        if ops.contains(&W3dDefaultDrawOp::Image) {
            // Neutral/white tint matches C++ winDrawImage (no color multiply).
            if let Some(entry) = entry {
                if let Some(image) = &entry.image {
                    let _ = ensure_client_mapped_image(&image.name);
                    let texture = {
                        let collection = get_mapped_image_collection();
                        let mut collection = collection.write();
                        if let Some(mapped) = collection.find_image_by_name_mut(&image.name) {
                            if mapped.get_gpu_texture().is_none() {
                                let _ =
                                    mapped.create_gpu_texture(renderer.device(), renderer.queue());
                            }
                            let texture = mapped.get_gpu_texture().map(|gpu| {
                                let uv = mapped.get_uv();
                                (
                                    Arc::new(gpu.view().clone()),
                                    UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height()),
                                )
                            });
                            texture
                        } else {
                            None
                        }
                    };

                    if let Some((texture, tex_rect)) = texture {
                        renderer.draw_textured_rect(
                            rect,
                            texture,
                            [1.0, 1.0, 1.0, 1.0],
                            Some(tex_rect),
                            0.0,
                        );
                    }
                }
            }
        }

        if let Some(frame) = video_frame.as_ref() {
            let video_rect = UIRect::new(x as f32, y as f32, width as f32, height as f32);
            let texture = renderer.create_texture_from_rgba(frame.width, frame.height, &frame.data);
            renderer.draw_textured_rect(video_rect, texture, [1.0, 1.0, 1.0, 1.0], None, 0.0);
        }
        // C++ parity: W3DGameWinDefaultDraw does NOT draw text here.
        // Text drawing is the responsibility of gadget-specific draw callbacks
        // (e.g., W3DGadgetPushButtonDraw, W3DGadgetStaticTextDraw) which call
        // drawButtonText() explicitly. The default draw only handles image/color
        // backgrounds and video buffers.
    });
}

pub(crate) fn resolve_window_text(raw_text: &str) -> String {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let localized = GameText::fetch(trimmed);
    if localized.is_empty() {
        trimmed.to_string()
    } else {
        localized
    }
}

pub(crate) struct VideoFrameData {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) data: Vec<u8>,
}

pub(crate) fn read_video_frame(buffer: &VideoBufferHandle) -> Option<VideoFrameData> {
    let mut guard = buffer.lock();
    if !guard.valid() {
        return None;
    }
    let width = guard.width();
    let height = guard.height();
    let pitch = guard.pitch();
    if width == 0 || height == 0 || pitch == 0 {
        return None;
    }
    let byte_len = (pitch as usize).saturating_mul(height as usize);
    let ptr = guard.lock();
    if ptr.is_null() || byte_len == 0 {
        guard.unlock();
        return None;
    }
    // SAFETY: ptr from guard.lock(), null-checked above; byte_len = pitch*height
    // SAFETY: bounds the locked backing store, and the slice dies before unlock().
    let src = unsafe { std::slice::from_raw_parts(ptr, byte_len) };
    let data = match guard.format() {
        VideoBufferType::X8R8G8B8 => convert_x8r8g8b8(src, width, height, pitch),
        VideoBufferType::R8G8B8 => convert_r8g8b8(src, width, height, pitch),
        VideoBufferType::R5G6B5 => convert_r5g6b5(src, width, height, pitch),
        VideoBufferType::X1R5G5B5 => convert_x1r5g5b5(src, width, height, pitch),
        VideoBufferType::Unknown => None,
    };
    guard.unlock();
    data.map(|data| VideoFrameData {
        width,
        height,
        data,
    })
}

pub(crate) fn convert_x8r8g8b8(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(4);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let src_idx = x * 4;
            let dst_idx = (y * width as usize + x) * 4;
            let b = row[src_idx];
            let g = row[src_idx + 1];
            let r = row[src_idx + 2];
            out[dst_idx] = r;
            out[dst_idx + 1] = g;
            out[dst_idx + 2] = b;
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

pub(crate) fn convert_r8g8b8(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(3);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let src_idx = x * 3;
            let dst_idx = (y * width as usize + x) * 4;
            out[dst_idx] = row[src_idx];
            out[dst_idx + 1] = row[src_idx + 1];
            out[dst_idx + 2] = row[src_idx + 2];
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

pub(crate) fn convert_r5g6b5(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(2);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let idx = x * 2;
            let value = u16::from_le_bytes([row[idx], row[idx + 1]]);
            let r = ((value >> 11) & 0x1F) as u8;
            let g = ((value >> 5) & 0x3F) as u8;
            let b = (value & 0x1F) as u8;
            let dst_idx = (y * width as usize + x) * 4;
            out[dst_idx] = (r << 3) | (r >> 2);
            out[dst_idx + 1] = (g << 2) | (g >> 4);
            out[dst_idx + 2] = (b << 3) | (b >> 2);
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

pub(crate) fn convert_x1r5g5b5(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(2);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let idx = x * 2;
            let value = u16::from_le_bytes([row[idx], row[idx + 1]]);
            let r = ((value >> 10) & 0x1F) as u8;
            let g = ((value >> 5) & 0x1F) as u8;
            let b = (value & 0x1F) as u8;
            let dst_idx = (y * width as usize + x) * 4;
            out[dst_idx] = (r << 3) | (r >> 2);
            out[dst_idx + 1] = (g << 3) | (g >> 2);
            out[dst_idx + 2] = (b << 3) | (b >> 2);
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

pub(crate) fn color_to_rgba(color: Color) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

pub fn default_input_callback(
    _window: &GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

pub fn default_system_callback(
    _window: &GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

pub fn default_tooltip_callback(
    _window: &GameWindow,
    _inst_data: &WindowInstanceData,
    _mouse: u32,
) {
    // Default implementation does nothing
}

pub(crate) fn map_keycode(data: WindowMsgData) -> KeyCode {
    let key = (data & 0xFF) as u8;
    match key {
        8 => KeyCode::Backspace,
        9 => KeyCode::Tab,
        13 => KeyCode::Enter,
        27 => KeyCode::Escape,
        32 => KeyCode::Space,
        33 => KeyCode::PageUp,
        34 => KeyCode::PageDown,
        35 => KeyCode::End,
        36 => KeyCode::Home,
        37 => KeyCode::Left,
        38 => KeyCode::Up,
        39 => KeyCode::Right,
        40 => KeyCode::Down,
        127 => KeyCode::Delete,
        b'0' => KeyCode::Num0,
        b'1' => KeyCode::Num1,
        b'2' => KeyCode::Num2,
        b'3' => KeyCode::Num3,
        b'4' => KeyCode::Num4,
        b'5' => KeyCode::Num5,
        b'6' => KeyCode::Num6,
        b'7' => KeyCode::Num7,
        b'8' => KeyCode::Num8,
        b'9' => KeyCode::Num9,
        b'a' | b'A' => KeyCode::A,
        b'b' | b'B' => KeyCode::B,
        b'c' | b'C' => KeyCode::C,
        b'd' | b'D' => KeyCode::D,
        b'e' | b'E' => KeyCode::E,
        b'f' | b'F' => KeyCode::F,
        b'g' | b'G' => KeyCode::G,
        b'h' | b'H' => KeyCode::H,
        b'i' | b'I' => KeyCode::I,
        b'j' | b'J' => KeyCode::J,
        b'k' | b'K' => KeyCode::K,
        b'l' | b'L' => KeyCode::L,
        b'm' | b'M' => KeyCode::M,
        b'n' | b'N' => KeyCode::N,
        b'o' | b'O' => KeyCode::O,
        b'p' | b'P' => KeyCode::P,
        b'q' | b'Q' => KeyCode::Q,
        b'r' | b'R' => KeyCode::R,
        b's' | b'S' => KeyCode::S,
        b't' | b'T' => KeyCode::T,
        b'u' | b'U' => KeyCode::U,
        b'v' | b'V' => KeyCode::V,
        b'w' | b'W' => KeyCode::W,
        b'x' | b'X' => KeyCode::X,
        b'y' | b'Y' => KeyCode::Y,
        b'z' | b'Z' => KeyCode::Z,
        _ => {
            let ch = key as char;
            KeyCode::Char(ch)
        }
    }
}

pub(crate) fn key_modifiers_from_state(state: WindowMsgData) -> KeyModifiers {
    KeyModifiers {
        shift: (state & (KEY_STATE_LSHIFT | KEY_STATE_RSHIFT)) != 0,
        ctrl: (state & (KEY_STATE_LCONTROL | KEY_STATE_RCONTROL)) != 0,
        alt: (state & (KEY_STATE_LALT | KEY_STATE_RALT)) != 0,
    }
}

pub(crate) fn char_input_event(key: WindowMsgData, state: WindowMsgData) -> Option<InputEvent> {
    let key = map_keycode(key);
    let modifiers = key_modifiers_from_state(state);

    if (state & KEY_STATE_DOWN) != 0 {
        Some(InputEvent::KeyDown { key, modifiers })
    } else if (state & KEY_STATE_UP) != 0 {
        Some(InputEvent::KeyUp { key, modifiers })
    } else {
        None
    }
}
