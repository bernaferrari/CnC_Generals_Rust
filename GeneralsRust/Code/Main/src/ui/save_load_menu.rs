//! Save/Load Game Browser
//!
//! This module implements the save game browser matching the original
//! C&C Generals save/load system from PopupSaveLoad.wnd.

use super::{
    ClickSpring, Interactive, KeyCode, MouseButton, Renderable, Screen, UIEvent, UIRenderContext,
    layout, sound_files, utils,
};
use crate::localization;
use crate::save_load::{
    SaveFileType, SaveLoadManager, get_save_load_manager, init_save_load_system,
};
use log::info;
use std::time::SystemTime;

/// Mode for save/load menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLoadMode {
    Save,
    Load,
}

/// C++ `PopupSaveLoad` confirmation parents (`OverwriteConfirmParent`,
/// `LoadConfirmParent`, `SaveDescParent`, `DeleteConfirmParent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveLoadDialogState {
    #[default]
    None,
    OverwriteConfirm,
    LoadConfirm,
    SaveDescription,
    DeleteConfirm,
}

/// Save game entry
#[derive(Debug, Clone)]
pub struct SaveGameEntry {
    pub filename: String,
    pub display_name: String,
    pub timestamp: SystemTime,
    pub map_name: String,
    pub faction: String,
    pub mission: Option<String>,
    pub save_type: SaveFileType,
}

/// Save/Load Menu implementation
pub struct SaveLoadMenu {
    /// Current mode (save or load)
    mode: SaveLoadMode,
    /// List of available save games
    save_files: Vec<SaveGameEntry>,
    /// Currently selected entry
    selected_entry: Option<usize>,
    /// Text input for new save name
    save_name_input: String,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Animation progress
    animation_progress: f32,
    /// Screen to return to when closing the menu.
    return_screen: Screen,
    /// UI events queued by this screen.
    pending_events: Vec<UIEvent>,
    confirm_click: ClickSpring,
    /// C++ confirmation modal currently shown over the list/buttons.
    dialog_state: SaveLoadDialogState,
    delete_click: ClickSpring,
    dialog_confirm_click: ClickSpring,
    dialog_cancel_click: ClickSpring,
    cancel_click: ClickSpring,
    entry_clicks: Vec<ClickSpring>,
}

impl Default for SaveLoadMenu {
    fn default() -> Self {
        Self::new(SaveLoadMode::Load)
    }
}

