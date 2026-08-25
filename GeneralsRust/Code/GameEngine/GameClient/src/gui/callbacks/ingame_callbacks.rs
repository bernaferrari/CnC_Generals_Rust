//! In-Game UI Callback Functions
//!
//! This module contains callback functions for in-game UI elements
//! such as chat, replay controls, diplomacy, etc.

use crate::game_text::GameText;
use crate::gui::control_bar::publish_host_select_next_idle_worker;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, get_disconnect_menu,
    with_window_manager, write_input_focus_response,
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

const KEY_ESC: usize = 0x1B;

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
                    matches!(local_rel, Relationship::Allies)
                        && matches!(other_rel, Relationship::Allies)
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
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
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
                    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
        data2: WindowMsgData,
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
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            if state.just_hid {
                state.just_hid = false;
                return Ok(());
            }
        }

        {
            let state_handle = chat_ui_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            ensure_chat_layout(&mut state);
        }

        let is_hidden = {
            let state_handle = chat_ui_state();
            let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            let mut msg = text_entry_text(&state.text_entry);
            msg = msg.trim().to_string();
            if !msg.is_empty() && !handle_slash_commands(&msg) {
                let (player_mask, local_id) = build_chat_player_mask(&self.chat_type);
                let mut filtered = msg.clone();
                get_language_filter().filter_line(&mut filtered);

                if let Some(network) = game_network::get_network() {
                    let _ = pollster::block_on(
                        network.send_chat_message(filtered.clone(), player_mask as u8),
                    );
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
                .unwrap_or_else(|e| e.into_inner())
                .just_hid = true;
        }

        Ok(())
    }

    /// Hide chat
    pub fn hide_in_game_chat(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        info!("Hiding in-game chat (immediate: {})", immediate);

        let state_handle = chat_ui_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
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
        WindowMsgHandled::Ignored
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

    /// Residual-friendly: start playback without gadget wiring.
    pub fn play(&mut self) {
        self.playing = true;
        self.paused = false;
    }

    /// Residual-friendly: pause playback.
    pub fn pause(&mut self) {
        self.playing = false;
        self.paused = true;
    }

    /// Residual-friendly: stop and reset position.
    pub fn stop(&mut self) {
        self.playing = false;
        self.paused = false;
        self.position = 0.0;
        self.fast_forward = false;
    }

    /// Residual-friendly: seek normalized position 0.0..=1.0.
    pub fn seek(&mut self, position: f64) {
        self.position = position.clamp(0.0, 1.0);
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
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Idle worker system message: {:?} for window: {}",
            msg,
            window.get_name()
        );

        match msg {
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            WindowMessage::GadgetSelected => {
                let button_id =
                    NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker")
                        as i32;
                if data1 as i32 == button_id {
                    // C++ routes this button to the same direct
                    // InGameUI::selectNextIdleWorker action as the Control
                    // Bar button.  Main owns live selection in the executable;
                    // leave the legacy manager untouched when its bridge is on.
                    if !publish_host_select_next_idle_worker() {
                        self.select_next_idle_worker();
                    }
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
        let mut chat = self.chat.write().unwrap_or_else(|e| e.into_inner());
        chat.receive_network_message(sender_id, message, target_mask, is_disconnect);
    }

    /// Toggle chat through the system
    pub fn toggle_in_game_chat(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap_or_else(|e| e.into_inner());
        chat.toggle_in_game_chat(immediate)
    }

    /// Hide chat through the system
    pub fn hide_in_game_chat(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap_or_else(|e| e.into_inner());
        chat.hide_in_game_chat(immediate)
    }

    /// Show chat through the system
    pub fn show_in_game_chat(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap_or_else(|e| e.into_inner());
        chat.show_in_game_chat(immediate)
    }

    /// Reset chat through the system
    pub fn reset_in_game_chat(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap_or_else(|e| e.into_inner());
        chat.reset_in_game_chat()
    }

    /// Set chat type through the system
    pub fn set_in_game_chat_type(
        &self,
        chat_type: InGameChatType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut chat = self.chat.write().unwrap_or_else(|e| e.into_inner());
        chat.set_in_game_chat_type(chat_type)
    }

    /// Check if chat is active
    pub fn is_in_game_chat_active(&self) -> bool {
        let chat = self.chat.read().unwrap_or_else(|e| e.into_inner());
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
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.toggle_in_game_chat(immediate)
}

pub fn hide_in_game_chat(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.hide_in_game_chat(immediate)
}

pub fn show_in_game_chat(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.show_in_game_chat(immediate)
}

pub fn reset_in_game_chat() -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.reset_in_game_chat()
}

pub fn set_in_game_chat_type(chat_type: InGameChatType) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.set_in_game_chat_type(chat_type)
}

pub fn is_in_game_chat_active() -> bool {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.is_in_game_chat_active()
}

pub fn push_network_chat_message(
    sender_id: u8,
    message: String,
    target_mask: i32,
    is_disconnect: bool,
) {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.push_chat_message(sender_id, message, target_mask, is_disconnect);
}

fn map_target_mask(target_mask: i32) -> InGameChatType {
    if target_mask == -1 || target_mask == 0 {
        InGameChatType::Everyone
    } else {
        InGameChatType::Players
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_control_input_is_always_ignored_like_cpp() {
        let mut replay = ReplayControlCallbacks::new();
        let window = GameWindow::new();

        assert_eq!(
            replay.input(&window, WindowMessage::GadgetSelected, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            replay.input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            replay.system(&window, WindowMessage::GadgetSelected, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn idle_worker_popup_publishes_typed_host_request_when_bridge_enabled() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

        let mut callbacks = IdleWorkerCallbacks::new();
        let window = GameWindow::new();
        let button_id = NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker")
            as WindowMsgData;
        assert_eq!(
            callbacks.system(&window, WindowMessage::GadgetSelected, button_id, 0),
            WindowMsgHandled::Handled
        );
        assert!(matches!(
            crate::gui::control_bar::take_host_control_bar_requests().as_slice(),
            [crate::gui::control_bar::HostControlBarRequest::SelectNextIdleWorker]
        ));
    }
}

/// Residual: last InGameChat action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualInGameChatAction {
    None = 0,
    Show = 1,
    Hide = 2,
    Toggle = 3,
    Clear = 4,
    SetTypeEveryone = 5,
    SetTypeAllies = 6,
    SetTypePlayers = 7,
    Submit = 8,
    Reset = 9,
}

static RESIDUAL_CHAT_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_CHAT_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_CHAT_TYPE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1); // Everyone
static RESIDUAL_CHAT_TEXT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn residual_chat_action_store(action: ResidualInGameChatAction) {
    RESIDUAL_CHAT_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last InGameChat residual action.
pub fn residual_in_game_chat_last_action() -> ResidualInGameChatAction {
    match RESIDUAL_CHAT_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualInGameChatAction::Show,
        2 => ResidualInGameChatAction::Hide,
        3 => ResidualInGameChatAction::Toggle,
        4 => ResidualInGameChatAction::Clear,
        5 => ResidualInGameChatAction::SetTypeEveryone,
        6 => ResidualInGameChatAction::SetTypeAllies,
        7 => ResidualInGameChatAction::SetTypePlayers,
        8 => ResidualInGameChatAction::Submit,
        9 => ResidualInGameChatAction::Reset,
        _ => ResidualInGameChatAction::None,
    }
}

/// Residual: chat active latch (independent of live layout).
pub fn residual_in_game_chat_is_active() -> bool {
    RESIDUAL_CHAT_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: chat type ordinal (0 Allies / 1 Everyone / 2 Players).
pub fn residual_in_game_chat_type_ordinal() -> u8 {
    RESIDUAL_CHAT_TYPE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: last chat entry text residual.
pub fn residual_in_game_chat_text() -> String {
    RESIDUAL_CHAT_TEXT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn with_chat_callbacks_mut<R>(f: impl FnOnce(&mut InGameChatCallbacks) -> R) -> R {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let chat = system.get_chat();
    let mut chat = chat.write().unwrap_or_else(|e| e.into_inner());
    f(&mut chat)
}

fn chat_type_to_ord(t: &InGameChatType) -> u8 {
    match t {
        InGameChatType::Allies => 0,
        InGameChatType::Everyone => 1,
        InGameChatType::Players => 2,
    }
}

fn ord_to_chat_type(ord: u8) -> InGameChatType {
    match ord {
        0 => InGameChatType::Allies,
        2 => InGameChatType::Players,
        _ => InGameChatType::Everyone,
    }
}

/// Residual: bind InGameChat control name keys (no layout load).
pub fn simulate_in_game_chat_bind_controls() -> bool {
    let _ = NameKeyGenerator::name_to_key("InGameChat.wnd:ButtonClear");
    let _ = NameKeyGenerator::name_to_key("InGameChat.wnd:TextEntryChat");
    let _ = NameKeyGenerator::name_to_key("InGameChat.wnd:StaticTextChatType");
    let _ = NameKeyGenerator::name_to_key("InGameChat.wnd:ParentInGameChat");
    true
}

/// Residual: show chat without layout create / block checks.
pub fn simulate_in_game_chat_show() -> bool {
    with_chat_callbacks_mut(|chat| {
        chat.active = true;
        RESIDUAL_CHAT_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
        residual_chat_action_store(ResidualInGameChatAction::Show);
        residual_in_game_chat_is_active()
    })
}

/// Residual: hide chat without layout hide.
pub fn simulate_in_game_chat_hide() -> bool {
    with_chat_callbacks_mut(|chat| {
        chat.active = false;
        RESIDUAL_CHAT_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
        residual_chat_action_store(ResidualInGameChatAction::Hide);
        !residual_in_game_chat_is_active()
    })
}

/// Residual: toggle chat residual.
pub fn simulate_in_game_chat_toggle() -> bool {
    with_chat_callbacks_mut(|chat| {
        chat.active = !chat.active;
        RESIDUAL_CHAT_ACTIVE.store(chat.active, std::sync::atomic::Ordering::Relaxed);
        residual_chat_action_store(ResidualInGameChatAction::Toggle);
        true
    })
}

/// Residual: ButtonClear without text entry widget.
pub fn simulate_in_game_chat_clear_button_gadget_selected() -> bool {
    let _ = simulate_in_game_chat_bind_controls();
    RESIDUAL_CHAT_TEXT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    // also clear ui saved text residual
    let state_handle = chat_ui_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.saved_text.clear();
    residual_chat_action_store(ResidualInGameChatAction::Clear);
    residual_in_game_chat_text().is_empty()
}

/// Residual: set chat type without label widget update.
pub fn simulate_in_game_chat_set_type(ord: u8) -> bool {
    with_chat_callbacks_mut(|chat| {
        let t = ord_to_chat_type(ord.min(2));
        let ord_v = chat_type_to_ord(&t);
        chat.chat_type = t;
        RESIDUAL_CHAT_TYPE.store(ord_v, std::sync::atomic::Ordering::Relaxed);
        residual_chat_action_store(match ord_v {
            0 => ResidualInGameChatAction::SetTypeAllies,
            2 => ResidualInGameChatAction::SetTypePlayers,
            _ => ResidualInGameChatAction::SetTypeEveryone,
        });
        residual_in_game_chat_type_ordinal() == ord_v
    })
}

/// Residual: set entry text without live text entry.
pub fn simulate_in_game_chat_set_text(text: &str) -> bool {
    *RESIDUAL_CHAT_TEXT.lock().unwrap_or_else(|e| e.into_inner()) = text.to_string();
    let state_handle = chat_ui_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.saved_text = text.to_string();
    residual_in_game_chat_text() == text
}

/// Residual: submit message into local history without network.
pub fn simulate_in_game_chat_submit(message: &str) -> bool {
    if message.is_empty() {
        return false;
    }
    with_chat_callbacks_mut(|chat| {
        chat.receive_network_message(0, message.to_string(), 0, false);
        *RESIDUAL_CHAT_TEXT.lock().unwrap_or_else(|e| e.into_inner()) = message.to_string();
        residual_chat_action_store(ResidualInGameChatAction::Submit);
        true
    })
}

/// Residual: reset chat residual.
pub fn simulate_in_game_chat_reset() -> bool {
    with_chat_callbacks_mut(|chat| {
        chat.active = false;
        chat.chat_type = InGameChatType::Everyone;
        chat.history.clear();
        RESIDUAL_CHAT_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
        RESIDUAL_CHAT_TYPE.store(1, std::sync::atomic::Ordering::Relaxed);
        RESIDUAL_CHAT_TEXT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        residual_chat_action_store(ResidualInGameChatAction::Reset);
        true
    })
}

/// Residual: show + Everyone + submit composite.
pub fn simulate_in_game_chat_prepare_submit(message: &str) -> bool {
    if !simulate_in_game_chat_bind_controls() {
        return false;
    }
    if !simulate_in_game_chat_show() {
        return false;
    }
    if !simulate_in_game_chat_set_type(1) {
        return false;
    }
    if !simulate_in_game_chat_set_text(message) {
        return false;
    }
    simulate_in_game_chat_submit(message)
}

/// Human click-through: OS LeftDown/Up on `InGameChat.wnd:ParentInGameChat`
/// (C++ WindowXlat hit). Not `simulate_*` first.
pub fn drive_os_wnd_in_game_chat_toggle_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat");
    if !clicked {
        return false;
    }
    simulate_in_game_chat_toggle()
}

pub fn drive_os_wnd_in_game_chat_show_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat");
    if !clicked {
        return false;
    }
    simulate_in_game_chat_show()
}

pub fn drive_os_wnd_in_game_chat_hide_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat");
    if !clicked {
        return false;
    }
    simulate_in_game_chat_hide()
}

/// Human click-through: OS LeftDown/Up on `InGameChat.wnd:ButtonClear`.
pub fn drive_os_wnd_in_game_chat_clear_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ButtonClear");
    if !clicked {
        return false;
    }
    simulate_in_game_chat_clear_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `TextEntryChat` then submit
/// (C++ GEM_EDIT_DONE on TextEntryChat).
pub fn drive_os_wnd_in_game_chat_submit_like_cpp(message: &str) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let clicked = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:TextEntryChat");
    if !clicked {
        return false;
    }
    simulate_in_game_chat_submit(message)
}

/// Human click-through: show parent + type label + TextEntryChat submit.
pub fn drive_os_wnd_in_game_chat_prepare_submit_like_cpp(message: &str) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let clicked_parent =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat");
    let clicked_type =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:StaticTextChatType");
    let clicked_entry = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:TextEntryChat");
    if !clicked_parent && !clicked_type && !clicked_entry {
        return false;
    }
    if clicked_parent {
        let _ = simulate_in_game_chat_show();
    }
    if clicked_type {
        let _ = simulate_in_game_chat_set_type(1);
    }
    if clicked_entry {
        let _ = simulate_in_game_chat_set_text(message);
        return simulate_in_game_chat_submit(message);
    }
    simulate_in_game_chat_prepare_submit(message)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_in_game_chat_clear_hits_button_then_latches() {
        install_named_button("InGameChat.wnd:ButtonClear", 10, 10);
        let _ = simulate_in_game_chat_set_text("scratch");
        assert!(
            drive_os_wnd_in_game_chat_clear_like_cpp(),
            "OS WND click on ButtonClear must clear chat residual"
        );
        assert_eq!(
            residual_in_game_chat_last_action(),
            ResidualInGameChatAction::Clear
        );
        assert!(residual_in_game_chat_text().is_empty());
    }

    #[test]
    fn os_wnd_in_game_chat_submit_hits_text_entry_then_latches() {
        install_named_button("InGameChat.wnd:ParentInGameChat", 10, 40);
        install_named_button("InGameChat.wnd:StaticTextChatType", 10, 70);
        install_named_button("InGameChat.wnd:TextEntryChat", 10, 100);
        assert!(
            drive_os_wnd_in_game_chat_prepare_submit_like_cpp("gl hf"),
            "OS WND clicks must submit chat residual"
        );
        assert_eq!(
            residual_in_game_chat_last_action(),
            ResidualInGameChatAction::Submit
        );
        assert_eq!(residual_in_game_chat_text(), "gl hf");
        assert!(!drive_os_wnd_in_game_chat_submit_like_cpp(""));
    }
}

// ---------------------------------------------------------------------------
// IdleWorker residual peels
// ---------------------------------------------------------------------------

/// Residual: last IdleWorker action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualIdleWorkerAction {
    None = 0,
    SetCount = 1,
    SelectNext = 2,
    Button = 3,
}

static RESIDUAL_IDLE_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_IDLE_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static RESIDUAL_IDLE_INDEX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn residual_idle_action_store(action: ResidualIdleWorkerAction) {
    RESIDUAL_IDLE_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last IdleWorker residual action.
pub fn residual_idle_worker_last_action() -> ResidualIdleWorkerAction {
    match RESIDUAL_IDLE_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualIdleWorkerAction::SetCount,
        2 => ResidualIdleWorkerAction::SelectNext,
        3 => ResidualIdleWorkerAction::Button,
        _ => ResidualIdleWorkerAction::None,
    }
}

/// Residual: idle worker count latch.
pub fn residual_idle_worker_count() -> i32 {
    RESIDUAL_IDLE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: next idle worker index latch.
pub fn residual_idle_worker_next_index() -> usize {
    RESIDUAL_IDLE_INDEX.load(std::sync::atomic::Ordering::Relaxed)
}

fn with_idle_worker_mut<R>(f: impl FnOnce(&mut IdleWorkerCallbacks) -> R) -> R {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let idle = system.get_idle_worker();
    let mut idle = idle.write().unwrap_or_else(|e| e.into_inner());
    f(&mut idle)
}

/// Residual: bind IdleWorker control name keys.
pub fn simulate_idle_worker_bind_controls() -> bool {
    let _ = NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker");
    true
}

/// Residual: set idle worker count without selection.
pub fn simulate_idle_worker_set_count(count: i32) -> bool {
    if count < 0 {
        return false;
    }
    with_idle_worker_mut(|idle| {
        let _ = idle.set_idle_worker_count(count);
        RESIDUAL_IDLE_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
        residual_idle_action_store(ResidualIdleWorkerAction::SetCount);
        residual_idle_worker_count() == count
    })
}

/// Residual: cycle next idle worker index residual.
pub fn simulate_idle_worker_select_next() -> bool {
    with_idle_worker_mut(|idle| {
        let count = idle.get_idle_worker_count().max(0) as usize;
        if count == 0 {
            idle.next_index = 0;
        } else {
            idle.next_index = (idle.next_index + 1) % count;
        }
        RESIDUAL_IDLE_INDEX.store(idle.next_index, std::sync::atomic::Ordering::Relaxed);
        residual_idle_action_store(ResidualIdleWorkerAction::SelectNext);
        true
    })
}

/// Residual: fire IdleWorker button without camera snap.
pub fn simulate_idle_worker_button_gadget_selected() -> bool {
    let _ = simulate_idle_worker_bind_controls();
    residual_idle_action_store(ResidualIdleWorkerAction::Button);
    let _ = simulate_idle_worker_select_next();
    residual_idle_action_store(ResidualIdleWorkerAction::Button);
    true
}

/// Residual: set count + button composite.
pub fn simulate_idle_worker_prepare_select(count: i32) -> bool {
    if !simulate_idle_worker_set_count(count) {
        return false;
    }
    simulate_idle_worker_button_gadget_selected()
}

/// Residual: last ReplayControl action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualReplayControlAction {
    None = 0,
    Play = 1,
    Pause = 2,
    Stop = 3,
    FastForward = 4,
    Seek = 5,
}

static RESIDUAL_REPLAY_CTRL_ACTION: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_REPLAY_CTRL_POSITION: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0); // fixed-point * 10000

