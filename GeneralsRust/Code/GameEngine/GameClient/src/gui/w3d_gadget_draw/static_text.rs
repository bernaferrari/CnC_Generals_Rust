use super::*;

pub(super) fn draw_static_text(
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
        if let crate::gui::game_window::WindowWidget::StaticText(static_text) = widget {
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
        (text_x, text_y) = static_text_text_position(
            origin_x,
            origin_y,
            width,
            height,
            text_w,
            text_h,
            left_margin,
            top_margin,
            align,
            valign,
        );
        display.draw(text_x, text_y, text_color, drop);
        display.set_clip_region(None);
        note_shipped_ui_draw_commands(1);
        return;
    }

    let font_size = inst_data.font.as_ref().map(|font| font.size).unwrap_or(12) as f32;
    let text_w = (text.chars().count() as f32 * font_size * 0.6).round() as i32;
    let text_h = font_size.round() as i32;
    (text_x, text_y) = static_text_text_position(
        origin_x,
        origin_y,
        width,
        height,
        text_w,
        text_h,
        left_margin,
        top_margin,
        align,
        valign,
    );
    let _ = with_ui_renderer_mut(|renderer| {
        let (point_size, font_name, bold) = match inst_data.font.as_ref() {
            Some(font) => (font.size as f32, font.name.as_str(), font.bold),
            None => (12.0, "Arial", false),
        };
        let _ = renderer.draw_text_simple_named(
            &text,
            glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
            point_size,
            crate::gui::game_window::color_to_rgba(drop),
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
    note_shipped_ui_draw_commands(1);
}

pub(super) fn static_text_text_position(
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
    text_w: i32,
    text_h: i32,
    left_margin: i32,
    top_margin: i32,
    align: TextAlignment,
    valign: VerticalAlignment,
) -> (i32, i32) {
    let text_x = if align == TextAlignment::Center {
        origin_x + (width / 2) - (text_w / 2)
    } else {
        origin_x + left_margin
    };
    let text_y = if valign == VerticalAlignment::Center {
        origin_y + (height / 2) - (text_h / 2)
    } else {
        origin_y + top_margin
    };
    (text_x, text_y)
}

pub(super) fn static_text_draw_data<'a>(
    window: &GameWindow,
    inst_data: &'a WindowInstanceData,
) -> (
    &'a [crate::gui::game_window::WindowDrawData],
    &'a crate::gui::game_window::WindowTextColors,
) {
    if !window.is_enabled() || inst_data.state.contains(WindowState::DISABLED) {
        (&inst_data.disabled_draw_data, &inst_data.disabled_text)
    } else {
        (&inst_data.enabled_draw_data, &inst_data.enabled_text)
    }
}

pub(super) fn static_text_text_colors(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) -> Option<(u32, u32)> {
    let (_, text) = static_text_draw_data(window, inst_data);
    if text.color == WIN_COLOR_UNDEFINED {
        None
    } else {
        Some((text.color, text.border_color))
    }
}

pub(super) fn draw_static_text_solid_background(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let (draw_data, _) = static_text_draw_data(window, inst_data);
    let Some(entry) = draw_data.first() else {
        return;
    };

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    if entry.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(
                entry.border_color,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
        note_shipped_ui_draw_commands(1);
    }
    if entry.color != WIN_COLOR_UNDEFINED && color_alpha(entry.color) > 16 {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                entry.color,
                1.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
        note_shipped_ui_draw_commands(1);
    }
}

pub(super) fn draw_static_text_image_background(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let (draw_data, _) = static_text_draw_data(window, inst_data);
    let image = draw_data.first().and_then(|entry| entry.image.as_ref());
    draw_window_image_or_fallback(window, inst_data, image, FALLBACK_FILL);
}

pub fn w3d_gadget_static_text_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_static_text_solid_background(window, inst_data);
    if let Some((text_color, drop)) = static_text_text_colors(window, inst_data) {
        draw_static_text(window, inst_data, text_color, drop);
    }
}

pub fn w3d_gadget_static_text_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_static_text_image_background(window, inst_data);
    if let Some((text_color, drop)) = static_text_text_colors(window, inst_data) {
        draw_static_text(window, inst_data, text_color, drop);
    }
}
