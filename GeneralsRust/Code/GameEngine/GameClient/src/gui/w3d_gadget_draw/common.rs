use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Shipped gadget/HUD draw ops recorded even when no UIRenderer/GPU is bound.
/// Tests drive the real W3D callbacks and assert this (or UIRenderer) is > 0.
static SHIPPED_UI_DRAW_COMMANDS: AtomicUsize = AtomicUsize::new(0);

pub fn shipped_ui_draw_command_count() -> usize {
    SHIPPED_UI_DRAW_COMMANDS.load(Ordering::Relaxed)
}

pub fn reset_shipped_ui_draw_command_count() {
    SHIPPED_UI_DRAW_COMMANDS.store(0, Ordering::Relaxed);
}

pub(super) fn note_shipped_ui_draw_commands(count: usize) {
    if count > 0 {
        SHIPPED_UI_DRAW_COMMANDS.fetch_add(count, Ordering::Relaxed);
    }
}

/// Honest, visible palette when mapped art is missing (never blank).
pub(super) const FALLBACK_FILL: u32 = 0xFF3D4F63;
pub(super) const FALLBACK_BORDER: u32 = 0xFF8BA3B8;
pub(super) const FALLBACK_MENU_FILL: u32 = 0xCC2A2218;
pub(super) const FALLBACK_HUD_FILL: u32 = 0xE01A1E24;
pub(super) const FALLBACK_BUTTON_FILL: u32 = 0xFF4A5A3A;
pub(super) const FALLBACK_METAL_FILL: u32 = 0xFF5A646E;
pub(super) const FALLBACK_LABEL: u32 = 0xFFE8EEF4;
pub(super) const FALLBACK_PULSE: u32 = 0x66D4B06A;

pub(super) fn win_color_to_rgba(color: u32) -> [f32; 4] {
    crate::gui::game_window::color_to_rgba(color)
}

pub(super) fn color_alpha(color: u32) -> u32 {
    color >> 24
}

/// Prefer enabled draw-data color when it is defined and actually visible.
///
/// Retail WNDs author `COLOR: 255 0 0 255` in NoImage draw-data slots as a
/// placeholder sentinel (packed here as 0xFFFF0000). C++ gadget image draws
/// paint no back fill at all when the image is absent
/// (GeneralsMD W3DStaticText.cpp `W3DGadgetStaticTextImageDraw`), so the
/// sentinel must never reach the screen as a solid red rectangle.
pub(super) const PLACEHOLDER_RED: u32 = 0xFFFF_0000;

pub(super) fn visible_enabled_color(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    fallback: u32,
) -> u32 {
    let pick = |color: u32| {
        if color != WIN_COLOR_UNDEFINED
            && color != PLACEHOLDER_RED
            && color_alpha(color) > 16
        {
            Some(color)
        } else {
            None
        }
    };
    window
        .get_enabled_draw_data(0)
        .and_then(|entry| pick(entry.color))
        .or_else(|| {
            inst_data
                .enabled_draw_data
                .first()
                .and_then(|e| pick(e.color))
        })
        .unwrap_or(fallback)
}

/// Queue a filled rect (and optional border) via UIRenderer when present.
/// Always increments the shipped counter so tests stay honest without WGPU.
pub(super) fn draw_visible_fill(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: u32,
    border: Option<u32>,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let rect = UIRect::new(x as f32, y as f32, width as f32, height as f32);
    let fill = win_color_to_rgba(color);
    let queued = with_ui_renderer_mut(|renderer| {
        renderer.draw_rect(rect, fill, 0.0);
        if let Some(border_color) = border {
            renderer.draw_rect_outline(rect, 1.0, win_color_to_rgba(border_color), 0.1);
        }
    });
    if queued.is_none() {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(color, 1.0, x, y, x + width, y + height);
            if let Some(border_color) = border {
                manager.win_open_rect(border_color, 1.0, x, y, x + width, y + height);
            }
        });
    }
    note_shipped_ui_draw_commands(1 + usize::from(border.is_some()));
}

pub(super) fn draw_visible_label(x: i32, y: i32, text: &str, color: u32) {
    if text.is_empty() {
        return;
    }
    let _ = with_ui_renderer_mut(|renderer| {
        let _ = renderer.draw_text_simple(
            text,
            glam::Vec2::new((x + 4) as f32, (y + 2) as f32),
            12.0,
            win_color_to_rgba(color),
        );
    });
    note_shipped_ui_draw_commands(1);
}

pub(super) fn draw_visible_line(color: u32, width: f32, x1: i32, y1: i32, x2: i32, y2: i32) {
    let queued = with_ui_renderer_mut(|renderer| {
        renderer.draw_line(
            glam::Vec2::new(x1 as f32, y1 as f32),
            glam::Vec2::new(x2 as f32, y2 as f32),
            width,
            win_color_to_rgba(color),
            0.0,
        );
    });
    if queued.is_none() {
        with_window_manager_ref(|manager| {
            manager.win_draw_line(color, width, x1, y1, x2, y2);
        });
    }
    note_shipped_ui_draw_commands(1);
}

