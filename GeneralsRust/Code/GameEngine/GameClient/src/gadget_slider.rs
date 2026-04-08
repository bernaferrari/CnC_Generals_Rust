use crate::gui::game_window::{Color, GameWindow, Image, WindowWidget};

pub const HORIZONTAL_SLIDER_THUMB_POSITION_NUMERATOR: i32 = 2;
pub const HORIZONTAL_SLIDER_THUMB_POSITION_DENOMINATOR: i32 = 3;

pub fn gadget_slider_get_min_max(window: &GameWindow) -> Option<(i32, i32)> {
    match window.widget() {
        Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.range()),
        Some(WindowWidget::VerticalSlider(slider)) => Some(slider.range()),
        _ => None,
    }
}

pub fn gadget_slider_get_thumb(
    window: &GameWindow,
) -> Option<std::rc::Rc<std::cell::RefCell<GameWindow>>> {
    window.children().first().cloned()
}

pub fn gadget_slider_set_position(window: &mut GameWindow, position: i32) -> bool {
    match window.widget_mut() {
        Some(WindowWidget::HorizontalSlider(slider)) => {
            slider.set_value(position);
            true
        }
        Some(WindowWidget::VerticalSlider(slider)) => {
            slider.set_value(position);
            true
        }
        _ => false,
    }
}

pub fn gadget_slider_get_position(window: &GameWindow) -> Option<i32> {
    match window.widget() {
        Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.value()),
        Some(WindowWidget::VerticalSlider(slider)) => Some(slider.value()),
        _ => None,
    }
}

pub fn gadget_slider_set_enabled_image_left(window: &mut GameWindow, image: Image) {
    let _ = window.set_enabled_image(0, image);
}

pub fn gadget_slider_set_enabled_image_right(window: &mut GameWindow, image: Image) {
    let _ = window.set_enabled_image(1, image);
}

pub fn gadget_slider_set_enabled_image_center(window: &mut GameWindow, image: Image) {
    let _ = window.set_enabled_image(2, image);
}

pub fn gadget_slider_set_enabled_image_small_center(window: &mut GameWindow, image: Image) {
    let _ = window.set_enabled_image(3, image);
}

pub fn gadget_slider_set_enabled_color(window: &mut GameWindow, color: Color) {
    let _ = window.set_enabled_color(0, color);
}

pub fn gadget_slider_get_enabled_image_left(window: &GameWindow) -> Option<Image> {
    window.get_enabled_draw_data(0).and_then(|data| data.image)
}

pub fn gadget_slider_get_enabled_image_right(window: &GameWindow) -> Option<Image> {
    window.get_enabled_draw_data(1).and_then(|data| data.image)
}

pub fn gadget_slider_get_enabled_image_center(window: &GameWindow) -> Option<Image> {
    window.get_enabled_draw_data(2).and_then(|data| data.image)
}

pub fn gadget_slider_get_enabled_image_small_center(window: &GameWindow) -> Option<Image> {
    window.get_enabled_draw_data(3).and_then(|data| data.image)
}

pub fn gadget_slider_get_enabled_color(window: &GameWindow) -> Option<Color> {
    window.get_enabled_draw_data(0).map(|data| data.color)
}

// PARITY_NOTE: the canonical Rust UI stores slider visuals in `WindowDrawData` and the concrete
// slider widgets. This facade exposes the C++ helper surface that is currently needed by callers.
// Disabled/hilite per-piece helpers can be added on demand if a direct caller appears.
