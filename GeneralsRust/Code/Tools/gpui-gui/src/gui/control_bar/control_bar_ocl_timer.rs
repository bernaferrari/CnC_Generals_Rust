use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarOCLTimer.cpp",
    "crate::gui::control_bar::control_bar_ocl_timer",
    "Control Bar OCL Timer",
    "Ports OCL countdown and timer-driven progress presentation.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "OCL Timer",
    "Countdown and timed progress elements.",
);
