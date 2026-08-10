use super::*;

pub(super) fn progress_percent(window: &GameWindow) -> i32 {
    if let Some(value) = window.get_user_data::<i32>() {
        return *value;
    }
    if let Some(widget) = window.widget() {
        if let crate::gui::game_window::WindowWidget::ProgressBar(bar) = widget {
            return bar.percentage().round() as i32;
        }
    }
    0
}

pub(super) fn progress_bar_solid_width(size_x: i32, progress: i32) -> i32 {
    (size_x * progress) / 100
}

pub(super) fn progress_bar_image_width(size_x: i32, progress: i32) -> i32 {
    ((size_x - 20) * progress) / 100
}

pub(super) fn draw_progress_bar_solid(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    back: &crate::gui::game_window::WindowDrawData,
    bar: &crate::gui::game_window::WindowDrawData,
) {
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let progress = progress_percent(window);

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

    if progress != 0 {
        let bar_width = progress_bar_solid_width(size_x, progress);
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

pub(super) fn draw_progress_bar_image(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    back_left: &crate::gui::game_window::Image,
    back_right: &crate::gui::game_window::Image,
    back_center: &crate::gui::game_window::Image,
    bar_right: &crate::gui::game_window::Image,
    bar_center: &crate::gui::game_window::Image,
) {
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let progress = progress_percent(window);
    let x_offset = inst_data.image_offset.x;
    let y_offset = inst_data.image_offset.y;

    let left_width = back_left.width.max(1);
    let right_width = back_right.width.max(1);
    let center_width = back_center.width.max(1);
    let bar_center_width = bar_center.width.max(1);
    let bar_right_width = bar_right.width.max(1);

    let left_end_x = origin_x + left_width + x_offset;
    let left_end_y = origin_y + size_y + y_offset;
    let right_start_x = origin_x + size_x - right_width + x_offset;
    let right_start_y = origin_y + y_offset;

    let mut start_x = left_end_x;
    let start_y = origin_y + y_offset;
    let end_y = start_y + size_y;
    let center_width_available = right_start_x - left_end_x;
    let pieces = center_width_available / center_width;
    for _ in 0..pieces {
        let end_x = start_x + center_width;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                back_center,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });
        start_x += center_width;
    }

    let center_width_available = right_start_x - start_x;
    if center_width_available > 0 {
        let clip = region_from_corners(start_x, start_y, right_start_x, end_y);
        draw_window_image_clipped(
            back_center,
            start_x,
            start_y,
            start_x + center_width,
            end_y,
            &clip,
        );
    }

    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            back_left,
            origin_x + x_offset,
            origin_y + y_offset,
            left_end_x,
            left_end_y,
            WIN_COLOR_UNDEFINED,
        );
        manager.win_draw_image(
            back_right,
            right_start_x,
            right_start_y,
            right_start_x + right_width,
            right_start_y + size_y,
            WIN_COLOR_UNDEFINED,
        );
    });

    let bar_width = progress_bar_image_width(size_x, progress);
    let filled_pieces = bar_width / bar_center_width;
    let mut start_x = origin_x + 10;
    let start_y = origin_y + y_offset + 5;
    let end_y = start_y + size_y - 10;
    for _ in 0..filled_pieces {
        let end_x = start_x + bar_center_width;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                bar_center,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });
        start_x += bar_center_width;
    }

    start_x = origin_x + 10 + bar_center_width * filled_pieces;
    let grey_pieces = ((size_x - 20) / bar_center_width) - filled_pieces;
    for _ in 0..grey_pieces {
        let end_x = start_x + bar_right_width;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                bar_right,
                start_x,
                start_y,
                end_x,
                end_y,
                WIN_COLOR_UNDEFINED,
            );
        });
        start_x += bar_right_width;
    }
}

pub(super) fn progress_bar_image_sources() -> (usize, usize, usize, usize, usize) {
    (0, 1, 2, 5, 6)
}

pub(super) fn progress_bar_solid_sources() -> (usize, usize) {
    (0, 4)
}

pub(super) fn progress_bar_image_draw_a_sources() -> (usize, usize, usize, usize, usize) {
    (6, 5, 0, 1, 2)
}

pub(super) fn progress_bar_image_draw_a_bank() -> PushButtonDrawBank {
    PushButtonDrawBank::Enabled
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
    let (back_index, bar_index) = progress_bar_solid_sources();
    let (Some(back), Some(bar)) = (draw_data.get(back_index), draw_data.get(bar_index)) else {
        return;
    };
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
    let (back_left_index, back_right_index, back_center_index, bar_right_index, bar_center_index) =
        progress_bar_image_sources();
    let back_left = draw_data
        .get(back_left_index)
        .and_then(|entry| entry.image.as_ref());
    let back_right = draw_data
        .get(back_right_index)
        .and_then(|entry| entry.image.as_ref());
    let back_center = draw_data
        .get(back_center_index)
        .and_then(|entry| entry.image.as_ref());
    let bar_right = draw_data
        .get(bar_right_index)
        .and_then(|entry| entry.image.as_ref());
    let bar_center = draw_data
        .get(bar_center_index)
        .and_then(|entry| entry.image.as_ref());
    let (Some(back_left), Some(back_right), Some(back_center), Some(bar_right), Some(bar_center)) =
        (back_left, back_right, back_center, bar_right, bar_center)
    else {
        return;
    };
    draw_progress_bar_image(
        window,
        inst_data,
        back_left,
        back_right,
        back_center,
        bar_right,
        bar_center,
    );
}

pub fn w3d_gadget_progress_bar_image_draw_a(window: &GameWindow, inst_data: &WindowInstanceData) {
    let progress = progress_percent(window);
    let (draw_data, _) = push_button_bank_data(inst_data, progress_bar_image_draw_a_bank());

    let (bar_center_index, bar_right_index, left_index, right_index, center_index) =
        progress_bar_image_draw_a_sources();
    let bar_center = draw_data
        .get(bar_center_index)
        .and_then(|entry| entry.image.as_ref());
    let bar_right = draw_data
        .get(bar_right_index)
        .and_then(|entry| entry.image.as_ref());
    let left = draw_data
        .get(left_index)
        .and_then(|entry| entry.image.as_ref());
    let right = draw_data
        .get(right_index)
        .and_then(|entry| entry.image.as_ref());
    let center = draw_data
        .get(center_index)
        .and_then(|entry| entry.image.as_ref());

    let (Some(bar_center), Some(_bar_right), Some(_left), Some(_right), Some(_center)) =
        (bar_center, bar_right, left, right, center)
    else {
        return;
    };

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let width = bar_center.width.max(1);
    let draw_width = progress_bar_solid_width(size_x, progress);
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

