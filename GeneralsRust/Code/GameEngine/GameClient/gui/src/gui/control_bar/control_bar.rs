use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};
use crate::model::LegacyCommandButton;

pub use super::control_bar_command::CommandBarStatePort;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBar.cpp",
    "crate::gui::control_bar::control_bar",
    "Control Bar",
    "Ports the top-level context-sensitive command interface and its command grid.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Control Bar Core",
    "Owns command-set population, UI dirtying, and shell/HUD coordination.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarCorePort {
    pub current_context: String,
    pub command_set_dirty: bool,
    pub shell_hidden: bool,
    pub radar_enabled: bool,
    pub selected_group_size: u8,
}

impl Default for ControlBarCorePort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarCorePort {
    pub fn sample() -> Self {
        Self {
            current_context: "USA Command Center".to_string(),
            command_set_dirty: true,
            shell_hidden: false,
            radar_enabled: true,
            selected_group_size: 3,
        }
    }
}

pub fn demo_buttons() -> Vec<LegacyCommandButton> {
    super::control_bar_command::demo_buttons()
}

pub fn render_command_strip(state: &CommandBarStatePort) -> impl gpui::IntoElement {
    super::control_bar_command::render_command_strip(state)
}
