//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::prelude::*;

#[derive(Default, Clone)]
pub(crate) struct BorderPieces {
    corner_ul: Option<Image>,
    corner_ur: Option<Image>,
    corner_ll: Option<Image>,
    corner_lr: Option<Image>,
    vertical_left: Option<Image>,
    vertical_left_short: Option<Image>,
    horizontal_top: Option<Image>,
    horizontal_top_short: Option<Image>,
    vertical_right: Option<Image>,
    vertical_right_short: Option<Image>,
    horizontal_bottom: Option<Image>,
    horizontal_bottom_short: Option<Image>,
}

pub(crate) fn border_pieces() -> &'static BorderPieces {
    static PIECES: OnceLock<BorderPieces> = OnceLock::new();
    PIECES.get_or_init(|| {
        with_window_manager_ref(|manager| BorderPieces {
            corner_ul: manager.win_find_image("BorderCornerUL"),
            corner_ur: manager.win_find_image("BorderCornerUR"),
            corner_ll: manager.win_find_image("BorderCornerLL"),
            corner_lr: manager.win_find_image("BorderCornerLR"),
            vertical_left: manager.win_find_image("BorderLeft"),
            vertical_left_short: manager.win_find_image("BorderLeftShort"),
            horizontal_top: manager.win_find_image("BorderTop"),
            horizontal_top_short: manager.win_find_image("BorderTopShort"),
            vertical_right: manager.win_find_image("BorderRight"),
            vertical_right_short: manager.win_find_image("BorderRightShort"),
            horizontal_bottom: manager.win_find_image("BorderBottom"),
            horizontal_bottom_short: manager.win_find_image("BorderBottomShort"),
        })
    })
}

impl GameWindow {
    /// Draw W3D border art for this window (port of W3DGameWindow::winDrawBorder).
    pub fn draw_border_w3d(&self) {
        const BORDER_CORNER_SIZE: i32 = 15;
        const BORDER_LINE_SIZE: i32 = 20;
        const OFFSET: i32 = 15;
        const OFFSET_LOWER: i32 = 5;
        const HALF_SIZE: i32 = BORDER_LINE_SIZE / 2;

        let (mut original_x, mut original_y) = self.get_screen_position();
        let (mut width, mut height) = self.get_size();

        let style = self.get_style();
        let mut found = false;

        for bit in [
            GWS_PUSH_BUTTON,
            GWS_RADIO_BUTTON,
            GWS_CHECK_BOX,
            GWS_VERT_SLIDER,
            GWS_HORZ_SLIDER,
            GWS_SCROLL_LISTBOX,
            GWS_ENTRY_FIELD,
            GWS_STATIC_TEXT,
            GWS_PROGRESS_BAR,
            GWS_USER_WINDOW,
            GWS_TAB_CONTROL,
        ] {
            if style & bit == 0 {
                continue;
            }

            match bit {
                GWS_CHECK_BOX => {
                    found = true;
                }
                GWS_ENTRY_FIELD => {
                    if !self.inst_data.text.is_empty() || !self.inst_data.text_label.is_empty() {
                        let text = if !self.inst_data.text.is_empty() {
                            self.inst_data.text.as_str()
                        } else {
                            self.inst_data.text_label.as_str()
                        };
                        let mut text_width = 0;
                        with_window_manager_ref(|manager| {
                            if let Some(font) = self.inst_data.font.as_ref() {
                                manager.win_get_text_size(
                                    font,
                                    text,
                                    Some(&mut text_width),
                                    None,
                                    0,
                                );
                            }
                        });
                        width = (width - (text_width + 6)).max(0);
                        original_x += text_width + 6;
                    }

                    self.blit_border_rect(
                        original_x,
                        original_y,
                        width,
                        height,
                        OFFSET,
                        OFFSET_LOWER,
                        BORDER_LINE_SIZE,
                        HALF_SIZE,
                        BORDER_CORNER_SIZE,
                    );
                    found = true;
                }
                GWS_VERT_SLIDER | GWS_HORZ_SLIDER => {
                    found = true;
                }
                GWS_SCROLL_LISTBOX => {
                    let slider_adjustment = 0;
                    let label_adjustment = if !self.inst_data.text.is_empty()
                        || !self.inst_data.text_label.is_empty()
                    {
                        4
                    } else {
                        0
                    };

                    self.blit_border_rect(
                        original_x - 3,
                        original_y - (3 + label_adjustment),
                        width + 3 - slider_adjustment,
                        height + 6,
                        OFFSET,
                        OFFSET_LOWER,
                        BORDER_LINE_SIZE,
                        HALF_SIZE,
                        BORDER_CORNER_SIZE,
                    );
                    found = true;
                }
                GWS_RADIO_BUTTON | GWS_STATIC_TEXT | GWS_PROGRESS_BAR | GWS_PUSH_BUTTON
                | GWS_USER_WINDOW | GWS_TAB_CONTROL => {
                    self.blit_border_rect(
                        original_x,
                        original_y,
                        width,
                        height,
                        OFFSET,
                        OFFSET_LOWER,
                        BORDER_LINE_SIZE,
                        HALF_SIZE,
                        BORDER_CORNER_SIZE,
                    );
                    found = true;
                }
                _ => {}
            }

            if found {
                break;
            }
        }
    }

