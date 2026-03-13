use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ReplayMenu.cpp",
    "crate::gui::callbacks::menus::replay_menu",
    "Replay Menu",
    "Replay-browser callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ReplayMenu",
    "Replay Menu",
    "Browse and launch saved replays.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayKindPort {
    SinglePlayer,
    Multiplayer,
}

impl ReplayKindPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::SinglePlayer => "SP",
            Self::Multiplayer => "MP",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayEntryPort {
    pub replay_name: String,
    pub replay_filename: String,
    pub display_time: String,
    pub version: String,
    pub map_name: String,
    pub replay_kind: ReplayKindPort,
    pub version_is_compatible: bool,
    pub requires_version_confirmation: bool,
    pub is_last_replay: bool,
}

impl ReplayEntryPort {
    pub fn display_label(&self) -> String {
        let color_hint = if self.version_is_compatible {
            "OK"
        } else {
            "CRC mismatch"
        };
        format!(
            "{} [{}] {} · {} · {}",
            self.replay_name,
            self.replay_kind.label(),
            self.display_time,
            self.map_name,
            color_hint
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayPromptPort {
    NoSelection {
        title: String,
        body: String,
    },
    OlderVersion {
        title: String,
        body: String,
        filename: String,
    },
    DeleteConfirm {
        title: String,
        body: String,
        filename: String,
    },
    CopyConfirm {
        title: String,
        body: String,
        filename: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayMenuPort {
    pub shell_map_visible: bool,
    pub visible: bool,
    pub is_shutting_down: bool,
    pub initial_gadget_delay: u16,
    pub just_entered: bool,
    pub gadget_parent_hidden: bool,
    pub wants_input_focus: bool,
    pub entries: Vec<ReplayEntryPort>,
    pub selected_index: Option<usize>,
    pub pending_prompt: Option<ReplayPromptPort>,
    pub call_copy: bool,
    pub call_delete: bool,
    pub loaded_replay: Option<String>,
    pub copied_replay: Option<String>,
    pub deleted_replay: Option<String>,
    pub back_requested: bool,
    pub active_transition_group: Option<String>,
    pub reverse_transition_group: Option<String>,
}

impl Default for ReplayMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ReplayMenuPort {
    pub fn init(entries: Vec<ReplayEntryPort>) -> Self {
        let selected_index = (!entries.is_empty()).then_some(0);
        Self {
            shell_map_visible: true,
            visible: true,
            is_shutting_down: false,
            initial_gadget_delay: 2,
            just_entered: true,
            gadget_parent_hidden: true,
            wants_input_focus: true,
            entries,
            selected_index,
            pending_prompt: None,
            call_copy: false,
            call_delete: false,
            loaded_replay: None,
            copied_replay: None,
            deleted_replay: None,
            back_requested: false,
            active_transition_group: None,
            reverse_transition_group: None,
        }
    }

    pub fn update(&mut self, shell_anim_finished: bool, transition_finished: bool) -> bool {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.active_transition_group = Some("ReplayMenuFade".to_string());
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay = self.initial_gadget_delay.saturating_sub(1);
            }
        }

        if self.call_copy {
            self.copy_selected();
        }
        if self.call_delete {
            self.delete_selected();
        }

        if self.is_shutting_down && shell_anim_finished && transition_finished {
            self.is_shutting_down = false;
            self.visible = false;
            return true;
        }

        false
    }

    pub fn shutdown(&mut self, pop_immediate: bool) -> bool {
        if pop_immediate {
            self.visible = false;
            return true;
        }

        self.reverse_transition_group = Some("ReplayMenuFade".to_string());
        self.is_shutting_down = true;
        false
    }

    pub fn handle_escape(&mut self, key_up: bool) -> bool {
        if !key_up {
            return false;
        }
        self.back_requested = true;
        true
    }

    pub fn take_input_focus(&self, offered_focus: bool) -> bool {
        offered_focus && self.wants_input_focus
    }

    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.selected_index = Some(index);
        true
    }

    pub fn double_click_selected(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        self.loaded_replay = Some(entry.replay_filename.clone());
        self.visible = false;
        true
    }

    pub fn load_selected(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        if entry.requires_version_confirmation {
            self.pending_prompt = Some(ReplayPromptPort::OlderVersion {
                title: "Older replay version".to_string(),
                body: "This replay was recorded with a different build. Continue anyway?"
                    .to_string(),
                filename: entry.replay_filename.clone(),
            });
            return false;
        }

        self.loaded_replay = Some(entry.replay_filename.clone());
        self.visible = false;
        true
    }

    pub fn confirm_version_load(&mut self) -> bool {
        let Some(ReplayPromptPort::OlderVersion { filename, .. }) = self.pending_prompt.take()
        else {
            return false;
        };
        self.loaded_replay = Some(filename);
        self.visible = false;
        true
    }

    pub fn request_delete(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        self.pending_prompt = Some(ReplayPromptPort::DeleteConfirm {
            title: "Delete file".to_string(),
            body: "Are you sure you want to delete this replay?".to_string(),
            filename: entry.replay_filename.clone(),
        });
        true
    }

    pub fn confirm_delete(&mut self) -> bool {
        let Some(ReplayPromptPort::DeleteConfirm { .. }) = self.pending_prompt else {
            return false;
        };
        self.call_delete = true;
        true
    }

    pub fn request_copy(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        self.pending_prompt = Some(ReplayPromptPort::CopyConfirm {
            title: "Copy replay".to_string(),
            body: "Copy this replay to the desktop?".to_string(),
            filename: entry.replay_filename.clone(),
        });
        true
    }

    pub fn confirm_copy(&mut self) -> bool {
        let Some(ReplayPromptPort::CopyConfirm { .. }) = self.pending_prompt else {
            return false;
        };
        self.call_copy = true;
        true
    }

    pub fn sample() -> Self {
        Self::init(vec![
            ReplayEntryPort {
                replay_name: "Last Replay".to_string(),
                replay_filename: "LastReplay.rep".to_string(),
                display_time: "2026-03-11 21:15".to_string(),
                version: "1.04".to_string(),
                map_name: "Tournament Desert".to_string(),
                replay_kind: ReplayKindPort::SinglePlayer,
                version_is_compatible: true,
                requires_version_confirmation: false,
                is_last_replay: true,
            },
            ReplayEntryPort {
                replay_name: "Ladder Finals".to_string(),
                replay_filename: "LadderFinals.rep".to_string(),
                display_time: "2026-03-09 18:42".to_string(),
                version: "1.04".to_string(),
                map_name: "Defcon 6".to_string(),
                replay_kind: ReplayKindPort::Multiplayer,
                version_is_compatible: true,
                requires_version_confirmation: false,
                is_last_replay: false,
            },
            ReplayEntryPort {
                replay_name: "Old Patch Run".to_string(),
                replay_filename: "OldPatchRun.rep".to_string(),
                display_time: "2025-11-28 09:05".to_string(),
                version: "1.03".to_string(),
                map_name: "Forgotten Forest".to_string(),
                replay_kind: ReplayKindPort::SinglePlayer,
                version_is_compatible: false,
                requires_version_confirmation: true,
                is_last_replay: false,
            },
        ])
    }

    fn selected_entry(&self) -> Option<&ReplayEntryPort> {
        self.selected_index
            .and_then(|index| self.entries.get(index))
    }

    fn copy_selected(&mut self) {
        self.call_copy = false;
        let Some(index) = self.selected_index else {
            return;
        };
        if let Some(entry) = self.entries.get(index) {
            self.copied_replay = Some(format!("Desktop/{}", entry.replay_filename));
        }
        self.pending_prompt = None;
    }

    fn delete_selected(&mut self) {
        self.call_delete = false;
        let Some(index) = self.selected_index else {
            return;
        };
        if index >= self.entries.len() {
            return;
        }
        let removed = self.entries.remove(index);
        self.deleted_replay = Some(removed.replay_filename);
        self.selected_index = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        self.pending_prompt = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incompatible_replay_requires_confirmation_before_load() {
        let mut menu = ReplayMenuPort::sample();
        assert!(menu.select_index(2));

        assert!(!menu.load_selected());
        assert!(matches!(
            menu.pending_prompt,
            Some(ReplayPromptPort::OlderVersion { .. })
        ));
        assert!(menu.confirm_version_load());
        assert_eq!(menu.loaded_replay.as_deref(), Some("OldPatchRun.rep"));
    }

    #[test]
    fn delete_replay_removes_selected_entry_after_confirmation() {
        let mut menu = ReplayMenuPort::sample();
        assert!(menu.select_index(1));

        assert!(menu.request_delete());
        assert!(menu.confirm_delete());
        assert!(!menu.update(false, false));

        assert_eq!(menu.entries.len(), 2);
        assert_eq!(menu.deleted_replay.as_deref(), Some("LadderFinals.rep"));
    }

    #[test]
    fn update_sets_fade_group_after_entry_delay() {
        let mut menu = ReplayMenuPort::sample();

        assert!(!menu.update(false, false));
        assert!(!menu.update(false, false));
        assert_eq!(
            menu.active_transition_group.as_deref(),
            Some("ReplayMenuFade")
        );
        assert!(!menu.just_entered);
    }
}
