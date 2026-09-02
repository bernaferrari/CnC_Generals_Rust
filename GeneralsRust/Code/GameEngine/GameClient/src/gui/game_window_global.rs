//! GameWindow global helpers for drawing and font lookup.

use std::sync::atomic::{AtomicUsize, Ordering};

use glam::Vec2;

use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};

use super::DisplayStringManager;
use super::display_string::DisplayStringHandle;
use super::font::{FontDesc, get_font_library};
use super::game_window::{Color, GameFont, Image, WIN_COLOR_UNDEFINED};
use super::ui_globals::with_ui_renderer_mut;
use super::ui_renderer::UIRect;
use super::window_manager::WindowManager;

/// Visible `win_draw_image` submits (textured or untextured fill).
/// Tests can read this when UIRenderer/GPU is not bound.
static WIN_DRAW_IMAGE_COMMANDS: AtomicUsize = AtomicUsize::new(0);

pub fn win_draw_image_queued_count() -> usize {
    WIN_DRAW_IMAGE_COMMANDS.load(Ordering::Relaxed)
}

pub fn reset_win_draw_image_queued_count() {
    WIN_DRAW_IMAGE_COMMANDS.store(0, Ordering::Relaxed);
}

fn note_win_draw_image_command() {
    WIN_DRAW_IMAGE_COMMANDS.fetch_add(1, Ordering::Relaxed);
}

impl WindowManager {
    /// Draw an image in the provided screen rect.
    pub fn win_draw_image(
        &self,
        image: &Image,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        color: Color,
    ) {
        self.win_draw_image_ex(
            image,
            start_x,
            start_y,
            end_x,
            end_y,
            color,
            crate::display::DrawImageMode::Alpha,
        );
    }

    /// C++ `TheDisplay->drawImage(..., DrawImageMode)`.
    pub fn win_draw_image_ex(
        &self,
        image: &Image,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        color: Color,
        mode: crate::display::DrawImageMode,
    ) {
        let color_rgba = if color != WIN_COLOR_UNDEFINED {
            color_to_rgba(color)
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };
        let submitted = with_ui_renderer_mut(|renderer| {
            let mut found_mapped = false;
            let texture = {
                let _ = ensure_client_mapped_image(&image.name);
                let collection = get_mapped_image_collection();
                let mut collection = collection.write();
                if let Some(mapped) = collection.find_image_by_name_mut(&image.name) {
                    found_mapped = true;
                    if mapped.get_gpu_texture().is_none() {
                        let _ = mapped.create_gpu_texture(renderer.device(), renderer.queue());
                    }
                    mapped.get_gpu_texture().map(|gpu| {
                        let uv = mapped.get_uv();
                        let rotated = mapped
                            .get_status()
                            .contains(crate::display::image::ImageStatus::ROTATED_90_CLOCKWISE);
                        (
                            std::sync::Arc::new(gpu.view().clone()),
                            uv.min.x,
                            uv.min.y,
                            uv.max.x,
                            uv.max.y,
                            rotated,
                        )
                    })
                } else {
                    None
                }
            };
            if let Some((texture, u0, v0, u1, v1, rotated)) = texture {
                crate::display::display_fx::queue_draw_image_mesh_on(
                    renderer,
                    texture,
                    start_x as f32,
                    start_y as f32,
                    end_x as f32,
                    end_y as f32,
                    u0,
                    v0,
                    u1,
                    v1,
                    color_rgba,
                    mode,
                    rotated,
                );
                note_win_draw_image_command();
                true
            } else if found_mapped {
                // C++ W3DDisplay::drawImage treats DrawData COLOR as a modulate
                // tint on the image, never a standalone fill. When the mapped
                // image exists but its pixels could not be hydrated (e.g. the
                // retail atlas lives in EnglishZH.big and decode failed), a
                // MainMenu.wnd "Buttons-Left" draw would paint its authored
                // `255 0 0 255` tint as a solid RED rectangle. Render the
                // honest missing-art palette instead (w3d_gadget_draw parity).
                let rect = UIRect::new(
                    start_x as f32,
                    start_y as f32,
                    (end_x - start_x) as f32,
                    (end_y - start_y) as f32,
                );
                renderer.draw_rect(rect, color_to_rgba(0xFF3D4F63), 0.0);
                note_win_draw_image_command();
                true
            } else {
                false
            }
        });
        if submitted != Some(true) {
            let _ = ensure_client_mapped_image(&image.name);
            if get_mapped_image_collection()
                .read()
                .find_image_by_name(&image.name)
                .is_some()
            {
                note_win_draw_image_command();
            }
        }
    }

