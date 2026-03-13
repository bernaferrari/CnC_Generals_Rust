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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarAnchorPort {
    pub label: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarPrintPositionsPort {
    pub anchors: Vec<ControlBarAnchorPort>,
}

impl Default for ControlBarPrintPositionsPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarPrintPositionsPort {
    pub fn sample() -> Self {
        Self {
            anchors: vec![
                ControlBarAnchorPort {
                    label: "ButtonGrid".to_string(),
                    x: 64,
                    y: 768,
                },
                ControlBarAnchorPort {
                    label: "Radar".to_string(),
                    x: 1048,
                    y: 706,
                },
                ControlBarAnchorPort {
                    label: "Money".to_string(),
                    x: 148,
                    y: 704,
                },
            ],
        }
    }
}
