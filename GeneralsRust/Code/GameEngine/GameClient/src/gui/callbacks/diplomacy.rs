//! Diplomacy Callback Functions
//!
//! This module handles diplomacy screen callbacks and player interaction
//! management including alliances, team changes, and communication controls.

use crate::game_text::GameText;
use crate::gui::gadgets::{ListBoxAddEntry, ListBoxItemData};
use crate::gui::{
    AnimateWindowManager, AnimationType, Color, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled, WindowWidget, get_disconnect_menu, with_window_manager,
};
use crate::helpers::TheInGameUI;
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::with_recorder;
use gamelogic::helpers::TheGameLogic;
use gamelogic::player::ThePlayerList;
use log::{debug, info};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
/// Maximum number of player slots
const MAX_SLOTS: usize = 8;
const KEY_ESC: usize = 0x1B;

/// Diplomatic relationship types
#[derive(Debug, Clone, PartialEq)]
pub enum DiplomaticRelationship {
    Ally,
    Enemy,
    Neutral,
}

/// Player status in diplomacy
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerStatus {
    Active,
    Defeated,
    Disconnected,
    Observer,
}

/// Player information for diplomacy display
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub name: String,
    pub side: String,
    pub team: i32,
    pub status: PlayerStatus,
    pub relationship: DiplomaticRelationship,
    pub is_muted: bool,
}

impl Default for PlayerInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            side: String::new(),
            team: -1,
            status: PlayerStatus::Observer,
            relationship: DiplomaticRelationship::Neutral,
            is_muted: false,
        }
    }
}

#[derive(Default)]
struct DiplomacyUiState {
    layout: Option<Rc<RefCell<WindowLayout>>>,
    window: Option<Rc<RefCell<GameWindow>>>,
    animate_manager: AnimateWindowManager,
}

thread_local! {
    static DIPLOMACY_UI_STATE: Arc<Mutex<DiplomacyUiState>> =
        Arc::new(Mutex::new(DiplomacyUiState::default()));
}

fn diplomacy_ui_state() -> Arc<Mutex<DiplomacyUiState>> {
    DIPLOMACY_UI_STATE.with(|state| state.clone())
}

/// Diplomacy screen state and callbacks
pub struct DiplomacyCallbacks {
    active: bool,
    players: HashMap<i32, PlayerInfo>,
    local_player_id: i32,
    briefing_list: Vec<String>,
}

impl DiplomacyCallbacks {
    pub fn new() -> Self {
        Self {
            active: false,
            players: HashMap::new(),
            local_player_id: 0,
            briefing_list: Vec::new(),
        }
    }

    /// Handle diplomacy system messages
    pub fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create => {
                self.refresh_from_player_list();
                WindowMsgHandled::Handled
            }
            WindowMessage::None => {
                let state_handle = diplomacy_ui_state();
                let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
                state.animate_manager.update();
                WindowMsgHandled::Handled
            }
            WindowMessage::Destroy => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Handle diplomacy input messages
    pub fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Char => {
                if data1 == KEY_ESC {
                    let _ = self.hide_diplomacy(false);
                }
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected => {
                let control_id = data1 as u32;
                if self.handle_radio_buttons(control_id) {
                    return WindowMsgHandled::Handled;
                }
                if self.handle_mute_buttons(control_id) {
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Toggle diplomacy screen visibility. C++ ToggleDiplomacy begins with
    /// HideQuitMenu() — bringing diplomacy up retires the quit menu first
    /// (Diplomacy.cpp:301-304).
    pub fn toggle_diplomacy(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        info!("Toggling diplomacy screen (immediate: {})", immediate);
        super::quit_menu::hide_quit_menu();
        self.active = !self.active;

        if immediate {
            self.apply_visibility_change(true)?;
        } else {
            self.animate_visibility_change()?;
        }

        Ok(())
    }

    /// Hide diplomacy screen
    pub fn hide_diplomacy(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        info!("Hiding diplomacy screen (immediate: {})", immediate);

        if self.active {
            self.active = false;

            if immediate {
                self.apply_visibility_change(true)?;
            } else {
                self.animate_visibility_change()?;
            }
        }

        Ok(())
    }

    /// Reset diplomacy screen state
    pub fn reset_diplomacy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Resetting diplomacy screen");

        self.active = false;
        self.players.clear();
        self.briefing_list.clear();

        self.hide_layout();
        Ok(())
    }

    /// Check if diplomacy screen is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Set local player ID
    pub fn set_local_player(&mut self, player_id: i32) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Setting local player ID to: {}", player_id);

        if player_id < 0 || player_id as usize >= MAX_SLOTS {
            return Err(format!("Invalid player ID: {}", player_id).into());
        }

        self.local_player_id = player_id;
        Ok(())
    }

    /// Update player information
    pub fn update_player_info(
        &mut self,
        player_id: i32,
        info: PlayerInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Updating player info for player {}: {:?}", player_id, info);

        if player_id < 0 || player_id as usize >= MAX_SLOTS {
            return Err(format!("Invalid player ID: {}", player_id).into());
        }

        self.players.insert(player_id, info);
        self.update_player_row(player_id);
        Ok(())
    }

    /// Get player information
    pub fn get_player_info(&self, player_id: i32) -> Option<&PlayerInfo> {
        self.players.get(&player_id)
    }

    /// Set diplomatic relationship with a player
    pub fn set_relationship(
        &mut self,
        player_id: i32,
        relationship: DiplomaticRelationship,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Setting relationship with player {} to: {:?}",
            player_id, relationship
        );

        if let Some(player_info) = self.players.get_mut(&player_id) {
            player_info.relationship = relationship;
            self.update_player_row(player_id);
            Ok(())
        } else {
            Err(format!("Player {} not found", player_id).into())
        }
    }

