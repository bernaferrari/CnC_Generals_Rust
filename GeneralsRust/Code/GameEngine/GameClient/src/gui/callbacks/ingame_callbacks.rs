//! In-Game UI Callback Functions
//!
//! This module contains callback functions for in-game UI elements
//! such as chat, replay controls, diplomacy, etc.

use crate::game_text::GameText;
use crate::gui::control_bar::publish_host_select_next_idle_worker;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    write_input_focus_response,
};

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
    ui: InGameChatUiState,
    last_action: ResidualInGameChatAction,
    last_text: String,
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

#[derive(Clone)]
struct ChatWindows {
    layout: Rc<RefCell<WindowLayout>>,
    parent: Rc<RefCell<GameWindow>>,
    text_entry: Rc<RefCell<GameWindow>>,
    chat_type_text: Rc<RefCell<GameWindow>>,
}
#[derive(Default)]
enum ChatLayoutPhase {
    #[default]
    Absent,
    Live(ChatWindows),
    Retiring(ChatWindows),
}
impl ChatLayoutPhase {
    fn windows(&self) -> Option<&ChatWindows> {
        match self {
            Self::Absent => None,
            Self::Live(windows) | Self::Retiring(windows) => Some(windows),
        }
    }
}
#[derive(Default)]
struct InGameChatUiState {
    phase: ChatLayoutPhase,
    generation: u64,
    saved_text: String,
    just_hid: bool,
}
mod chat_owner;
pub use chat_owner::*;

fn handle_slash_commands(
    message: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let trimmed = message.trim();
    if !trimmed.starts_with('/') {
        return false;
    }

    let mut parts = trimmed[1..].split_whitespace();
    let Some(cmd) = parts.next() else {
        return false;
    };

    if cmd.eq_ignore_ascii_case("host") {
        effects.message("Hosting qr2:0 thread:0");
        return true;
    }

    false
}

impl InGameChatCallbacks {
    pub fn new() -> Self {
        Self {
            active: false,
            chat_type: InGameChatType::Allies,
            history: VecDeque::new(),
            max_history: 200,
            ui: Default::default(),
            last_action: ResidualInGameChatAction::None,
            last_text: String::new(),
        }
    }
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
    ) -> String {
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

        let ui_line = self
            .history
            .back()
            .map(|entry| {
                if is_disconnect {
                    format!("[DISCONNECT] P{}: {}", sender_id, entry.message)
                } else {
                    format!("P{}: {}", sender_id, entry.message)
                }
            })
            .expect("just-admitted chat entry");

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
        ui_line
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
    last_action: ResidualReplayControlAction,
}

impl ReplayControlCallbacks {
    pub fn new() -> Self {
        Self {
            playing: false,
            paused: false,
            fast_forward: false,
            position: 0.0,
            last_action: ResidualReplayControlAction::None,
        }
    }

    /// Handle replay control system messages
    pub fn system(
        &mut self,
        window: &mut GameWindow,
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
        window: &mut GameWindow,
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
    last_action: ResidualIdleWorkerAction,
}

impl IdleWorkerCallbacks {
    pub fn new() -> Self {
        Self {
            worker_count: 0,
            idle_workers: Vec::new(),
            next_index: 0,
            last_action: ResidualIdleWorkerAction::None,
        }
    }

    /// Get idle worker count
    pub fn get_idle_worker_count(&self) -> i32 {
        self.worker_count
    }

    /// Update idle worker count
    pub fn set_idle_worker_count(
        &mut self,
        count: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &crate::core::subsystems::InGameUISubsystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Setting idle worker count to: {}", count);
        self.worker_count = count;
        self.update_idle_worker_button(receiving_manager, ui.get_input_enabled());
        Ok(())
    }

    /// C++ InGameUI.cpp5444-5480 performs each list mutation immediately.
    /// Consume the completed callbacks in that same order for this receiver.
    pub fn apply_idle_worker_events(
        &mut self,
        local_player: i32,
        events: impl IntoIterator<Item = gamelogic::helpers::IdleWorkerEvent>,
    ) {
        use gamelogic::helpers::IdleWorkerEvent;
        for event in events {
            match event {
                IdleWorkerEvent::Add { object, player } if player == local_player => {
                    if !self.idle_workers.contains(&object) {
                        self.idle_workers.push(object);
                    }
                }
                IdleWorkerEvent::Remove { object, player } if player == local_player => {
                    self.idle_workers.retain(|id| *id != object);
                }
                _ => {}
            }
        }
        self.worker_count = self.idle_workers.len() as i32;
    }

