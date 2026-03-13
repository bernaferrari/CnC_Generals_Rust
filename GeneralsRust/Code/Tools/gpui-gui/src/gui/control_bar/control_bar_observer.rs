use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarObserver.cpp",
    "crate::gui::control_bar::control_bar_observer",
    "Control Bar Observer",
    "Ports observer-mode overlays and passive HUD presentation.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Observer Mode",
    "Observer-specific HUD composition and restrictions.",
);
