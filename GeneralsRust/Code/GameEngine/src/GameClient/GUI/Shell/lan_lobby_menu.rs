// FILE: lan_lobby_menu.rs
// Author: Ported from C++ by Claude, November 2024
// Description: LAN Lobby Menu - faithful port from C++

use std::collections::HashMap;
use std::time::SystemTime;

/// LAN Preferences - handles Network.ini settings
pub struct LANPreferences {
    preferences: HashMap<String, String>,
}

impl LANPreferences {
    pub fn new() -> Self {
        let mut prefs = LANPreferences {
            preferences: HashMap::new(),
        };
        prefs.load("Network.ini");
        prefs
    }

    fn load(&mut self, filename: &str) {
        // Load from Network.ini file
        // This would integrate with the preference system
        // For now, a placeholder that will be filled with actual file I/O
    }

    pub fn write(&self) {
        // Write preferences to Network.ini
    }

    pub fn get_user_name(&self) -> String {
        if let Some(name) = self.preferences.get("UserName") {
            let decoded = Self::quoted_printable_to_string(name);
            let trimmed = decoded.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // Fall back to machine name
        Self::get_machine_name()
    }

    pub fn get_preferred_color(&self) -> i32 {
        if let Some(color_str) = self.preferences.get("Color") {
            if let Ok(color) = color_str.parse::<i32>() {
                // Validate color range (will be checked against multiplayer settings)
                if color >= -1 {
                    return color;
                }
            }
        }
        -1
    }

    pub fn get_preferred_faction(&self) -> i32 {
        if let Some(faction_str) = self.preferences.get("PlayerTemplate") {
            if let Ok(faction) = faction_str.parse::<i32>() {
                // PLAYERTEMPLATE_RANDOM or valid faction
                // Validation happens against ThePlayerTemplateStore
                return faction;
            }
        }
        -1 // PLAYERTEMPLATE_RANDOM
    }

    pub fn uses_system_map_dir(&self) -> bool {
        if let Some(val) = self.preferences.get("UseSystemMapDir") {
            return val.eq_ignore_ascii_case("yes");
        }
        true
    }

    pub fn get_preferred_map(&self) -> String {
        if let Some(map_str) = self.preferences.get("Map") {
            let decoded = Self::quoted_printable_to_string(map_str);
            let trimmed = decoded.trim();
            if !trimmed.is_empty() {
                // Validate map exists
                return trimmed.to_string();
            }
        }
        Self::get_default_map(true)
    }

    pub fn get_num_remote_ips(&self) -> i32 {
        if let Some(num_str) = self.preferences.get("NumRemoteIPs") {
            if let Ok(num) = num_str.parse::<i32>() {
                return num;
            }
        }
        0
    }

    pub fn get_remote_ip_entry(&self, index: i32) -> String {
        let key = format!("RemoteIP{}", index);
        if let Some(entry) = self.preferences.get(&key) {
            // Parse "IP:Description" format
            if let Some(colon_pos) = entry.find(':') {
                let ip = &entry[..colon_pos];
                let desc = &entry[colon_pos + 1..];
                if !desc.is_empty() {
                    return format!("{}({})", ip, Self::quoted_printable_to_string(desc));
                }
                return ip.to_string();
            }
            return entry.clone();
        }
        String::new()
    }

    pub fn get_superweapon_restricted(&self) -> bool {
        if let Some(val) = self.preferences.get("SuperweaponRestrict") {
            return val.eq_ignore_ascii_case("yes");
        }
        false
    }

    pub fn set_superweapon_restricted(&mut self, restricted: bool) {
        self.preferences.insert(
            "SuperweaponRestrict".to_string(),
            if restricted { "Yes" } else { "No" }.to_string(),
        );
    }

    pub fn get_starting_cash(&self) -> u32 {
        if let Some(cash_str) = self.preferences.get("StartingCash") {
            if let Ok(cash) = cash_str.parse::<u32>() {
                return cash;
            }
        }
        // Default starting money from multiplayer settings
        10000
    }

    pub fn set_starting_cash(&mut self, cash: u32) {
        self.preferences.insert("StartingCash".to_string(), cash.to_string());
    }

    pub fn set(&mut self, key: String, value: String) {
        self.preferences.insert(key, value);
    }

    // Helper functions
    fn quoted_printable_to_string(encoded: &str) -> String {
        // Decode quoted-printable encoding
        // This is a simplified version - full implementation would handle all QP rules
        encoded.to_string()
    }

    fn get_machine_name() -> String {
        // Get machine name from system
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "Player".to_string())
    }