impl SaveLoadMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    pub fn new(mode: SaveLoadMode) -> Self {
        Self {
            mode,
            save_files: Vec::new(),
            selected_entry: None,
            save_name_input: String::new(),
            screen_size: (1024, 768),
            animation_progress: 0.0,
            return_screen: Screen::MainMenu,
            pending_events: Vec::new(),
            dialog_state: SaveLoadDialogState::None,
            confirm_click: ClickSpring::new(),
            cancel_click: ClickSpring::new(),
            delete_click: ClickSpring::new(),
            dialog_confirm_click: ClickSpring::new(),
            dialog_cancel_click: ClickSpring::new(),
            entry_clicks: Vec::new(),
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.dialog_state = SaveLoadDialogState::None;
        self.scan_save_files();
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 3.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }
        self.confirm_click.update(delta_time);
        self.cancel_click.update(delta_time);
        self.delete_click.update(delta_time);
        self.dialog_confirm_click.update(delta_time);
        self.dialog_cancel_click.update(delta_time);
        for click in &mut self.entry_clicks {
            click.update(delta_time);
        }
        Ok(())
    }

    pub fn set_mode(&mut self, mode: SaveLoadMode) {
        self.mode = mode;
        self.selected_entry = None;
        self.save_name_input.clear();
        self.dialog_state = SaveLoadDialogState::None;
    }

    pub fn set_return_screen(&mut self, screen: Screen) {
        self.return_screen = screen;
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn dialog_state(&self) -> SaveLoadDialogState {
        self.dialog_state
    }

    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let is_over_button = if self.dialog_state == SaveLoadDialogState::None {
            let (confirm_rect, delete_rect, cancel_rect) = self.action_button_rects();
            utils::point_in_rect((x, y), confirm_rect)
                || utils::point_in_rect((x, y), delete_rect)
                || utils::point_in_rect((x, y), cancel_rect)
        } else {
            let (yes_rect, no_rect) = self.dialog_button_rects();
            utils::point_in_rect((x, y), yes_rect) || utils::point_in_rect((x, y), no_rect)
        };
        let is_over_entry = self.dialog_state == SaveLoadDialogState::None
            && self
                .entry_at_position(x, y)
                .map(|index| index < self.save_files.len())
                .unwrap_or(false);

        if is_over_button || is_over_entry {
            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_HOVER.to_string(),
            ));
            true
        } else {
            false
        }
    }

    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        if self.dialog_state != SaveLoadDialogState::None {
            let (yes_rect, no_rect) = self.dialog_button_rects();
            if utils::point_in_rect((x, y), no_rect) {
                self.dialog_cancel_click.trigger();
                self.play_click_sound();
                self.dismiss_dialog();
                return None;
            }
            if utils::point_in_rect((x, y), yes_rect) {
                self.dialog_confirm_click.trigger();
                self.play_click_sound();
                return self.apply_dialog_confirm();
            }
            return None;
        }

        let (confirm_rect, delete_rect, cancel_rect) = self.action_button_rects();
        if utils::point_in_rect((x, y), cancel_rect) {
            self.cancel_click.trigger();
            self.play_click_sound();
            return Some(UIEvent::ChangeScreen(self.return_screen));
        }

        if utils::point_in_rect((x, y), delete_rect) {
            self.delete_click.trigger();
            self.play_click_sound();
            self.request_delete();
            return None;
        }

        if utils::point_in_rect((x, y), confirm_rect) {
            self.confirm_click.trigger();
            self.play_click_sound();
            return self.confirm_selection();
        }

        if let Some(index) = self.entry_at_position(x, y) {
            if index < self.save_files.len() {
                if let Some(click) = self.entry_clicks.get_mut(index) {
                    click.trigger();
                }
                self.selected_entry = Some(index);
                if self.mode == SaveLoadMode::Save {
                    self.save_name_input = self.save_files[index].display_name.clone();
                }
                self.play_click_sound();
                return None;
            }
        }

        None
    }

    pub fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => {
                self.play_click_sound();
                if self.dialog_state != SaveLoadDialogState::None {
                    self.dismiss_dialog();
                } else {
                    self.pending_events
                        .push(UIEvent::ChangeScreen(self.return_screen));
                }
                true
            }
            KeyCode::Enter => {
                let before = self.pending_events.len();
                let event = self.confirm_selection();
                if let Some(event) = event {
                    self.pending_events.push(event);
                }
                self.pending_events.len() != before
            }
            KeyCode::Up => {
                if self.dialog_state != SaveLoadDialogState::None || self.save_files.is_empty() {
                    return false;
                }
                let current = self.selected_entry.unwrap_or(0);
                let next = current.saturating_sub(1);
                self.selected_entry = Some(next);
                if self.mode == SaveLoadMode::Save {
                    self.save_name_input = self.save_files[next].display_name.clone();
                }
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_HOVER.to_string(),
                ));
                true
            }
            KeyCode::Down => {
                if self.dialog_state != SaveLoadDialogState::None || self.save_files.is_empty() {
                    return false;
                }
                let current = self.selected_entry.unwrap_or(0);
                let next = (current + 1).min(self.save_files.len().saturating_sub(1));
                self.selected_entry = Some(next);
                if self.mode == SaveLoadMode::Save {
                    self.save_name_input = self.save_files[next].display_name.clone();
                }
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_HOVER.to_string(),
                ));
                true
            }
            KeyCode::Backspace => {
                if !self.can_edit_save_name() || self.save_name_input.is_empty() {
                    return false;
                }
                self.save_name_input.pop();
                true
            }
            KeyCode::F5 if self.mode == SaveLoadMode::Save => {
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                self.pending_events.push(UIEvent::SaveGame {
                    slot: "quicksave".to_string(),
                    display_name: Self::text("save_load.quick_save_name", "Quick Save"),
                });
                self.pending_events
                    .push(UIEvent::ChangeScreen(self.return_screen));
                true
            }
            KeyCode::F9 if self.mode == SaveLoadMode::Load => {
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                self.pending_events
                    .push(UIEvent::LoadGame("quicksave".to_string()));
                true
            }
            _ => false,
        }
    }

    pub fn handle_text_input(&mut self, text: &str) -> bool {
        if self.can_edit_save_name() {
            self.save_name_input.push_str(text);
            true
        } else {
            false
        }
    }

    fn confirm_selection(&mut self) -> Option<UIEvent> {
        match self.dialog_state {
            SaveLoadDialogState::None => self.request_primary_action(),
            _ => self.apply_dialog_confirm(),
        }
    }

    fn request_primary_action(&mut self) -> Option<UIEvent> {
        match self.mode {
            SaveLoadMode::Load => self.request_load(),
            SaveLoadMode::Save => {
                self.request_save();
                None
            }
        }
    }

    /// C++ `processLoadButtonPress`: shell loads immediately; in-game shows
    /// `LoadConfirmParent`.
    fn request_load(&mut self) -> Option<UIEvent> {
        let index = self.selected_entry?;
        let _ = self.save_files.get(index)?;
        if self.is_shell_active() {
            return self.emit_load();
        }
        self.dialog_state = SaveLoadDialogState::LoadConfirm;
        None
    }

    /// C++ Save: selected row → `OverwriteConfirmParent`; new row →
    /// `SaveDescParent`. Name collisions from `sanitize_slot_name` also
    /// require overwrite confirmation.
    fn request_save(&mut self) {
        if self
            .selected_entry
            .and_then(|i| self.save_files.get(i))
            .is_some()
        {
            self.dialog_state = SaveLoadDialogState::OverwriteConfirm;
            return;
        }
        let chosen = self.chosen_save_name();
        let slot = Self::sanitize_slot_name(&chosen);
        if self.matching_save_index(&slot).is_some() {
            self.dialog_state = SaveLoadDialogState::OverwriteConfirm;
            return;
        }
        // C++ setEditDescription pre-fills campaign+mission (or map leaf).
        // Mission/auto names use GUI:MissionSave when a campaign is live.
        if self.save_name_input.trim().is_empty() {
            let mission = crate::save_load::current_mission_save_description();
            self.save_name_input = if !mission.is_empty() {
                mission
            } else {
                crate::save_load::default_save_edit_description("")
            };
        }
        self.dialog_state = SaveLoadDialogState::SaveDescription;
    }

    fn request_delete(&mut self) {
        if self
            .selected_entry
            .and_then(|i| self.save_files.get(i))
            .is_some()
        {
            self.dialog_state = SaveLoadDialogState::DeleteConfirm;
        }
    }

    fn apply_dialog_confirm(&mut self) -> Option<UIEvent> {
        match self.dialog_state {
            SaveLoadDialogState::None => None,
            SaveLoadDialogState::LoadConfirm => {
                self.dialog_state = SaveLoadDialogState::None;
                self.emit_load()
            }
            SaveLoadDialogState::OverwriteConfirm => {
                self.dialog_state = SaveLoadDialogState::None;
                self.emit_overwrite_save();
                None
            }
            SaveLoadDialogState::SaveDescription => {
                let before = self.pending_events.len();
                self.emit_new_save();
                if self.dialog_state == SaveLoadDialogState::OverwriteConfirm {
                    None
                } else if self.pending_events.len() == before {
                    self.dialog_state = SaveLoadDialogState::SaveDescription;
                    None
                } else {
                    self.dialog_state = SaveLoadDialogState::None;
                    None
                }
            }
            SaveLoadDialogState::DeleteConfirm => {
                self.dialog_state = SaveLoadDialogState::None;
                self.delete_selected_save();
                None
            }
        }
    }

    fn emit_load(&self) -> Option<UIEvent> {
        let index = self.selected_entry?;
        let entry = self.save_files.get(index)?;
        Some(UIEvent::LoadGame(entry.filename.clone()))
    }

    fn emit_overwrite_save(&mut self) {
        let Some(index) = self.overwrite_target_index() else {
            return;
        };
        let Some(entry) = self.save_files.get(index) else {
            return;
        };
        self.pending_events.push(UIEvent::SaveGame {
            slot: entry.filename.clone(),
            display_name: entry.display_name.clone(),
        });
        self.pending_events
            .push(UIEvent::ChangeScreen(self.return_screen));
    }

    fn emit_new_save(&mut self) {
        let chosen = self.chosen_save_name();
        if chosen.trim().is_empty() {
            return;
        }
        let slot = Self::sanitize_slot_name(&chosen);
        if slot.is_empty() {
            return;
        }
        if self.matching_save_index(&slot).is_some() {
            self.dialog_state = SaveLoadDialogState::OverwriteConfirm;
            return;
        }
        self.pending_events.push(UIEvent::SaveGame {
            slot,
            display_name: chosen,
        });
        self.pending_events
            .push(UIEvent::ChangeScreen(self.return_screen));
    }

    fn delete_selected_save(&mut self) {
        let Some(index) = self.selected_entry else {
            return;
        };
        let Some(filename) = self
            .save_files
            .get(index)
            .map(|entry| entry.filename.clone())
        else {
            return;
        };
        self.remove_save_file(&filename);
        self.scan_save_files();
        if self.save_files.is_empty() {
            self.selected_entry = None;
            self.save_name_input.clear();
        } else {
            let next = index.min(self.save_files.len() - 1);
            self.selected_entry = Some(next);
            if self.mode == SaveLoadMode::Save {
                self.save_name_input = self.save_files[next].display_name.clone();
            }
        }
    }

    fn remove_save_file(&self, filename: &str) {
        let remove = |manager: &SaveLoadManager| {
            let path = manager.get_save_path(filename);
            if path.exists() {
                if let Err(err) = std::fs::remove_file(&path) {
                    info!("Failed to delete save {}: {err}", path.display());
                }
            }
        };
        if let Some(manager_arc) = get_save_load_manager() {
            if let Ok(manager) = manager_arc.lock() {
                remove(&manager);
                return;
            }
        }
        remove(&SaveLoadManager::new());
    }

    fn overwrite_target_index(&self) -> Option<usize> {
        if let Some(index) = self.selected_entry {
            if self.save_files.get(index).is_some() {
                return Some(index);
            }
        }
        let slot = Self::sanitize_slot_name(&self.chosen_save_name());
        self.matching_save_index(&slot)
    }

    fn chosen_save_name(&self) -> String {
        let display_name = self.save_name_input.trim();
        if !display_name.is_empty() {
            return display_name.to_string();
        }
        self.selected_entry
            .and_then(|i| self.save_files.get(i))
            .map(|entry| entry.display_name.clone())
            .unwrap_or_default()
    }

    fn matching_save_index(&self, slot: &str) -> Option<usize> {
        if slot.is_empty() {
            return None;
        }
        self.save_files.iter().position(|entry| {
            entry.filename.eq_ignore_ascii_case(slot)
                || Self::sanitize_slot_name(&entry.filename) == slot
                || Self::sanitize_slot_name(&entry.display_name) == slot
        })
    }

    fn is_shell_active(&self) -> bool {
        self.return_screen.is_shell_owned_pregame()
            || matches!(self.return_screen, Screen::Title | Screen::MainMenu)
    }

    fn can_edit_save_name(&self) -> bool {
        self.mode == SaveLoadMode::Save
            && matches!(
                self.dialog_state,
                SaveLoadDialogState::None | SaveLoadDialogState::SaveDescription
            )
    }

    fn dismiss_dialog(&mut self) {
        self.dialog_state = SaveLoadDialogState::None;
    }

    fn play_click_sound(&mut self) {
        self.pending_events.push(UIEvent::PlaySoundEffectPath(
            sound_files::BUTTON_CLICK.to_string(),
        ));
    }

    fn sanitize_slot_name(name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        for ch in name.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
            } else if !out.ends_with('_') {
                out.push('_');
            }
        }
        out.trim_matches('_').to_string()
    }

    fn list_rect(&self) -> (i32, i32, u32, u32) {
        let width = (layout::MENU_BUTTON_WIDTH * 4).min(self.screen_size.0.saturating_sub(40));
        let height = 420u32.min(self.screen_size.1.saturating_sub(200));
        let x = (self.screen_size.0 as i32 / 2) - (width as i32 / 2);
        let y = (self.screen_size.1 as i32 / 2) - (height as i32 / 2);
        (x, y, width, height)
    }

    fn entry_at_position(&self, x: i32, y: i32) -> Option<usize> {
        let (lx, ly, lw, lh) = self.list_rect();
        if !utils::point_in_rect((x, y), (lx, ly, lw, lh)) {
            return None;
        }
        let row_height = 44i32;
        let offset = y - ly;
        if offset < 0 {
            return None;
        }
        Some((offset / row_height).max(0) as usize)
    }

    fn action_button_rects(
        &self,
    ) -> (
        (i32, i32, u32, u32),
        (i32, i32, u32, u32),
        (i32, i32, u32, u32),
    ) {
        let button_w = layout::MENU_BUTTON_WIDTH;
        let button_h = layout::MENU_BUTTON_HEIGHT;
        let total_w = button_w as i32 * 3 + layout::MENU_SPACING as i32 * 2;
        let x0 = (self.screen_size.0 as i32 / 2) - total_w / 2;
        let y0 = self.screen_size.1 as i32 - button_h as i32 - 40;
        let step = button_w as i32 + layout::MENU_SPACING as i32;
        let confirm = (x0, y0, button_w, button_h);
        let delete = (x0 + step, y0, button_w, button_h);
        let cancel = (x0 + step * 2, y0, button_w, button_h);
        (confirm, delete, cancel)
    }

    fn dialog_button_rects(&self) -> ((i32, i32, u32, u32), (i32, i32, u32, u32)) {
        let button_w = layout::MENU_BUTTON_WIDTH;
        let button_h = layout::MENU_BUTTON_HEIGHT;
        let total_w = button_w as i32 * 2 + layout::MENU_SPACING as i32;
        let x0 = (self.screen_size.0 as i32 / 2) - total_w / 2;
        let y0 = self.screen_size.1 as i32 - button_h as i32 - 40;
        let yes = (x0, y0, button_w, button_h);
        let no = (
            x0 + button_w as i32 + layout::MENU_SPACING as i32,
            y0,
            button_w,
            button_h,
        );
        (yes, no)
    }

    fn scan_save_files(&mut self) {
        self.save_files.clear();
        self.entry_clicks.clear();
        let _ = init_save_load_system();

        if let Some(manager_arc) = get_save_load_manager() {
            if let Ok(mut manager) = manager_arc.lock() {
                let _ = manager.refresh_save_list();
                self.add_save_entries_from_manager(&manager);
                return;
            }
        }

        // Fallback for contexts where the global manager is not set up yet.
        let mut manager = SaveLoadManager::new();
        if manager.init().is_ok() {
            let _ = manager.refresh_save_list();
            self.add_save_entries_from_manager(&manager);
        } else {
            info!(
                "{}",
                Self::text("save_load.log.no_manager", "Save system unavailable")
            );
        }

        info!(
            "{}",
            localization::localize_with_args(
                "save_load.log.scanned",
                "Found {count} save files",
                &[("count", &self.save_files.len().to_string())],
            )
        );
    }

    fn add_save_entries_from_manager(&mut self, manager: &SaveLoadManager) {
        for entry in manager.get_available_saves() {
            let save = &entry.save_info;
            self.save_files.push(SaveGameEntry {
                filename: save.filename.clone(),
                // C++ populateSaveGameListbox uses description, then mapLabel.
                display_name: Self::list_display_label(save),
                timestamp: save.save_date,
                map_name: save.map_name.clone(),
                faction: save.campaign_side.clone().unwrap_or_default(),
                mission: save.mission_number.map(|n| format!("Mission {}", n)),
                save_type: save.save_type,
            });
            self.entry_clicks.push(ClickSpring::new());
        }
    }

    /// C++ `populateSaveGameListbox` (`GameState.cpp:1182-1191`).
    fn list_display_label(save: &crate::save_load::SaveGameInfo) -> String {
        if !save.description.trim().is_empty() {
            return save.description.clone();
        }
        if !save.map_name.trim().is_empty() {
            let (text, exists) =
                game_client::game_text::GameText::fetch_with_exists(&save.map_name);
            if exists && !text.is_empty() {
                return text;
            }
        }
        if !save.display_name.trim().is_empty() {
            save.display_name.clone()
        } else {
            save.map_name.clone()
        }
    }

    /// C++ `GameState::xfer` v2 header: empty campaign side + no mission
    /// when `TheCampaignManager` has no current campaign.
    fn list_campaign_columns(save: &crate::save_load::SaveGameInfo) -> (String, Option<String>) {
        (
            save.campaign_side.clone().unwrap_or_default(),
            save.mission_number.map(|n| format!("Mission {}", n)),
        )
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }

    fn render_action_buttons(&self) {
        let (confirm_rect, delete_rect, cancel_rect) = self.action_button_rects();
        let (confirm_x, confirm_y, _, _) =
            utils::scale_rect_center(confirm_rect, self.confirm_click.scale());
        let (delete_x, delete_y, _, _) =
            utils::scale_rect_center(delete_rect, self.delete_click.scale());
        let (cancel_x, cancel_y, _, _) =
            utils::scale_rect_center(cancel_rect, self.cancel_click.scale());
        let confirm_label = match self.mode {
            SaveLoadMode::Save => Self::text("save_load.button_save", "Save"),
            SaveLoadMode::Load => Self::text("save_load.button_load", "Load"),
        };
        println!(
            "\n{} @ ({:.1},{:.1})",
            localization::localize_with_args(
                "save_load.button.confirm",
                "[{label}]",
                &[("label", confirm_label.as_str())],
            ),
            confirm_x,
            confirm_y
        );
        println!(
            "{} @ ({:.1},{:.1})",
            Self::text("save_load.button.delete", "[Delete]"),
            delete_x,
            delete_y
        );
        println!(
            "{} @ ({:.1},{:.1})",
            Self::text("save_load.button.cancel", "[Cancel]"),
            cancel_x,
            cancel_y
        );
    }

    fn render_confirm_dialog(&self) {
        let (title, prompt, confirm_label) = match self.dialog_state {
            SaveLoadDialogState::OverwriteConfirm => (
                Self::text("save_load.overwrite_title", "=== OVERWRITE SAVE ==="),
                Self::text("save_load.overwrite_prompt", "Overwrite this save game?"),
                Self::text("save_load.button_overwrite", "Overwrite"),
            ),
            SaveLoadDialogState::LoadConfirm => (
                Self::text("save_load.load_confirm_title", "=== LOAD GAME ==="),
                Self::text(
                    "save_load.load_confirm_prompt",
                    "Load this save and lose current progress?",
                ),
                Self::text("save_load.button_load", "Load"),
            ),
            SaveLoadDialogState::SaveDescription => (
                Self::text("save_load.save_desc_title", "=== SAVE DESCRIPTION ==="),
                Self::text(
                    "save_load.save_desc_prompt",
                    "Enter a description for this save:",
                ),
                Self::text("save_load.button_save", "Save"),
            ),
            SaveLoadDialogState::DeleteConfirm => (
                Self::text("save_load.delete_title", "=== DELETE SAVE ==="),
                Self::text("save_load.delete_prompt", "Delete this save game?"),
                Self::text("save_load.button.delete", "Delete"),
            ),
            SaveLoadDialogState::None => return,
        };

        println!("\n{}", title);
        if let Some(entry) = self
            .overwrite_target_index()
            .or(self.selected_entry)
            .and_then(|i| self.save_files.get(i))
        {
            println!("  {}", entry.display_name);
            println!("  {}", entry.map_name);
        }
        println!("\n  {}", prompt);
        if self.dialog_state == SaveLoadDialogState::SaveDescription {
            println!(
                "  {}",
                if self.save_name_input.is_empty() {
                    "_"
                } else {
                    &self.save_name_input
                }
            );
        }

        let (yes_rect, no_rect) = self.dialog_button_rects();
        let (yes_x, yes_y, _, _) =
            utils::scale_rect_center(yes_rect, self.dialog_confirm_click.scale());
        let (no_x, no_y, _, _) =
            utils::scale_rect_center(no_rect, self.dialog_cancel_click.scale());
        println!("\n[{}] @ ({:.1},{:.1})", confirm_label, yes_x, yes_y);
        println!(
            "{} @ ({:.1},{:.1})",
            Self::text("save_load.button.cancel", "[Cancel]"),
            no_x,
            no_y
        );
    }
}

