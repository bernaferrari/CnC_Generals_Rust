use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarBeacon.cpp",
    "crate::gui::control_bar::control_bar_beacon",
    "Control Bar Beacon",
    "Ports beacon placement, deletion, and beacon-specific command presentation.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Beacon Controls",
    "Beacon-specific buttons and targeting flow.",
);
