// FILE: replay_menu.rs
// Author: Chris Huybregts, December 2001 (original C++), Rust port
// Description: Replay Menus - Browse and playback game replays
//
// Ported from: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ReplayMenu.cpp

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game_text::GameText;
use crate::gui::window_manager::{with_window_manager, with_window_manager_ref};
use game_engine::common::recorder::{self, ReplayHeader as CommonReplayHeader};

/// Maximum number of player slots in a game
pub const MAX_SLOTS: usize = 8;

/// Slot state enumeration - matches C++ SlotState
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Open,
    Closed,
    EasyAI,
    MedAI,
    BrutalAI,
    Player,
}

/// Game slot information - simplified version for replay parsing
///
/// Matches C++ GameSlot structure from GameInfo.h:36-125
#[derive(Debug, Clone)]
pub struct GameSlot {
    state: SlotState,
    name: String,
    color: i32,
    start_pos: i32,
    player_template: i32,
    team_number: i32,
    ip: u32,
    port: u16,
}

impl GameSlot {
    pub fn new() -> Self {
        GameSlot {
            state: SlotState::Open,
            name: String::new(),
            color: -1,
            start_pos: -1,
            player_template: 0,
            team_number: -1,
            ip: 0,
            port: 0,
        }
    }

    pub fn get_state(&self) -> SlotState {
        self.state
    }

    pub fn set_state(&mut self, state: SlotState) {
        self.state = state;
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn is_human(&self) -> bool {
        matches!(self.state, SlotState::Player)
    }

    pub fn is_occupied(&self) -> bool {
        !matches!(self.state, SlotState::Open | SlotState::Closed)
    }
}

impl Default for GameSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Color for UI rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    pub fn white() -> Self {
        Color::new(255, 255, 255, 255)
    }

    pub fn gray() -> Self {
        Color::new(128, 128, 128, 255)
    }
}

/// Represents a system time value
#[derive(Debug, Clone)]
pub struct SystemTimeValue {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

impl SystemTimeValue {
    pub fn now() -> Self {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap();
        let secs = duration.as_secs();

        // Simple conversion - in real implementation would use proper date/time library
        SystemTimeValue {
            year: 2001,
            month: 1,
            day: 1,
            hour: ((secs / 3600) % 24) as u16,
            minute: ((secs / 60) % 60) as u16,
            second: (secs % 60) as u16,
        }
    }

    pub fn to_display_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

fn system_time_to_value(time: SystemTime) -> SystemTimeValue {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    SystemTimeValue {
        year: 2001,
        month: 1,
        day: 1,
        hour: ((secs / 3600) % 24) as u16,
        minute: ((secs / 60) % 60) as u16,
        second: (secs % 60) as u16,
    }
}

fn map_common_header(header: CommonReplayHeader) -> ReplayHeader {
    ReplayHeader {
        filename: header.filename,
        for_playback: header.for_playback,
        replay_name: header.replay_name,
        time_val: system_time_to_value(header.time_val),
        version_string: header.version_string,
        version_time_string: header.version_time_string,
        version_number: header.version_number,
        exe_crc: header.exe_crc,
        ini_crc: header.ini_crc,
        start_time: header.start_time,
        end_time: header.end_time,
        frame_duration: header.frame_duration,
        quit_early: header.quit_early,
        desync_game: header.desync_game,
        player_discons: header.player_discons,
        game_options: header.game_options,
        local_player_index: header.local_player_index,
    }
}

/// Replay game information - matches C++ ReplayGameInfo
///
/// Matches C++ Recorder.h:10-21
#[derive(Debug, Clone)]
pub struct ReplayGameInfo {
    replay_slots: [GameSlot; MAX_SLOTS],
    map_name: String,
    game_mode: i32,
    difficulty: i32,
}

impl ReplayGameInfo {
    pub fn new() -> Self {
        ReplayGameInfo {
            replay_slots: [
                GameSlot::new(),
                GameSlot::new(),
                GameSlot::new(),
                GameSlot::new(),
                GameSlot::new(),
                GameSlot::new(),
                GameSlot::new(),
                GameSlot::new(),
            ],
            map_name: String::new(),
            game_mode: 0,
            difficulty: 0,
        }
    }