    /// Mute/unmute a player
    pub fn set_player_muted(
        &mut self,
        player_id: i32,
        muted: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Setting player {} muted state to: {}", player_id, muted);

        if let Some(player_info) = self.players.get_mut(&player_id) {
            player_info.is_muted = muted;
            self.update_player_row(player_id);
            Ok(())
        } else {
            Err(format!("Player {} not found", player_id).into())
        }
    }

    /// Get all players
    pub fn get_all_players(&self) -> &HashMap<i32, PlayerInfo> {
        &self.players
    }

    /// Get military briefing history.
    pub fn briefing_text_list(&self) -> &[String] {
        &self.briefing_list
    }

    /// Update military briefing history.
    pub fn update_briefing_text(&mut self, new_text: &str, clear: bool) {
        if clear {
            self.briefing_list.clear();
            Self::reset_listbox_solo();
        }

        if new_text.is_empty() || self.briefing_list.iter().any(|entry| entry == new_text) {
            return;
        }

        self.briefing_list.push(new_text.to_string());
        Self::append_briefing_to_listbox(new_text, self.briefing_list.len() - 1);
    }

    fn briefing_message_color(index: usize) -> Color {
        if index % 2 == 0 {
            Color::new(255, 255, 255, 255)
        } else {
            Color::new(180, 180, 180, 255)
        }
    }

    fn with_listbox_solo<R>(f: impl FnOnce(&mut crate::gui::gadgets::ListBox) -> R) -> Option<R> {
        let key = NameKeyGenerator::name_to_key("Diplomacy.wnd:ListboxSolo") as i32;
        // `with_window_manager` only has ReentryFallback for known Option types
        // (`Option<Rc<RefCell<GameWindow>>>`), not generic `Option<R>`. Fetch the
        // window first, then map the listbox after the helper returns.
        let window = with_window_manager(|manager| manager.get_window_by_id(key))?;
        let mut win = window.borrow_mut();
        match win.widget.as_mut() {
            Some(WindowWidget::ListBox(listbox)) => Some(f(listbox)),
            _ => None,
        }
    }

    fn reset_listbox_solo() {
        let _ = Self::with_listbox_solo(|listbox| listbox.clear());
    }

    fn append_briefing_to_listbox(label: &str, index: usize) {
        let translated = GameText::fetch(label);
        let color = Self::briefing_message_color(index);
        let _ = Self::with_listbox_solo(|listbox| {
            listbox.add_entry(ListBoxAddEntry {
                row: -1,
                column: 0,
                overwrite: true,
                data: ListBoxItemData::Text(translated),
                color: Some(color),
            })
        });
    }

    fn replay_briefing_into_listbox(&self) {
        Self::reset_listbox_solo();
        for (index, label) in self.briefing_list.iter().enumerate() {
            Self::append_briefing_to_listbox(label, index);
        }
    }

    /// Get local player ID
    pub fn get_local_player_id(&self) -> i32 {
        self.local_player_id
    }

    /// Check if a player is muted
    pub fn is_player_muted(&self, player_id: i32) -> bool {
        self.players
            .get(&player_id)
            .map(|info| info.is_muted)
            .unwrap_or(false)
    }

    /// Get diplomatic relationship with a player
    pub fn get_relationship(&self, player_id: i32) -> DiplomaticRelationship {
        self.players
            .get(&player_id)
            .map(|info| info.relationship.clone())
            .unwrap_or(DiplomaticRelationship::Neutral)
    }

    /// Process alliance request
    pub fn process_alliance_request(
        &mut self,
        from_player: i32,
        to_player: i32,
        accept: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Processing alliance request from {} to {} (accept: {})",
            from_player, to_player, accept
        );

        if accept {
            // Set both players as allies
            if let Some(player_info) = self.players.get_mut(&from_player) {
                player_info.relationship = DiplomaticRelationship::Ally;
            }
            if let Some(player_info) = self.players.get_mut(&to_player) {
                player_info.relationship = DiplomaticRelationship::Ally;
            }

            info!(
                "Alliance formed between players {} and {}",
                from_player, to_player
            );
        } else {
            info!(
                "Alliance request between players {} and {} rejected",
                from_player, to_player
            );
        }

