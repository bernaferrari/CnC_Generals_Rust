// FILE: wol_lobby_menu.rs
// Author: Ported from C++ by Claude, November 2024
// Description: WOL (Westwood Online / GameSpy) Lobby Menu - faithful port from C++

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, Duration};

const MAX_SLOTS: usize = 8;
const COLUMN_PLAYERNAME: usize = 1;

/// Peer response types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerResponseType {
    Login,
    Disconnect,
    Message,
    GroupRoom,
    StagingRoom,
    StagingRoomPlayerInfo,
    JoinGroupRoom,
    CreateStagingRoom,
    JoinStagingRoom,
    PlayerJoin,
    PlayerLeft,
    PlayerChangedNick,
    PlayerInfo,
    PlayerChangedFlags,
    RoomUTM,
    PlayerUTM,
    QuickMatchStatus,
    GameStart,
    FailedToHost,
    StagingRoomListComplete,
}

/// Player room type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    GroupRoom,
    StagingRoom,
}

/// Player information
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub name: String,
    pub profile_id: u32,
    pub flags: u32,
    pub wins: i32,
    pub losses: i32,
    pub locale: String,
    pub rank_points: i32,
    pub side: i32,
    pub preorder: bool,
    pub ignored: bool,
}

impl PlayerInfo {
    pub fn new() -> Self {
        PlayerInfo {
            name: String::new(),
            profile_id: 0,
            flags: 0,
            wins: 0,
            losses: 0,
            locale: String::new(),
            rank_points: 0,
            side: 0,
            preorder: false,
            ignored: false,
        }
    }

    pub fn is_ignored(&self) -> bool {
        self.ignored
    }
}

/// GameSpy group room
#[derive(Debug, Clone)]
pub struct GameSpyGroupRoom {
    pub group_id: i32,
    pub translated_name: String,
}

/// GameSpy staging room (game lobby)
#[derive(Debug, Clone)]
pub struct GameSpyStagingRoom {
    pub id: i32,
    pub game_name: String,
    pub map_name: String,
    pub has_password: bool,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub version: i32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub ping_string: String,
    pub ladder_ip: String,
    pub ladder_port: u16,
    pub reported_num_players: usize,
    pub reported_max_players: usize,
    pub reported_num_observers: usize,
}

/// Peer response message
#[derive(Debug, Clone)]
pub struct PeerResponse {
    pub response_type: PeerResponseType,
    pub nick: String,
    pub text: String,
    pub locale: String,
    pub password: String,
    pub staging_server_name: String,
    pub staging_room_map_name: String,
    pub staging_server_game_options: String,
    pub staging_server_ping_string: String,
    pub staging_server_ladder_ip: String,
    pub staging_room_player_names: [String; MAX_SLOTS],
    pub player: PlayerInfo,
    pub message_profile_id: u32,
    pub message_is_private: bool,
    pub message_is_action: bool,
    pub join_group_room_ok: bool,
    pub join_group_room_id: i32,
    pub create_staging_room_result: i32,
    pub join_staging_room_ok: bool,
    pub join_staging_room_result: i32,
    pub disconnect_reason: i32,
    pub staging_room_action: i32,
    pub staging_room_id: i32,
    pub staging_room_percent_complete: i32,
    pub staging_room_requires_password: bool,
    pub staging_room_version: i32,
    pub staging_room_exe_crc: u32,
    pub staging_room_ini_crc: u32,
    pub staging_room_allow_observers: bool,
    pub staging_room_use_stats: bool,
    pub staging_room_ladder_port: u16,
    pub staging_room_num_players: usize,
    pub staging_room_max_players: usize,
    pub staging_room_num_observers: usize,
    pub staging_room_wins: [i32; MAX_SLOTS],
    pub staging_room_losses: [i32; MAX_SLOTS],
    pub staging_room_profile_id: [i32; MAX_SLOTS],
    pub staging_room_faction: [i32; MAX_SLOTS],
    pub staging_room_color: [i32; MAX_SLOTS],
    pub player_room_type: RoomType,
}

