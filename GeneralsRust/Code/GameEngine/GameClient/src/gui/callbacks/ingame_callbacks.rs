//! In-Game UI Callback Functions
//!
//! This module contains callback functions for in-game UI elements
//! such as chat, replay controls, diplomacy, etc.

use crate::game_text::GameText;
use crate::gui::{
    get_disconnect_menu, with_window_manager, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use crate::helpers::TheInGameUI;
use crate::language_filter::get_language_filter;
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::common::Relationship;
use gamelogic::helpers::TheGameLogic;
use gamelogic::player::{PlayerIndex, ThePlayerList};
use log::{debug, info, warn};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

const KEY_ESC: u32 = 0x1B;

/// In-game chat types
#[derive(Debug, Clone, PartialEq)]
pub enum InGameChatType {
    Allies,
    Everyone,
    Players,
}

/// In-game chat system
pub struct InGameChatCallbacks {
    active: bool,
    chat_type: InGameChatType,
    history: VecDeque<ChatEntry>,
    max_history: usize,
}

/// Chat message entry stored by the in-game UI system.
#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub sender_id: u8,
    pub message: String,
    pub target_mask: i32,
    pub is_disconnect: bool,
    pub timestamp: SystemTime,
    pub chat_type: InGameChatType,
}

#[derive(Default)]
struct InGameChatUiState {
    layout: Option<Rc<RefCell<WindowLayout>>>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    text_entry: Option<Rc<RefCell<GameWindow>>>,
    chat_type_text: Option<Rc<RefCell<GameWindow>>>,
    saved_text: String,
    just_hid: bool,
}

thread_local! {
    static CHAT_UI_STATE: Arc<Mutex<InGameChatUiState>> =
        Arc::new(Mutex::new(InGameChatUiState::default()));
}

fn chat_ui_state() -> Arc<Mutex<InGameChatUiState>> {
    CHAT_UI_STATE.with(|state| state.clone())
}

fn text_entry_text(window: &Option<Rc<RefCell<GameWindow>>>) -> String {
    let Some(window) = window.as_ref() else {
        return String::new();
    };
    let guard = window.borrow();
    if let Some(entry) = guard.widget().and_then(|widget| match widget {
        crate::gui::WindowWidget::TextEntry(entry) => Some(entry),
        _ => None,
    }) {
        return entry.text().to_string();
    }
    String::new()
}

fn set_text_entry_text(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(entry) = guard.text_entry_mut() {
        entry.set_text(value);
    }
}

fn ensure_chat_layout(state: &mut InGameChatUiState) {
    if state.layout.is_some() {
        return;
    }

    let layout =
        with_window_manager(|manager| manager.create_layout_with_windows("InGameChat.wnd").ok());
    let Some((layout, _info)) = layout else {
        return;
    };

    let parent_id = NameKeyGenerator::name_to_key("InGameChat.wnd:ParentInGameChat");
    let text_entry_id = NameKeyGenerator::name_to_key("InGameChat.wnd:TextEntryChat");
    let chat_type_id = NameKeyGenerator::name_to_key("InGameChat.wnd:StaticTextChatType");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let text_entry = with_window_manager(|manager| manager.get_window_by_id(text_entry_id as i32));
    let chat_type_text =
        with_window_manager(|manager| manager.get_window_by_id(chat_type_id as i32));

    set_text_entry_text(&text_entry, "");

    state.layout = Some(layout);
    state.parent = parent;
    state.text_entry = text_entry;
    state.chat_type_text = chat_type_text;
}

fn should_block_chat() -> bool {
    if TheGameLogic::is_in_replay_game() {
        return true;
    }
    if TheInGameUI::is_quit_menu_visible() {
        return true;
    }
    if let Ok(menu) = get_disconnect_menu().read() {
        if menu.is_visible() {
            return true;
        }
    }
    false
}

fn should_block_chat_in_single_player() -> bool {
    if TheGameLogic::is_in_multiplayer_game() {
        return false;
    }
    let Some(data) = get_global_data() else {
        return false;
    };
    let data = data.read();
    data.net_min_players > 0
}

fn handle_slash_commands(message: &str) -> bool {
    let trimmed = message.trim();
    if !trimmed.starts_with('/') {
        return false;
    }

    let mut parts = trimmed[1..].split_whitespace();
    let Some(cmd) = parts.next() else {
        return false;
    };

    if cmd.eq_ignore_ascii_case("host") {
        TheInGameUI::message("Hosting qr2:0 thread:0");
        return true;
    }

    false
}

