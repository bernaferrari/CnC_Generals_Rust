//! W3D gadget draw callbacks (push button) for device-style rendering.

use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};
use crate::gui::callbacks::get_menu_manager;
use crate::gui::font::{get_font_library, FontDesc};
use crate::gui::gadgets::tabcontrol::{
    TP_BOTTOMRIGHT, TP_BOTTOM_SIDE, TP_CENTER, TP_LEFT_SIDE, TP_RIGHT_SIDE, TP_TOP_SIDE,
};
use crate::gui::gadgets::{ClockMode, PushButton, TabControl, TextAlignment, VerticalAlignment};
use crate::gui::game_window::{
    read_video_frame, resolve_window_text, Point2D, WindowState, WindowStatus, WIN_COLOR_UNDEFINED,
};
use crate::gui::shell::get_shell;
use crate::gui::ui_globals::with_ui_renderer_mut;
use crate::gui::ui_renderer::UIRect;
use crate::gui::window_manager::with_window_manager_ref;
use crate::gui::{GameWindow, WindowInstanceData};
use crate::helpers::TheControlBar;
use crate::map_util::{find_draw_positions, get_supply_and_tech_image_locations};
use crate::message_stream::game_message::IRegion2D;
use chrono::Local;
use game_engine::common::ini::get_control_bar_scheme_manager;
use game_engine::common::ini::get_global_data;
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::ini::set_scheme_draw_func;
use game_engine::common::ini::ICoord2D;
use game_engine::common::ini::SchemeDrawFunc;
use game_engine::common::system::radar::get_radar_system;
use gamelogic::player::{RankProgressInfo, ThePlayerList};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Draw callback for control bar scheme images.
/// Resolves image name via the window manager and draws the image.
fn scheme_draw_image(image_name: &str, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image(image_name) {
            manager.win_draw_image(&image, start_x, start_y, end_x, end_y, WIN_COLOR_UNDEFINED);
        }
    });
}

/// One-time initialization for scheme draw callback.
fn ensure_scheme_draw_registered() {
    static REGISTER_DRAW: OnceLock<()> = OnceLock::new();
    REGISTER_DRAW.get_or_init(|| {
        set_scheme_draw_func(scheme_draw_image);
    });
}

fn press_scaled_rect(window: &GameWindow) -> UIRect {
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let mut rect = UIRect::new(x as f32, y as f32, width as f32, height as f32);
    let scale = window.get_press_scale();
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
    rect
}

fn press_scaled_bounds_i32(window: &GameWindow) -> (i32, i32, i32, i32) {
    let rect = press_scaled_rect(window);
    (
        rect.x.round() as i32,
        rect.y.round() as i32,
        rect.width.round() as i32,
        rect.height.round() as i32,
    )
}

trait RgbaColor {
    fn rgba(self) -> (u8, u8, u8, u8);
}

impl RgbaColor for crate::gui::gadgets::Color {
    fn rgba(self) -> (u8, u8, u8, u8) {
        (self.r, self.g, self.b, self.a)
    }
}

impl RgbaColor for crate::gui::shell::Color {
    fn rgba(self) -> (u8, u8, u8, u8) {
        (self.r, self.g, self.b, self.a)
    }
}

fn gadget_color_to_win_color<C: RgbaColor>(color: C) -> u32 {
    let (r, g, b, a) = color.rgba();
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn gadget_color_opt_to_win_color<C: RgbaColor>(color: Option<C>) -> Option<u32> {
    color.map(gadget_color_to_win_color)
}

fn global_hotkey_text_color() -> u32 {
    get_global_data()
        .map(|global| global.read().hot_key_text_color)
        .map(|color| {
            ((1.0_f32.clamp(0.0, 1.0) * 255.0).round() as u32) << 24
                | ((color.r.clamp(0.0, 1.0) * 255.0).round() as u32) << 16
                | ((color.g.clamp(0.0, 1.0) * 255.0).round() as u32) << 8
                | (color.b.clamp(0.0, 1.0) * 255.0).round() as u32
        })
        .unwrap_or(0)
}

fn region_from_corners(x1: i32, y1: i32, x2: i32, y2: i32) -> IRegion2D {
    IRegion2D {
        x: x1,
        y: y1,
        width: (x2 - x1).max(0),
        height: (y2 - y1).max(0),
    }
}

fn region_right(region: &IRegion2D) -> i32 {
    region.x + region.width
}

fn region_bottom(region: &IRegion2D) -> i32 {
    region.y + region.height
}

fn draw_button_text(window: &GameWindow, inst_data: &WindowInstanceData) {
    let raw_text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    let text = resolve_window_text(raw_text);
    if text.is_empty() {
        return;
    }

    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let width = rect.width as i32;
    let height = rect.height as i32;
    let mut text_x = origin_x;
    let mut text_y = origin_y;

    if window.get_status().contains(WindowStatus::SHORTCUT_BUTTON) {
        text_x += 2;
    } else if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text.to_string());
        display.set_word_wrap(width);
        display.set_word_wrap_centered(window.get_status().contains(WindowStatus::WRAP_CENTERED));
        if let Some(font) = inst_data.font.as_ref() {
            display.set_font(font);
        }
        let (text_width, text_height) = display.get_size();
        text_x += (width / 2) - (text_width / 2);
        text_y += (height / 2) - (text_height / 2);
    } else {
        text_x += 2;
        text_y += 2;
    }

    let (text_color, border_color) =
        if !window.is_enabled() || inst_data.state.contains(WindowState::DISABLED) {
            (
                inst_data.disabled_text.color,
                inst_data.disabled_text.border_color,
            )
        } else if inst_data.state.contains(WindowState::HILITED) {
            (
                inst_data.hilite_text.color,
                inst_data.hilite_text.border_color,
            )
        } else {
            (
                inst_data.enabled_text.color,
                inst_data.enabled_text.border_color,
            )
        };

    if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text.clone());
        display.draw(text_x, text_y, text_color, border_color);
    } else {
        let _ = with_ui_renderer_mut(|renderer| {
            let font_size = inst_data.font.as_ref().map(|font| font.size).unwrap_or(12) as f32;
            if let Err(err) = renderer.draw_text_simple(
                &text,
                glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
                font_size,
                super::game_window::color_to_rgba(border_color),
            ) {
                log::warn!("W3DGadgetDraw text shadow render failed: {err}");
            }
            if let Err(err) = renderer.draw_text_simple(
                &text,
                glam::Vec2::new(text_x as f32, text_y as f32),
                font_size,
                super::game_window::color_to_rgba(text_color),
            ) {
                log::warn!("W3DGadgetDraw text render failed: {err}");
            }
        });
    }
}

fn draw_main_menu_button_drop_shadow_text(window: &GameWindow, inst_data: &WindowInstanceData) {
    let raw_text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    let text = resolve_window_text(raw_text);
    if text.is_empty() {
        return;
    }

    let (origin_x, origin_y, width, height) = press_scaled_bounds_i32(window);
    let (text_color, drop_color) =
        if !window.is_enabled() || inst_data.state.contains(WindowState::DISABLED) {
            (
                inst_data.disabled_text.color,
                inst_data.disabled_text.border_color,
            )
        } else if inst_data.state.contains(WindowState::HILITED) {
            (
                inst_data.hilite_text.color,
                inst_data.hilite_text.border_color,
            )
        } else {
            (
                inst_data.enabled_text.color,
                inst_data.enabled_text.border_color,
            )
        };

    if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text);
        display.set_word_wrap(width);
        display.set_word_wrap_centered(window.get_status().contains(WindowStatus::WRAP_CENTERED));
        if let Some(font) = inst_data.font.as_ref() {
            display.set_font(font);
        }
        let (text_width, text_height) = display.get_size();
        let text_x = origin_x + (width / 2) - (text_width / 2);
        let text_y = origin_y + (height / 2) - (text_height / 2);
        display.draw(text_x, text_y, text_color, drop_color);
        return;
    }

    let _ = with_ui_renderer_mut(|renderer| {
        let font_size = inst_data.font.as_ref().map(|font| font.size).unwrap_or(12) as f32;
        let text_width = (text.chars().count() as f32 * font_size * 0.6).round() as i32;
        let text_height = font_size.round() as i32;
        let text_x = origin_x + (width / 2) - (text_width / 2);
        let text_y = origin_y + (height / 2) - (text_height / 2);
        let _ = renderer.draw_text_simple(
            &text,
            glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
            font_size,
            super::game_window::color_to_rgba(drop_color),
        );
        let _ = renderer.draw_text_simple(
            &text,
            glam::Vec2::new(text_x as f32, text_y as f32),
            font_size,
            super::game_window::color_to_rgba(text_color),
        );
    });
}

#[derive(Debug)]
struct MainMenuPulseState {
    started_at: Instant,
    going_forward: bool,
    width: i32,
    x: i32,
    y: i32,
    initialized: bool,
}

fn main_menu_pulse_state() -> &'static Mutex<MainMenuPulseState> {
    static STATE: OnceLock<Mutex<MainMenuPulseState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(MainMenuPulseState {
            started_at: Instant::now(),
            going_forward: true,
            width: 0,
            x: -800,
            y: 0,
            initialized: false,
        })
    })
}

#[inline]
fn truncate_to_i32(value: f32) -> i32 {
    value as i32
}

fn ui_screen_height() -> i32 {
    with_ui_renderer_mut(|renderer| renderer.screen_size().1 as i32).unwrap_or(720)
}

fn draw_main_menu_frame(window: &GameWindow, vertical_ratios: &[f32]) {
    const COLOR: u32 = 0xFFA7865E;
    const COLOR_DROP: u32 = 0xFF261E15;

    let (pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let height = ui_screen_height();

    let top_horizontal_1 = (pos_x, pos_y, pos_x + size_x, pos_y);
    let top_horizontal_1_drop = (pos_x, pos_y + 1, pos_x + size_x, pos_y + 1);
    let top_horizontal_2 = (
        pos_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.1),
        pos_x + size_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.1),
    );
    let top_horizontal_2_drop = (
        pos_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.12),
        pos_x + size_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.12),
    );
    let bottom_horizontal_1 = (
        pos_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.9),
        pos_x + size_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.9),
    );
    let bottom_horizontal_1_drop = (
        pos_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.92),
        pos_x + size_x,
        pos_y + truncate_to_i32(size_y as f32 * 0.92),
    );
    let bottom_horizontal_2 = (pos_x, pos_y + size_y, pos_x + size_x, pos_y + size_y);
    let bottom_horizontal_2_drop = (
        pos_x,
        pos_y + size_y + 1,
        pos_x + size_x,
        pos_y + size_y + 1,
    );

    with_window_manager_ref(|manager| {
        for (x1, y1, x2, y2, width, color) in [
            (
                top_horizontal_1.0,
                top_horizontal_1.1,
                top_horizontal_1.2,
                top_horizontal_1.3,
                2.0,
                COLOR,
            ),
            (
                top_horizontal_1_drop.0,
                top_horizontal_1_drop.1,
                top_horizontal_1_drop.2,
                top_horizontal_1_drop.3,
                2.0,
                COLOR_DROP,
            ),
            (
                top_horizontal_2.0,
                top_horizontal_2.1,
                top_horizontal_2.2,
                top_horizontal_2.3,
                1.0,
                COLOR,
            ),
            (
                top_horizontal_2_drop.0,
                top_horizontal_2_drop.1,
                top_horizontal_2_drop.2,
                top_horizontal_2_drop.3,
                1.0,
                COLOR_DROP,
            ),
            (
                bottom_horizontal_1.0,
                bottom_horizontal_1.1,
                bottom_horizontal_1.2,
                bottom_horizontal_1.3,
                1.0,
                COLOR,
            ),
            (
                bottom_horizontal_1_drop.0,
                bottom_horizontal_1_drop.1,
                bottom_horizontal_1_drop.2,
                bottom_horizontal_1_drop.3,
                1.0,
                COLOR_DROP,
            ),
            (
                bottom_horizontal_2.0,
                bottom_horizontal_2.1,
                bottom_horizontal_2.2,
                bottom_horizontal_2.3,
                2.0,
                COLOR,
            ),
            (
                bottom_horizontal_2_drop.0,
                bottom_horizontal_2_drop.1,
                bottom_horizontal_2_drop.2,
                bottom_horizontal_2_drop.3,
                2.0,
                COLOR_DROP,
            ),
        ] {
            manager.win_draw_line(color, width, x1, y1, x2, y2);
        }

        for ratio in vertical_ratios {
            let x = pos_x + truncate_to_i32(size_x as f32 * ratio);
            manager.win_draw_line(COLOR, 3.0, x, pos_y, x, height);
        }
    });
}

fn animate_main_menu_pulse(window: &GameWindow, pulse_image_name: &str) {
    let Some(image) = with_window_manager_ref(|manager| manager.win_find_image(pulse_image_name))
    else {
        return;
    };

    let (_pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let mut state = main_menu_pulse_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !state.initialized {
        state.width = size_x + image.width;
        state.x = -800;
        state.y = pos_y - (image.height / 2);
        state.started_at = Instant::now();
        state.going_forward = true;
        state.initialized = true;
    }

    let elapsed = state.started_at.elapsed().as_secs_f32();
    let percent_done = (elapsed / 10.0).clamp(0.0, 1.0);

    if state.going_forward {
        if percent_done >= 1.0 {
            state.y = pos_y + size_y - (image.height / 2);
            state.started_at = Instant::now();
            state.going_forward = false;
        } else {
            state.y = pos_y - (image.height / 2);
            state.x = truncate_to_i32(percent_done * state.width as f32) - image.width;
        }
    } else {
        if percent_done >= 1.0 {
            state.y = pos_y - (image.height / 2);
            state.started_at = Instant::now();
            state.going_forward = true;
        } else {
            state.y = pos_y + size_y - (image.height / 2);
            state.x = size_x - truncate_to_i32(percent_done * state.width as f32);
        }
    }

    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &image,
            state.x,
            state.y,
            state.x + image.width,
            state.y + image.height,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub fn w3d_main_menu_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    draw_main_menu_frame(window, &[0.225, 0.445, 0.6662, 0.885]);
    animate_main_menu_pulse(window, "MainMenuPulse");
}

pub fn w3d_main_menu_four_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    draw_main_menu_frame(window, &[0.295, 0.59, 0.885]);
    animate_main_menu_pulse(window, "MainMenuPulse");
}

pub fn w3d_metal_bar_menu_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    window.draw_border_w3d();
}