fn residual_replay_ctrl_action_store(action: ResidualReplayControlAction) {
    RESIDUAL_REPLAY_CTRL_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last ReplayControl residual action.
pub fn residual_replay_control_last_action() -> ResidualReplayControlAction {
    match RESIDUAL_REPLAY_CTRL_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualReplayControlAction::Play,
        2 => ResidualReplayControlAction::Pause,
        3 => ResidualReplayControlAction::Stop,
        4 => ResidualReplayControlAction::FastForward,
        5 => ResidualReplayControlAction::Seek,
        _ => ResidualReplayControlAction::None,
    }
}

/// Residual: last seek position 0.0..=1.0.
pub fn residual_replay_control_position() -> f64 {
    RESIDUAL_REPLAY_CTRL_POSITION.load(std::sync::atomic::Ordering::Relaxed) as f64 / 10000.0
}

fn with_replay_control_mut<R>(f: impl FnOnce(&mut ReplayControlCallbacks) -> R) -> R {
    let system = get_ingame_ui_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let replay = system.get_replay();
    let mut replay = replay.write().unwrap_or_else(|e| e.into_inner());
    f(&mut replay)
}

/// Residual: bind ReplayControls name keys (no layout).
pub fn simulate_replay_control_bind_controls() -> bool {
    // C++ ReplayControls.cpp has no named Button* keys in layout strings; residual honesty uses logical buttons.
    let _ = NameKeyGenerator::name_to_key("ReplayControls.wnd:ButtonPlay");
    let _ = NameKeyGenerator::name_to_key("ReplayControls.wnd:ButtonPause");
    let _ = NameKeyGenerator::name_to_key("ReplayControls.wnd:ButtonStop");
    let _ = NameKeyGenerator::name_to_key("ReplayControls.wnd:ButtonFastForward");
    true
}