fn build_chat_player_mask(chat_type: &InGameChatType) -> (i32, Option<u8>) {
    let Ok(list) = ThePlayerList().read() else {
        return (0, None);
    };

    let local_player = list.get_local_player().cloned();
    let local_index = local_player
        .as_ref()
        .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()))
        .unwrap_or(gamelogic::player::PLAYER_INDEX_INVALID);

    let mut mask: i32 = 0;
    let mut local_id: Option<u8> = None;

    for i in 0..game_network::MAX_SLOTS {
        let player_index = i as PlayerIndex;
        let player = list.get_player(player_index).cloned().or_else(|| {
            let name = format!("player{}", i);
            list.find_player_by_name(&name)
        });
        let Some(player) = player else {
            continue;
        };
        let Ok(player_guard) = player.read() else {
            continue;
        };

        if player_guard.get_player_index() == local_index {
            local_id = Some(i as u8);
        }

        let include = match chat_type {
            InGameChatType::Everyone => true,
            InGameChatType::Players => player_guard.get_player_index() == local_index,
            InGameChatType::Allies => {
                if player_guard.get_player_index() == local_index {
                    true
                } else {
                    let Some(local_player) = local_player.as_ref() else {
                        continue;
                    };
                    let Ok(local_guard) = local_player.read() else {
                        continue;
                    };
                    let Some(other_team) = player_guard.get_default_team() else {
                        continue;
                    };
                    let Some(local_team) = local_guard.get_default_team() else {
                        continue;
                    };
                    let Ok(other_team_guard) = other_team.read() else {
                        continue;
                    };
                    let Ok(local_team_guard) = local_team.read() else {
                        continue;
                    };
                    let local_rel = local_guard.get_relationship_with_team(&other_team_guard);
                    let other_rel = player_guard.get_relationship_with_team(&local_team_guard);
                    matches!(local_rel, Relationship::Ally | Relationship::Allies)
                        && matches!(other_rel, Relationship::Ally | Relationship::Allies)
                }
            }
        };

        if include {
            mask |= 1 << i;
        }
    }

    (mask, local_id)
}

impl InGameChatCallbacks {
    pub fn new() -> Self {
        Self {
            active: false,
            chat_type: InGameChatType::Allies,
            history: VecDeque::new(),
            max_history: 200,
        }
    }