pub fn w3d_main_menu_map_border(window: &GameWindow, _inst_data: &WindowInstanceData) {
    const BORDER_CORNER_SIZE: i32 = 10;
    const BORDER_LINE_SIZE: i32 = 20;
    const SIZE: i32 = 20;
    const HALF_SIZE: i32 = SIZE / 2;

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let max_x = x + width;
    let max_y = y + height;

    with_window_manager_ref(|manager| {
        let mut drew_any_piece = false;

        if let Some(image) = manager.win_find_image("FrameCornerHorizontal") {
            drew_any_piece = true;
            let top_y = y - BORDER_CORNER_SIZE;
            let bottom_y = max_y - BORDER_CORNER_SIZE;
            let mut draw_x = x + BORDER_CORNER_SIZE;
            let limit_x = max_x - (BORDER_CORNER_SIZE + BORDER_LINE_SIZE);
            while draw_x <= limit_x {
                manager.win_draw_image(
                    &image,
                    draw_x,
                    top_y,
                    draw_x + SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &image,
                    draw_x,
                    bottom_y,
                    draw_x + SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                draw_x += BORDER_LINE_SIZE;
            }
            let border_end = max_x - BORDER_CORNER_SIZE;
            if (border_end - draw_x) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &image,
                    draw_x,
                    top_y,
                    draw_x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &image,
                    draw_x,
                    bottom_y,
                    draw_x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                draw_x += BORDER_LINE_SIZE / 2;
            }
            if draw_x < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - draw_x) + 1) & !1);
                draw_x -= adjust;
                manager.win_draw_image(
                    &image,
                    draw_x,
                    top_y,
                    draw_x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &image,
                    draw_x,
                    bottom_y,
                    draw_x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        if let Some(image) = manager.win_find_image("FrameCornerVertical") {
            drew_any_piece = true;
            let left_x = x - BORDER_CORNER_SIZE;
            let right_x = max_x - BORDER_CORNER_SIZE;
            let mut draw_y = y + BORDER_CORNER_SIZE;
            let limit_y = max_y - (BORDER_CORNER_SIZE + BORDER_LINE_SIZE);
            while draw_y <= limit_y {
                manager.win_draw_image(
                    &image,
                    left_x,
                    draw_y,
                    left_x + SIZE,
                    draw_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &image,
                    right_x,
                    draw_y,
                    right_x + SIZE,
                    draw_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                draw_y += BORDER_LINE_SIZE;
            }
            let border_end = max_y - BORDER_CORNER_SIZE;
            if (border_end - draw_y) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &image,
                    left_x,
                    draw_y,
                    left_x + SIZE,
                    draw_y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &image,
                    right_x,
                    draw_y,
                    right_x + SIZE,
                    draw_y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                draw_y += BORDER_LINE_SIZE / 2;
            }
            if draw_y < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - draw_y) + 1) & !1);
                draw_y -= adjust;
                manager.win_draw_image(
                    &image,
                    left_x,
                    draw_y,
                    left_x + SIZE,
                    draw_y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &image,
                    right_x,
                    draw_y,
                    right_x + SIZE,
                    draw_y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        for (name, draw_x, draw_y) in [
            (
                "FrameCornerUL",
                x - BORDER_CORNER_SIZE,
                y - BORDER_CORNER_SIZE,
            ),
            (
                "FrameCornerUR",
                max_x - BORDER_CORNER_SIZE,
                y - BORDER_CORNER_SIZE,
            ),
            (
                "FrameCornerLL",
                x - BORDER_CORNER_SIZE,
                max_y - BORDER_CORNER_SIZE,
            ),
            (
                "FrameCornerLR",
                max_x - BORDER_CORNER_SIZE,
                max_y - BORDER_CORNER_SIZE,
            ),
        ] {
            if let Some(image) = manager.win_find_image(name) {
                drew_any_piece = true;
                manager.win_draw_image(
                    &image,
                    draw_x,
                    draw_y,
                    draw_x + SIZE,
                    draw_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        if !drew_any_piece {
            const COLOR: u32 = 0xFF5E86A7;
            const COLOR_DROP: u32 = 0xFF151E26;

            let left = x - BORDER_CORNER_SIZE;
            let top = y - BORDER_CORNER_SIZE;
            let right = max_x + BORDER_CORNER_SIZE;
            let bottom = max_y + BORDER_CORNER_SIZE;

            manager.win_draw_line(COLOR, 1.0, left, top, right, top);
            manager.win_draw_line(COLOR_DROP, 1.0, left, top + 1, right, top + 1);
            manager.win_draw_line(COLOR, 1.0, left, bottom, right, bottom);
            manager.win_draw_line(COLOR_DROP, 1.0, left, bottom - 1, right, bottom - 1);
            manager.win_draw_line(COLOR, 1.0, left, top, left, bottom);
            manager.win_draw_line(COLOR_DROP, 1.0, left + 1, top, left + 1, bottom);
            manager.win_draw_line(COLOR, 1.0, right, top, right, bottom);
            manager.win_draw_line(COLOR_DROP, 1.0, right - 1, top, right - 1, bottom);
        }
    });
}

pub fn w3d_main_menu_button_drop_shadow_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_push_button_base(window, inst_data);
    draw_main_menu_button_drop_shadow_text(window, inst_data);
    draw_video_buffer(window, inst_data);
    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::PushButton(button) = widget {
            draw_button_style_overlay(window, button);
        }
    }
    draw_button_overlays(window, inst_data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_to_i32_matches_cpp_cast_behavior() {
        assert_eq!(truncate_to_i32(76.8), 76);
        assert_eq!(truncate_to_i32(76.2), 76);
        assert_eq!(truncate_to_i32(-3.7), -3);
    }

    #[test]
    fn progress_bar_image_draws_match_cpp_piece_layout() {
        let draws = progress_bar_image_draws(0, 0, 100, 20, 0, 0, 30, 5, 5, 8, 10, 10);

        assert_eq!(
            &draws[0..3],
            &[
                ProgressBarImageDraw {
                    image_slot: 2,
                    start_x: 5,
                    start_y: 0,
                    end_x: 13,
                    end_y: 20,
                    clip: None,
                },
                ProgressBarImageDraw {
                    image_slot: 2,
                    start_x: 13,
                    start_y: 0,
                    end_x: 21,
                    end_y: 20,
                    clip: None,
                },
                ProgressBarImageDraw {
                    image_slot: 2,
                    start_x: 21,
                    start_y: 0,
                    end_x: 29,
                    end_y: 20,
                    clip: None,
                },
            ]
        );
        assert!(draws.iter().any(|draw| {
            draw.image_slot == 2
                && draw.start_x == 93
                && draw.end_x == 101
                && draw.clip == Some(region_from_corners(93, 0, 95, 20))
        }));
        assert!(draws.iter().any(|draw| {
            draw.image_slot == 0
                && draw.start_x == 0
                && draw.end_x == 5
                && draw.start_y == 0
                && draw.end_y == 20
        }));
        assert!(draws.iter().any(|draw| {
            draw.image_slot == 1
                && draw.start_x == 95
                && draw.end_x == 100
                && draw.start_y == 0
                && draw.end_y == 20
        }));

        let filled_bar: Vec<_> = draws
            .iter()
            .filter(|draw| draw.image_slot == 6)
            .map(|draw| (draw.start_x, draw.end_x, draw.start_y, draw.end_y))
            .collect();
        assert_eq!(filled_bar, vec![(10, 20, 5, 15), (20, 30, 5, 15)]);

        let empty_bar: Vec<_> = draws
            .iter()
            .filter(|draw| draw.image_slot == 5)
            .map(|draw| (draw.start_x, draw.end_x, draw.start_y, draw.end_y))
            .collect();
        assert_eq!(
            empty_bar,
            vec![
                (30, 40, 5, 15),
                (40, 50, 5, 15),
                (50, 60, 5, 15),
                (60, 70, 5, 15),
                (70, 80, 5, 15),
                (80, 90, 5, 15),
            ]
        );
    }

    #[test]
    fn progress_bar_image_draws_preserve_cpp_offsets() {
        let draws = progress_bar_image_draws(20, 30, 60, 14, 3, 2, 50, 4, 6, 9, 5, 7);

        assert_eq!(
            draws.iter().find(|draw| draw.image_slot == 0).unwrap(),
            &ProgressBarImageDraw {
                image_slot: 0,
                start_x: 23,
                start_y: 32,
                end_x: 27,
                end_y: 46,
                clip: None,
            }
        );
        assert_eq!(
            draws.iter().find(|draw| draw.image_slot == 1).unwrap(),
            &ProgressBarImageDraw {
                image_slot: 1,
                start_x: 77,
                start_y: 32,
                end_x: 83,
                end_y: 46,
                clip: None,
            }
        );
        assert!(draws.iter().any(|draw| {
            draw.image_slot == 6
                && draw.start_x == 30
                && draw.start_y == 37
                && draw.end_x == 35
                && draw.end_y == 41
        }));
    }

    #[test]
    fn combobox_title_text_matches_cpp_empty_title_gate() {
        let mut inst_data = WindowInstanceData::default();
        assert_eq!(combobox_title_text(&inst_data), None);

        inst_data.text_label = "GUI:Map".to_string();
        assert_eq!(combobox_title_text(&inst_data), Some("GUI:Map"));

        inst_data.text = "Direct".to_string();
        assert_eq!(combobox_title_text(&inst_data), Some("Direct"));

        inst_data.text.clear();
        inst_data.text_label.clear();
        assert_eq!(combobox_title_text(&inst_data), None);
    }

    #[test]
    fn radio_button_image_slots_match_cpp_state_order() {
        assert_eq!(
            radio_button_image_slots(true, false, false),
            RadioButtonImageSlots {
                state: RadioDrawState::Selected,
                left: 3,
                center: 4,
                right: 5,
            }
        );
        assert_eq!(
            radio_button_image_slots(false, false, true).state,
            RadioDrawState::Disabled
        );
        assert_eq!(
            radio_button_image_slots(false, true, true).state,
            RadioDrawState::Hilite
        );
        assert_eq!(
            radio_button_image_slots(false, true, false).state,
            RadioDrawState::Enabled
        );
    }

    #[test]
    fn checkbox_box_image_slot_matches_cpp_checked_state() {
        assert_eq!(checkbox_box_image_slot(false), 1);
        assert_eq!(checkbox_box_image_slot(true), 2);
    }

    #[test]
    fn checkbox_box_image_rect_matches_cpp_offsets() {
        assert_eq!(
            checkbox_box_image_rect(10, 20, 18, Point2D { x: 4, y: 9 }),
            CheckBoxImageRect {
                start_x: 14,
                start_y: 23,
                end_x: 26,
                end_y: 35,
            }
        );
    }

    #[test]
    fn static_text_draw_state_matches_cpp_enabled_only_choice() {
        assert_eq!(
            static_text_draw_state(true, WindowState::HILITED | WindowState::PUSHED),
            BinaryDrawState::Enabled
        );
        assert_eq!(
            static_text_draw_state(false, WindowState::NONE),
            BinaryDrawState::Disabled
        );
    }

    #[test]
    fn static_text_image_rect_preserves_cpp_offsets() {
        assert_eq!(
            static_text_image_rect(10, 20, 40, 12, Point2D { x: 3, y: 5 }),
            StaticTextImageRect {
                start_x: 13,
                start_y: 25,
                end_x: 53,
                end_y: 37,
            }
        );
    }

    #[test]
    fn static_text_skips_undefined_text_color_like_cpp() {
        assert!(!static_text_should_draw_text(WIN_COLOR_UNDEFINED));
        assert!(static_text_should_draw_text(0xff00ff00));
    }

    #[test]
    fn text_entry_image_draws_match_cpp_four_piece_layout() {
        assert_eq!(
            text_entry_image_draws(
                10,
                20,
                100,
                14,
                Point2D { x: 2, y: 3 },
                TextEntryImageMetrics {
                    left_width: 7,
                    right_width: 9,
                    center_width: 10,
                    small_center_width: 4,
                },
            ),
            vec![
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 19,
                    start_y: 23,
                    end_x: 29,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 29,
                    start_y: 23,
                    end_x: 39,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 39,
                    start_y: 23,
                    end_x: 49,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 49,
                    start_y: 23,
                    end_x: 59,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 59,
                    start_y: 23,
                    end_x: 69,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 69,
                    start_y: 23,
                    end_x: 79,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 79,
                    start_y: 23,
                    end_x: 89,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 2,
                    start_x: 89,
                    start_y: 23,
                    end_x: 99,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 3,
                    start_x: 99,
                    start_y: 23,
                    end_x: 103,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 3,
                    start_x: 103,
                    start_y: 23,
                    end_x: 107,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 0,
                    start_x: 12,
                    start_y: 23,
                    end_x: 19,
                    end_y: 37,
                },
                TextEntryImageDraw {
                    slot: 1,
                    start_x: 103,
                    start_y: 23,
                    end_x: 112,
                    end_y: 37,
                },
            ]
        );
    }

    #[test]
    fn horizontal_slider_box_image_sources_match_cpp() {
        assert_eq!(
            horizontal_slider_box_image_sources(),
            HorizontalSliderBoxImageSources {
                filled_bank: SliderImageBank::Disabled,
                filled_slot: 0,
                blank_bank: SliderImageBank::Disabled,
                blank_slot: 1,
                highlight_bank: SliderImageBank::Hilite,
                highlight_slot: 0,
            }
        );
    }

    #[test]
    fn list_box_selected_image_slots_match_cpp_gate() {
        assert_eq!(
            list_box_selected_image_slots([true, true, true, true]),
            Some([1, 2, 3, 4])
        );
        assert_eq!(
            list_box_selected_image_slots([true, true, false, true]),
            None
        );
    }

    #[test]
    fn list_box_selected_image_rect_matches_cpp_clip() {
        let clip = region_from_corners(11, 17, 79, 50);
        assert_eq!(
            list_box_selected_image_rect(10, 14, 70, 9, &clip),
            Some(ListBoxSelectedImageRect {
                start_x: 11,
                start_y: 17,
                end_x: 80,
                end_y: 23,
            })
        );
        assert_eq!(list_box_selected_image_rect(10, 55, 70, 9, &clip), None);
    }

    #[test]
    fn list_box_slider_width_adjustment_matches_cpp_draw_modes() {
        assert_eq!(
            list_box_adjusted_width_for_slider(100, Some(12), false, ListBoxDrawMode::Color),
            85
        );
        assert_eq!(
            list_box_adjusted_width_for_slider(100, Some(12), true, ListBoxDrawMode::Color),
            100
        );
        assert_eq!(
            list_box_adjusted_width_for_slider(100, Some(12), true, ListBoxDrawMode::Image),
            88
        );
        assert_eq!(
            list_box_adjusted_width_for_slider(100, None, false, ListBoxDrawMode::Image),
            100
        );
    }

    #[test]
    fn push_button_color_entry_index_matches_cpp_selected_slot() {
        assert_eq!(push_button_color_entry_index(false), 0);
        assert_eq!(push_button_color_entry_index(true), 1);
    }

    #[test]
    fn push_button_one_image_source_matches_cpp_state_order() {
        assert_eq!(
            push_button_one_image_source(false, false, true, false),
            (PushButtonImageBank::Disabled, 1)
        );
        assert_eq!(
            push_button_one_image_source(true, true, false, false),
            (PushButtonImageBank::Hilite, 0)
        );
        assert_eq!(
            push_button_one_image_source(true, true, true, false),
            (PushButtonImageBank::Hilite, 1)
        );
        assert_eq!(
            push_button_one_image_source(true, false, true, false),
            (PushButtonImageBank::Hilite, 1)
        );
        assert_eq!(
            push_button_one_image_source(false, true, true, true),
            (PushButtonImageBank::Enabled, 0)
        );
    }

    #[test]
    fn push_button_three_piece_slots_match_cpp_without_fallback() {
        assert_eq!(push_button_three_piece_slots(false), (0, 5, 6));
        assert_eq!(push_button_three_piece_slots(true), (1, 3, 4));
    }

    #[test]
    fn push_button_three_piece_layout_matches_cpp_overlap_offsets() {
        assert_eq!(
            push_button_three_piece_image_draws(10, 20, 30, 12, 3, 2, 20, 18, 5),
            vec![
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Left,
                    start_x: 13,
                    start_y: 22,
                    end_x: 28,
                    end_y: 34,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Right,
                    start_x: 28,
                    start_y: 22,
                    end_x: 40,
                    end_y: 34,
                    clip: None,
                },
            ]
        );
    }

    #[test]
    fn push_button_three_piece_layout_matches_cpp_center_clip() {
        assert_eq!(
            push_button_three_piece_image_draws(10, 20, 60, 12, 3, 2, 8, 9, 7),
            vec![
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 21,
                    start_y: 22,
                    end_x: 28,
                    end_y: 36,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 28,
                    start_y: 22,
                    end_x: 35,
                    end_y: 36,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 35,
                    start_y: 22,
                    end_x: 42,
                    end_y: 36,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 42,
                    start_y: 22,
                    end_x: 49,
                    end_y: 36,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 49,
                    start_y: 22,
                    end_x: 56,
                    end_y: 36,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 56,
                    start_y: 22,
                    end_x: 63,
                    end_y: 36,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Center,
                    start_x: 63,
                    start_y: 22,
                    end_x: 70,
                    end_y: 36,
                    clip: Some(region_from_corners(63, 22, 64, 36)),
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Left,
                    start_x: 13,
                    start_y: 22,
                    end_x: 21,
                    end_y: 34,
                    clip: None,
                },
                PushButtonThreeImageDraw {
                    part: PushButtonThreeImagePart::Right,
                    start_x: 64,
                    start_y: 22,
                    end_x: 73,
                    end_y: 34,
                    clip: None,
                },
            ]
        );
    }

    #[test]
    fn push_button_status_overlays_match_cpp_draw_scope() {
        assert!(!push_button_draw_mode_runs_status_overlays(
            PushButtonDrawMode::Color
        ));
        assert!(push_button_draw_mode_runs_status_overlays(
            PushButtonDrawMode::OneImage
        ));
        assert!(!push_button_draw_mode_runs_status_overlays(
            PushButtonDrawMode::ThreeImage
        ));
    }
}

pub fn w3d_main_menu_random_text_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let raw_text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    let text = resolve_window_text(raw_text);
    if text.is_empty() {
        return;
    }

    let (origin_x, origin_y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let clip_region = IRegion2D {
        x: origin_x + 1,
        y: origin_y + 1,
        width: (width - 2).max(0),
        height: (height - 2).max(0),
    };

    if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text);
        display.set_word_wrap(0);
        display.set_word_wrap_centered(false);
        if let Some(font) = inst_data.font.as_ref() {
            display.set_font(font);
        }
        let (_, text_height) = display.get_size();
        let text_y = origin_y + (height / 2) - (text_height / 2);
        display.set_clip_region(Some(clip_region));
        display.draw_with_drop(
            origin_x,
            text_y,
            inst_data.disabled_text.color,
            inst_data.disabled_text.border_color,
            1,
            1,
        );
        display.set_clip_region(None);
        return;
    }

    let _ = with_ui_renderer_mut(|renderer| {
        let font_size = inst_data.font.as_ref().map(|font| font.size).unwrap_or(12) as f32;
        let text_height = font_size.round() as i32;
        let text_y = origin_y + (height / 2) - (text_height / 2);
        let scissor = UIRect::new(
            clip_region.x as f32,
            clip_region.y as f32,
            clip_region.width as f32,
            clip_region.height as f32,
        );
        let _ = renderer.draw_text_simple_with_scissor(
            &text,
            glam::Vec2::new((origin_x + 1) as f32, (text_y + 1) as f32),
            font_size,
            super::game_window::color_to_rgba(inst_data.disabled_text.border_color),
            scissor,
        );
        let _ = renderer.draw_text_simple_with_scissor(
            &text,
            glam::Vec2::new(origin_x as f32, text_y as f32),
            font_size,
            super::game_window::color_to_rgba(inst_data.disabled_text.color),
            scissor,
        );
    });
}

pub fn w3d_thin_border_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let Some(draw_data) = window.get_enabled_draw_data(0) else {
        return;
    };
    let Some(image) = draw_data.image else {
        return;
    };

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &image,
            x + inst_data.image_offset.x,
            y + inst_data.image_offset.y,
            x + inst_data.image_offset.x + width,
            y + inst_data.image_offset.y + height,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub fn w3d_shell_menu_scheme_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    let mut shell = get_shell();
    if shell.is_shell_active() {
        shell.get_shell_menu_scheme_manager().draw();
    }
}

pub fn w3d_credits_menu_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    let manager = get_menu_manager();
    let Ok(manager) = manager.read() else {
        return;
    };
    let menu = manager.get_credits_menu();
    let Ok(mut menu) = menu.write() else {
        return;
    };
    menu.draw();
}

fn draw_data_has_compat_default_content(entry: &crate::gui::game_window::WindowDrawData) -> bool {
    entry.image.is_some()
        || entry.color != WIN_COLOR_UNDEFINED
        || entry.border_color != WIN_COLOR_UNDEFINED
}

fn has_compat_default_content(window: &GameWindow, inst_data: &WindowInstanceData) -> bool {
    window.get_status().contains(WindowStatus::IMAGE)
        || inst_data.video_buffer.is_some()
        || inst_data
            .enabled_draw_data
            .iter()
            .any(draw_data_has_compat_default_content)
        || inst_data
            .disabled_draw_data
            .iter()
            .any(draw_data_has_compat_default_content)
        || inst_data
            .hilite_draw_data
            .iter()
            .any(draw_data_has_compat_default_content)
}

pub fn w3d_no_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    if has_compat_default_content(window, inst_data) {
        super::game_window::default_draw_callback(window, inst_data);
    }
}

pub fn w3d_compat_default_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    super::game_window::default_draw_callback(window, inst_data);
}