    /// Draw a filled rectangle using UI renderer.
    pub fn win_fill_rect(
        &self,
        color: Color,
        _width: f32,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let rect = UIRect::new(
            start_x as f32,
            start_y as f32,
            (end_x - start_x) as f32,
            (end_y - start_y) as f32,
        );
        let color = color_to_rgba(color);
        let _ = with_ui_renderer_mut(|renderer| {
            renderer.draw_rect(rect, color, 0.0);
        });
    }

    /// Draw a rectangle outline.
    pub fn win_open_rect(
        &self,
        color: Color,
        width: f32,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let rect = UIRect::new(
            start_x as f32,
            start_y as f32,
            (end_x - start_x) as f32,
            (end_y - start_y) as f32,
        );
        let color = color_to_rgba(color);
        let _ = with_ui_renderer_mut(|renderer| {
            renderer.draw_rect_outline(rect, width, color, 0.0);
        });
    }

    /// Draw a line between two points.
    pub fn win_draw_line(
        &self,
        color: Color,
        width: f32,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let _ = with_ui_renderer_mut(|renderer| {
            renderer.draw_line(
                Vec2::new(start_x as f32, start_y as f32),
                Vec2::new(end_x as f32, end_y as f32),
                width,
                color_to_rgba(color),
                0.0,
            );
        });
    }