    fn next_idle_worker(&mut self) -> Option<gamelogic::common::ObjectID> {
        if self.idle_workers.is_empty() {
            return None;
        }
        if self.next_index >= self.idle_workers.len() {
            self.next_index = 0;
        }
        let id = self.idle_workers[self.next_index];
        self.next_index = (self.next_index + 1) % self.idle_workers.len();
        Some(id)
    }

    fn update_idle_worker_button(
        &self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        input_enabled: bool,
    ) {
        let button_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker") as i32;
        (|manager: &mut crate::gui::window_manager::WindowManager| {
            if let Some(button) = manager.get_window_by_id(button_id) {
                let enabled = self.worker_count > 0 && input_enabled;
                let _ = button.borrow_mut().enable(enabled);
            }
        })(receiving_manager);
    }
}

impl Default for IdleWorkerCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable callbacks belong to exactly one GameClient's MenuManager.
#[derive(Default)]
pub struct InGameUISystem {
    chat: InGameChatCallbacks,
    replay: ReplayControlCallbacks,
    idle_worker: IdleWorkerCallbacks,
}
impl InGameUISystem {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn chat(&self) -> &InGameChatCallbacks {
        &self.chat
    }
    pub fn replay(&self) -> &ReplayControlCallbacks {
        &self.replay
    }
    pub fn idle_worker(&self) -> &IdleWorkerCallbacks {
        &self.idle_worker
    }
    pub fn idle_worker_mut(&mut self) -> &mut IdleWorkerCallbacks {
        &mut self.idle_worker
    }
    pub(crate) fn assign_chat_type(
        &mut self,
        chat_type: InGameChatType,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        self.chat.chat_type = chat_type;
        self.chat
            .ui
            .phase
            .windows()
            .map(|windows| windows.chat_type_text.clone())
    }
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
        let mut window = GameWindow::new();