    /// Handle in-game chat system messages
    pub fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::InputFocus => WindowMsgHandled::Handled,
            WindowMessage::GadgetEditDone => {
                let _ = self.toggle_in_game_chat(false);
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected => {
                let control_id = data1 as i32;
                let button_clear_id =
                    NameKeyGenerator::name_to_key("InGameChat.wnd:ButtonClear") as i32;
                if control_id == button_clear_id {
                    let state_handle = chat_ui_state();
                    let mut state = state_handle
                        .lock()
                        .expect("InGameChat ui state lock poisoned");
                    set_text_entry_text(&state.text_entry, "");
                    state.saved_text.clear();
                }
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Handle in-game chat input messages
    pub fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char {
            return WindowMsgHandled::Ignored;
        }
        let key = data1;
        if key == KEY_ESC {
            let _ = self.hide_in_game_chat(false);
            return WindowMsgHandled::Handled;
        }
        WindowMsgHandled::Handled
    }

    /// Toggle chat visibility
    pub fn toggle_in_game_chat(
        &mut self,
        immediate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Toggling in-game chat (immediate: {})", immediate);
        if should_block_chat() || should_block_chat_in_single_player() {
            return Ok(());
        }

        {
            let state_handle = chat_ui_state();
            let mut state = state_handle
                .lock()
                .expect("InGameChat ui state lock poisoned");
            if state.just_hid {
                state.just_hid = false;
                return Ok(());
            }
        }

        {
            let state_handle = chat_ui_state();
            let mut state = state_handle
                .lock()
                .expect("InGameChat ui state lock poisoned");
            ensure_chat_layout(&mut state);
        }

        let is_hidden = {
            let state_handle = chat_ui_state();
            let state = state_handle
                .lock()
                .expect("InGameChat ui state lock poisoned");
            state
                .parent
                .as_ref()
                .map(|parent| parent.borrow().is_hidden())
                .unwrap_or(true)
        };

        if is_hidden {
            self.show_in_game_chat(immediate)?;
        } else {
            let state_handle = chat_ui_state();
            let mut state = state_handle
                .lock()
                .expect("InGameChat ui state lock poisoned");
            let mut msg = text_entry_text(&state.text_entry);
            msg = msg.trim().to_string();
            if !msg.is_empty() && !handle_slash_commands(&msg) {
                let (player_mask, local_id) = build_chat_player_mask(&self.chat_type);
                let mut filtered = msg.clone();
                get_language_filter().filter_line(&mut filtered);

                if let Some(network) = game_network::get_network() {
                    let _ =
                        pollster::block_on(network.send_chat(filtered.clone(), player_mask as u32));
                } else {
                    warn!("send_chat ignored; network not initialized");
                }

                if let Some(sender) = local_id {
                    self.receive_network_message(sender, filtered.clone(), player_mask, false);
                }
            }
            set_text_entry_text(&state.text_entry, "");
            drop(state);
            self.hide_in_game_chat(immediate)?;
            let state_handle = chat_ui_state();
            state_handle
                .lock()
                .expect("InGameChat ui state lock poisoned")
                .just_hid = true;
        }

        Ok(())
    }

    /// Hide chat
    pub fn hide_in_game_chat(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        info!("Hiding in-game chat (immediate: {})", immediate);

        let state_handle = chat_ui_state();
        let mut state = state_handle
            .lock()
            .expect("InGameChat ui state lock poisoned");
        state.saved_text = text_entry_text(&state.text_entry);
        let parent = state.parent.clone();
        let text_entry = state.text_entry.clone();

        if let Some(parent) = parent {
            let _ = parent.borrow_mut().hide(true);
            let _ = parent.borrow_mut().enable(false);
        }
        if let Some(entry) = text_entry {
            let _ = entry.borrow_mut().hide(true);
            let _ = entry.borrow_mut().enable(false);
        }
        drop(state);

        with_window_manager(|manager| {
            let _ = manager.set_focus(None);
        });
        self.active = false;

        Ok(())
    }

    /// Show chat
    pub fn show_in_game_chat(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        info!("Showing in-game chat (immediate: {})", immediate);
        if should_block_chat() {
            return Ok(());
        }

        let state_handle = chat_ui_state();
        let mut state = state_handle
            .lock()
            .expect("InGameChat ui state lock poisoned");
        ensure_chat_layout(&mut state);

        if let Some(parent) = &state.parent {
            let _ = parent.borrow_mut().hide(false);
            let _ = parent.borrow_mut().enable(true);
        }
        if let Some(entry) = &state.text_entry {
            let _ = entry.borrow_mut().hide(false);
            let _ = entry.borrow_mut().enable(true);
            set_text_entry_text(&state.text_entry, &state.saved_text);
            state.saved_text.clear();
        }
        if let Some(entry) = &state.text_entry {
            with_window_manager(|manager| {
                let _ = manager.set_focus(Some(entry));
            });
        }
        drop(state);
        let _ = self.set_in_game_chat_type(InGameChatType::Everyone);
        self.active = true;

        Ok(())
    }

    /// Reset chat state
    pub fn reset_in_game_chat(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Resetting in-game chat");
        self.active = false;
        self.chat_type = InGameChatType::Allies;
        self.history.clear();
        let state_handle = chat_ui_state();
        let mut state = state_handle
            .lock()
            .expect("InGameChat ui state lock poisoned");
        if let Some(layout) = &state.layout {
            with_window_manager(|manager| manager.destroy_layout(layout));
        }
        *state = InGameChatUiState::default();

        Ok(())
    }

    /// Set chat type
    pub fn set_in_game_chat_type(
        &mut self,
        chat_type: InGameChatType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Setting in-game chat type to: {:?}", chat_type);
        self.chat_type = chat_type;
        let state_handle = chat_ui_state();
        let state = state_handle
            .lock()
            .expect("InGameChat ui state lock poisoned");
        let Some(label) = &state.chat_type_text else {
            return Ok(());
        };
        let label_text = match self.chat_type {
            InGameChatType::Everyone => {
                let is_active = ThePlayerList()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
                    .and_then(|player| player.read().ok().map(|guard| guard.is_player_active()))
                    .unwrap_or(true);
                if is_active {
                    GameText::fetch("Chat:Everyone")
                } else {
                    GameText::fetch("Chat:Observers")
                }
            }
            InGameChatType::Allies => GameText::fetch("Chat:Allies"),
            InGameChatType::Players => GameText::fetch("Chat:Players"),
        };
        let _ = label.borrow_mut().set_text(&label_text);
        Ok(())
    }

    /// Check if chat is active
    pub fn is_in_game_chat_active(&self) -> bool {
        let state_handle = chat_ui_state();
        let state = state_handle
            .lock()
            .expect("InGameChat ui state lock poisoned");
        state
            .parent
            .as_ref()
            .map(|parent| !parent.borrow().is_hidden())
            .unwrap_or(false)
    }

    /// Get current chat type
    pub fn get_chat_type(&self) -> &InGameChatType {
        &self.chat_type
    }

    pub fn get_history(&self) -> Vec<ChatEntry> {
        self.history.iter().cloned().collect()
    }

    pub fn receive_network_message(
        &mut self,
        sender_id: u8,
        message: String,
        target_mask: i32,
        is_disconnect: bool,
    ) {
        let chat_type = map_target_mask(target_mask);

        self.history.push_back(ChatEntry {
            sender_id,
            message,
            target_mask,
            is_disconnect,
            timestamp: SystemTime::now(),
            chat_type: chat_type.clone(),
        });

        while self.history.len() > self.max_history {
            self.history.pop_front();
        }

        if let Some(entry) = self.history.back() {
            let ui_line = if is_disconnect {
                format!("[DISCONNECT] P{}: {}", sender_id, entry.message)
            } else {
                format!("P{}: {}", sender_id, entry.message)
            };
            TheInGameUI::message(&ui_line);
        }

        if is_disconnect {
            warn!(
                "Chat disconnect event from player {} (mask {})",
                sender_id, target_mask
            );
        } else {
            debug!(
                "Chat message from player {}: {}",
                sender_id,
                self.history
                    .back()
                    .map(|entry| entry.message.as_str())
                    .unwrap_or("")
            );
        }
    }
}

impl Default for InGameChatCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Replay control system
pub struct ReplayControlCallbacks {
    playing: bool,
    paused: bool,
    fast_forward: bool,
    position: f64,
}

impl ReplayControlCallbacks {
    pub fn new() -> Self {
        Self {
            playing: false,
            paused: false,
            fast_forward: false,
            position: 0.0,
        }
    }