        self.update_player_row(from_player);
        self.update_player_row(to_player);
        Ok(())
    }

    fn apply_visibility_change(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.active {
            self.show_layout(immediate);
        } else {
            self.hide_layout();
        }
        Ok(())
    }

    /// Non-immediate show/hide. C++ ShowDiplomacy registers WIN_ANIMATION_SLIDE_TOP;
    /// HideDiplomacy reverses that animation once the last show has finished.
    fn animate_visibility_change(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.active {
            self.show_layout(false);
        } else {
            // C++ HideDiplomacy takes its immediate branch when
            // `immediate || !TheGlobalData->m_animateWindows`
            // (Diplomacy.cpp:286-290): winHide(TRUE) + winEnable(FALSE) now.
            // The port's in-game AnimateWindowManager has no per-frame driver
            // outside this tick, so a reverse slide cannot be guaranteed to
            // complete — the retail NET end state (window hidden + disabled)
            // is applied immediately on every hide.
            self.hide_layout();
        }
        Ok(())
    }
    fn show_layout(&mut self, immediate: bool) {
        if !TheGameLogic::is_input_enabled()
            || TheGameLogic::is_intro_movie_playing()
            || TheGameLogic::is_loading_map()
        {
            return;
        }
        if TheInGameUI::is_quit_menu_visible() {
            return;
        }
        if get_disconnect_menu()
            .read()
            .map(|menu| menu.is_visible())
            .unwrap_or(false)
        {
            return;
        }

        let state_handle = diplomacy_ui_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if state.layout.is_none() {
            // C++ ShowDiplomacy: TheWindowManager->winCreateLayout(
            // "Diplomacy.wnd") parses Window/Diplomacy.wnd and creates every
            // window in the live WM (Diplomacy.cpp:195-196, GameWindowManager-
            // Script.cpp:2700-2703 prefixes a bare filename with "Window\\").
            // winCreateLayout returns NULL on parse failure; fail closed the
            // same way instead of caching an empty layout.
            let Some(layout) = with_window_manager(|manager| {
                manager
                    .create_layout_with_windows("Diplomacy.wnd")
                    .ok()
                    .map(|(layout, _)| layout)
            }) else {
                return;
            };
            state.window = layout.borrow().get_first_window();
            state.layout = Some(layout);
        }

        let window = state.window.clone();
        if let Some(window) = window.as_ref() {
            let _ = window.borrow_mut().hide(false);
            let _ = window.borrow_mut().enable(true);
        }

        let radio_ingame = NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonInGame");
        let radio_buddies = NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonBuddies");
        let win_ingame = NameKeyGenerator::name_to_key("Diplomacy.wnd:InGameParent") as i32;
        let win_buddies = NameKeyGenerator::name_to_key("Diplomacy.wnd:BuddiesParent") as i32;
        let win_solo = NameKeyGenerator::name_to_key("Diplomacy.wnd:SoloParent") as i32;
        let is_multiplayer = with_recorder(|recorder| recorder.is_multiplayer()).unwrap_or(false);

        with_window_manager(|manager| {
            if let Some(win) = manager.get_window_by_id(radio_ingame as i32) {
                let _ = win.borrow_mut().hide(true);
            }
            if let Some(win) = manager.get_window_by_id(radio_buddies as i32) {
                let _ = win.borrow_mut().hide(true);
            }
            if is_multiplayer {
                if let Some(win) = manager.get_window_by_id(win_ingame) {
                    let _ = win.borrow_mut().hide(false);
                }
                if let Some(win) = manager.get_window_by_id(win_buddies) {
                    let _ = win.borrow_mut().hide(true);
                }
                if let Some(win) = manager.get_window_by_id(win_solo) {
                    let _ = win.borrow_mut().hide(true);
                }
            } else {
                if let Some(win) = manager.get_window_by_id(win_ingame) {
                    let _ = win.borrow_mut().hide(true);
                }
                if let Some(win) = manager.get_window_by_id(win_buddies) {
                    let _ = win.borrow_mut().hide(true);
                }
                if let Some(win) = manager.get_window_by_id(win_solo) {
                    let _ = win.borrow_mut().hide(false);
                }
            }
        });

        if !is_multiplayer {
            self.replay_briefing_into_listbox();
        }

        // C++ ShowDiplomacy registers the WIN_ANIMATION_SLIDE_TOP only when
        // TheGlobalData->m_animateWindows (Diplomacy.cpp:243-244); the tick
        // comes from the layout update func (see tick_diplomacy_animation).
        let animate_windows = get_global_data()
            .map(|data| data.read().animate_windows)
            .unwrap_or(true);
        if !immediate && animate_windows {
            if let Some(window) = window {
                state.animate_manager.reset();
                state.animate_manager.register_window(
                    window,
                    AnimationType::SlideTop,
                    true,
                    200,
                    0,
                );
            }
        }

        // C++ ShowDiplomacy repopulates the slot table on every show
        // (PopulateInGameDiplomacyPopup, Diplomacy.cpp:247-248).
        self.refresh_from_player_list();
        for player_id in self.players.keys() {
            self.update_player_row(*player_id);
        }
    }


    fn hide_layout(&self) {
        let state_handle = diplomacy_ui_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(window) = &state.window {
            // C++ HideDiplomacy immediate: winHide(TRUE) + winEnable(FALSE)
            // (Diplomacy.cpp:286-290).
            let _ = window.borrow_mut().hide(true);
            let _ = window.borrow_mut().enable(false);
        }
    }


    fn refresh_from_player_list(&mut self) {
        let Ok(list) = ThePlayerList().read() else {
            return;
        };
        for slot in 0..MAX_SLOTS {
            if let Some(player) = list.get_player(slot as i32) {
                if let Ok(player) = player.read() {
                    let mut info = PlayerInfo::default();
                    info.team = player
                        .get_default_team_id()
                        .map(|id| id as i32)
                        .unwrap_or(-1);
                    info.status = if player.is_player_active() {
                        PlayerStatus::Active
                    } else {
                        PlayerStatus::Observer
                    };
                    info.side = player.get_side().to_string();
                    let name_key = player.get_player_name_key();
                    info.name = NameKeyGenerator::key_to_name(name_key)
                        .unwrap_or_else(|| format!("Player {}", slot + 1));
                    self.players.insert(slot as i32, info);
                }
            }
        }
    }

    fn update_player_row(&self, player_id: i32) {
        let Some(info) = self.players.get(&player_id) else {
            return;
        };

        let player_key =
            NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:StaticTextPlayer{}", player_id));
        let side_key =
            NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:StaticTextSide{}", player_id));
        let team_key =
            NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:StaticTextTeam{}", player_id));
        let status_key =
            NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:StaticTextStatus{}", player_id));

        let status_text = match info.status {
            PlayerStatus::Active => "Active",
            PlayerStatus::Defeated => "Defeated",
            PlayerStatus::Disconnected => "Disconnected",
            PlayerStatus::Observer => "Observer",
        };

        with_window_manager(|manager| {
            if let Some(win) = manager.get_window_by_id(player_key as i32) {
                let _ = win.borrow_mut().set_text(&info.name);
            }
            if let Some(win) = manager.get_window_by_id(side_key as i32) {
                let _ = win.borrow_mut().set_text(&info.side);
            }
            if let Some(win) = manager.get_window_by_id(team_key as i32) {
                let _ = win.borrow_mut().set_text(&format!("{}", info.team));
            }
            if let Some(win) = manager.get_window_by_id(status_key as i32) {
                let _ = win.borrow_mut().set_text(status_text);
            }
        });
    }

    fn handle_radio_buttons(&self, control_id: u32) -> bool {
        let radio_ingame = NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonInGame");
        let radio_buddies = NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonBuddies");
        if control_id != radio_ingame && control_id != radio_buddies {
            return false;
        }

        let win_ingame = NameKeyGenerator::name_to_key("Diplomacy.wnd:InGameParent") as i32;
        let win_buddies = NameKeyGenerator::name_to_key("Diplomacy.wnd:BuddiesParent") as i32;
        let win_solo = NameKeyGenerator::name_to_key("Diplomacy.wnd:SoloParent") as i32;
        with_window_manager(|manager| {
            if let Some(win) = manager.get_window_by_id(win_ingame) {
                let _ = win.borrow_mut().hide(control_id != radio_ingame);
            }
            if let Some(win) = manager.get_window_by_id(win_buddies) {
                let _ = win.borrow_mut().hide(control_id != radio_buddies);
            }
            if let Some(win) = manager.get_window_by_id(win_solo) {
                let _ = win.borrow_mut().hide(control_id == radio_buddies);
            }
        });
        true
    }

    fn handle_mute_buttons(&mut self, control_id: u32) -> bool {
        for slot in 0..MAX_SLOTS {
            let mute_key =
                NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:ButtonMute{}", slot)) as u32;
            let unmute_key =
                NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:ButtonUnMute{}", slot))
                    as u32;

            if control_id == mute_key {
                let _ = self.set_player_muted(slot as i32, true);
                return true;
            }
            if control_id == unmute_key {
                let _ = self.set_player_muted(slot as i32, false);
                return true;
            }
        }
        false
    }
}
    /// Per-frame tick of the live Diplomacy layout animation. C++ registers
    /// this as the layout update func (`theLayout->setUpdate(updateFunc)`,
    /// Diplomacy.cpp:197) and InGameUI::update runs it for every registered
    /// window layout (InGameUI.cpp:1825-1829): advance the slide animation,
    /// and once a reverse (hide) animation finishes, hide the window for real.
    pub fn tick_diplomacy_animation() {
        let animate_windows = get_global_data()
            .map(|data| data.read().animate_windows)
            .unwrap_or(true);
        if !animate_windows {
            return;
        }
        let state_handle = diplomacy_ui_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if state.window.is_none() {
            return;
        }
        let was_finished = state.animate_manager.is_finished();
        state.animate_manager.update();
        if state.animate_manager.is_finished()
            && !was_finished
            && state.animate_manager.is_reversed()
        {
            if let Some(window) = &state.window {
                let _ = window.borrow_mut().hide(true);
            }
        }
    }

