use super::*;

/// Draw callback for control bar scheme images.
/// Resolves image name via the window manager and draws the image.
pub(super) fn scheme_draw_image(image_name: &str, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image(image_name) {
            manager.win_draw_image(&image, start_x, start_y, end_x, end_y, WIN_COLOR_UNDEFINED);
        }
    });
}

/// One-time initialization for scheme draw callback.
pub(super) fn ensure_scheme_draw_registered() {
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

pub(super) fn radar_object_overlay_texture_cache() -> &'static Mutex<RadarObjectOverlayTextureCache> {
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
            let font_size = inst_data.font.as_ref().map(|font| font.size).unwrap_or(12) as f32;
            if let Err(err) = renderer.draw_text_simple(
                &text,
                glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
                font_size,
                crate::gui::game_window::color_to_rgba(border_color),
            ) {
                log::warn!("W3DGadgetDraw text shadow render failed: {err}");
            }
            if let Err(err) = renderer.draw_text_simple(
                &text,
                glam::Vec2::new(text_x as f32, text_y as f32),
                font_size,
                crate::gui::game_window::color_to_rgba(text_color),
            ) {
                log::warn!("W3DGadgetDraw text render failed: {err}");
            }
        });
    }
}

pub(super) fn draw_main_menu_button_drop_shadow_text(window: &GameWindow, inst_data: &WindowInstanceData) {
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
            crate::gui::game_window::color_to_rgba(drop_color),
        );
        let _ = renderer.draw_text_simple(
            &text,
            glam::Vec2::new(text_x as f32, text_y as f32),
            font_size,
            crate::gui::game_window::color_to_rgba(text_color),
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

pub(super) fn animate_main_menu_pulse(window: &GameWindow, pulse_image_name: &str) {
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