        assert_eq!(
            replay.input(&mut window, WindowMessage::GadgetSelected, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            replay.input(
                &mut window,
                WindowMessage::Char,
                KEY_ESC as WindowMsgData,
                0
            ),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            replay.system(&mut window, WindowMessage::GadgetSelected, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn idle_worker_popup_publishes_typed_host_request_when_bridge_enabled() {
        let mut transport_ui = crate::core::subsystems::InGameUISubsystem::default();
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        transport_ui.admit_native_control_bar_transport();

        let mut owner = crate::gui::callbacks::menu_callbacks::MenuManager::new();
        let mut messages = Vec::new();
        let mut effects = crate::gui::game_window::WindowEventEffects::new(&mut messages);
        effects.ui = Some(&mut transport_ui);
        effects.menus = Some(
            owner.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted),
        );
        let mut window = GameWindow::new();
        let button_id = NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker")
            as WindowMsgData;
        assert_eq!(
            idle_worker_system_receiving(
                &mut window,
                WindowMessage::GadgetSelected,
                button_id,
                0,
                &mut effects
            ),
            WindowMsgHandled::Handled
        );
        assert!(matches!(
            crate::gui::control_bar::take_host_control_bar_requests(&mut transport_ui).as_slice(),
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

pub fn residual_in_game_chat_last_action(owner: &InGameUISystem) -> ResidualInGameChatAction {
    owner.chat.last_action
}
pub fn residual_in_game_chat_is_active(owner: &InGameUISystem) -> bool {
    owner.chat.active
}
pub fn residual_in_game_chat_type_ordinal(owner: &InGameUISystem) -> u8 {
    chat_type_to_ord(&owner.chat.chat_type)
}
pub fn residual_in_game_chat_text(owner: &InGameUISystem) -> String {
    owner.chat.last_text.clone()
}
fn chat_type_to_ord(chat_type: &InGameChatType) -> u8 {
    match chat_type {
        InGameChatType::Allies => 0,
        InGameChatType::Everyone => 1,
        InGameChatType::Players => 2,
    }
}
fn ord_to_chat_type(ordinal: u8) -> InGameChatType {
    match ordinal {
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

/// Diagnostic-only state operations. They do not claim an authored WND was hit.
pub fn simulate_in_game_chat_show(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.chat.active = true;
        owner.chat.last_action = ResidualInGameChatAction::Show;
        true
    })
}
pub fn simulate_in_game_chat_hide(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.chat.active = false;
        owner.chat.last_action = ResidualInGameChatAction::Hide;
        true
    })
}
pub fn simulate_in_game_chat_toggle(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.chat.active = !owner.chat.active;
        owner.chat.last_action = ResidualInGameChatAction::Toggle;
        true
    })
}
pub fn simulate_in_game_chat_clear_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let _ = simulate_in_game_chat_bind_controls();
    effects.with_ingame_ui(|owner| {
        owner.chat.last_text.clear();
        owner.chat.ui.saved_text.clear();
        owner.chat.last_action = ResidualInGameChatAction::Clear;
        true
    })
}
pub fn simulate_in_game_chat_set_type(
    ordinal: u8,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.chat.chat_type = ord_to_chat_type(ordinal.min(2));
        owner.chat.last_action = match chat_type_to_ord(&owner.chat.chat_type) {
            0 => ResidualInGameChatAction::SetTypeAllies,
            2 => ResidualInGameChatAction::SetTypePlayers,
            _ => ResidualInGameChatAction::SetTypeEveryone,
        };
        true
    })
}
pub fn simulate_in_game_chat_set_text(
    text: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.chat.last_text = text.into();
        owner.chat.ui.saved_text = text.into();
        true
    })
}
pub fn simulate_in_game_chat_submit(
    message: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if message.is_empty() {
        return false;
    }
    let line = effects.with_ingame_ui(|owner| {
        let line = owner
            .chat
            .receive_network_message(0, message.into(), 0, false);
        owner.chat.last_text = message.into();
        owner.chat.last_action = ResidualInGameChatAction::Submit;
        line
    });
    effects.message(&line);
    true
}
pub fn simulate_in_game_chat_reset(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.chat.active = false;
        owner.chat.chat_type = InGameChatType::Everyone;
        owner.chat.history.clear();
        owner.chat.last_text.clear();
        owner.chat.last_action = ResidualInGameChatAction::Reset;
        true
    })
}

pub fn simulate_in_game_chat_prepare_submit(
    message: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_in_game_chat_bind_controls() {
        return false;
    }
    if !simulate_in_game_chat_show(effects) {
        return false;
    }
    if !simulate_in_game_chat_set_type(1, effects) {
        return false;
    }
    if !simulate_in_game_chat_set_text(message, effects) {
        return false;
    }
    simulate_in_game_chat_submit(message, effects)
}

/// Human click-through: OS LeftDown/Up on `InGameChat.wnd:ParentInGameChat`
/// (C++ WindowXlat hit). Not `simulate_*` first.
pub fn drive_os_wnd_in_game_chat_toggle_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat", effects);
    if !clicked {
        return false;
    }
    simulate_in_game_chat_toggle(effects)
}

pub fn drive_os_wnd_in_game_chat_show_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat", effects);
    if !clicked {
        return false;
    }
    simulate_in_game_chat_show(effects)
}

pub fn drive_os_wnd_in_game_chat_hide_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat", effects);
    if !clicked {
        return false;
    }
    simulate_in_game_chat_hide(effects)
}

/// Human click-through: OS LeftDown/Up on `InGameChat.wnd:ButtonClear`.
pub fn drive_os_wnd_in_game_chat_clear_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ButtonClear", effects);
    if !clicked {
        return false;
    }
    simulate_in_game_chat_clear_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `TextEntryChat` then submit
/// (C++ GEM_EDIT_DONE on TextEntryChat).
pub fn drive_os_wnd_in_game_chat_submit_like_cpp(
    message: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let clicked =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:TextEntryChat", effects);
    if !clicked {
        return false;
    }
    simulate_in_game_chat_submit(message, effects)
}