pub fn w3d_clock_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    super::game_window::default_draw_callback(window, inst_data);

    let datestr = Local::now().format("%H:%M:%S").to_string();
    let font = get_font_library()
        .get_font(&FontDesc::new("Arial", 16, false))
        .ok();
    let text_width = font
        .as_ref()
        .map(|font| font.measure_text(&datestr))
        .unwrap_or((datestr.len() as i32 * 10).max(1));
    let text_height = font
        .as_ref()
        .map(|font| font.get_line_height())
        .unwrap_or(16);

    let (pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let text_x = pos_x + (size_x / 2) - (text_width / 2);
    let text_y = pos_y + (size_y / 2) - (text_height / 2);
    let scissor = UIRect::new(
        (pos_x + 1) as f32,
        (pos_y + 1) as f32,
        (size_x - 2).max(0) as f32,
        (size_y - 2).max(0) as f32,
    );

    let _ = with_ui_renderer_mut(|renderer| {
        let font_size = font.as_ref().map(|font| font.desc.size).unwrap_or(16) as f32;
        let _ = renderer.draw_text_simple_with_scissor(
            &datestr,
            glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
            font_size,
            [0.0, 0.0, 0.0, 1.0],
            scissor,
        );
        let _ = renderer.draw_text_simple_with_scissor(
            &datestr,
            glam::Vec2::new(text_x as f32, text_y as f32),
            font_size,
            [1.0, 1.0, 1.0, 1.0],
            scissor,
        );
    });
}

pub fn w3d_cameo_movie_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_video_buffer(window, inst_data);
}

/// Check if radar should be drawn (helper function to avoid lifetime issues)
fn should_draw_radar_check() -> bool {
    let radar_system = get_radar_system();
    let Ok(radar) = radar_system.read() else {
        return false;
    };

    if radar.is_radar_forced() {
        return true;
    }

    if radar.is_radar_hidden() {
        return false;
    }

    // Check if local player has radar
    let Ok(list) = ThePlayerList().read() else {
        return false;
    };

    let player_arc = TheControlBar::get_observer_look_at_player_index()
        .and_then(|index| {
            if index >= 0 {
                list.get_player(index as i32).cloned()
            } else {
                None
            }
        })
        .or_else(|| list.get_local_player().cloned());

    if let Some(player_arc) = player_arc {
        if let Ok(player) = player_arc.read() {
            return player.has_radar();
        }
    }

    false
}

pub fn w3d_left_hud_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    // First check for video buffer (in-game movies)
    if inst_data
        .video_buffer
        .as_ref()
        .and_then(read_video_frame)
        .is_some()
    {
        draw_video_buffer(window, inst_data);
        return;
    }

    // C++ parity: check if radar should be drawn
    // W3DLeftHUDDraw draws radar when:
    // - TheRadar->isRadarForced() OR
    // - (!TheRadar->isRadarHidden() AND player->hasRadar())
    if should_draw_radar_check() {
        // Get window position and size for radar drawing
        let (pos_x, pos_y) = window.get_screen_position();
        let (size_x, size_y) = window.get_size();

        // Draw radar with 1-pixel border (matching C++ TheRadar->draw(pos.x + 1, pos.y + 1, size.x - 2, size.y - 2))
        draw_radar_in_hud(
            pos_x + 1,
            pos_y + 1,
            size_x.saturating_sub(2),
            size_y.saturating_sub(2),
        );
    } else {
        // Fall back to default drawing when no radar
        super::game_window::default_draw_callback(window, inst_data);
    }
}

/// Draw radar in the HUD area (matches C++ TheRadar->draw())
fn draw_radar_in_hud(x: i32, y: i32, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }

    let radar_system = get_radar_system();
    let Ok(radar) = radar_system.read() else {
        return;
    };

    // Draw terrain texture from radar system
    let terrain_texture = radar.get_terrain_texture();
    if terrain_texture.is_empty() {
        return;
    }

    let _ = with_ui_renderer_mut(|renderer| {
        let texture = renderer.create_texture_from_rgba(
            game_engine::common::system::radar::RADAR_CELL_WIDTH,
            game_engine::common::system::radar::RADAR_CELL_HEIGHT,
            &terrain_texture,
        );

        // Draw the radar texture scaled to the HUD area
        let rect = UIRect::new(x as f32, y as f32, width as f32, height as f32);
        renderer.draw_textured_rect(rect, texture, [1.0, 1.0, 1.0, 1.0], None, 0.0);

        // Draw radar objects (units, structures)
        for obj in radar.get_all_objects() {
            // Skip objects that are temporarily hidden (stealthed/jammed) or not visible on radar
            if obj.is_temporarily_hidden() || !obj.priority.is_visible() {
                continue;
            }

            // Convert world position to radar screen coordinates
            if let Some(radar_pos) = radar.world_to_radar(&obj.world_pos) {
                let screen_x = x
                    + (radar_pos.x as i64 * width as i64
                        / game_engine::common::system::radar::RADAR_CELL_WIDTH as i64)
                        as i32;
                let screen_y = y
                    + (radar_pos.y as i64 * height as i64
                        / game_engine::common::system::radar::RADAR_CELL_HEIGHT as i64)
                        as i32;

                // Draw object as a small colored dot
                let dot_size = 2;
                // Convert u32 color to RGBA [f32; 4] format
                let color_rgba = [
                    ((obj.color >> 16) & 0xFF) as f32 / 255.0,
                    ((obj.color >> 8) & 0xFF) as f32 / 255.0,
                    (obj.color & 0xFF) as f32 / 255.0,
                    ((obj.color >> 24) & 0xFF) as f32 / 255.0,
                ];
                renderer.draw_rect(
                    UIRect::new(
                        (screen_x - dot_size) as f32,
                        (screen_y - dot_size) as f32,
                        (dot_size * 2) as f32,
                        (dot_size * 2) as f32,
                    ),
                    color_rgba,
                    0.0,
                );
            }
        }

        // Draw active radar events
        for event in radar.get_active_events() {
            let screen_x = x
                + (event.radar_loc.x as i64 * width as i64
                    / game_engine::common::system::radar::RADAR_CELL_WIDTH as i64)
                    as i32;
            let screen_y = y
                + (event.radar_loc.y as i64 * height as i64
                    / game_engine::common::system::radar::RADAR_CELL_HEIGHT as i64)
                    as i32;

            // Draw event indicator (pulsing/blinking based on frame)
            let alpha = if (event.create_frame / 10) % 2 == 0 {
                1.0
            } else {
                0.5
            };
            let color1 = [
                event.color1.r as f32 / 255.0,
                event.color1.g as f32 / 255.0,
                event.color1.b as f32 / 255.0,
                alpha,
            ];

            let event_size = 4;
            renderer.draw_rect(
                UIRect::new(
                    (screen_x - event_size) as f32,
                    (screen_y - event_size) as f32,
                    (event_size * 2) as f32,
                    (event_size * 2) as f32,
                ),
                color1,
                0.0,
            );
        }
    });
}

pub fn w3d_right_hud_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    if window.get_status().contains(WindowStatus::IMAGE) {
        super::game_window::default_draw_callback(window, inst_data);
    }
}

fn log_n(value: f32, base: f32) -> f32 {
    if value <= 0.0 || base <= 1.0 {
        return 0.0;
    }
    value.log10() / base.log10()
}

fn draw_tiled_horiz(image: &super::game_window::Image, x: i32, y: i32, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let tile_width = image.width.max(1);
    with_window_manager_ref(|manager| {
        let mut draw_x = x;
        let end_x = x + width;
        while draw_x < end_x {
            let next_x = (draw_x + tile_width).min(end_x);
            manager.win_draw_image(image, draw_x, y, next_x, y + height, WIN_COLOR_UNDEFINED);
            draw_x += tile_width;
        }
    });
}

fn draw_tiled_vert(image: &super::game_window::Image, x: i32, y: i32, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let tile_height = image.height.max(1);
    with_window_manager_ref(|manager| {
        let mut draw_y = y;
        let end_y = y + height;
        while draw_y < end_y {
            let next_y = (draw_y + tile_height).min(end_y);
            manager.win_draw_image(image, x, draw_y, x + width, next_y, WIN_COLOR_UNDEFINED);
            draw_y += tile_height;
        }
    });
}

pub fn w3d_power_draw_a(window: &GameWindow, inst_data: &WindowInstanceData) {
    let Some(global) = get_global_data() else {
        return;
    };
    let global = global.read();
    let power_bar_base = global.power_bar_base.max(2) as f32;
    let power_bar_intervals = global.power_bar_intervals.max(1.0);
    let yellow_range = global.power_bar_yellow_range;
    drop(global);

    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let player_arc = TheControlBar::get_observer_look_at_player_index()
        .and_then(|index| {
            if index >= 0 {
                list.get_player(index as i32).cloned()
            } else {
                None
            }
        })
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };
    let energy = player.get_energy();
    let consumption = energy.consumption();
    let production = energy.production();
    drop(player);

    let (end_bar, begin_bar, center_bar) =
        if consumption > production - yellow_range && consumption <= production {
            ("PowerBarYellowEndR", "PowerBarYellowEndL", "PowerBarYellow")
        } else if consumption > production {
            ("PowerBarRedEndR", "PowerBarRedEndL", "PowerBarRed")
        } else {
            ("PowerBarGreenEndR", "PowerBarGreenEndL", "PowerBarGreen")
        };

    let (end_bar, begin_bar, center_bar, slider) = with_window_manager_ref(|manager| {
        (
            manager.win_find_image(end_bar),
            manager.win_find_image(begin_bar),
            manager.win_find_image(center_bar),
            manager.win_find_image("PowerBarSlider"),
        )
    });
    let (Some(end_bar), Some(begin_bar), Some(center_bar), Some(slider)) =
        (end_bar, begin_bar, center_bar, slider)
    else {
        return;
    };

    let (pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    if size_x <= 0 || size_y <= 0 {
        return;
    }

    let prod_for_log = production.max(1) as f32;
    let mut range = (log_n(prod_for_log, power_bar_base) * (size_x as f32 / power_bar_intervals))
        .round() as i32;
    range = range.clamp(0, size_x);

    let begin_w = begin_bar.width.max(1);
    let end_w = end_bar.width.max(1);
    if range < begin_w + end_w {
        range = begin_w + end_w;
    }

    let left_end_x = pos_x + begin_w;
    let right_start_x = pos_x + range - end_w;

    if right_start_x <= left_end_x {
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &begin_bar,
                pos_x,
                pos_y,
                pos_x + range / 2,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
            manager.win_draw_image(
                &end_bar,
                pos_x + range / 2,
                pos_y,
                pos_x + range,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    } else {
        let center_w = center_bar.width.max(1);
        let center_width = right_start_x - left_end_x;
        let pieces = center_width / center_w;
        let mut x = left_end_x;
        for _ in 0..pieces {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    &center_bar,
                    x,
                    pos_y,
                    x + center_w,
                    pos_y + size_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
            x += center_w;
        }

        let remaining = right_start_x - x;
        if remaining > 0 {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    &center_bar,
                    x,
                    pos_y,
                    x + center_w,
                    pos_y + size_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }

        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &begin_bar,
                pos_x,
                pos_y,
                left_end_x,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });

        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &end_bar,
                right_start_x,
                pos_y,
                right_start_x + end_w,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    }

    let consumption_for_needle = if consumption == 1 {
        1.5f32
    } else {
        consumption.max(1) as f32
    };
    let mut needle = (log_n(consumption_for_needle, power_bar_base)
        * (size_x as f32 / power_bar_intervals)) as i32;
    needle = needle.clamp(0, size_x);

    let slider_w = slider.width.max(1);
    let slider_h = slider.height.max(1);
    let mut slider_start = if needle >= size_x {
        pos_x + size_x - slider_w
    } else {
        pos_x + needle - slider_w / 2
    };
    if slider_start <= pos_x {
        slider_start = pos_x;
    }

    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &slider,
            slider_start,
            pos_y + size_y - slider_h,
            slider_start + slider_w,
            pos_y + size_y,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub fn w3d_power_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let Some(global) = get_global_data() else {
        return;
    };
    let global = global.read();
    let power_bar_base = global.power_bar_base.max(2) as f32;
    let power_bar_intervals = global.power_bar_intervals.max(1.0);
    let yellow_range = global.power_bar_yellow_range;
    drop(global);

    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let player_arc = TheControlBar::get_observer_look_at_player_index()
        .and_then(|index| {
            if index >= 0 {
                list.get_player(index as i32).cloned()
            } else {
                None
            }
        })
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };
    let energy = player.get_energy();
    let consumption = energy.consumption();
    let production = energy.production();
    drop(player);

    let center_name = if consumption > production - yellow_range && consumption <= production {
        "PowerPointY"
    } else if consumption > production {
        "PowerPointR"
    } else {
        "PowerPointG"
    };

    let (center_bar, slider) = with_window_manager_ref(|manager| {
        (
            manager.win_find_image(center_name),
            manager.win_find_image("PowerBarSlider"),
        )
    });
    let (Some(center_bar), Some(slider)) = (center_bar, slider) else {
        super::game_window::default_draw_callback(window, inst_data);
        return;
    };

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    if width <= 0 || height <= 0 {
        return;
    }

    let prod_for_log = production.max(1) as f32;
    let mut power_range =
        (log_n(prod_for_log, power_bar_base) * (width as f32 / power_bar_intervals)).round() as i32;
    power_range = power_range.clamp(0, width);
    if power_range > 0 {
        draw_tiled_horiz(&center_bar, x, y, power_range, height);
    }

    let consumption_for_needle = if consumption == 1 {
        1.5
    } else {
        consumption.max(1) as f32
    };
    let mut needle = (log_n(consumption_for_needle, power_bar_base)
        * (width as f32 / power_bar_intervals))
        .round() as i32;
    needle = needle.clamp(0, width);

    let slider_w = slider.width.max(1);
    let slider_h = slider.height.max(1);
    let mut slider_start = if needle >= width {
        x + width - slider_w
    } else {
        x + needle - slider_w / 2
    };
    if slider_w >= width {
        slider_start = x;
    } else {
        slider_start = slider_start.max(x).min(x + width - slider_w);
    }
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &slider,
            slider_start,
            y + height - slider_h,
            slider_start + slider_w,
            y + height,
            WIN_COLOR_UNDEFINED,
        );
    });
}

fn draw_vertical_meter(
    window: &GameWindow,
    top_name: &str,
    bottom_name: &str,
    center_name: &str,
    filled_height: i32,
) {
    let (top, bottom, center) = with_window_manager_ref(|manager| {
        (
            manager.win_find_image(top_name),
            manager.win_find_image(bottom_name),
            manager.win_find_image(center_name),
        )
    });
    let (Some(top), Some(bottom), Some(center)) = (top, bottom, center) else {
        return;
    };

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    if width <= 0 || height <= 0 {
        return;
    }

    let fill = filled_height.clamp(0, height);
    if fill <= 0 {
        return;
    }

    let top_h = top.height.max(1);
    let bottom_h = bottom.height.max(1);
    let fill_top = y + height - fill;

    let bottom_start = y + height - bottom_h;
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &bottom,
            x,
            bottom_start,
            x + width,
            y + height,
            WIN_COLOR_UNDEFINED,
        );
    });

    let top_start = (fill_top - top_h).max(y);
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &top,
            x,
            top_start,
            x + width,
            top_start + top_h,
            WIN_COLOR_UNDEFINED,
        );
    });

    let center_start = top_start + top_h;
    let center_end = bottom_start;
    if center_end > center_start {
        draw_tiled_vert(&center, x, center_start, width, center_end - center_start);
    }
}

pub fn w3d_command_bar_top_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ callback is effectively no-op in W3DControlBar.cpp.
}

pub fn w3d_command_bar_background_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let manager_handle = get_control_bar_scheme_manager();
    let Some(manager_handle) = manager_handle else {
        super::game_window::default_draw_callback(window, inst_data);
        return;
    };

    let manager = manager_handle.read();

    let base_pos = manager.get_background_marker_pos();
    let win_name = "ControlBar.wnd:BackgroundMarker";
    let marker_window = with_window_manager_ref(|wm| wm.find_window_by_name(win_name));
    let marker_window = match marker_window {
        Some(w) => w,
        None => {
            super::game_window::default_draw_callback(window, inst_data);
            return;
        }
    };

    let (pos_x, pos_y) = marker_window.borrow().get_screen_position();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    manager.draw_background(offset);
}

pub fn w3d_command_bar_foreground_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let manager_handle = get_control_bar_scheme_manager();
    let Some(manager_handle) = manager_handle else {
        super::game_window::default_draw_callback(window, inst_data);
        return;
    };

    let manager = manager_handle.read();

    let base_pos = manager.get_foreground_marker_pos();
    let win_name = "ControlBar.wnd:BackgroundMarker";
    let marker_window = with_window_manager_ref(|wm| wm.find_window_by_name(win_name));
    let marker_window = match marker_window {
        Some(w) => w,
        None => {
            super::game_window::default_draw_callback(window, inst_data);
            return;
        }
    };

    let (pos_x, pos_y) = marker_window.borrow().get_screen_position();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    manager.draw_foreground(offset);
}

pub fn w3d_command_bar_grid_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    if window.get_status().contains(WindowStatus::IMAGE) {
        super::game_window::default_draw_callback(window, inst_data);
        return;
    }

    super::game_window::default_draw_callback(window, inst_data);
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let color = window
        .get_enabled_draw_data(0)
        .map(|entry| entry.border_color)
        .filter(|color| *color != WIN_COLOR_UNDEFINED)
        .unwrap_or(0xFF808080);

    with_window_manager_ref(|manager| {
        manager.win_draw_line(
            color,
            1.0,
            x,
            y + (height as f32 * 0.33) as i32,
            x + width,
            y + (height as f32 * 0.33) as i32,
        );
        manager.win_draw_line(
            color,
            1.0,
            x,
            y + (height as f32 * 0.66) as i32,
            x + width,
            y + (height as f32 * 0.66) as i32,
        );
        manager.win_draw_line(
            color,
            1.0,
            x + (width as f32 * 0.33) as i32,
            y,
            x + (width as f32 * 0.33) as i32,
            y + height,
        );
        manager.win_draw_line(
            color,
            1.0,
            x + (width as f32 * 0.66) as i32,
            y,
            x + (width as f32 * 0.66) as i32,
            y + height,
        );
    });
}

pub fn w3d_command_bar_gen_exp_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = inst_data;
    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let Some(player_arc) = list.get_local_player().cloned() else {
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };
    if !player.is_player_active() {
        return;
    }
    let Some(rank_progress) = RankProgressInfo::from_player(&player) else {
        return;
    };
    let mut progress = (rank_progress.progress_percentage * 100.0).round() as i32;
    progress = progress.clamp(0, 100);
    if progress <= 0 {
        return;
    }

    let (_, height) = window.get_size();
    let filled_height = (height * progress) / 100;
    draw_vertical_meter(
        window,
        "GenExpBarTop1",
        "GenExpBarBottom1",
        "GenExpBar1",
        filled_height,
    );
}