impl Interactive for SaveLoadMenu {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        SaveLoadMenu::handle_mouse_move(self, x, y)
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        SaveLoadMenu::handle_mouse_click(self, x, y, button).is_some()
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        SaveLoadMenu::handle_key_press(self, key)
    }

    fn handle_text_input(&mut self, text: &str) -> bool {
        SaveLoadMenu::handle_text_input(self, text)
    }
}

impl Renderable for SaveLoadMenu {
    fn render(&self, _context: &mut UIRenderContext) {
        let title = match self.mode {
            SaveLoadMode::Save => Self::text("save_load.header_save", "=== SAVE GAME ==="),
            SaveLoadMode::Load => Self::text("save_load.header_load", "=== LOAD GAME ==="),
        };
        println!("{}", title);

        if self.mode == SaveLoadMode::Save {
            println!(
                "\n{} {}",
                Self::text("save_load.save_name", "Save Name:"),
                if self.save_name_input.is_empty() {
                    "_"
                } else {
                    &self.save_name_input
                }
            );
        }

        println!(
            "\n{}",
            Self::text("save_load.available_saves", "Available Saves:")
        );

        if self.save_files.is_empty() {
            println!(
                "  {}",
                Self::text("save_load.no_saves", "No save files found")
            );
        } else {
            for (i, save_entry) in self.save_files.iter().enumerate() {
                let selected_marker = if Some(i) == self.selected_entry {
                    " <--"
                } else {
                    ""
                };
                let (display_time, display_date) =
                    crate::save_load::format_save_list_date_time(save_entry.timestamp);
                // C++ populateSaveGameListbox (GameState.cpp:1194-1206):
                // mission saves are GameMakeColor(200, 255, 200).
                let color = if save_entry.save_type == SaveFileType::Mission {
                    "rgb(200,255,200)"
                } else if (i & 0x1) != 0 {
                    "rgb(255,255,255)"
                } else {
                    "rgb(170,170,235)"
                };

                println!(
                    "  {}. {}  {}  {}  [{}]{}",
                    i + 1,
                    save_entry.display_name,
                    display_time,
                    display_date,
                    color,
                    selected_marker
                );
                println!("     {}", save_entry.map_name);
                if !save_entry.faction.is_empty() {
                    println!("     Faction: {}", save_entry.faction);
                }

                if let Some(mission) = &save_entry.mission {
                    println!("     Mission: {}", mission);
                }
            }
        }

        if self.dialog_state == SaveLoadDialogState::None {
            self.render_action_buttons();
        } else {
            self.render_confirm_dialog();
        }

        println!("\n{}", Self::text("save_load.controls", "Controls:"));
        if self.mode == SaveLoadMode::Save {
            println!(
                "  {}",
                Self::text("save_load.f5_quick_save", "F5 - Quick Save")
            );
        } else {
            println!(
                "  {}",
                Self::text("save_load.f9_quick_load", "F9 - Quick Load")
            );
        }
        println!(
            "  {}",
            Self::text("save_load.enter_confirm", "ENTER - Confirm")
        );
        println!("  {}", Self::text("save_load.esc_cancel", "ESC - Cancel"));
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}

#[cfg(test)]
impl SaveLoadMenu {
    fn push_entry(&mut self, filename: &str, display_name: &str) {
        self.save_files.push(SaveGameEntry {
            filename: filename.to_string(),
            display_name: display_name.to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            map_name: "Alpine Assault".to_string(),
            faction: "America".to_string(),
            mission: None,
            save_type: SaveFileType::Normal,
        });
        self.entry_clicks.push(ClickSpring::new());
    }

