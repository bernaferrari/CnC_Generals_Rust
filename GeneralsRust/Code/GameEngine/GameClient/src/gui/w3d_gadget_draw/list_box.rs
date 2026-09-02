use super::*;

pub(super) fn list_box_selected_image_slots(images_present: [bool; 4]) -> Option<[usize; 4]> {
    if images_present.into_iter().all(|present| present) {
        Some([1, 2, 3, 4])
    } else {
        None
    }
}

pub(super) fn list_box_selected_image_rect(
    x: i32,
    draw_y: i32,
    width: i32,
    item_height: i32,
    list_clip: &IRegion2D,
) -> Option<(i32, i32, i32, i32)> {
    let start_x = x + 1;
    let mut start_y = draw_y;
    let end_x = x + width;
    let mut end_y = draw_y + item_height + 1;

    if end_y > region_bottom(list_clip) {
        end_y = region_bottom(list_clip);
    }
    if start_y < list_clip.y {
        start_y = list_clip.y;
    }

    if start_y >= end_y || start_x >= end_x {
        None
    } else {
        Some((start_x, start_y, end_x, end_y))
    }
}

pub(super) fn draw_list_box_selected_image_bar(
    draw_data: &[crate::gui::game_window::WindowDrawData],
    x: i32,
    draw_y: i32,
    width: i32,
    item_height: i32,
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
    let Some((start_x, start_y, end_x, end_y)) =
        list_box_selected_image_rect(x, draw_y, width, item_height, list_clip)
    else {
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
        start_x,
        start_y,
        end_x,
        end_y,
    );
}

pub(super) fn list_box_solid_content_width(width: i32, slider: Option<(i32, bool)>) -> i32 {
    match slider {
        Some((slider_width, false)) => (width - slider_width - 3).max(0),
        _ => width,
    }
}

pub(super) fn list_box_solid_frame_and_content_widths(
    width: i32,
    slider: Option<(i32, bool)>,
) -> (i32, i32) {
    (width, list_box_solid_content_width(width, slider))
}

pub(super) fn list_box_image_content_width(width: i32, slider_width: Option<i32>) -> i32 {
    match slider_width {
        Some(slider_width) => (width - slider_width).max(0),
        None => width,
    }
}

pub(super) const LIST_BOX_TEXT_X_OFFSET: i32 = 5;
pub(super) const LIST_BOX_TEXT_WIDTH_OFFSET: i32 = 7;