pub fn w3d_command_bar_help_popup_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = inst_data;
    let (_, height) = window.get_size();
    draw_vertical_meter(
        window,
        "Helpbox-top",
        "Helpbox-bottom",
        "Helpbox-middle",
        height,
    );
}

fn draw_video_buffer(window: &GameWindow, inst_data: &WindowInstanceData) {
    let frame = inst_data.video_buffer.as_ref().and_then(read_video_frame);
    let Some(frame) = frame else {
        return;
    };
    let rect = press_scaled_rect(window);
    let offset = inst_data.image_offset;
    let rect = UIRect::new(
        rect.x + offset.x as f32,
        rect.y + offset.y as f32,
        rect.width,
        rect.height,
    );
    let _ = with_ui_renderer_mut(|renderer| {
        let texture = renderer.create_texture_from_rgba(frame.width, frame.height, &frame.data);
        renderer.draw_textured_rect(rect, texture, [1.0, 1.0, 1.0, 1.0], None, 0.0);
    });
}

fn draw_overlay_image(window: &GameWindow, name: &str) {
    let (x, y, w, h) = press_scaled_bounds_i32(window);
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image(name) {
            manager.win_draw_image(&image, x, y, x + w, y + h, WIN_COLOR_UNDEFINED);
        }
    });
}

fn draw_button_overlays(window: &GameWindow, inst_data: &WindowInstanceData) {
    let status = window.get_status();
    if status.contains(WindowStatus::FLASHING) {
        draw_overlay_image(window, "Cameo_push");
    }

    if status.contains(WindowStatus::USE_OVERLAY_STATES) && status.contains(WindowStatus::ENABLED) {
        if inst_data.state.contains(WindowState::HILITED) {
            if inst_data.state.contains(WindowState::PUSHED) {
                draw_overlay_image(window, "Cameo_push");
            } else {
                draw_overlay_image(window, "Cameo_hilited");
            }
        } else if inst_data.state.contains(WindowState::PUSHED) {
            draw_overlay_image(window, "Cameo_push");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushButtonDrawMode {
    Color,
    OneImage,
    ThreeImage,
}

fn push_button_draw_mode_runs_status_overlays(mode: PushButtonDrawMode) -> bool {
    matches!(mode, PushButtonDrawMode::OneImage)
}

fn draw_button_style_overlay(window: &GameWindow, button: &PushButton) {
    let (x, y, w, h) = press_scaled_bounds_i32(window);
    if let Some(ref overlay) = button.style().overlay_image {
        with_window_manager_ref(|manager| {
            if let Some(image) = manager.win_find_image(overlay) {
                manager.win_draw_image(&image, x, y, x + w, y + h, WIN_COLOR_UNDEFINED);
            }
        });
    }

    match button.style().clock_mode {
        ClockMode::Normal => {
            with_window_manager_ref(|manager| {
                manager.win_draw_rect_clock(
                    x,
                    y,
                    w,
                    h,
                    button.style().clock_progress as i32,
                    gadget_color_to_win_color(button.style().clock_color),
                );
            });
        }
        ClockMode::Inverse => {
            with_window_manager_ref(|manager| {
                manager.win_draw_remaining_rect_clock(
                    x,
                    y,
                    w,
                    h,
                    button.style().clock_progress as i32,
                    gadget_color_to_win_color(button.style().clock_color),
                );
            });
        }
        ClockMode::None => {}
    }
}

fn current_push_button_draw_data<'a>(
    window: &GameWindow,
    inst_data: &'a WindowInstanceData,
) -> (
    &'a [super::game_window::WindowDrawData],
    &'a super::game_window::WindowTextColors,
) {
    if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    }
}

fn button_draw_entry_image<'a>(
    draw_data: &'a [super::game_window::WindowDrawData],
    index: usize,
) -> Option<&'a super::game_window::Image> {
    draw_data.get(index).and_then(|entry| entry.image.as_ref())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushButtonImageBank {
    Enabled,
    Disabled,
    Hilite,
}

fn push_button_one_image_source(
    enabled: bool,
    hilited: bool,
    selected: bool,
    overlay_states: bool,
) -> (PushButtonImageBank, usize) {
    if overlay_states {
        return (PushButtonImageBank::Enabled, 0);
    }

    if !enabled {
        return (PushButtonImageBank::Disabled, if selected { 1 } else { 0 });
    }

    if hilited {
        return (PushButtonImageBank::Hilite, if selected { 1 } else { 0 });
    }

    if selected {
        (PushButtonImageBank::Hilite, 1)
    } else {
        (PushButtonImageBank::Enabled, 0)
    }
}

fn push_button_image_bank_draw_data<'a>(
    inst_data: &'a WindowInstanceData,
    bank: PushButtonImageBank,
) -> &'a [super::game_window::WindowDrawData] {
    match bank {
        PushButtonImageBank::Enabled => &inst_data.enabled_draw_data,
        PushButtonImageBank::Disabled => &inst_data.disabled_draw_data,
        PushButtonImageBank::Hilite => &inst_data.hilite_draw_data,
    }
}

fn resolve_push_button_one_image<'a>(
    window: &GameWindow,
    inst_data: &'a WindowInstanceData,
) -> Option<&'a super::game_window::Image> {
    let enabled = !inst_data.state.contains(WindowState::DISABLED) && window.is_enabled();
    let hilited = inst_data.state.contains(WindowState::HILITED);
    let selected = inst_data.state.contains(WindowState::PUSHED);
    let overlay_states = window
        .get_status()
        .contains(WindowStatus::USE_OVERLAY_STATES);
    let (bank, index) = push_button_one_image_source(enabled, hilited, selected, overlay_states);
    button_draw_entry_image(push_button_image_bank_draw_data(inst_data, bank), index)
}

fn draw_push_button_image_one(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    image: &super::game_window::Image,
) {
    let rect = press_scaled_rect(window);
    let start_x = rect.x as i32 + inst_data.image_offset.x;
    let start_y = rect.y as i32 + inst_data.image_offset.y;
    let end_x = start_x + rect.width as i32;
    let end_y = start_y + rect.height as i32;
    with_window_manager_ref(|manager| {
        manager.win_draw_image(image, start_x, start_y, end_x, end_y, WIN_COLOR_UNDEFINED);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushButtonThreeImagePart {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
struct PushButtonThreeImageDraw {
    part: PushButtonThreeImagePart,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    clip: Option<IRegion2D>,
}

#[allow(clippy::too_many_arguments)]
fn push_button_three_piece_image_draws(
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
    x_offset: i32,
    y_offset: i32,
    left_width: i32,
    right_width: i32,
    center_width: i32,
) -> Vec<PushButtonThreeImageDraw> {
    let left_width = left_width.max(1);
    let right_width = right_width.max(1);
    let center_width = center_width.max(1);
    let left_end_x = origin_x + left_width + x_offset;
    let left_end_y = origin_y + height + y_offset;
    let right_start_x = origin_x + width - right_width + x_offset;
    let right_start_y = origin_y + y_offset;
    let center_band_width = right_start_x - left_end_x;
    let mut draws = Vec::new();

    if center_band_width <= 0 {
        let mid_x = origin_x + x_offset + width / 2;
        draws.push(PushButtonThreeImageDraw {
            part: PushButtonThreeImagePart::Left,
            start_x: origin_x + x_offset,
            start_y: origin_y + y_offset,
            end_x: mid_x,
            end_y: left_end_y,
            clip: None,
        });
        draws.push(PushButtonThreeImageDraw {
            part: PushButtonThreeImagePart::Right,
            start_x: mid_x,
            start_y: right_start_y,
            end_x: origin_x + width,
            end_y: right_start_y + height,
            clip: None,
        });
        return draws;
    }

    let start_y = origin_y + y_offset;
    let end_y = start_y + height + y_offset;
    let mut x = left_end_x;
    let pieces = center_band_width / center_width;
    for _ in 0..pieces {
        draws.push(PushButtonThreeImageDraw {
            part: PushButtonThreeImagePart::Center,
            start_x: x,
            start_y,
            end_x: x + center_width,
            end_y,
            clip: None,
        });
        x += center_width;
    }

    if right_start_x - x > 0 {
        draws.push(PushButtonThreeImageDraw {
            part: PushButtonThreeImagePart::Center,
            start_x: x,
            start_y,
            end_x: x + center_width,
            end_y,
            clip: Some(region_from_corners(x, start_y, right_start_x, end_y)),
        });
    }

    draws.push(PushButtonThreeImageDraw {
        part: PushButtonThreeImagePart::Left,
        start_x: origin_x + x_offset,
        start_y: origin_y + y_offset,
        end_x: left_end_x,
        end_y: left_end_y,
        clip: None,
    });
    draws.push(PushButtonThreeImageDraw {
        part: PushButtonThreeImagePart::Right,
        start_x: right_start_x,
        start_y: right_start_y,
        end_x: right_start_x + right_width,
        end_y: right_start_y + height,
        clip: None,
    });
    draws
}

fn resolve_push_button_three_piece_images<'a>(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    draw_data: &'a [super::game_window::WindowDrawData],
) -> Option<(
    &'a super::game_window::Image,
    &'a super::game_window::Image,
    &'a super::game_window::Image,
)> {
    let selected = inst_data.state.contains(WindowState::PUSHED);
    if window
        .get_status()
        .contains(WindowStatus::USE_OVERLAY_STATES)
    {
        return None;
    }

    let (left_idx, center_idx, right_idx) = push_button_three_piece_slots(selected);
    let left = button_draw_entry_image(draw_data, left_idx)?;
    let center = button_draw_entry_image(draw_data, center_idx)?;
    let right = button_draw_entry_image(draw_data, right_idx)?;
    Some((left, center, right))
}

fn push_button_three_piece_slots(selected: bool) -> (usize, usize, usize) {
    if selected {
        (1usize, 3usize, 4usize)
    } else {
        (0usize, 5usize, 6usize)
    }
}

fn draw_push_button_image_three(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    left: &super::game_window::Image,
    center: &super::game_window::Image,
    right: &super::game_window::Image,
) {
    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let width = rect.width as i32;
    let height = rect.height as i32;
    let x_offset = inst_data.image_offset.x;
    let y_offset = inst_data.image_offset.y;

    let left_w = left.width.max(1);
    let right_w = right.width.max(1);
    let center_w = center.width.max(1);

    for draw in push_button_three_piece_image_draws(
        origin_x, origin_y, width, height, x_offset, y_offset, left_w, right_w, center_w,
    ) {
        let image = match draw.part {
            PushButtonThreeImagePart::Left => left,
            PushButtonThreeImagePart::Center => center,
            PushButtonThreeImagePart::Right => right,
        };
        if let Some(clip) = draw.clip {
            draw_window_image_clipped(
                image,
                draw.start_x,
                draw.start_y,
                draw.end_x,
                draw.end_y,
                &clip,
            );
        } else {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    image,
                    draw.start_x,
                    draw.start_y,
                    draw.end_x,
                    draw.end_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }
    }
}

fn push_button_color_entry_index(selected: bool) -> usize {
    if selected {
        1
    } else {
        0
    }
}

fn draw_push_button_color_base(window: &GameWindow, entry: &super::game_window::WindowDrawData) {
    let (x, y, width, height) = press_scaled_bounds_i32(window);
    with_window_manager_ref(|manager| {
        if entry.border_color != WIN_COLOR_UNDEFINED {
            manager.win_open_rect(entry.border_color, 1.0, x, y, x + width, y + height);
        }
        if entry.color != WIN_COLOR_UNDEFINED {
            manager.win_fill_rect(
                entry.color,
                1.0,
                x + 1,
                y + 1,
                x + width - 1,
                y + height - 1,
            );
        }
    });
}

fn draw_push_button_base(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) -> PushButtonDrawMode {
    let (draw_data, text_colors) = current_push_button_draw_data(window, inst_data);

    if let Some((left, center, right)) =
        resolve_push_button_three_piece_images(window, inst_data, draw_data)
    {
        draw_push_button_image_three(window, inst_data, left, center, right);
        let _ = text_colors;
        return PushButtonDrawMode::ThreeImage;
    }

    let one_image = resolve_push_button_one_image(window, inst_data);
    if one_image.is_some() || button_draw_entry_image(draw_data, 0).is_some() {
        if let Some(image) = one_image {
            draw_push_button_image_one(window, inst_data, image);
        }
        let _ = text_colors;
        return PushButtonDrawMode::OneImage;
    }

    let selected = inst_data.state.contains(WindowState::PUSHED);
    if let Some(entry) = draw_data.get(push_button_color_entry_index(selected)) {
        draw_push_button_color_base(window, entry);
    }

    let _ = text_colors;
    PushButtonDrawMode::Color
}

pub fn w3d_gadget_push_button_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = draw_push_button_base(window, inst_data);
    draw_button_text(window, inst_data);
    draw_video_buffer(window, inst_data);
    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::PushButton(button) = widget {
            draw_button_style_overlay(window, button);
        }
    }
}

pub fn w3d_gadget_push_button_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let mode = draw_push_button_base(window, inst_data);
    draw_button_text(window, inst_data);
    draw_video_buffer(window, inst_data);
    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::PushButton(button) = widget {
            draw_button_style_overlay(window, button);
        }
    }
    if push_button_draw_mode_runs_status_overlays(mode) {
        draw_button_overlays(window, inst_data);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryDrawState {
    Enabled,
    Disabled,
}

fn static_text_draw_state(window_enabled: bool, _inst_state: WindowState) -> BinaryDrawState {
    if window_enabled {
        BinaryDrawState::Enabled
    } else {
        BinaryDrawState::Disabled
    }
}

fn static_text_draw_resources<'a>(
    window: &GameWindow,
    inst_data: &'a WindowInstanceData,
) -> (
    BinaryDrawState,
    &'a super::game_window::WindowDrawData,
    &'a super::game_window::WindowTextColors,
) {
    match static_text_draw_state(window.is_enabled(), inst_data.state) {
        BinaryDrawState::Enabled => (
            BinaryDrawState::Enabled,
            &inst_data.enabled_draw_data[0],
            &inst_data.enabled_text,
        ),
        BinaryDrawState::Disabled => (
            BinaryDrawState::Disabled,
            &inst_data.disabled_draw_data[0],
            &inst_data.disabled_text,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StaticTextImageRect {
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
}

fn static_text_image_rect(
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
    image_offset: Point2D,
) -> StaticTextImageRect {
    let start_x = origin_x + image_offset.x;
    let start_y = origin_y + image_offset.y;
    StaticTextImageRect {
        start_x,
        start_y,
        end_x: start_x + width,
        end_y: start_y + height,
    }
}

fn static_text_should_draw_text(text_color: u32) -> bool {
    text_color != WIN_COLOR_UNDEFINED
}

fn draw_static_text_background(
    window: &GameWindow,
    draw_data: &super::game_window::WindowDrawData,
) {
    let (x, y, width, height) = press_scaled_bounds_i32(window);
    with_window_manager_ref(|manager| {
        if draw_data.border_color != WIN_COLOR_UNDEFINED {
            manager.win_open_rect(draw_data.border_color, 1.0, x, y, x + width, y + height);
        }
        if draw_data.color != WIN_COLOR_UNDEFINED {
            manager.win_fill_rect(
                draw_data.color,
                1.0,
                x + 1,
                y + 1,
                x + width - 1,
                y + height - 1,
            );
        }
    });
}

fn draw_static_text_back_image(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    draw_data: &super::game_window::WindowDrawData,
) {
    let Some(image) = draw_data.image.as_ref() else {
        return;
    };
    let rect = press_scaled_rect(window);
    let image_rect = static_text_image_rect(
        rect.x as i32,
        rect.y as i32,
        rect.width as i32,
        rect.height as i32,
        inst_data.image_offset,
    );
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            image,
            image_rect.start_x,
            image_rect.start_y,
            image_rect.end_x,
            image_rect.end_y,
            WIN_COLOR_UNDEFINED,
        );
    });
}

fn draw_static_text(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    text_color: u32,
    drop: u32,
) {
    let raw_text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    let text = resolve_window_text(raw_text);
    if text.is_empty() {
        return;
    }

    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let width = rect.width as i32;
    let height = rect.height as i32;

    let mut left_margin = 0;
    let mut top_margin = 0;
    let mut align = TextAlignment::Left;
    let mut valign = VerticalAlignment::Top;

    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::StaticText(static_text) = widget {
            let cfg = static_text.config();
            left_margin = cfg.left_margin as i32;
            top_margin = cfg.top_margin as i32;
            align = cfg.alignment;
            valign = cfg.vertical_alignment;
        }
    }

    let mut text_x = origin_x + left_margin;
    let mut text_y = origin_y + top_margin;

    if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text.clone());
        let wrap = (width - 10).max(0);
        display.set_word_wrap(wrap);
        display.set_word_wrap_centered(window.get_status().contains(WindowStatus::WRAP_CENTERED));
        display.set_use_hotkey(
            window.get_status().contains(WindowStatus::HOTKEY_TEXT),
            global_hotkey_text_color(),
        );
        display.set_clip_region(Some(region_from_corners(
            origin_x,
            origin_y,
            origin_x + width,
            origin_y + height,
        )));
        if let Some(font) = inst_data.font.as_ref() {
            display.set_font(font);
        }
        let (text_w, text_h) = display.get_size();
        if align == TextAlignment::Center {
            text_x = origin_x + (width / 2) - (text_w / 2);
        } else if align == TextAlignment::Right {
            text_x = origin_x + width - text_w - left_margin;
        }
        if valign == VerticalAlignment::Center {
            text_y = origin_y + (height / 2) - (text_h / 2);
        } else if valign == VerticalAlignment::Bottom {
            text_y = origin_y + height - text_h - top_margin;
        }
        display.draw(text_x, text_y, text_color, drop);
        display.set_clip_region(None);
    }
}

pub fn w3d_gadget_static_text_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (_, draw_data, text_colors) = static_text_draw_resources(window, inst_data);
    draw_static_text_background(window, draw_data);
    if static_text_should_draw_text(text_colors.color) {
        draw_static_text(
            window,
            inst_data,
            text_colors.color,
            text_colors.border_color,
        );
    }
}

pub fn w3d_gadget_static_text_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (_, draw_data, text_colors) = static_text_draw_resources(window, inst_data);
    draw_static_text_back_image(window, inst_data, draw_data);
    if static_text_should_draw_text(text_colors.color) {
        draw_static_text(
            window,
            inst_data,
            text_colors.color,
            text_colors.border_color,
        );
    }
}