    /// Draw a filled clock overlay (0-100%) matching W3DDisplay::drawRectClock.
    pub fn win_draw_rect_clock(
        &self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        percent: i32,
        color: Color,
    ) {
        if !(1..=100).contains(&percent) {
            return;
        }
        let color = color_to_rgba(color);
        let start_xf = start_x as f32;
        let start_yf = start_y as f32;
        let width_f = width as f32;
        let height_f = height as f32;
        let mid_x = start_xf + width_f * 0.5;
        let mid_y = start_yf + height_f * 0.5;
        let end_x = start_xf + width_f;
        let end_y = start_yf + height_f;

        let _ = with_ui_renderer_mut(|renderer| {
            let add_rect = |renderer: &mut super::ui_renderer::UIRenderer,
                            x0: f32,
                            y0: f32,
                            x1: f32,
                            y1: f32| {
                renderer.draw_rect(UIRect::new(x0, y0, x1 - x0, y1 - y0), color, 0.0);
            };
            let add_tri =
                |renderer: &mut super::ui_renderer::UIRenderer, p0: Vec2, p1: Vec2, p2: Vec2| {
                    renderer.draw_triangle(p0, p1, p2, color, 0.0);
                };

            if percent == 100 {
                add_rect(renderer, start_xf, start_yf, end_x, end_y);
                return;
            }

            if percent > 75 {
                add_rect(renderer, mid_x, start_yf, end_x, end_y);
                add_rect(renderer, start_xf, mid_y, mid_x, end_y);
                let remain = (percent - 75) as f32;
                if remain > 12.0 {
                    add_tri(
                        renderer,
                        Vec2::new(start_xf, start_yf),
                        Vec2::new(start_xf, mid_y),
                        Vec2::new(mid_x, mid_y),
                    );
                    let percent_draw = (remain - 12.0) / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(start_xf, start_yf),
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(start_xf + width_f * 0.5 * percent_draw, start_yf),
                    );
                } else {
                    let percent_draw = remain / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(start_xf, mid_y - (height_f * 0.5 * percent_draw)),
                        Vec2::new(start_xf, mid_y),
                        Vec2::new(mid_x, mid_y),
                    );
                }
            } else if percent > 50 {
                add_rect(renderer, mid_x, start_yf, end_x, end_y);
                let remain = (percent - 50) as f32;
                if remain > 12.0 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(start_xf, end_y),
                        Vec2::new(mid_x, end_y),
                    );
                    let percent_draw = (remain - 12.0) / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(start_xf, end_y - (height_f * 0.5 * percent_draw)),
                        Vec2::new(start_xf, end_y),
                        Vec2::new(mid_x, mid_y),
                    );
                } else {
                    let percent_draw = remain / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, end_y),
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(mid_x - (width_f * 0.5 * percent_draw), end_y),
                    );
                }
            } else if percent > 25 {
                add_rect(renderer, mid_x, start_yf, end_x, mid_y);
                let remain = (percent - 25) as f32;
                if remain > 12.0 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, end_y),
                        Vec2::new(end_x, mid_y),
                    );
                    let percent_draw = (remain - 12.0) / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x - (width_f * 0.5 * percent_draw), end_y),
                        Vec2::new(end_x, end_y),
                    );
                } else {
                    let percent_draw = remain / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(end_x, mid_y),
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, mid_y + (height_f * 0.5 * percent_draw)),
                    );
                }
            } else {
                let remain = percent as f32;
                if remain > 12.0 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, start_yf),
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, start_yf),
                    );
                    let percent_draw = (remain - 12.0) / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(end_x, start_yf),
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, start_yf + (height_f * 0.5 * percent_draw)),
                    );
                } else {
                    let percent_draw = remain / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, start_yf),
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(mid_x + (width_f * 0.5 * percent_draw), start_yf),
                    );
                }
            }
        });
    }

    /// Draw a remaining clock overlay (0-99%) matching W3DDisplay::drawRemainingRectClock.
    pub fn win_draw_remaining_rect_clock(
        &self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        percent: i32,
        color: Color,
    ) {
        if !(0..=99).contains(&percent) {
            return;
        }
        let color = color_to_rgba(color);
        let start_xf = start_x as f32;
        let start_yf = start_y as f32;
        let width_f = width as f32;
        let height_f = height as f32;
        let mid_x = start_xf + width_f * 0.5;
        let mid_y = start_yf + height_f * 0.5;
        let end_x = start_xf + width_f;
        let end_y = start_yf + height_f;
        let half_w = width_f * 0.5;
        let half_h = height_f * 0.5;

        let _ = with_ui_renderer_mut(|renderer| {
            let add_rect = |renderer: &mut super::ui_renderer::UIRenderer,
                            x0: f32,
                            y0: f32,
                            x1: f32,
                            y1: f32| {
                renderer.draw_rect(UIRect::new(x0, y0, x1 - x0, y1 - y0), color, 0.0);
            };
            let add_tri =
                |renderer: &mut super::ui_renderer::UIRenderer, p0: Vec2, p1: Vec2, p2: Vec2| {
                    renderer.draw_triangle(p0, p1, p2, color, 0.0);
                };

            if percent == 0 {
                add_rect(renderer, start_xf, start_yf, end_x, end_y);
                return;
            }

            if percent < 25 {
                add_rect(renderer, start_xf, start_yf, mid_x, end_y);
                add_rect(renderer, mid_x, mid_y, end_x, end_y);
                if percent < 13 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, mid_y),
                        Vec2::new(end_x, start_yf),
                    );
                    let percent_draw = (13 - percent) as f32 / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, start_yf),
                        Vec2::new(end_x - half_w * percent_draw, start_yf),
                    );
                } else {
                    let percent_draw = (percent - 13) as f32 / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, mid_y),
                        Vec2::new(end_x, start_yf + half_h * percent_draw),
                    );
                }
            } else if percent < 50 {
                add_rect(renderer, start_xf, start_yf, mid_x, end_y);
                if percent < 38 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(mid_x, end_y),
                        Vec2::new(end_x, end_y),
                    );
                    let percent_draw = (percent - 25) as f32 / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(end_x, end_y),
                        Vec2::new(end_x, mid_y + half_h * percent_draw),
                    );
                } else {
                    let percent_draw = (percent - 38) as f32 / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(mid_x, end_y),
                        Vec2::new(end_x - half_w * percent_draw, end_y),
                    );
                }
            } else if percent < 75 {
                add_rect(renderer, start_xf, start_yf, mid_x, mid_y);
                if percent < 63 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(start_xf, mid_y),
                        Vec2::new(start_xf, end_y),
                    );
                    let percent_draw = (percent - 50) as f32 / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(start_xf, end_y),
                        Vec2::new(mid_x - half_w * percent_draw, end_y),
                    );
                } else {
                    let percent_draw = (percent - 62) as f32 / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(start_xf, mid_y),
                        Vec2::new(start_xf, end_y - half_h * percent_draw),
                    );
                }
            } else {
                if percent < 87 {
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(mid_x, start_yf),
                        Vec2::new(start_xf, start_yf),
                    );
                    let percent_draw = (percent - 75) as f32 / 13.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(start_xf, start_yf),
                        Vec2::new(start_xf, mid_y - half_h * percent_draw),
                    );
                } else {
                    let percent_draw = (percent - 88) as f32 / 12.0;
                    add_tri(
                        renderer,
                        Vec2::new(mid_x, mid_y),
                        Vec2::new(mid_x, start_yf),
                        Vec2::new(start_xf + half_w * percent_draw, start_yf),
                    );
                }
            }
        });
    }

    /// Find an image by name using the mapped image collection.
    pub fn win_find_image(&self, name: &str) -> Option<Image> {
        let _ = ensure_client_mapped_image(name);
        let collection = get_mapped_image_collection();
        let collection = collection.read();
        let mapped = collection.find_image_by_name(name)?;
        let size = mapped.get_image_size();
        Some(Image {
            name: name.to_string(),
            width: size.x,
            height: size.y,
        })
    }

    /// Convert RGBA components into a packed color.
    pub fn win_make_color(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32)
    }

    /// Draw formatted text in the given bounds.
    pub fn win_format_text(
        &self,
        font: &GameFont,
        text: &str,
        color: Color,
        x: i32,
        y: i32,
        width: i32,
        _height: i32,
    ) {
        let display = create_display_string(font, text, width);
        display.borrow_mut().draw_with_drop(x, y, color, 0, 0, 0);
    }

    /// Get the text size for a font and string.
    pub fn win_get_text_size(
        &self,
        font: &GameFont,
        text: &str,
        width: Option<&mut i32>,
        height: Option<&mut i32>,
        max_width: i32,
    ) {
        let display = create_display_string(font, text, max_width);
        let (w, h) = display.borrow_mut().get_size();
        if let Some(width) = width {
            *width = w;
        }
        if let Some(height) = height {
            *height = h;
        }
    }

    /// Return the font height in pixels.
    pub fn win_font_height(&self, font: &GameFont) -> i32 {
        font.size
    }

    /// Check whether character is digit.
    pub fn win_is_digit(&self, c: i32) -> bool {
        (c as u8 as char).is_ascii_digit()
    }

    /// Check whether character is ASCII.
    pub fn win_is_ascii(&self, c: i32) -> bool {
        (c as u8 as char).is_ascii()
    }

    /// Check whether character is alphanumeric.
    pub fn win_is_alnum(&self, c: i32) -> bool {
        (c as u8 as char).is_ascii_alphanumeric()
    }

    /// Find a font by name.
    pub fn win_find_font(&self, font_name: &str, point_size: i32, bold: bool) -> Option<GameFont> {
        let desc = FontDesc::new(font_name, point_size, bold);
        if get_font_library().get_font(&desc).is_ok() {
            Some(GameFont {
                name: font_name.to_string(),
                size: point_size,
                bold,
            })
        } else {
            None
        }
    }
}

fn create_display_string(font: &GameFont, text: &str, max_width: i32) -> DisplayStringHandle {
    let mut manager = DisplayStringManager::new();
    let mut display = manager.new_display_string();
    if let Ok(font_ref) = get_font_library().get_font(&font.to_font_desc()) {
        display.borrow_mut().set_font(font_ref);
    }
    display.borrow_mut().set_text(text.to_string());
    if max_width > 0 {
        display.borrow_mut().set_word_wrap(max_width);
    }
    display
}

fn color_to_rgba(color: Color) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}