    pub fn get_slot(&self, index: usize) -> Option<&GameSlot> {
        if index < MAX_SLOTS {
            Some(&self.replay_slots[index])
        } else {
            None
        }
    }

    pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut GameSlot> {
        if index < MAX_SLOTS {
            Some(&mut self.replay_slots[index])
        } else {
            None
        }
    }

    pub fn set_slot(&mut self, index: usize, slot: GameSlot) {
        if index < MAX_SLOTS {
            self.replay_slots[index] = slot;
        }
    }

    pub fn get_map(&self) -> &str {
        &self.map_name
    }

    pub fn set_map(&mut self, map: String) {
        self.map_name = map;
    }
}

impl Default for ReplayGameInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Replay header information - matches C++ RecorderClass::ReplayHeader
///
/// Matches C++ Recorder.h:61-80
#[derive(Debug, Clone)]
pub struct ReplayHeader {
    pub filename: String,
    pub for_playback: bool,
    pub replay_name: String,
    pub time_val: SystemTimeValue,
    pub version_string: String,
    pub version_time_string: String,
    pub version_number: u32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub start_time: u64,
    pub end_time: u64,
    pub frame_duration: u32,
    pub quit_early: bool,
    pub desync_game: bool,
    pub player_discons: [bool; MAX_SLOTS],
    pub game_options: String,
    pub local_player_index: i32,
}

impl ReplayHeader {
    pub fn new() -> Self {
        ReplayHeader {
            filename: String::new(),
            for_playback: false,
            replay_name: String::new(),
            time_val: SystemTimeValue::now(),
            version_string: String::new(),
            version_time_string: String::new(),
            version_number: 0,
            exe_crc: 0,
            ini_crc: 0,
            start_time: 0,
            end_time: 0,
            frame_duration: 0,
            quit_early: false,
            desync_game: false,
            player_discons: [false; MAX_SLOTS],
            game_options: String::new(),
            local_player_index: -1,
        }
    }

    /// Check if replay matches current version
    ///
    /// Matches C++ ReplayMenu.cpp:203-231
    pub fn is_version_compatible(
        &self,
        current_version: &str,
        current_version_num: u32,
        current_exe_crc: u32,
        current_ini_crc: u32,
    ) -> bool {
        self.version_string == current_version
            && self.version_number == current_version_num
            && self.exe_crc == current_exe_crc
            && self.ini_crc == current_ini_crc
    }

    /// Determine replay color based on type and version compatibility
    ///
    /// Matches C++ ReplayMenu.cpp:202-231
    pub fn get_display_color(
        &self,
        current_version: &str,
        current_version_num: u32,
        current_exe_crc: u32,
        current_ini_crc: u32,
    ) -> Color {
        if self.is_version_compatible(
            current_version,
            current_version_num,
            current_exe_crc,
            current_ini_crc,
        ) {
            // Good version
            if self.local_player_index >= 0 {
                // Multiplayer
                Color::white()
            } else {
                // Single player
                Color::white()
            }
        } else {
            // Version mismatch
            if self.local_player_index >= 0 {
                // Multiplayer with mismatch
                Color::gray()
            } else {
                // Single player with mismatch
                Color::gray()
            }
        }
    }

    /// Get replay duration in seconds
    pub fn get_duration_seconds(&self) -> u64 {
        if self.end_time > self.start_time {
            self.end_time - self.start_time
        } else {
            0
        }
    }

    /// Format duration as MM:SS
    pub fn format_duration(&self) -> String {
        let total_secs = self.get_duration_seconds();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{}:{:02}", mins, secs)
    }