    /// Handle replay control system messages
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Replay control system message: {:?} for window: {}",
            msg,
            window.get_name()
        );

        match msg {
            WindowMessage::GadgetSelected => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Handle replay control input messages
    pub fn input(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Replay control input message: {:?} for window: {}",
            msg,
            window.get_name()
        );
        match msg {
            WindowMessage::GadgetSelected => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Toggle fast forward mode
    pub fn toggle_fast_forward(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Toggling replay fast forward mode");
        self.fast_forward = !self.fast_forward;
        Ok(())
    }

    /// Get current replay state
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn is_fast_forward(&self) -> bool {
        self.fast_forward
    }

    pub fn get_position(&self) -> f64 {
        self.position
    }
}

impl Default for ReplayControlCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Idle worker system
pub struct IdleWorkerCallbacks {
    worker_count: i32,
    idle_workers: Vec<gamelogic::common::ObjectID>,
    next_index: usize,
}

impl IdleWorkerCallbacks {
    pub fn new() -> Self {
        Self {
            worker_count: 0,
            idle_workers: Vec::new(),
            next_index: 0,
        }
    }

    /// Handle idle worker system messages
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Idle worker system message: {:?} for window: {}",
            msg,
            window.get_name()
        );

        match msg {
            WindowMessage::InputFocus => WindowMsgHandled::Handled,
            WindowMessage::GadgetSelected => {
                let button_id =
                    NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker")
                        as i32;
                if data1 as i32 == button_id {
                    self.select_next_idle_worker();
                }
                WindowMsgHandled::Handled
            }
            WindowMessage::None => {
                self.refresh_idle_workers_from_logic();
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Get idle worker count
    pub fn get_idle_worker_count(&self) -> i32 {
        self.worker_count
    }

    /// Update idle worker count
    pub fn set_idle_worker_count(&mut self, count: i32) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Setting idle worker count to: {}", count);
        self.worker_count = count;
        self.update_idle_worker_button();
        Ok(())
    }

    fn refresh_idle_workers_from_logic(&mut self) {
        let (adds, removes) = gamelogic::helpers::TheInGameUI::take_idle_worker_events();
        let local_index = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(gamelogic::player::PLAYER_INDEX_INVALID);

        for (object_id, player_index) in adds {
            if player_index == local_index as i32 && !self.idle_workers.contains(&object_id) {
                self.idle_workers.push(object_id);
            }
        }

        if !removes.is_empty() {
            self.idle_workers.retain(|id| {
                !removes.iter().any(|(remove_id, player_index)| {
                    *remove_id == *id && *player_index == local_index as i32
                })
            });
        }

        self.worker_count = self.idle_workers.len() as i32;
        self.update_idle_worker_button();
    }

    fn select_next_idle_worker(&mut self) {
        if self.idle_workers.is_empty() {
            return;
        }

        if self.next_index >= self.idle_workers.len() {
            self.next_index = 0;
        }

        let object_id = self.idle_workers[self.next_index];
        self.next_index = (self.next_index + 1) % self.idle_workers.len();

        let Ok(list) = ThePlayerList().read() else {
            return;
        };
        let local_index = list.get_local_player_index();
        if local_index == gamelogic::player::PLAYER_INDEX_INVALID {
            return;
        }

        let selection_manager = gamelogic::commands::selection::get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(local_index) {
                let _ = selection.select_objects(
                    vec![object_id],
                    gamelogic::commands::selection::SelectionType::Replace,
                );
            }
        };
    }

    fn update_idle_worker_button(&self) {
        let button_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker") as i32;
        let input_enabled = TheInGameUI::get_input_enabled();
        with_window_manager(|manager| {
            if let Some(button) = manager.get_window_by_id(button_id) {
                let enabled = self.worker_count > 0 && input_enabled;
                let _ = button.borrow_mut().enable(enabled);
            }
        });
    }
}

impl Default for IdleWorkerCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined in-game UI system
pub struct InGameUISystem {
    chat: Arc<RwLock<InGameChatCallbacks>>,
    replay: Arc<RwLock<ReplayControlCallbacks>>,
    idle_worker: Arc<RwLock<IdleWorkerCallbacks>>,
}

impl InGameUISystem {
    pub fn new() -> Self {
        Self {
            chat: Arc::new(RwLock::new(InGameChatCallbacks::new())),
            replay: Arc::new(RwLock::new(ReplayControlCallbacks::new())),
            idle_worker: Arc::new(RwLock::new(IdleWorkerCallbacks::new())),
        }
    }

