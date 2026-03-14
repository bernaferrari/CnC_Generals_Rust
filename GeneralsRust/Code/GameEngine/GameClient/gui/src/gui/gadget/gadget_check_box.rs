use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetCheckBox.cpp",
    "crate::gui::gadget::gadget_check_box",
    "Gadget Check Box",
    "Ports dual-state check-like button behavior and owner notifications.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Check Box",
    "Boolean toggle gadget.",
    "Immediate toggle on press.",
    GadgetKind::CheckBox,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckBoxAction {
    MouseEntering,
    MouseLeaving,
    Drag,
    Selected,
    SelectedRight,
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckBoxState {
    pub checked: bool,
    pub hilited: bool,
    pub mouse_track: bool,
    pub right_click_enabled: bool,
}

impl Default for CheckBoxState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl CheckBoxState {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            hilited: false,
            mouse_track: true,
            right_click_enabled: true,
        }
    }

    pub fn mouse_entering(&mut self) -> CheckBoxAction {
        if self.mouse_track {
            self.hilited = true;
            CheckBoxAction::MouseEntering
        } else {
            CheckBoxAction::Ignored
        }
    }

    pub fn mouse_leaving(&mut self) -> CheckBoxAction {
        if self.mouse_track {
            self.hilited = false;
            CheckBoxAction::MouseLeaving
        } else {
            CheckBoxAction::Ignored
        }
    }

    pub fn left_drag(&self) -> CheckBoxAction {
        CheckBoxAction::Drag
    }

    pub fn left_up(&mut self) -> CheckBoxAction {
        if !self.hilited {
            return CheckBoxAction::Ignored;
        }

        self.checked = !self.checked;
        CheckBoxAction::Selected
    }

    pub fn right_up(&mut self) -> CheckBoxAction {
        if !self.right_click_enabled || !self.checked {
            return CheckBoxAction::Ignored;
        }

        self.checked = false;
        CheckBoxAction::SelectedRight
    }

    pub fn key_press(&mut self, key: &str, down: bool) -> CheckBoxAction {
        if !down {
            return CheckBoxAction::Ignored;
        }

        match key {
            "Enter" | "Space" => {
                self.checked = !self.checked;
                CheckBoxAction::Selected
            }
            "Down" | "Right" | "Tab" => CheckBoxAction::NextTab,
            "Up" | "Left" => CheckBoxAction::PrevTab,
            _ => CheckBoxAction::Ignored,
        }
    }
}

pub fn render_demo(label: &str, checked: bool) -> AnyElement {
    render(label, &CheckBoxState::new(checked))
}

pub fn render(label: impl Into<String>, state: &CheckBoxState) -> AnyElement {
    let label = label.into();
    div()
        .flex()
        .gap_2()
        .items_center()
        .child(
            div()
                .size(px(18.))
                .rounded_sm()
                .border_1()
                .border_color(if state.hilited {
                    rgb(0xd1a65d)
                } else {
                    rgb(0x8dc0ff)
                })
                .bg(if state.checked {
                    rgb(0x23412e)
                } else {
                    rgb(0x162331)
                })
                .child(if state.checked { "X" } else { "" }),
        )
        .child(
            div()
                .text_color(if state.hilited {
                    rgb(0xd6b179)
                } else {
                    rgb(0xc3d1de)
                })
                .child(label),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_up_only_toggles_when_hilited() {
        let mut state = CheckBoxState::new(false);
        assert_eq!(state.left_up(), CheckBoxAction::Ignored);
        state.mouse_entering();
        assert_eq!(state.left_up(), CheckBoxAction::Selected);
        assert!(state.checked);
    }

    #[test]
    fn right_click_clears_checked_state() {
        let mut state = CheckBoxState::new(true);
        assert_eq!(state.right_up(), CheckBoxAction::SelectedRight);
        assert!(!state.checked);
    }
}
