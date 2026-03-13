use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameWindowGlobal.cpp",
    "crate::gui::game_window_global",
    "Game Window Global",
    "Holds shared window-system globals such as transitions and display-wide state.",
);

#[derive(Clone, Debug)]
pub struct GameWindowGlobalPort {
    pub send_mouse_pos_messages: bool,
    pub transition_handler_loaded: bool,
    pub shell_active: bool,
}

impl Default for GameWindowGlobalPort {
    fn default() -> Self {
        Self {
            send_mouse_pos_messages: true,
            transition_handler_loaded: true,
            shell_active: true,
        }
    }
}
