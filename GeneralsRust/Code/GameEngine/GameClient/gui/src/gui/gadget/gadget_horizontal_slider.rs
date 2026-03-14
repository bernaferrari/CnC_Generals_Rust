use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetHorizontalSlider.cpp",
    "crate::gui::gadget::gadget_horizontal_slider",
    "Gadget Horizontal Slider",
    "Ports horizontal slider track, thumb hover, and drag-to-value behavior.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Horizontal Slider",
    "Continuous value slider laid out horizontally.",
    "Drag thumb along a horizontal rail.",
    GadgetKind::HorizontalSlider,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HorizontalSliderAction {
    MouseEntering,
    MouseLeaving,
    Drag,
    Track,
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HorizontalSliderState {
    pub min: i32,
    pub max: i32,
    pub position: i32,
    pub track_width: i32,
    pub thumb_width: i32,
    pub hilited: bool,
    pub mouse_track: bool,
}

impl Default for HorizontalSliderState {
    fn default() -> Self {
        Self {
            min: 0,
            max: 100,
            position: 50,
            track_width: 144,
            thumb_width: 16,
            hilited: false,
            mouse_track: true,
        }
    }
}

impl HorizontalSliderState {
    pub fn normalized(&self) -> f32 {
        let range = (self.max - self.min).max(1) as f32;
        (self.position - self.min) as f32 / range
    }

    pub fn mouse_entering(&mut self) -> HorizontalSliderAction {
        if self.mouse_track {
            self.hilited = true;
            HorizontalSliderAction::MouseEntering
        } else {
            HorizontalSliderAction::Ignored
        }
    }

    pub fn mouse_leaving(&mut self) -> HorizontalSliderAction {
        if self.mouse_track {
            self.hilited = false;
            HorizontalSliderAction::MouseLeaving
        } else {
            HorizontalSliderAction::Ignored
        }
    }

    pub fn page_click(&mut self, click_x: i32) -> HorizontalSliderAction {
        let thumb_center = (self.normalized() * self.track_width as f32) as i32;
        let mut next = if click_x >= thumb_center {
            thumb_center + self.track_width / 5
        } else {
            thumb_center - self.track_width / 5
        };
        next = next.clamp(0, self.track_width - self.thumb_width / 2);
        self.position = self.min
            + ((next as f32 / self.track_width.max(1) as f32) * (self.max - self.min) as f32)
                .round() as i32;
        self.position = self.position.clamp(self.min, self.max);
        HorizontalSliderAction::Track
    }

    pub fn key_press(&mut self, key: &str, down: bool) -> HorizontalSliderAction {
        if !down {
            return HorizontalSliderAction::Ignored;
        }

        match key {
            "Right" if self.position > self.min + 1 => {
                self.position = (self.position - 2).clamp(self.min, self.max);
                HorizontalSliderAction::Track
            }
            "Left" if self.position < self.max - 1 => {
                self.position = (self.position + 2).clamp(self.min, self.max);
                HorizontalSliderAction::Track
            }
            "Down" | "Tab" => HorizontalSliderAction::NextTab,
            "Up" => HorizontalSliderAction::PrevTab,
            _ => HorizontalSliderAction::Ignored,
        }
    }
}

pub fn render_demo(label: &str, value: f32) -> AnyElement {
    let mut state = HorizontalSliderState::default();
    state.position =
        (state.min as f32 + (state.max - state.min) as f32 * value.clamp(0.0, 1.0)).round() as i32;
    render(label, &state)
}

pub fn render(label: impl Into<String>, state: &HorizontalSliderState) -> AnyElement {
    let label = label.into();
    let track_width = state.track_width.max(1) as f32;
    let width = track_width * state.normalized();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .w(px(track_width))
                .h(px(8.))
                .rounded_full()
                .bg(rgb(0x1f2a35))
                .child(
                    div()
                        .w(px(width))
                        .h(px(8.))
                        .rounded_full()
                        .bg(if state.hilited {
                            rgb(0x8dc0ff)
                        } else {
                            rgb(0x69d18a)
                        }),
                ),
        )
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(label))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_step_matches_legacy_direction() {
        let mut state = HorizontalSliderState::default();
        let start = state.position;
        assert_eq!(
            state.key_press("Right", true),
            HorizontalSliderAction::Track
        );
        assert_eq!(state.position, start - 2);
    }

    #[test]
    fn page_click_updates_slider_position() {
        let mut state = HorizontalSliderState::default();
        assert_eq!(state.page_click(140), HorizontalSliderAction::Track);
        assert!(state.position >= state.min);
    }
}
