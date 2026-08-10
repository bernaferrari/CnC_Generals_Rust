use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextEntryImageTileKind {
    Left,
    Right,
    Center,
    SmallCenter,
}

pub(super) type TextEntryImageTileRect = (TextEntryImageTileKind, i32, i32, i32, i32);

pub(super) fn text_entry_image_tile_rects(
    origin_x: i32,
    origin_y: i32,
    size_x: i32,
    size_y: i32,
    x_offset: i32,
    y_offset: i32,
    left_width: i32,
    right_width: i32,
    center_width: i32,
    small_center_width: i32,
) -> Vec<TextEntryImageTileRect> {
    let left_width = left_width.max(1);
    let right_width = right_width.max(1);
    let center_width = center_width.max(1);
    let small_center_width = small_center_width.max(1);

    let left_end_x = origin_x + left_width + x_offset;
    let right_start_x = origin_x + size_x - right_width + x_offset;
    let start_y = origin_y + y_offset;
    let end_y = start_y + size_y;

    let mut tiles = Vec::new();
    let mut start_x = left_end_x;
    let center_span = right_start_x - left_end_x;
    let center_pieces = center_span / center_width;
    for _ in 0..center_pieces {
        let end_x = start_x + center_width;
        tiles.push((
            TextEntryImageTileKind::Center,
            start_x,
            start_y,
            end_x,
            end_y,
        ));
        start_x += center_width;
    }

    let center_span = right_start_x - start_x;
    let small_center_pieces = center_span / small_center_width + 1;
    for _ in 0..small_center_pieces {
        let end_x = start_x + small_center_width;
        tiles.push((
            TextEntryImageTileKind::SmallCenter,
            start_x,
            start_y,
            end_x,
            end_y,
        ));
        start_x += small_center_width;
    }

    tiles.push((
        TextEntryImageTileKind::Left,
        origin_x + x_offset,
        start_y,
        left_end_x,
        end_y,
    ));
    tiles.push((
        TextEntryImageTileKind::Right,
        right_start_x,
        start_y,
        right_start_x + right_width,
        end_y,
    ));
    tiles
}

pub(super) fn draw_text_entry_text(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
    text_color: u32,
    drop_color: u32,
    composite_color: u32,
    composite_drop: u32,
    start_x: i32,
    start_y: i32,
    width: i32,
    font_height: i32,
    origin_x: i32,
    origin_y: i32,
    size_x: i32,
    size_y: i32,
) {
    if !text_entry_text_color_defined(text_color) {
        return;
    }
    let Some(widget) = window.widget() else {
        return;
    };
    let crate::gui::game_window::WindowWidget::TextEntry(entry) = widget else {
        return;
    };

    let mut display = if let Some(display) = inst_data.display_text.as_ref() {
        display.borrow_mut()
    } else {
        return;
    };
    let text = text_entry_w3d_display_text(entry);
    display.set_text(text.to_string());
    if let Some(font) = inst_data.font.as_ref() {
        display.set_font(font);
    }
    let draw_from_start = entry.draw_text_from_start();
    display.set_clip_region(Some(text_entry_clip_region(
        draw_from_start,
        start_x,
        start_y,
        width,
        font_height,
        origin_x,
        origin_y,
        size_x,
        size_y,
    )));

    let text_width = display.get_width(-1);
    let draw_x = text_entry_text_draw_x(draw_from_start, start_x, width, text_width);
    display.draw(draw_x, start_y, text_color, drop_color);
    let mut cursor_pos = if text_entry_password_composition_is_masked(entry) {
        draw_x + text_width
    } else {
        draw_x + display.get_width(entry.cursor_position() as i32)
    };

    if text_entry_draws_visible_composition(entry) {
        let comp_text = entry.ime_composition().to_string();
        let comp_x = draw_x + text_width;
        display.set_text(comp_text);
        display.draw(comp_x, start_y, composite_color, composite_drop);
        cursor_pos += display.get_width(entry.ime_cursor() as i32);
    }

    pub(super) static DRAW_CNT: AtomicU8 = AtomicU8::new(0);
    let cnt = DRAW_CNT.fetch_add(1, Ordering::Relaxed);
    if text_entry_caret_has_focus(window) && (cnt >> 3) & 0x1 == 1 {
        with_window_manager_ref(|manager| {
            manager.win_fill_rect(
                text_color,
                1.0,
                cursor_pos,
                origin_y + 3,
                cursor_pos + 2,
                origin_y + size_y - 3,
            );
        });
    }

    window.set_cursor_position_from_draw(text_entry_cursor_window_x(cursor_pos, origin_x), 0);
    display.set_clip_region(None);
}

pub(super) fn text_entry_w3d_display_text(entry: &TextEntry) -> String {
    if text_entry_password_composition_is_masked(entry) {
        "*".repeat(entry.displayed_text().len() + entry.ime_composition().len())
    } else {
        entry.displayed_text().to_string()
    }
}

pub(super) fn text_entry_draws_visible_composition(entry: &TextEntry) -> bool {
    !entry.is_password() && !entry.ime_composition().is_empty()
}

pub(super) fn text_entry_password_composition_is_masked(entry: &TextEntry) -> bool {
    entry.is_password() && !entry.ime_composition().is_empty()
}