    pub fn get_chat(&self) -> Arc<RwLock<InGameChatCallbacks>> {
        self.chat.clone()
    }

    pub fn get_replay(&self) -> Arc<RwLock<ReplayControlCallbacks>> {
        self.replay.clone()
    }

    pub fn get_idle_worker(&self) -> Arc<RwLock<IdleWorkerCallbacks>> {
        self.idle_worker.clone()
    }

    pub fn push_chat_message(
        &self,
        sender_id: u8,
        message: String,
        target_mask: i32,
        is_disconnect: bool,
    ) {
        let mut chat = self.chat.write().unwrap();
        chat.receive_network_message(sender_id, message, target_mask, is_disconnect);
    }

    /// Toggle chat through the system
    pub fn toggle_in_game_chat(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap();
        chat.toggle_in_game_chat(immediate)
    }

    /// Hide chat through the system
    pub fn hide_in_game_chat(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap();
        chat.hide_in_game_chat(immediate)
    }

    /// Show chat through the system
    pub fn show_in_game_chat(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap();
        chat.show_in_game_chat(immediate)
    }

    /// Reset chat through the system
    pub fn reset_in_game_chat(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap();
        chat.reset_in_game_chat()
    }

    /// Set chat type through the system
    pub fn set_in_game_chat_type(
        &self,
        chat_type: InGameChatType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap();
        chat.set_in_game_chat_type(chat_type)
    }

    /// Check if chat is active
    pub fn is_in_game_chat_active(&self) -> bool {
        let chat = self.chat.read().unwrap();
        chat.is_in_game_chat_active()
    }
}

impl Default for InGameUISystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global in-game UI system instance
lazy_static::lazy_static! {
    pub static ref THE_INGAME_UI_SYSTEM: Arc<RwLock<InGameUISystem>> =
        Arc::new(RwLock::new(InGameUISystem::new()));
}

/// Helper function to get the global in-game UI system
pub fn get_ingame_ui_system() -> Arc<RwLock<InGameUISystem>> {
    THE_INGAME_UI_SYSTEM.clone()
}

/// Convenience functions for global in-game UI operations
pub fn toggle_in_game_chat(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.toggle_in_game_chat(immediate)
}

pub fn hide_in_game_chat(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.hide_in_game_chat(immediate)
}

pub fn show_in_game_chat(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.show_in_game_chat(immediate)
}

pub fn reset_in_game_chat() -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.reset_in_game_chat()
}

pub fn set_in_game_chat_type(chat_type: InGameChatType) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.set_in_game_chat_type(chat_type)
}

pub fn is_in_game_chat_active() -> bool {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.is_in_game_chat_active()
}

pub fn push_network_chat_message(
    sender_id: u8,
    message: String,
    target_mask: i32,
    is_disconnect: bool,
) {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap();
    system.push_chat_message(sender_id, message, target_mask, is_disconnect);
}

fn map_target_mask(target_mask: i32) -> InGameChatType {
    if target_mask == -1 || target_mask == 0 {
        InGameChatType::Everyone
    } else {
        InGameChatType::Players
    }
}