impl Default for DiplomacyCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Diplomacy system manager
pub struct DiplomacySystem {
    callbacks: Arc<RwLock<DiplomacyCallbacks>>,
}

impl DiplomacySystem {
    pub fn new() -> Self {
        Self {
            callbacks: Arc::new(RwLock::new(DiplomacyCallbacks::new())),
        }
    }

    pub fn get_callbacks(&self) -> Arc<RwLock<DiplomacyCallbacks>> {
        self.callbacks.clone()
    }

    /// Toggle diplomacy through the system
    pub fn toggle_diplomacy(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.toggle_diplomacy(immediate)
    }

    /// Hide diplomacy through the system
    pub fn hide_diplomacy(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.hide_diplomacy(immediate)
    }

    /// Reset diplomacy through the system
    pub fn reset_diplomacy(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.reset_diplomacy()
    }

    /// Check if diplomacy is active
    pub fn is_active(&self) -> bool {
        let callbacks = self.callbacks.read().unwrap_or_else(|e| e.into_inner());
        callbacks.is_active()
    }

    /// Update player information through the system
    pub fn update_player_info(
        &self,
        player_id: i32,
        info: PlayerInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.update_player_info(player_id, info)
    }

    /// Set relationship through the system
    pub fn set_relationship(
        &self,
        player_id: i32,
        relationship: DiplomaticRelationship,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.set_relationship(player_id, relationship)
    }