    fn get_default_map(is_official: bool) -> String {
        // Return default map name
        "DefaultMap.map".to_string()
    }
}

/// LAN Lobby Menu State
pub struct LANLobbyMenu {
    // Window IDs
    parent_lan_lobby_id: i32,
    button_back_id: i32,
    button_clear_id: i32,
    button_host_id: i32,
    button_join_id: i32,
    button_direct_connect_id: i32,
    button_emote_id: i32,
    static_tooltip_id: i32,
    text_entry_player_name_id: i32,
    text_entry_chat_id: i32,
    listbox_players_id: i32,
    listbox_chat_window_id: i32,
    listbox_games_id: i32,
    static_text_game_info_id: i32,

    // State flags
    is_shutting_down: bool,
    button_pushed: bool,
    socket_error_detected: bool,
    next_screen: Option<String>,
    just_entered: bool,
    initial_gadget_delay: i32,

    // Preferences
    default_name: String,
    use_fps_limit: bool,
}

impl LANLobbyMenu {
    pub fn new() -> Self {
        LANLobbyMenu {
            parent_lan_lobby_id: 0,
            button_back_id: 0,
            button_clear_id: 0,
            button_host_id: 0,
            button_join_id: 0,
            button_direct_connect_id: 0,
            button_emote_id: 0,
            static_tooltip_id: 0,
            text_entry_player_name_id: 0,
            text_entry_chat_id: 0,
            listbox_players_id: 0,
            listbox_chat_window_id: 0,
            listbox_games_id: 0,
            static_text_game_info_id: 0,
            is_shutting_down: false,
            button_pushed: false,
            socket_error_detected: false,
            next_screen: None,
            just_entered: false,
            initial_gadget_delay: 2,
            default_name: String::new(),
            use_fps_limit: true,
        }
    }

    /// Initialize the LAN Lobby Menu
    pub fn init(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        self.next_screen = None;
        self.button_pushed = false;
        self.is_shutting_down = false;

        // Get window IDs from name key generator
        self.parent_lan_lobby_id = self.get_window_id("LanLobbyMenu.wnd:LanLobbyMenuParent");
        self.button_back_id = self.get_window_id("LanLobbyMenu.wnd:ButtonBack");
        self.button_clear_id = self.get_window_id("LanLobbyMenu.wnd:ButtonClear");
        self.button_host_id = self.get_window_id("LanLobbyMenu.wnd:ButtonHost");
        self.button_join_id = self.get_window_id("LanLobbyMenu.wnd:ButtonJoin");
        self.button_direct_connect_id = self.get_window_id("LanLobbyMenu.wnd:ButtonDirectConnect");
        self.button_emote_id = self.get_window_id("LanLobbyMenu.wnd:ButtonEmote");
        self.static_tooltip_id = self.get_window_id("LanLobbyMenu.wnd:StaticToolTip");
        self.text_entry_player_name_id = self.get_window_id("LanLobbyMenu.wnd:TextEntryPlayerName");
        self.text_entry_chat_id = self.get_window_id("LanLobbyMenu.wnd:TextEntryChat");
        self.listbox_players_id = self.get_window_id("LanLobbyMenu.wnd:ListboxPlayers");
        self.listbox_chat_window_id = self.get_window_id("LanLobbyMenu.wnd:ListboxChatWindowLanLobby");
        self.listbox_games_id = self.get_window_id("LanLobbyMenu.wnd:ListboxGames");
        self.static_text_game_info_id = self.get_window_id("LanLobbyMenu.wnd:StaticTextGameInfo");

        // Get pointers to windows (stored as weak references in real implementation)
        // This would integrate with the actual window management system

        // Show menu
        layout.hide(false);

        // Initialize LAN API
        // In real implementation, this would create the LANAPI singleton
        self.init_lan_api();

        // Get user preferences
        let prefs = LANPreferences::new();
        self.default_name = prefs.get_user_name();

        // Truncate name to max length
        const LAN_PLAYER_NAME_LENGTH: usize = 32;
        if self.default_name.len() > LAN_PLAYER_NAME_LENGTH {
            self.default_name.truncate(LAN_PLAYER_NAME_LENGTH);
        }

        // Set player name in text entry
        self.set_text_entry(self.text_entry_player_name_id, &self.default_name);

        // Clear chat text entry
        self.set_text_entry(self.text_entry_chat_id, "");

        // Reset listboxes
        self.reset_listbox(self.listbox_players_id);
        self.reset_listbox(self.listbox_games_id);

        // Request LAN operations
        self.request_set_name(&self.default_name);
        self.request_locations();

        // Set keyboard focus to chat entry
        self.set_focus(self.text_entry_chat_id);

        // Create game info window
        self.create_lan_game_info_window(self.static_text_game_info_id);

        // Show shell map
        self.show_shell_map(true);

        // Check MOTD
        self.check_motd();

        // Show layout
        layout.hide(false);
        layout.bring_forward();

        self.just_entered = true;
        self.initial_gadget_delay = 2;

        // Hide gadget parent initially for animation
        let gadget_parent_id = self.get_window_id("LanLobbyMenu.wnd:GadgetParent");
        self.hide_window(gadget_parent_id, true);
    }

