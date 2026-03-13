use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetStaticText.cpp",
    "crate::gui::gadget::gadget_static_text",
    "Gadget Static Text",
    "Ports multi-line or single-line text labels with layout-aware wrapping.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Static Text",
    "Read-only text label gadget.",
    "Wrap, align, and tint text content.",
    GadgetKind::StaticText,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticTextAction {
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticTextState {
    pub label: String,
    pub body: String,
    pub font_name: Option<String>,
}

impl StaticTextState {
    pub fn new(label: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            body: body.into(),
            font_name: None,
        }
    }

    pub fn set_text(&mut self, body: impl Into<String>) {
        self.body = body.into();
    }

    pub fn get_text(&self) -> &str {
        &self.body
    }

    pub fn set_font(&mut self, font_name: impl Into<String>) {
        self.font_name = Some(font_name.into());
    }

    pub fn key_press(&self, key: &str, down: bool) -> StaticTextAction {
        if !down {
            return StaticTextAction::Ignored;
        }
        match key {
            "Down" | "Right" | "Tab" => StaticTextAction::NextTab,
            "Up" | "Left" => StaticTextAction::PrevTab,
            _ => StaticTextAction::Ignored,
        }
    }
}

pub fn render_demo(label: &str, body: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(label.to_string())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(body.to_string()),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_text_replaces_body() {
        let mut state = StaticTextState::new("Label", "Body");
        state.set_text("Next");
        assert_eq!(state.get_text(), "Next");
    }
}