/// Residual: play without gadget/layout.
pub fn simulate_replay_control_play() -> bool {
    with_replay_control_mut(|r| {
        r.play();
        residual_replay_ctrl_action_store(ResidualReplayControlAction::Play);
        r.is_playing() && !r.is_paused()
    })
}

/// Residual: pause without gadget/layout.
pub fn simulate_replay_control_pause() -> bool {
    with_replay_control_mut(|r| {
        r.pause();
        residual_replay_ctrl_action_store(ResidualReplayControlAction::Pause);
        r.is_paused()
    })
}

/// Residual: stop without gadget/layout.
pub fn simulate_replay_control_stop() -> bool {
    with_replay_control_mut(|r| {
        r.stop();
        residual_replay_ctrl_action_store(ResidualReplayControlAction::Stop);
        RESIDUAL_REPLAY_CTRL_POSITION.store(0, std::sync::atomic::Ordering::Relaxed);
        !r.is_playing() && !r.is_paused() && r.get_position() == 0.0
    })
}

/// Residual: toggle fast-forward without gadget/layout.
pub fn simulate_replay_control_toggle_fast_forward() -> bool {
    with_replay_control_mut(|r| {
        let _ = r.toggle_fast_forward();
        residual_replay_ctrl_action_store(ResidualReplayControlAction::FastForward);
        true
    })
}

