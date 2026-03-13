use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupLadderSelect.cpp",
    "crate::gui::callbacks::menus::popup_ladder_select",
    "Popup Ladder Select",
    "Ladder-select popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupLadderSelect",
    "Ladder Select",
    "Popup ladder selection dialog.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LadderEntryPort {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupLadderSelectPort {
    pub entries: Vec<LadderEntryPort>,
    pub selected_index: usize,
}

impl Default for PopupLadderSelectPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupLadderSelectPort {
    pub fn current(&self) -> Option<&LadderEntryPort> {
        self.entries.get(self.selected_index)
    }

    pub fn sample() -> Self {
        Self {
            entries: vec![
                LadderEntryPort {
                    name: "Ranked 1v1".to_string(),
                    description: "Official ladder, stats tracked, no mods.".to_string(),
                },
                LadderEntryPort {
                    name: "Clan Ladder".to_string(),
                    description: "Team-based clan reporting.".to_string(),
                },
            ],
            selected_index: 0,
        }
    }
}
