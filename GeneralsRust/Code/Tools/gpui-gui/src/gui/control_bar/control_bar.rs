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

pub fn demo_buttons() -> Vec<LegacyCommandButton> {
    super::control_bar_command::demo_buttons()
}

pub fn render_command_strip(state: &CommandBarStatePort) -> impl gpui::IntoElement {
    super::control_bar_command::render_command_strip(state)
}
