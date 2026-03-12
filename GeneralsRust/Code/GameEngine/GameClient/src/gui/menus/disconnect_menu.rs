//! Disconnect Menu Implementation
//!
//! Rust conversion of DisconnectMenu.cpp - handles network disconnection scenarios
//! and shows player status during multiplayer games.
//!
//! Original C++ file: GameClient/GUI/DisconnectMenu/DisconnectMenu.cpp

use crate::gui::game_window::WindowWidget;
use crate::gui::{GameWindow, WindowManager, WindowMessage, WindowMsgData, WindowMsgHandled};
use game_network::get_network;
use gamelogic::helpers::TheGameText;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Player names text control names (matching C++ array)
const PLAYER_NAME_TEXT_CONTROLS: &[&str] = &[
    "DisconnectScreen.wnd:StaticPlayer1Name",
    "DisconnectScreen.wnd:StaticPlayer2Name",
    "DisconnectScreen.wnd:StaticPlayer3Name",
    "DisconnectScreen.wnd:StaticPlayer4Name",
    "DisconnectScreen.wnd:StaticPlayer5Name",
    "DisconnectScreen.wnd:StaticPlayer6Name",
    "DisconnectScreen.wnd:StaticPlayer7Name",
];

/// Player timeout text control names (matching C++ array)  
const PLAYER_TIMEOUT_TEXT_CONTROLS: &[&str] = &[
    "DisconnectScreen.wnd:StaticPlayer1Timeout",
    "DisconnectScreen.wnd:StaticPlayer2Timeout",
    "DisconnectScreen.wnd:StaticPlayer3Timeout",
    "DisconnectScreen.wnd:StaticPlayer4Timeout",
    "DisconnectScreen.wnd:StaticPlayer5Timeout",
    "DisconnectScreen.wnd:StaticPlayer6Timeout",
    "DisconnectScreen.wnd:StaticPlayer7Timeout",
];

/// Player vote button control names.
const PLAYER_VOTE_BUTTON_CONTROLS: &[&str] = &[
    "DisconnectScreen.wnd:ButtonKickPlayer1",
    "DisconnectScreen.wnd:ButtonKickPlayer2",
    "DisconnectScreen.wnd:ButtonKickPlayer3",
    "DisconnectScreen.wnd:ButtonKickPlayer4",
    "DisconnectScreen.wnd:ButtonKickPlayer5",
    "DisconnectScreen.wnd:ButtonKickPlayer6",
    "DisconnectScreen.wnd:ButtonKickPlayer7",
];

/// Player vote count control names.
const PLAYER_VOTE_COUNT_CONTROLS: &[&str] = &[
    "DisconnectScreen.wnd:StaticPlayer1Votes",
    "DisconnectScreen.wnd:StaticPlayer2Votes",
    "DisconnectScreen.wnd:StaticPlayer3Votes",
    "DisconnectScreen.wnd:StaticPlayer4Votes",
    "DisconnectScreen.wnd:StaticPlayer5Votes",
    "DisconnectScreen.wnd:StaticPlayer6Votes",
    "DisconnectScreen.wnd:StaticPlayer7Votes",
];

const PACKET_ROUTER_TIMEOUT_LABEL: &str = "DisconnectScreen.wnd:StaticPacketRouterTimeoutLabel";
const PACKET_ROUTER_TIMEOUT_TEXT: &str = "DisconnectScreen.wnd:StaticPacketRouterTimeout";
const TEXT_ENTRY_CONTROL: &str = "DisconnectScreen.wnd:TextEntry";
const TEXT_DISPLAY_CONTROL: &str = "DisconnectScreen.wnd:ListboxTextDisplay";
const BUTTON_QUIT_CONTROL: &str = "DisconnectScreen.wnd:ButtonQuitGame";

/// Player disconnect status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDisconnectStatus {
    Connected,
    Disconnected,
    TimedOut,
    Kicked,
}

/// Player information for disconnect screen
#[derive(Debug, Clone)]
pub struct PlayerDisconnectInfo {
    pub player_id: u32,
    pub player_name: String,
    pub status: PlayerDisconnectStatus,
    pub timeout_remaining: Duration,
    pub disconnect_time: Option<Instant>,
}