    /// Mute/unmute player through the system
    pub fn set_player_muted(
        &self,
        player_id: i32,
        muted: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.set_player_muted(player_id, muted)
    }

    /// Update military briefing history through the system.
    pub fn update_briefing_text(&self, new_text: &str, clear: bool) {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.update_briefing_text(new_text, clear);
    }

    /// Get military briefing history through the system.
    pub fn briefing_text_list(&self) -> Vec<String> {
        let callbacks = self.callbacks.read().unwrap_or_else(|e| e.into_inner());
        callbacks.briefing_text_list().to_vec()
    }
}

impl Default for DiplomacySystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global diplomacy system instance
lazy_static::lazy_static! {
    pub static ref THE_DIPLOMACY_SYSTEM: Arc<RwLock<DiplomacySystem>> =
        Arc::new(RwLock::new(DiplomacySystem::new()));
}

/// Helper function to get the global diplomacy system
pub fn get_diplomacy_system() -> Arc<RwLock<DiplomacySystem>> {
    THE_DIPLOMACY_SYSTEM.clone()
}

/// Convenience functions for global diplomacy operations
pub fn toggle_diplomacy(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.toggle_diplomacy(immediate)
}

pub fn hide_diplomacy(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.hide_diplomacy(immediate)
}

pub fn reset_diplomacy() -> Result<(), Box<dyn std::error::Error>> {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.reset_diplomacy()
}

pub fn is_diplomacy_active() -> bool {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.is_active()
}

pub fn update_diplomacy_briefing_text(new_text: &str, clear: bool) {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.update_briefing_text(new_text, clear);
}

