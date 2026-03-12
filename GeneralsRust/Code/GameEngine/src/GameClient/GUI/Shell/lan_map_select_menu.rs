// FILE: lan_map_select_menu.rs
// Author: Ported from C++ by Claude, November 2024
// Description: LAN Map Select Menu - faithful port from C++

const MAX_SLOTS: usize = 8;

/// LAN Map Select Menu State
pub struct LANMapSelectMenu {
    // Window IDs
    button_back_id: i32,
    button_ok_id: i32,
    listbox_map_id: i32,
    win_map_preview_id: i32,
    radio_button_system_maps_id: i32,
    radio_button_user_maps_id: i32,
    button_map_start_position_id: [i32; MAX_SLOTS],

    // Parent window
    parent_id: i32,

    // Map select layout reference
    map_select_layout: Option<*mut super::lan_game_options_menu::WindowLayout>,
}

impl LANMapSelectMenu {
    pub fn new() -> Self {
        LANMapSelectMenu {
            button_back_id: 0,
            button_ok_id: 0,
            listbox_map_id: 0,
            win_map_preview_id: 0,
            radio_button_system_maps_id: 0,
            radio_button_user_maps_id: 0,
            button_map_start_position_id: [0; MAX_SLOTS],
            parent_id: 0,
            map_select_layout: None,
        }
    }

    /// Initialize the LAN Map Select Menu
    pub fn init(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Hide underlying LAN Game Options GUI elements
        self.show_lan_game_options_underlying_gui_elements(false);

        // Set keyboard focus to parent
        self.parent_id = self.get_window_id("LanMapSelectMenu.wnd:LanMapSelectMenuParent");
        self.set_focus(self.parent_id);

        // Get preferences
        let prefs = super::lan_lobby_menu::LANPreferences::new();
        let mut uses_system_map_dir = prefs.uses_system_map_dir();

        // Check current map to determine if it's official
        let current_map = self.get_current_game_map();
        if let Some(map_data) = self.find_map(&current_map) {
            uses_system_map_dir = map_data.is_official;
        }

        // Get window IDs
        self.button_back_id = self.get_window_id("LanMapSelectMenu.wnd:ButtonBack");
        self.button_ok_id = self.get_window_id("LanMapSelectMenu.wnd:ButtonOK");
        self.listbox_map_id = self.get_window_id("LanMapSelectMenu.wnd:ListboxMap");
        self.win_map_preview_id = self.get_window_id("LanMapSelectMenu.wnd:WinMapPreview");
        self.radio_button_system_maps_id = self.get_window_id("LanMapSelectMenu.wnd:RadioButtonSystemMaps");
        self.radio_button_user_maps_id = self.get_window_id("LanMapSelectMenu.wnd:RadioButtonUserMaps");

        // Set radio button selection
        if uses_system_map_dir {
            self.set_radio_selection(self.radio_button_system_maps_id, false);
        } else {
            self.set_radio_selection(self.radio_button_user_maps_id, false);
        }

        // Initialize map start position buttons
        for i in 0..MAX_SLOTS {
            let button_name = format!("LanMapSelectMenu.wnd:ButtonMapStartPosition{}", i);
            self.button_map_start_position_id[i] = self.get_window_id(&button_name);

            // Hide and disable start position buttons
            self.hide_window(self.button_map_start_position_id[i], true);
            self.enable_window(self.button_map_start_position_id[i], false);
        }

        // Update map cache
        self.update_map_cache();

        // Populate map listbox
        self.populate_map_listbox(uses_system_map_dir, true, &current_map);
    }

    /// Shutdown the LAN Map Select Menu
    pub fn shutdown(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Hide menu
        layout.hide(true);

        // Nullify controls
        self.nullify_controls();

        // Our shutdown is complete
        self.shell_shutdown_complete(layout);
    }

    /// Update the LAN Map Select Menu
    pub fn update(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // No periodic updates needed
    }