    /// Calculate average FPS
    pub fn calculate_fps(&self) -> f64 {
        let duration = self.get_duration_seconds();
        if duration > 0 {
            self.frame_duration as f64 / duration as f64
        } else {
            0.0
        }
    }
}

impl Default for ReplayHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Listbox entry for replay files
#[derive(Debug, Clone)]
pub struct ReplayListEntry {
    pub name: String,
    pub date: String,
    pub version: String,
    pub map: String,
    pub color: Color,
    pub filename: String,
}

impl ReplayListEntry {
    pub fn new(
        name: String,
        date: String,
        version: String,
        map: String,
        color: Color,
        filename: String,
    ) -> Self {
        ReplayListEntry {
            name,
            date,
            version,
            map,
            color,
            filename,
        }
    }
}

/// Replay menu state management
pub struct ReplayMenu {
    // Window IDs
    parent_id: u32,
    button_load_id: u32,
    button_back_id: u32,
    button_delete_id: u32,
    button_copy_id: u32,
    listbox_id: u32,

    // State
    is_shutting_down: bool,
    just_entered: bool,
    initial_gadget_delay: i32,
    call_copy: bool,
    call_delete: bool,

    // Data
    replay_list: Vec<ReplayListEntry>,
    selected_index: i32,
    replay_directory: PathBuf,
    replay_extension: String,
    last_replay_filename: String,

    // Version info for compatibility checking
    current_version: String,
    current_version_number: u32,
    current_exe_crc: u32,
    current_ini_crc: u32,
}

impl ReplayMenu {
    pub fn new(replay_dir: PathBuf, replay_ext: String) -> Self {
        ReplayMenu {
            parent_id: 0,
            button_load_id: 0,
            button_back_id: 0,
            button_delete_id: 0,
            button_copy_id: 0,
            listbox_id: 0,
            is_shutting_down: false,
            just_entered: false,
            initial_gadget_delay: 2,
            call_copy: false,
            call_delete: false,
            replay_list: Vec::new(),
            selected_index: -1,
            replay_directory: replay_dir,
            replay_extension: replay_ext,
            last_replay_filename: "LastReplay".to_string(),
            current_version: "1.0".to_string(),
            current_version_number: 100,
            current_exe_crc: 0,
            current_ini_crc: 0,
        }
    }

    /// Initialize the replay menu
    ///
    /// Matches C++ ReplayMenu.cpp:247-295
    pub fn init(&mut self) {
        self.is_shutting_down = false;
        self.just_entered = true;
        self.initial_gadget_delay = 2;
        self.populate_replay_listbox();
    }

    /// Shutdown the replay menu
    ///
    /// Matches C++ ReplayMenu.cpp:300-316
    pub fn shutdown(&mut self, pop_immediate: bool) {
        if pop_immediate {
            // Immediate shutdown
            return;
        }

        // Reverse transition animation
        self.is_shutting_down = true;
        self.transition_reverse("ReplayMenuFade");
    }

    /// Update the replay menu
    ///
    /// Matches C++ ReplayMenu.cpp:321-343
    pub fn update(&mut self, _delta_time: f32) {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.transition_set_group("ReplayMenuFade", false);
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay -= 1;
            }
        }

        if self.call_copy {
            self.copy_replay();
        }

        if self.call_delete {
            self.delete_replay();
        }

        // Check if shutdown animation is complete
        if self.is_shutting_down {
            // In actual implementation, check if animation is finished
            if self.transitions_finished() {
                self.is_shutting_down = false;
            }
        }
    }

    fn transition_set_group(&self, group: &str, immediate: bool) {
        with_window_manager(|manager| manager.transition_set_group(group, immediate));
    }

    fn transition_reverse(&self, group: &str) {
        with_window_manager(|manager| manager.transition_reverse(group));
    }

    fn transitions_finished(&self) -> bool {
        with_window_manager_ref(|manager| manager.transitions_finished())
    }

    /// Get replay filename from listbox index
    ///
    /// Matches C++ ReplayMenu.cpp:65-79
    pub fn get_replay_filename_from_listbox(&self, index: i32) -> String {
        if index < 0 || index >= self.replay_list.len() as i32 {
            return String::new();
        }

        let entry = &self.replay_list[index as usize];
        entry.filename.clone()
    }

    fn replay_display_name_from_filename(filename: &str, replay_extension: &str) -> String {
        filename
            .strip_suffix(replay_extension)
            .unwrap_or(filename)
            .to_string()
    }