    pub(crate) fn blit_border_rect(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        offset: i32,
        offset_lower: i32,
        line_size: i32,
        half_size: i32,
        corner_size: i32,
    ) {
        let pieces = border_pieces().clone();
        let max_x = x + width;
        let max_y = y + height;

        with_window_manager_ref(|manager| {
            let mut draw_piece = |piece: &Option<Image>, x1: i32, y1: i32, x2: i32, y2: i32| {
                if let Some(image) = piece {
                    manager.win_draw_image(image, x1, y1, x2, y2, WIN_COLOR_UNDEFINED);
                }
            };

            // Horizontal lines
            let y_top = y - offset;
            let y_bottom = max_y - offset_lower;
            let mut x_iter = x + offset_lower;
            let x_end = max_x - (offset_lower + line_size);
            while x_iter <= x_end {
                draw_piece(
                    &pieces.horizontal_top,
                    x_iter,
                    y_top,
                    x_iter + line_size,
                    y_top + line_size,
                );
                draw_piece(
                    &pieces.horizontal_bottom,
                    x_iter,
                    y_bottom,
                    x_iter + line_size,
                    y_bottom + line_size,
                );
                x_iter += line_size;
            }

            let x_end_short = max_x - 5;
            if (x_end_short - x_iter) >= half_size {
                draw_piece(
                    &pieces.horizontal_top_short,
                    x_iter,
                    y_top,
                    x_iter + half_size,
                    y_top + line_size,
                );
                draw_piece(
                    &pieces.horizontal_bottom_short,
                    x_iter,
                    y_bottom,
                    x_iter + half_size,
                    y_bottom + line_size,
                );
                x_iter += half_size;
            }

            if x_iter < x_end_short {
                x_iter -= half_size - (((x_end_short - x_iter) + 1) & !1);
                draw_piece(
                    &pieces.horizontal_top_short,
                    x_iter,
                    y_top,
                    x_iter + half_size,
                    y_top + line_size,
                );
                draw_piece(
                    &pieces.horizontal_bottom_short,
                    x_iter,
                    y_bottom,
                    x_iter + half_size,
                    y_bottom + line_size,
                );
            }

            // Vertical lines
            let x_left = x - offset;
            let x_right = max_x - offset_lower;
            let mut y_iter = y + offset_lower;
            let y_end = max_y - (offset_lower + line_size);
            while y_iter <= y_end {
                draw_piece(
                    &pieces.vertical_left,
                    x_left,
                    y_iter,
                    x_left + line_size,
                    y_iter + line_size,
                );
                draw_piece(
                    &pieces.vertical_right,
                    x_right,
                    y_iter,
                    x_right + line_size,
                    y_iter + line_size,
                );
                y_iter += line_size;
            }

            let y_end_short = max_y - offset_lower;
            if (y_end_short - y_iter) >= half_size {
                draw_piece(
                    &pieces.vertical_left_short,
                    x_left,
                    y_iter,
                    x_left + line_size,
                    y_iter + half_size,
                );
                draw_piece(
                    &pieces.vertical_right_short,
                    x_right,
                    y_iter,
                    x_right + line_size,
                    y_iter + half_size,
                );
                y_iter += half_size;
            }

            if y_iter < y_end_short {
                y_iter -= half_size - (((y_end_short - y_iter) + 1) & !1);
                draw_piece(
                    &pieces.vertical_left_short,
                    x_left,
                    y_iter,
                    x_left + line_size,
                    y_iter + half_size,
                );
                draw_piece(
                    &pieces.vertical_right_short,
                    x_right,
                    y_iter,
                    x_right + line_size,
                    y_iter + half_size,
                );
            }

            // Corners
            draw_piece(
                &pieces.corner_ul,
                x - corner_size,
                y - corner_size,
                x - corner_size + line_size,
                y - corner_size + line_size,
            );
            draw_piece(
                &pieces.corner_ur,
                max_x - 5,
                y - corner_size,
                max_x - 5 + line_size,
                y - corner_size + line_size,
            );
            draw_piece(
                &pieces.corner_ll,
                x - corner_size,
                max_y - 5,
                x - corner_size + line_size,
                max_y - 5 + line_size,
            );
            draw_piece(
                &pieces.corner_lr,
                max_x - 5,
                max_y - 5,
                max_x - 5 + line_size,
                max_y - 5 + line_size,
            );
        });
    }
}
