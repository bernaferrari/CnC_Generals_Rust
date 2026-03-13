use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarResizer.cpp",
    "crate::gui::control_bar::control_bar_resizer",
    "Control Bar Resizer",
    "Ports resolution-aware control bar anchoring and resize behavior.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Resizer",
    "Resolution-aware anchoring and resizing rules.",
);