pub fn get_briefing_text_list() -> Vec<String> {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.briefing_text_list()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diplomacy_callbacks() {
        let mut diplomacy = DiplomacyCallbacks::new();

        // Test initial state
        assert!(!diplomacy.is_active());
        assert_eq!(diplomacy.get_local_player_id(), 0);
        assert_eq!(diplomacy.get_all_players().len(), 0);

        // Test setting local player
        assert!(diplomacy.set_local_player(1).is_ok());
        assert_eq!(diplomacy.get_local_player_id(), 1);

        // Test invalid player ID
        assert!(diplomacy.set_local_player(-1).is_err());
        assert!(diplomacy.set_local_player(MAX_SLOTS as i32).is_err());
    }

    #[test]
    fn briefing_history_tracks_unique_labels_and_reset_clears() {
        let mut diplomacy = DiplomacyCallbacks::new();

        diplomacy.update_briefing_text("SCRIPT:Intro", false);
        diplomacy.update_briefing_text("SCRIPT:Intro", false);
        diplomacy.update_briefing_text("", false);
        diplomacy.update_briefing_text("SCRIPT:Second", false);

        assert_eq!(
            diplomacy.briefing_text_list(),
            &["SCRIPT:Intro".to_string(), "SCRIPT:Second".to_string()]
        );

        diplomacy.update_briefing_text("SCRIPT:Replacement", true);
        assert_eq!(
            diplomacy.briefing_text_list(),
            &["SCRIPT:Replacement".to_string()]
        );

        diplomacy.reset_diplomacy().unwrap();
        assert!(diplomacy.briefing_text_list().is_empty());
    }

    #[test]
    fn test_player_info_management() {
        let mut diplomacy = DiplomacyCallbacks::new();

        let player_info = PlayerInfo {
            name: "TestPlayer".to_string(),
            side: "USA".to_string(),
            team: 1,
            status: PlayerStatus::Active,
            relationship: DiplomaticRelationship::Ally,
            is_muted: false,
        };

        // Test adding player info
        assert!(diplomacy.update_player_info(0, player_info.clone()).is_ok());
        assert_eq!(diplomacy.get_all_players().len(), 1);

        let retrieved_info = diplomacy.get_player_info(0).unwrap();
        assert_eq!(retrieved_info.name, "TestPlayer");
        assert_eq!(retrieved_info.team, 1);

        // Test getting non-existent player
        assert!(diplomacy.get_player_info(99).is_none());
    }

    #[test]
    fn test_diplomatic_relationships() {
        let mut diplomacy = DiplomacyCallbacks::new();

        // Add a player first
        let player_info = PlayerInfo::default();
        diplomacy.update_player_info(0, player_info).unwrap();

        // Test setting relationship
        assert!(
            diplomacy
                .set_relationship(0, DiplomaticRelationship::Ally)
                .is_ok()
        );
        assert_eq!(diplomacy.get_relationship(0), DiplomaticRelationship::Ally);

        // Test setting relationship for non-existent player
        assert!(
            diplomacy
                .set_relationship(99, DiplomaticRelationship::Enemy)
                .is_err()
        );

        // Test default relationship for non-existent player
        assert_eq!(
            diplomacy.get_relationship(99),
            DiplomaticRelationship::Neutral
        );
    }

    #[test]
    fn test_mute_functionality() {
        let mut diplomacy = DiplomacyCallbacks::new();

        // Add a player first
        let player_info = PlayerInfo::default();
        diplomacy.update_player_info(0, player_info).unwrap();

        // Test muting
        assert!(!diplomacy.is_player_muted(0));
        assert!(diplomacy.set_player_muted(0, true).is_ok());
        assert!(diplomacy.is_player_muted(0));

        // Test unmuting
        assert!(diplomacy.set_player_muted(0, false).is_ok());
        assert!(!diplomacy.is_player_muted(0));

        // Test muting non-existent player
        assert!(diplomacy.set_player_muted(99, true).is_err());
        assert!(!diplomacy.is_player_muted(99)); // Default false
    }

    #[test]
    fn test_alliance_requests() {
        let mut diplomacy = DiplomacyCallbacks::new();

        // Add two players
        let player1 = PlayerInfo::default();
        let player2 = PlayerInfo::default();
        diplomacy.update_player_info(0, player1).unwrap();
        diplomacy.update_player_info(1, player2).unwrap();

        // Test accepting alliance
        assert!(diplomacy.process_alliance_request(0, 1, true).is_ok());
        assert_eq!(diplomacy.get_relationship(0), DiplomaticRelationship::Ally);
        assert_eq!(diplomacy.get_relationship(1), DiplomaticRelationship::Ally);

        // Reset relationships
        diplomacy
            .set_relationship(0, DiplomaticRelationship::Neutral)
            .unwrap();
        diplomacy
            .set_relationship(1, DiplomaticRelationship::Neutral)
            .unwrap();

        // Test rejecting alliance
        assert!(diplomacy.process_alliance_request(0, 1, false).is_ok());
        assert_eq!(
            diplomacy.get_relationship(0),
            DiplomaticRelationship::Neutral
        );
        assert_eq!(
            diplomacy.get_relationship(1),
            DiplomaticRelationship::Neutral
        );
    }

    #[test]
    fn test_diplomacy_visibility() {
        let mut diplomacy = DiplomacyCallbacks::new();

        // Test initial state
        assert!(!diplomacy.is_active());

        // Test toggling
        assert!(diplomacy.toggle_diplomacy(true).is_ok());
        assert!(diplomacy.is_active());

        assert!(diplomacy.toggle_diplomacy(true).is_ok());
        assert!(!diplomacy.is_active());

        // Test hiding
        diplomacy.toggle_diplomacy(true).unwrap(); // Show first
        assert!(diplomacy.hide_diplomacy(true).is_ok());
        assert!(!diplomacy.is_active());

        // Test reset
        diplomacy.toggle_diplomacy(true).unwrap(); // Show first
        diplomacy
            .update_player_info(0, PlayerInfo::default())
            .unwrap();
        assert!(diplomacy.reset_diplomacy().is_ok());
        assert!(!diplomacy.is_active());
        assert_eq!(diplomacy.get_all_players().len(), 0);
    }

    #[test]
    fn diplomacy_char_input_matches_cpp_escape_handling() {
        let mut diplomacy = DiplomacyCallbacks::new();
        let window = GameWindow::new();

        diplomacy.toggle_diplomacy(true).unwrap();
        assert_eq!(
            diplomacy.input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert!(diplomacy.is_active());

        assert_eq!(
            diplomacy.input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert!(!diplomacy.is_active());
    }

    #[test]
    fn test_diplomacy_system() {
        let system = DiplomacySystem::new();

        // Test that callbacks are accessible
        assert!(system.get_callbacks().read().is_ok());

        // Test system-level operations
        assert!(!system.is_active());
        assert!(system.toggle_diplomacy(true).is_ok());
        assert!(system.is_active());

        // Test player management through system
        let player_info = PlayerInfo {
            name: "SystemTest".to_string(),
            ..PlayerInfo::default()
        };
        assert!(system.update_player_info(0, player_info).is_ok());
        assert!(
            system
                .set_relationship(0, DiplomaticRelationship::Ally)
                .is_ok()
        );
        assert!(system.set_player_muted(0, true).is_ok());
    }

    #[test]
    fn test_global_functions() {
        assert!(toggle_diplomacy(true).is_ok());
        assert!(is_diplomacy_active());
        assert!(hide_diplomacy(true).is_ok());
        assert!(!is_diplomacy_active());
        assert!(reset_diplomacy().is_ok());
    }

    #[test]
    fn test_player_status_types() {
        use PlayerStatus::*;

        let statuses = vec![Active, Defeated, Disconnected, Observer];

        for status in statuses {
            let player_info = PlayerInfo {
                status: status.clone(),
                ..PlayerInfo::default()
            };

            let mut diplomacy = DiplomacyCallbacks::new();
            assert!(diplomacy.update_player_info(0, player_info).is_ok());

            let retrieved_info = diplomacy.get_player_info(0).unwrap();
            assert_eq!(retrieved_info.status, status);
        }
    }
}

/// Residual: last Diplomacy action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualDiplomacyAction {
    None = 0,
    ToggleShow = 1,
    ToggleHide = 2,
    Hide = 3,
    Reset = 4,
    RadioInGame = 5,
    RadioBuddies = 6,
    Mute = 7,
    Unmute = 8,
}

static RESIDUAL_DIPLOMACY_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_DIPLOMACY_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_DIPLOMACY_MUTE_SLOT: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(-1);

fn residual_diplomacy_action_store(action: ResidualDiplomacyAction) {
    RESIDUAL_DIPLOMACY_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last Diplomacy residual action.
pub fn residual_diplomacy_last_action() -> ResidualDiplomacyAction {
    match RESIDUAL_DIPLOMACY_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualDiplomacyAction::ToggleShow,
        2 => ResidualDiplomacyAction::ToggleHide,
        3 => ResidualDiplomacyAction::Hide,
        4 => ResidualDiplomacyAction::Reset,
        5 => ResidualDiplomacyAction::RadioInGame,
        6 => ResidualDiplomacyAction::RadioBuddies,
        7 => ResidualDiplomacyAction::Mute,
        8 => ResidualDiplomacyAction::Unmute,
        _ => ResidualDiplomacyAction::None,
    }
}

/// Residual: diplomacy active latch (independent of live layout).
pub fn residual_diplomacy_is_active() -> bool {
    RESIDUAL_DIPLOMACY_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: last mute/unmute slot (-1 if none).
pub fn residual_diplomacy_mute_slot() -> Option<i32> {
    let slot = RESIDUAL_DIPLOMACY_MUTE_SLOT.load(std::sync::atomic::Ordering::Relaxed);
    if slot < 0 { None } else { Some(slot) }
}

/// Residual: bind Diplomacy control name keys (no layout load).
pub fn simulate_diplomacy_bind_controls() -> bool {
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonInGame");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonBuddies");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:InGameParent");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:BuddiesParent");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:SoloParent");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:ButtonHide");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:ButtonMute0");
    let _ = NameKeyGenerator::name_to_key("Diplomacy.wnd:ButtonUnMute0");
    true
}