/// Disconnect Menu - equivalent to C++ DisconnectMenu class
pub struct DisconnectMenu {
    /// Window reference
    window: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,

    /// Window manager reference
    window_manager: Option<Arc<Mutex<WindowManager>>>,

    /// Player information
    players: HashMap<u32, PlayerDisconnectInfo>,

    /// Player name text controls
    player_name_controls: Vec<Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>>,

    /// Player timeout text controls  
    player_timeout_controls: Vec<Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>>,

    /// Player vote buttons
    player_vote_controls: Vec<Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>>,

    /// Player vote count text controls
    player_vote_count_controls: Vec<Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>>,

    /// Packet router timeout label control
    packet_router_label: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,

    /// Packet router timeout text control
    packet_router_timeout: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,

    /// Disconnect list box
    text_display: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,

    /// Chat text entry
    text_entry: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,

    /// Menu state
    is_visible: bool,
    is_initialized: bool,

    /// Timing
    last_update: Instant,
    update_interval: Duration,

    /// Disconnect reason
    disconnect_reason: String,

    /// Whether we're waiting for players to reconnect
    waiting_for_reconnection: bool,

    /// Maximum wait time for reconnection
    max_reconnection_time: Duration,
}

impl DisconnectMenu {
    /// Create new DisconnectMenu
    pub fn new() -> Self {
        Self {
            window: None,
            window_manager: None,
            players: HashMap::new(),
            player_name_controls: Vec::with_capacity(PLAYER_NAME_TEXT_CONTROLS.len()),
            player_timeout_controls: Vec::with_capacity(PLAYER_TIMEOUT_TEXT_CONTROLS.len()),
            player_vote_controls: Vec::with_capacity(PLAYER_VOTE_BUTTON_CONTROLS.len()),
            player_vote_count_controls: Vec::with_capacity(PLAYER_VOTE_COUNT_CONTROLS.len()),
            packet_router_label: None,
            packet_router_timeout: None,
            text_display: None,
            text_entry: None,
            is_visible: false,
            is_initialized: false,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(100),
            disconnect_reason: String::new(),
            waiting_for_reconnection: true,
            max_reconnection_time: Duration::from_secs(30),
        }
    }