pub(super) fn draw_list_box_cell_text(
    text: &str,
    inst_data: &WindowInstanceData,
    column_region: IRegion2D,
    column_x: i32,
    draw_y: i32,
    column_width: i32,
    window_status: WindowStatus,
    text_color: u32,
    border_color: u32,
) {
    let mut display = DisplayString::new();
    display.set_text(text.to_string());
    if let Some(font) = inst_data.font.as_ref() {
        display.set_font(font);
    }
    if window_status.contains(WindowStatus::ONE_LINE) {
        display.set_word_wrap(0);
    } else {
        display.set_word_wrap(column_width - LIST_BOX_TEXT_WIDTH_OFFSET);
    }
    display.set_clip_region(Some(column_region));
    display.draw(
        column_x + LIST_BOX_TEXT_X_OFFSET,
        draw_y,
        text_color,
        border_color,
    );
    note_shipped_ui_draw_commands(1);
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
    let (width, mut height) = window.get_size();
    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });

    if let Some(title) = inst_data.display_text.as_ref() {
        let mut title = title.borrow_mut();
        if title.get_text_length() > 0 {
            if let Some(font) = inst_data.font.as_ref() {
                title.set_font(font);
            }
            title.draw(x + 1, y, text_colors.color, text_colors.border_color);
            y += font_height + 1;
            height -= font_height + 1;
        }
    }

    let mut slider_hidden = false;
    let mut slider_width = None;
    if let Some(links) = window.listbox_links() {
        if let Some(slider) = window.find_child_by_id(links.slider) {
            slider_hidden = slider.borrow().is_hidden();
            let (width, _) = slider.borrow().get_size();
            slider_width = Some(width);
        }
    }
    let (frame_width, content_width) =
        list_box_solid_frame_and_content_widths(width, slider_width.map(|w| (w, slider_hidden)));

    if back.border_color != WIN_COLOR_UNDEFINED {
        with_window_manager_ref(|manager| {
            manager.win_open_rect(back.border_color, 1.0, x, y, x + frame_width, y + height);
        });
        note_shipped_ui_draw_commands(1);
    }
    if back.color != WIN_COLOR_UNDEFINED && color_alpha(back.color) > 16 {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                back.color,
                1.0,
                x + 1,
                y + 1,
                x + frame_width - 1,
                y + height - 1,
            );
        });
        note_shipped_ui_draw_commands(1);
    } else {
        draw_visible_fill(
            x,
            y,
            frame_width,
            height,
            FALLBACK_FILL,
            Some(FALLBACK_BORDER),
        );
    }

    if let Some(widget) = window.widget() {
        if let crate::gui::game_window::WindowWidget::ListBox(listbox) = widget {
            let mut draw_y = y + 4;
            let selected = listbox.selected_indices();
            let columns = listbox.columns().max(1) as usize;
            let mut column_widths = listbox.column_widths_for_width(content_width as u32);
            if columns == 1 && slider_hidden {
                if let Some(first) = column_widths.get_mut(0) {
                    *first = first.saturating_sub(3);
                }
            }
            let list_clip =
                region_from_corners(x + 1, y - 3, x + content_width - 1, y + height - 1);
            for (idx, item) in listbox.items().iter().enumerate() {
                if idx < listbox.scroll_offset() {
                    continue;
                }
                let item_height = item.row_height.max(listbox.item_height()) as i32;
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
                        content_width,
                        item_height,
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
                        Some(crate::gui::gadgets::ListBoxItemData::Text(text)) => {
                            let color = gadget_color_opt_to_win_color(column_color)
                                .or(gadget_color_opt_to_win_color(item.text_color))
                                .unwrap_or(text_colors.color);
                            draw_list_box_cell_text(
                                text,
                                inst_data,
                                column_region,
                                column_x,
                                draw_y,
                                column_width,
                                window.get_status(),
                                color,
                                text_colors.border_color,
                            );
                        }
                        Some(crate::gui::gadgets::ListBoxItemData::Image {
                            name,
                            width,
                            height,
                            ..
                        }) => {
                            // Geometry under a short read guard; the draw
                            // re-locks the collection for write, so never hold
                            // this guard across draw_mapped_image_clipped
                            // (same-thread read->write on std RwLock deadlocks).
                            let draw_rect = {
                                let collection = get_mapped_image_collection();
                                collection.try_read().and_then(|collection| {
                                    collection.find_image_by_name(name)?;
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
                                    Some((
                                        offset_x,
                                        offset_y,
                                        offset_x + draw_width_i,
                                        offset_y + draw_height_i,
                                    ))
                                })
                            };
                            if let Some((ix1, iy1, ix2, iy2)) = draw_rect {
                                let draw_color = gadget_color_opt_to_win_color(column_color)
                                    .unwrap_or(WIN_COLOR_UNDEFINED);
                                draw_mapped_image_clipped(
                                    name,
                                    ix1,
                                    iy1,
                                    ix2,
                                    iy2,
                                    &column_region,
                                    draw_color,
                                );
                            }
                        }
                        _ => {
                            if column == 0 {
                                let color = gadget_color_opt_to_win_color(column_color)
                                    .or(gadget_color_opt_to_win_color(item.text_color))
                                    .unwrap_or(text_colors.color);
                                draw_list_box_cell_text(
                                    &item.text,
                                    inst_data,
                                    column_region,
                                    column_x,
                                    draw_y,
                                    column_width,
                                    window.get_status(),
                                    color,
                                    text_colors.border_color,
                                );
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
    if let Some(links) = window.listbox_links() {
        if let Some(slider) = window.find_child_by_id(links.slider) {
            slider_hidden = slider.borrow().is_hidden();
            let (slider_width, _) = slider.borrow().get_size();
            width = list_box_image_content_width(width, Some(slider_width));
        }
    }

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
        note_shipped_ui_draw_commands(1);
    } else {
        draw_visible_fill(x, y, width, height, FALLBACK_FILL, Some(FALLBACK_BORDER));
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
        if title.get_text_length() > 0 {
            if let Some(font) = inst_data.font.as_ref() {
                title.set_font(font);
            }
            title.draw(x + 1, y, text_colors.color, text_colors.border_color);
            y += font_height + 1;
            height -= font_height + 1;
        }
    }

    if let Some(widget) = window.widget() {
        if let crate::gui::game_window::WindowWidget::ListBox(listbox) = widget {
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
                        item_height,
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
                        Some(crate::gui::gadgets::ListBoxItemData::Text(text)) => {
                            let color = gadget_color_opt_to_win_color(column_color)
                                .or(gadget_color_opt_to_win_color(item.text_color))
                                .unwrap_or(text_colors.color);
                            draw_list_box_cell_text(
                                text,
                                inst_data,
                                column_region,
                                column_x,
                                draw_y,
                                column_width,
                                window.get_status(),
                                color,
                                text_colors.border_color,
                            );
                        }
                        Some(crate::gui::gadgets::ListBoxItemData::Image {
                            name,
                            width,
                            height,
                            ..
                        }) => {
                            // Geometry under a short read guard; the draw
                            // re-locks the collection for write, so never hold
                            // this guard across draw_mapped_image_clipped
                            // (same-thread read->write on std RwLock deadlocks).
                            let draw_rect = {
                                let collection = get_mapped_image_collection();
                                collection.try_read().and_then(|collection| {
                                    collection.find_image_by_name(name)?;
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
                                    Some((
                                        offset_x,
                                        offset_y,
                                        offset_x + draw_width_i,
                                        offset_y + draw_height_i,
                                    ))
                                })
                            };
                            if let Some((ix1, iy1, ix2, iy2)) = draw_rect {
                                let draw_color = gadget_color_opt_to_win_color(column_color)
                                    .unwrap_or(WIN_COLOR_UNDEFINED);
                                draw_mapped_image_clipped(
                                    name,
                                    ix1,
                                    iy1,
                                    ix2,
                                    iy2,
                                    &column_region,
                                    draw_color,
                                );
                            }
                        }
                        _ => {
                            if column == 0 {
                                let color = gadget_color_opt_to_win_color(column_color)
                                    .or(gadget_color_opt_to_win_color(item.text_color))
                                    .unwrap_or(text_colors.color);
                                draw_list_box_cell_text(
                                    &item.text,
                                    inst_data,
                                    column_region,
                                    column_x,
                                    draw_y,
                                    column_width,
                                    window.get_status(),
                                    color,
                                    text_colors.border_color,
                                );
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

pub(super) fn draw_listbox_hilite_bar(
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

pub(super) fn draw_mapped_image_clipped(
    image_name: &str,
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
            if let Some(mapped) = collection.find_image_by_name_mut(image_name) {
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
                crate::gui::game_window::color_to_rgba(color)
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            renderer.draw_textured_rect(rect, texture, color, Some(tex_rect), 0.0);
        }
    });
}

pub(super) fn draw_window_image_clipped(
    image: &crate::gui::game_window::Image,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    clip_region: &IRegion2D,
) {
    let collection = get_mapped_image_collection();
    let Some(collection) = collection.try_read() else {
        return;
    };
    if collection.find_image_by_name(&image.name).is_none() {
        return;
    }
    drop(collection);
    draw_mapped_image_clipped(
        &image.name,
        start_x,
        start_y,
        end_x,
        end_y,
        clip_region,
        WIN_COLOR_UNDEFINED,
    );
}