impl PeerResponse {
    pub fn new() -> Self {
        PeerResponse {
            response_type: PeerResponseType::Message,
            nick: String::new(),
            text: String::new(),
            locale: String::new(),
            password: String::new(),
            staging_server_name: String::new(),
            staging_room_map_name: String::new(),
            staging_server_game_options: String::new(),
            staging_server_ping_string: String::new(),
            staging_server_ladder_ip: String::new(),
            staging_room_player_names: Default::default(),
            player: PlayerInfo::new(),
            message_profile_id: 0,
            message_is_private: false,
            message_is_action: false,
            join_group_room_ok: false,
            join_group_room_id: 0,
            create_staging_room_result: 0,
            join_staging_room_ok: false,
            join_staging_room_result: 0,
            disconnect_reason: 0,
            staging_room_action: 0,
            staging_room_id: 0,
            staging_room_percent_complete: 0,
            staging_room_requires_password: false,
            staging_room_version: 0,
            staging_room_exe_crc: 0,
            staging_room_ini_crc: 0,
            staging_room_allow_observers: false,
            staging_room_use_stats: false,
            staging_room_ladder_port: 0,
            staging_room_num_players: 0,
            staging_room_max_players: 0,
            staging_room_num_observers: 0,
            staging_room_wins: [0; MAX_SLOTS],
            staging_room_losses: [0; MAX_SLOTS],
            staging_room_profile_id: [0; MAX_SLOTS],
            staging_room_faction: [0; MAX_SLOTS],
            staging_room_color: [0; MAX_SLOTS],
            player_room_type: RoomType::GroupRoom,
        }
    }
}

/// WOL Lobby Menu State
pub struct WOLLobbyMenu {
    // Window IDs
    parent_wol_lobby_id: i32,
    button_back_id: i32,
    button_host_id: i32,
    button_refresh_id: i32,
    button_join_id: i32,
    button_buddy_id: i32,
    button_emote_id: i32,
    text_entry_chat_id: i32,
    listbox_lobby_players_id: i32,
    listbox_lobby_chat_id: i32,
    combo_lobby_group_rooms_id: i32,

    // State flags
    is_shutting_down: bool,
    button_pushed: bool,
    next_screen: Option<String>,
    raise_message_boxes: bool,
    trying_to_host_or_join: bool,

    // Timing
    game_list_refresh_time: SystemTime,
    player_list_refresh_time: SystemTime,
    game_list_refresh_interval: Duration,
    player_list_refresh_interval: Duration,

    // Data
    group_room_to_join: i32,
    initial_gadget_delay: i32,
    just_entered: bool,

    // Player and game lists
    player_info_map: HashMap<String, PlayerInfo>,
    buddy_map: HashSet<u32>,
    group_room_list: HashMap<i32, GameSpyGroupRoom>,
    staging_room_list: HashMap<i32, GameSpyStagingRoom>,

    // Queued UTMs (user-to-user messages)
    lobby_queued_utms: Vec<PeerResponse>,
}

impl WOLLobbyMenu {
    pub fn new() -> Self {
        WOLLobbyMenu {
            parent_wol_lobby_id: 0,
            button_back_id: 0,
            button_host_id: 0,
            button_refresh_id: 0,
            button_join_id: 0,
            button_buddy_id: 0,
            button_emote_id: 0,
            text_entry_chat_id: 0,
            listbox_lobby_players_id: 0,
            listbox_lobby_chat_id: 0,
            combo_lobby_group_rooms_id: 0,
            is_shutting_down: false,
            button_pushed: false,
            next_screen: None,
            raise_message_boxes: false,
            trying_to_host_or_join: false,
            game_list_refresh_time: SystemTime::now(),
            player_list_refresh_time: SystemTime::now(),
            game_list_refresh_interval: Duration::from_secs(10),
            player_list_refresh_interval: Duration::from_secs(5),
            group_room_to_join: 0,
            initial_gadget_delay: 2,
            just_entered: false,
            player_info_map: HashMap::new(),
            buddy_map: HashSet::new(),
            group_room_list: HashMap::new(),
            staging_room_list: HashMap::new(),
            lobby_queued_utms: Vec::new(),
        }
    }

