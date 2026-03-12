// FILE: lan_game_options_menu.rs
// Author: Ported from C++ by Claude, November 2024
// Description: LAN Game Options Menu - faithful port from C++

use std::collections::HashSet;

const MAX_SLOTS: usize = 8;

/// Slot state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Open = 0,
    Closed = 1,
    Player = 2,
    EasyAI = 3,
    MediumAI = 4,
    HardAI = 5,
}

/// LAN Game Options Menu State
pub struct LANGameOptionsMenu {
    // Window IDs
    parent_lan_game_options_id: i32,
    combo_box_player_id: [i32; MAX_SLOTS],
    button_accept_id: [i32; MAX_SLOTS],
    combo_box_color_id: [i32; MAX_SLOTS],
    combo_box_player_template_id: [i32; MAX_SLOTS],
    combo_box_team_id: [i32; MAX_SLOTS],
    button_map_start_position_id: [i32; MAX_SLOTS],

    text_entry_chat_id: i32,
    text_entry_map_display_id: i32,
    button_back_id: i32,
    button_start_id: i32,
    button_emote_id: i32,
    button_select_map_id: i32,
    checkbox_limit_superweapons_id: i32,
    combo_box_starting_cash_id: i32,
    window_map_id: i32,
    listbox_chat_window_lan_game_id: i32,

    // State flags
    is_shutting_down: bool,
    button_pushed: bool,
    next_screen: Option<String>,
    is_initing: bool,
}

impl LANGameOptionsMenu {
    pub fn new() -> Self {
        LANGameOptionsMenu {
            parent_lan_game_options_id: 0,
            combo_box_player_id: [0; MAX_SLOTS],
            button_accept_id: [0; MAX_SLOTS],
            combo_box_color_id: [0; MAX_SLOTS],
            combo_box_player_template_id: [0; MAX_SLOTS],
            combo_box_team_id: [0; MAX_SLOTS],
            button_map_start_position_id: [0; MAX_SLOTS],
            text_entry_chat_id: 0,
            text_entry_map_display_id: 0,
            button_back_id: 0,
            button_start_id: 0,
            button_emote_id: 0,
            button_select_map_id: 0,
            checkbox_limit_superweapons_id: 0,
            combo_box_starting_cash_id: 0,
            window_map_id: 0,
            listbox_chat_window_lan_game_id: 0,
            is_shutting_down: false,
            button_pushed: false,
            next_screen: None,
            is_initing: false,
        }
    }

    /// Initialize the LAN Game Options Menu
    pub fn init(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Check if returning from game
        if self.is_game_in_progress() {
            // Pop back to lobby
            self.pop_immediate();
            return;
        }

        self.is_initing = true;
        self.button_pushed = false;
        self.is_shutting_down = false;

        // Disable slot list updates during initialization
        self.enable_slot_list_updates(false);
        self.init_lan_game_gadgets();
        self.enable_slot_list_updates(true);

        // Clear text fields
        self.reset_listbox(self.listbox_chat_window_lan_game_id);
        self.set_text_entry(self.text_entry_chat_id, "");

        // Update map cache
        self.update_map_cache();

        // Different initialization for host vs client
        if self.am_i_host() {
            // Read preferences as host
            let prefs = super::lan_lobby_menu::LANPreferences::new();

            // Set slot 0 preferences (local player)
            self.set_local_player_color(prefs.get_preferred_color());
            self.set_local_player_template(prefs.get_preferred_faction());
            self.set_local_player_nat_behavior(0); // FIREWALL_TYPE_SIMPLE

            // Set game options
            let map = prefs.get_preferred_map();
            self.set_game_map(&map);
            self.set_starting_cash(prefs.get_starting_cash());
            self.set_superweapon_restriction(if prefs.get_superweapon_restricted() { 1 } else { 0 });

            // Validate map and set availability
            if let Some(map_data) = self.find_map(&map) {
                self.set_local_player_map_availability(true);
                self.set_game_map_crc(map_data.crc);
                self.set_game_map_size(map_data.file_size);
                self.adjust_slots_for_map();
            }

            self.lan_update_slot_list();
            self.update_game_options();
        } else {
            // Client initialization
            self.set_button_text(self.button_start_id, "Accept");
            self.enable_button(self.button_select_map_id, false);
            self.enable_window(self.checkbox_limit_superweapons_id, false);
            self.enable_window(self.combo_box_starting_cash_id, false);

            // Force recheck of map availability
            let map_crc = self.get_game_map_crc();
            let map_size = self.get_game_map_size();
            self.set_game_map_crc(map_crc);
            self.set_game_map_size(map_size);
            self.request_has_map();

            self.lan_update_slot_list();
            self.update_game_options();
        }

        // Disable controls for non-host slots
        let local_slot = self.get_local_slot_num();
        let start_idx = if self.am_i_host() { 1 } else { 0 };

        for i in start_idx..MAX_SLOTS {
            if !self.am_i_host() {
                self.enable_combo_box(self.combo_box_player_id[i], false);
            }
            self.enable_combo_box(self.combo_box_color_id[i], false);
            self.enable_combo_box(self.combo_box_player_template_id[i], false);
            self.enable_combo_box(self.combo_box_team_id[i], false);
        }

        // Show menu
        layout.hide(false);

        // Set keyboard focus
        self.set_focus(self.parent_lan_game_options_id);

        self.is_initing = false;

        // Send game options if host
        if self.am_i_host() {
            let options = self.generate_game_options_string();
            self.request_game_options(&options, true);
            self.request_game_announce();
        }

        self.lan_update_slot_list();
        self.lan_position_start_spots();
        self.set_transition_group("LanGameOptionsFade");
    }