fn progress_percent(window: &GameWindow) -> i32 {
    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::ProgressBar(bar) = widget {
            return bar.percentage().round() as i32;
        }
    }
    if let Some(value) = window.get_user_data::<i32>() {
        return *value;
    }
    0
}

fn draw_progress_bar_solid(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    back: &super::game_window::WindowDrawData,
    bar: &super::game_window::WindowDrawData,
) {
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let progress = progress_percent(window).clamp(0, 100);

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(
                back.border_color,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                back.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }

    if progress > 0 {
        let bar_width = (size_x * progress) / 100;
        if bar.border_color != WIN_COLOR_UNDEFINED && bar_width > 1 {
            with_window_manager_ref(|manager| {
                manager.win_open_rect(
                    bar.border_color,
                    1.0,
                    origin_x,
                    origin_y,
                    origin_x + bar_width,
                    origin_y + size_y,
                );
            });
        }
        if bar.color != WIN_COLOR_UNDEFINED && bar_width > 1 {
            with_window_manager_ref(|manager| {
                manager.win_fill_rect(
                    bar.color,
                    1.0,
                    origin_x + 1,
                    origin_y + 1,
                    origin_x + bar_width - 1,
                    origin_y + size_y - 1,
                );
                manager.win_draw_line(
                    0xFFFFFFFF,
                    1.0,
                    origin_x + 1,
                    origin_y + 1,
                    origin_x + bar_width - 1,
                    origin_y + 1,
                );
                manager.win_draw_line(
                    0xFFC8C8C8,
                    1.0,
                    origin_x + 1,
                    origin_y + 1,
                    origin_x + 1,
                    origin_y + size_y - 1,
                );
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ProgressBarImageDraw {
    image_slot: usize,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    clip: Option<IRegion2D>,
}

#[allow(clippy::too_many_arguments)]
fn progress_bar_image_draws(
    origin_x: i32,
    origin_y: i32,
    size_x: i32,
    size_y: i32,
    x_offset: i32,
    y_offset: i32,
    progress: i32,
    back_left_width: i32,
    back_right_width: i32,
    back_center_width: i32,
    bar_center_width: i32,
    bar_right_width: i32,
) -> Vec<ProgressBarImageDraw> {
    let back_center_width = back_center_width.max(1);
    let bar_center_width = bar_center_width.max(1);
    let bar_right_width = bar_right_width.max(1);
    let left_end_x = origin_x + back_left_width + x_offset;
    let right_start_x = origin_x + size_x - back_right_width + x_offset;
    let strip_y = origin_y + y_offset;
    let strip_bottom_y = strip_y + size_y;
    let mut draws = Vec::new();

    let center_width = right_start_x - left_end_x;
    let center_pieces = center_width / back_center_width;
    let mut x = left_end_x;
    for _ in 0..center_pieces {
        draws.push(ProgressBarImageDraw {
            image_slot: 2,
            start_x: x,
            start_y: strip_y,
            end_x: x + back_center_width,
            end_y: strip_bottom_y,
            clip: None,
        });
        x += back_center_width;
    }

    let clipped_center_width = right_start_x - x;
    if clipped_center_width > 0 {
        draws.push(ProgressBarImageDraw {
            image_slot: 2,
            start_x: x,
            start_y: strip_y,
            end_x: x + back_center_width,
            end_y: strip_bottom_y,
            clip: Some(region_from_corners(
                x,
                strip_y,
                right_start_x,
                strip_bottom_y,
            )),
        });
    }

    draws.push(ProgressBarImageDraw {
        image_slot: 0,
        start_x: origin_x + x_offset,
        start_y: strip_y,
        end_x: left_end_x,
        end_y: strip_bottom_y,
        clip: None,
    });
    draws.push(ProgressBarImageDraw {
        image_slot: 1,
        start_x: right_start_x,
        start_y: strip_y,
        end_x: right_start_x + back_right_width,
        end_y: strip_bottom_y,
        clip: None,
    });

    let bar_draw_width = ((size_x - 20) * progress) / 100;
    let filled_pieces = bar_draw_width / bar_center_width;
    let bar_y = origin_y + y_offset + 5;
    let bar_bottom_y = bar_y + size_y - 10;
    let mut bar_x = origin_x + 10;
    for _ in 0..filled_pieces {
        draws.push(ProgressBarImageDraw {
            image_slot: 6,
            start_x: bar_x,
            start_y: bar_y,
            end_x: bar_x + bar_center_width,
            end_y: bar_bottom_y,
            clip: None,
        });
        bar_x += bar_center_width;
    }

    bar_x = origin_x + 10 + bar_center_width * filled_pieces;
    let remaining_pieces = ((size_x - 20) / bar_center_width) - filled_pieces;
    for _ in 0..remaining_pieces {
        draws.push(ProgressBarImageDraw {
            image_slot: 5,
            start_x: bar_x,
            start_y: bar_y,
            end_x: bar_x + bar_right_width,
            end_y: bar_bottom_y,
            clip: None,
        });
        bar_x += bar_right_width;
    }

    draws
}

fn draw_progress_bar_image(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    draw_data: &[super::game_window::WindowDrawData],
) {
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let progress = progress_percent(window).clamp(0, 100);

    let required_images = [0, 1, 2, 5, 6];
    if required_images.iter().any(|slot| {
        draw_data
            .get(*slot)
            .and_then(|data| data.image.as_ref())
            .is_none()
    }) {
        return;
    }
    let image = |slot: usize| draw_data[slot].image.as_ref().expect("checked image");
    let draws = progress_bar_image_draws(
        origin_x,
        origin_y,
        size_x,
        size_y,
        inst_data.image_offset.x,
        inst_data.image_offset.y,
        progress,
        image(0).width,
        image(1).width,
        image(2).width,
        image(6).width,
        image(5).width,
    );

    for draw in draws {
        let image = image(draw.image_slot);
        if let Some(clip) = draw.clip {
            draw_window_image_clipped(
                image,
                draw.start_x,
                draw.start_y,
                draw.end_x,
                draw.end_y,
                &clip,
            );
        } else {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    image,
                    draw.start_x,
                    draw.start_y,
                    draw.end_x,
                    draw.end_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }
    }
}

pub fn w3d_gadget_progress_bar_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };
    let back = &draw_data[0];
    let bar = &draw_data[1];
    draw_progress_bar_solid(window, inst_data, back, bar);
}

pub fn w3d_gadget_progress_bar_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };
    draw_progress_bar_image(window, inst_data, draw_data);
}

pub fn w3d_gadget_progress_bar_image_draw_a(window: &GameWindow, inst_data: &WindowInstanceData) {
    let progress = progress_percent(window).clamp(0, 100);
    let draw_data = &inst_data.enabled_draw_data;

    let bar_center = &draw_data[6].image;
    let bar_right = &draw_data[5].image;
    let left = &draw_data[0].image;
    let right = &draw_data[1].image;
    let center = &draw_data[2].image;

    let (Some(bar_center), Some(_bar_right), Some(_left), Some(_right), Some(_center)) =
        (bar_center, bar_right, left, right, center)
    else {
        return;
    };

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let width = bar_center.width.max(1);
    let draw_width = (size_x * progress) / 100;
    let pieces = draw_width / width;
    let mut x = origin_x;
    for _ in 0..pieces {
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                bar_center,
                x,
                origin_y,
                x + width,
                origin_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });
        x += width;
    }
}

fn draw_check_box_text(window: &GameWindow, inst_data: &WindowInstanceData) {
    let raw_text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    let text = resolve_window_text(raw_text);
    if text.is_empty() {
        return;
    }
    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let size_x = rect.width as i32;
    let size_y = rect.height as i32;

    let (text_color, drop_color) =
        if !window.is_enabled() || inst_data.state.contains(WindowState::DISABLED) {
            (
                inst_data.disabled_text.color,
                inst_data.disabled_text.border_color,
            )
        } else if inst_data.state.contains(WindowState::HILITED) {
            (
                inst_data.hilite_text.color,
                inst_data.hilite_text.border_color,
            )
        } else {
            (
                inst_data.enabled_text.color,
                inst_data.enabled_text.border_color,
            )
        };

    if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text.clone());
        if let Some(font) = inst_data.font.as_ref() {
            display.set_font(font);
        }
        let (text_w, text_h) = display.get_size();
        let text_x = origin_x + size_y;
        let text_y = origin_y + (size_y / 2) - (text_h / 2);
        display.draw(text_x, text_y, text_color, drop_color);
    }
}

fn is_check_box_checked(window: &GameWindow) -> bool {
    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::CheckBox(checkbox) = widget {
            return checkbox.is_checked();
        }
    }
    window.instance_data().state.contains(WindowState::PUSHED)
}

fn checkbox_box_image_slot(checked: bool) -> usize {
    if checked {
        2
    } else {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckBoxImageRect {
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
}

fn checkbox_box_image_rect(
    origin_x: i32,
    origin_y: i32,
    size_y: i32,
    image_offset: Point2D,
) -> CheckBoxImageRect {
    let start_x = origin_x + image_offset.x;
    let start_y = origin_y + 3;
    let end_x = start_x + (size_y - 6);
    let end_y = start_y + (size_y - 6);
    CheckBoxImageRect {
        start_x,
        start_y,
        end_x,
        end_y,
    }
}

pub fn w3d_gadget_check_box_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };
    let back = &draw_data[0];
    let checked = is_check_box_checked(window);
    let check_box = if checked {
        draw_data.get(2).unwrap_or(&draw_data[1])
    } else {
        &draw_data[1]
    };

    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let size_x = rect.width as i32;
    let size_y = rect.height as i32;
    let check_offset = size_x / 16;

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(
                back.border_color,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                back.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }

    let box_x = origin_x + check_offset;
    let box_y = origin_y + (size_y / 3);
    let box_end_x = box_x + (size_y / 3);
    let box_end_y = box_y + (size_y / 3);
    with_window_manager_ref(|manager| {
        manager.win_open_rect(
            check_box.border_color,
            1.0,
            box_x,
            box_y,
            box_end_x,
            box_end_y,
        );
    });

    if is_check_box_checked(window) && check_box.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_draw_line(check_box.color, 1.0, box_x, box_y, box_end_x, box_end_y);
            manager.win_draw_line(check_box.color, 1.0, box_x, box_end_y, box_end_x, box_y);
        });
    }

    draw_check_box_text(window, inst_data);
}

pub fn w3d_gadget_check_box_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };
    let checked = is_check_box_checked(window);
    let check_box = &draw_data[checkbox_box_image_slot(checked)];
    if let Some(image) = &check_box.image {
        let rect = press_scaled_rect(window);
        let image_rect = checkbox_box_image_rect(
            rect.x as i32,
            rect.y as i32,
            rect.height as i32,
            inst_data.image_offset,
        );
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                image,
                image_rect.start_x,
                image_rect.start_y,
                image_rect.end_x,
                image_rect.end_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    }
    draw_check_box_text(window, inst_data);
}

fn draw_radio_button_text(window: &GameWindow, inst_data: &WindowInstanceData) {
    let raw_text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    let text = resolve_window_text(raw_text);
    if text.is_empty() {
        return;
    }
    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let size_x = rect.width as i32;
    let size_y = rect.height as i32;

    let (text_color, drop_color) =
        if !window.is_enabled() || inst_data.state.contains(WindowState::DISABLED) {
            (
                inst_data.disabled_text.color,
                inst_data.disabled_text.border_color,
            )
        } else if inst_data.state.contains(WindowState::HILITED) {
            (
                inst_data.hilite_text.color,
                inst_data.hilite_text.border_color,
            )
        } else {
            (
                inst_data.enabled_text.color,
                inst_data.enabled_text.border_color,
            )
        };

    if let Some(display) = inst_data.display_text.as_ref() {
        let mut display = display.borrow_mut();
        display.set_text(text.clone());
        if let Some(font) = inst_data.font.as_ref() {
            display.set_font(font);
        }
        let (text_w, text_h) = display.get_size();
        let text_x = origin_x + (size_x / 2) - (text_w / 2);
        let text_y = origin_y + (size_y / 2) - (text_h / 2);
        display.draw(text_x, text_y, text_color, drop_color);
    }
}

fn is_radio_selected(window: &GameWindow) -> bool {
    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::RadioButton(radio) = widget {
            return radio.is_selected();
        }
    }
    window.instance_data().state.contains(WindowState::PUSHED)
}

pub fn w3d_gadget_radio_button_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };
    let back = &draw_data[0];
    let radio_box = if is_radio_selected(window) {
        &draw_data[2]
    } else {
        &draw_data[1]
    };

    let rect = press_scaled_rect(window);
    let origin_x = rect.x as i32;
    let origin_y = rect.y as i32;
    let size_x = rect.width as i32;
    let size_y = rect.height as i32;

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(
                back.border_color,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                back.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_draw_line(
                back.border_color,
                1.0,
                origin_x + size_y,
                origin_y,
                origin_x + size_y,
                origin_y + size_y,
            );
            manager.win_draw_line(
                back.border_color,
                1.0,
                origin_x + size_x - size_y,
                origin_y,
                origin_x + size_x - size_y,
                origin_y + size_y,
            );
        });
    }

    if radio_box.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                radio_box.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_y - 1,
                origin_y + size_y - 1,
            );
            manager.win_fill_rect(
                radio_box.color,
                1.0,
                origin_x + size_x - size_y,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }

    draw_radio_button_text(window, inst_data);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RadioDrawState {
    Enabled,
    Disabled,
    Hilite,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RadioButtonImageSlots {
    state: RadioDrawState,
    left: usize,
    center: usize,
    right: usize,
}

fn radio_button_image_slots(selected: bool, enabled: bool, hilited: bool) -> RadioButtonImageSlots {
    if selected {
        RadioButtonImageSlots {
            state: RadioDrawState::Selected,
            left: 3,
            center: 4,
            right: 5,
        }
    } else if !enabled {
        RadioButtonImageSlots {
            state: RadioDrawState::Disabled,
            left: 0,
            center: 1,
            right: 2,
        }
    } else if hilited {
        RadioButtonImageSlots {
            state: RadioDrawState::Hilite,
            left: 0,
            center: 1,
            right: 2,
        }
    } else {
        RadioButtonImageSlots {
            state: RadioDrawState::Enabled,
            left: 0,
            center: 1,
            right: 2,
        }
    }
}

fn draw_radio_button_image_strip(
    left: &crate::gui::game_window::Image,
    center: &crate::gui::game_window::Image,
    right: &crate::gui::game_window::Image,
    origin_x: i32,
    origin_y: i32,
    size_x: i32,
    size_y: i32,
    x_offset: i32,
    y_offset: i32,
) {
    let left_w = left.width.max(1);
    let right_w = right.width.max(1);
    let center_w = center.width.max(1);
    let left_end_x = origin_x + x_offset + left_w;
    let right_start_x = origin_x + size_x - right_w + x_offset;
    let strip_bottom_y = origin_y + size_y + y_offset;
    let center_clip = region_from_corners(left_end_x, origin_y, right_start_x, strip_bottom_y);

    let mut start_x = left_end_x;
    while start_x < right_start_x {
        let end_x = (start_x + center_w).min(right_start_x);
        draw_window_image_clipped(
            center,
            start_x,
            origin_y + y_offset,
            end_x,
            strip_bottom_y,
            &center_clip,
        );
        start_x += center_w;
    }

    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            left,
            origin_x + x_offset,
            origin_y + y_offset,
            left_end_x,
            strip_bottom_y,
            WIN_COLOR_UNDEFINED,
        );
        manager.win_draw_image(
            right,
            right_start_x,
            origin_y + y_offset,
            origin_x + size_x,
            strip_bottom_y,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub fn w3d_gadget_radio_button_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let selected = is_radio_selected(window);
    let slots = radio_button_image_slots(
        selected,
        window.is_enabled() && !inst_data.state.contains(WindowState::DISABLED),
        inst_data.state.contains(WindowState::HILITED),
    );
    let draw_data = match slots.state {
        RadioDrawState::Enabled => &inst_data.enabled_draw_data,
        RadioDrawState::Disabled => &inst_data.disabled_draw_data,
        RadioDrawState::Hilite | RadioDrawState::Selected => &inst_data.hilite_draw_data,
    };
    let image_set = (
        &draw_data[slots.left].image,
        &draw_data[slots.center].image,
        &draw_data[slots.right].image,
    );

    if let (Some(left), Some(center), Some(right)) = image_set {
        let rect = press_scaled_rect(window);
        let origin_x = rect.x as i32;
        let origin_y = rect.y as i32;
        let size_x = rect.width as i32;
        let size_y = rect.height as i32;
        draw_radio_button_image_strip(
            left,
            center,
            right,
            origin_x,
            origin_y,
            size_x,
            size_y,
            inst_data.image_offset.x,
            inst_data.image_offset.y,
        );
    }
    draw_radio_button_text(window, inst_data);
}

fn slider_percent(
    window: &GameWindow,
    slider_data: Option<&crate::gui::window_script::SliderData>,
) -> f32 {
    if let Some(widget) = window.widget() {
        match widget {
            super::game_window::WindowWidget::HorizontalSlider(slider) => {
                let (min, max) = slider.range();
                let value = slider.value();
                let range = (max - min).max(1);
                return (value - min) as f32 / range as f32;
            }
            super::game_window::WindowWidget::VerticalSlider(slider) => {
                let (min, max) = slider.range();
                let value = slider.value();
                let range = (max - min).max(1);
                return (value - min) as f32 / range as f32;
            }
            _ => {}
        }
    }
    if let Some(data) = slider_data {
        let range = (data.max_value - data.min_value).max(1);
        return (data.position - data.min_value) as f32 / range as f32;
    }
    0.0
}

pub fn w3d_gadget_horizontal_slider_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };
    let back = &draw_data[0];
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(
                back.border_color,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                back.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderImageBank {
    Disabled,
    Hilite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HorizontalSliderBoxImageSources {
    filled_bank: SliderImageBank,
    filled_slot: usize,
    blank_bank: SliderImageBank,
    blank_slot: usize,
    highlight_bank: SliderImageBank,
    highlight_slot: usize,
}

fn horizontal_slider_box_image_sources() -> HorizontalSliderBoxImageSources {
    HorizontalSliderBoxImageSources {
        filled_bank: SliderImageBank::Disabled,
        filled_slot: 0,
        blank_bank: SliderImageBank::Disabled,
        blank_slot: 1,
        highlight_bank: SliderImageBank::Hilite,
        highlight_slot: 0,
    }
}

fn slider_image_from_bank<'a>(
    inst_data: &'a WindowInstanceData,
    bank: SliderImageBank,
    slot: usize,
) -> Option<&'a super::game_window::Image> {
    let draw_data = match bank {
        SliderImageBank::Disabled => &inst_data.disabled_draw_data,
        SliderImageBank::Hilite => &inst_data.hilite_draw_data,
    };
    draw_data.get(slot).and_then(|entry| entry.image.as_ref())
}