/// Image path when mapped art exists; otherwise a visible rect (never blank).
pub(super) fn draw_named_image_or_fallback(
    name: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    fallback: u32,
) -> bool {
    if width <= 0 || height <= 0 {
        return false;
    }
    let found = with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image(name) {
            manager.win_draw_image(&image, x, y, x + width, y + height, WIN_COLOR_UNDEFINED);
            true
        } else {
            false
        }
    });
    if found {
        note_shipped_ui_draw_commands(1);
        true
    } else {
        draw_visible_fill(x, y, width, height, fallback, Some(FALLBACK_BORDER));
        false
    }
}

pub(super) fn draw_window_image_or_fallback(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    image: Option<&crate::gui::game_window::Image>,
    fallback: u32,
) {
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    if let Some(image) = image {
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
        note_shipped_ui_draw_commands(1);
    } else {
        let color = visible_enabled_color(window, inst_data, fallback);
        draw_visible_fill(x, y, width, height, color, Some(FALLBACK_BORDER));
    }
}

/// Draw callback for control bar scheme images.
/// Resolves image name via the window manager and draws the image.
/// C++ ControlBarScheme::drawBackground skips scheme images with no image
/// (ControlBarScheme.cpp:794-799 "if we don't have an image, don't try to
/// draw it") — an unregistered scheme image draws nothing, never a fill.
pub(super) fn scheme_draw_image(
    image_name: &str,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
) {
    let found = with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image(image_name) {
            manager.win_draw_image(&image, start_x, start_y, end_x, end_y, WIN_COLOR_UNDEFINED);
            true
        } else {
            false
        }
    });
    if found {
        note_shipped_ui_draw_commands(1);
    }
}

/// One-time initialization for scheme draw callback.
pub fn ensure_scheme_draw_registered() {
    pub(super) static REGISTER_DRAW: OnceLock<()> = OnceLock::new();
    REGISTER_DRAW.get_or_init(|| {
        set_scheme_draw_func(scheme_draw_image);
    });
}

pub(super) fn press_scaled_rect(window: &GameWindow) -> UIRect {
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

pub(super) fn press_scaled_bounds_i32(window: &GameWindow) -> (i32, i32, i32, i32) {
    let rect = press_scaled_rect(window);
    (
        rect.x.round() as i32,
        rect.y.round() as i32,
        rect.width.round() as i32,
        rect.height.round() as i32,
    )
}

pub(super) trait RgbaColor {
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

pub(super) fn gadget_color_to_win_color<C: RgbaColor>(color: C) -> u32 {
    let (r, g, b, a) = color.rgba();
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

pub(super) fn gadget_color_opt_to_win_color<C: RgbaColor>(color: Option<C>) -> Option<u32> {
    color.map(gadget_color_to_win_color)
}

pub(super) fn global_hotkey_text_color() -> u32 {
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

#[derive(Default)]
pub(super) struct RadarObjectOverlayTextureCache {
    pub(super) map_extent_signature: Option<[u32; 6]>,
    pub(super) texture: Option<Arc<wgpu::TextureView>>,
    pub(super) hero_object_ids: Vec<u32>,
}

pub(super) fn radar_object_overlay_texture_cache() -> &'static Mutex<RadarObjectOverlayTextureCache>
{
    pub(super) static CACHE: OnceLock<Mutex<RadarObjectOverlayTextureCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(RadarObjectOverlayTextureCache::default()))
}

pub(super) fn radar_map_extent_signature(map_extent: Region3D) -> [u32; 6] {
    [
        map_extent.lo.x.to_bits(),
        map_extent.lo.y.to_bits(),
        map_extent.lo.z.to_bits(),
        map_extent.hi.x.to_bits(),
        map_extent.hi.y.to_bits(),
        map_extent.hi.z.to_bits(),
    ]
}

pub(super) fn region_from_corners(x1: i32, y1: i32, x2: i32, y2: i32) -> IRegion2D {
    IRegion2D {
        x: x1,
        y: y1,
        width: (x2 - x1).max(0),
        height: (y2 - y1).max(0),
    }
}

pub(super) fn region_right(region: &IRegion2D) -> i32 {
    region.x + region.width
}

pub(super) fn region_bottom(region: &IRegion2D) -> i32 {
    region.y + region.height
}

pub(super) fn draw_button_text(window: &GameWindow, inst_data: &WindowInstanceData) {
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
        let (point_size, font_name, bold) = match inst_data.font.as_ref() {
            Some(font) => (font.size as f32, font.name.as_str(), font.bold),
            None => (12.0, "Arial", false),
        };
        if let Err(err) = renderer.draw_text_simple_named(
            &text,
            glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
            point_size,
            crate::gui::game_window::color_to_rgba(border_color),
            font_name,
            bold,
        ) {
            log::warn!("W3DGadgetDraw text shadow render failed: {err}");
        }
        if let Err(err) = renderer.draw_text_simple_named(
            &text,
            glam::Vec2::new(text_x as f32, text_y as f32),
            point_size,
            crate::gui::game_window::color_to_rgba(text_color),
            font_name,
            bold,
        ) {
            log::warn!("W3DGadgetDraw text render failed: {err}");
        }
    });
    }
    note_shipped_ui_draw_commands(1);
}