    /// Populate the listbox with replay files
    ///
    /// Matches C++ ReplayMenu.cpp:85-242
    pub fn populate_replay_listbox(&mut self) {
        self.replay_list.clear();

        // Scan replay directory for files
        let search_pattern = format!("*{}", self.replay_extension);
        let replay_files = self.scan_replay_directory(&search_pattern);

        for filepath in replay_files {
            // Read replay header
            if let Some(header) = self.read_replay_header(&filepath) {
                // Parse game info from header
                if let Some(game_info) = self.parse_game_info(&header.game_options) {
                    // Extract filename without path
                    let filename = filepath
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    // C++ overwrites header.replayName with the directory filename before
                    // stripping the replay extension for display.
                    let mut display_name =
                        Self::replay_display_name_from_filename(&filename, &self.replay_extension);

                    // Check if this is the last replay
                    let last_replay_full =
                        format!("{}{}", self.last_replay_filename, self.replay_extension);
                    if filename == last_replay_full {
                        display_name = GameText::fetch("GUI:LastReplay");
                    }

                    // Format date/time
                    let date_str = header.time_val.to_display_string();

                    // Get map display name
                    let map_str = game_info.get_map().to_string();

                    // Determine color based on version compatibility
                    let color = header.get_display_color(
                        &self.current_version,
                        self.current_version_number,
                        self.current_exe_crc,
                        self.current_ini_crc,
                    );

                    // Create listbox entry
                    let entry = ReplayListEntry::new(
                        display_name,
                        date_str,
                        header.version_string.clone(),
                        map_str,
                        color,
                        filename,
                    );

                    self.replay_list.push(entry);
                }
            }
        }

        // Select first entry by default
        if !self.replay_list.is_empty() {
            self.selected_index = 0;
        }
    }

    /// Scan replay directory for files matching pattern
    fn scan_replay_directory(&self, _pattern: &str) -> Vec<PathBuf> {
        let mut files = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.replay_directory) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let path = entry.path();
                        if let Some(ext) = path.extension() {
                            if ext == &self.replay_extension[1..] {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }

        files
    }

    /// Read replay header from file
    fn read_replay_header(&self, filepath: &Path) -> Option<ReplayHeader> {
        let filename = filepath.file_name()?.to_string_lossy().to_string();
        recorder::init_recorder();
        match recorder::with_recorder_mut(|rec| rec.load_replay_header(filename)) {
            Some(Ok(header)) => Some(map_common_header(header)),
            _ => None,
        }
    }

    /// Parse game info from options string
    fn parse_game_info(&self, options: &str) -> Option<ReplayGameInfo> {
        let mut info = ReplayGameInfo::new();
        if parse_ascii_string_to_game_info(&mut info, options) {
            Some(info)
        } else {
            None
        }
    }

    /// Load and playback the selected replay
    ///
    /// Matches C++ ReplayMenu.cpp:396-418 and 519-550
    pub fn load_replay(&self) -> Result<(), String> {
        if self.selected_index < 0 {
            return Err("GUI:NoFileSelected".to_string());
        }

        let filename = self.get_replay_filename_from_listbox(self.selected_index);
        if filename.is_empty() {
            return Err("GUI:PleaseSelectAFile".to_string());
        }

        // Check version compatibility
        let filepath = self.replay_directory.join(&filename);
        if let Some(header) = self.read_replay_header(&filepath) {
            if !header.is_version_compatible(
                &self.current_version,
                self.current_version_number,
                self.current_exe_crc,
                self.current_ini_crc,
            ) {
                // Show warning dialog for older version
                return Err("GUI:OlderReplayVersion".to_string());
            }
        }

        // Start playback
        self.playback_file(&filename)?;

        Ok(())
    }

    /// Actually start playback of a replay file
    fn playback_file(&self, filename: &str) -> Result<(), String> {
        recorder::init_recorder();
        let ok = recorder::with_recorder_mut(|rec| rec.playback_file(filename.to_string()))
            .ok_or_else(|| "Replay recorder unavailable".to_string())?
            .map_err(|err| err.to_string())?;
        if ok {
            Ok(())
        } else {
            Err("Failed to start replay playback".to_string())
        }
    }

