use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ControlBarCallback.cpp",
    "crate::gui::callbacks::control_bar_callback",
    "Control Bar Callback",
    "Routes gadget and command-bar messages into gameplay-facing control bar handlers.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Control Bar Callback",
    "Owner callback entry point for command-bar messages.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlBarMessagePort {
    Selected,
    Hovered,
    RightClicked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedControlBarMessagePort {
    pub control_name: String,
    pub message: ControlBarMessagePort,
    pub gameplay_handler: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ControlBarCallbackPort {
    pub takes_focus: bool,
    pub last_routed: Option<RoutedControlBarMessagePort>,
    pub routed_messages: Vec<RoutedControlBarMessagePort>,
}

impl ControlBarCallbackPort {
    pub fn route(
        &mut self,
        control_name: impl Into<String>,
        message: ControlBarMessagePort,
        gameplay_handler: impl Into<String>,
    ) {
        let routed = RoutedControlBarMessagePort {
            control_name: control_name.into(),
            message,
            gameplay_handler: gameplay_handler.into(),
        };
        self.last_routed = Some(routed.clone());
        self.routed_messages.push(routed);
    }

    pub fn handle_input_focus(&self, offered_focus: bool) -> bool {
        offered_focus && self.takes_focus
    }

    pub fn sample() -> Self {
        let mut state = Self {
            takes_focus: true,
            ..Self::default()
        };
        state.route(
            "ButtonStrategyCenter",
            ControlBarMessagePort::Selected,
            "processContextSensitiveButtonClick",
        );
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_updates_last_message() {
        let mut state = ControlBarCallbackPort::default();
        state.route("ButtonDozer", ControlBarMessagePort::Hovered, "tooltip");

        assert_eq!(
            state.last_routed,
            Some(RoutedControlBarMessagePort {
                control_name: "ButtonDozer".to_string(),
                message: ControlBarMessagePort::Hovered,
                gameplay_handler: "tooltip".to_string(),
            })
        );
    }
}