    /// Initialize the WOL Lobby Menu
    pub fn init(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        self.next_screen = None;
        self.button_pushed = false;
        self.is_shutting_down = false;
        self.trying_to_host_or_join = false;

        self.game_list_refresh_time = SystemTime::now();
        self.player_list_refresh_time = SystemTime::now();

        // Get window IDs
        self.parent_wol_lobby_id = self.get_window_id("WOLCustomLobby.wnd:WOLLobbyMenuParent");
        self.button_back_id = self.get_window_id("WOLCustomLobby.wnd:ButtonBack");
        self.button_host_id = self.get_window_id("WOLCustomLobby.wnd:ButtonHost");
        self.button_refresh_id = self.get_window_id("WOLCustomLobby.wnd:ButtonRefresh");
        self.button_join_id = self.get_window_id("WOLCustomLobby.wnd:ButtonJoin");
        self.button_buddy_id = self.get_window_id("WOLCustomLobby.wnd:ButtonBuddy");
        self.button_emote_id = self.get_window_id("WOLCustomLobby.wnd:ButtonEmote");
        self.text_entry_chat_id = self.get_window_id("WOLCustomLobby.wnd:TextEntryChat");
        self.listbox_lobby_players_id = self.get_window_id("WOLCustomLobby.wnd:ListboxPlayers");
        self.listbox_lobby_chat_id = self.get_window_id("WOLCustomLobby.wnd:ListboxChat");
        self.combo_lobby_group_rooms_id = self.get_window_id("WOLCustomLobby.wnd:ComboBoxGroupRooms");

        // Disable join button initially
        self.enable_button(self.button_join_id, false);

        // Set player tooltip
        self.set_player_tooltip(self.listbox_lobby_players_id);

        // Register chat window
        self.register_text_window(self.listbox_lobby_chat_id);

        // Clear chat text entry
        self.set_text_entry(self.text_entry_chat_id, "");

        // Populate group room list
        self.populate_group_room_listbox();

        // Show menu
        layout.hide(false);

        // Join group room if not already in one
        if self.get_current_group_room() == 0 {
            if self.group_room_to_join != 0 {
                self.join_group_room(self.group_room_to_join);
                self.group_room_to_join = 0;
            } else {
                self.join_best_group_room();
            }
        }

        self.grab_window_info();

        // Clear staging room list and start game list
        self.clear_staging_room_list();
        self.peer_request_start_game_list(true);

        // Show shell map
        self.show_shell_map(true);

        // Reset game state
        self.reset_game_state();

        // Set keyboard focus to chat
        self.set_focus(self.text_entry_chat_id);

        self.raise_message_boxes = true;
        self.lobby_queued_utms.clear();

        self.just_entered = true;
        self.initial_gadget_delay = 2;

        // Hide gadget parent for animation
        let gadget_parent_id = self.get_window_id("WOLCustomLobby.wnd:GadgetParent");
        self.hide_window(gadget_parent_id, true);
    }

    /// Shutdown the WOL Lobby Menu
    pub fn shutdown(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Save preferences
        self.save_preferences();

        self.release_window_info();
        self.unregister_text_window(self.listbox_lobby_chat_id);

        // Stop game list
        self.peer_request_stop_game_list();

        self.is_shutting_down = true;

        let pop_immediate = user_data.is_some();

        if pop_immediate {
            self.shutdown_complete(layout);
            return;
        }

        self.reverse_animate_window();
        self.raise_gs_message_box();
        self.reverse_transition("WOLCustomLobbyFade");
    }

