use super::*;

pub(super) fn draw_skinny_border(pixel_x: i32, pixel_y: i32, width: i32, height: i32) {
    pub(super) const BORDER_LINE_SIZE: i32 = 5;
    pub(super) const SIZE: i32 = 5;
    pub(super) const HALF_SIZE: i32 = SIZE / 2;
    pub(super) const OFFSET: i32 = 2;
    pub(super) const OFFSET_LOWER: i32 = 5;

    let max_x = pixel_x + width;
    let max_y = pixel_y + height;

    with_window_manager_ref(|manager| {
        let top = manager.win_find_image("FrameT");
        let bottom = manager.win_find_image("FrameB");
        if let (Some(top), Some(bottom)) = (top, bottom) {
            let top_y = pixel_y - OFFSET;
            let bottom_y = max_y - OFFSET_LOWER;
            let mut x = pixel_x + 3;
            let x_limit = max_x - (OFFSET_LOWER + SIZE);
            while x <= x_limit {
                manager.win_draw_image(&top, x, top_y, x + SIZE, top_y + SIZE, WIN_COLOR_UNDEFINED);
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                x += SIZE;
            }
            let border_end = max_x - SIZE;
            if (border_end - x) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &top,
                    x,
                    top_y,
                    x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                x += BORDER_LINE_SIZE / 2;
            }
            if x < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - x) + 1) & !1);
                x -= adjust;
                manager.win_draw_image(
                    &top,
                    x,
                    top_y,
                    x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        let left = manager.win_find_image("FrameL");
        let right = manager.win_find_image("FrameR");
        if let (Some(left), Some(right)) = (left, right) {
            let left_x = pixel_x - OFFSET;
            let right_x = max_x - OFFSET_LOWER;
            let mut y = pixel_y + 3;
            let y_limit = max_y - (OFFSET_LOWER + SIZE);
            while y <= y_limit {
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                y += SIZE;
            }
            let border_end = max_y - OFFSET_LOWER;
            if (border_end - y) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                y += BORDER_LINE_SIZE / 2;
            }
            if y < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - y) + 1) & !1);
                y -= adjust;
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        for (name, x, y) in [
            ("FrameCornerUL", pixel_x - 2, pixel_y - 2),
            ("FrameCornerUR", max_x - 5, pixel_y - 2),
            ("FrameCornerLL", pixel_x - 2, max_y - 5),
            ("FrameCornerLR", max_x - 5, max_y - 5),
        ] {
            if let Some(image) = manager.win_find_image(name) {
                manager.win_draw_image(&image, x, y, x + SIZE, y + SIZE, WIN_COLOR_UNDEFINED);
            }
        }
    });
}

pub fn w3d_draw_map_preview(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (x, y) = window.get_screen_position();
    let (w, h) = window.get_size();
    if w <= 0 || h <= 0 {
        return;
    }

    let meta = window
        .get_user_data::<Option<MapMetaData>>()
        .and_then(|meta| meta.as_ref())
        .cloned();
    let Some(meta) = meta else {
        crate::gui::game_window::default_draw_callback(window, inst_data);
        draw_skinny_border(x - 1, y - 1, w + 2, h + 2);
        return;
    };

    let (ul, lr) = find_draw_positions(x, y, w, h, meta.extent);
    let fill_color: u32 = 0xFF000000;
    let line_color: u32 = 0xFF323232;

    with_window_manager_ref(|manager| {
        let map_ratio = (meta.extent.hi.x - meta.extent.lo.x) / (w as f32).max(1.0);
        let window_ratio = (meta.extent.hi.y - meta.extent.lo.y) / (h as f32).max(1.0);
        if map_ratio >= window_ratio {
            manager.win_fill_rect(fill_color, 1.0, x, y, x + w, ul.y - 1);
            manager.win_fill_rect(fill_color, 1.0, x, lr.y + 1, x + w, y + h);
            manager.win_draw_line(line_color, 1.0, x, ul.y, x + w, ul.y);
            manager.win_draw_line(line_color, 1.0, x, lr.y + 1, x + w, lr.y + 1);
        } else {
            manager.win_fill_rect(fill_color, 1.0, x, y, ul.x - 1, y + h);
            manager.win_fill_rect(fill_color, 1.0, lr.x + 1, y, x + w, y + h);
            manager.win_draw_line(line_color, 1.0, ul.x, y, ul.x, y + h);
            manager.win_draw_line(line_color, 1.0, lr.x + 1, y, lr.x + 1, y + h);
        }
    });

    if let Some(draw) = window.get_enabled_draw_data(0) {
        if window.get_status().contains(WindowStatus::IMAGE) {
            if let Some(image) = draw.image {
                with_window_manager_ref(|manager| {
                    manager.win_draw_image(&image, ul.x, ul.y, lr.x, lr.y, draw.color);
                });
            } else {
                with_window_manager_ref(|manager| {
                    manager.win_fill_rect(line_color, 1.0, ul.x, ul.y, lr.x, lr.y);
                });
            }
        } else {
            with_window_manager_ref(|manager| {
                manager.win_fill_rect(line_color, 1.0, ul.x, ul.y, lr.x, lr.y);
            });
        }
    }

    pub(super) const SUPPLY_TECH_SIZE: i32 = 15;
    let supply_and_tech = get_supply_and_tech_image_locations();
    let overlay = supply_and_tech.lock().unwrap_or_else(|e| e.into_inner());
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image("TecBuilding") {
            for pos in &overlay.tech_positions {
                manager.win_draw_image(
                    &image,
                    x + pos.x,
                    y + pos.y,
                    x + pos.x + SUPPLY_TECH_SIZE,
                    y + pos.y + SUPPLY_TECH_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }
        if let Some(image) = manager.win_find_image("Cash") {
            for pos in &overlay.supply_positions {
                manager.win_draw_image(
                    &image,
                    x + pos.x,
                    y + pos.y,
                    x + pos.x + SUPPLY_TECH_SIZE,
                    y + pos.y + SUPPLY_TECH_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }
    });

    draw_skinny_border(x - 1, y - 1, w + 2, h + 2);
}
