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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveLoadButtonId {
    Back,
    Save,
    Load,
    Delete,
    OverwriteCancel,
    OverwriteConfirm,
    LoadCancel,
    LoadConfirm,
    SaveDescCancel,
    SaveDescConfirm,
    DeleteConfirm,
    DeleteCancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveFileTypePort {
    Normal,
    Mission,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveLoadCommand {
    CloseMenu,
    ShellPop,
    PopulateList,
    UpdateMenuActions,
    ShowOverwriteConfirm,
    HideOverwriteConfirm,
    ShowLoadConfirm,
    HideLoadConfirm,
    ShowSaveDescription,
    HideSaveDescription,
    ShowDeleteConfirm,
    HideDeleteConfirm,
    EnableListbox,
    DisableListbox,
    EnableButtonFrame,
    DisableButtonFrame,
    SetFocusEditDesc,
    SetEditDescription,
    Save {
        filename: String,
        description: String,
        file_type: SaveFileTypePort,
    },
    Load {
        filename: String,
    },
    DeleteFile {
        filename: String,
    },
    PrepareNewGame,
    DestroyQuitMenu,
    RemoveTransition(String),
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
    pub pending_description: Option<String>,
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

    pub fn set_pending_description(&mut self, desc: String) {
        self.pending_description = Some(desc);
    }

    pub fn save_file_type(&self) -> SaveFileTypePort {
        if self.layout_type == SaveLoadLayoutTypePort::SaveAndLoad {
            SaveFileTypePort::Normal
        } else {
            SaveFileTypePort::Mission
        }
    }

    pub fn get_selected_entry(&self) -> Option<&SaveGameEntryPort> {
        self.selected_index.and_then(|i| self.entries.get(i))
    }

    pub fn populate_save_load_list(&mut self, entries: Vec<SaveGameEntryPort>) {
        self.entries = entries;
        if !self.entries.is_empty() {
            self.selected_index = Some(0);
        } else {
            self.selected_index = None;
        }
    }

    pub fn do_save(&mut self, description: String) -> Option<(String, String, SaveFileTypePort)> {
        let entry = self.get_selected_entry()?;
        Some((entry.filename.clone(), description, self.save_file_type()))
    }

    pub fn do_load(&self) -> Option<String> {
        self.get_selected_entry().map(|e| e.filename.clone())
    }

    pub fn do_delete(&self) -> Option<String> {
        self.get_selected_entry().map(|e| e.filename.clone())
    }

    pub fn handle_button(
        &mut self,
        button: SaveLoadButtonId,
        shell_active: bool,
    ) -> Vec<SaveLoadCommand> {
        let mut commands = Vec::new();

        match button {
            SaveLoadButtonId::Back => {
                if self.is_popup {
                    commands.push(SaveLoadCommand::CloseMenu);
                } else {
                    commands.push(SaveLoadCommand::ShellPop);
                }
            }

            SaveLoadButtonId::Load => {
                if self.get_selected_entry().is_none() {
                    return commands;
                }
                if shell_active {
                    commands.push(SaveLoadCommand::CloseMenu);
                    commands.push(SaveLoadCommand::RemoveTransition(
                        "MainMenuLoadReplayMenu".into(),
                    ));
                    commands.push(SaveLoadCommand::RemoveTransition(
                        "MainMenuLoadReplayMenuBack".into(),
                    ));
                    commands.push(SaveLoadCommand::PrepareNewGame);
                    if let Some(entry) = self.get_selected_entry() {
                        commands.push(SaveLoadCommand::Load {
                            filename: entry.filename.clone(),
                        });
                    }
                } else {
                    commands.push(SaveLoadCommand::DisableListbox);
                    commands.push(SaveLoadCommand::DisableButtonFrame);
                    commands.push(SaveLoadCommand::ShowLoadConfirm);
                }
            }

            SaveLoadButtonId::Save => {
                if !self.can_save() {
                    return commands;
                }
                if self.get_selected_entry().is_none() {
                    commands.push(SaveLoadCommand::ShowSaveDescription);
                    commands.push(SaveLoadCommand::SetEditDescription);
                    commands.push(SaveLoadCommand::DisableListbox);
                    commands.push(SaveLoadCommand::SetFocusEditDesc);
                } else {
                    commands.push(SaveLoadCommand::DisableListbox);
                    commands.push(SaveLoadCommand::DisableButtonFrame);
                    commands.push(SaveLoadCommand::ShowOverwriteConfirm);
                }
            }

            SaveLoadButtonId::Delete => {
                if self.get_selected_entry().is_none() {
                    return commands;
                }
                commands.push(SaveLoadCommand::DisableListbox);
                commands.push(SaveLoadCommand::DisableButtonFrame);
                commands.push(SaveLoadCommand::ShowDeleteConfirm);
            }

            SaveLoadButtonId::DeleteConfirm => {
                if let Some(entry) = self.get_selected_entry() {
                    commands.push(SaveLoadCommand::DeleteFile {
                        filename: entry.filename.clone(),
                    });
                }
                commands.push(SaveLoadCommand::PopulateList);
                commands.push(SaveLoadCommand::HideDeleteConfirm);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
            }

            SaveLoadButtonId::DeleteCancel => {
                commands.push(SaveLoadCommand::HideDeleteConfirm);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
            }

            SaveLoadButtonId::OverwriteConfirm => {
                commands.push(SaveLoadCommand::HideOverwriteConfirm);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
                commands.push(SaveLoadCommand::CloseMenu);
                if let Some(entry) = self.get_selected_entry() {
                    commands.push(SaveLoadCommand::Save {
                        filename: entry.filename.clone(),
                        description: entry.description.clone(),
                        file_type: self.save_file_type(),
                    });
                }
            }

            SaveLoadButtonId::OverwriteCancel => {
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
                commands.push(SaveLoadCommand::EnableListbox);
            }

            SaveLoadButtonId::SaveDescConfirm => {
                let desc = self.pending_description.clone().unwrap_or_default();
                commands.push(SaveLoadCommand::HideSaveDescription);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
                commands.push(SaveLoadCommand::CloseMenu);
                let filename = self
                    .get_selected_entry()
                    .map(|e| e.filename.clone())
                    .unwrap_or_default();
                commands.push(SaveLoadCommand::Save {
                    filename,
                    description: desc,
                    file_type: self.save_file_type(),
                });
            }

            SaveLoadButtonId::SaveDescCancel => {
                commands.push(SaveLoadCommand::HideSaveDescription);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
            }

            SaveLoadButtonId::LoadConfirm => {
                commands.push(SaveLoadCommand::HideLoadConfirm);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
                commands.push(SaveLoadCommand::CloseMenu);
                if let Some(entry) = self.get_selected_entry() {
                    commands.push(SaveLoadCommand::Load {
                        filename: entry.filename.clone(),
                    });
                }
            }

            SaveLoadButtonId::LoadCancel => {
                commands.push(SaveLoadCommand::HideLoadConfirm);
                commands.push(SaveLoadCommand::EnableListbox);
                commands.push(SaveLoadCommand::EnableButtonFrame);
                commands.push(SaveLoadCommand::UpdateMenuActions);
            }
        }

        commands
    }

    pub fn sample() -> Self {
        Self {
            layout_type: SaveLoadLayoutTypePort::SaveAndLoad,
            is_popup: true,
            selected_index: Some(0),
            active_modal: SaveLoadModalPort::None,
            pending_action: None,
            pending_description: None,
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

    #[test]
    fn handle_button_back_popup_closes_menu() {
        let mut state = PopupSaveLoadPort::sample();
        state.is_popup = true;
        let cmds = state.handle_button(SaveLoadButtonId::Back, false);
        assert!(cmds.contains(&SaveLoadCommand::CloseMenu));
        assert!(!cmds.iter().any(|c| matches!(c, SaveLoadCommand::ShellPop)));
    }

    #[test]
    fn handle_button_back_fullscreen_pops_shell() {
        let mut state = PopupSaveLoadPort::sample();
        state.is_popup = false;
        let cmds = state.handle_button(SaveLoadButtonId::Back, false);
        assert!(cmds.contains(&SaveLoadCommand::ShellPop));
        assert!(!cmds.iter().any(|c| matches!(c, SaveLoadCommand::CloseMenu)));
    }

    #[test]
    fn handle_button_load_no_selection_is_noop() {
        let mut state = PopupSaveLoadPort::sample();
        state.selected_index = None;
        let cmds = state.handle_button(SaveLoadButtonId::Load, false);
        assert!(cmds.is_empty());
    }

    #[test]
    fn handle_button_load_shell_active_direct_load() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::Load, true);
        assert!(cmds.contains(&SaveLoadCommand::CloseMenu));
        assert!(cmds.contains(&SaveLoadCommand::PrepareNewGame));
        assert!(cmds.iter().any(
            |c| matches!(c, SaveLoadCommand::Load { filename } if filename == "mission01.sav")
        ));
    }

    #[test]
    fn handle_button_load_in_game_shows_confirm() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::Load, false);
        assert!(cmds.contains(&SaveLoadCommand::DisableListbox));
        assert!(cmds.contains(&SaveLoadCommand::DisableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::ShowLoadConfirm));
    }

    #[test]
    fn handle_button_load_confirm_closes_and_loads() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::LoadConfirm, false);
        assert!(cmds.contains(&SaveLoadCommand::HideLoadConfirm));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
        assert!(cmds.contains(&SaveLoadCommand::CloseMenu));
        assert!(cmds.iter().any(
            |c| matches!(c, SaveLoadCommand::Load { filename } if filename == "mission01.sav")
        ));
    }

    #[test]
    fn handle_button_load_cancel_restores_ui() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::LoadCancel, false);
        assert!(cmds.contains(&SaveLoadCommand::HideLoadConfirm));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
        assert!(!cmds
            .iter()
            .any(|c| matches!(c, SaveLoadCommand::Load { .. })));
    }

    #[test]
    fn handle_button_save_no_selection_shows_desc() {
        let mut state = PopupSaveLoadPort::sample();
        state.selected_index = None;
        let cmds = state.handle_button(SaveLoadButtonId::Save, false);
        assert!(cmds.contains(&SaveLoadCommand::ShowSaveDescription));
        assert!(cmds.contains(&SaveLoadCommand::SetEditDescription));
        assert!(cmds.contains(&SaveLoadCommand::DisableListbox));
        assert!(cmds.contains(&SaveLoadCommand::SetFocusEditDesc));
    }

    #[test]
    fn handle_button_save_with_selection_shows_overwrite() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::Save, false);
        assert!(cmds.contains(&SaveLoadCommand::ShowOverwriteConfirm));
        assert!(cmds.contains(&SaveLoadCommand::DisableListbox));
        assert!(cmds.contains(&SaveLoadCommand::DisableButtonFrame));
    }

    #[test]
    fn handle_button_save_disabled_in_load_only() {
        let mut state = PopupSaveLoadPort::sample();
        state.layout_type = SaveLoadLayoutTypePort::LoadOnly;
        let cmds = state.handle_button(SaveLoadButtonId::Save, false);
        assert!(cmds.is_empty());
    }

    #[test]
    fn handle_button_overwrite_confirm_saves_with_existing_desc() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::OverwriteConfirm, false);
        assert!(cmds.contains(&SaveLoadCommand::HideOverwriteConfirm));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
        assert!(cmds.contains(&SaveLoadCommand::CloseMenu));
        assert!(cmds.iter().any(|c| matches!(c, SaveLoadCommand::Save {
            filename,
            description,
            file_type: SaveFileTypePort::Normal,
        } if filename == "mission01.sav" && description == "Black Gold briefing")));
    }

    #[test]
    fn handle_button_overwrite_cancel_restores_ui() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::OverwriteCancel, false);
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(!cmds
            .iter()
            .any(|c| matches!(c, SaveLoadCommand::Save { .. })));
    }

    #[test]
    fn handle_button_save_desc_confirm_uses_pending_description() {
        let mut state = PopupSaveLoadPort::sample();
        state.set_pending_description("My custom save".to_string());
        let cmds = state.handle_button(SaveLoadButtonId::SaveDescConfirm, false);
        assert!(cmds.contains(&SaveLoadCommand::HideSaveDescription));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
        assert!(cmds.contains(&SaveLoadCommand::CloseMenu));
        assert!(cmds.iter().any(|c| matches!(c, SaveLoadCommand::Save {
            description,
            file_type: SaveFileTypePort::Normal,
            ..
        } if description == "My custom save")));
    }

    #[test]
    fn handle_button_save_desc_cancel_restores_ui() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::SaveDescCancel, false);
        assert!(cmds.contains(&SaveLoadCommand::HideSaveDescription));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
    }

    #[test]
    fn handle_button_delete_no_selection_is_noop() {
        let mut state = PopupSaveLoadPort::sample();
        state.selected_index = None;
        let cmds = state.handle_button(SaveLoadButtonId::Delete, false);
        assert!(cmds.is_empty());
    }

    #[test]
    fn handle_button_delete_shows_confirm() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::Delete, false);
        assert!(cmds.contains(&SaveLoadCommand::DisableListbox));
        assert!(cmds.contains(&SaveLoadCommand::DisableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::ShowDeleteConfirm));
    }

    #[test]
    fn handle_button_delete_confirm_deletes_and_repopulates() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::DeleteConfirm, false);
        assert!(cmds.iter().any(
            |c| matches!(c, SaveLoadCommand::DeleteFile { filename } if filename == "mission01.sav")
        ));
        assert!(cmds.contains(&SaveLoadCommand::PopulateList));
        assert!(cmds.contains(&SaveLoadCommand::HideDeleteConfirm));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
    }

    #[test]
    fn handle_button_delete_cancel_restores_ui() {
        let mut state = PopupSaveLoadPort::sample();
        let cmds = state.handle_button(SaveLoadButtonId::DeleteCancel, false);
        assert!(cmds.contains(&SaveLoadCommand::HideDeleteConfirm));
        assert!(cmds.contains(&SaveLoadCommand::EnableListbox));
        assert!(cmds.contains(&SaveLoadCommand::EnableButtonFrame));
        assert!(cmds.contains(&SaveLoadCommand::UpdateMenuActions));
    }

    #[test]
    fn populate_save_load_list_updates_entries() {
        let mut state = PopupSaveLoadPort::sample();
        let entries = vec![
            SaveGameEntryPort {
                filename: "a.sav".to_string(),
                description: "A".to_string(),
            },
            SaveGameEntryPort {
                filename: "b.sav".to_string(),
                description: "B".to_string(),
            },
        ];
        state.populate_save_load_list(entries);
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.selected_index, Some(0));
        assert_eq!(state.entries[0].filename, "a.sav");
    }

    #[test]
    fn populate_save_load_list_empty_clears_selection() {
        let mut state = PopupSaveLoadPort::sample();
        state.populate_save_load_list(vec![]);
        assert!(state.entries.is_empty());
        assert_eq!(state.selected_index, None);
    }

    #[test]
    fn do_save_returns_file_info() {
        let mut state = PopupSaveLoadPort::sample();
        let result = state.do_save("test desc".to_string());
        assert_eq!(
            result,
            Some((
                "mission01.sav".to_string(),
                "test desc".to_string(),
                SaveFileTypePort::Normal
            ))
        );
    }

    #[test]
    fn do_save_no_selection_returns_none() {
        let mut state = PopupSaveLoadPort::sample();
        state.selected_index = None;
        assert!(state.do_save("desc".to_string()).is_none());
    }

    #[test]
    fn do_load_returns_filename() {
        let state = PopupSaveLoadPort::sample();
        assert_eq!(state.do_load(), Some("mission01.sav".to_string()));
    }

    #[test]
    fn do_delete_returns_filename() {
        let state = PopupSaveLoadPort::sample();
        assert_eq!(state.do_delete(), Some("mission01.sav".to_string()));
    }

    #[test]
    fn save_file_type_matches_layout() {
        let mut state = PopupSaveLoadPort::sample();
        assert_eq!(state.save_file_type(), SaveFileTypePort::Normal);
        state.layout_type = SaveLoadLayoutTypePort::LoadOnly;
        assert_eq!(state.save_file_type(), SaveFileTypePort::Mission);
    }
}
