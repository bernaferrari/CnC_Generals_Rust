use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarScheme.cpp",
    "crate::gui::control_bar::control_bar_scheme",
    "Control Bar Scheme",
    "Ports faction-specific art layers, colors, and animations for the command bar.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Scheme",
    "Faction-specific imagery, colors, and animation layers.",
);
