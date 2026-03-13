use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetProgressBar.cpp",
    "crate::gui::gadget::gadget_progress_bar",
    "Gadget Progress Bar",
    "Ports fill-percentage rendering for build, load, and transfer progress.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Progress Bar",
    "Non-interactive progress indicator.",
    "Reflects percentage and completion state.",
    GadgetKind::ProgressBar,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressBarState {
    pub progress: u8,
}

impl ProgressBarState {
    pub fn new(progress: u8) -> Self {
        Self { progress }
    }

    pub fn set_progress(&mut self, progress: i32) -> bool {
        if !(0..=100).contains(&progress) {
            return false;
        }
        self.progress = progress as u8;
        true
    }

    pub fn normalized(&self) -> f32 {
        self.progress as f32 / 100.0
    }
}

pub fn render_demo(label: &str, progress: f32) -> AnyElement {
    let state = ProgressBarState::new((progress.clamp(0.0, 1.0) * 100.0).round() as u8);
    let width = 144.0_f32 * state.normalized();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div().h(px(10.)).rounded_full().bg(rgb(0x1e2935)).child(
                div()
                    .w(px(width))
                    .h(px(10.))
                    .rounded_full()
                    .bg(rgb(0xd88a44)),
            ),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(label.to_string()),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_progress_is_ignored() {
        let mut state = ProgressBarState::new(40);
        assert!(!state.set_progress(101));
        assert_eq!(state.progress, 40);
        assert!(state.set_progress(100));
        assert_eq!(state.progress, 100);
    }
}
