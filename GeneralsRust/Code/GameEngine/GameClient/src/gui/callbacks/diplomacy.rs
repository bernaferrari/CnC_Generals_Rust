//! Diplomacy Callback Functions
//!
//! This module handles diplomacy screen callbacks and player interaction
//! management including alliances, team changes, and communication controls.

use crate::game_text::GameText;
use crate::gui::gadgets::{ListBoxAddEntry, ListBoxItemData};
use crate::gui::{
    AnimateWindowManager, AnimationType, Color, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled, WindowWidget,
};

use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::with_recorder;
use gamelogic::helpers::TheGameLogic;
use gamelogic::player::ThePlayerList;
use log::{debug, info};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
    slots_in_rows: [Option<i32>; MAX_SLOTS],
}

/// Diplomacy screen state and callbacks
pub struct DiplomacyCallbacks {
    active: bool,
    players: HashMap<i32, PlayerInfo>,
    local_player_id: i32,
    briefing_list: Vec<String>,
    ui: DiplomacyUiState,
    generation: u64,
    retiring: bool,
    last_action: ResidualDiplomacyAction,
    mute_slot: Option<i32>,
}

impl DiplomacyCallbacks {
    pub fn new() -> Self {
        Self {
            active: false,
            players: HashMap::new(),
            local_player_id: 0,
            briefing_list: Vec::new(),
            ui: Default::default(),
            generation: 0,
            retiring: false,
            last_action: ResidualDiplomacyAction::None,
            mute_slot: None,
        }
    }

    /// Handle diplomacy system messages

    /// Handle diplomacy input messages

    /// Toggle diplomacy screen visibility. C++ ToggleDiplomacy begins with
    /// HideQuitMenu() — bringing diplomacy up retires the quit menu first
    /// (Diplomacy.cpp:301-304).

    /// Hide diplomacy screen

    /// Reset diplomacy screen state

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
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Updating player info for player {}: {:?}", player_id, info);

        if player_id < 0 || player_id as usize >= MAX_SLOTS {
            return Err(format!("Invalid player ID: {}", player_id).into());
        }

