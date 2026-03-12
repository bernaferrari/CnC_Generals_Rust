//! Save/Load Game Browser
//!
//! This module implements the save game browser matching the original
//! C&C Generals save/load system from PopupSaveLoad.wnd.

use super::{
    layout, sound_files, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, Screen,
    UIEvent, UIRenderContext,
};
use crate::localization;
use crate::save_load::{get_save_load_manager, init_save_load_system, SaveLoadManager};
use log::info;
use std::time::SystemTime;

/// Mode for save/load menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLoadMode {
    Save,
    Load,
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
            confirm_click: ClickSpring::new(),
            cancel_click: ClickSpring::new(),
            entry_clicks: Vec::new(),
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
        for click in &mut self.entry_clicks {
            click.update(delta_time);
        }
        Ok(())
    }

    pub fn set_mode(&mut self, mode: SaveLoadMode) {
        self.mode = mode;
        self.selected_entry = None;
        self.save_name_input.clear();
    }

    pub fn set_return_screen(&mut self, screen: Screen) {
        self.return_screen = screen;
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let (confirm_rect, cancel_rect) = self.action_button_rects();
        let is_over_button =
            utils::point_in_rect((x, y), confirm_rect) || utils::point_in_rect((x, y), cancel_rect);
        let is_over_entry = self
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

        let (confirm_rect, cancel_rect) = self.action_button_rects();
        if utils::point_in_rect((x, y), cancel_rect) {
            self.cancel_click.trigger();
            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
            return Some(UIEvent::ChangeScreen(self.return_screen));
        }

        if utils::point_in_rect((x, y), confirm_rect) {
            self.confirm_click.trigger();
            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
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
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                return None;
            }
        }

        None
    }

    pub fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => {
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                self.pending_events
                    .push(UIEvent::ChangeScreen(self.return_screen));
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
                if self.save_files.is_empty() {
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
                if self.save_files.is_empty() {
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
                if self.mode != SaveLoadMode::Save || self.save_name_input.is_empty() {
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
        if self.mode == SaveLoadMode::Save {
            self.save_name_input.push_str(text);
            true
        } else {
            false
        }
    }

    fn confirm_selection(&mut self) -> Option<UIEvent> {
        match self.mode {
            SaveLoadMode::Load => {
                let index = self.selected_entry?;
                let entry = self.save_files.get(index)?;
                Some(UIEvent::LoadGame(entry.filename.clone()))
            }
            SaveLoadMode::Save => {
                let display_name = self.save_name_input.trim();
                let chosen_name = if !display_name.is_empty() {
                    display_name.to_string()
                } else if let Some(index) = self.selected_entry {
                    self.save_files
                        .get(index)
                        .map(|entry| entry.display_name.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                if chosen_name.trim().is_empty() {
                    return None;
                }

                let slot = Self::sanitize_slot_name(&chosen_name);
                self.pending_events.push(UIEvent::SaveGame {
                    slot,
                    display_name: chosen_name,
                });
                self.pending_events
                    .push(UIEvent::ChangeScreen(self.return_screen));
                None
            }
        }
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

    fn action_button_rects(&self) -> ((i32, i32, u32, u32), (i32, i32, u32, u32)) {
        let button_w = layout::MENU_BUTTON_WIDTH;
        let button_h = layout::MENU_BUTTON_HEIGHT;
        let total_w = button_w as i32 * 2 + layout::MENU_SPACING as i32;
        let x0 = (self.screen_size.0 as i32 / 2) - total_w / 2;
        let y0 = self.screen_size.1 as i32 - button_h as i32 - 40;
        let confirm = (x0, y0, button_w, button_h);
        let cancel = (
            x0 + button_w as i32 + layout::MENU_SPACING as i32,
            y0,
            button_w,
            button_h,
        );
        (confirm, cancel)
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
                display_name: save.display_name.clone(),
                timestamp: save.save_date,
                map_name: save.map_name.clone(),
                faction: save
                    .campaign_side
                    .clone()
                    .unwrap_or_else(|| "Skirmish".to_string()),
                mission: save.mission_number.map(|n| format!("Mission {}", n)),
            });
            self.entry_clicks.push(ClickSpring::new());
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
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

                println!(
                    "  {}. {}{}",
                    i + 1,
                    save_entry.display_name,
                    selected_marker
                );
                println!("     {}", save_entry.map_name);
                println!("     Faction: {}", save_entry.faction);

                if let Some(mission) = &save_entry.mission {
                    println!("     Mission: {}", mission);
                }
            }
        }

        let (confirm_rect, cancel_rect) = self.action_button_rects();
        let (confirm_x, confirm_y, _, _) =
            utils::scale_rect_center(confirm_rect, self.confirm_click.scale());
        let (cancel_x, cancel_y, _, _) =
            utils::scale_rect_center(cancel_rect, self.cancel_click.scale());
        let confirm_x_value = format!("{:.1}", confirm_x);
        let confirm_y_value = format!("{:.1}", confirm_y);
        let cancel_x_value = format!("{:.1}", cancel_x);
        let cancel_y_value = format!("{:.1}", cancel_y);
        let confirm_label = match self.mode {
            SaveLoadMode::Save => Self::text("save_load.button_save", "Save"),
            SaveLoadMode::Load => Self::text("save_load.button_load", "Load"),
        };
        println!(
            "\n{} @ ({},{})",
            localization::localize_with_args(
                "save_load.button.confirm",
                "[{label}]",
                &[("label", confirm_label.as_str())],
            ),
            confirm_x_value,
            confirm_y_value
        );
        println!(
            "{} @ ({},{})",
            Self::text("save_load.button.cancel", "[Cancel]"),
            cancel_x_value,
            cancel_y_value
        );

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