    /// Initialize gadgets
    fn init_lan_game_gadgets(&mut self) {
        self.parent_lan_game_options_id = self.get_window_id("LanGameOptionsMenu.wnd:LanGameOptionsMenuParent");
        self.button_back_id = self.get_window_id("LanGameOptionsMenu.wnd:ButtonBack");
        self.button_start_id = self.get_window_id("LanGameOptionsMenu.wnd:ButtonStart");
        self.text_entry_chat_id = self.get_window_id("LanGameOptionsMenu.wnd:TextEntryChat");
        self.text_entry_map_display_id = self.get_window_id("LanGameOptionsMenu.wnd:TextEntryMapDisplay");
        self.listbox_chat_window_lan_game_id = self.get_window_id("LanGameOptionsMenu.wnd:ListboxChatWindowLanGame");
        self.button_emote_id = self.get_window_id("LanGameOptionsMenu.wnd:ButtonEmote");
        self.button_select_map_id = self.get_window_id("LanGameOptionsMenu.wnd:ButtonSelectMap");
        self.checkbox_limit_superweapons_id = self.get_window_id("LanGameOptionsMenu.wnd:CheckboxLimitSuperweapons");
        self.combo_box_starting_cash_id = self.get_window_id("LanGameOptionsMenu.wnd:ComboBoxStartingCash");
        self.window_map_id = self.get_window_id("LanGameOptionsMenu.wnd:MapWindow");

        // Populate starting cash combo box
        self.populate_starting_cash_combo_box();

        // Setup map window tooltip
        self.set_map_tooltip(self.window_map_id);

        let local_slot_num = self.get_local_slot_num();

        // Initialize per-player controls
        for i in 0..MAX_SLOTS {
            self.combo_box_player_id[i] = self.get_window_id(&format!("LanGameOptionsMenu.wnd:ComboBoxPlayer{}", i));
            self.reset_combo_box(self.combo_box_player_id[i]);
            self.set_player_tooltip(self.combo_box_player_id[i]);

            // Setup player combo box
            if local_slot_num == i as i32 {
                let my_name = self.get_my_name();
                self.add_combo_box_entry(self.combo_box_player_id[i], &my_name);
            } else {
                self.add_combo_box_entry(self.combo_box_player_id[i], "Open");
                self.add_combo_box_entry(self.combo_box_player_id[i], "Closed");
                self.add_combo_box_entry(self.combo_box_player_id[i], "Easy AI");
                self.add_combo_box_entry(self.combo_box_player_id[i], "Medium AI");
                self.add_combo_box_entry(self.combo_box_player_id[i], "Hard AI");
                self.set_combo_box_selected_pos(self.combo_box_player_id[i], 0);
            }

            // Setup color combo box
            self.combo_box_color_id[i] = self.get_window_id(&format!("LanGameOptionsMenu.wnd:ComboBoxColor{}", i));
            self.populate_color_combo_box(i);
            self.set_combo_box_selected_pos(self.combo_box_color_id[i], 0);

            // Setup faction combo box
            self.combo_box_player_template_id[i] = self.get_window_id(&format!("LanGameOptionsMenu.wnd:ComboBoxPlayerTemplate{}", i));
            self.populate_player_template_combo_box(i, true);
            self.set_player_template_tooltip(self.combo_box_player_template_id[i]);

            // Setup team combo box
            self.combo_box_team_id[i] = self.get_window_id(&format!("LanGameOptionsMenu.wnd:ComboBoxTeam{}", i));
            self.populate_team_combo_box(i);

            // Setup accept button
            self.button_accept_id[i] = self.get_window_id(&format!("LanGameOptionsMenu.wnd:ButtonAccept{}", i));
            self.set_accept_tooltip(self.button_accept_id[i]);

            // Setup map start position button
            self.button_map_start_position_id[i] = self.get_window_id(&format!("LanGameOptionsMenu.wnd:ButtonMapStartPosition{}", i));

            // Hide accept buttons for non-local players initially
            if i != 0 {
                self.hide_window(self.button_accept_id[i], true);
            }
        }

        // Set accept button color for local player
        self.set_button_enabled_color(self.button_accept_id[0], 0xFF00FF00); // acceptTrueColor
    }

