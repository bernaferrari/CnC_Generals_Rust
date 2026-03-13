use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameWindowManagerScript.cpp",
    "crate::gui::game_window_manager_script",
    "Game Window Manager Script",
    "Ports .wnd script loading and layout callback resolution into the GPUI toolchain.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowScriptPort {
    pub filename: String,
    pub root_window_count: usize,
    pub init_callback: Option<String>,
    pub update_callback: Option<String>,
    pub shutdown_callback: Option<String>,
}

impl WindowScriptPort {
    pub fn from_layout(filename: impl Into<String>, root_window_count: usize) -> Self {
        Self {
            filename: filename.into(),
            root_window_count,
            init_callback: Some("Init".to_string()),
            update_callback: Some("Update".to_string()),
            shutdown_callback: Some("Shutdown".to_string()),
        }
    }
}