    /// Handle double-click on listbox entry
    ///
    /// Matches C++ ReplayMenu.cpp:459-483
    pub fn handle_listbox_double_click(&mut self, row_selected: i32) -> Result<(), String> {
        if row_selected < 0 {
            return Ok(());
        }

        let filename = self.get_replay_filename_from_listbox(row_selected);
        if filename.is_empty() {
            return Err("Invalid replay file".to_string());
        }

        self.playback_file(&filename)?;

        Ok(())
    }

    /// Delete the selected replay file
    ///
    /// Matches C++ ReplayMenu.cpp:593-619
    pub fn delete_replay(&mut self) {
        self.call_delete = false;

        if self.selected_index < 0 {
            // Show error: "GUI:NoFileSelected"
            return;
        }

        let filename = self.get_replay_filename_from_listbox(self.selected_index);
        let filepath = self.replay_directory.join(&filename);

        match fs::remove_file(&filepath) {
            Ok(_) => {
                // Successfully deleted
                self.populate_replay_listbox();
            }
            Err(e) => {
                // Show error message
                eprintln!("Error deleting file: {}", e);
            }
        }
    }

    /// Copy the selected replay to desktop
    ///
    /// Matches C++ ReplayMenu.cpp:622-656
    pub fn copy_replay(&mut self) {
        self.call_copy = false;

        if self.selected_index < 0 {
            // Show error: "GUI:NoFileSelected"
            return;
        }

        let filename = self.get_replay_filename_from_listbox(self.selected_index);
        let source = self.replay_directory.join(&filename);

        // Get desktop path
        if let Some(desktop_path) = self.get_desktop_path() {
            let dest = desktop_path.join(&filename);

            match fs::copy(&source, &dest) {
                Ok(_) => {
                    // Successfully copied
                }
                Err(e) => {
                    // Show error message
                    eprintln!("Error copying file: {}", e);
                }
            }
        }
    }

