use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarCommand.cpp",
    "crate::gui::control_bar::control_bar_command",
    "Control Bar Command",
    "Ports command-button metadata, labels, images, and cursor mappings.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Command Buttons",
    "Command metadata, labels, images, and border types.",
);