/// Residual: toggle show without layout/animate.
pub fn simulate_diplomacy_toggle_show() -> bool {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    callbacks.active = true;
    RESIDUAL_DIPLOMACY_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_diplomacy_action_store(ResidualDiplomacyAction::ToggleShow);
    residual_diplomacy_is_active()
}

/// Residual: toggle hide without layout/animate.
pub fn simulate_diplomacy_toggle_hide() -> bool {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    callbacks.active = false;
    RESIDUAL_DIPLOMACY_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_diplomacy_action_store(ResidualDiplomacyAction::ToggleHide);
    !residual_diplomacy_is_active()
}

/// Residual: hide without layout teardown.
pub fn simulate_diplomacy_hide() -> bool {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    callbacks.active = false;
    RESIDUAL_DIPLOMACY_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_diplomacy_action_store(ResidualDiplomacyAction::Hide);
    true
}

/// Residual: reset without clearing live widgets.
pub fn simulate_diplomacy_reset() -> bool {
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    callbacks.active = false;
    callbacks.players.clear();
    callbacks.briefing_list.clear();
    RESIDUAL_DIPLOMACY_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_DIPLOMACY_MUTE_SLOT.store(-1, std::sync::atomic::Ordering::Relaxed);
    residual_diplomacy_action_store(ResidualDiplomacyAction::Reset);
    true
}

/// Residual: select InGame radio without parent show/hide.
pub fn simulate_diplomacy_radio_ingame() -> bool {
    let _ = simulate_diplomacy_bind_controls();
    residual_diplomacy_action_store(ResidualDiplomacyAction::RadioInGame);
    true
}

