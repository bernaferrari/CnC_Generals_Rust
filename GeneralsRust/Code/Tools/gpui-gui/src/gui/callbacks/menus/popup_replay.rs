use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupReplay.cpp",
    "crate::gui::callbacks::menus::popup_replay",
    "Popup Replay",
    "Replay-save popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupReplay",
    "Replay Popup",
    "Replay save and naming popup.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupReplayPort {
    pub replay_name: String,
    pub description: String,
    pub overwrite_existing: bool,
    pub can_save: bool,
}

impl Default for PopupReplayPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupReplayPort {
    pub fn sample() -> Self {
        Self {
            replay_name: "usa-vs-gla-final".to_string(),
            description: "Tournament Desert ladder replay".to_string(),
            overwrite_existing: false,
            can_save: true,
        }
    }
}