/// Human click-through: show parent + type label + TextEntryChat submit.
pub fn drive_os_wnd_in_game_chat_prepare_submit_like_cpp(
    message: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let clicked_parent =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:ParentInGameChat", effects);
    let clicked_type =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:StaticTextChatType", effects);
    let clicked_entry =
        crate::gui::dispatch_os_click_named_window("InGameChat.wnd:TextEntryChat", effects);
    if !clicked_parent && !clicked_type && !clicked_entry {
        return false;
    }
    if clicked_parent {
        let _ = simulate_in_game_chat_show(effects);
    }
    if clicked_type {
        let _ = simulate_in_game_chat_set_type(1, effects);
    }
    if clicked_entry {
        let _ = simulate_in_game_chat_set_text(message, effects);
        return simulate_in_game_chat_submit(message, effects);
    }
    simulate_in_game_chat_prepare_submit(message, effects)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    fn install_named_button(
        client: &mut crate::core::game_client::GameClient,
        name: &str,
        x: i32,
        y: i32,
    ) {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let button = manager
                    .create_window(None, x, y, 80, 24, effects)
                    .expect(name);
                button.borrow_mut().set_name(name);
                let _ = button.borrow_mut().hide(false);
            });
        });
    }
    #[test]
    fn os_wnd_in_game_chat_clear_hits_button_then_latches() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "InGameChat.wnd:ButtonClear", 10, 10);
        let _ = client.with_window_event_effects(None, |effects| {
            simulate_in_game_chat_set_text("scratch", effects)
        });
        assert!(
            client.with_window_event_effects(None, |effects| {
                drive_os_wnd_in_game_chat_clear_like_cpp(effects)
            }),
            "OS WND click on ButtonClear must clear chat residual"
        );
        assert_eq!(
            residual_in_game_chat_last_action(client.in_game_callbacks()),
            ResidualInGameChatAction::Clear
        );
        assert!(residual_in_game_chat_text(client.in_game_callbacks()).is_empty());
    }

    #[test]
    fn os_wnd_in_game_chat_submit_hits_text_entry_then_latches() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "InGameChat.wnd:ParentInGameChat", 10, 40);
        install_named_button(&mut client, "InGameChat.wnd:StaticTextChatType", 10, 70);
        install_named_button(&mut client, "InGameChat.wnd:TextEntryChat", 10, 100);
        assert!(
            client.with_window_event_effects(None, |effects| {
                drive_os_wnd_in_game_chat_prepare_submit_like_cpp("gl hf", effects)
            }),
            "OS WND clicks must submit chat residual"
        );
        assert_eq!(
            residual_in_game_chat_last_action(client.in_game_callbacks()),
            ResidualInGameChatAction::Submit
        );
        assert_eq!(
            residual_in_game_chat_text(client.in_game_callbacks()),
            "gl hf"
        );
        assert!(!client.with_window_event_effects(None, |effects| {
            drive_os_wnd_in_game_chat_submit_like_cpp("", effects)
        }));
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

pub fn residual_idle_worker_last_action(owner: &InGameUISystem) -> ResidualIdleWorkerAction {
    owner.idle_worker.last_action
}
pub fn residual_idle_worker_count(owner: &InGameUISystem) -> i32 {
    owner.idle_worker.worker_count
}
pub fn residual_idle_worker_next_index(owner: &InGameUISystem) -> usize {
    owner.idle_worker.next_index
}
/// Residual: bind IdleWorker control name keys.
pub fn simulate_idle_worker_bind_controls() -> bool {
    let _ = NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker");
    true
}

