use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupSaveLoad.cpp",
    "crate::gui::callbacks::menus::popup_save_load",
    "Popup Save Load",
    "Save/load popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SaveLoadMenu",
    "Save / Load",
    "Popup save-load flow and slot management.",
    "Popup",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveLoadLayoutTypePort {
    SaveAndLoad,
    LoadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveLoadModalPort {
    None,
    OverwriteConfirm,
    LoadConfirm,
    SaveDescription,
    DeleteConfirm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveGameEntryPort {
    pub filename: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupSaveLoadPort {
    pub layout_type: SaveLoadLayoutTypePort,
    pub is_popup: bool,
    pub selected_index: Option<usize>,
    pub entries: Vec<SaveGameEntryPort>,
    pub active_modal: SaveLoadModalPort,
    pub pending_action: Option<String>,
}

impl Default for PopupSaveLoadPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupSaveLoadPort {
    pub fn select_entry(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.selected_index = Some(index);
        true
    }

    pub fn can_load(&self) -> bool {
        self.selected_index.is_some()
    }

    pub fn can_save(&self) -> bool {
        self.layout_type != SaveLoadLayoutTypePort::LoadOnly
    }

    pub fn request_save(&mut self) -> bool {
        if !self.can_save() {
            return false;
        }
        self.active_modal = SaveLoadModalPort::SaveDescription;
        true
    }

    pub fn request_load(&mut self) -> bool {
        if !self.can_load() {
            return false;
        }
        self.active_modal = SaveLoadModalPort::LoadConfirm;
        true
    }

    pub fn request_delete(&mut self) -> bool {
        if !self.can_load() {
            return false;
        }
        self.active_modal = SaveLoadModalPort::DeleteConfirm;
        true
    }

    pub fn confirm(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        let entry = &self.entries[index];
        self.pending_action = Some(match self.active_modal {
            SaveLoadModalPort::LoadConfirm => format!("load:{}", entry.filename),
            SaveLoadModalPort::DeleteConfirm => format!("delete:{}", entry.filename),
            SaveLoadModalPort::SaveDescription => format!("save:{}", entry.filename),
            SaveLoadModalPort::OverwriteConfirm => format!("overwrite:{}", entry.filename),
            SaveLoadModalPort::None => return false,
        });
        self.active_modal = SaveLoadModalPort::None;
        true
    }

    pub fn sample() -> Self {
        Self {
            layout_type: SaveLoadLayoutTypePort::SaveAndLoad,
            is_popup: true,
            selected_index: Some(0),
            active_modal: SaveLoadModalPort::None,
            pending_action: None,
            entries: vec![
                SaveGameEntryPort {
                    filename: "mission01.sav".to_string(),
                    description: "Black Gold briefing".to_string(),
                },
                SaveGameEntryPort {
                    filename: "mission02.sav".to_string(),
                    description: "A Fallback Position".to_string(),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_confirm_generates_pending_action() {
        let mut state = PopupSaveLoadPort::sample();
        assert!(state.request_load());
        assert!(state.confirm());
        assert_eq!(state.pending_action.as_deref(), Some("load:mission01.sav"));
    }

    #[test]
    fn load_only_layout_disables_save() {
        let mut state = PopupSaveLoadPort::sample();
        state.layout_type = SaveLoadLayoutTypePort::LoadOnly;

        assert!(!state.request_save());
        assert_eq!(state.active_modal, SaveLoadModalPort::None);
    }
}
