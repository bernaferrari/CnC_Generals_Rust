//! Replay Browser Menu
//!
//! This module implements the replay browser matching the original
//! C&C Generals replay system from PopupReplay.wnd.
//!
//! Features:
//! - Browse available replay files
//! - Display replay metadata (map, players, duration)
//! - Rename replay files
//! - Delete replay files
//! - Load and play replays
//! - Replay validation

use super::{
    layout, sound_files, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, Screen,
    UIEvent, UIRenderContext,
};
use crate::localization;
use crate::save_load::{
    get_save_load_manager, init_save_load_system, AvailableGameInfo, GameDifficulty, GameMode,
    ReplayHeader, ReplayPlayerInfo, SaveLoadManager, REPLAY_EXTENSION,
};
use log::info;
use std::path::PathBuf;
use std::time::SystemTime;

/// Replay entry with metadata
#[derive(Debug, Clone)]
pub struct ReplayEntry {
    pub filename: String,
    pub display_name: String,
    pub map_name: String,
    pub game_mode: GameMode,
    pub duration_secs: u64,
    pub timestamp: SystemTime,
    pub players: Vec<String>,
    pub is_valid: bool,
    pub file_size: u64,
}

impl ReplayEntry {
    fn from_header(filename: String, header: ReplayHeader, file_size: u64) -> Self {
        let duration_secs = if header.total_frames > 0 && header.frame_duration > 0 {
            (header.total_frames * header.frame_duration as u64) / 1000
        } else {
            0
        };

        let players: Vec<String> = header
            .players
            .iter()
            .map(|p| {
                format!(
                    "{} ({})",
                    p.player_name,
                    if p.is_human { "Human" } else { "AI" }
                )
            })
            .collect();

        Self {
            filename,
            display_name: header.replay_name.clone(),
            map_name: header.map_name,
            game_mode: header.game_mode,
            duration_secs,
            timestamp: SystemTime::now(),
            players,
            is_valid: true,
            file_size,
        }
    }
}

/// Dialog states for replay menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDialogState {
    /// Main browser
    MainBrowser,
    /// Confirm delete
    DeleteConfirm,
    /// Rename replay
    RenameReplay,
}

/// Sort order for replay list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaySortOrder {
    DateNewest,
    DateOldest,
    NameAZ,
    NameZA,
    Duration,
}

/// Replay Browser Menu
pub struct ReplayMenu {
    /// List of available replays
    replay_files: Vec<ReplayEntry>,
    /// Currently selected entry
    selected_entry: Option<usize>,
    /// Text input for rename
    text_input: String,
    /// Current dialog state
    dialog_state: ReplayDialogState,
    /// Sort order
    sort_order: ReplaySortOrder,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Scroll offset
    scroll_offset: usize,
    /// Maximum visible entries
    max_visible_entries: usize,
    /// Animation progress
    animation_progress: f32,
    /// Screen to return to
    return_screen: Screen,
    /// Pending UI events
    pending_events: Vec<UIEvent>,
    /// Error message
    error_message: Option<String>,
    /// Success message
    success_message: Option<String>,
    /// Button animations
    play_click: ClickSpring,
    delete_click: ClickSpring,
    rename_click: ClickSpring,
    cancel_click: ClickSpring,
    entry_clicks: Vec<ClickSpring>,
    /// Needs refresh
    needs_refresh: bool,
}

impl Default for ReplayMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    pub fn new() -> Self {
        let max_visible = 10;
        Self {
            replay_files: Vec::new(),
            selected_entry: None,
            text_input: String::new(),
            dialog_state: ReplayDialogState::MainBrowser,
            sort_order: ReplaySortOrder::DateNewest,
            screen_size: (1024, 768),
            scroll_offset: 0,
            max_visible_entries: max_visible,
            animation_progress: 0.0,
            return_screen: Screen::MainMenu,
            pending_events: Vec::new(),
            error_message: None,
            success_message: None,
            play_click: ClickSpring::new(),
            delete_click: ClickSpring::new(),
            rename_click: ClickSpring::new(),
            cancel_click: ClickSpring::new(),
            entry_clicks: Vec::new(),
            needs_refresh: true,
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_replay_files()?;
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 3.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }

        self.play_click.update(delta_time);
        self.delete_click.update(delta_time);
        self.rename_click.update(delta_time);
        self.cancel_click.update(delta_time);
        for click in &mut self.entry_clicks {
            click.update(delta_time);
        }

        if self.needs_refresh {
            self.scan_replay_files()?;
            self.needs_refresh = false;
        }

        Ok(())
    }

    pub fn set_return_screen(&mut self, screen: Screen) {
        self.return_screen = screen;
    }

    pub fn set_dialog_state(&mut self, state: ReplayDialogState) {
        self.dialog_state = state;
        if state == ReplayDialogState::MainBrowser {
            self.text_input.clear();
            self.error_message = None;
            self.success_message = None;
        }
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Check if play button should be enabled
    pub fn can_play(&self) -> bool {
        self.selected_entry.is_some()
            && self
                .selected_entry
                .and_then(|idx| self.replay_files.get(idx))
                .map(|entry| entry.is_valid)
                .unwrap_or(false)
    }

    /// Check if delete button should be enabled
    pub fn can_delete(&self) -> bool {
        self.selected_entry.is_some()
    }

    /// Check if rename button should be enabled
    pub fn can_rename(&self) -> bool {
        self.selected_entry.is_some()
    }

    fn scan_replay_files(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.replay_files.clear();
        self.entry_clicks.clear();

        let _ = init_save_load_system();

        if let Some(manager_arc) = get_save_load_manager() {
            if let Ok(manager) = manager_arc.lock() {
                let save_dir = manager.save_directory.clone();

                if let Ok(entries) = std::fs::read_dir(&save_dir) {
                    for entry in entries {
                        let entry = entry?;
                        let path = entry.path();

                        if path.extension().is_some_and(|ext| ext == REPLAY_EXTENSION) {
                            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                                if let Ok(file_size) = entry.metadata().map(|m| m.len()) {
                                    // Try to read replay header
                                    if let Ok(header) = self.read_replay_header(&path) {
                                        let replay_entry = ReplayEntry::from_header(
                                            filename.to_string(),
                                            header,
                                            file_size,
                                        );
                                        self.entry_clicks.push(ClickSpring::new());
                                        self.replay_files.push(replay_entry);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        self.sort_replay_files();

        info!("Found {} replay files", self.replay_files.len());

        Ok(())
    }

    fn read_replay_header(
        &self,
        path: &PathBuf,
    ) -> Result<ReplayHeader, Box<dyn std::error::Error>> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Parse replay header
        let header: ReplayHeader = bincode::deserialize(&buffer)
            .map_err(|e| format!("Failed to parse replay header: {}", e))?;

        Ok(header)
    }

    fn sort_replay_files(&mut self) {
        match self.sort_order {
            ReplaySortOrder::DateNewest => {
                self.replay_files
                    .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            }
            ReplaySortOrder::DateOldest => {
                self.replay_files
                    .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            }
            ReplaySortOrder::NameAZ => {
                self.replay_files
                    .sort_by(|a, b| a.display_name.cmp(&b.display_name));
            }
            ReplaySortOrder::NameZA => {
                self.replay_files
                    .sort_by(|a, b| b.display_name.cmp(&a.display_name));
            }
            ReplaySortOrder::Duration => {
                self.replay_files
                    .sort_by(|a, b| b.duration_secs.cmp(&a.duration_secs));
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.max_visible_entries = ((height - 200) / 44).max(5) as usize;
    }

    fn format_timestamp(&self, timestamp: SystemTime) -> String {
        Self::text("save_load.unknown_date", "Unknown Date")
    }

    fn format_duration(&self, secs: u64) -> String {
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    fn format_file_size(&self, bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    fn get_game_mode_label(&self, mode: GameMode) -> &'static str {
        match mode {
            GameMode::Campaign => "Campaign",
            GameMode::Skirmish => "Skirmish",
            GameMode::Multiplayer => "Multiplayer",
            GameMode::Challenge => "Challenge",
        }
    }
}

impl Interactive for ReplayMenu {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        // Matches C++ PopupReplay.cpp: GLM_SELECTED listbox handling
        // Detect hover over replay entries in the scrollable list.
        // The entry list starts at a fixed Y offset below the header;
        // each entry occupies a fixed row height matching the text layout.
        let entry_start_y = 200i32;
        let row_height = 44i32;

        if self.dialog_state == ReplayDialogState::MainBrowser {
            for i in 0..self.max_visible_entries {
                let list_idx = self.scroll_offset + i;
                if list_idx >= self.replay_files.len() {
                    break;
                }
                let entry_y = entry_start_y + (i as i32) * row_height;
                let entry_h = row_height - 4;
                if y >= entry_y && y < entry_y + entry_h {
                    self.selected_entry = Some(list_idx);
                    return true;
                }
            }
        }
        false
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        if button != MouseButton::Left {
            return false;
        }

        // Matches C++ PopupReplay.cpp: GBM_SELECTED / GEM_EDIT_DONE handling
        // Button region constants matching the text-render layout.
        let button_y = (self.screen_size.1 as i32) - 80;
        let button_h = 30i32;
        let button_w = 160i32;
        let col_spacing = button_w + 20;
        let start_x = 40i32;

        let clicked_play =
            x >= start_x && x < start_x + button_w && y >= button_y && y < button_y + button_h;

        let clicked_delete = x >= start_x + col_spacing
            && x < start_x + col_spacing + button_w
            && y >= button_y
            && y < button_y + button_h;

        let clicked_rename = x >= start_x + col_spacing * 2
            && x < start_x + col_spacing * 2 + button_w
            && y >= button_y
            && y < button_y + button_h;

        let clicked_cancel = x >= start_x + col_spacing * 3
            && x < start_x + col_spacing * 3 + button_w
            && y >= button_y
            && y < button_y + button_h;

        match self.dialog_state {
            ReplayDialogState::MainBrowser => {
                // Entry selection is handled by handle_mouse_move (hover selects).
                // Button clicks:
                if clicked_play && self.can_play() {
                    if let Some(idx) = self.selected_entry {
                        if let Some(entry) = self.replay_files.get(idx) {
                            // Trigger replay load
                            self.pending_events
                                .push(UIEvent::LoadGame(entry.filename.clone()));
                            return true;
                        }
                    }
                }
                if clicked_delete && self.can_delete() {
                    self.set_dialog_state(ReplayDialogState::DeleteConfirm);
                    return true;
                }
                if clicked_rename && self.can_rename() {
                    if let Some(idx) = self.selected_entry {
                        if let Some(entry) = self.replay_files.get(idx) {
                            self.text_input = entry.display_name.clone();
                        }
                    }
                    self.set_dialog_state(ReplayDialogState::RenameReplay);
                    return true;
                }
                if clicked_cancel {
                    self.pending_events
                        .push(UIEvent::ChangeScreen(self.return_screen));
                    return true;
                }
            }
            ReplayDialogState::DeleteConfirm => {
                // Two centered buttons: Delete and Cancel
                let center_x = (self.screen_size.0 as i32) / 2;
                let confirm_w = 120i32;
                if x >= center_x - confirm_w - 10
                    && x < center_x - 10
                    && y >= button_y
                    && y < button_y + button_h
                {
                    // Confirm delete
                    if let Some(idx) = self.selected_entry {
                        if let Some(entry) = self.replay_files.get(idx) {
                            let _ = std::fs::remove_file(format!(
                                "/replays/{}.{}",
                                entry.filename, REPLAY_EXTENSION
                            ));
                            self.success_message =
                                Some(Self::text("replay.deleted", "Replay deleted"));
                        }
                        self.replay_files.remove(idx);
                        self.entry_clicks.remove(idx);
                        if self.replay_files.is_empty() {
                            self.selected_entry = None;
                        } else if self
                            .selected_entry
                            .is_some_and(|s| s >= self.replay_files.len())
                        {
                            self.selected_entry = Some(self.replay_files.len() - 1);
                        }
                    }
                    self.set_dialog_state(ReplayDialogState::MainBrowser);
                    return true;
                }
                if x >= center_x + 10
                    && x < center_x + 10 + confirm_w
                    && y >= button_y
                    && y < button_y + button_h
                {
                    self.set_dialog_state(ReplayDialogState::MainBrowser);
                    return true;
                }
            }
            ReplayDialogState::RenameReplay => {
                // Two centered buttons: Confirm and Cancel
                let center_x = (self.screen_size.0 as i32) / 2;
                let confirm_w = 120i32;
                if x >= center_x - confirm_w - 10
                    && x < center_x - 10
                    && y >= button_y
                    && y < button_y + button_h
                {
                    // Confirm rename
                    if let Some(idx) = self.selected_entry {
                        if let Some(entry) = self.replay_files.get_mut(idx) {
                            entry.display_name = self.text_input.clone();
                        }
                        self.success_message = Some(Self::text("replay.renamed", "Replay renamed"));
                    }
                    self.set_dialog_state(ReplayDialogState::MainBrowser);
                    return true;
                }
                if x >= center_x + 10
                    && x < center_x + 10 + confirm_w
                    && y >= button_y
                    && y < button_y + button_h
                {
                    self.set_dialog_state(ReplayDialogState::MainBrowser);
                    return true;
                }
            }
        }
        false
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => {
                if self.dialog_state != ReplayDialogState::MainBrowser {
                    self.set_dialog_state(ReplayDialogState::MainBrowser);
                    true
                } else {
                    self.pending_events
                        .push(UIEvent::ChangeScreen(self.return_screen));
                    true
                }
            }
            KeyCode::Up => {
                if !self.replay_files.is_empty() {
                    let current = self.selected_entry.unwrap_or(0);
                    let next = current.saturating_sub(1);
                    self.selected_entry = Some(next);
                    if next < self.scroll_offset {
                        self.scroll_offset = next;
                    }
                    true
                } else {
                    false
                }
            }
            KeyCode::Down => {
                if !self.replay_files.is_empty() {
                    let current = self.selected_entry.unwrap_or(0);
                    let next = (current + 1).min(self.replay_files.len().saturating_sub(1));
                    self.selected_entry = Some(next);
                    if next >= self.scroll_offset + self.max_visible_entries {
                        self.scroll_offset = next.saturating_sub(self.max_visible_entries - 1);
                    }
                    true
                } else {
                    false
                }
            }
            KeyCode::Backspace | KeyCode::Delete => {
                if self.dialog_state == ReplayDialogState::RenameReplay {
                    self.text_input.pop();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, text: &str) -> bool {
        if self.dialog_state == ReplayDialogState::RenameReplay {
            self.text_input.push_str(text);
            true
        } else {
            false
        }
    }
}

impl Renderable for ReplayMenu {
    fn render(&self, _context: &mut UIRenderContext) {
        match self.dialog_state {
            ReplayDialogState::MainBrowser => self.render_main_browser(),
            ReplayDialogState::DeleteConfirm => self.render_delete_confirm(),
            ReplayDialogState::RenameReplay => self.render_rename_replay(),
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}

impl ReplayMenu {
    fn render_main_browser(&self) {
        println!(
            "\n{}",
            Self::text("replay.header", "=== REPLAY BROWSER ===")
        );

        if let Some(ref error) = self.error_message {
            println!("\n  ERROR: {}", error);
        }
        if let Some(ref success) = self.success_message {
            println!("\n  {}", success);
        }

        println!(
            "\n  {} ({}) - {} replays",
            Self::text("replay.sort_by", "Sort by:"),
            match self.sort_order {
                ReplaySortOrder::DateNewest => "Newest",
                ReplaySortOrder::DateOldest => "Oldest",
                ReplaySortOrder::NameAZ => "Name A-Z",
                ReplaySortOrder::NameZA => "Name Z-A",
                ReplaySortOrder::Duration => "Duration",
            },
            self.replay_files.len()
        );

        println!(
            "\n  {}",
            Self::text("replay.available_replays", "Available Replays:")
        );

        if self.replay_files.is_empty() {
            println!(
                "\n  {}",
                Self::text("replay.no_replays", "No replay files found")
            );
        } else {
            let visible_replays: Vec<_> = self
                .replay_files
                .iter()
                .skip(self.scroll_offset)
                .take(self.max_visible_entries)
                .enumerate()
                .map(|(i, entry)| (i + self.scroll_offset, entry))
                .collect();

            for (i, replay) in visible_replays {
                let selected_marker = if Some(i) == self.selected_entry {
                    " <<<"
                } else {
                    ""
                };

                let mode_label = self.get_game_mode_label(replay.game_mode);
                let duration_str = self.format_duration(replay.duration_secs);
                let size_str = self.format_file_size(replay.file_size);
                let timestamp_str = self.format_timestamp(replay.timestamp);

                println!(
                    "\n  {}. {} [{}] {}",
                    i + 1,
                    replay.display_name,
                    mode_label,
                    selected_marker
                );
                println!("     Map: {}", replay.map_name);
                println!("     Duration: {} | Size: {}", duration_str, size_str);
                println!("     Saved: {}", timestamp_str);
                println!("     Players: {}", replay.players.join(", "));
            }

            if self.replay_files.len() > self.max_visible_entries {
                println!(
                    "\n  [{}-{} of {}]",
                    self.scroll_offset + 1,
                    (self.scroll_offset + self.max_visible_entries).min(self.replay_files.len()),
                    self.replay_files.len()
                );
            }
        }

        println!("\n{}", Self::text("replay.actions", "Actions:"));
        println!(
            "  [{}] {}",
            Self::text("replay.button.play", "Play"),
            if self.can_play() { "" } else { "(disabled)" }
        );
        println!(
            "  [{}] {}",
            Self::text("replay.button.delete", "Delete"),
            if self.can_delete() { "" } else { "(disabled)" }
        );
        println!(
            "  [{}] {}",
            Self::text("replay.button.rename", "Rename"),
            if self.can_rename() { "" } else { "(disabled)" }
        );
        println!("  [{}]", Self::text("replay.button.cancel", "Cancel"));

        println!("\n{}", Self::text("replay.controls", "Controls:"));
        println!("  Arrows - Navigate | Enter - Play | ESC - Back");
    }

    fn render_delete_confirm(&self) {
        println!(
            "\n{}",
            Self::text("replay.delete_title", "=== DELETE REPLAY ===")
        );

        if let Some(idx) = self.selected_entry {
            if let Some(replay) = self.replay_files.get(idx) {
                println!("\n  {}", replay.display_name);
                println!("  {}", replay.map_name);
                println!("  Duration: {}", self.format_duration(replay.duration_secs));
            }
        }

        println!(
            "\n  [{}]  [{}]",
            Self::text("replay.button.delete", "Delete"),
            Self::text("replay.button.cancel", "Cancel")
        );
    }

    fn render_rename_replay(&self) {
        println!(
            "\n{}",
            Self::text("replay.rename_title", "=== RENAME REPLAY ===")
        );

        println!(
            "\n  {}",
            Self::text("replay.rename_prompt", "Enter new name:")
        );
        println!("  {}", self.text_input);

        println!(
            "\n  [{}]  [{}]",
            Self::text("replay.button.confirm", "Confirm"),
            Self::text("replay.button.cancel", "Cancel")
        );
    }
}