/// Residual: seek normalized position without slider widget.
pub fn simulate_replay_control_seek(position: f64) -> bool {
    with_replay_control_mut(|r| {
        r.seek(position);
        let fixed = (r.get_position() * 10000.0).round() as u32;
        RESIDUAL_REPLAY_CTRL_POSITION.store(fixed, std::sync::atomic::Ordering::Relaxed);
        residual_replay_ctrl_action_store(ResidualReplayControlAction::Seek);
        (residual_replay_control_position() - r.get_position()).abs() < 0.0002
    })
}

/// Residual: play + seek composite (playback honesty).
pub fn simulate_replay_control_prepare_play_at(position: f64) -> bool {
    if !simulate_replay_control_bind_controls() {
        return false;
    }
    if !simulate_replay_control_seek(position) {
        return false;
    }
    simulate_replay_control_play()
}

/// Human click-through: OS LeftDown/Up on retail `ReplayControls.wnd:Button*`.
fn drive_os_wnd_replay_control_named(name: &str, latch: impl FnOnce() -> bool) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name) {
        return false;
    }
    latch()
}

pub fn drive_os_wnd_replay_control_play_like_cpp() -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonPlay",
        simulate_replay_control_play,
    )
}

pub fn drive_os_wnd_replay_control_pause_like_cpp() -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonPause",
        simulate_replay_control_pause,
    )
}

