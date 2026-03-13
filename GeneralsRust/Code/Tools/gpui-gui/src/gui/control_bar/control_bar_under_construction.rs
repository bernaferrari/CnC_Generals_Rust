use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarUnderConstruction.cpp",
    "crate::gui::control_bar::control_bar_under_construction",
    "Control Bar Under Construction",
    "Ports building-under-construction progress and option locking.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Under Construction",
    "Construction progress and locked command presentation.",
);
