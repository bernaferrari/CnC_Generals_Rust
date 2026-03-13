use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarPrintPositions.cpp",
    "crate::gui::control_bar::control_bar_print_positions",
    "Control Bar Print Positions",
    "Ports debug-position dumping and layout inspection helpers for the control bar.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Position Debug",
    "Debug printing for button and HUD anchor positions.",
);