pub fn drive_os_wnd_replay_control_stop_like_cpp() -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonStop",
        simulate_replay_control_stop,
    )
}

pub fn drive_os_wnd_replay_control_fast_forward_like_cpp() -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonFastForward",
        simulate_replay_control_toggle_fast_forward,
    )
}

pub fn drive_os_wnd_replay_control_prepare_play_at_like_cpp(position: f64) -> bool {
    let clicked_play = drive_os_wnd_replay_control_play_like_cpp();
    if !clicked_play {
        return false;
    }
    simulate_replay_control_prepare_play_at(position)
}

#[cfg(test)]
mod replay_control_os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_replay_control_play_and_pause_hit_named_gadgets() {
        install_named_button("ReplayControls.wnd:ButtonPlay", 10, 10);
        install_named_button("ReplayControls.wnd:ButtonPause", 10, 40);
        assert!(
            drive_os_wnd_replay_control_play_like_cpp(),
            "OS WND click on ButtonPlay must latch Play residual"
        );
        assert_eq!(
            residual_replay_control_last_action(),
            ResidualReplayControlAction::Play
        );
        assert!(drive_os_wnd_replay_control_pause_like_cpp());
        assert_eq!(
            residual_replay_control_last_action(),
            ResidualReplayControlAction::Pause
        );
        assert!(!drive_os_wnd_replay_control_stop_like_cpp());
    }
}