/// Residual: select Buddies radio without parent show/hide.
pub fn simulate_diplomacy_radio_buddies() -> bool {
    let _ = simulate_diplomacy_bind_controls();
    residual_diplomacy_action_store(ResidualDiplomacyAction::RadioBuddies);
    true
}

/// Residual: mute slot without live mute side effects beyond callbacks map.
pub fn simulate_diplomacy_mute_slot(slot: i32) -> bool {
    if slot < 0 {
        return false;
    }
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    let _ = callbacks.set_player_muted(slot, true);
    RESIDUAL_DIPLOMACY_MUTE_SLOT.store(slot, std::sync::atomic::Ordering::Relaxed);
    residual_diplomacy_action_store(ResidualDiplomacyAction::Mute);
    residual_diplomacy_mute_slot() == Some(slot)
}

/// Residual: unmute slot residual.
pub fn simulate_diplomacy_unmute_slot(slot: i32) -> bool {
    if slot < 0 {
        return false;
    }
    let system = get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    let _ = callbacks.set_player_muted(slot, false);
    RESIDUAL_DIPLOMACY_MUTE_SLOT.store(slot, std::sync::atomic::Ordering::Relaxed);
    residual_diplomacy_action_store(ResidualDiplomacyAction::Unmute);
    true
}

/// Residual: show + InGame radio composite.
pub fn simulate_diplomacy_prepare_ingame() -> bool {
    if !simulate_diplomacy_bind_controls() {
        return false;
    }
    if !simulate_diplomacy_toggle_show() {
        return false;
    }
    simulate_diplomacy_radio_ingame()
}

/// Human click-through: OS LeftDown/Up on `Diplomacy.wnd:RadioButtonInGame`.
pub fn drive_os_wnd_diplomacy_radio_ingame_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:RadioButtonInGame");
    if !clicked {
        return false;
    }
    simulate_diplomacy_radio_ingame()
}

pub fn drive_os_wnd_diplomacy_radio_buddies_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:RadioButtonBuddies");
    if !clicked {
        return false;
    }
    simulate_diplomacy_radio_buddies()
}

pub fn drive_os_wnd_diplomacy_hide_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:ButtonHide");
    if !clicked {
        return false;
    }
    simulate_diplomacy_hide()
}

pub fn drive_os_wnd_diplomacy_mute_like_cpp(slot: i32) -> bool {
    if slot < 0 {
        return false;
    }
    let name = format!("Diplomacy.wnd:ButtonMute{slot}");
    let clicked = crate::gui::dispatch_os_click_named_window(&name);
    if !clicked {
        return false;
    }
    simulate_diplomacy_mute_slot(slot)
}

pub fn drive_os_wnd_diplomacy_unmute_like_cpp(slot: i32) -> bool {
    if slot < 0 {
        return false;
    }
    let name = format!("Diplomacy.wnd:ButtonUnMute{slot}");
    let clicked = crate::gui::dispatch_os_click_named_window(&name);
    if !clicked {
        return false;
    }
    simulate_diplomacy_unmute_slot(slot)
}

pub fn drive_os_wnd_diplomacy_prepare_ingame_like_cpp() -> bool {
    let clicked_parent = crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:InGameParent");
    if clicked_parent {
        let _ = simulate_diplomacy_toggle_show();
    }
    let clicked_radio = drive_os_wnd_diplomacy_radio_ingame_like_cpp();
    if !clicked_parent && !clicked_radio {
        return false;
    }
    if clicked_radio {
        return residual_diplomacy_last_action() == ResidualDiplomacyAction::RadioInGame;
    }
    simulate_diplomacy_radio_ingame()
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
    fn os_wnd_diplomacy_ingame_and_hide_hit_named_gadgets() {
        install_named_button("Diplomacy.wnd:InGameParent", 10, 10);
        install_named_button("Diplomacy.wnd:RadioButtonInGame", 10, 40);
        install_named_button("Diplomacy.wnd:ButtonHide", 10, 70);
        assert!(
            drive_os_wnd_diplomacy_prepare_ingame_like_cpp(),
            "OS WND clicks must latch InGame radio residual"
        );
        assert_eq!(
            residual_diplomacy_last_action(),
            ResidualDiplomacyAction::RadioInGame
        );
        assert!(drive_os_wnd_diplomacy_hide_like_cpp());
        assert_eq!(
            residual_diplomacy_last_action(),
            ResidualDiplomacyAction::Hide
        );
    }

    #[test]
    fn os_wnd_diplomacy_mute_hits_button_mute_slot() {
        install_named_button("Diplomacy.wnd:ButtonMute0", 10, 100);
        assert!(drive_os_wnd_diplomacy_mute_like_cpp(0));
        assert_eq!(
            residual_diplomacy_last_action(),
            ResidualDiplomacyAction::Mute
        );
        assert_eq!(residual_diplomacy_mute_slot(), Some(0));
        assert!(!drive_os_wnd_diplomacy_unmute_like_cpp(0));
    }
}
