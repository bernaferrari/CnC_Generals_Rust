use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarMultiSelect.cpp",
    "crate::gui::control_bar::control_bar_multi_select",
    "Control Bar Multi Select",
    "Ports merged command-set presentation for multi-selection contexts.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Multi Select",
    "Merged command grid for multiple selected units.",
);
