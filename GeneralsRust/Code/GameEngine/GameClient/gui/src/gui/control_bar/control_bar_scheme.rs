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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarSchemePort {
    pub side: String,
    pub right_hud_image: String,
    pub command_bar_border_color: String,
    pub build_border_color: String,
    pub action_border_color: String,
    pub beacon_button_image: String,
}

impl Default for ControlBarSchemePort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarSchemePort {
    pub fn sample() -> Self {
        Self {
            side: "USA".to_string(),
            right_hud_image: "USA_RightHUD".to_string(),
            command_bar_border_color: "#466b94".to_string(),
            build_border_color: "#7da6d1".to_string(),
            action_border_color: "#d1a65d".to_string(),
            beacon_button_image: "BeaconButtonEnable".to_string(),
        }
    }
}