pub fn simulate_idle_worker_set_count(
    count: i32,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if count < 0 {
        return false;
    }
    effects.with_ingame_ui(|owner| {
        owner.idle_worker.worker_count = count;
        owner.idle_worker.last_action = ResidualIdleWorkerAction::SetCount;
    });
    let enabled = count > 0 && effects.ui().get_input_enabled();
    if let Some(button) = receiving_manager
        .get_window_by_id(NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker") as i32)
    {
        let _ = button.borrow_mut().enable(enabled);
    }
    true
}
pub fn simulate_idle_worker_select_next(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        let idle = &mut owner.idle_worker;
        let count = idle.worker_count.max(0) as usize;
        idle.next_index = if count == 0 {
            0
        } else {
            (idle.next_index + 1) % count
        };
        idle.last_action = ResidualIdleWorkerAction::SelectNext;
        true
    })
}
pub fn simulate_idle_worker_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let _ = simulate_idle_worker_bind_controls();
    let result = simulate_idle_worker_select_next(effects);
    effects
        .with_ingame_ui(|owner| owner.idle_worker.last_action = ResidualIdleWorkerAction::Button);
    result
}
pub fn simulate_idle_worker_prepare_select(
    count: i32,
    manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_idle_worker_set_count(count, manager, effects)
        && simulate_idle_worker_button_gadget_selected(effects)
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

pub fn residual_replay_control_last_action(owner: &InGameUISystem) -> ResidualReplayControlAction {
    owner.replay.last_action
}
pub fn residual_replay_control_position(owner: &InGameUISystem) -> f64 {
    owner.replay.position
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

pub fn simulate_replay_control_play(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.replay.play();
        owner.replay.last_action = ResidualReplayControlAction::Play;
        owner.replay.is_playing() && !owner.replay.is_paused()
    })
}
pub fn simulate_replay_control_pause(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.replay.pause();
        owner.replay.last_action = ResidualReplayControlAction::Pause;
        owner.replay.is_paused()
    })
}
pub fn simulate_replay_control_stop(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.replay.stop();
        owner.replay.last_action = ResidualReplayControlAction::Stop;
        !owner.replay.is_playing() && !owner.replay.is_paused() && owner.replay.position == 0.0
    })
}
pub fn simulate_replay_control_toggle_fast_forward(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        let _ = owner.replay.toggle_fast_forward();
        owner.replay.last_action = ResidualReplayControlAction::FastForward;
        true
    })
}
pub fn simulate_replay_control_seek(
    position: f64,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_ingame_ui(|owner| {
        owner.replay.seek(position);
        owner.replay.last_action = ResidualReplayControlAction::Seek;
        true
    })
}
pub fn simulate_replay_control_prepare_play_at(
    position: f64,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_replay_control_bind_controls()
        && simulate_replay_control_seek(position, effects)
        && simulate_replay_control_play(effects)
}

/// Human click-through: OS LeftDown/Up on retail `ReplayControls.wnd:Button*`.
fn drive_os_wnd_replay_control_named(
    name: &str,
    latch: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>) -> bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name, effects) {
        return false;
    }
    latch(effects)
}

pub fn drive_os_wnd_replay_control_play_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonPlay",
        simulate_replay_control_play,
        effects,
    )
}

pub fn drive_os_wnd_replay_control_pause_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonPause",
        simulate_replay_control_pause,
        effects,
    )
}

pub fn drive_os_wnd_replay_control_stop_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonStop",
        simulate_replay_control_stop,
        effects,
    )
}

pub fn drive_os_wnd_replay_control_fast_forward_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_replay_control_named(
        "ReplayControls.wnd:ButtonFastForward",
        simulate_replay_control_toggle_fast_forward,
        effects,
    )
}

pub fn drive_os_wnd_replay_control_prepare_play_at_like_cpp(
    position: f64,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked_play = drive_os_wnd_replay_control_play_like_cpp(effects);
    if !clicked_play {
        return false;
    }
    simulate_replay_control_prepare_play_at(position, effects)
}

#[cfg(test)]
mod replay_control_os_wnd_tests {
    use super::*;
    fn install_named_button(
        client: &mut crate::core::game_client::GameClient,
        name: &str,
        x: i32,
        y: i32,
    ) {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let button = manager
                    .create_window(None, x, y, 80, 24, effects)
                    .expect(name);
                button.borrow_mut().set_name(name);
                let _ = button.borrow_mut().hide(false);
            });
        });
    }
    #[test]
    fn os_wnd_replay_control_play_and_pause_hit_named_gadgets() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "ReplayControls.wnd:ButtonPlay", 10, 10);
        install_named_button(&mut client, "ReplayControls.wnd:ButtonPause", 10, 40);
        assert!(
            client.with_window_event_effects(None, |effects| {
                drive_os_wnd_replay_control_play_like_cpp(effects)
            }),
            "OS WND click on ButtonPlay must latch Play residual"
        );
        assert_eq!(
            residual_replay_control_last_action(client.in_game_callbacks()),
            ResidualReplayControlAction::Play
        );
        assert!(client.with_window_event_effects(None, |effects| {
            drive_os_wnd_replay_control_pause_like_cpp(effects)
        }));
        assert_eq!(
            residual_replay_control_last_action(client.in_game_callbacks()),
            ResidualReplayControlAction::Pause
        );
        assert!(!client.with_window_event_effects(None, |effects| {
            drive_os_wnd_replay_control_stop_like_cpp(effects)
        }));
    }
}