        self.players.insert(player_id, info);
        self.update_player_row(player_id, receiving_manager);
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
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Setting relationship with player {} to: {:?}",
            player_id, relationship
        );

        if let Some(player_info) = self.players.get_mut(&player_id) {
            player_info.relationship = relationship;
            self.update_player_row(player_id, receiving_manager);
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
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Setting player {} muted state to: {}", player_id, muted);

        if let Some(player_info) = self.players.get_mut(&player_id) {
            player_info.is_muted = muted;
            self.update_player_row(player_id, receiving_manager);
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
    pub fn update_briefing_text(
        &mut self,
        new_text: &str,
        clear: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if clear {
            self.briefing_list.clear();
            Self::reset_listbox_solo(receiving_manager);
        }

        if new_text.is_empty() || self.briefing_list.iter().any(|entry| entry == new_text) {
            return;
        }

        self.briefing_list.push(new_text.to_string());
        Self::append_briefing_to_listbox(new_text, self.briefing_list.len() - 1, receiving_manager);
    }

    fn briefing_message_color(index: usize) -> Color {
        if index % 2 == 0 {
            Color::new(255, 255, 255, 255)
        } else {
            Color::new(180, 180, 180, 255)
        }
    }

    fn with_listbox_solo<R>(
        f: impl FnOnce(&mut crate::gui::gadgets::ListBox) -> R,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Option<R> {
        let key = NameKeyGenerator::name_to_key("Diplomacy.wnd:ListboxSolo") as i32;
        // Resolve the window in this receiving tree, then borrow its listbox.
        let window = (|manager: &mut crate::gui::window_manager::WindowManager| {
            manager.get_window_by_id(key)
        })(receiving_manager)?;
        let mut win = window.borrow_mut();
        match win.widget.as_mut() {
            Some(WindowWidget::ListBox(listbox)) => Some(f(listbox)),
            _ => None,
        }
    }

    fn reset_listbox_solo(receiving_manager: &mut crate::gui::window_manager::WindowManager) {
        let _ = Self::with_listbox_solo(|listbox| listbox.clear(), receiving_manager);
    }

    fn append_briefing_to_listbox(
        label: &str,
        index: usize,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        let translated = GameText::fetch(label);
        let color = Self::briefing_message_color(index);
        let _ = Self::with_listbox_solo(
            |listbox| {
                listbox.add_entry(ListBoxAddEntry {
                    row: -1,
                    column: 0,
                    overwrite: true,
                    data: ListBoxItemData::Text(translated),
                    color: Some(color),
                })
            },
            receiving_manager,
        );
    }

    fn replay_briefing_into_listbox(
        &self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        Self::reset_listbox_solo(receiving_manager);
        for (index, label) in self.briefing_list.iter().enumerate() {
            Self::append_briefing_to_listbox(label, index, receiving_manager);
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
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
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

        self.update_player_row(from_player, receiving_manager);
        self.update_player_row(to_player, receiving_manager);
        Ok(())
    }

    /// Non-immediate show/hide. C++ ShowDiplomacy registers WIN_ANIMATION_SLIDE_TOP;
    /// HideDiplomacy reverses that animation once the last show has finished.

    fn update_player_row(
        &self,
        player_id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
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

        (|manager: &mut crate::gui::window_manager::WindowManager| {
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
        })(receiving_manager);
    }
}
impl Default for DiplomacyCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

mod owner;
pub use owner::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diplomacy_callbacks() {
        let mut diplomacy = DiplomacyCallbacks::new();
        let mut manager = crate::gui::window_manager::WindowManager::new();

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
        let mut manager = crate::gui::window_manager::WindowManager::new();

        diplomacy.update_briefing_text("SCRIPT:Intro", false, &mut manager);
        diplomacy.update_briefing_text("SCRIPT:Intro", false, &mut manager);
        diplomacy.update_briefing_text("", false, &mut manager);
        diplomacy.update_briefing_text("SCRIPT:Second", false, &mut manager);

        assert_eq!(
            diplomacy.briefing_text_list(),
            &["SCRIPT:Intro".to_string(), "SCRIPT:Second".to_string()]
        );

        diplomacy.update_briefing_text("SCRIPT:Replacement", true, &mut manager);
        assert_eq!(
            diplomacy.briefing_text_list(),
            &["SCRIPT:Replacement".to_string()]
        );

        diplomacy.update_briefing_text("", true, &mut manager);
        assert!(diplomacy.briefing_text_list().is_empty());
    }

    #[test]
    fn test_player_info_management() {
        let mut diplomacy = DiplomacyCallbacks::new();
        let mut manager = crate::gui::window_manager::WindowManager::new();

        let player_info = PlayerInfo {
            name: "TestPlayer".to_string(),
            side: "USA".to_string(),
            team: 1,
            status: PlayerStatus::Active,
            relationship: DiplomaticRelationship::Ally,
            is_muted: false,
        };

        // Test adding player info
        assert!(
            diplomacy
                .update_player_info(0, player_info.clone(), &mut manager)
                .is_ok()
        );
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
        let mut manager = crate::gui::window_manager::WindowManager::new();

        // Add a player first
        let player_info = PlayerInfo::default();
        diplomacy
            .update_player_info(0, player_info, &mut manager)
            .unwrap();

        // Test setting relationship
        assert!(
            diplomacy
                .set_relationship(0, DiplomaticRelationship::Ally, &mut manager)
                .is_ok()
        );
        assert_eq!(diplomacy.get_relationship(0), DiplomaticRelationship::Ally);

        // Test setting relationship for non-existent player
        assert!(
            diplomacy
                .set_relationship(99, DiplomaticRelationship::Enemy, &mut manager)
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
        let mut manager = crate::gui::window_manager::WindowManager::new();

        // Add a player first
        let player_info = PlayerInfo::default();
        diplomacy
            .update_player_info(0, player_info, &mut manager)
            .unwrap();

        // Test muting
        assert!(!diplomacy.is_player_muted(0));
        assert!(diplomacy.set_player_muted(0, true, &mut manager).is_ok());
        assert!(diplomacy.is_player_muted(0));

        // Test unmuting
        assert!(diplomacy.set_player_muted(0, false, &mut manager).is_ok());
        assert!(!diplomacy.is_player_muted(0));

        // Test muting non-existent player
        assert!(diplomacy.set_player_muted(99, true, &mut manager).is_err());
        assert!(!diplomacy.is_player_muted(99)); // Default false
    }

    #[test]
    fn test_alliance_requests() {
        let mut diplomacy = DiplomacyCallbacks::new();
        let mut manager = crate::gui::window_manager::WindowManager::new();

        // Add two players
        let player1 = PlayerInfo::default();
        let player2 = PlayerInfo::default();
        diplomacy
            .update_player_info(0, player1, &mut manager)
            .unwrap();
        diplomacy
            .update_player_info(1, player2, &mut manager)
            .unwrap();

        // Test accepting alliance
        assert!(
            diplomacy
                .process_alliance_request(0, 1, true, &mut manager)
                .is_ok()
        );
        assert_eq!(diplomacy.get_relationship(0), DiplomaticRelationship::Ally);
        assert_eq!(diplomacy.get_relationship(1), DiplomaticRelationship::Ally);

        // Reset relationships
        diplomacy
            .set_relationship(0, DiplomaticRelationship::Neutral, &mut manager)
            .unwrap();
        diplomacy
            .set_relationship(1, DiplomaticRelationship::Neutral, &mut manager)
            .unwrap();

        // Test rejecting alliance
        assert!(
            diplomacy
                .process_alliance_request(0, 1, false, &mut manager)
                .is_ok()
        );
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
    fn test_player_status_types() {
        use PlayerStatus::*;

        let statuses = vec![Active, Defeated, Disconnected, Observer];

        for status in statuses {
            let player_info = PlayerInfo {
                status: status.clone(),
                ..PlayerInfo::default()
            };

            let mut diplomacy = DiplomacyCallbacks::new();
            let mut manager = crate::gui::window_manager::WindowManager::new();
            assert!(
                diplomacy
                    .update_player_info(0, player_info, &mut manager)
                    .is_ok()
            );

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

pub fn residual_diplomacy_last_action(owner: &DiplomacyCallbacks) -> ResidualDiplomacyAction {
    owner.last_action
}
pub fn residual_diplomacy_is_active(owner: &DiplomacyCallbacks) -> bool {
    owner.active
}
pub fn residual_diplomacy_mute_slot(owner: &DiplomacyCallbacks) -> Option<i32> {
    owner.mute_slot
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

fn record_residual(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    action: ResidualDiplomacyAction,
    active: Option<bool>,
) -> bool {
    effects.with_diplomacy(|owner| {
        owner.last_action = action;
        if let Some(active) = active {
            owner.active = active;
        }
    });
    true
}
pub fn simulate_diplomacy_toggle_show(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    record_residual(effects, ResidualDiplomacyAction::ToggleShow, Some(true))
}
pub fn simulate_diplomacy_toggle_hide(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    record_residual(effects, ResidualDiplomacyAction::ToggleHide, Some(false))
}
pub fn simulate_diplomacy_hide(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    record_residual(effects, ResidualDiplomacyAction::Hide, Some(false))
}
pub fn simulate_diplomacy_reset(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_diplomacy(|owner| {
        owner.active = false;
        owner.players.clear();
        owner.briefing_list.clear();
        owner.mute_slot = None;
        owner.last_action = ResidualDiplomacyAction::Reset;
    });
    true
}
pub fn simulate_diplomacy_radio_ingame(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    record_residual(effects, ResidualDiplomacyAction::RadioInGame, None)
}
pub fn simulate_diplomacy_radio_buddies(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    record_residual(effects, ResidualDiplomacyAction::RadioBuddies, None)
}
pub fn simulate_diplomacy_mute_slot(
    slot: i32,
    manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if slot < 0 {
        return false;
    }
    effects.with_diplomacy(|owner| {
        let _ = owner.set_player_muted(slot, true, manager);
        owner.mute_slot = Some(slot);
        owner.last_action = ResidualDiplomacyAction::Mute;
    });
    true
}
pub fn simulate_diplomacy_unmute_slot(
    slot: i32,
    manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if slot < 0 {
        return false;
    }
    effects.with_diplomacy(|owner| {
        let _ = owner.set_player_muted(slot, false, manager);
        owner.mute_slot = Some(slot);
        owner.last_action = ResidualDiplomacyAction::Unmute;
    });
    true
}
pub fn simulate_diplomacy_prepare_ingame(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_diplomacy_bind_controls()
        && simulate_diplomacy_toggle_show(effects)
        && simulate_diplomacy_radio_ingame(effects)
}

/// Human click-through: OS LeftDown/Up on `Diplomacy.wnd:RadioButtonInGame`.
pub fn drive_os_wnd_diplomacy_radio_ingame_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:RadioButtonInGame", effects);
    if !clicked {
        return false;
    }
    simulate_diplomacy_radio_ingame(effects)
}

pub fn drive_os_wnd_diplomacy_radio_buddies_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:RadioButtonBuddies", effects);
    if !clicked {
        return false;
    }
    simulate_diplomacy_radio_buddies(effects)
}

pub fn drive_os_wnd_diplomacy_hide_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:ButtonHide", effects);
    if !clicked {
        return false;
    }
    simulate_diplomacy_hide(effects)
}

pub fn drive_os_wnd_diplomacy_mute_like_cpp(
    slot: i32,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if slot < 0 {
        return false;
    }
    let name = format!("Diplomacy.wnd:ButtonMute{slot}");
    let clicked = crate::gui::dispatch_os_click_named_window(&name, effects);
    if !clicked {
        return false;
    }
    effects.with_manager(|manager, effects| simulate_diplomacy_mute_slot(slot, manager, effects))
}

pub fn drive_os_wnd_diplomacy_unmute_like_cpp(
    slot: i32,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if slot < 0 {
        return false;
    }
    let name = format!("Diplomacy.wnd:ButtonUnMute{slot}");
    let clicked = crate::gui::dispatch_os_click_named_window(&name, effects);
    if !clicked {
        return false;
    }
    effects.with_manager(|manager, effects| simulate_diplomacy_unmute_slot(slot, manager, effects))
}

pub fn drive_os_wnd_diplomacy_prepare_ingame_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked_parent =
        crate::gui::dispatch_os_click_named_window("Diplomacy.wnd:InGameParent", effects);
    if clicked_parent {
        let _ = simulate_diplomacy_toggle_show(effects);
    }
    let clicked_radio = drive_os_wnd_diplomacy_radio_ingame_like_cpp(effects);
    if !clicked_parent && !clicked_radio {
        return false;
    }
    if clicked_radio {
        return effects.with_diplomacy(|owner| residual_diplomacy_last_action(owner))
            == ResidualDiplomacyAction::RadioInGame;
    }
    simulate_diplomacy_radio_ingame(effects)
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
            })
        });
    }
    #[test]
    fn os_wnd_diplomacy_ingame_and_hide_hit_named_gadgets() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "Diplomacy.wnd:InGameParent", 10, 10);
        install_named_button(&mut client, "Diplomacy.wnd:RadioButtonInGame", 10, 40);
        install_named_button(&mut client, "Diplomacy.wnd:ButtonHide", 10, 70);
        assert!(
            client.with_window_event_effects(None, |effects| {
                drive_os_wnd_diplomacy_prepare_ingame_like_cpp(effects)
            }),
            "OS WND clicks must latch InGame radio residual"
        );
        assert_eq!(
            residual_diplomacy_last_action(client.diplomacy()),
            ResidualDiplomacyAction::RadioInGame
        );
        assert!(client.with_window_event_effects(None, |effects| {
            drive_os_wnd_diplomacy_hide_like_cpp(effects)
        }));
        assert_eq!(
            residual_diplomacy_last_action(client.diplomacy()),
            ResidualDiplomacyAction::Hide
        );
    }

    #[test]
    fn os_wnd_diplomacy_mute_hits_button_mute_slot() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "Diplomacy.wnd:ButtonMute0", 10, 100);
        assert!(client.with_window_event_effects(None, |effects| {
            drive_os_wnd_diplomacy_mute_like_cpp(0, effects)
        }));
        assert_eq!(
            residual_diplomacy_last_action(client.diplomacy()),
            ResidualDiplomacyAction::Mute
        );
        assert_eq!(residual_diplomacy_mute_slot(client.diplomacy()), Some(0));
        assert!(!client.with_window_event_effects(None, |effects| {
            drive_os_wnd_diplomacy_unmute_like_cpp(0, effects)
        }));
    }
}