    /// Get desktop directory path
    fn get_desktop_path(&self) -> Option<PathBuf> {
        // Platform-specific desktop path retrieval
        #[cfg(target_os = "windows")]
        {
            // On Windows, get desktop from environment variable
            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                let desktop = PathBuf::from(userprofile).join("Desktop");
                if desktop.exists() {
                    return Some(desktop);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // On macOS, desktop is in ~/Desktop
            if let Ok(home) = std::env::var("HOME") {
                let desktop = PathBuf::from(home).join("Desktop");
                if desktop.exists() {
                    return Some(desktop);
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, desktop is typically in ~/Desktop
            if let Ok(home) = std::env::var("HOME") {
                let desktop = PathBuf::from(home).join("Desktop");
                if desktop.exists() {
                    return Some(desktop);
                }
            }
        }

        None
    }

    /// Set flags for deferred operations
    ///
    /// Matches C++ ReplayMenu.cpp:62-63
    pub fn delete_replay_flag(&mut self) {
        self.call_delete = true;
    }

    pub fn copy_replay_flag(&mut self) {
        self.call_copy = true;
    }

    /// Handle button press events
    ///
    /// Matches C++ ReplayMenu.cpp:485-584
    pub fn handle_button_selected(&mut self, button_id: u32) -> Result<(), String> {
        if button_id == self.button_load_id {
            self.load_replay()?;
        } else if button_id == self.button_back_id {
            // Return to previous menu
            self.shutdown(false);
        } else if button_id == self.button_delete_id {
            if self.selected_index < 0 {
                return Err("GUI:NoFileSelected".to_string());
            }
            // Show confirmation dialog
            self.delete_replay_flag();
        } else if button_id == self.button_copy_id {
            if self.selected_index < 0 {
                return Err("GUI:NoFileSelected".to_string());
            }
            // Show confirmation dialog
            self.copy_replay_flag();
        }

        Ok(())
    }

    /// Handle keyboard input
    ///
    /// Matches C++ ReplayMenu.cpp:349-394
    pub fn handle_key_input(&mut self, key: KeyCode, state: KeyState) -> bool {
        match key {
            KeyCode::Escape => {
                if state == KeyState::Up {
                    // Simulate back button press
                    let _ = self.handle_button_selected(self.button_back_id);
                }
                true
            }
            _ => false,
        }
    }

    /// Get listbox entries for display
    pub fn get_replay_list(&self) -> &Vec<ReplayListEntry> {
        &self.replay_list
    }

    /// Get selected index
    pub fn get_selected_index(&self) -> i32 {
        self.selected_index
    }

    /// Set selected index
    pub fn set_selected_index(&mut self, index: i32) {
        if index >= -1 && index < self.replay_list.len() as i32 {
            self.selected_index = index;
        }
    }
}

/// Key codes for keyboard input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Escape,
    Enter,
    Up,
    Down,
    Other(u8),
}

/// Key states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Down,
    Up,
}

/// Parse ASCII string to game info
///
/// Matches C++ GameInfo parsing logic
pub fn parse_ascii_string_to_game_info(game_info: &mut ReplayGameInfo, options: &str) -> bool {
    let mut net_info = game_network::game_info::GameInfo::new();
    if !game_network::game_info::serialization::parse_ascii_string_to_game_info(
        &mut net_info,
        options,
    ) {
        return false;
    }

    game_info.set_map(net_info.get_map().to_string());

    for index in 0..MAX_SLOTS {
        let mut slot = GameSlot::new();
        if let Some(net_slot) = net_info.get_slot(index) {
            slot.state = map_slot_state(net_slot.get_state());
            slot.name = net_slot.get_name().to_string();
            slot.color = net_slot.get_color();
            slot.start_pos = net_slot.get_start_pos();
            slot.player_template = net_slot.get_player_template();
            slot.team_number = net_slot.get_team_number();
            slot.ip = net_slot.get_ip();
            slot.port = net_slot.get_port();
        }
        game_info.set_slot(index, slot);
    }

    true
}

/// Get Unicode time buffer from system time
///
/// Matches C++ getUnicodeTimeBuffer function
pub fn get_unicode_time_buffer(time_val: &SystemTimeValue) -> String {
    time_val.to_display_string()
}

fn map_slot_state(state: game_network::game_info::SlotState) -> SlotState {
    match state {
        game_network::game_info::SlotState::Open => SlotState::Open,
        game_network::game_info::SlotState::Closed => SlotState::Closed,
        game_network::game_info::SlotState::EasyAI => SlotState::EasyAI,
        game_network::game_info::SlotState::MedAI => SlotState::MedAI,
        game_network::game_info::SlotState::BrutalAI => SlotState::BrutalAI,
        game_network::game_info::SlotState::Player => SlotState::Player,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_header_creation() {
        let header = ReplayHeader::new();
        assert_eq!(header.filename, "");
        assert!(!header.for_playback);
        assert_eq!(header.local_player_index, -1);
        assert_eq!(header.player_discons.len(), MAX_SLOTS);
    }

    #[test]
    fn test_replay_header_duration() {
        let mut header = ReplayHeader::new();
        header.start_time = 1000;
        header.end_time = 1180;

        assert_eq!(header.get_duration_seconds(), 180);
        assert_eq!(header.format_duration(), "3:00");
    }

    #[test]
    fn test_replay_header_fps() {
        let mut header = ReplayHeader::new();
        header.start_time = 0;
        header.end_time = 100;
        header.frame_duration = 3000;

        let fps = header.calculate_fps();
        assert_eq!(fps, 30.0);
    }

    #[test]
    fn test_replay_header_version_compatibility() {
        let mut header = ReplayHeader::new();
        header.version_string = "1.0".to_string();
        header.version_number = 100;
        header.exe_crc = 0x12345678;
        header.ini_crc = 0x87654321;

        assert!(header.is_version_compatible("1.0", 100, 0x12345678, 0x87654321));
        assert!(!header.is_version_compatible("1.1", 100, 0x12345678, 0x87654321));
        assert!(!header.is_version_compatible("1.0", 101, 0x12345678, 0x87654321));
    }

    #[test]
    fn test_replay_header_display_color() {
        let mut header = ReplayHeader::new();
        header.version_string = "1.0".to_string();
        header.version_number = 100;
        header.exe_crc = 0x12345678;
        header.ini_crc = 0x87654321;
        header.local_player_index = 0; // Multiplayer

        let color = header.get_display_color("1.0", 100, 0x12345678, 0x87654321);
        assert_eq!(color, Color::white());

        let color = header.get_display_color("1.1", 100, 0x12345678, 0x87654321);
        assert_eq!(color, Color::gray());
    }

    #[test]
    fn test_replay_game_info_creation() {
        let game_info = ReplayGameInfo::new();
        assert_eq!(game_info.map_name, "");
        assert_eq!(game_info.game_mode, 0);
    }

    #[test]
    fn test_replay_game_info_slots() {
        let mut game_info = ReplayGameInfo::new();

        let slot = game_info.get_slot(0);
        assert!(slot.is_some());

        let slot = game_info.get_slot(MAX_SLOTS);
        assert!(slot.is_none());

        let new_slot = GameSlot::new();
        game_info.set_slot(0, new_slot);
    }

    #[test]
    fn test_replay_menu_creation() {
        let menu = ReplayMenu::new(PathBuf::from("/replays"), ".rep".to_string());

        assert_eq!(menu.selected_index, -1);
        assert_eq!(menu.replay_list.len(), 0);
        assert!(!menu.is_shutting_down);
    }

    #[test]
    fn test_replay_menu_flags() {
        let mut menu = ReplayMenu::new(PathBuf::from("/replays"), ".rep".to_string());

        assert!(!menu.call_delete);
        assert!(!menu.call_copy);

        menu.delete_replay_flag();
        assert!(menu.call_delete);

        menu.copy_replay_flag();
        assert!(menu.call_copy);
    }

    #[test]
    fn test_replay_list_entry_creation() {
        let entry = ReplayListEntry::new(
            "Test Replay".to_string(),
            "2024-01-01".to_string(),
            "1.0".to_string(),
            "TestMap".to_string(),
            Color::white(),
            "test.rep".to_string(),
        );

        assert_eq!(entry.name, "Test Replay");
        assert_eq!(entry.map, "TestMap");
        assert_eq!(entry.filename, "test.rep");
    }

    #[test]
    fn replay_filename_lookup_uses_stored_filename_like_cpp() {
        let mut menu = ReplayMenu::new(PathBuf::from("/replays"), ".rep".to_string());
        menu.replay_list.push(ReplayListEntry::new(
            "HeaderReplayName".to_string(),
            "2024-01-01".to_string(),
            "1.0".to_string(),
            "TestMap".to_string(),
            Color::white(),
            "ActualFileName.rep".to_string(),
        ));

        assert_eq!(
            menu.get_replay_filename_from_listbox(0),
            "ActualFileName.rep"
        );
    }

    #[test]
    fn replay_display_name_is_derived_from_filename_like_cpp() {
        assert_eq!(
            ReplayMenu::replay_display_name_from_filename("ActualFileName.rep", ".rep"),
            "ActualFileName"
        );
        assert_eq!(
            ReplayMenu::replay_display_name_from_filename("HeaderNameWasDifferent.rep", ".rep"),
            "HeaderNameWasDifferent"
        );
    }

    #[test]
    fn test_system_time_value() {
        let time = SystemTimeValue::now();
        let display = time.to_display_string();
        assert!(display.contains(':'));
    }

    #[test]
    fn test_color_creation() {
        let white = Color::white();
        assert_eq!(white.r, 255);
        assert_eq!(white.g, 255);
        assert_eq!(white.b, 255);
        assert_eq!(white.a, 255);

        let gray = Color::gray();
        assert_eq!(gray.r, 128);
        assert_eq!(gray.g, 128);
        assert_eq!(gray.b, 128);
    }
}
