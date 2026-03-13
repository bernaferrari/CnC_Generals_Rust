use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/MessageBox.cpp",
    "crate::gui::callbacks::message_box",
    "Message Box",
    "Ports basic prompt, yes-no, ok-cancel, and close callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Message Box",
    "Standard prompt and dialog callbacks.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageBoxButtonPort {
    Ok,
    Yes,
    No,
    Cancel,
}

impl MessageBoxButtonPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Yes => "Yes",
            Self::No => "No",
            Self::Cancel => "Cancel",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageBoxStatePort {
    pub title: String,
    pub body: String,
    pub buttons: Vec<MessageBoxButtonPort>,
    pub wants_input_focus: bool,
    pub destroyed: bool,
    pub callback_log: Vec<MessageBoxButtonPort>,
    pub quit_box: bool,
}

impl MessageBoxStatePort {
    pub fn yes_no(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(
            title,
            body,
            vec![MessageBoxButtonPort::Yes, MessageBoxButtonPort::No],
            false,
        )
    }

    pub fn ok_cancel(title: impl Into<String>, body: impl Into<String>, quit_box: bool) -> Self {
        Self::new(
            title,
            body,
            vec![MessageBoxButtonPort::Ok, MessageBoxButtonPort::Cancel],
            quit_box,
        )
    }

    pub fn handle_input_focus(&self, offered_focus: bool) -> bool {
        offered_focus && self.wants_input_focus
    }

    pub fn select(&mut self, button: MessageBoxButtonPort) -> bool {
        if !self.buttons.contains(&button) {
            return false;
        }
        self.callback_log.push(button);
        self.destroyed = true;
        true
    }

    fn new(
        title: impl Into<String>,
        body: impl Into<String>,
        buttons: Vec<MessageBoxButtonPort>,
        quit_box: bool,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            buttons,
            wants_input_focus: true,
            destroyed: false,
            callback_log: Vec::new(),
            quit_box,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_valid_button_destroys_message_box() {
        let mut message_box =
            MessageBoxStatePort::yes_no("Overwrite Save", "Do you want to overwrite this save?");

        assert!(message_box.select(MessageBoxButtonPort::Yes));
        assert!(message_box.destroyed);
        assert_eq!(message_box.callback_log, vec![MessageBoxButtonPort::Yes]);
    }

    #[test]
    fn input_focus_is_only_taken_when_offered() {
        let message_box = MessageBoxStatePort::ok_cancel("Quit?", "Leave the game?", true);

        assert!(message_box.handle_input_focus(true));
        assert!(!message_box.handle_input_focus(false));
    }
}