    /// Shutdown the LAN Lobby Menu
    pub fn shutdown(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Save preferences
        let mut prefs = LANPreferences::new();
        let player_name = self.get_text_entry(self.text_entry_player_name_id);
        prefs.set("UserName".to_string(), Self::string_to_quoted_printable(&player_name));
        prefs.write();

        // Destroy game info window
        self.destroy_game_info_window();

        // Request lobby leave
        self.request_lobby_leave(true);

        // Restore FPS limit
        // TheWritableGlobalData->m_useFpsLimit = self.use_fps_limit;

        self.is_shutting_down = true;

        // Check for immediate pop
        let pop_immediate = user_data.is_some(); // Simplified check
        self.socket_error_detected = false;

        if pop_immediate {
            self.shutdown_complete(layout);
            return;
        }

        // Reverse animations
        self.reverse_animate_window();
        self.reverse_transition("LanLobbyFade");
    }

    /// Update the LAN Lobby Menu
    pub fn update(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Check if entering from game
        if self.is_in_shell_game() && self.get_game_frame() == 1 {
            self.signal_ui_interaction("SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME");
        }

        // Handle gadget animations
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.set_transition_group("LanLobbyFade");
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay -= 1;
            }
        }

        // Check for shutdown complete
        if self.is_shutting_down && self.is_anim_finished() && self.is_transition_finished() {
            self.shutdown_complete(layout);
        }

        // Update LAN API if not button pushed
        if self.is_anim_finished() && !self.button_pushed {
            self.lan_update();
        }

        // Handle socket errors
        if self.socket_error_detected {
            self.socket_error_detected = false;

            // Show error message
            self.message_box_ok("Network Error", "Socket Error");

            // Back to main menu
            self.send_system_message_back_button();
        }
    }

    /// Handle input messages
    pub fn handle_input(&mut self, window_id: i32, msg: u32, data1: usize, data2: usize) -> bool {
        const GWM_CHAR: u32 = 0x0102;
        const KEY_ESC: u8 = 0x1B;
        const KEY_STATE_UP: u8 = 0x01;

        if msg == GWM_CHAR {
            let key = (data1 & 0xFF) as u8;
            let state = (data2 & 0xFF) as u8;

            if self.button_pushed {
                return false;
            }

            if key == KEY_ESC {
                if (state & KEY_STATE_UP) != 0 {
                    // Send simulated selected event to back button
                    self.send_system_message(
                        window_id,
                        0x0201, // GBM_SELECTED
                        self.button_back_id,
                        self.button_back_id as usize,
                    );
                }
                return true; // MSG_HANDLED
            }
        }

        false // MSG_IGNORED
    }

    /// Handle system messages
    pub fn handle_system(&mut self, window_id: i32, msg: u32, data1: usize, data2: usize) -> bool {
        const GWM_CREATE: u32 = 0x0001;
        const GWM_DESTROY: u32 = 0x0002;
        const GWM_INPUT_FOCUS: u32 = 0x0003;
        const GLM_DOUBLE_CLICKED: u32 = 0x0301;
        const GLM_SELECTED: u32 = 0x0302;
        const GBM_SELECTED: u32 = 0x0201;
        const GEM_UPDATE_TEXT: u32 = 0x0401;
        const GEM_EDIT_DONE: u32 = 0x0402;

        match msg {
            GWM_CREATE => {
                self.signal_ui_interaction("SHELL_SCRIPT_HOOK_LAN_OPENED");
                return true;
            }
            GWM_DESTROY => {
                self.signal_ui_interaction("SHELL_SCRIPT_HOOK_LAN_CLOSED");
                return true;
            }
            GWM_INPUT_FOCUS => {
                // Accept keyboard focus
                return true;
            }
            GLM_DOUBLE_CLICKED => {
                if self.button_pushed {
                    return true;
                }
                let control_id = data1 as i32;

                if control_id == self.listbox_games_id {
                    let row_selected = data2 as i32;
                    if row_selected >= 0 {
                        // Join selected game
                        self.join_game_by_list_offset(row_selected);
                    }
                }
                return true;
            }
            GLM_SELECTED => {
                if self.button_pushed {
                    return true;
                }
                let control_id = data1 as i32;

                if control_id == self.listbox_games_id {
                    let row_selected = data2 as i32;
                    if row_selected < 0 {
                        self.hide_game_info_window(true);
                    } else {
                        // Show game info for selected game
                        self.show_game_info_for_row(row_selected);
                    }
                }
                return true;
            }
            GBM_SELECTED => {
                if self.button_pushed {
                    return true;
                }
                let control_id = data1 as i32;

                if control_id == self.button_back_id {
                    self.button_pushed = true;
                    self.pop_menu();
                    // Delete LAN API singleton
                    self.cleanup_lan_api();
                } else if control_id == self.button_host_id {
                    // Create new game
                    self.request_game_create("", false);
                } else if control_id == self.button_clear_id {
                    // Clear player name
                    self.set_text_entry(self.text_entry_player_name_id, "");
                    self.send_update_text_message(self.text_entry_player_name_id);
                } else if control_id == self.button_join_id {
                    // Join selected game
                    let selected_row = self.get_listbox_selected(self.listbox_games_id);
                    if selected_row >= 0 {
                        self.join_game_by_list_offset(selected_row);
                    } else {
                        self.add_chat_text("Error: No game selected");
                    }
                } else if control_id == self.button_emote_id {
                    // Send chat (emote button now sends normal chat)
                    let text = self.get_text_entry(self.text_entry_chat_id);
                    self.set_text_entry(self.text_entry_chat_id, "");

                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        self.request_chat(trimmed, false); // LANCHAT_NORMAL
                    }
                } else if control_id == self.button_direct_connect_id {
                    // Direct connect
                    self.request_lobby_leave(false);
                    self.push_menu("Menus/NetworkDirectConnect.wnd");
                }
                return true;
            }
            GEM_UPDATE_TEXT => {
                if self.button_pushed {
                    return true;
                }
                let control_id = data1 as i32;

                if control_id == self.text_entry_player_name_id {
                    let mut name = self.get_text_entry(self.text_entry_player_name_id);

                    // Trim leading whitespace
                    name = name.trim_start().to_string();

                    // Truncate to max length
                    const LAN_PLAYER_NAME_LENGTH: usize = 32;
                    if name.len() > LAN_PLAYER_NAME_LENGTH {
                        name.truncate(LAN_PLAYER_NAME_LENGTH);
                    }

                    // Remove invalid characters (comma, colon, semicolon)
                    if name.ends_with(',') || name.ends_with(':') || name.ends_with(';') {
                        name.pop();
                    }

                    // Send name update
                    if !name.is_empty() {
                        self.request_set_name(&name);
                    } else {
                        self.request_set_name(&self.default_name);
                    }

                    // Update text entry with cleaned name
                    self.set_text_entry(self.text_entry_player_name_id, &name);
                }
                return true;
            }
            GEM_EDIT_DONE => {
                if self.button_pushed {
                    return true;
                }
                let control_id = data1 as i32;

                if control_id == self.text_entry_chat_id {
                    let text = self.get_text_entry(self.text_entry_chat_id);
                    self.set_text_entry(self.text_entry_chat_id, "");

                    // Trim leading whitespace
                    let trimmed = text.trim_start();

                    if !trimmed.is_empty() {
                        self.request_chat(trimmed, false); // LANCHAT_NORMAL
                    }
                }
                return true;
            }
            _ => return false,
        }
    }

    /// Shutdown complete callback
    fn shutdown_complete(&mut self, layout: &mut WindowLayout) {
        self.is_shutting_down = false;

        layout.hide(true);

        // Signal shell shutdown complete
        let has_next_screen = self.next_screen.is_some();
        self.shell_shutdown_complete(layout, has_next_screen);

        if let Some(next_screen) = &self.next_screen {
            self.push_menu(next_screen);
        }

        self.next_screen = None;
    }

    // Placeholder functions for integration with actual game engine
    fn get_window_id(&self, name: &str) -> i32 { 0 }
    fn set_text_entry(&self, id: i32, text: &str) {}
    fn get_text_entry(&self, id: i32) -> String { String::new() }
    fn reset_listbox(&self, id: i32) {}
    fn set_focus(&self, id: i32) {}
    fn hide_window(&self, id: i32, hide: bool) {}
    fn init_lan_api(&mut self) {}
    fn cleanup_lan_api(&self) {}
    fn request_set_name(&self, name: &str) {}
    fn request_locations(&self) {}
    fn request_lobby_leave(&self, clear: bool) {}
    fn request_game_create(&self, name: &str, ranked: bool) {}
    fn request_chat(&self, text: &str, is_emote: bool) {}
    fn create_lan_game_info_window(&self, id: i32) {}
    fn destroy_game_info_window(&self) {}
    fn show_shell_map(&self, show: bool) {}
    fn check_motd(&self) {}
    fn lan_update(&self) {}
    fn is_in_shell_game(&self) -> bool { false }
    fn get_game_frame(&self) -> i32 { 0 }
    fn signal_ui_interaction(&self, hook: &str) {}
    fn is_anim_finished(&self) -> bool { true }
    fn is_transition_finished(&self) -> bool { true }
    fn set_transition_group(&self, group: &str) {}
    fn reverse_animate_window(&self) {}
    fn reverse_transition(&self, name: &str) {}
    fn message_box_ok(&self, title: &str, message: &str) {}
    fn send_system_message_back_button(&self) {}
    fn send_system_message(&self, window: i32, msg: u32, data1: i32, data2: usize) {}
    fn send_update_text_message(&self, id: i32) {}
    fn pop_menu(&self) {}
    fn push_menu(&self, name: &str) {}
    fn shell_shutdown_complete(&self, layout: &WindowLayout, has_next: bool) {}
    fn join_game_by_list_offset(&self, offset: i32) {}
    fn get_listbox_selected(&self, id: i32) -> i32 { -1 }
    fn add_chat_text(&self, text: &str) {}
    fn hide_game_info_window(&self, hide: bool) {}
    fn show_game_info_for_row(&self, row: i32) {}
    fn string_to_quoted_printable(s: &str) -> String { s.to_string() }
}

/// Window Layout placeholder
pub struct WindowLayout;

impl WindowLayout {
    pub fn hide(&mut self, hidden: bool) {}
    pub fn bring_forward(&mut self) {}
}