    /// Update the WOL Lobby Menu
    pub fn update(&mut self, layout: &mut WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
        // Handle gadget animations
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.remove_transition("MainMenuDefaultMenuLogoFade");
                self.set_transition_group("WOLCustomLobbyFade");
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay -= 1;
            }
        }

        // Check if entering from game
        if self.is_in_shell_game() && self.get_game_frame() == 1 {
            self.signal_ui_interaction("SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME");
        }

        // Check for shutdown complete
        if self.is_shutting_down && self.is_anim_finished() && self.is_transition_finished() {
            self.shutdown_complete(layout);
        }

        // Raise pending message boxes
        if self.raise_message_boxes {
            self.raise_gs_message_box();
            self.raise_message_boxes = false;
        }

        // Process GameSpy messages
        if self.is_anim_finished() && self.is_transition_finished() && !self.button_pushed {
            self.handle_buddy_responses();
            self.handle_persistent_storage_responses();

            let max_messages = self.get_max_messages_per_update();
            let mut messages_processed = 0;
            let mut saw_important_message = false;
            let mut should_repopulate_players = false;

            // Process peer responses
            while messages_processed < max_messages && !saw_important_message {
                if let Some(resp) = self.get_peer_response() {
                    messages_processed += 1;

                    match resp.response_type {
                        PeerResponseType::JoinGroupRoom => {
                            saw_important_message = true;
                            if resp.join_group_room_ok {
                                self.set_current_group_room(resp.join_group_room_id);
                                self.clear_player_info_map();

                                // Show join message
                                let room_name = self.get_group_room_name(resp.join_group_room_id);
                                self.add_text(&format!("Joined lobby: {}", room_name));
                            } else {
                                self.join_best_group_room();
                            }
                            self.populate_group_room_listbox();
                            should_repopulate_players = true;
                        }
                        PeerResponseType::PlayerChangedFlags |
                        PeerResponseType::PlayerChangedNick |
                        PeerResponseType::PlayerInfo => {
                            self.update_player_info(&resp.player);
                            should_repopulate_players = true;
                        }
                        PeerResponseType::PlayerJoin => {
                            if resp.player_room_type == RoomType::GroupRoom {
                                self.update_player_info(&resp.player);
                                should_repopulate_players = true;
                            }
                        }
                        PeerResponseType::PlayerUTM | PeerResponseType::RoomUTM => {
                            self.lobby_queued_utms.push(resp.clone());
                        }
                        PeerResponseType::PlayerLeft => {
                            self.player_left_group_room(&resp.nick);
                            should_repopulate_players = true;
                        }
                        PeerResponseType::Message => {
                            self.add_chat(
                                &resp.nick,
                                resp.message_profile_id,
                                &resp.text,
                                !resp.message_is_private,
                                resp.message_is_action,
                            );
                        }
                        PeerResponseType::Disconnect => {
                            saw_important_message = true;
                            self.handle_disconnect(resp.disconnect_reason);
                        }
                        PeerResponseType::CreateStagingRoom => {
                            saw_important_message = true;
                            self.trying_to_host_or_join = false;
                            if resp.create_staging_room_result == 0 { // PEERJoinSuccess
                                self.button_pushed = true;
                                self.next_screen = Some("Menus/GameSpyGameOptionsMenu.wnd".to_string());
                                self.pop_menu();
                                self.mark_as_staging_room_host();
                                self.set_game_options();
                            }
                        }
                        PeerResponseType::JoinStagingRoom => {
                            saw_important_message = true;
                            self.trying_to_host_or_join = false;
                            self.handle_join_staging_room(&resp);
                        }
                        PeerResponseType::StagingRoomListComplete => {
                            self.saw_full_game_list();
                        }
                        PeerResponseType::StagingRoom => {
                            self.handle_staging_room_update(&resp);
                        }
                        _ => {}
                    }
                } else {
                    break;
                }
            }

            // Refresh player list if needed
            if should_repopulate_players {
                self.refresh_player_list(false);
            }

            // Refresh game list periodically
            self.refresh_game_list(false);
            self.refresh_player_list(false);
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
        const GLM_SELECTED: u32 = 0x0302;
        const GBM_SELECTED: u32 = 0x0201;
        const GCM_SELECTED: u32 = 0x0303;
        const GLM_DOUBLE_CLICKED: u32 = 0x0301;
        const GLM_RIGHT_CLICKED: u32 = 0x0304;
        const GEM_EDIT_DONE: u32 = 0x0402;

        match msg {
            GWM_CREATE => true,
            GWM_DESTROY => true,
            GWM_INPUT_FOCUS => true,
            GLM_SELECTED => self.handle_listbox_selected(data1 as i32, data2 as i32),
            GBM_SELECTED => self.handle_button_selected(data1 as i32),
            GCM_SELECTED => self.handle_combo_selected(data1 as i32, data2 as i32),
            GLM_DOUBLE_CLICKED => self.handle_listbox_double_clicked(data1 as i32, data2 as i32),
            GLM_RIGHT_CLICKED => self.handle_listbox_right_clicked(data1 as i32, data2 as usize),
            GEM_EDIT_DONE => self.handle_edit_done(),
            _ => false,
        }
    }

    /// Handle listbox selection
    fn handle_listbox_selected(&mut self, control_id: i32, row_selected: i32) -> bool {
        let game_list_id = self.get_game_list_box_id();

        if control_id == game_list_id {
            if row_selected >= 0 {
                self.enable_button(self.button_join_id, true);

                // Request extended info for game
                self.request_extended_staging_room_info(row_selected);
            } else {
                self.enable_button(self.button_join_id, false);
            }

            // Refresh game info if available
            self.refresh_game_info_listbox();
        }

        true
    }

    /// Handle button selection
    fn handle_button_selected(&mut self, control_id: i32) -> bool {
        if self.button_pushed {
            return true;
        }

        // Check sort buttons
        if self.handle_sort_button(control_id) {
            return true;
        }

        if control_id == self.button_back_id {
            if self.trying_to_host_or_join {
                return true;
            }

            self.leave_group_room();
            self.trying_to_host_or_join = true;
            self.button_pushed = true;
            self.next_screen = Some("Menus/WOLWelcomeMenu.wnd".to_string());
            self.pop_menu();
        } else if control_id == self.button_refresh_id {
            self.refresh_game_list(true);
            self.refresh_player_list(true);
        } else if control_id == self.button_host_id {
            if self.trying_to_host_or_join {
                return true;
            }

            self.trying_to_host_or_join = true;
            self.lobby_queued_utms.clear();
            self.group_room_to_join = self.get_current_group_room();
            self.open_game_spy_overlay("GSOVERLAY_GAMEOPTIONS");
        } else if control_id == self.button_join_id {
            if self.trying_to_host_or_join {
                return true;
            }

            self.handle_join_button();
        } else if control_id == self.button_buddy_id {
            self.toggle_game_spy_overlay("GSOVERLAY_BUDDY");
        } else if control_id == self.button_emote_id {
            let text = self.get_text_entry(self.text_entry_chat_id);
            self.set_text_entry(self.text_entry_chat_id, "");
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                self.send_chat(trimmed, false);
            }
        }

        true
    }

    /// Handle combo box selection
    fn handle_combo_selected(&mut self, control_id: i32, row_selected: i32) -> bool {
        if self.trying_to_host_or_join {
            return true;
        }

        if control_id == self.combo_lobby_group_rooms_id {
            if row_selected >= 0 {
                let group_id = self.get_combo_box_item_data(control_id, row_selected);
                if group_id != 0 && group_id != self.get_current_group_room() {
                    self.leave_group_room();
                    self.join_group_room(group_id);

                    if self.restrict_games_to_lobby() {
                        self.clear_staging_room_list();
                        self.refresh_game_list_boxes();
                        self.peer_request_start_game_list(true);
                    }
                }
            }
        }

        true
    }

    /// Handle listbox double-click
    fn handle_listbox_double_clicked(&mut self, control_id: i32, row_selected: i32) -> bool {
        if self.button_pushed {
            return true;
        }

        let game_list_id = self.get_game_list_box_id();

        if control_id == game_list_id && row_selected >= 0 {
            self.set_listbox_selected(control_id, row_selected);
            self.send_system_message(
                control_id,
                0x0201, // GBM_SELECTED
                self.button_join_id,
                self.button_join_id as usize,
            );
        }

        true
    }

    /// Handle listbox right-click
    fn handle_listbox_right_clicked(&mut self, control_id: i32, data: usize) -> bool {
        // Handle right-click context menus for players and games
        // This would show player options (buddy, ignore, etc.) or game details
        true
    }

    /// Handle edit done
    fn handle_edit_done(&mut self) -> bool {
        if self.button_pushed {
            return true;
        }

        let text = self.get_text_entry(self.text_entry_chat_id);
        self.set_text_entry(self.text_entry_chat_id, "");
        let trimmed = text.trim();

        if !trimmed.is_empty() {
            if !self.handle_lobby_slash_commands(trimmed) {
                self.send_chat(trimmed, false);
            }
        }

        true
    }

    /// Handle join button
    fn handle_join_button(&mut self) {
        self.lobby_queued_utms.clear();
        self.group_room_to_join = self.get_current_group_room();

        let selected = self.get_game_list_selected();
        if selected >= 0 {
            let selected_id = self.get_game_list_item_data(selected);
            if selected_id > 0 {
                if let Some(room) = self.staging_room_list.get(&selected_id) {
                    // Validate CRC, ladder, etc.
                    if !self.validate_join_game(room) {
                        return;
                    }

                    self.mark_as_staging_room_joiner(selected_id);
                    self.trying_to_host_or_join = true;

                    if room.has_password {
                        self.open_game_spy_overlay("GSOVERLAY_GAMEPASSWORD");
                    } else {
                        self.join_staging_room(selected_id, "");
                    }
                }
            } else {
                self.show_error("No game info available");
            }
        } else {
            self.show_error("No game selected");
        }
    }

    /// Handle staging room update
    fn handle_staging_room_update(&mut self, resp: &PeerResponse) {
        const PEER_CLEAR: i32 = 0;
        const PEER_ADD: i32 = 1;
        const PEER_UPDATE: i32 = 2;
        const PEER_REMOVE: i32 = 3;

        match resp.staging_room_action {
            PEER_CLEAR => {
                self.clear_staging_room_list();
            }
            PEER_ADD | PEER_UPDATE => {
                if resp.staging_room_percent_complete == 100 {
                    self.saw_full_game_list();
                }

                if !resp.staging_room_map_name.is_empty() {
                    // Create staging room from response
                    let room = self.create_staging_room_from_response(resp);

                    if resp.staging_room_action == PEER_ADD {
                        self.add_staging_room(room);
                    } else {
                        self.update_staging_room(room);
                    }
                } else {
                    // Invalid room - remove it
                    self.remove_staging_room(resp.staging_room_id);
                }
            }
            PEER_REMOVE => {
                self.remove_staging_room(resp.staging_room_id);
            }
            _ => {}
        }
    }

    /// Handle join staging room response
    fn handle_join_staging_room(&mut self, resp: &PeerResponse) {
        let is_host_present = self.check_host_present(resp);

        if resp.join_staging_room_ok && is_host_present {
            self.button_pushed = true;
            self.next_screen = Some("Menus/GameSpyGameOptionsMenu.wnd".to_string());
            self.pop_menu();
        } else {
            // Show error message based on result
            let error_msg = self.get_join_error_message(resp.join_staging_room_result);
            self.show_error(&error_msg);

            // Rejoin group room
            if self.group_room_to_join != 0 {
                self.join_group_room(self.group_room_to_join);
                self.group_room_to_join = 0;
            } else {
                self.join_best_group_room();
            }
        }
    }

    /// Populate lobby player listbox
    fn populate_lobby_player_listbox(&mut self) {
        // Save selection
        let selected_names = self.get_selected_player_names();
        let previous_top_index = self.get_listbox_top_visible_entry(self.listbox_lobby_players_id);

        // Reset listbox
        self.reset_listbox(self.listbox_lobby_players_id);

        // Add players in order: Ops, Buddies, Everyone else
        let mut indices_to_select = Vec::new();

        // Operators
        for (name, info) in &self.player_info_map {
            if self.is_player_op(info) {
                let index = self.insert_player_in_listbox(info);
                if selected_names.contains(name) {
                    indices_to_select.push(index);
                }
            }
        }

        // Buddies
        for (name, info) in &self.player_info_map {
            if !self.is_player_op(info) && self.buddy_map.contains(&info.profile_id) {
                let index = self.insert_player_in_listbox(info);
                if selected_names.contains(name) {
                    indices_to_select.push(index);
                }
            }
        }

        // Everyone else
        for (name, info) in &self.player_info_map {
            if !self.is_player_op(info) && !self.buddy_map.contains(&info.profile_id) {
                let index = self.insert_player_in_listbox(info);
                if selected_names.contains(name) {
                    indices_to_select.push(index);
                }
            }
        }

        // Restore selection
        self.set_listbox_selected_multiple(self.listbox_lobby_players_id, &indices_to_select);

        // Restore top visible entry
        self.set_listbox_top_visible_entry(self.listbox_lobby_players_id, previous_top_index);
    }

    /// Populate group room listbox
    fn populate_group_room_listbox(&mut self) {
        self.reset_combo_box(self.combo_lobby_group_rooms_id);

        let current_room = self.get_current_group_room();
        let mut index_to_select = -1;

        for (group_id, room) in &self.group_room_list {
            if *group_id != self.get_qm_channel() {
                let color = if *group_id == current_room {
                    0xFF00FFFF // current room color
                } else {
                    0xFFFFFFFF // room color
                };

                let selected = self.add_combo_box_entry_colored(
                    self.combo_lobby_group_rooms_id,
                    &room.translated_name,
                    color,
                );

                self.set_combo_box_item_data(
                    self.combo_lobby_group_rooms_id,
                    selected,
                    *group_id,
                );

                if *group_id == current_room {
                    index_to_select = selected;
                }
            }
        }

        self.set_combo_box_selected_pos(self.combo_lobby_group_rooms_id, index_to_select);
    }

    /// Refresh game list
    fn refresh_game_list(&mut self, force_refresh: bool) {
        let now = SystemTime::now();
        let should_refresh = force_refresh ||
            now.duration_since(self.game_list_refresh_time).unwrap_or(Duration::ZERO) >= self.game_list_refresh_interval;

        if should_refresh && self.has_staging_room_list_changed() {
            self.refresh_game_list_boxes();
            self.game_list_refresh_time = now;
        }
    }

    /// Refresh player list
    fn refresh_player_list(&mut self, force_refresh: bool) {
        let now = SystemTime::now();
        let should_refresh = force_refresh ||
            now.duration_since(self.player_list_refresh_time).unwrap_or(Duration::ZERO) >= self.player_list_refresh_interval;

        if should_refresh {
            self.populate_lobby_player_listbox();
            self.player_list_refresh_time = now;
        }
    }

    /// Handle lobby slash commands
    fn handle_lobby_slash_commands(&self, text: &str) -> bool {
        if !text.starts_with('/') {
            return false;
        }

        let parts: Vec<&str> = text[1..].split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let command = parts[0].to_lowercase();

        match command.as_str() {
            "me" if text.len() > 4 => {
                self.send_chat(&text[4..], true);
                true
            }
            "refresh" => {
                self.refresh_game_list(true);
                self.refresh_player_list(true);
                true
            }
            _ => false,
        }
    }

    /// Shutdown complete
    fn shutdown_complete(&mut self, layout: &mut WindowLayout) {
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
    fn enable_button(&self, id: i32, enable: bool) {}
    fn set_player_tooltip(&self, id: i32) {}
    fn register_text_window(&self, id: i32) {}
    fn unregister_text_window(&self, id: i32) {}
    fn show_shell_map(&self, show: bool) {}
    fn grab_window_info(&self) {}
    fn release_window_info(&self) {}
    fn join_group_room(&self, id: i32) {}
    fn leave_group_room(&self) {}
    fn join_best_group_room(&self) {}
    fn get_current_group_room(&self) -> i32 { 0 }
    fn clear_staging_room_list(&mut self) {}
    fn peer_request_start_game_list(&self, restrict: bool) {}
    fn peer_request_stop_game_list(&self) {}
    fn reset_game_state(&self) {}
    fn save_preferences(&self) {}
    fn is_in_shell_game(&self) -> bool { false }
    fn get_game_frame(&self) -> i32 { 0 }
    fn signal_ui_interaction(&self, hook: &str) {}
    fn is_anim_finished(&self) -> bool { true }
    fn is_transition_finished(&self) -> bool { true }
    fn set_transition_group(&self, group: &str) {}
    fn remove_transition(&self, name: &str) {}
    fn reverse_animate_window(&self) {}
    fn reverse_transition(&self, name: &str) {}
    fn raise_gs_message_box(&self) {}
    fn send_system_message(&self, window: i32, msg: u32, data1: i32, data2: usize) {}
    fn pop_menu(&self) {}
    fn push_menu(&self, name: &str) {}
    fn shell_shutdown_complete(&self, layout: &WindowLayout, has_next: bool) {}
    fn get_peer_response(&mut self) -> Option<PeerResponse> { None }
    fn get_max_messages_per_update(&self) -> usize { 10 }
    fn handle_buddy_responses(&self) {}
    fn handle_persistent_storage_responses(&self) {}
    fn clear_player_info_map(&mut self) {}
    fn update_player_info(&mut self, info: &PlayerInfo) {}
    fn player_left_group_room(&mut self, nick: &str) {}
    fn set_current_group_room(&mut self, id: i32) {}
    fn get_group_room_name(&self, id: i32) -> String { String::new() }
    fn add_text(&self, text: &str) {}
    fn add_chat(&self, nick: &str, profile_id: u32, text: &str, public: bool, is_action: bool) {}
    fn send_chat(&self, text: &str, is_emote: bool) {}
    fn handle_disconnect(&self, reason: i32) {}
    fn mark_as_staging_room_host(&self) {}
    fn set_game_options(&self) {}
    fn handle_sort_button(&self, id: i32) -> bool { false }
    fn open_game_spy_overlay(&self, overlay: &str) {}
    fn toggle_game_spy_overlay(&self, overlay: &str) {}
    fn get_game_list_box_id(&self) -> i32 { 0 }
    fn get_game_list_selected(&self) -> i32 { -1 }
    fn get_game_list_item_data(&self, index: i32) -> i32 { 0 }
    fn set_listbox_selected(&self, id: i32, index: i32) {}
    fn set_listbox_selected_multiple(&self, id: i32, indices: &[i32]) {}
    fn get_listbox_top_visible_entry(&self, id: i32) -> i32 { 0 }
    fn set_listbox_top_visible_entry(&self, id: i32, index: i32) {}
    fn request_extended_staging_room_info(&self, index: i32) {}
    fn refresh_game_info_listbox(&self) {}
    fn refresh_game_list_boxes(&self) {}
    fn mark_as_staging_room_joiner(&self, id: i32) {}
    fn join_staging_room(&self, id: i32, password: &str) {}
    fn validate_join_game(&self, room: &GameSpyStagingRoom) -> bool { true }
    fn check_host_present(&self, resp: &PeerResponse) -> bool { true }
    fn get_join_error_message(&self, result: i32) -> String { "Failed to join".to_string() }
    fn show_error(&self, msg: &str) {}
    fn saw_full_game_list(&self) {}
    fn add_staging_room(&mut self, room: GameSpyStagingRoom) {}
    fn update_staging_room(&mut self, room: GameSpyStagingRoom) {}
    fn remove_staging_room(&mut self, id: i32) {}
    fn create_staging_room_from_response(&self, resp: &PeerResponse) -> GameSpyStagingRoom {
        GameSpyStagingRoom {
            id: resp.staging_room_id,
            game_name: resp.staging_server_name.clone(),
            map_name: resp.staging_room_map_name.clone(),
            has_password: resp.staging_room_requires_password,
            allow_observers: resp.staging_room_allow_observers,
            use_stats: resp.staging_room_use_stats,
            version: resp.staging_room_version,
            exe_crc: resp.staging_room_exe_crc,
            ini_crc: resp.staging_room_ini_crc,
            ping_string: resp.staging_server_ping_string.clone(),
            ladder_ip: resp.staging_server_ladder_ip.clone(),
            ladder_port: resp.staging_room_ladder_port,
            reported_num_players: resp.staging_room_num_players,
            reported_max_players: resp.staging_room_max_players,
            reported_num_observers: resp.staging_room_num_observers,
        }
    }
    fn is_player_op(&self, info: &PlayerInfo) -> bool { false }
    fn insert_player_in_listbox(&self, info: &PlayerInfo) -> i32 { 0 }
    fn get_selected_player_names(&self) -> HashSet<String> { HashSet::new() }
    fn add_combo_box_entry_colored(&self, id: i32, text: &str, color: u32) -> i32 { 0 }
    fn set_combo_box_item_data(&self, id: i32, index: i32, data: i32) {}
    fn get_combo_box_item_data(&self, id: i32, index: i32) -> i32 { 0 }
    fn set_combo_box_selected_pos(&self, id: i32, index: i32) {}
    fn get_qm_channel(&self) -> i32 { 0 }
    fn has_staging_room_list_changed(&self) -> bool { false }
    fn restrict_games_to_lobby(&self) -> bool { false }
}

/// Window Layout placeholder
pub struct WindowLayout;

impl WindowLayout {
    pub fn hide(&mut self, hidden: bool) {}
    pub fn bring_forward(&mut self) {}
}