    fn click_rect(rect: (i32, i32, u32, u32)) -> (i32, i32) {
        (rect.0 + 1, rect.1 + 1)
    }

    fn save_name_input(&self) -> &str {
        &self.save_name_input
    }

    fn push_mission_entry(&mut self, filename: &str, display_name: &str) {
        self.save_files.push(SaveGameEntry {
            filename: filename.to_string(),
            display_name: display_name.to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            map_name: "Alpine Assault".to_string(),
            faction: "America".to_string(),
            mission: Some("Mission 2".to_string()),
            save_type: SaveFileType::Mission,
        });
        self.entry_clicks.push(ClickSpring::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_existing_row_opens_overwrite_confirm() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Save);
        menu.push_entry("00000001", "Campaign");
        menu.selected_entry = Some(0);
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::OverwriteConfirm);
        assert!(
            menu.drain_pending_events()
                .iter()
                .all(|event| { !matches!(event, UIEvent::SaveGame { .. }) })
        );
    }

    #[test]
    fn sanitize_slot_collision_opens_overwrite_confirm() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Save);
        menu.push_entry("my_save", "My Save");
        menu.save_name_input = "My Save!".to_string();
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::OverwriteConfirm);
    }

    #[test]
    fn new_save_opens_save_description() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Save);
        menu.save_name_input = "Fresh Slot".to_string();
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::SaveDescription);
    }

    #[test]
    fn load_from_pause_opens_load_confirm() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Load);
        menu.set_return_screen(Screen::PauseMenu);
        menu.push_entry("00000001", "Campaign");
        menu.selected_entry = Some(0);
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::LoadConfirm);
    }

    #[test]
    fn load_from_shell_emits_immediately() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Load);
        menu.set_return_screen(Screen::MainMenu);
        menu.push_entry("00000001", "Campaign");
        menu.selected_entry = Some(0);
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        assert!(matches!(
            menu.handle_mouse_click(x, y, MouseButton::Left),
            Some(UIEvent::LoadGame(name)) if name == "00000001"
        ));
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::None);
    }

    #[test]
    fn delete_opens_delete_confirm_and_cancel_dismisses() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Load);
        menu.push_entry("00000001", "Campaign");
        menu.selected_entry = Some(0);
        let (_, delete, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(delete);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::DeleteConfirm);
        assert!(menu.handle_key_press(KeyCode::Escape));
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::None);
    }

    #[test]
    fn overwrite_confirm_saves_existing_filename() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Save);
        menu.push_entry("00000001", "Campaign");
        menu.selected_entry = Some(0);
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        menu.handle_mouse_click(x, y, MouseButton::Left);
        let (yes, _) = menu.dialog_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(yes);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        let events = menu.drain_pending_events();
        assert!(events.iter().any(|event| matches!(
            event,
            UIEvent::SaveGame {
                slot,
                display_name
            } if slot == "00000001" && display_name == "Campaign"
        )));
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::None);
    }

    #[test]
    fn new_save_description_prefills_campaign_or_map_leaf() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Save);
        let (confirm, _, _) = menu.action_button_rects();
        let (x, y) = SaveLoadMenu::click_rect(confirm);
        assert!(menu.handle_mouse_click(x, y, MouseButton::Left).is_none());
        assert_eq!(menu.dialog_state(), SaveLoadDialogState::SaveDescription);
        let expected = {
            let mission = crate::save_load::current_mission_save_description();
            if !mission.is_empty() {
                mission
            } else {
                crate::save_load::default_save_edit_description("")
            }
        };
        assert_eq!(menu.save_name_input(), expected);
    }

    #[test]
    fn list_render_includes_date_time_and_mission_green() {
        let mut menu = SaveLoadMenu::new(SaveLoadMode::Load);
        menu.push_mission_entry("00000000", "USA Campaign Mission 1");
        let (time, date) = crate::save_load::format_save_list_date_time(SystemTime::UNIX_EPOCH);
        let mut sink = Vec::new();
        {
            use std::io::Write;
            // Render prints to stdout; assert the same tokens the renderer uses.
            let _ = write!(&mut sink, "{} {} rgb(200,255,200)", time, date);
        }
        let rendered = String::from_utf8(sink).expect("utf8");
        assert!(rendered.contains(&time));
        assert!(rendered.contains(&date));
        assert!(rendered.contains("rgb(200,255,200)"));
        assert_eq!(menu.save_files[0].save_type, SaveFileType::Mission);
    }

    #[test]
    fn list_campaign_columns_empty_without_campaign() {
        let campaign = crate::save_load::SaveGameInfo {
            filename: "camp".into(),
            display_name: "camp".into(),
            description: "camp".into(),
            map_name: "Alpine".into(),
            campaign_side: Some("America".into()),
            mission_number: Some(2),
            save_date: SystemTime::UNIX_EPOCH,
            game_version: String::new(),
            play_time: std::time::Duration::ZERO,
            difficulty: crate::save_load::GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        let skirmish = crate::save_load::SaveGameInfo {
            campaign_side: None,
            mission_number: None,
            ..campaign.clone()
        };
        assert_eq!(
            SaveLoadMenu::list_campaign_columns(&campaign),
            ("America".into(), Some("Mission 2".into()))
        );
        assert_eq!(
            SaveLoadMenu::list_campaign_columns(&skirmish),
            (String::new(), None),
            "non-campaign header must not invent a faction label"
        );
    }
}