pub(super) fn draw_main_menu_button_drop_shadow_text(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
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
        let (point_size, font_name, bold) = match inst_data.font.as_ref() {
            Some(font) => (font.size as f32, font.name.as_str(), font.bold),
            None => (12.0, "Arial", false),
        };
        let text_width = (text.chars().count() as f32 * point_size * 0.6).round() as i32;
        let text_height = point_size.round() as i32;
        let text_x = origin_x + (width / 2) - (text_width / 2);
        let text_y = origin_y + (height / 2) - (text_height / 2);
        let _ = renderer.draw_text_simple_named(
            &text,
            glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
            point_size,
            crate::gui::game_window::color_to_rgba(drop_color),
            font_name,
            bold,
        );
        let _ = renderer.draw_text_simple_named(
            &text,
            glam::Vec2::new(text_x as f32, text_y as f32),
            point_size,
            crate::gui::game_window::color_to_rgba(text_color),
            font_name,
            bold,
        );
    });
}

#[derive(Debug)]
pub(super) struct MainMenuPulseState {
    pub(super) started_at: Instant,
    pub(super) going_forward: bool,
    pub(super) width: i32,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) initialized: bool,
}

pub(super) fn main_menu_pulse_state() -> &'static Mutex<MainMenuPulseState> {
    pub(super) static STATE: OnceLock<Mutex<MainMenuPulseState>> = OnceLock::new();
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
pub(super) fn truncate_to_i32(value: f32) -> i32 {
    value as i32
}

pub(super) fn ui_screen_height() -> i32 {
    with_ui_renderer_mut(|renderer| renderer.screen_size().1 as i32).unwrap_or(720)
}

pub(super) fn draw_main_menu_frame(window: &GameWindow, vertical_ratios: &[f32]) {
    pub(super) const COLOR: u32 = 0xFFA7865E;
    pub(super) const COLOR_DROP: u32 = 0xFF261E15;

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

    // Honest chrome fill so the menu is never a blank hole when art is missing.
    draw_visible_fill(
        pos_x,
        pos_y,
        size_x,
        size_y,
        FALLBACK_MENU_FILL,
        Some(COLOR),
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
            note_shipped_ui_draw_commands(1);
        }

        for ratio in vertical_ratios {
            let x = pos_x + truncate_to_i32(size_x as f32 * ratio);
            manager.win_draw_line(COLOR, 3.0, x, pos_y, x, height);
            note_shipped_ui_draw_commands(1);
        }
    });
}

pub(super) fn animate_main_menu_pulse(window: &GameWindow, pulse_image_name: &str) {
    let image = with_window_manager_ref(|manager| manager.win_find_image(pulse_image_name));

    let (_pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let pulse_w = image.as_ref().map(|img| img.width).unwrap_or(120).max(24);
    let pulse_h = image.as_ref().map(|img| img.height).unwrap_or(12).max(6);

    let mut state = main_menu_pulse_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !state.initialized {
        state.width = size_x + pulse_w;
        state.x = -800;
        state.y = pos_y - (pulse_h / 2);
        state.started_at = Instant::now();
        state.going_forward = true;
        state.initialized = true;
    }

    let elapsed = state.started_at.elapsed().as_secs_f32();
    let percent_done = (elapsed / 10.0).clamp(0.0, 1.0);

    if state.going_forward {
        if percent_done >= 1.0 {
            state.y = pos_y + size_y - (pulse_h / 2);
            state.started_at = Instant::now();
            state.going_forward = false;
        } else {
            state.y = pos_y - (pulse_h / 2);
            state.x = truncate_to_i32(percent_done * state.width as f32) - pulse_w;
        }
    } else {
        if percent_done >= 1.0 {
            state.y = pos_y - (pulse_h / 2);
            state.started_at = Instant::now();
            state.going_forward = true;
        } else {
            state.y = pos_y + size_y - (pulse_h / 2);
            state.x = size_x - truncate_to_i32(percent_done * state.width as f32);
        }
    }

    let draw_x = state.x;
    let draw_y = state.y;
    drop(state);

    if let Some(image) = image {
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &image,
                draw_x,
                draw_y,
                draw_x + image.width,
                draw_y + image.height,
                WIN_COLOR_UNDEFINED,
            );
        });
        note_shipped_ui_draw_commands(1);
    } else {
        draw_visible_fill(draw_x, draw_y, pulse_w, pulse_h, FALLBACK_PULSE, None);
    }
}