pub(super) fn text_entry_text_draw_x(draw_from_start: bool, start_x: i32, width: i32, text_width: i32) -> i32 {
    if draw_from_start {
        return start_x + 5;
    }
    let draw_x = start_x + 2;
    if text_width < width {
        return draw_x;
    }
    let half_width = width / 2;
    if half_width <= 0 {
        return draw_x;
    }
    let div = text_width / half_width - 1;
    draw_x - (div * half_width)
}

pub(super) fn text_entry_clip_region(
    draw_from_start: bool,
    start_x: i32,
    start_y: i32,
    width: i32,
    font_height: i32,
    origin_x: i32,
    origin_y: i32,
    size_x: i32,
    size_y: i32,
) -> IRegion2D {
    if draw_from_start {
        IRegion2D {
            x: origin_x,
            y: origin_y,
            width: size_x.max(0),
            height: size_y.max(0),
        }
    } else {
        IRegion2D {
            x: start_x,
            y: start_y,
            width: width.max(0),
            height: font_height.max(0),
        }
    }
}

pub(super) fn text_entry_start_y(origin_y: i32, size_y: i32, font_height: i32, one_line: bool) -> i32 {
    if one_line {
        size_y / 2 - font_height / 2
    } else {
        origin_y + 5
    }
}

pub(super) fn text_entry_cursor_window_x(cursor_pos: i32, origin_x: i32) -> i32 {
    cursor_pos + 2 - origin_x
}

pub(super) fn text_entry_text_color_defined(text_color: u32) -> bool {
    text_color != WIN_COLOR_UNDEFINED
}

pub(super) fn text_entry_caret_has_focus(window: &GameWindow) -> bool {
    let focus_id = with_window_manager_ref(|manager| {
        manager
            .get_focus()
            .as_ref()
            .map(|focus| focus.borrow().get_id())
    });
    let combo_parent_id = window.get_parent().and_then(|parent| {
        let parent = parent.borrow();
        if parent.get_style() & GWS_COMBO_BOX != 0 {
            Some(parent.get_id())
        } else {
            None
        }
    });

    text_entry_focus_matches(window.get_id(), combo_parent_id, focus_id)
}

pub(super) fn text_entry_focus_matches(
    window_id: WindowId,
    combo_parent_id: Option<WindowId>,
    focus_id: Option<WindowId>,
) -> bool {
    focus_id == Some(window_id)
        || combo_parent_id.is_some_and(|parent_id| focus_id == Some(parent_id))
}

pub fn w3d_gadget_text_entry_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
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

    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });
    let start_offset = 5;
    let width = size_x - (2 * start_offset);
    let start_x = origin_x + start_offset;
    let start_y = text_entry_start_y(
        origin_y,
        size_y,
        font_height,
        window.get_status().contains(WindowStatus::ONE_LINE),
    );

    draw_text_entry_text(
        window,
        inst_data,
        text_colors.color,
        text_colors.border_color,
        inst_data.ime_composite_text.color,
        inst_data.ime_composite_text.border_color,
        start_x,
        start_y,
        width,
        font_height,
        origin_x,
        origin_y,
        size_x,
        size_y,
    );
}

pub fn w3d_gadget_text_entry_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (draw_data, text_colors) =
        if inst_data.state.contains(WindowState::DISABLED) || !window.is_enabled() {
            (&inst_data.disabled_draw_data, &inst_data.disabled_text)
        } else if inst_data.state.contains(WindowState::HILITED) {
            (&inst_data.hilite_draw_data, &inst_data.hilite_text)
        } else {
            (&inst_data.enabled_draw_data, &inst_data.enabled_text)
        };
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let left_image = draw_data[0].image.as_ref();
    let right_image = draw_data[1].image.as_ref();
    let center_image = draw_data[2].image.as_ref();
    let small_center_image = draw_data[3].image.as_ref();

    if let (Some(left_image), Some(right_image), Some(center_image), Some(small_center_image)) =
        (left_image, right_image, center_image, small_center_image)
    {
        let tiles = text_entry_image_tile_rects(
            origin_x,
            origin_y,
            size_x,
            size_y,
            inst_data.image_offset.x,
            inst_data.image_offset.y,
            left_image.width,
            right_image.width,
            center_image.width,
            small_center_image.width,
        );
        with_window_manager_ref(|manager| {
            for (kind, start_x, start_y, end_x, end_y) in tiles {
                let image = match kind {
                    TextEntryImageTileKind::Left => left_image,
                    TextEntryImageTileKind::Right => right_image,
                    TextEntryImageTileKind::Center => center_image,
                    TextEntryImageTileKind::SmallCenter => small_center_image,
                };
                manager.win_draw_image(image, start_x, start_y, end_x, end_y, WIN_COLOR_UNDEFINED);
            }
        });
    }

    let font_height = with_window_manager_ref(|manager| {
        inst_data
            .font
            .as_ref()
            .map(|font| manager.win_font_height(font))
            .unwrap_or(12)
    });
    let start_offset = 5;
    let width = size_x - (2 * start_offset);
    let start_x = origin_x + start_offset;
    let start_y = text_entry_start_y(
        origin_y,
        size_y,
        font_height,
        window.get_status().contains(WindowStatus::ONE_LINE),
    );

    draw_text_entry_text(
        window,
        inst_data,
        text_colors.color,
        text_colors.border_color,
        inst_data.ime_composite_text.color,
        inst_data.ime_composite_text.border_color,
        start_x,
        start_y,
        width,
        font_height,
        origin_x,
        origin_y,
        size_x,
        size_y,
    );
}
