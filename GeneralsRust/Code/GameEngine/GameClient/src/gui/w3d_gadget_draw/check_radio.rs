use super::*;

pub(super) fn draw_check_box_text(window: &GameWindow, inst_data: &WindowInstanceData) {
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
    let (size_x, size_y) = window.get_size();

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

pub(super) fn is_check_box_checked(window: &GameWindow) -> bool {
    window.instance_data().state.contains(WindowState::SELECTED)
}

pub(super) fn check_box_image_source(
    state: WindowState,
    enabled: bool,
) -> (PushButtonDrawBank, usize) {
    let bank = if !enabled || state.contains(WindowState::DISABLED) {
        PushButtonDrawBank::Disabled
    } else if state.contains(WindowState::HILITED) {
        PushButtonDrawBank::Hilite
    } else {
        PushButtonDrawBank::Enabled
    };
    let image_index = if state.contains(WindowState::SELECTED) {
        2
    } else {
        1
    };
    (bank, image_index)
}

pub(super) fn solid_check_box_mark_color(color: u32) -> Option<u32> {
    (color != WIN_COLOR_UNDEFINED).then_some(color)
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

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
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

    if let Some(mark_color) = solid_check_box_mark_color(check_box.color) {
        with_window_manager_ref(|manager| {
            manager.win_draw_line(mark_color, 1.0, box_x, box_y, box_end_x, box_end_y);
            manager.win_draw_line(mark_color, 1.0, box_x, box_end_y, box_end_x, box_y);
        });
    }

    draw_check_box_text(window, inst_data);
}

pub fn w3d_gadget_check_box_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let state = if is_check_box_checked(window) {
        inst_data.state | WindowState::SELECTED
    } else {
        inst_data.state & !WindowState::SELECTED
    };
    let (bank, image_index) = check_box_image_source(state, window.is_enabled());
    let (draw_data, _) = push_button_bank_data(inst_data, bank);
    let Some(check_box) = draw_data.get(image_index) else {
        draw_check_box_text(window, inst_data);
        return;
    };
    if let Some(image) = &check_box.image {
        let (origin_x, origin_y) = window.get_screen_position();
        let (_, size_y) = window.get_size();
        let start_x = origin_x + inst_data.image_offset.x;
        let start_y = origin_y + 3;
        let end_x = start_x + (size_y - 6);
        let end_y = start_y + (size_y - 6);
        with_window_manager_ref(|manager| {
            manager.win_draw_image(image, start_x, start_y, end_x, end_y, WIN_COLOR_UNDEFINED);
        });
    }
    draw_check_box_text(window, inst_data);
}

pub(super) fn draw_radio_button_text(window: &GameWindow, inst_data: &WindowInstanceData) {
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
    let (size_x, size_y) = window.get_size();

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

pub(super) fn is_radio_selected(window: &GameWindow) -> bool {
    window.instance_data().state.contains(WindowState::SELECTED)
}

pub(super) fn radio_button_image_sources(
    state: WindowState,
    enabled: bool,
) -> (PushButtonDrawBank, [usize; 3]) {
    if state.contains(WindowState::SELECTED) {
        (PushButtonDrawBank::Hilite, [3, 4, 5])
    } else if !enabled || state.contains(WindowState::DISABLED) {
        (PushButtonDrawBank::Disabled, [0, 1, 2])
    } else if state.contains(WindowState::HILITED) {
        (PushButtonDrawBank::Hilite, [0, 1, 2])
    } else {
        (PushButtonDrawBank::Enabled, [0, 1, 2])
    }
}

pub(super) fn radio_button_solid_box_source(
    state: WindowState,
    enabled: bool,
) -> (PushButtonDrawBank, usize) {
    let bank = if !enabled || state.contains(WindowState::DISABLED) {
        PushButtonDrawBank::Disabled
    } else if state.contains(WindowState::HILITED) {
        PushButtonDrawBank::Hilite
    } else {
        PushButtonDrawBank::Enabled
    };
    let box_index = if state.contains(WindowState::SELECTED) {
        2
    } else {
        1
    };
    (bank, box_index)
}

pub fn w3d_gadget_radio_button_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let state = if is_radio_selected(window) {
        inst_data.state | WindowState::SELECTED
    } else {
        inst_data.state & !WindowState::SELECTED
    };
    let (bank, box_index) = radio_button_solid_box_source(state, window.is_enabled());
    let (draw_data, _) = push_button_bank_data(inst_data, bank);
    let back = &draw_data[0];
    let radio_box = draw_data.get(box_index).unwrap_or(&draw_data[1]);

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

pub(super) fn radio_button_image_set_complete(images: [bool; 3]) -> bool {
    images.into_iter().all(|present| present)
}

pub(super) fn draw_radio_button_image_strip(
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
    let center_width = right_start_x - left_end_x;
    let pieces = center_width / center_w + 1;
    for _ in 0..pieces {
        let end_x = start_x + center_w;
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
    let state = if is_radio_selected(window) {
        inst_data.state | WindowState::SELECTED
    } else {
        inst_data.state & !WindowState::SELECTED
    };
    let (bank, [left_index, center_index, right_index]) =
        radio_button_image_sources(state, window.is_enabled());
    let (draw_data, _) = push_button_bank_data(inst_data, bank);
    let image_set = (
        draw_data
            .get(left_index)
            .and_then(|entry| entry.image.as_ref()),
        draw_data
            .get(center_index)
            .and_then(|entry| entry.image.as_ref()),
        draw_data
            .get(right_index)
            .and_then(|entry| entry.image.as_ref()),
    );

    let [left_present, center_present, right_present] = [
        image_set.0.is_some(),
        image_set.1.is_some(),
        image_set.2.is_some(),
    ];
    if !radio_button_image_set_complete([left_present, center_present, right_present]) {
        return;
    }
    let (Some(left), Some(center), Some(right)) = image_set else {
        return;
    };
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
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
    draw_radio_button_text(window, inst_data);
}