pub fn w3d_gadget_horizontal_slider_image_draw(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let image_sources = horizontal_slider_box_image_sources();
    let filled = slider_image_from_bank(
        inst_data,
        image_sources.filled_bank,
        image_sources.filled_slot,
    );
    let blank = slider_image_from_bank(
        inst_data,
        image_sources.blank_bank,
        image_sources.blank_slot,
    );
    let highlight = slider_image_from_bank(
        inst_data,
        image_sources.highlight_bank,
        image_sources.highlight_slot,
    );

    let (mut origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let slider_data = None;
    let selected_percent = slider_percent(window, slider_data);

    let (box_width, box_padding) = if let Some(image) = filled {
        let x_multi = with_window_manager_ref(|manager| manager.screen_size().0 as f32 / 800.0);
        (((image.width as f32 * x_multi).round() as i32).max(1), 2)
    } else {
        (8, 2)
    };

    let mut num_boxes = 0;
    let mut num_selected = 0;
    let mut start_x = origin_x;
    let mut end_x = start_x + box_width;
    let max_selected_x = origin_x + (selected_percent * size_x as f32) as i32;
    while end_x < origin_x + size_x {
        if start_x <= max_selected_x && end_x < origin_x + size_x && selected_percent > 0.0 {
            num_selected += 1;
        }
        start_x = end_x + box_padding;
        end_x = start_x + box_width;
        num_boxes += 1;
    }
    let distance = end_x - origin_x - box_width;
    let blankness = size_x - distance;
    origin_x += blankness / 2;

    if inst_data.state.contains(WindowState::HILITED) {
        if let Some(image) = highlight {
            let mut bg_start_x = origin_x - (box_width + box_padding) / 2;
            let bg_start_y = origin_y + box_width / 3;
            let bg_end_y = bg_start_y + box_width + box_padding;
            for _ in 0..(num_boxes + 1) {
                let bg_end_x = bg_start_x + box_width + box_padding;
                with_window_manager_ref(|manager| {
                    manager.win_draw_image(
                        image,
                        bg_start_x,
                        bg_start_y,
                        bg_end_x,
                        bg_end_y,
                        WIN_COLOR_UNDEFINED,
                    );
                });
                bg_start_x = bg_end_x;
            }
        }
    }

    for i in 0..num_selected {
        if let Some(image) = filled {
            let sx = origin_x + i * (box_width + box_padding);
            let ex = sx + box_width;
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    image,
                    sx,
                    origin_y,
                    ex,
                    origin_y + box_width,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }
    }
    for i in num_selected..num_boxes {
        if let Some(image) = blank {
            let sx = origin_x + i * (box_width + box_padding);
            let ex = sx + box_width;
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    image,
                    sx,
                    origin_y,
                    ex,
                    origin_y + box_width,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }
    }
}

const HORIZONTAL_SLIDER_THUMB_WIDTH: i32 = 8;

pub fn w3d_gadget_horizontal_slider_image_draw_a(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };

    let left_image_left = draw_data[0].image.as_ref();
    let right_image_left = draw_data[1].image.as_ref();
    let center_image_left = draw_data[2].image.as_ref();
    let small_center_image_left = draw_data[3].image.as_ref();
    let left_image_right = draw_data[4].image.as_ref();
    let right_image_right = draw_data[5].image.as_ref();
    let center_image_right = draw_data[6].image.as_ref();
    let small_center_image_right = draw_data[7].image.as_ref();

    if left_image_left.is_none()
        || right_image_left.is_none()
        || center_image_left.is_none()
        || small_center_image_left.is_none()
        || left_image_right.is_none()
        || right_image_right.is_none()
        || center_image_right.is_none()
        || small_center_image_right.is_none()
    {
        return;
    }

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let slider_data = window.get_user_data::<crate::gui::window_script::SliderData>();
    let (num_ticks, position, min_val) = if let Some(s) = slider_data {
        (s.num_ticks, s.position, s.min_value)
    } else {
        (10.0, 0, 0)
    };
    let trans_pos = (num_ticks as i32 * (position - min_val)) + HORIZONTAL_SLIDER_THUMB_WIDTH / 2;

    let x_offset = inst_data.image_offset.x;
    let y_offset = inst_data.image_offset.y;

    let left_image_left = left_image_left.unwrap();
    let right_image_left = right_image_left.unwrap();
    let center_image_left = center_image_left.unwrap();
    let small_center_image_left = small_center_image_left.unwrap();
    let left_image_right = left_image_right.unwrap();
    let right_image_right = right_image_right.unwrap();
    let center_image_right = center_image_right.unwrap();
    let small_center_image_right = small_center_image_right.unwrap();

    let left_size_x = left_image_left.width;
    let right_size_x = right_image_left.width;

    let left_end_x = origin_x + left_size_x + x_offset;
    let left_end_y = origin_y + size_y + y_offset;
    let right_start_x = origin_x + size_x - right_size_x + x_offset;
    let right_start_y = origin_y + size_y - left_size_x + y_offset;

    let clip_left = IRegion2D {
        x: origin_x,
        y: right_start_y,
        width: (origin_x + trans_pos - origin_x).max(0),
        height: (left_end_y - right_start_y).max(0),
    };
    let clip_right = IRegion2D {
        x: origin_x + trans_pos,
        y: right_start_y,
        width: (origin_x + size_x - (origin_x + trans_pos)).max(0),
        height: (left_end_y - right_start_y).max(0),
    };

    // Draw center pieces
    let center_width = right_start_x - left_end_x;
    let pieces = center_width / center_image_left.width.max(1);
    let mut start_x = left_end_x;
    let start_y = origin_y + size_y - left_size_x + y_offset;
    let end_y = origin_y + size_y + y_offset;

    for _ in 0..pieces {
        let end_x = start_x + center_image_left.width;
        draw_window_image_clipped(
            center_image_left,
            start_x,
            start_y,
            end_x,
            end_y,
            &clip_left,
        );
        draw_window_image_clipped(
            center_image_right,
            start_x,
            start_y,
            end_x,
            end_y,
            &clip_right,
        );
        start_x += center_image_left.width;
    }

    // Draw small center pieces in the gap
    let center_width = right_start_x - start_x;
    let pieces = center_width / small_center_image_left.width.max(1) + 1;
    for _ in 0..pieces {
        let end_x = start_x + small_center_image_left.width;
        draw_window_image_clipped(
            small_center_image_left,
            start_x,
            start_y,
            end_x,
            end_y,
            &clip_left,
        );
        draw_window_image_clipped(
            small_center_image_right,
            start_x,
            start_y,
            end_x,
            end_y,
            &clip_right,
        );
        start_x += small_center_image_left.width;
    }

    // Draw left end
    draw_window_image_clipped(
        left_image_left,
        origin_x + x_offset,
        right_start_y,
        left_end_x,
        left_end_y,
        &clip_left,
    );
    draw_window_image_clipped(
        left_image_right,
        origin_x + x_offset,
        right_start_y,
        left_end_x,
        left_end_y,
        &clip_right,
    );

    // Draw right end
    draw_window_image_clipped(
        right_image_left,
        right_start_x,
        right_start_y,
        right_start_x + right_size_x,
        left_end_y,
        &clip_left,
    );
    draw_window_image_clipped(
        right_image_right,
        right_start_x,
        right_start_y,
        right_start_x + right_size_x,
        left_end_y,
        &clip_right,
    );
}

pub fn w3d_gadget_horizontal_slider_image_draw_b(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let slider_data = window.get_user_data::<crate::gui::window_script::SliderData>();

    let (display_w, display_h) = with_window_manager_ref(|manager| manager.screen_size());
    let x_multi = display_w as f32 / 800.0;
    let y_multi = display_h as f32 / 600.0;

    let x_offset = inst_data.image_offset.x;
    let y_offset = inst_data.image_offset.y;

    let mut tooltip = format!(
        "mult:{:.4}/{:.4}, img offset:{},{}",
        x_multi, y_multi, x_offset, y_offset
    );

    tooltip.push_str(&format!(
        "\norigin: {},{} size:{},{}",
        origin_x, origin_y, size_x, size_y
    ));

    let (min_val, max_val, num_ticks, position) = if let Some(s) = slider_data {
        (s.min_value, s.max_value, s.num_ticks, s.position)
    } else {
        (0, 100, 10.0, 0)
    };
    tooltip.push_str(&format!(
        "\ns= {} <--> {}, numTicks={:.4}, pos = {}",
        min_val, max_val, num_ticks, position
    ));

    let (draw_data, _) = if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled()
    {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };

    if inst_data.state.contains(WindowState::HILITED) {
        let highlight_square = draw_data[0].image.as_ref();
        if let Some(highlight_square) = highlight_square {
            let hw = highlight_square.width.max(1);
            let hh = highlight_square.height.max(1);
            let mut background_start_x = origin_x - ((hw as f32 * x_multi) / 2.0).round() as i32;
            let background_start_y = origin_y + ((hh as f32 * y_multi) / 3.0).round() as i32;
            let background_end_y = background_start_y + (hh as f32 * y_multi).round() as i32;
            let mut background_end_x = background_start_x + (hw as f32 * x_multi).round() as i32;

            tooltip.push_str(&format!(
                "\nHighlighted: ({},{}) -> ({},{}), step {}/({}), full {}/{}",
                background_start_x,
                background_start_y,
                background_end_x,
                background_end_y,
                hw,
                hw as f32 * x_multi,
                origin_x,
                size_x
            ));

            while background_start_x < origin_x + size_x {
                with_window_manager_ref(|manager| {
                    manager.win_draw_image(
                        highlight_square,
                        background_start_x,
                        background_start_y,
                        background_end_x,
                        background_end_y,
                        WIN_COLOR_UNDEFINED,
                    );
                });
                background_start_x = background_end_x;
                background_end_x = background_start_x + (hw as f32 * x_multi).round() as i32;
            }
            tooltip.push_str(&format!(
                "\n  bsX = {}, beX = {} ({} < {}+{} or {}?)",
                background_start_x,
                background_end_x,
                background_start_x,
                origin_x,
                size_x,
                origin_x + size_x
            ));
        }
    }

    // Draw filled squares up to position
    let fill_square = draw_data[0].image.as_ref();
    if let Some(fill_square) = fill_square {
        let fw = fill_square.width.max(1);
        let fh = fill_square.height.max(1);
        let mut start_x = origin_x;
        let start_y = origin_y;
        let end_y = start_y + (fh as f32 * y_multi).round() as i32;
        let mut end_x = start_x + (fw as f32 * x_multi).round() as i32;

        tooltip.push_str(&format!(
            "\ntop: start={},{}, end={},{}",
            start_x, start_y, end_x, end_y
        ));

        let max_selected_x = origin_x + (num_ticks * (position - min_val) as f32) as i32;
        while start_x <= max_selected_x && end_x < origin_x + size_x && position != min_val {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    fill_square,
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
            start_x = end_x + 2;
            end_x = start_x + (fw as f32 * x_multi).round() as i32;
        }
    }

    // Draw blank squares for the rest
    let blank_square = draw_data[1].image.as_ref();
    if let Some(blank_square) = blank_square {
        let bw = blank_square.width.max(1);
        let bh = blank_square.height.max(1);
        let mut start_x = origin_x + (num_ticks * (position - min_val) as f32) as i32;
        let start_y = origin_y;
        let end_y = start_y + (bh as f32 * y_multi).round() as i32;
        let mut end_x = start_x + (bw as f32 * x_multi).round() as i32;

        while end_x < origin_x + size_x {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    blank_square,
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
            start_x = end_x + 2;
            end_x = start_x + (bw as f32 * x_multi).round() as i32;
        }
    }
}

pub fn w3d_gadget_vertical_slider_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    w3d_gadget_horizontal_slider_draw(window, inst_data);
}

pub fn w3d_gadget_vertical_slider_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, _) = if !window.is_enabled() {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else if inst_data.state.contains(WindowState::HILITED) {
        (&inst_data.hilite_draw_data, &inst_data.hilite_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    };

    let top_image = draw_data[0].image.as_ref();
    let bottom_image = draw_data[1].image.as_ref();
    let center_image = draw_data[2].image.as_ref();
    let small_center_image = draw_data[3].image.as_ref();

    if top_image.is_none()
        || bottom_image.is_none()
        || center_image.is_none()
        || small_center_image.is_none()
    {
        return;
    }
    let top_image = top_image.unwrap();
    let bottom_image = bottom_image.unwrap();
    let center_image = center_image.unwrap();
    let small_center_image = small_center_image.unwrap();

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let x_offset = inst_data.image_offset.x;
    let y_offset = inst_data.image_offset.y;

    let top_width = top_image.width;
    let top_height = top_image.height;
    let bottom_width = bottom_image.width;
    let bottom_height = bottom_image.height;

    if top_height + bottom_height >= size_y {
        // top and bottom images overlap or fill the whole window
        // draw top end in first half
        let start_x = origin_x + x_offset;
        let start_y = origin_y + y_offset;
        let end_x = origin_x + x_offset + top_width;
        let end_y = origin_y + size_y / 2;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                top_image,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });

        // draw bottom end in second half
        let start_y = origin_y + size_y / 2;
        let end_x = origin_x + x_offset + bottom_width;
        let end_y = origin_y + y_offset + size_y;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                bottom_image,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    } else {
        // get two key points used in the end drawing
        let top_end_x = origin_x + top_width + x_offset;
        let top_end_y = origin_y + top_height + y_offset;
        let bottom_start_x = origin_x + x_offset;
        let bottom_start_y = origin_y + size_y - bottom_height + y_offset;

        // draw the center repeating bar
        let center_height = bottom_start_y - top_end_y;
        let pieces = center_height / center_image.height.max(1);

        let start_x = origin_x + x_offset;
        let mut start_y = top_end_y;
        let end_x = start_x + center_image.width;
        for _ in 0..pieces {
            let end_y = start_y + center_image.height;
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    center_image,
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
            start_y += center_image.height;
        }

        // fill remaining gap with small center pieces, overlapping underneath the bottom end
        let center_height = bottom_start_y - start_y;
        let pieces = center_height / small_center_image.height.max(1) + 1;
        let end_x = start_x + small_center_image.width;
        for _ in 0..pieces {
            let end_y = start_y + small_center_image.height;
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    small_center_image,
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
            start_y += small_center_image.height;
        }

        // draw top end
        let start_x = origin_x + x_offset;
        let start_y = origin_y + y_offset;
        let end_x = top_end_x;
        let end_y = top_end_y;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                top_image,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });

        // draw bottom end
        let start_x = bottom_start_x;
        let start_y = bottom_start_y;
        let end_x = start_x + bottom_width;
        let end_y = start_y + bottom_height;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                bottom_image,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    }
}

fn draw_text_entry_text(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    text_color: u32,
    drop_color: u32,
    composite_color: u32,
    composite_drop: u32,
    start_x: i32,
    start_y: i32,
    width: i32,
    font_height: i32,
) {
    let Some(widget) = window.widget() else {
        return;
    };
    let super::game_window::WindowWidget::TextEntry(entry) = widget else {
        return;
    };

    let text = entry.displayed_text();
    if text.is_empty() {
        return;
    }

    let mut display = if let Some(display) = inst_data.display_text.as_ref() {
        display.borrow_mut()
    } else {
        return;
    };
    display.set_text(text.to_string());
    if let Some(font) = inst_data.font.as_ref() {
        display.set_font(font);
    }
    display.set_clip_region(Some(IRegion2D {
        x: start_x,
        y: start_y,
        width: width.max(0),
        height: font_height.max(0),
    }));

    display.draw(start_x, start_y, text_color, drop_color);
    let mut cursor_pos = start_x + display.get_width(entry.cursor_position() as i32);

    if !entry.ime_composition().is_empty() {
        let comp_text = entry.ime_composition().to_string();
        let comp_x = start_x + display.get_width(-1);
        display.set_text(comp_text);
        display.draw(comp_x, start_y, composite_color, composite_drop);
        cursor_pos += display.get_width(entry.ime_cursor() as i32);
    }

    static DRAW_CNT: AtomicU8 = AtomicU8::new(0);
    let cnt = DRAW_CNT.fetch_add(1, Ordering::Relaxed);
    if (cnt >> 3) & 0x1 == 1 {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                text_color,
                1.0,
                cursor_pos,
                start_y + 3,
                cursor_pos + 2,
                start_y + font_height - 3,
            );
        });
    }

    display.set_clip_region(None);
    let _ = cursor_pos;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextEntryImageMetrics {
    left_width: i32,
    right_width: i32,
    center_width: i32,
    small_center_width: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextEntryImageDraw {
    slot: usize,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
}

fn text_entry_image_draws(
    origin_x: i32,
    origin_y: i32,
    size_x: i32,
    size_y: i32,
    image_offset: Point2D,
    metrics: TextEntryImageMetrics,
) -> Vec<TextEntryImageDraw> {
    let x_offset = image_offset.x;
    let y_offset = image_offset.y;
    let left_width = metrics.left_width.max(1);
    let right_width = metrics.right_width.max(1);
    let center_width = metrics.center_width.max(1);
    let small_center_width = metrics.small_center_width.max(1);

    let left_end_x = origin_x + left_width + x_offset;
    let right_start_x = origin_x + size_x - right_width + x_offset;
    let start_y = origin_y + y_offset;
    let end_y = start_y + size_y;

    let mut draws = Vec::new();
    let mut start_x = left_end_x;
    let center_pieces = (right_start_x - left_end_x) / center_width;
    for _ in 0..center_pieces.max(0) {
        draws.push(TextEntryImageDraw {
            slot: 2,
            start_x,
            start_y,
            end_x: start_x + center_width,
            end_y,
        });
        start_x += center_width;
    }

    let small_center_pieces = ((right_start_x - start_x) / small_center_width) + 1;
    for _ in 0..small_center_pieces.max(0) {
        draws.push(TextEntryImageDraw {
            slot: 3,
            start_x,
            start_y,
            end_x: start_x + small_center_width,
            end_y,
        });
        start_x += small_center_width;
    }

    draws.push(TextEntryImageDraw {
        slot: 0,
        start_x: origin_x + x_offset,
        start_y,
        end_x: left_end_x,
        end_y,
    });
    draws.push(TextEntryImageDraw {
        slot: 1,
        start_x: right_start_x,
        start_y,
        end_x: right_start_x + right_width,
        end_y,
    });
    draws
}

fn draw_text_entry_image_frame(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    draw_data: &[super::game_window::WindowDrawData],
) {
    let Some(left_image) = draw_data.get(0).and_then(|entry| entry.image.as_ref()) else {
        return;
    };
    let Some(right_image) = draw_data.get(1).and_then(|entry| entry.image.as_ref()) else {
        return;
    };
    let Some(center_image) = draw_data.get(2).and_then(|entry| entry.image.as_ref()) else {
        return;
    };
    let Some(small_center_image) = draw_data.get(3).and_then(|entry| entry.image.as_ref()) else {
        return;
    };

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let draws = text_entry_image_draws(
        origin_x,
        origin_y,
        size_x,
        size_y,
        inst_data.image_offset,
        TextEntryImageMetrics {
            left_width: left_image.width,
            right_width: right_image.width,
            center_width: center_image.width,
            small_center_width: small_center_image.width,
        },
    );
    with_window_manager_ref(|manager| {
        for draw in draws {
            let image = match draw.slot {
                0 => left_image,
                1 => right_image,
                2 => center_image,
                3 => small_center_image,
                _ => continue,
            };
            manager.win_draw_image(
                image,
                draw.start_x,
                draw.start_y,
                draw.end_x,
                draw.end_y,
                WIN_COLOR_UNDEFINED,
            );
        }
    });
}

pub fn w3d_gadget_text_entry_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };
    let back = &draw_data[0];
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(
                back.border_color,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                back.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }

    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });
    let start_offset = 5;
    let width = size_x - (2 * start_offset);
    let start_x = origin_x + start_offset;
    let start_y = if window.get_status().contains(WindowStatus::ONE_LINE) {
        origin_y + (size_y / 2) - (font_height / 2)
    } else {
        origin_y + start_offset
    };

    draw_text_entry_text(
        window,
        inst_data,
        text_colors.color,
        text_colors.border_color,
        inst_data.ime_composite_text.color,
        inst_data.ime_composite_text.border_color,
        start_x,
        start_y,
        width,
        font_height,
    );
}

