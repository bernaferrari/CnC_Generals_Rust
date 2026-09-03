use super::*;

pub fn w3d_main_menu_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    draw_main_menu_frame(window, &[0.225, 0.445, 0.6662, 0.885]);
    animate_main_menu_pulse(window, "MainMenuPulse");
}

pub fn w3d_main_menu_four_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    draw_main_menu_frame(window, &[0.295, 0.59, 0.885]);
    animate_main_menu_pulse(window, "MainMenuPulse");
}

pub fn w3d_metal_bar_menu_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let color = visible_enabled_color(window, inst_data, FALLBACK_METAL_FILL);
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    draw_visible_fill(x, y, width, height, color, Some(FALLBACK_BORDER));
    window.draw_border_w3d();
}

pub fn w3d_main_menu_map_border(window: &GameWindow, _inst_data: &WindowInstanceData) {
    pub(super) const BORDER_CORNER_SIZE: i32 = 10;
    pub(super) const BORDER_LINE_SIZE: i32 = 20;
    pub(super) const SIZE: i32 = 20;
    pub(super) const HALF_SIZE: i32 = SIZE / 2;

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
            pub(super) const COLOR: u32 = 0xFF5E86A7;
            pub(super) const COLOR_DROP: u32 = 0xFF151E26;

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
            note_shipped_ui_draw_commands(8);
        } else {
            note_shipped_ui_draw_commands(1);
        }
    });
}

pub fn w3d_main_menu_button_drop_shadow_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_push_button_image_base(window, inst_data);
    draw_main_menu_button_drop_shadow_text(window, inst_data);
    draw_video_buffer(window, inst_data);
    if let Some(widget) = window.widget() {
        if let crate::gui::game_window::WindowWidget::PushButton(button) = widget {
            draw_button_style_overlay(window, button);
        }
    }
    draw_button_overlays(window, inst_data);
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
            crate::gui::game_window::color_to_rgba(inst_data.disabled_text.border_color),
            scissor,
        );
        let _ = renderer.draw_text_simple_with_scissor(
            &text,
            glam::Vec2::new(origin_x as f32, text_y as f32),
            font_size,
            crate::gui::game_window::color_to_rgba(inst_data.disabled_text.color),
            scissor,
        );
    });
}

pub fn w3d_thin_border_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let image = window
        .get_enabled_draw_data(0)
        .and_then(|draw_data| draw_data.image);
    draw_window_image_or_fallback(window, inst_data, image.as_ref(), FALLBACK_FILL);
}

pub fn w3d_shell_menu_scheme_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // Drawing may re-enter from a shell layout callback.  It has no durable
    // shell mutation to queue, so skip the frame rather than aliasing a live
    // lifecycle borrow.
    let _ = crate::gui::shell::try_with_shell_mut(|shell| {
        if shell.is_shell_active() {
            shell.get_shell_menu_scheme_manager().draw();
        }
    });
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


/// C++ W3DControlBar.cpp:661-667 — `W3DNoDraw` has an EMPTY body (the
/// `W3DGameWinDefaultDraw` call is commented out): a NoDraw window renders
/// NOTHING even when it carries IMAGE status + draw-data images. Those images
/// are dormant data; e.g. ControlBar.wnd:Munkee (ENABLED+IMAGE, rect
/// 0,414-799,599) holds `IMAGE: InGameUIChinaBase`, which the ZH retail
/// window manager never paints. Painting it here leaked atlas art across the
/// in-match control bar as a giant blurry text band.
pub fn w3d_no_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {}
