use super::*;

pub(super) fn slider_percent(
    window: &GameWindow,
    slider_data: Option<&crate::gui::window_script::SliderData>,
) -> f32 {
    if let Some(widget) = window.widget() {
        match widget {
            crate::gui::game_window::WindowWidget::HorizontalSlider(slider) => {
                let (min, max) = slider.range();
                let value = slider.value();
                let range = (max - min).max(1);
                return (value - min) as f32 / range as f32;
            }
            crate::gui::game_window::WindowWidget::VerticalSlider(slider) => {
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

/// C++ gadget draw-data images are pointers into `TheMappedImageCollection`
/// carrying real dims (`W3DHorizontalSlider.cpp:132`
/// `fillSquare->getImageWidth() * xMulti`). The WND parser stores name-only
/// stubs (`window_script.rs parse_draw_data`: `Image { name, width: 0,
/// height: 0 }`), so every slider image draw must resolve the real size by
/// name at draw time or every box degenerates to a 1px speck. Guard is
/// dropped before any draw call (DriveRunner read→write discipline).
pub(super) fn mapped_image_dims(image: &crate::gui::game_window::Image) -> (i32, i32) {
    let (width, height) = if image.width > 0 && image.height > 0 {
        (image.width, image.height)
    } else {
        let collection = get_mapped_image_collection();
        let collection = collection.read();
        collection
            .find_image_by_name(&image.name)
            .map(|mapped| {
                let size = mapped.get_image_size();
                (size.x, size.y)
            })
            .unwrap_or((0, 0))
    };
    (width.max(1), height.max(1))
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

pub(super) fn horizontal_slider_box_image_sources() -> (usize, usize, usize) {
    (0, 1, 0)
}

pub(super) fn horizontal_slider_image_draw_b_sources() -> (usize, usize, usize) {
    (0, 1, 0)
}

pub(super) fn horizontal_slider_image_draw_a_sources(
    enabled: bool,
) -> [(PushButtonDrawBank, usize); 8] {
    if enabled {
        [
            (PushButtonDrawBank::Hilite, 0),
            (PushButtonDrawBank::Hilite, 1),
            (PushButtonDrawBank::Hilite, 2),
            (PushButtonDrawBank::Hilite, 3),
            (PushButtonDrawBank::Enabled, 0),
            (PushButtonDrawBank::Enabled, 1),
            (PushButtonDrawBank::Enabled, 2),
            (PushButtonDrawBank::Enabled, 3),
        ]
    } else {
        [
            (PushButtonDrawBank::Disabled, 0),
            (PushButtonDrawBank::Disabled, 1),
            (PushButtonDrawBank::Disabled, 2),
            (PushButtonDrawBank::Disabled, 3),
            (PushButtonDrawBank::Disabled, 0),
            (PushButtonDrawBank::Disabled, 1),
            (PushButtonDrawBank::Disabled, 2),
            (PushButtonDrawBank::Disabled, 3),
        ]
    }
}

pub(super) fn horizontal_slider_box_counts(
    box_width: i32,
    size_x: i32,
    selected_percent: f32,
) -> (i32, i32, i32) {
    let box_width = box_width.max(1);
    let box_padding = 2;
    let mut num_boxes = 0;
    let mut num_selected = 0;
    let mut start_x = 0;
    let mut end_x = start_x + box_width;
    let max_selected_x = (selected_percent * size_x as f32) as i32;
    while end_x < size_x {
        if start_x <= max_selected_x && end_x < size_x && selected_percent > 0.0 {
            num_selected += 1;
        }
        start_x = end_x + box_padding;
        end_x = start_x + box_width;
        num_boxes += 1;
    }
    let distance = end_x - box_width;
    let blankness = size_x - distance;
    (num_boxes, num_selected, blankness / 2)
}

pub fn w3d_gadget_horizontal_slider_image_draw(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let (filled_index, blank_index, highlight_index) = horizontal_slider_box_image_sources();
    let filled = inst_data
        .disabled_draw_data
        .get(filled_index)
        .and_then(|entry| entry.image.as_ref());
    let blank = inst_data
        .disabled_draw_data
        .get(blank_index)
        .and_then(|entry| entry.image.as_ref());
    let highlight = inst_data
        .hilite_draw_data
        .get(highlight_index)
        .and_then(|entry| entry.image.as_ref());

    let (Some(filled), Some(blank), Some(highlight)) = (filled, blank, highlight) else {
        return;
    };

    let (mut origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let slider_data = None;
    let selected_percent = slider_percent(window, slider_data);
    let x_multi = with_window_manager_ref(|manager| manager.screen_size().0 as f32 / 800.0);
    let (filled_w, _) = mapped_image_dims(filled);
    let box_width = ((filled_w as f32 * x_multi) as i32).max(1);
    let box_padding = 2;
    let (num_boxes, num_selected, origin_offset) =
        horizontal_slider_box_counts(box_width, size_x, selected_percent);
    origin_x += origin_offset;

    if inst_data.state.contains(WindowState::HILITED) {
        let mut bg_start_x = origin_x - (box_width + box_padding) / 2;
        let bg_start_y = origin_y + box_width / 3;
        let bg_end_y = bg_start_y + box_width + box_padding;
        for _ in 0..(num_boxes + 1) {
            let bg_end_x = bg_start_x + box_width + box_padding;
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    highlight,
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

    for i in 0..num_selected {
        let sx = origin_x + i * (box_width + box_padding);
        let ex = sx + box_width;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                filled,
                sx,
                origin_y,
                ex,
                origin_y + box_width,
                WIN_COLOR_UNDEFINED,
            );
        });
    }
    for i in num_selected..num_boxes {
        let sx = origin_x + i * (box_width + box_padding);
        let ex = sx + box_width;
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                blank,
                sx,
                origin_y,
                ex,
                origin_y + box_width,
                WIN_COLOR_UNDEFINED,
            );
        });
    }
}

pub(super) const HORIZONTAL_SLIDER_THUMB_WIDTH: i32 = 8;

pub fn w3d_gadget_horizontal_slider_image_draw_a(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    let source_enabled = !inst_data.state.contains(WindowState::DISABLED) && window.is_enabled();
    let sources = horizontal_slider_image_draw_a_sources(source_enabled);
    let resolve_image = |(bank, index): (PushButtonDrawBank, usize)| {
        let (draw_data, _) = push_button_bank_data(inst_data, bank);
        draw_data.get(index).and_then(|entry| entry.image.as_ref())
    };

    let Some(left_image_left) = resolve_image(sources[0]) else {
        return;
    };
    let Some(right_image_left) = resolve_image(sources[1]) else {
        return;
    };
    let Some(center_image_left) = resolve_image(sources[2]) else {
        return;
    };
    let Some(small_center_image_left) = resolve_image(sources[3]) else {
        return;
    };
    let Some(left_image_right) = resolve_image(sources[4]) else {
        return;
    };
    let Some(right_image_right) = resolve_image(sources[5]) else {
        return;
    };
    let Some(center_image_right) = resolve_image(sources[6]) else {
        return;
    };
    let Some(small_center_image_right) = resolve_image(sources[7]) else {
        return;
    };

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

    let (left_size_x, left_size_y) = mapped_image_dims(left_image_left);
    let (right_size_x, _) = mapped_image_dims(right_image_left);
    let (center_w, _) = mapped_image_dims(center_image_left);
    let (small_center_w, _) = mapped_image_dims(small_center_image_left);

    let left_end_x = origin_x + left_size_x + x_offset;
    let left_end_y = origin_y + size_y + y_offset;
    let right_start_x = origin_x + size_x - right_size_x + x_offset;
    let right_start_y = origin_y + size_y - left_size_y + y_offset;

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
    let pieces = center_width / center_w.max(1);
    let mut start_x = left_end_x;
    let start_y = origin_y + size_y - left_size_y + y_offset;
    let end_y = origin_y + size_y + y_offset;

    for _ in 0..pieces {
        let end_x = start_x + center_w;
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
        start_x += center_w;
    }

    // Draw small center pieces in the gap
    let center_width = right_start_x - start_x;
    let pieces = center_width / small_center_w.max(1) + 1;
    for _ in 0..pieces {
        let end_x = start_x + small_center_w;
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
        start_x += small_center_w;
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

    let (fill_index, blank_index, highlight_index) = horizontal_slider_image_draw_b_sources();
    let fill_square = inst_data
        .disabled_draw_data
        .get(fill_index)
        .and_then(|entry| entry.image.as_ref());
    let blank_square = inst_data
        .disabled_draw_data
        .get(blank_index)
        .and_then(|entry| entry.image.as_ref());
    let highlight_square = inst_data
        .hilite_draw_data
        .get(highlight_index)
        .and_then(|entry| entry.image.as_ref());
    let (Some(fill_square), Some(blank_square)) = (fill_square, blank_square) else {
        return;
    };

    if inst_data.state.contains(WindowState::HILITED) {
        let Some(highlight_square) = highlight_square else {
            return;
        };
        let (hw, hh) = mapped_image_dims(highlight_square);
        let mut background_start_x = origin_x - ((hw as f32 * x_multi) / 2.0) as i32;
        let background_start_y = origin_y + ((hh as f32 * y_multi) / 3.0) as i32;
        let background_end_y = background_start_y + (hh as f32 * y_multi) as i32;
        let mut background_end_x = background_start_x + (hw as f32 * x_multi) as i32;

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
            background_end_x = background_start_x + (hw as f32 * x_multi) as i32;
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

    let (fw, fh) = mapped_image_dims(fill_square);
    let mut start_x = origin_x;
    let start_y = origin_y;
    let end_y = start_y + (fh as f32 * y_multi) as i32;
    let mut end_x = start_x + (fw as f32 * x_multi) as i32;

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
        end_x = start_x + (fw as f32 * x_multi) as i32;
    }

    let (bw, _) = mapped_image_dims(blank_square);
    end_x = start_x + (bw as f32 * x_multi) as i32;

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
        end_x = start_x + (bw as f32 * x_multi) as i32;
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

    let (top_width, top_height) = mapped_image_dims(top_image);
    let (bottom_width, bottom_height) = mapped_image_dims(bottom_image);
    let (center_w, center_h) = mapped_image_dims(center_image);
    let (small_center_w, small_center_h) = mapped_image_dims(small_center_image);

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
        let pieces = center_height / center_h;

        let start_x = origin_x + x_offset;
        let mut start_y = top_end_y;
        let end_x = start_x + center_w;
        for _ in 0..pieces {
            let end_y = start_y + center_h;
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
            start_y += center_h;
        }

        // fill remaining gap with small center pieces, overlapping underneath the bottom end
        let center_height = bottom_start_y - start_y;
        let pieces = center_height / small_center_h + 1;
        let end_x = start_x + small_center_w;
        for _ in 0..pieces {
            let end_y = start_y + small_center_h;
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
            start_y += small_center_h;
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