    /// Deinitialize gadgets
    fn deinit_lan_game_gadgets(&mut self) {
        // Clear all window references
        self.parent_lan_game_options_id = 0;
        self.button_emote_id = 0;
        self.button_select_map_id = 0;
        self.button_start_id = 0;
        self.button_back_id = 0;
        self.listbox_chat_window_lan_game_id = 0;
        self.text_entry_chat_id = 0;
        self.text_entry_map_display_id = 0;
        self.checkbox_limit_superweapons_id = 0;
        self.combo_box_starting_cash_id = 0;
        self.window_map_id = 0;

        for i in 0..MAX_SLOTS {
            self.combo_box_player_id[i] = 0;
            self.combo_box_color_id[i] = 0;
            self.combo_box_player_template_id[i] = 0;
            self.combo_box_team_id[i] = 0;
            self.button_accept_id[i] = 0;
            self.button_map_start_position_id[i] = 0;
        }
    }

    /// Shutdown the LAN Game Options Menu
    pub fn shutdown(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        self.set_mouse_cursor("ARROW");
        self.clear_mouse_text();
        self.enable_slot_list_updates(false);
        self.is_shutting_down = true;

        let pop_immediate = user_data.is_some();

        if pop_immediate {
            self.shutdown_complete(layout);
            return;
        }

        self.reverse_animate_window();
        self.reverse_transition("LanGameOptionsFade");
        self.reset_game_start_timer();
    }

