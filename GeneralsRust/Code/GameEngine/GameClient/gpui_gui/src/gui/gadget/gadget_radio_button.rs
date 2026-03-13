use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetRadioButton.cpp",
    "crate::gui::gadget::gadget_radio_button",
    "Gadget Radio Button",
    "Ports mutually-exclusive grouped selection behavior for radio gadgets.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Radio Button",
    "Exclusive option selector.",
    "Single active option per group.",
    GadgetKind::RadioButton,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioButtonAction {
    MouseEntering,
    MouseLeaving,
    Drag,
    Selected,
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioButtonState {
    pub label: String,
    pub group: i32,
    pub screen: i32,
    pub selected: bool,
    pub hilited: bool,
    pub mouse_track: bool,
}

impl RadioButtonState {
    pub fn new(label: impl Into<String>, group: i32, screen: i32, selected: bool) -> Self {
        Self {
            label: label.into(),
            group,
            screen,
            selected,
            hilited: false,
            mouse_track: true,
        }
    }

    pub fn mouse_entering(&mut self) -> RadioButtonAction {
        if self.mouse_track {
            self.hilited = true;
            RadioButtonAction::MouseEntering
        } else {
            RadioButtonAction::Ignored
        }
    }

    pub fn mouse_leaving(&mut self) -> RadioButtonAction {
        if self.mouse_track {
            self.hilited = false;
            RadioButtonAction::MouseLeaving
        } else {
            RadioButtonAction::Ignored
        }
    }

    pub fn left_up(&mut self) -> RadioButtonAction {
        if self.selected && !self.hilited {
            return RadioButtonAction::Ignored;
        }

        if !self.selected {
            self.selected = true;
            RadioButtonAction::Selected
        } else {
            RadioButtonAction::Ignored
        }
    }

    pub fn key_press(&mut self, key: &str, down: bool) -> RadioButtonAction {
        if !down {
            return RadioButtonAction::Ignored;
        }
        match key {
            "Enter" | "Space" if !self.selected => {
                self.selected = true;
                RadioButtonAction::Selected
            }
            "Down" | "Right" | "Tab" => RadioButtonAction::NextTab,
            "Up" | "Left" => RadioButtonAction::PrevTab,
            _ => RadioButtonAction::Ignored,
        }
    }
}

pub fn select_exclusive(buttons: &mut [RadioButtonState], index: usize) -> bool {
    let Some((group, screen)) = buttons.get(index).map(|b| (b.group, b.screen)) else {
        return false;
    };

    for (i, button) in buttons.iter_mut().enumerate() {
        if i == index {
            button.selected = true;
        } else if button.group == group && button.screen == screen {
            button.selected = false;
        }
    }
    true
}

pub fn render_demo(options: &[&str], selected: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(options.iter().map(|label| {
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .size(px(16.))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x8dc0ff))
                        .bg(if *label == selected {
                            rgb(0x32567c)
                        } else {
                            rgb(0x101720)
                        }),
                )
                .child((*label).to_string())
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_selection_clears_other_buttons_in_group() {
        let mut buttons = vec![
            RadioButtonState::new("A", 1, 1, true),
            RadioButtonState::new("B", 1, 1, false),
            RadioButtonState::new("C", 2, 1, true),
        ];
        assert!(select_exclusive(&mut buttons, 1));
        assert!(!buttons[0].selected);
        assert!(buttons[1].selected);
        assert!(buttons[2].selected);
    }
}
