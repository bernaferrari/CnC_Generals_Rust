use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetPushButton.cpp",
    "crate::gui::gadget::gadget_push_button",
    "Gadget Push Button",
    "Ports mouse enter, mouse leave, press, release, and owner message routing for push buttons.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Push Button",
    "Primary command and menu activation button.",
    "Hover, press, release, optional mouse-down trigger.",
    GadgetKind::PushButton,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PushButtonAction {
    MouseEntering,
    MouseLeaving,
    Drag,
    Selected,
    SelectedRight,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushButtonState {
    pub selected: bool,
    pub hilited: bool,
    pub mouse_track: bool,
    pub check_like: bool,
    pub trigger_on_mouse_down: bool,
    pub right_click_enabled: bool,
}

impl Default for PushButtonState {
    fn default() -> Self {
        Self {
            selected: false,
            hilited: false,
            mouse_track: true,
            check_like: false,
            trigger_on_mouse_down: false,
            right_click_enabled: false,
        }
    }
}

impl PushButtonState {
    pub fn mouse_entering(&mut self) -> PushButtonAction {
        if self.mouse_track {
            self.hilited = true;
            PushButtonAction::MouseEntering
        } else {
            PushButtonAction::Ignored
        }
    }

    pub fn mouse_leaving(&mut self) -> PushButtonAction {
        if self.mouse_track {
            self.hilited = false;
        }
        if !self.check_like {
            self.selected = false;
        }
        PushButtonAction::MouseLeaving
    }

    pub fn left_drag(&self) -> PushButtonAction {
        PushButtonAction::Drag
    }

    pub fn left_down(&mut self) -> PushButtonAction {
        if self.check_like {
            self.selected = !self.selected;
        } else {
            self.selected = true;
        }

        if self.trigger_on_mouse_down || self.check_like {
            PushButtonAction::Selected
        } else {
            PushButtonAction::Ignored
        }
    }

    pub fn left_up(&mut self) -> PushButtonAction {
        if !self.selected || self.check_like {
            return PushButtonAction::Ignored;
        }

        self.selected = false;
        if self.trigger_on_mouse_down {
            PushButtonAction::Ignored
        } else {
            PushButtonAction::Selected
        }
    }

    pub fn right_down(&mut self) -> PushButtonAction {
        if !self.right_click_enabled {
            return PushButtonAction::Ignored;
        }

        if self.check_like {
            self.selected = !self.selected;
            PushButtonAction::SelectedRight
        } else {
            self.selected = true;
            PushButtonAction::Ignored
        }
    }

    pub fn right_up(&mut self) -> PushButtonAction {
        if !self.right_click_enabled || !self.selected {
            return PushButtonAction::Ignored;
        }

        if !self.check_like {
            self.selected = false;
        }
        PushButtonAction::SelectedRight
    }
}

pub fn render_demo(label: &str) -> AnyElement {
    render(label, &PushButtonState::default())
}

pub fn render(label: impl Into<String>, state: &PushButtonState) -> AnyElement {
    let label = label.into();
    div()
        .px_4()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(if state.selected || state.hilited {
            rgb(0xd1a65d)
        } else {
            rgb(0x35506b)
        })
        .bg(if state.selected {
            rgb(0x2a2011)
        } else if state.hilited {
            rgb(0x182231)
        } else {
            rgb(0x101720)
        })
        .child(label)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_button_triggers_on_mouse_up() {
        let mut state = PushButtonState::default();
        assert_eq!(state.left_down(), PushButtonAction::Ignored);
        assert!(state.selected);
        assert_eq!(state.left_up(), PushButtonAction::Selected);
        assert!(!state.selected);
    }

    #[test]
    fn check_like_button_triggers_on_mouse_down() {
        let mut state = PushButtonState {
            check_like: true,
            ..Default::default()
        };
        assert_eq!(state.left_down(), PushButtonAction::Selected);
        assert!(state.selected);
    }
}
