use super::*;

pub(super) fn draw_video_buffer(window: &GameWindow, inst_data: &WindowInstanceData) {
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

pub(super) fn draw_overlay_image(window: &GameWindow, name: &str) {
    let (x, y, w, h) = press_scaled_bounds_i32(window);
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image(name) {
            manager.win_draw_image(&image, x, y, x + w, y + h, WIN_COLOR_UNDEFINED);
        }
    });
}

pub(super) fn draw_button_overlays(window: &GameWindow, inst_data: &WindowInstanceData) {
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

pub(super) fn draw_button_style_overlay(window: &GameWindow, button: &PushButton) {
    let (x, y, w, h) = press_scaled_bounds_i32(window);
    if let Some(ref overlay) = button.style().overlay_image {
        with_window_manager_ref(|manager| {
            if let Some(image) = manager.win_find_image(overlay) {
                manager.win_draw_image(&image, x, y, x + w, y + h, WIN_COLOR_UNDEFINED);
            }
        });
    }

    match button.consume_clock_request() {
        Some((ClockMode::Normal, progress, color)) => {
            with_window_manager_ref(|manager| {
                manager.win_draw_rect_clock(
                    x,
                    y,
                    w,
                    h,
                    progress as i32,
                    gadget_color_to_win_color(color),
                );
            });
        }
        Some((ClockMode::Inverse, progress, color)) => {
            with_window_manager_ref(|manager| {
                manager.win_draw_remaining_rect_clock(
                    x,
                    y,
                    w,
                    h,
                    progress as i32,
                    gadget_color_to_win_color(color),
                );
            });
        }
        Some((ClockMode::None, _, _)) | None => {}
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PushButtonDrawBank {
    Enabled,
    Disabled,
    Hilite,
}

pub(super) fn push_button_is_selected(state: WindowState) -> bool {
    state.contains(WindowState::SELECTED)
}

pub(super) fn push_button_bank_data(
    inst_data: &WindowInstanceData,
    bank: PushButtonDrawBank,
) -> (
    &[crate::gui::game_window::WindowDrawData],
    &crate::gui::game_window::WindowTextColors,
) {
    match bank {
        PushButtonDrawBank::Enabled => (&inst_data.enabled_draw_data, &inst_data.enabled_text),
        PushButtonDrawBank::Disabled => (&inst_data.disabled_draw_data, &inst_data.disabled_text),
        PushButtonDrawBank::Hilite => (&inst_data.hilite_draw_data, &inst_data.hilite_text),
    }
}

pub(super) fn push_button_color_entry_index(
    status: WindowStatus,
    state: WindowState,
    enabled: bool,
) -> (PushButtonDrawBank, usize) {
    let selected = push_button_is_selected(state);
    if !enabled || state.contains(WindowState::DISABLED) || !status.contains(WindowStatus::ENABLED)
    {
        (PushButtonDrawBank::Disabled, usize::from(selected))
    } else if state.contains(WindowState::HILITED) {
        (PushButtonDrawBank::Hilite, usize::from(selected))
    } else {
        (PushButtonDrawBank::Enabled, usize::from(selected))
    }
}

pub(super) fn push_button_one_image_source(
    status: WindowStatus,
    state: WindowState,
    enabled: bool,
) -> (PushButtonDrawBank, usize) {
    if status.contains(WindowStatus::USE_OVERLAY_STATES) {
        return (PushButtonDrawBank::Enabled, 0);
    }

    let selected = push_button_is_selected(state);
    if !enabled || state.contains(WindowState::DISABLED) || !status.contains(WindowStatus::ENABLED)
    {
        (PushButtonDrawBank::Disabled, usize::from(selected))
    } else if state.contains(WindowState::HILITED) {
        (PushButtonDrawBank::Hilite, usize::from(selected))
    } else if selected {
        (PushButtonDrawBank::Hilite, 1)
    } else {
        (PushButtonDrawBank::Enabled, 0)
    }
}

pub(super) fn current_push_button_draw_data<'a>(
    window: &GameWindow,
    inst_data: &'a WindowInstanceData,
) -> (
    &'a [crate::gui::game_window::WindowDrawData],
    &'a crate::gui::game_window::WindowTextColors,
) {
    let (bank, _) =
        push_button_color_entry_index(window.get_status(), inst_data.state, window.is_enabled());
    push_button_bank_data(inst_data, bank)
}

pub(super) fn button_draw_entry_image(
    draw_data: &[crate::gui::game_window::WindowDrawData],
    index: usize,
) -> Option<&crate::gui::game_window::Image> {
    draw_data.get(index).and_then(|entry| entry.image.as_ref())
}

pub(super) fn draw_push_button_image_one(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (bank, index) =
        push_button_one_image_source(window.get_status(), inst_data.state, window.is_enabled());
    let (draw_data, _) = push_button_bank_data(inst_data, bank);
    let image = button_draw_entry_image(draw_data, index);

    let Some(image) = image else {
        draw_push_button_solid_base(window, inst_data);
        return;
    };

    let rect = press_scaled_rect(window);
    let start_x = rect.x as i32 + inst_data.image_offset.x;
    let start_y = rect.y as i32 + inst_data.image_offset.y;
    let end_x = start_x + rect.width as i32;
    let end_y = start_y + rect.height as i32;
    let status = window.get_status();
    // C++ W3DPushButton.cpp:345-368 — overlay + !ENABLED + !NOT_READY + !ALWAYS_COLOR.
    let mode = if status.contains(crate::gui::game_window::WindowStatus::USE_OVERLAY_STATES)
        && !window.is_enabled()
        && !status.contains(crate::gui::game_window::WindowStatus::NOT_READY)
        && !status.contains(crate::gui::game_window::WindowStatus::ALWAYS_COLOR)
    {
        crate::display::DrawImageMode::Grayscale
    } else {
        crate::display::DrawImageMode::Alpha
    };
    with_window_manager_ref(|manager| {
        manager.win_draw_image_ex(
            image,
            start_x,
            start_y,
            end_x,
            end_y,
            WIN_COLOR_UNDEFINED,
            mode,
        );
    });
    note_shipped_ui_draw_commands(1);
}

pub(super) fn resolve_push_button_three_piece_images<'a>(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    draw_data: &'a [crate::gui::game_window::WindowDrawData],
) -> Option<(
    &'a crate::gui::game_window::Image,
    &'a crate::gui::game_window::Image,
    &'a crate::gui::game_window::Image,
)> {
    let selected = push_button_is_selected(inst_data.state);
    if window
        .get_status()
        .contains(WindowStatus::USE_OVERLAY_STATES)
    {
        return None;
    }

    let (left_idx, center_idx, right_idx) = if selected {
        (1usize, 3usize, 4usize)
    } else {
        (0usize, 5usize, 6usize)
    };

    let left = button_draw_entry_image(draw_data, left_idx)?;
    let center = button_draw_entry_image(draw_data, center_idx)?;
    let right = button_draw_entry_image(draw_data, right_idx)?;
    Some((left, center, right))
}

pub(super) fn push_button_three_piece_tail_clip(
    start_x: i32,
    right_start_x: i32,
    start_y: i32,
    end_y: i32,
    center_w: i32,
) -> Option<((i32, i32, i32, i32), IRegion2D)> {
    if start_x >= right_start_x {
        return None;
    }

    let clip = region_from_corners(start_x, start_y, right_start_x, end_y);
    Some(((start_x, start_y, start_x + center_w, end_y), clip))
}

pub(super) fn draw_push_button_image_three(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    left: &crate::gui::game_window::Image,
    center: &crate::gui::game_window::Image,
    right: &crate::gui::game_window::Image,
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

    // C++ W3DGadgetPushButtonImageDrawThree clips; debug i32 add must not panic
    // when a WND button is smaller than its art (retail MainMenu buttons).
    let left_end_x = origin_x.saturating_add(x_offset).saturating_add(left_w);
    let right_start_x = origin_x
        .saturating_add(width)
        .saturating_sub(right_w)
        .saturating_add(x_offset);
    let start_y = origin_y.saturating_add(y_offset);
    let end_y = start_y.saturating_add(height);

    with_window_manager_ref(|manager| {
        if right_start_x <= left_end_x {
            let mid_x = origin_x.saturating_add(x_offset).saturating_add(width / 2);
            manager.win_draw_image(
                left,
                origin_x.saturating_add(x_offset),
                start_y,
                mid_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
            manager.win_draw_image(
                right,
                mid_x,
                start_y,
                origin_x.saturating_add(width).saturating_add(x_offset),
                end_y,
                WIN_COLOR_UNDEFINED,
            );
            return;
        }

        let mut x = left_end_x;
        while x.saturating_add(center_w) <= right_start_x {
            manager.win_draw_image(
                center,
                x,
                start_y,
                x.saturating_add(center_w),
                end_y,
                WIN_COLOR_UNDEFINED,
            );
            x = x.saturating_add(center_w);
            if center_w <= 0 {
                break;
            }
        }

        if let Some(((tail_start_x, tail_start_y, tail_end_x, tail_end_y), clip)) =
            push_button_three_piece_tail_clip(x, right_start_x, start_y, end_y, center_w)
        {
            draw_window_image_clipped(
                center,
                tail_start_x,
                tail_start_y,
                tail_end_x,
                tail_end_y,
                &clip,
            );
        }

        manager.win_draw_image(
            left,
            origin_x.saturating_add(x_offset),
            start_y,
            left_end_x,
            end_y,
            WIN_COLOR_UNDEFINED,
        );
        manager.win_draw_image(
            right,
            right_start_x,
            start_y,
            right_start_x.saturating_add(right_w),
            end_y,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub(super) fn draw_push_button_solid_base(window: &GameWindow, inst_data: &WindowInstanceData) {
    let rect = press_scaled_rect(window);

    let (draw_data, text_colors) = current_push_button_draw_data(window, inst_data);
    let (_, color_index) =
        push_button_color_entry_index(window.get_status(), inst_data.state, window.is_enabled());
    let entry = draw_data.get(color_index);
    let fill = entry
        .map(|e| e.color)
        .filter(|&c| c != WIN_COLOR_UNDEFINED && color_alpha(c) > 16)
        .unwrap_or(visible_enabled_color(
            window,
            inst_data,
            FALLBACK_BUTTON_FILL,
        ));
    let border = entry
        .map(|e| e.border_color)
        .filter(|&c| c != WIN_COLOR_UNDEFINED);
    draw_visible_fill(
        rect.x.round() as i32,
        rect.y.round() as i32,
        rect.width.round() as i32,
        rect.height.round() as i32,
        fill,
        border.or(Some(FALLBACK_BORDER)),
    );

    let _ = text_colors;
}

pub(super) fn draw_push_button_image_base(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) = current_push_button_draw_data(window, inst_data);

    if let Some((left, center, right)) =
        resolve_push_button_three_piece_images(window, inst_data, draw_data)
    {
        draw_push_button_image_three(window, inst_data, left, center, right);
        let _ = text_colors;
        return;
    }

    let (one_bank, one_index) =
        push_button_one_image_source(window.get_status(), inst_data.state, window.is_enabled());
    let (one_draw_data, _) = push_button_bank_data(inst_data, one_bank);
    if button_draw_entry_image(one_draw_data, one_index).is_some() {
        draw_push_button_image_one(window, inst_data);
        let _ = text_colors;
        return;
    }

    // C++ W3DGadgetPushButtonImageDrawOne (W3DPushButton.cpp:288-368) draws
    // nothing when the state's image slot is empty — a button parsed with
    // WIN_STATUS_IMAGE (ControlBar.wnd command/queue buttons) stays invisible
    // until `setControlCommand` binds its cameo. Falling back to the solid
    // draw here painted the authored `255 0 0 255` placeholder as solid red.
}

pub fn w3d_gadget_push_button_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_push_button_solid_base(window, inst_data);
    draw_button_text(window, inst_data);
    draw_video_buffer(window, inst_data);
    if let Some(widget) = window.widget() {
        if let crate::gui::game_window::WindowWidget::PushButton(button) = widget {
            draw_button_style_overlay(window, button);
        }
    }
    draw_button_overlays(window, inst_data);
}

pub fn w3d_gadget_push_button_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_push_button_image_base(window, inst_data);
    draw_button_text(window, inst_data);
    draw_video_buffer(window, inst_data);
    if let Some(widget) = window.widget() {
        if let crate::gui::game_window::WindowWidget::PushButton(button) = widget {
            draw_button_style_overlay(window, button);
        }
    }
    draw_button_overlays(window, inst_data);
}