pub fn w3d_gadget_text_entry_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    draw_text_entry_image_frame(window, inst_data, draw_data);

    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });
    let start_offset = 5;
    let width = size_x - (2 * start_offset);
    let start_x = origin_x + start_offset;
    let start_y = if window.get_status().contains(WindowStatus::ONE_LINE) {
        origin_y + (size_y / 2) - (font_height / 2)
    } else {
        origin_y + start_offset
    };

    draw_text_entry_text(
        window,
        inst_data,
        text_colors.color,
        text_colors.border_color,
        inst_data.ime_composite_text.color,
        inst_data.ime_composite_text.border_color,
        start_x,
        start_y,
        width,
        font_height,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ListBoxSelectedImageRect {
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
}

fn list_box_selected_image_slots(images_present: [bool; 4]) -> Option<[usize; 4]> {
    if images_present.into_iter().all(|present| present) {
        Some([1, 2, 3, 4])
    } else {
        None
    }
}

fn list_box_selected_image_rect(
    x: i32,
    draw_y: i32,
    width: i32,
    line_height: i32,
    list_clip: &IRegion2D,
) -> Option<ListBoxSelectedImageRect> {
    let start_y = draw_y.max(list_clip.y);
    let end_y = (draw_y + line_height).min(region_bottom(list_clip));
    if end_y <= start_y {
        return None;
    }
    Some(ListBoxSelectedImageRect {
        start_x: x + 1,
        start_y,
        end_x: x + width,
        end_y,
    })
}

fn draw_list_box_selected_image_bar(
    draw_data: &[super::game_window::WindowDrawData],
    x: i32,
    draw_y: i32,
    width: i32,
    line_height: i32,
    list_clip: &IRegion2D,
) {
    let selected_images = [
        draw_data.get(1).and_then(|entry| entry.image.as_ref()),
        draw_data.get(2).and_then(|entry| entry.image.as_ref()),
        draw_data.get(3).and_then(|entry| entry.image.as_ref()),
        draw_data.get(4).and_then(|entry| entry.image.as_ref()),
    ];
    let Some(slots) = list_box_selected_image_slots(selected_images.map(|image| image.is_some()))
    else {
        return;
    };
    let Some(rect) = list_box_selected_image_rect(x, draw_y, width, line_height, list_clip) else {
        return;
    };
    let left = selected_images[slots[0] - 1].unwrap();
    let right = selected_images[slots[1] - 1].unwrap();
    let center = selected_images[slots[2] - 1].unwrap();
    let small_center = selected_images[slots[3] - 1].unwrap();
    draw_listbox_hilite_bar(
        left,
        right,
        center,
        small_center,
        rect.start_x,
        rect.start_y,
        rect.end_x,
        rect.end_y,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListBoxDrawMode {
    Color,
    Image,
}

fn list_box_adjusted_width_for_slider(
    width: i32,
    slider_width: Option<i32>,
    slider_hidden: bool,
    mode: ListBoxDrawMode,
) -> i32 {
    match (mode, slider_width) {
        (ListBoxDrawMode::Color, Some(slider_width)) if !slider_hidden => {
            (width - slider_width - 3).max(0)
        }
        (ListBoxDrawMode::Image, Some(slider_width)) => (width - slider_width).max(0),
        _ => width,
    }
}

pub fn w3d_gadget_list_box_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };
    let back = &draw_data[0];

    let (mut x, mut y) = window.get_screen_position();
    let (mut width, mut height) = window.get_size();
    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });

    if let Some(title) = inst_data.display_text.as_ref() {
        let mut title = title.borrow_mut();
        if let Some(font) = inst_data.font.as_ref() {
            title.set_font(font);
        }
        title.draw(x + 1, y, text_colors.color, text_colors.border_color);
        y += font_height + 1;
        height -= font_height + 1;
    }

    let mut slider_hidden = false;
    let mut slider_width = None;
    if let Some(links) = window.listbox_links() {
        if let Some(slider) = window.find_child_by_id(links.slider) {
            slider_hidden = slider.borrow().is_hidden();
            slider_width = Some(slider.borrow().get_size().0);
        }
    }

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(back.border_color, 1.0, x, y, x + width, y + height);
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(back.color, 1.0, x + 1, y + 1, x + width - 1, y + height - 1);
        });
    }

    width = list_box_adjusted_width_for_slider(
        width,
        slider_width,
        slider_hidden,
        ListBoxDrawMode::Color,
    );

    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::ListBox(listbox) = widget {
            let item_height = listbox.item_height() as i32;
            let scroll = listbox.scroll_offset() as i32 * item_height;
            let mut draw_y = y + 4 - scroll;
            let selected = listbox.selected_indices();
            let columns = listbox.columns().max(1) as usize;
            let mut column_widths = listbox.column_widths_for_width(width as u32);
            if columns == 1 && slider_hidden {
                if let Some(first) = column_widths.get_mut(0) {
                    *first = first.saturating_sub(3);
                }
            }
            let list_clip = region_from_corners(x + 1, y - 3, x + width - 1, y + height - 1);
            for (idx, item) in listbox.items().iter().enumerate() {
                if draw_y + item_height < y {
                    draw_y += item_height + 1;
                    continue;
                }
                if draw_y > y + height {
                    break;
                }
                if selected.contains(&idx) {
                    draw_list_box_selected_image_bar(
                        draw_data,
                        x,
                        draw_y,
                        width,
                        item_height + 1,
                        &list_clip,
                    );
                }
                let mut column_x = x;
                for column in 0..columns {
                    let column_width = column_widths.get(column).copied().unwrap_or(0) as i32;
                    if column_width <= 0 {
                        continue;
                    }
                    let mut column_region = region_from_corners(
                        column_x,
                        draw_y,
                        column_x + column_width,
                        draw_y + item_height,
                    );
                    if column_region.x < list_clip.x {
                        column_region.x = list_clip.x;
                    }
                    if column_region.y < list_clip.y {
                        column_region.y = list_clip.y;
                    }
                    let max_right = region_right(&list_clip);
                    let max_bottom = region_bottom(&list_clip);
                    let column_right = region_right(&column_region);
                    let column_bottom = region_bottom(&column_region);
                    if column_right > max_right {
                        column_region.width = (max_right - column_region.x).max(0);
                    }
                    if column_bottom > max_bottom {
                        column_region.height = (max_bottom - column_region.y).max(0);
                    }

                    let cell = item.column_data.get(column);
                    let column_color = item.column_colors.get(column).and_then(|color| *color);
                    match cell {
                        Some(super::gadgets::ListBoxItemData::Text(text)) => {
                            if let Some(display) = inst_data.display_text.as_ref() {
                                let mut display = display.borrow_mut();
                                display.set_text(text.clone());
                                if let Some(font) = inst_data.font.as_ref() {
                                    display.set_font(font);
                                }
                                display.set_clip_region(Some(column_region));
                                let color = gadget_color_opt_to_win_color(column_color)
                                    .or(gadget_color_opt_to_win_color(item.text_color))
                                    .unwrap_or(text_colors.color);
                                display.draw(column_x + 4, draw_y, color, text_colors.border_color);
                                display.set_clip_region(None);
                            }
                        }
                        Some(super::gadgets::ListBoxItemData::Image {
                            name,
                            width,
                            height,
                            ..
                        }) => {
                            let collection = get_mapped_image_collection();
                            if let Some(collection) = collection.try_read() {
                                if let Some(image) = collection.find_image_by_name(name) {
                                    let mut draw_width = if *width > 0 {
                                        *width
                                    } else {
                                        column_width as u32
                                    };
                                    let mut draw_height = if *height > 0 {
                                        *height
                                    } else {
                                        item_height as u32
                                    };
                                    if column == 0 && draw_width > 0 {
                                        draw_width = draw_width.saturating_sub(1);
                                    }
                                    let draw_width_i = draw_width as i32;
                                    let draw_height_i = draw_height as i32;
                                    let mut offset_x = if draw_width_i < column_width {
                                        column_x + (column_width - draw_width_i) / 2
                                    } else {
                                        column_x
                                    };
                                    let mut offset_y = if draw_height_i < item_height {
                                        draw_y + (item_height - draw_height_i) / 2
                                    } else {
                                        draw_y
                                    };
                                    offset_y += 1;
                                    if offset_x < x + 1 {
                                        offset_x = x + 1;
                                    }
                                    let draw_color = gadget_color_opt_to_win_color(column_color)
                                        .unwrap_or(WIN_COLOR_UNDEFINED);
                                    draw_mapped_image_clipped(
                                        image,
                                        offset_x,
                                        offset_y,
                                        offset_x + draw_width_i,
                                        offset_y + draw_height_i,
                                        &column_region,
                                        draw_color,
                                    );
                                }
                            };
                        }
                        _ => {
                            if column == 0 {
                                if let Some(display) = inst_data.display_text.as_ref() {
                                    let mut display = display.borrow_mut();
                                    display.set_text(item.text.clone());
                                    if let Some(font) = inst_data.font.as_ref() {
                                        display.set_font(font);
                                    }
                                    display.set_clip_region(Some(column_region));
                                    let color = gadget_color_opt_to_win_color(column_color)
                                        .or(gadget_color_opt_to_win_color(item.text_color))
                                        .unwrap_or(text_colors.color);
                                    display.draw(
                                        column_x + 4,
                                        draw_y,
                                        color,
                                        text_colors.border_color,
                                    );
                                    display.set_clip_region(None);
                                }
                            }
                        }
                    }
                    column_x += column_width;
                }
                draw_y += item_height + 1;
            }
        }
    }
}

pub fn w3d_gadget_list_box_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };

    let (mut x, mut y) = window.get_screen_position();
    let (mut width, mut height) = window.get_size();
    let mut slider_hidden = false;
    let mut slider_width = None;
    if let Some(links) = window.listbox_links() {
        if let Some(slider) = window.find_child_by_id(links.slider) {
            slider_hidden = slider.borrow().is_hidden();
            slider_width = Some(slider.borrow().get_size().0);
        }
    }
    width = list_box_adjusted_width_for_slider(
        width,
        slider_width,
        slider_hidden,
        ListBoxDrawMode::Image,
    );

    if let Some(image) = &draw_data[0].image {
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                image,
                x + inst_data.image_offset.x,
                y + inst_data.image_offset.y,
                x + inst_data.image_offset.x + width,
                y + inst_data.image_offset.y + height,
                WIN_COLOR_UNDEFINED,
            );
        });
    }

    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });
    if let Some(title) = inst_data.display_text.as_ref() {
        let mut title = title.borrow_mut();
        if let Some(font) = inst_data.font.as_ref() {
            title.set_font(font);
        }
        title.draw(x + 1, y, text_colors.color, text_colors.border_color);
        y += font_height + 1;
        height -= font_height + 1;
    }

    if let Some(widget) = window.widget() {
        if let super::game_window::WindowWidget::ListBox(listbox) = widget {
            let item_height = listbox.item_height() as i32;
            let scroll = listbox.scroll_offset() as i32 * item_height;
            let mut draw_y = y + 4 - scroll;
            let selected = listbox.selected_indices();
            let columns = listbox.columns().max(1) as usize;
            let mut column_widths = listbox.column_widths_for_width(width as u32);
            if columns == 1 && slider_hidden {
                if let Some(first) = column_widths.get_mut(0) {
                    *first = first.saturating_sub(3);
                }
            }
            let list_clip = region_from_corners(x + 1, y - 3, x + width - 1, y + height - 1);
            for (idx, item) in listbox.items().iter().enumerate() {
                if draw_y + item_height < y {
                    draw_y += item_height + 1;
                    continue;
                }
                if draw_y > y + height {
                    break;
                }
                if selected.contains(&idx) {
                    draw_list_box_selected_image_bar(
                        draw_data,
                        x,
                        draw_y,
                        width,
                        item_height + 1,
                        &list_clip,
                    );
                }
                let mut column_x = x;
                for column in 0..columns {
                    let column_width = column_widths.get(column).copied().unwrap_or(0) as i32;
                    if column_width <= 0 {
                        continue;
                    }
                    let mut column_region = region_from_corners(
                        column_x,
                        draw_y,
                        column_x + column_width,
                        draw_y + item_height,
                    );
                    if column_region.x < list_clip.x {
                        column_region.x = list_clip.x;
                    }
                    if column_region.y < list_clip.y {
                        column_region.y = list_clip.y;
                    }
                    let max_right = region_right(&list_clip);
                    let max_bottom = region_bottom(&list_clip);
                    let column_right = region_right(&column_region);
                    let column_bottom = region_bottom(&column_region);
                    if column_right > max_right {
                        column_region.width = (max_right - column_region.x).max(0);
                    }
                    if column_bottom > max_bottom {
                        column_region.height = (max_bottom - column_region.y).max(0);
                    }

                    let cell = item.column_data.get(column);
                    let column_color = item.column_colors.get(column).and_then(|color| *color);
                    match cell {
                        Some(super::gadgets::ListBoxItemData::Text(text)) => {
                            if let Some(display) = inst_data.display_text.as_ref() {
                                let mut display = display.borrow_mut();
                                display.set_text(text.clone());
                                if let Some(font) = inst_data.font.as_ref() {
                                    display.set_font(font);
                                }
                                display.set_clip_region(Some(column_region));
                                let color = gadget_color_opt_to_win_color(column_color)
                                    .or(gadget_color_opt_to_win_color(item.text_color))
                                    .unwrap_or(text_colors.color);
                                display.draw(column_x + 4, draw_y, color, text_colors.border_color);
                                display.set_clip_region(None);
                            }
                        }
                        Some(super::gadgets::ListBoxItemData::Image {
                            name,
                            width,
                            height,
                            ..
                        }) => {
                            let collection = get_mapped_image_collection();
                            if let Some(collection) = collection.try_read() {
                                if let Some(image) = collection.find_image_by_name(name) {
                                    let mut draw_width = if *width > 0 {
                                        *width
                                    } else {
                                        column_width as u32
                                    };
                                    let mut draw_height = if *height > 0 {
                                        *height
                                    } else {
                                        item_height as u32
                                    };
                                    if column == 0 && draw_width > 0 {
                                        draw_width = draw_width.saturating_sub(1);
                                    }
                                    let draw_width_i = draw_width as i32;
                                    let draw_height_i = draw_height as i32;
                                    let mut offset_x = if draw_width_i < column_width {
                                        column_x + (column_width - draw_width_i) / 2
                                    } else {
                                        column_x
                                    };
                                    let mut offset_y = if draw_height_i < item_height {
                                        draw_y + (item_height - draw_height_i) / 2
                                    } else {
                                        draw_y
                                    };
                                    offset_y += 1;
                                    if offset_x < x + 1 {
                                        offset_x = x + 1;
                                    }
                                    let draw_color = gadget_color_opt_to_win_color(column_color)
                                        .unwrap_or(WIN_COLOR_UNDEFINED);
                                    draw_mapped_image_clipped(
                                        image,
                                        offset_x,
                                        offset_y,
                                        offset_x + draw_width_i,
                                        offset_y + draw_height_i,
                                        &column_region,
                                        draw_color,
                                    );
                                }
                            };
                        }
                        _ => {
                            if column == 0 {
                                if let Some(display) = inst_data.display_text.as_ref() {
                                    let mut display = display.borrow_mut();
                                    display.set_text(item.text.clone());
                                    if let Some(font) = inst_data.font.as_ref() {
                                        display.set_font(font);
                                    }
                                    display.set_clip_region(Some(column_region));
                                    let color = gadget_color_opt_to_win_color(column_color)
                                        .or(gadget_color_opt_to_win_color(item.text_color))
                                        .unwrap_or(text_colors.color);
                                    display.draw(
                                        column_x + 4,
                                        draw_y,
                                        color,
                                        text_colors.border_color,
                                    );
                                    display.set_clip_region(None);
                                }
                            }
                        }
                    }
                    column_x += column_width;
                }
                draw_y += item_height + 1;
            }
        }
    }
}