    /// Initialize the disconnect menu
    pub fn init(
        &mut self,
        window_manager: Arc<Mutex<WindowManager>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing Disconnect Menu");

        self.window_manager = Some(window_manager);

        // Load the disconnect screen window
        if let Some(manager) = &self.window_manager {
            let mut manager = manager.lock().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "WindowManager lock poisoned")
            })?;
            self.window = Some(manager.load_window("DisconnectScreen.wnd")?);
        }
        if let Some(window) = &self.window {
            let _ = window.borrow_mut().hide(true);
        }

        // Initialize player name controls
        self.player_name_controls.clear();
        for control_name in PLAYER_NAME_TEXT_CONTROLS {
            if let Some(window) = &self.window {
                let control = window.borrow().find_child_window(control_name);
                self.player_name_controls.push(control);
            }
        }

        // Initialize player timeout controls
        self.player_timeout_controls.clear();
        for control_name in PLAYER_TIMEOUT_TEXT_CONTROLS {
            if let Some(window) = &self.window {
                let control = window.borrow().find_child_window(control_name);
                self.player_timeout_controls.push(control);
            }
        }

        // Initialize vote buttons and vote count controls
        self.player_vote_controls.clear();
        for control_name in PLAYER_VOTE_BUTTON_CONTROLS {
            if let Some(window) = &self.window {
                let control = window.borrow().find_child_window(control_name);
                self.player_vote_controls.push(control);
            }
        }
        self.player_vote_count_controls.clear();
        for control_name in PLAYER_VOTE_COUNT_CONTROLS {
            if let Some(window) = &self.window {
                let control = window.borrow().find_child_window(control_name);
                self.player_vote_count_controls.push(control);
            }
        }

        if let Some(window) = &self.window {
            self.packet_router_label = window
                .borrow()
                .find_child_window(PACKET_ROUTER_TIMEOUT_LABEL);
            self.packet_router_timeout = window
                .borrow()
                .find_child_window(PACKET_ROUTER_TIMEOUT_TEXT);
        }

        // Initialize disconnect list + text entry
        if let Some(window) = &self.window {
            self.text_display = window.borrow().find_child_window(TEXT_DISPLAY_CONTROL);
            self.text_entry = window.borrow().find_child_window(TEXT_ENTRY_CONTROL);
        }

        if let (Some(manager), Some(entry)) = (&self.window_manager, &self.text_entry) {
            if let Ok(mut manager) = manager.lock() {
                let _ = manager.set_focus(Some(entry));
            }
        }

        if let Some(entry) = &self.text_entry {
            let mut entry = entry.borrow_mut();
            if let Some(WindowWidget::TextEntry(text_entry)) = entry.widget_mut() {
                text_entry.set_text("");
            }
        }

        let mut buttons: Vec<std::rc::Rc<std::cell::RefCell<GameWindow>>> = Vec::new();
        for control in &self.player_vote_controls {
            if let Some(control) = control {
                buttons.push(control.clone());
            }
        }
        if let Some(window) = &self.window {
            if let Some(button) = window.borrow().find_child_window(BUTTON_QUIT_CONTROL) {
                buttons.push(button);
            }
        }

        self.is_initialized = true;
        log::info!("Disconnect Menu initialized successfully");
        Ok(())
    }

    /// Show the disconnect menu
    pub fn show(&mut self, reason: &str) {
        if !self.is_initialized {
            log::warn!("DisconnectMenu::show called before initialization");
            return;
        }

        log::info!("Showing disconnect menu: {}", reason);

        self.disconnect_reason = reason.to_string();
        self.is_visible = true;
        self.waiting_for_reconnection = true;

        // Show the window
        if let Some(window) = &self.window {
            let mut window = window.borrow_mut();
            let _ = window.show();
            let _ = window.bring_to_front();
        }

        if let (Some(manager), Some(entry)) = (&self.window_manager, &self.text_entry) {
            if let Ok(mut manager) = manager.lock() {
                let _ = manager.set_focus(Some(entry));
            }
        }

        if let Some(list) = &self.text_display {
            let mut list = list.borrow_mut();
            if let Some(list_box) = list.list_box_mut() {
                list_box.clear();
                let text = TheGameText::fetch("GUI:InternetDisconnectionMenuBody1");
                list_box.add_item(&text);
            }
        }

        for button in &self.player_vote_controls {
            if let Some(button) = button {
                let _ = button.borrow_mut().enable(true);
            }
        }

        if let Some(window) = &self.window {
            if let Some(button) = window.borrow().find_child_window(BUTTON_QUIT_CONTROL) {
                let _ = button.borrow_mut().enable(true);
            }
        }

        // Update initial display
        self.update_display();
    }

    /// Hide the disconnect menu
    pub fn hide(&mut self) {
        log::info!("Hiding disconnect menu");

        self.is_visible = false;

        if let Some(window) = &self.window {
            let _ = window.borrow_mut().hide(true);
        }
    }

    /// Add player to disconnect tracking
    pub fn add_player(&mut self, player_id: u32, player_name: &str) {
        let player_info = PlayerDisconnectInfo {
            player_id,
            player_name: player_name.to_string(),
            status: PlayerDisconnectStatus::Connected,
            timeout_remaining: self.max_reconnection_time,
            disconnect_time: None,
        };

        self.players.insert(player_id, player_info);
        log::debug!("Added player {} to disconnect tracking", player_name);
    }

    /// Remove player from disconnect tracking
    pub fn remove_player(&mut self, player_id: u32) {
        if let Some(player) = self.players.remove(&player_id) {
            log::debug!(
                "Removed player {} from disconnect tracking",
                player.player_name
            );
            self.remove_player_from_slot(player_id as usize, &player.player_name);
            self.update_display();
        }
    }

    /// Set player disconnect status
    pub fn set_player_status(&mut self, player_id: u32, status: PlayerDisconnectStatus) {
        if let Some(player) = self.players.get_mut(&player_id) {
            let old_status = player.status;
            player.status = status;

            if status == PlayerDisconnectStatus::Disconnected
                && old_status == PlayerDisconnectStatus::Connected
            {
                player.disconnect_time = Some(Instant::now());
                player.timeout_remaining = self.max_reconnection_time;
            }

            log::info!(
                "Player {} status changed from {:?} to {:?}",
                player.player_name,
                old_status,
                status
            );

            self.update_display();
        }
    }

    /// Set player name for a slot and update related controls.
    pub fn set_player_name(&mut self, slot: usize, name: &str) {
        let entry = self
            .players
            .entry(slot as u32)
            .or_insert(PlayerDisconnectInfo {
                player_id: slot as u32,
                player_name: String::new(),
                status: PlayerDisconnectStatus::Connected,
                timeout_remaining: self.max_reconnection_time,
                disconnect_time: None,
            });
        entry.player_name = name.to_string();

        if let Some(control) = self.player_name_controls.get(slot).and_then(|c| c.as_ref()) {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(name.to_string());
            }
            let _ = control.set_text(name);
        }

        if let Some(control) = self
            .player_timeout_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(String::new());
            }
            let _ = control.set_text("");
        }

        if name.is_empty() {
            self.hide_player_controls(slot);
        } else {
            self.show_player_controls(slot);
        }
    }

    /// Update the disconnect menu
    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_visible || !self.is_initialized {
            return Ok(());
        }

        let now = Instant::now();
        if now.duration_since(self.last_update) < self.update_interval {
            return Ok(());
        }

        let mut needs_update = false;

        // Update player timeouts
        for (_, player) in self.players.iter_mut() {
            if player.status == PlayerDisconnectStatus::Disconnected {
                if let Some(disconnect_time) = player.disconnect_time {
                    let elapsed = now.duration_since(disconnect_time);
                    if elapsed < self.max_reconnection_time {
                        player.timeout_remaining = self.max_reconnection_time - elapsed;
                        needs_update = true;
                    } else {
                        // Player timed out
                        player.status = PlayerDisconnectStatus::TimedOut;
                        player.timeout_remaining = Duration::ZERO;
                        needs_update = true;
                        log::warn!("Player {} timed out", player.player_name);
                    }
                }
            }
        }

        if needs_update {
            self.update_display();
        }

        self.last_update = now;
        Ok(())
    }

    /// Update the visual display
    fn update_display(&mut self) {
        if !self.is_initialized {
            return;
        }

        // Sort players by ID for consistent display order
        let mut sorted_players: Vec<_> = self.players.values().collect();
        sorted_players.sort_by_key(|p| p.player_id);

        // Update player name and timeout controls
        for (i, control) in self.player_name_controls.iter_mut().enumerate() {
            if let Some(player_control) = control {
                let mut window = player_control.borrow_mut();
                if let Some(widget) = window.static_text_mut() {
                    if let Some(player) = sorted_players.get(i) {
                        widget.set_text(&player.player_name);
                        let _ = window.hide(false);
                    } else {
                        widget.set_text("");
                        let _ = window.hide(true);
                    }
                }
            }
        }

        for (i, control) in self.player_timeout_controls.iter_mut().enumerate() {
            if let Some(timeout_control) = control {
                let mut window = timeout_control.borrow_mut();
                if let Some(widget) = window.static_text_mut() {
                    if let Some(player) = sorted_players.get(i) {
                        let text = if player.status == PlayerDisconnectStatus::Disconnected {
                            format!("{}", player.timeout_remaining.as_secs())
                        } else if player.status == PlayerDisconnectStatus::TimedOut {
                            "0".to_string()
                        } else {
                            String::new()
                        };
                        widget.set_text(&text);
                        let _ = window.hide(text.is_empty());
                    } else {
                        widget.set_text("");
                        let _ = window.hide(true);
                    }
                }
            }
        }

        let visible_player_count = sorted_players.len();
        for slot in 0..PLAYER_NAME_TEXT_CONTROLS.len() {
            if slot < visible_player_count {
                self.show_player_controls(slot);
            } else {
                self.hide_player_controls(slot);
            }
        }

        // Listbox is updated by chat events; no per-frame list rebuild here.
    }

    /// Check if all players are connected
    pub fn all_players_connected(&self) -> bool {
        self.players
            .values()
            .all(|p| p.status == PlayerDisconnectStatus::Connected)
    }

    /// Check if any player has timed out
    pub fn has_timed_out_players(&self) -> bool {
        self.players
            .values()
            .any(|p| p.status == PlayerDisconnectStatus::TimedOut)
    }

    /// Get count of disconnected players
    pub fn get_disconnected_count(&self) -> usize {
        self.players
            .values()
            .filter(|p| p.status == PlayerDisconnectStatus::Disconnected)
            .count()
    }

    /// Set maximum reconnection time
    pub fn set_max_reconnection_time(&mut self, duration: Duration) {
        self.max_reconnection_time = duration;
    }

    /// Check if menu is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Handle button press events
    pub fn handle_button_press(
        &mut self,
        button_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        match button_name {
            BUTTON_QUIT_CONTROL => {
                self.quit_game();
                return Ok(true);
            }
            "DisconnectScreen.wnd:ButtonKickPlayer1" => return self.vote_player(0),
            "DisconnectScreen.wnd:ButtonKickPlayer2" => return self.vote_player(1),
            "DisconnectScreen.wnd:ButtonKickPlayer3" => return self.vote_player(2),
            "DisconnectScreen.wnd:ButtonKickPlayer4" => return self.vote_player(3),
            "DisconnectScreen.wnd:ButtonKickPlayer5" => return self.vote_player(4),
            "DisconnectScreen.wnd:ButtonKickPlayer6" => return self.vote_player(5),
            "DisconnectScreen.wnd:ButtonKickPlayer7" => return self.vote_player(6),
            _ => {}
        }
        Ok(false)
    }

    fn vote_player(&mut self, slot: u8) -> Result<bool, Box<dyn std::error::Error>> {
        self.vote_for_player(slot);
        if let Some(button) = self
            .player_vote_controls
            .get(slot as usize)
            .and_then(|b| b.as_ref())
        {
            let _ = button.borrow_mut().enable(false);
        }
        Ok(true)
    }

    /// Show a player name and related controls.
    fn show_player_controls(&mut self, slot: usize) {
        if let Some(control) = self.player_name_controls.get(slot).and_then(|c| c.as_ref()) {
            let _ = control.borrow_mut().hide(false);
        }
        if let Some(control) = self
            .player_timeout_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let _ = control.borrow_mut().hide(false);
        }
        if let Some(control) = self.player_vote_controls.get(slot).and_then(|c| c.as_ref()) {
            let mut control = control.borrow_mut();
            let _ = control.hide(false);
            let _ = control.enable(true);
        }
        if let Some(control) = self
            .player_vote_count_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let _ = control.borrow_mut().hide(false);
        }
    }

    /// Hide player controls.
    fn hide_player_controls(&mut self, slot: usize) {
        if let Some(control) = self.player_name_controls.get(slot).and_then(|c| c.as_ref()) {
            let _ = control.borrow_mut().hide(true);
        }
        if let Some(control) = self
            .player_timeout_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let _ = control.borrow_mut().hide(true);
        }
        if let Some(control) = self.player_vote_controls.get(slot).and_then(|c| c.as_ref()) {
            let mut control = control.borrow_mut();
            let _ = control.hide(true);
            let _ = control.enable(true);
        }
        if let Some(control) = self
            .player_vote_count_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let _ = control.borrow_mut().hide(true);
        }
    }

    /// Set a player timeout value.
    pub fn set_player_timeout_time(&mut self, slot: usize, new_time: i64) {
        let value = new_time.to_string();
        if let Some(control) = self
            .player_timeout_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(value.clone());
            }
            let _ = control.set_text(&value);
        }
    }

    pub fn show_packet_router_timeout(&mut self) {
        if let Some(control) = &self.packet_router_label {
            let _ = control.borrow_mut().hide(false);
        }
        if let Some(control) = &self.packet_router_timeout {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(String::new());
            }
            let _ = control.set_text("");
            let _ = control.hide(false);
        }
    }

    pub fn hide_packet_router_timeout(&mut self) {
        if let Some(control) = &self.packet_router_label {
            let _ = control.borrow_mut().hide(true);
        }
        if let Some(control) = &self.packet_router_timeout {
            let _ = control.borrow_mut().hide(true);
        }
    }

    pub fn set_packet_router_timeout_time(&mut self, new_time: i64) {
        let value = new_time.to_string();
        if let Some(control) = &self.packet_router_timeout {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(value.clone());
            }
            let _ = control.set_text(&value);
        }
    }

    pub fn send_chat(&self, text: &str) {
        let Some(network) = get_network() else {
            log::warn!("send_chat ignored; network not initialized");
            return;
        };
        let message = text.to_string();
        tokio::spawn(async move {
            if let Err(err) = network.send_disconnect_chat_message(message, 0).await {
                log::warn!("Failed to send disconnect chat message: {}", err);
            }
        });
    }

    fn submit_chat(&mut self) {
        let Some(entry) = &self.text_entry else {
            return;
        };
        let mut entry = entry.borrow_mut();
        let Some(WindowWidget::TextEntry(text_entry)) = entry.widget_mut() else {
            return;
        };
        let text = text_entry.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        text_entry.set_text("");
        drop(entry);
        self.show_chat(&text);
        self.send_chat(&text);
    }

    pub fn show_chat(&mut self, text: &str) {
        if let Some(list) = &self.text_display {
            let mut list = list.borrow_mut();
            if let Some(list_box) = list.list_box_mut() {
                list_box.add_item(text);
            }
        }
    }

    pub fn quit_game(&self) {
        let Some(network) = get_network() else {
            log::warn!("quit_game ignored; network not initialized");
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = network.quit_game().await {
                log::warn!("Failed to quit game: {}", err);
            }
        });
    }

    pub fn remove_player_from_slot(&mut self, slot: usize, player_name: &str) {
        self.hide_player_controls(slot);
        let template = TheGameText::fetch("Network:PlayerLeftGame");
        let message = format!("{} {}", template, player_name);
        self.show_chat(&message);
    }

    pub fn vote_for_player(&self, slot: u8) {
        let Some(network) = get_network() else {
            log::warn!("vote_for_player ignored; network not initialized");
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = network.vote_for_player_disconnect(slot as i32).await {
                log::warn!("Failed to vote for player {} disconnect: {}", slot, err);
            }
        });
    }

    pub fn update_votes(&mut self, slot: usize, votes: i32) {
        let value = votes.to_string();
        if let Some(control) = self
            .player_vote_count_controls
            .get(slot)
            .and_then(|c| c.as_ref())
        {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(value.clone());
            }
            let _ = control.set_text(&value);
        }
    }

    /// Reset the disconnect menu
    pub fn reset(&mut self) {
        self.players.clear();
        self.disconnect_reason.clear();
        self.is_visible = false;
        self.waiting_for_reconnection = true;

        if let Some(window) = &mut self.window {
            let _ = window.borrow_mut().hide(true);
        }

        if let Some(list) = &self.text_display {
            let mut list = list.borrow_mut();
            if let Some(list_box) = list.list_box_mut() {
                list_box.clear();
            }
        }
    }

    /// Handle system messages for disconnect controls.
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        let target_name = if data1 != 0 {
            window
                .find_child_by_id(data1 as i32)
                .map(|child| child.borrow().get_name().to_string())
        } else {
            None
        };
        let name = target_name.as_deref().unwrap_or_else(|| window.get_name());

        match msg {
            WindowMessage::GadgetSelected => {
                if let Ok(handled) = self.handle_button_press(name) {
                    if handled {
                        return WindowMsgHandled::Handled;
                    }
                }
            }
            WindowMessage::GadgetEditDone => {
                if name == TEXT_ENTRY_CONTROL {
                    self.submit_chat();
                    return WindowMsgHandled::Handled;
                }
            }
            _ => {}
        }

        WindowMsgHandled::Ignored
    }
}

impl Default for DisconnectMenu {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static THE_DISCONNECT_MENU: Arc<RwLock<DisconnectMenu>> =
        Arc::new(RwLock::new(DisconnectMenu::new()));
}

/// Helper function to get the global DisconnectMenu.
pub fn get_disconnect_menu() -> Arc<RwLock<DisconnectMenu>> {
    THE_DISCONNECT_MENU.with(|menu| menu.clone())
}
