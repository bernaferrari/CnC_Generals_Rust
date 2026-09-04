use super::*;

pub(super) fn draw_tabcontrol_background(
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

pub(super) fn compute_tab_layout(
    window: &GameWindow,
    tab_control: &TabControl,
) -> (i32, i32, i32, i32, i32, i32, usize) {
    let (win_width_u, win_height_u) = window.get_size();
    let win_width = win_width_u;
    let win_height = win_height_u;
    let tab_count = tab_control.tab_count().min(8);
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
    let Some(crate::gui::game_window::WindowWidget::TabControl(tab_control)) = widget else {
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
    let Some(crate::gui::game_window::WindowWidget::TabControl(tab_control)) = widget else {
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

pub(super) fn draw_combobox_title(
    inst_data: &WindowInstanceData,
    x: i32,
    y: i32,
    text_colors: &crate::gui::game_window::WindowTextColors,
) -> bool {
    let text = if !inst_data.text.is_empty() {
        inst_data.text.as_str()
    } else {
        inst_data.text_label.as_str()
    };
    if text.is_empty() {
        return false;
    }
    if let Some(title) = inst_data.display_text.as_ref() {
        let mut title = title.borrow_mut();
        title.set_text(text.to_string());
        // C++ combo titles never wrap: W3DGadgetComboBoxDraw draws the title
        // with a plain `title->draw(x + 1, y, ...)` (W3DComboBox.cpp:98) and
        // nothing ever calls setWordWrap on the combo title display string.
        // The shared display string must not inherit a wrap width from other
        // gadget draws.
        title.set_word_wrap(0);
        if let Some(font) = inst_data.font.as_ref() {
            title.set_font(font);
        }
        title.draw(x + 1, y, text_colors.color, text_colors.border_color);
        return true;
    }
    false
}

pub(super) fn combo_box_title_adjustment(
    title_drawn: bool,
    font_height: i32,
    image_draw: bool,
) -> Option<(i32, i32)> {
    if !title_drawn {
        return None;
    }
    let y_delta = if image_draw {
        font_height
    } else {
        font_height + 1
    };
    Some((y_delta, font_height + 1))
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

    if let Some((y_delta, height_delta)) = combo_box_title_adjustment(
        draw_combobox_title(inst_data, x, y, text_colors),
        font_height,
        false,
    ) {
        y += y_delta;
        height -= height_delta;
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

    let (mut x, mut y) = window.get_screen_position();
    let (mut width, mut height) = window.get_size();

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

    if let Some((y_delta, height_delta)) = combo_box_title_adjustment(
        draw_combobox_title(inst_data, x, y, text_colors),
        font_height,
        true,
    ) {
        y += y_delta;
        height -= height_delta;
    }
}