fn draw_listbox_hilite_bar(
    left: &crate::gui::game_window::Image,
    right: &crate::gui::game_window::Image,
    center: &crate::gui::game_window::Image,
    small_center: &crate::gui::game_window::Image,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
) {
    let mut bar_width = (end_x - start_x).max(0);
    let bar_height = (end_y - start_y).max(0);
    let min_width = left.width + right.width;
    if bar_width < min_width {
        bar_width = min_width;
    }

    let left_w = left.width.max(1);
    let right_w = right.width.max(1);
    let center_w = center.width.max(1);
    let small_w = small_center.width.max(1);

    let left_end_x = start_x + left_w;
    let right_start_x = start_x + bar_width - right_w;
    let center_clip = region_from_corners(left_end_x, start_y, right_start_x, start_y + bar_height);

    let mut x = left_end_x;
    while x + center_w <= right_start_x {
        let sx = x;
        let ex = sx + center_w;
        draw_window_image_clipped(center, sx, start_y, ex, start_y + bar_height, &center_clip);
        x += center_w;
    }

    while x < right_start_x {
        let sx = x;
        let ex = (sx + small_w).min(right_start_x);
        draw_window_image_clipped(
            small_center,
            sx,
            start_y,
            ex,
            start_y + bar_height,
            &center_clip,
        );
        x += small_w;
    }

    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            left,
            start_x,
            start_y,
            left_end_x,
            start_y + bar_height,
            WIN_COLOR_UNDEFINED,
        );
        manager.win_draw_image(
            right,
            right_start_x,
            start_y,
            right_start_x + right_w,
            start_y + bar_height,
            WIN_COLOR_UNDEFINED,
        );
    });
}

fn draw_mapped_image_clipped(
    image: &crate::display::image::Image,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    clip_region: &IRegion2D,
    color: u32,
) {
    let x1 = start_x;
    let y1 = start_y;
    let x2 = end_x;
    let y2 = end_y;
    if x2 <= x1 || y2 <= y1 {
        return;
    }
    let ix1 = x1.max(clip_region.x);
    let iy1 = y1.max(clip_region.y);
    let ix2 = x2.min(region_right(clip_region));
    let iy2 = y2.min(region_bottom(clip_region));
    if ix2 <= ix1 || iy2 <= iy1 {
        return;
    }

    let dest_w = (x2 - x1) as f32;
    let dest_h = (y2 - y1) as f32;
    let left_frac = (ix1 - x1) as f32 / dest_w;
    let right_frac = (ix2 - x1) as f32 / dest_w;
    let top_frac = (iy1 - y1) as f32 / dest_h;
    let bottom_frac = (iy2 - y1) as f32 / dest_h;

    let rect = UIRect::new(
        ix1 as f32,
        iy1 as f32,
        (ix2 - ix1) as f32,
        (iy2 - iy1) as f32,
    );

    let _ = with_ui_renderer_mut(|renderer| {
        let texture = {
            let collection = get_mapped_image_collection();
            let mut collection = collection.write();
            if let Some(mapped) = collection.find_image_by_name_mut(image.get_name()) {
                if mapped.get_gpu_texture().is_none() {
                    let _ = mapped.create_gpu_texture(renderer.device(), renderer.queue());
                }
                mapped.get_gpu_texture().map(|gpu| {
                    let uv = mapped.get_uv();
                    (
                        std::sync::Arc::new(gpu.view().clone()),
                        UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height()),
                    )
                })
            } else {
                None
            }
        };
        if let Some((texture, tex_rect)) = texture {
            let uv_x = tex_rect.x + tex_rect.width * left_frac;
            let uv_y = tex_rect.y + tex_rect.height * top_frac;
            let uv_w = tex_rect.width * (right_frac - left_frac);
            let uv_h = tex_rect.height * (bottom_frac - top_frac);
            let tex_rect = UIRect::new(uv_x, uv_y, uv_w, uv_h);
            let color = if color != WIN_COLOR_UNDEFINED {
                super::game_window::color_to_rgba(color)
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            renderer.draw_textured_rect(rect, texture, color, Some(tex_rect), 0.0);
        }
    });
}

fn draw_window_image_clipped(
    image: &crate::gui::game_window::Image,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    clip_region: &IRegion2D,
) {
    let _ = ensure_client_mapped_image(&image.name);
    let collection = get_mapped_image_collection();
    let Some(collection) = collection.try_read() else {
        return;
    };
    let Some(mapped) = collection.find_image_by_name(&image.name) else {
        return;
    };
    draw_mapped_image_clipped(
        mapped,
        start_x,
        start_y,
        end_x,
        end_y,
        clip_region,
        WIN_COLOR_UNDEFINED,
    );
}

fn draw_tabcontrol_background(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    use_images: bool,
) {
    let (draw_data, _text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };

    let back = &draw_data[0];
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();

    if use_images {
        if let Some(image) = back.image.as_ref() {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    image,
                    x + inst_data.image_offset.x,
                    y + inst_data.image_offset.y,
                    x + inst_data.image_offset.x + width,
                    y + inst_data.image_offset.y + height,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }
    } else {
        if back.border_color != WIN_COLOR_UNDEFINED {
            with_window_manager_ref(|manager| {
                manager.win_open_rect(back.border_color, 1.0, x, y, x + width, y + height);
            });
        }
        if back.color != WIN_COLOR_UNDEFINED {
            with_window_manager_ref(|manager| {
                manager.win_fill_rect(back.color, 1.0, x + 1, y + 1, x + width - 1, y + height - 1);
            });
        }
    }
}

fn compute_tab_layout(
    window: &GameWindow,
    tab_control: &TabControl,
) -> (i32, i32, i32, i32, i32, i32, usize) {
    let (win_width_u, win_height_u) = window.get_size();
    let win_width = win_width_u as i32;
    let win_height = win_height_u as i32;
    let tab_count = tab_control.tab_count().min(8).max(1);
    let tab_width = tab_control.tab_width_px();
    let tab_height = tab_control.tab_height_px();
    let pane_border = tab_control.pane_border();
    let tab_edge = tab_control.tab_edge();
    let tab_orientation = tab_control.tab_orientation();

    let mut horz_offset = 0;
    let mut vert_offset = 0;

    if tab_edge == TP_TOP_SIDE || tab_edge == TP_BOTTOM_SIDE {
        if tab_orientation == TP_CENTER {
            horz_offset = win_width - (2 * pane_border) - ((tab_count as i32) * tab_width);
            horz_offset /= 2;
        } else if tab_orientation == TP_BOTTOMRIGHT {
            horz_offset = win_width - (2 * pane_border) - ((tab_count as i32) * tab_width);
        }
    } else {
        if tab_orientation == TP_CENTER {
            vert_offset = win_height - (2 * pane_border) - ((tab_count as i32) * tab_height);
            vert_offset /= 2;
        } else if tab_orientation == TP_BOTTOMRIGHT {
            vert_offset = win_height - (2 * pane_border) - ((tab_count as i32) * tab_height);
        }
    }

    let (tabs_left, tabs_top) = if tab_edge == TP_TOP_SIDE {
        (pane_border + horz_offset, pane_border)
    } else if tab_edge == TP_BOTTOM_SIDE {
        (
            pane_border + horz_offset,
            win_height - pane_border - tab_height,
        )
    } else if tab_edge == TP_RIGHT_SIDE {
        (
            win_width - pane_border - tab_width,
            pane_border + vert_offset,
        )
    } else if tab_edge == TP_LEFT_SIDE {
        (pane_border, pane_border + vert_offset)
    } else {
        (pane_border, pane_border)
    };

    let (tab_dx, tab_dy) = if tab_edge == TP_TOP_SIDE || tab_edge == TP_BOTTOM_SIDE {
        (tab_width, 0)
    } else {
        (0, tab_height)
    };

    (
        tabs_left, tabs_top, tab_width, tab_height, tab_dx, tab_dy, tab_count,
    )
}

pub fn w3d_gadget_tab_control_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_tabcontrol_background(window, inst_data, false);

    let widget = window.widget();
    let Some(super::game_window::WindowWidget::TabControl(tab_control)) = widget else {
        return;
    };

    let (tabs_left, tabs_top, tab_width, tab_height, tab_dx, tab_dy, tab_count) =
        compute_tab_layout(window, tab_control);
    let active_tab = tab_control.active_tab_index();

    for tab_index in 0..tab_count {
        let is_disabled = tab_control.is_sub_pane_disabled(tab_index);
        let draw_data = if is_disabled {
            &inst_data.disabled_draw_data
        } else if active_tab == tab_index {
            &inst_data.hilite_draw_data
        } else {
            &inst_data.enabled_draw_data
        };

        let entry_index = tab_index + 1;
        if entry_index >= draw_data.len() {
            continue;
        }
        let entry = &draw_data[entry_index];
        let tab_x = tabs_left + (tab_dx * tab_index as i32);
        let tab_y = tabs_top + (tab_dy * tab_index as i32);
        let (origin_x, origin_y) = window.get_screen_position();
        let x1 = origin_x + tab_x;
        let y1 = origin_y + tab_y;
        let x2 = x1 + tab_width;
        let y2 = y1 + tab_height;

        if entry.border_color != WIN_COLOR_UNDEFINED {
            with_window_manager_ref(|manager| {
                manager.win_open_rect(entry.border_color, 1.0, x1, y1, x2, y2);
            });
        }
        if entry.color != WIN_COLOR_UNDEFINED {
            with_window_manager_ref(|manager| {
                manager.win_fill_rect(entry.color, 1.0, x1 + 1, y1 + 1, x2 - 1, y2 - 1);
            });
        }
    }
}

pub fn w3d_gadget_tab_control_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_tabcontrol_background(window, inst_data, true);

    let widget = window.widget();
    let Some(super::game_window::WindowWidget::TabControl(tab_control)) = widget else {
        return;
    };

    let (tabs_left, tabs_top, tab_width, tab_height, tab_dx, tab_dy, tab_count) =
        compute_tab_layout(window, tab_control);
    let active_tab = tab_control.active_tab_index();

    for tab_index in 0..tab_count {
        let is_disabled = tab_control.is_sub_pane_disabled(tab_index);
        let draw_data = if is_disabled {
            &inst_data.disabled_draw_data
        } else if active_tab == tab_index {
            &inst_data.hilite_draw_data
        } else {
            &inst_data.enabled_draw_data
        };

        let entry_index = tab_index + 1;
        if entry_index >= draw_data.len() {
            continue;
        }
        let entry = &draw_data[entry_index];
        let image = match entry.image.as_ref() {
            Some(image) => image,
            None => continue,
        };

        let tab_x = tabs_left + (tab_dx * tab_index as i32);
        let tab_y = tabs_top + (tab_dy * tab_index as i32);
        let (origin_x, origin_y) = window.get_screen_position();
        let x1 = origin_x + tab_x;
        let y1 = origin_y + tab_y;
        let x2 = x1 + tab_width;
        let y2 = y1 + tab_height;

        with_window_manager_ref(|manager| {
            manager.win_draw_image(image, x1, y1, x2, y2, WIN_COLOR_UNDEFINED);
        });
    }
}

fn draw_combobox_title(
    inst_data: &WindowInstanceData,
    x: i32,
    y: i32,
    text_colors: &crate::gui::game_window::WindowTextColors,
) -> bool {
    let Some(text) = combobox_title_text(inst_data) else {
        return false;
    };

    if let Some(title) = inst_data.display_text.as_ref() {
        let mut title = title.borrow_mut();
        title.set_text(text.to_string());
        if let Some(font) = inst_data.font.as_ref() {
            title.set_font(font);
        }
        title.draw(x + 1, y, text_colors.color, text_colors.border_color);
        return true;
    }
    false
}

fn combobox_title_text(inst_data: &WindowInstanceData) -> Option<&str> {
    let text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    (!text.is_empty()).then_some(text)
}

pub fn w3d_gadget_combo_box_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };

    let (mut x, mut y) = window.get_screen_position();
    let (mut width, mut height) = window.get_size();

    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });

    if draw_combobox_title(inst_data, x, y, text_colors) {
        y += font_height + 1;
        height -= font_height + 1;
    }

    let back = &draw_data[0];
    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(back.border_color, 1.0, x, y, x + width, y + height);
        });
    }
    if back.color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(back.color, 1.0, x + 1, y + 1, x + width - 1, y + height - 1);
        });
    }
}

pub fn w3d_gadget_combo_box_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();

    if let Some(image) = &draw_data[0].image {
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                image,
                x + inst_data.image_offset.x,
                y + inst_data.image_offset.y,
                x + inst_data.image_offset.x + width,
                y + inst_data.image_offset.y + height,
                WIN_COLOR_UNDEFINED,
            );
        });
    }
    draw_combobox_title(inst_data, x, y, text_colors);
}

fn draw_skinny_border(pixel_x: i32, pixel_y: i32, width: i32, height: i32) {
    const BORDER_LINE_SIZE: i32 = 5;
    const SIZE: i32 = 5;
    const HALF_SIZE: i32 = SIZE / 2;
    const OFFSET: i32 = 2;
    const OFFSET_LOWER: i32 = 5;

    let max_x = pixel_x + width;
    let max_y = pixel_y + height;

    with_window_manager_ref(|manager| {
        let top = manager.win_find_image("FrameT");
        let bottom = manager.win_find_image("FrameB");
        if let (Some(top), Some(bottom)) = (top, bottom) {
            let top_y = pixel_y - OFFSET;
            let bottom_y = max_y - OFFSET_LOWER;
            let mut x = pixel_x + 3;
            let x_limit = max_x - (OFFSET_LOWER + SIZE);
            while x <= x_limit {
                manager.win_draw_image(&top, x, top_y, x + SIZE, top_y + SIZE, WIN_COLOR_UNDEFINED);
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                x += SIZE;
            }
            let border_end = max_x - SIZE;
            if (border_end - x) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &top,
                    x,
                    top_y,
                    x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                x += BORDER_LINE_SIZE / 2;
            }
            if x < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - x) + 1) & !1);
                x -= adjust;
                manager.win_draw_image(
                    &top,
                    x,
                    top_y,
                    x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        let left = manager.win_find_image("FrameL");
        let right = manager.win_find_image("FrameR");
        if let (Some(left), Some(right)) = (left, right) {
            let left_x = pixel_x - OFFSET;
            let right_x = max_x - OFFSET_LOWER;
            let mut y = pixel_y + 3;
            let y_limit = max_y - (OFFSET_LOWER + SIZE);
            while y <= y_limit {
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                y += SIZE;
            }
            let border_end = max_y - OFFSET_LOWER;
            if (border_end - y) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                y += BORDER_LINE_SIZE / 2;
            }
            if y < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - y) + 1) & !1);
                y -= adjust;
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        for (name, x, y) in [
            ("FrameCornerUL", pixel_x - 2, pixel_y - 2),
            ("FrameCornerUR", max_x - 5, pixel_y - 2),
            ("FrameCornerLL", pixel_x - 2, max_y - 5),
            ("FrameCornerLR", max_x - 5, max_y - 5),
        ] {
            if let Some(image) = manager.win_find_image(name) {
                manager.win_draw_image(&image, x, y, x + SIZE, y + SIZE, WIN_COLOR_UNDEFINED);
            }
        }
    });
}

pub fn w3d_draw_map_preview(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (x, y) = window.get_screen_position();
    let (w, h) = window.get_size();
    if w <= 0 || h <= 0 {
        return;
    }

    let meta = window
        .get_user_data::<Option<MapMetaData>>()
        .and_then(|meta| meta.as_ref())
        .cloned();
    let Some(meta) = meta else {
        super::game_window::default_draw_callback(window, inst_data);
        draw_skinny_border(x - 1, y - 1, w + 2, h + 2);
        return;
    };

    let (ul, lr) = find_draw_positions(x, y, w, h, meta.extent);
    let fill_color: u32 = 0xFF000000;
    let line_color: u32 = 0xFF323232;

    with_window_manager_ref(|manager| {
        let map_ratio = (meta.extent.hi.x - meta.extent.lo.x) / (w as f32).max(1.0);
        let window_ratio = (meta.extent.hi.y - meta.extent.lo.y) / (h as f32).max(1.0);
        if map_ratio >= window_ratio {
            manager.win_fill_rect(fill_color, 1.0, x, y, x + w, ul.y - 1);
            manager.win_fill_rect(fill_color, 1.0, x, lr.y + 1, x + w, y + h);
            manager.win_draw_line(line_color, 1.0, x, ul.y, x + w, ul.y);
            manager.win_draw_line(line_color, 1.0, x, lr.y + 1, x + w, lr.y + 1);
        } else {
            manager.win_fill_rect(fill_color, 1.0, x, y, ul.x - 1, y + h);
            manager.win_fill_rect(fill_color, 1.0, lr.x + 1, y, x + w, y + h);
            manager.win_draw_line(line_color, 1.0, ul.x, y, ul.x, y + h);
            manager.win_draw_line(line_color, 1.0, lr.x + 1, y, lr.x + 1, y + h);
        }
    });

    if let Some(draw) = window.get_enabled_draw_data(0) {
        if window.get_status().contains(WindowStatus::IMAGE) {
            if let Some(image) = draw.image {
                with_window_manager_ref(|manager| {
                    manager.win_draw_image(&image, ul.x, ul.y, lr.x, lr.y, draw.color);
                });
            } else {
                with_window_manager_ref(|manager| {
                    manager.win_fill_rect(line_color, 1.0, ul.x, ul.y, lr.x, lr.y);
                });
            }
        } else {
            with_window_manager_ref(|manager| {
                manager.win_fill_rect(line_color, 1.0, ul.x, ul.y, lr.x, lr.y);
            });
        }
    }

    const SUPPLY_TECH_SIZE: i32 = 15;
    let supply_and_tech = get_supply_and_tech_image_locations();
    let overlay = supply_and_tech.lock().unwrap_or_else(|e| e.into_inner());
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image("TecBuilding") {
            for pos in &overlay.tech_positions {
                manager.win_draw_image(
                    &image,
                    x + pos.x,
                    y + pos.y,
                    x + pos.x + SUPPLY_TECH_SIZE,
                    y + pos.y + SUPPLY_TECH_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }
        if let Some(image) = manager.win_find_image("Cash") {
            for pos in &overlay.supply_positions {
                manager.win_draw_image(
                    &image,
                    x + pos.x,
                    y + pos.y,
                    x + pos.x + SUPPLY_TECH_SIZE,
                    y + pos.y + SUPPLY_TECH_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }
    });

    draw_skinny_border(x - 1, y - 1, w + 2, h + 2);
}