    /// Handle input messages
    pub fn handle_input(&mut self, window_id: i32, msg: u32, data1: usize, data2: usize) -> bool {
        const GWM_CHAR: u32 = 0x0102;
        const KEY_ESC: u8 = 0x1B;
        const KEY_STATE_UP: u8 = 0x01;

        if msg == GWM_CHAR {
            let key = (data1 & 0xFF) as u8;
            let state = (data2 & 0xFF) as u8;

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
        const GLM_DOUBLE_CLICKED: u32 = 0x0301;
        const GBM_SELECTED: u32 = 0x0201;
        const GLM_SELECTED: u32 = 0x0302;

        match msg {
            GWM_CREATE => true,
            GWM_DESTROY => {
                self.nullify_controls();
                true
            }
            GWM_INPUT_FOCUS => true,
            GLM_DOUBLE_CLICKED => self.handle_listbox_double_clicked(data1 as i32, data2 as i32),
            GBM_SELECTED => self.handle_button_selected(data1 as i32),
            GLM_SELECTED => self.handle_listbox_selected(data1 as i32, data2 as i32),
            _ => false,
        }
    }

    /// Handle listbox double-click
    fn handle_listbox_double_clicked(&mut self, control_id: i32, row_selected: i32) -> bool {
        if control_id == self.listbox_map_id && row_selected >= 0 {
            self.set_listbox_selected(control_id, row_selected);

            // Simulate OK button click
            self.send_system_message(
                control_id,
                0x0201, // GBM_SELECTED
                self.button_ok_id,
                self.button_ok_id as usize,
            );
        }

        true
    }

    /// Handle button selection
    fn handle_button_selected(&mut self, control_id: i32) -> bool {
        if control_id == self.radio_button_system_maps_id {
            self.update_map_cache();
            let current_map = self.get_current_game_map();
            self.populate_map_listbox(true, true, &current_map);

            // Save preference
            let mut prefs = super::lan_lobby_menu::LANPreferences::new();
            prefs.set("UseSystemMapDir".to_string(), "yes".to_string());
            prefs.write();
        } else if control_id == self.radio_button_user_maps_id {
            self.update_map_cache();
            let current_map = self.get_current_game_map();
            self.populate_map_listbox(false, true, &current_map);

            // Save preference
            let mut prefs = super::lan_lobby_menu::LANPreferences::new();
            prefs.set("UseSystemMapDir".to_string(), "no".to_string());
            prefs.write();
        } else if control_id == self.button_back_id {
            // Destroy layout and return to game options
            self.destroy_map_select_layout();
            self.nullify_controls();
            self.show_lan_game_options_underlying_gui_elements(true);
            self.post_to_lan_game_options(PostToLanGameType::MapBack);
        } else if control_id == self.button_ok_id {
            let selected = self.get_listbox_selected(self.listbox_map_id);

            if selected != -1 {
                // Get selected map name
                let map_display_name = self.get_listbox_text(self.listbox_map_id, selected, 0);

                // Get map filename from item data
                let map_filename = self.get_listbox_item_data_string(self.listbox_map_id, selected);

                // Set map in game
                self.set_game_map(&map_filename);

                // Find map metadata
                if let Some(map_data) = self.find_map(&map_filename.to_lowercase()) {
                    self.set_local_player_map_availability(true);
                    self.set_game_map_crc(map_data.crc);
                    self.set_game_map_size(map_data.file_size);

                    // Reset start spots and adjust slots for new map
                    self.reset_start_spots();
                    self.adjust_slots_for_map();
                }

                // Destroy layout and return to game options
                self.destroy_map_select_layout();
                self.nullify_controls();
                self.show_lan_game_options_underlying_gui_elements(true);
                self.post_to_lan_game_options(PostToLanGameType::SendGameOpts);
            }
        }

        true
    }

    /// Handle listbox selection
    fn handle_listbox_selected(&mut self, control_id: i32, row_selected: i32) -> bool {
        if control_id == self.listbox_map_id {
            if row_selected < 0 {
                // Clear map preview
                self.position_start_spots("", &self.button_map_start_position_id);
                return true;
            }

            // Show map preview
            self.set_window_status(self.win_map_preview_id, true);

            // Get map filename
            let map_filename = self.get_listbox_item_data_string(self.listbox_map_id, row_selected);

            // Load and display map preview image
            if let Some(image) = self.get_map_preview_image(&map_filename.to_lowercase()) {
                self.set_window_enabled_image(self.win_map_preview_id, 0, image);
            } else {
                self.clear_window_status(self.win_map_preview_id);
            }

            // Set map metadata as user data
            if let Some(map_data) = self.find_map(&map_filename.to_lowercase()) {
                self.set_window_user_data(self.win_map_preview_id, Box::new(map_data));
            }

            // Position start spots on preview
            self.position_start_spots(&map_filename.to_lowercase(), &self.button_map_start_position_id);
        }

        true
    }

    /// Nullify controls (clear references)
    fn nullify_controls(&mut self) {
        self.listbox_map_id = 0;
        self.win_map_preview_id = 0;
        self.parent_id = 0;

        for i in 0..MAX_SLOTS {
            self.button_map_start_position_id[i] = 0;
        }
    }

    /// Show/hide underlying LAN Game Options GUI elements
    fn show_lan_game_options_underlying_gui_elements(&self, show: bool) {
        // List of gadgets to hide/show
        let gadgets_to_hide = [
            "MapWindow",
            "StaticTextTeam",
            "StaticTextFaction",
            "StaticTextColor",
            "TextEntryMapDisplay",
            "ButtonSelectMap",
            "ButtonStart",
            "StaticTextMapPreview",
        ];

        let per_player_gadgets_to_hide = [
            "ComboBoxTeam",
            "ComboBoxColor",
            "ComboBoxPlayerTemplate",
        ];

        // Hide/show main gadgets
        for gadget in &gadgets_to_hide {
            let full_name = format!("LanGameOptionsMenu.wnd:{}", gadget);
            let window_id = self.get_window_id(&full_name);
            if window_id != 0 {
                self.hide_window(window_id, !show);
            }
        }

        // Hide/show per-player gadgets
        for i in 0..MAX_SLOTS {
            for gadget in &per_player_gadgets_to_hide {
                let full_name = format!("LanGameOptionsMenu.wnd:{}{}", gadget, i);
                let window_id = self.get_window_id(&full_name);
                if window_id != 0 {
                    self.hide_window(window_id, !show);
                }
            }
        }

        // Enable/disable back button
        let back_button_id = self.get_window_id("LanGameOptionsMenu.wnd:ButtonBack");
        if back_button_id != 0 {
            self.enable_window(back_button_id, show);
        }
    }

    /// Populate map listbox
    fn populate_map_listbox(&self, use_system_dir: bool, is_lan: bool, current_map: &str) {
        // This would integrate with the actual map cache and populate the listbox
        // with available maps, marking the current map as selected
        self.reset_listbox(self.listbox_map_id);

        // Get map list from cache
        let maps = self.get_map_list(use_system_dir, is_lan);

        let mut selected_index = -1;

        for (index, (map_filename, map_display_name)) in maps.iter().enumerate() {
            // Add map to listbox
            self.add_listbox_entry(self.listbox_map_id, map_display_name);

            // Store filename as item data
            self.set_listbox_item_data_string(self.listbox_map_id, index as i32, map_filename);

            // Check if this is the current map
            if map_filename.eq_ignore_ascii_case(current_map) {
                selected_index = index as i32;
            }
        }

        // Select current map if found
        if selected_index >= 0 {
            self.set_listbox_selected(self.listbox_map_id, selected_index);

            // Trigger selection to show preview
            self.send_system_message(
                self.listbox_map_id,
                0x0302, // GLM_SELECTED
                self.listbox_map_id,
                selected_index as usize,
            );
        }
    }

    /// Position start spots on map preview
    fn position_start_spots(&self, map_name: &str, button_ids: &[i32; MAX_SLOTS]) {
        if map_name.is_empty() {
            // Hide all start position buttons
            for &button_id in button_ids {
                if button_id != 0 {
                    self.hide_window(button_id, true);
                }
            }
            return;
        }

        // Get map metadata
        if let Some(map_data) = self.find_map(map_name) {
            // Get map preview window size
            let (map_width, map_height) = self.get_window_size(self.win_map_preview_id);

            // Position start spots based on map data
            for i in 0..map_data.num_players {
                if i < MAX_SLOTS && button_ids[i] != 0 {
                    // Get start position from map data
                    if let Some((x, y)) = map_data.start_positions.get(i) {
                        // Calculate button position (scaled to preview window)
                        let button_x = (*x as f32 / map_data.width as f32 * map_width as f32) as i32;
                        let button_y = (*y as f32 / map_data.height as f32 * map_height as f32) as i32;

                        // Position and show button
                        self.set_window_position(button_ids[i], button_x, button_y);
                        self.hide_window(button_ids[i], false);
                        self.enable_window(button_ids[i], false); // Not clickable in map select
                    }
                }
            }

            // Hide unused start position buttons
            for i in map_data.num_players..MAX_SLOTS {
                if button_ids[i] != 0 {
                    self.hide_window(button_ids[i], true);
                }
            }
        }
    }

    // Placeholder helper functions - would integrate with actual game engine
    fn get_window_id(&self, name: &str) -> i32 { 0 }
    fn set_focus(&self, id: i32) {}
    fn hide_window(&self, id: i32, hide: bool) {}
    fn enable_window(&self, id: i32, enable: bool) {}
    fn set_radio_selection(&self, id: i32, group: bool) {}
    fn reset_listbox(&self, id: i32) {}
    fn add_listbox_entry(&self, id: i32, text: &str) -> i32 { 0 }
    fn get_listbox_selected(&self, id: i32) -> i32 { -1 }
    fn set_listbox_selected(&self, id: i32, index: i32) {}
    fn get_listbox_text(&self, id: i32, row: i32, col: i32) -> String { String::new() }
    fn get_listbox_item_data_string(&self, id: i32, index: i32) -> String { String::new() }
    fn set_listbox_item_data_string(&self, id: i32, index: i32, data: &str) {}
    fn set_window_status(&self, id: i32, has_image: bool) {}
    fn clear_window_status(&self, id: i32) {}
    fn set_window_enabled_image(&self, id: i32, index: i32, image: MapImage) {}
    fn set_window_user_data(&self, id: i32, data: Box<dyn std::any::Any>) {}
    fn set_window_position(&self, id: i32, x: i32, y: i32) {}
    fn get_window_size(&self, id: i32) -> (i32, i32) { (0, 0) }
    fn send_system_message(&self, window: i32, msg: u32, data1: i32, data2: usize) {}
    fn shell_shutdown_complete(&self, layout: &WindowLayout) {}
    fn destroy_map_select_layout(&self) {}
    fn update_map_cache(&self) {}
    fn get_current_game_map(&self) -> String { String::new() }
    fn set_game_map(&self, map: &str) {}
    fn set_local_player_map_availability(&self, available: bool) {}
    fn set_game_map_crc(&self, crc: u32) {}
    fn set_game_map_size(&self, size: u32) {}
    fn reset_start_spots(&self) {}
    fn adjust_slots_for_map(&self) {}
    fn find_map(&self, name: &str) -> Option<MapMetaData> { None }
    fn get_map_preview_image(&self, name: &str) -> Option<MapImage> { None }
    fn get_map_list(&self, system_dir: bool, is_lan: bool) -> Vec<(String, String)> { Vec::new() }
    fn post_to_lan_game_options(&self, msg_type: PostToLanGameType) {}
}

/// Post message types for LAN Game Options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostToLanGameType {
    SendGameOpts,
    MapBack,
}

/// Map metadata structure
pub struct MapMetaData {
    pub crc: u32,
    pub file_size: u32,
    pub num_players: usize,
    pub is_official: bool,
    pub width: u32,
    pub height: u32,
    pub start_positions: Vec<(i32, i32)>,
}

/// Map image placeholder
pub struct MapImage;

/// Window Layout placeholder
pub struct WindowLayout;

impl WindowLayout {
    pub fn hide(&mut self, hidden: bool) {}
    pub fn bring_forward(&mut self) {}
}