    /// Update the LAN Game Options Menu
    pub fn update(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        if self.is_shutting_down && self.is_anim_finished() && self.is_transition_finished() {
            self.shutdown_complete(layout);
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
                    self.send_system_message(
                        window_id,
                        0x0201, // GBM_SELECTED
                        self.button_back_id,
                        self.button_back_id as usize,
                    );
                }
                return true;
            }
        }

        false
    }

    /// Handle system messages
    pub fn handle_system(&mut self, window_id: i32, msg: u32, data1: usize, data2: usize) -> bool {
        const GWM_CREATE: u32 = 0x0001;
        const GWM_DESTROY: u32 = 0x0002;
        const GWM_INPUT_FOCUS: u32 = 0x0003;
        const GCM_SELECTED: u32 = 0x0303;
        const GBM_SELECTED: u32 = 0x0201;
        const GBM_SELECTED_RIGHT: u32 = 0x0202;
        const GEM_EDIT_DONE: u32 = 0x0402;

        match msg {
            GWM_CREATE => true,
            GWM_DESTROY => true,
            GWM_INPUT_FOCUS => true,
            GCM_SELECTED => self.handle_combo_selected(data1 as i32, data2 as i32),
            GBM_SELECTED => self.handle_button_selected(data1 as i32, window_id),
            GBM_SELECTED_RIGHT => self.handle_button_selected_right(data1 as i32),
            GEM_EDIT_DONE => self.handle_edit_done(data1 as i32),
            _ => false,
        }
    }

    /// Handle combo box selection
    fn handle_combo_selected(&mut self, control_id: i32, selected_pos: i32) -> bool {
        if self.button_pushed {
            return true;
        }

        // Check starting cash combo
        if control_id == self.combo_box_starting_cash_id {
            self.handle_starting_cash_selection();
            return true;
        }

        // Check per-player combos
        for i in 0..MAX_SLOTS {
            if control_id == self.combo_box_color_id[i] {
                self.handle_color_selection(i);
                return true;
            } else if control_id == self.combo_box_player_template_id[i] {
                self.handle_player_template_selection(i);
                return true;
            } else if control_id == self.combo_box_team_id[i] {
                self.handle_team_selection(i);
                return true;
            } else if control_id == self.combo_box_player_id[i] && self.am_i_host() {
                self.handle_player_selection(i, selected_pos);
                return true;
            }
        }

        true
    }

    /// Handle button selection
    fn handle_button_selected(&mut self, control_id: i32, window_id: i32) -> bool {
        if self.button_pushed {
            return true;
        }

        if control_id == self.button_back_id {
            self.destroy_map_select_layout();
            self.request_game_leave();
        } else if control_id == self.button_emote_id {
            let text = self.get_text_entry(self.text_entry_chat_id);
            self.set_text_entry(self.text_entry_chat_id, "");
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                self.request_chat(trimmed, true); // LANCHAT_EMOTE
            }
        } else if control_id == self.button_select_map_id {
            self.create_map_select_layout();
        } else if control_id == self.button_start_id {
            if self.am_i_host() {
                self.start_pressed();
            } else {
                self.request_accept();
                self.enable_accept_controls(true);
            }
        } else if control_id == self.checkbox_limit_superweapons_id {
            self.handle_limit_superweapons_click();
        } else {
            // Check map start position buttons
            for i in 0..MAX_SLOTS {
                if control_id == self.button_map_start_position_id[i] {
                    self.handle_map_start_position_click(i);
                    return true;
                }
            }
        }

        true
    }

    /// Handle right-click on button
    fn handle_button_selected_right(&mut self, control_id: i32) -> bool {
        if self.button_pushed {
            return true;
        }

        for i in 0..MAX_SLOTS {
            if control_id == self.button_map_start_position_id[i] {
                self.handle_map_start_position_right_click(i);
                return true;
            }
        }

        true
    }

    /// Handle edit done
    fn handle_edit_done(&mut self, control_id: i32) -> bool {
        if self.button_pushed {
            return true;
        }

        if control_id == self.text_entry_chat_id {
            let text = self.get_text_entry(self.text_entry_chat_id);
            self.set_text_entry(self.text_entry_chat_id, "");
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                self.request_chat(trimmed, false); // LANCHAT_NORMAL
            }
        }

        true
    }

    /// Start button pressed
    fn start_pressed(&mut self) {
        if !self.validate_game_start() {
            return;
        }

        // Set local player as accepted
        self.set_slot_accepted(0, true);

        let (num_users, num_humans) = self.count_players();

        // Check for too many players
        if !self.check_max_players(num_users) {
            return;
        }

        // Check for observer + AI only
        if !self.check_human_players(num_humans) {
            return;
        }

        // Check for too few players
        if !self.check_min_players(num_users) {
            return;
        }

        // Check for too few teams
        if !self.check_min_teams() {
            return;
        }

        // Check if all players accepted
        let (is_ready, all_have_map) = self.check_all_accepted();

        if is_ready {
            // Close all open slots
            for i in 0..MAX_SLOTS {
                if self.is_slot_open(i) {
                    self.set_slot_state(i, SlotState::Closed);
                    self.set_combo_box_selected_pos(self.combo_box_player_id[i], SlotState::Closed as i32);
                }
            }

            // Start game with countdown timer
            let countdown_seconds = self.get_start_countdown_timer_seconds();
            if countdown_seconds > 0 {
                self.request_game_start_timer(countdown_seconds);
            } else {
                self.request_game_start();
            }
            self.lan_enable_start_button(false);
        } else {
            if all_have_map {
                self.add_chat_system_text("Notified players of start intent");
                self.request_accept();
            }
        }
    }

    /// Update game options display
    fn update_game_options(&mut self) {
        if !self.are_slot_list_updates_enabled() {
            return;
        }

        // Update map display name
        let map_name = self.get_game_map();
        let map_display_name = self.get_map_display_name(&map_name);
        self.set_static_text(self.text_entry_map_display_id, &map_display_name);

        // Update superweapon restriction checkbox
        let restricted = self.get_superweapon_restriction() != 0;
        self.set_checkbox_checked(self.checkbox_limit_superweapons_id, restricted);

        // Update starting cash combo box
        let starting_cash = self.get_starting_cash();
        self.select_starting_cash_in_combo_box(starting_cash);
    }

    /// Update slot list
    pub fn lan_update_slot_list(&mut self) {
        if !self.are_slot_list_updates_enabled() || self.is_initing {
            return;
        }

        self.update_slot_list_internal();
        self.update_map_start_spots();
    }

    /// Shutdown complete
    fn shutdown_complete(&mut self, layout: &mut WindowLayout) {
        self.deinit_lan_game_gadgets();
        self.is_shutting_down = false;

        layout.hide(true);

        let has_next_screen = self.next_screen.is_some();
        self.shell_shutdown_complete(layout, has_next_screen);

        if let Some(next_screen) = &self.next_screen {
            self.push_menu(next_screen);
        }

        self.next_screen = None;
    }

    // Placeholder helper functions - would integrate with actual game engine
    fn get_window_id(&self, name: &str) -> i32 { 0 }
    fn set_text_entry(&self, id: i32, text: &str) {}
    fn get_text_entry(&self, id: i32) -> String { String::new() }
    fn reset_listbox(&self, id: i32) {}
    fn reset_combo_box(&self, id: i32) {}
    fn set_focus(&self, id: i32) {}
    fn hide_window(&self, id: i32, hide: bool) {}
    fn enable_window(&self, id: i32, enable: bool) {}
    fn enable_button(&self, id: i32, enable: bool) {}
    fn enable_combo_box(&self, id: i32, enable: bool) {}
    fn set_button_text(&self, id: i32, text: &str) {}
    fn set_static_text(&self, id: i32, text: &str) {}
    fn set_checkbox_checked(&self, id: i32, checked: bool) {}
    fn add_combo_box_entry(&self, id: i32, text: &str) {}
    fn set_combo_box_selected_pos(&self, id: i32, pos: i32) {}
    fn set_button_enabled_color(&self, id: i32, color: u32) {}
    fn set_player_tooltip(&self, id: i32) {}
    fn set_player_template_tooltip(&self, id: i32) {}
    fn set_accept_tooltip(&self, id: i32) {}
    fn set_map_tooltip(&self, id: i32) {}
    fn set_mouse_cursor(&self, cursor: &str) {}
    fn clear_mouse_text(&self) {}
    fn enable_slot_list_updates(&self, enable: bool) {}
    fn are_slot_list_updates_enabled(&self) -> bool { true }
    fn populate_starting_cash_combo_box(&self) {}
    fn populate_color_combo_box(&self, slot: usize) {}
    fn populate_player_template_combo_box(&self, slot: usize, include_observer: bool) {}
    fn populate_team_combo_box(&self, slot: usize) {}
    fn update_map_cache(&self) {}
    fn am_i_host(&self) -> bool { false }
    fn get_local_slot_num(&self) -> i32 { 0 }
    fn get_my_name(&self) -> String { String::new() }
    fn set_local_player_color(&self, color: i32) {}
    fn set_local_player_template(&self, template: i32) {}
    fn set_local_player_nat_behavior(&self, behavior: i32) {}
    fn set_local_player_map_availability(&self, available: bool) {}
    fn set_game_map(&self, map: &str) {}
    fn get_game_map(&self) -> String { String::new() }
    fn set_game_map_crc(&self, crc: u32) {}
    fn get_game_map_crc(&self) -> u32 { 0 }
    fn set_game_map_size(&self, size: u32) {}
    fn get_game_map_size(&self) -> u32 { 0 }
    fn set_starting_cash(&self, cash: u32) {}
    fn get_starting_cash(&self) -> u32 { 0 }
    fn set_superweapon_restriction(&self, restriction: i32) {}
    fn get_superweapon_restriction(&self) -> i32 { 0 }
    fn find_map(&self, name: &str) -> Option<MapMetaData> { None }
    fn adjust_slots_for_map(&self) {}
    fn request_has_map(&self) {}
    fn request_game_options(&self, options: &str, announce: bool) {}
    fn request_game_announce(&self) {}
    fn request_game_leave(&self) {}
    fn request_game_start(&self) {}
    fn request_game_start_timer(&self, seconds: i32) {}
    fn request_accept(&self) {}
    fn request_chat(&self, text: &str, is_emote: bool) {}
    fn generate_game_options_string(&self) -> String { String::new() }
    fn set_transition_group(&self, group: &str) {}
    fn lan_position_start_spots(&self) {}
    fn lan_enable_start_button(&self, enable: bool) {}
    fn is_game_in_progress(&self) -> bool { false }
    fn pop_immediate(&self) {}
    fn is_anim_finished(&self) -> bool { true }
    fn is_transition_finished(&self) -> bool { true }
    fn reverse_animate_window(&self) {}
    fn reverse_transition(&self, name: &str) {}
    fn reset_game_start_timer(&self) {}
    fn send_system_message(&self, window: i32, msg: u32, data1: i32, data2: usize) {}
    fn push_menu(&self, name: &str) {}
    fn shell_shutdown_complete(&self, layout: &WindowLayout, has_next: bool) {}
    fn create_map_select_layout(&self) {}
    fn destroy_map_select_layout(&self) {}
    fn enable_accept_controls(&self, enable: bool) {}
    fn add_chat_system_text(&self, text: &str) {}
    fn get_map_display_name(&self, map: &str) -> String { String::new() }
    fn select_starting_cash_in_combo_box(&self, cash: u32) {}
    fn update_slot_list_internal(&self) {}
    fn update_map_start_spots(&self) {}
    fn get_start_countdown_timer_seconds(&self) -> i32 { 0 }
    fn set_slot_accepted(&self, slot: usize, accepted: bool) {}
    fn set_slot_state(&self, slot: usize, state: SlotState) {}
    fn is_slot_open(&self, slot: usize) -> bool { false }

    fn handle_color_selection(&mut self, slot: usize) {}
    fn handle_player_template_selection(&mut self, slot: usize) {}
    fn handle_team_selection(&mut self, slot: usize) {}
    fn handle_player_selection(&mut self, slot: usize, pos: i32) {}
    fn handle_starting_cash_selection(&mut self) {}
    fn handle_limit_superweapons_click(&mut self) {}
    fn handle_map_start_position_click(&mut self, pos: usize) {}
    fn handle_map_start_position_right_click(&mut self, pos: usize) {}

    fn validate_game_start(&self) -> bool { true }
    fn count_players(&self) -> (usize, usize) { (0, 0) }
    fn check_max_players(&self, num: usize) -> bool { true }
    fn check_human_players(&self, num: usize) -> bool { true }
    fn check_min_players(&self, num: usize) -> bool { true }
    fn check_min_teams(&self) -> bool { true }
    fn check_all_accepted(&self) -> (bool, bool) { (false, false) }
}

/// Map metadata structure
pub struct MapMetaData {
    pub crc: u32,
    pub file_size: u32,
    pub num_players: usize,
    pub is_official: bool,
}

/// Window Layout placeholder
pub struct WindowLayout;

impl WindowLayout {
    pub fn hide(&mut self, hidden: bool) {}
    pub fn bring_forward(&mut self) {}
}