#[cfg(test)]
mod owned_idle_worker_event_tests {
    use super::*;
    use gamelogic::helpers::{IdleWorkerEvent, IdleWorkerEvents};
    #[test]
    fn actual_ui_receivers_preserve_opposite_callback_orders_dedup_and_local_filter() {
        let mut first = IdleWorkerCallbacks::new();
        let mut peer = IdleWorkerCallbacks::new();
        let add = IdleWorkerEvent::Add {
            object: 17,
            player: 2,
        };
        let remove = IdleWorkerEvent::Remove {
            object: 17,
            player: 2,
        };
        first.apply_idle_worker_events(2, [add, remove]);
        assert_eq!(first.get_idle_worker_count(), 0);
        first.apply_idle_worker_events(2, [remove, add, add]);
        assert_eq!(first.idle_workers, vec![17]);
        assert_eq!(first.get_idle_worker_count(), 1);
        peer.apply_idle_worker_events(7, [add, remove]);
        assert_eq!(peer.get_idle_worker_count(), 0);
        let mut events = IdleWorkerEvents::default();
        events.remove(17, 7);
        events.add(17, 7);
        peer.apply_idle_worker_events(7, events.take());
        assert_eq!(peer.idle_workers, vec![17]);
        first.apply_idle_worker_events(
            2,
            [IdleWorkerEvent::Remove {
                object: 17,
                player: 7,
            }],
        );
        assert_eq!(first.idle_workers, vec![17]);
        peer.apply_idle_worker_events(7, events.take());
        assert_eq!(peer.get_idle_worker_count(), 1);
    }
}

/// ReplayControls.cpp intentionally has no playback mutations in these callbacks.
/// The actual native Recorder remains driven by its existing receiving host route.
pub fn replay_control_system_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_ingame_ui(|owner| owner.replay.system(window, msg, data1, data2))
}
pub fn replay_control_input_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_ingame_ui(|owner| owner.replay.input(window, msg, data1, data2))
}
pub fn idle_worker_system_receiving(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    use crate::core::game_client::ClientInputAuthority;
    match msg {
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            if data1 as u32
                == NameKeyGenerator::name_to_key("IdleWorker.wnd:ButtonSelectNextIdleWorker")
            {
                if effects.input_authority() == ClientInputAuthority::NativeHosted {
                    if !publish_host_select_next_idle_worker(effects.ui_mut()) {
                        log::error!(
                            "native idle worker callback has no receiving host request route"
                        );
                        return WindowMsgHandled::Ignored;
                    }
                } else {
                    let object =
                        effects.with_ingame_ui(|owner| owner.idle_worker.next_idle_worker());
                    if let Some(object) = object {
                        // No callback-owner loan crosses this old Core selection boundary.
                        let local = ThePlayerList()
                            .read()
                            .ok()
                            .map(|list| list.get_local_player_index());
                        if let Some(local) = local {
                            if let Ok(mut manager) =
                                gamelogic::commands::selection::get_selection_manager().write()
                            {
                                if let Some(selection) = manager.get_player_selection(local) {
                                    let _ = selection.select_objects(
                                        vec![object],
                                        gamelogic::commands::selection::SelectionType::Replace,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::None => {
            if effects.input_authority() == ClientInputAuthority::StandaloneCore {
                let events = gamelogic::helpers::TheInGameUI::take_idle_worker_events();
                let local = ThePlayerList()
                    .read()
                    .ok()
                    .map(|list| list.get_local_player_index())
                    .unwrap_or(gamelogic::player::PLAYER_INDEX_INVALID);
                let count = effects.with_ingame_ui(|owner| {
                    owner
                        .idle_worker
                        .apply_idle_worker_events(local as i32, events);
                    owner.idle_worker.worker_count
                });
                let enabled = count > 0 && effects.ui().get_input_enabled();
                effects.with_manager(|manager, _| {
                    if let Some(button) = manager.get_window_by_id(NameKeyGenerator::name_to_key(
                        "ControlBar.wnd:ButtonIdleWorker",
                    ) as i32)
                    {
                        let _ = button.borrow_mut().enable(enabled);
                    }
                });
            }
            // Native already presents its actual Main idle roster; never consume
            // the separate Core queue or overwrite that owner's button state.
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
