use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarCommandProcessing.cpp",
    "crate::gui::control_bar::control_bar_command_processing",
    "Control Bar Command Processing",
    "Ports command dispatch, contextual targeting, and queueability checks.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Command Processing",
    "Context-sensitive command dispatch and target gating.",
);
