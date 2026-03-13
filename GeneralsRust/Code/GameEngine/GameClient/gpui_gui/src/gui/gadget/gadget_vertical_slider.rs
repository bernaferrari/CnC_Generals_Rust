use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetVerticalSlider.cpp",
    "crate::gui::gadget::gadget_vertical_slider",
    "Gadget Vertical Slider",
    "Ports vertical slider track, thumb state, and scroll-style interaction.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Vertical Slider",
    "Continuous value slider laid out vertically.",
    "Drag thumb along a vertical rail.",
    GadgetKind::VerticalSlider,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalSliderAction {
    MouseEntering,
    MouseLeaving,
    Drag,
    Track,
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VerticalSliderState {
    pub min: i32,
    pub max: i32,
    pub position: i32,
    pub track_height: i32,
    pub thumb_height: i32,
    pub hilited: bool,
    pub mouse_track: bool,
}

impl Default for VerticalSliderState {
    fn default() -> Self {
        Self {
            min: 0,
            max: 100,
            position: 50,
            track_height: 96,
            thumb_height: 16,
            hilited: false,
            mouse_track: true,
        }
    }
}

impl VerticalSliderState {
    pub fn normalized(&self) -> f32 {
        let range = (self.max - self.min).max(1) as f32;
        (self.position - self.min) as f32 / range
    }

    pub fn page_click(&mut self, click_y: i32) -> VerticalSliderAction {
        let thumb_center = ((1.0 - self.normalized()) * self.track_height as f32) as i32;
        let mut next = if click_y >= thumb_center {
            thumb_center + self.track_height / 5
        } else {
            thumb_center - self.track_height / 5
        };
        next = next.clamp(0, self.track_height - self.thumb_height / 2);
        let normalized = 1.0 - (next as f32 / self.track_height.max(1) as f32);
        self.position = self.min + (normalized * (self.max - self.min) as f32).round() as i32;
        self.position = self.position.clamp(self.min, self.max);
        VerticalSliderAction::Track
    }

    pub fn key_press(&mut self, key: &str, down: bool) -> VerticalSliderAction {
        if !down {
            return VerticalSliderAction::Ignored;
        }

        match key {
            "Up" if self.position < self.max - 1 => {
                self.position = (self.position + 2).clamp(self.min, self.max);
                VerticalSliderAction::Track
            }
            "Down" if self.position > self.min + 1 => {
                self.position = (self.position - 2).clamp(self.min, self.max);
                VerticalSliderAction::Track
            }
            "Right" | "Tab" => VerticalSliderAction::NextTab,
            "Left" => VerticalSliderAction::PrevTab,
            _ => VerticalSliderAction::Ignored,
        }
    }
}

pub fn render_demo(value: f32) -> AnyElement {
    let mut state = VerticalSliderState::default();
    state.position =
        (state.min as f32 + (state.max - state.min) as f32 * value.clamp(0.0, 1.0)).round() as i32;
    let height = 96.0_f32 * state.normalized();
    div()
        .flex()
        .items_end()
        .h(px(96.))
        .w(px(16.))
        .rounded_full()
        .bg(rgb(0x1f2a35))
        .child(
            div()
                .w(px(16.))
                .h(px(height))
                .rounded_full()
                .bg(rgb(0x8dc0ff)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_key_increases_position() {
        let mut state = VerticalSliderState::default();
        let start = state.position;
        assert_eq!(state.key_press("Up", true), VerticalSliderAction::Track);
        assert_eq!(state.position, start + 2);
    }
}
