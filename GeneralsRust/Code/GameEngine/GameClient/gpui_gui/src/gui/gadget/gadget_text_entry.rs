use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetTextEntry.cpp",
    "crate::gui::gadget::gadget_text_entry",
    "Gadget Text Entry",
    "Ports editable text fields, secret text, numeric filtering, and IME-aware input.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Text Entry",
    "Editable single-line text field.",
    "Keyboard focus, selection, and filtered input.",
    GadgetKind::TextEntry,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEntryAction {
    UpdateText,
    EditDone,
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEntryState {
    pub text: String,
    pub secret_text: String,
    pub max_text_len: usize,
    pub numerical_only: bool,
    pub alpha_numerical_only: bool,
    pub ascii_only: bool,
    pub is_secret: bool,
    pub ime_composing: bool,
}

impl Default for TextEntryState {
    fn default() -> Self {
        Self {
            text: String::new(),
            secret_text: String::new(),
            max_text_len: 64,
            numerical_only: false,
            alpha_numerical_only: false,
            ascii_only: false,
            is_secret: false,
            ime_composing: false,
        }
    }
}

impl TextEntryState {
    pub fn input_char(&mut self, ch: char) -> TextEntryAction {
        if self.ime_composing {
            return TextEntryAction::Ignored;
        }
        if ch == '\r' || ch == '\n' {
            return TextEntryAction::EditDone;
        }
        if self.numerical_only && !ch.is_ascii_digit() {
            return TextEntryAction::Ignored;
        }
        if self.alpha_numerical_only && !ch.is_ascii_alphanumeric() {
            return TextEntryAction::Ignored;
        }
        if self.ascii_only && !ch.is_ascii() {
            return TextEntryAction::Ignored;
        }
        if self.text.chars().count() >= self.max_text_len.saturating_sub(1) {
            return TextEntryAction::Ignored;
        }

        self.text.push(ch);
        self.secret_text.push('*');
        TextEntryAction::UpdateText
    }

    pub fn key_press(&mut self, key: &str, down: bool, alt_or_ctrl: bool) -> TextEntryAction {
        if !down {
            return TextEntryAction::Ignored;
        }
        if alt_or_ctrl {
            return TextEntryAction::Ignored;
        }

        match key {
            "Esc" | "PgUp" | "PgDn" | "Home" | "End" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6"
            | "F7" | "F8" | "F9" | "F10" | "F11" | "F12" | "Caps" | "Del" => {
                TextEntryAction::Ignored
            }
            "Down" | "Right" | "Tab" => TextEntryAction::NextTab,
            "Up" | "Left" => TextEntryAction::PrevTab,
            "Backspace" => {
                if self.text.pop().is_some() {
                    self.secret_text.pop();
                    TextEntryAction::UpdateText
                } else {
                    TextEntryAction::Ignored
                }
            }
            _ => TextEntryAction::Ignored,
        }
    }

    pub fn visible_text(&self) -> &str {
        if self.is_secret {
            &self.secret_text
        } else {
            &self.text
        }
    }
}

pub fn render_demo(value: &str) -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x111922))
        .child(value.to_string())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_non_numeric_input() {
        let mut state = TextEntryState {
            numerical_only: true,
            ..Default::default()
        };
        assert_eq!(state.input_char('a'), TextEntryAction::Ignored);
        assert_eq!(state.input_char('7'), TextEntryAction::UpdateText);
        assert_eq!(state.text, "7");
    }

    #[test]
    fn backspace_removes_character() {
        let mut state = TextEntryState::default();
        state.input_char('x');
        assert_eq!(
            state.key_press("Backspace", true, false),
            TextEntryAction::UpdateText
        );
        assert!(state.text.is_empty());
    }
}
