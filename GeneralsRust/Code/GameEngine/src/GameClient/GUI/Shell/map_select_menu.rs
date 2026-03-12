// FILE: map_select_menu.rs
// Ported from: MapSelectMenu.cpp
// Author: Colin Day (original C++), Rust port 2025
// Purpose: Map selection menu for single player campaigns

use super::campaign_manager::{CampaignManager, GameDifficulty};
use std::time::Instant;

// Menu state tracking
pub struct MapSelectMenuState {
    show_solo_maps: bool,
    is_shutting_down: bool,
    start_game: bool,
    button_pushed: bool,
    difficulty: GameDifficulty,
    selected_map: Option<String>,
}

impl MapSelectMenuState {
    pub fn new() -> Self {
        Self {
            show_solo_maps: true,
            is_shutting_down: false,
            start_game: false,
            button_pushed: false,
            difficulty: GameDifficulty::Normal,
            selected_map: None,
        }
    }

    pub fn get_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    pub fn get_selected_map(&self) -> Option<&str> {
        self.selected_map.as_deref()
    }

    pub fn set_selected_map(&mut self, map: Option<String>) {
        self.selected_map = map;
    }

    pub fn is_solo_maps_shown(&self) -> bool {
        self.show_solo_maps
    }

    pub fn set_show_solo_maps(&mut self, show: bool) {
        self.show_solo_maps = show;
    }
}

// Setup game start (prepares transition to gameplay)
// Matches C++ setupGameStart (MapSelectMenu.cpp lines 36-41)
pub fn setup_game_start(state: &mut MapSelectMenuState, map_name: String) {
    state.start_game = true;
    state.selected_map = Some(map_name);
    // In real implementation: TheWritableGlobalData->m_pendingFile = mapName
    // In real implementation: TheShell->reverseAnimatewindow()
}

// Actually start the game (called after animations complete)
// Matches C++ doGameStart (MapSelectMenu.cpp lines 44-66)
pub fn do_game_start(state: &mut MapSelectMenuState, campaign_manager: &mut CampaignManager) {
    state.start_game = false;

    // In real implementation would:
    // - Clear existing game if in-game: TheGameLogic->clearGameData()
    // - Post MSG_NEW_GAME message
    // - Set game mode to GAME_SINGLE_PLAYER
    // - Pass difficulty and rank points
    // - Initialize random seed

    state.is_shutting_down = true;
}

// Initialize map select menu
// Matches C++ MapSelectMenuInit (MapSelectMenu.cpp lines 136-187)
pub fn map_select_menu_init(state: &mut MapSelectMenuState) {
    state.show_solo_maps = true;
    state.button_pushed = false;
    state.is_shutting_down = false;
    state.start_game = false;

    // In real implementation would:
    // - Show shell map: TheShell->showShellMap(TRUE)
    // - Load map list from cache
    // - Populate listbox with maps
    // - Set keyboard focus
    // - Register animations for buttons
    // - Set difficulty radio button based on user preferences
}

// Shutdown map select menu
// Matches C++ MapSelectMenuShutdown (MapSelectMenu.cpp lines 192-210)
pub fn map_select_menu_shutdown(state: &mut MapSelectMenuState, immediate: bool) {
    if !state.start_game {
        state.is_shutting_down = true;
    }

    if immediate {
        // Immediate shutdown - skip animations
        return;
    }

    if !state.start_game {
        // In real implementation: TheShell->reverseAnimatewindow()
    }
}

// Update map select menu each frame
// Matches C++ MapSelectMenuUpdate (MapSelectMenu.cpp lines 215-226)
pub fn map_select_menu_update(state: &mut MapSelectMenuState, campaign_manager: &mut CampaignManager) {
    if state.start_game {
        // In real implementation: check TheShell->isAnimFinished()
        // For now just start immediately
        do_game_start(state, campaign_manager);
    }

    if state.is_shutting_down {
        // In real implementation: check TheShell->isAnimFinished()
        // then call shutdownComplete(layout)
    }
}

// Handle button selections
// Matches C++ MapSelectMenuSystem GBM_SELECTED case (MapSelectMenu.cpp lines 334-420)
pub fn handle_button_selected(
    state: &mut MapSelectMenuState,
    button_id: i32,
    campaign_manager: &mut CampaignManager,
) -> MapSelectMenuAction {
    if state.button_pushed {
        return MapSelectMenuAction::None;
    }

    // Single player / multiplayer toggle
    if button_id == 100 {  // ButtonSinglePlayer
        state.show_solo_maps = true;
        // In real implementation: populateMapListbox with solo maps
        return MapSelectMenuAction::RefreshMapList;
    } else if button_id == 101 {  // ButtonMultiplayer
        state.show_solo_maps = false;
        // In real implementation: populateMapListbox with MP maps
        return MapSelectMenuAction::RefreshMapList;
    }

    // Map directory toggle (system vs user maps)
    if button_id == 102 {  // RadioButtonSystemMaps
        // In real implementation: update map cache and repopulate list
        // Save preference: pref["UseSystemMapDir"] = "yes"
        return MapSelectMenuAction::RefreshMapList;
    } else if button_id == 103 {  // RadioButtonUserMaps
        // In real implementation: update map cache and repopulate list
        // Save preference: pref["UseSystemMapDir"] = "no"
        return MapSelectMenuAction::RefreshMapList;
    }

    // Difficulty selection
    if button_id == 104 {  // RadioButtonEasyAI
        state.difficulty = GameDifficulty::Easy;
        return MapSelectMenuAction::None;
    } else if button_id == 105 {  // RadioButtonMediumAI
        state.difficulty = GameDifficulty::Normal;
        return MapSelectMenuAction::None;
    } else if button_id == 106 {  // RadioButtonHardAI
        state.difficulty = GameDifficulty::Hard;
        return MapSelectMenuAction::None;
    }

    // Back button
    if button_id == 107 {  // ButtonBack
        state.button_pushed = true;
        return MapSelectMenuAction::Back;
    }

    // OK button - start the selected map
    if button_id == 108 {  // ButtonOK
        if let Some(map_name) = state.selected_map.clone() {
            state.button_pushed = true;

            // Reset campaign manager (this is skirmish, not campaign)
            campaign_manager.set_campaign("");

            setup_game_start(state, map_name);
            return MapSelectMenuAction::StartGame;
        }
    }

    MapSelectMenuAction::None
}

// Handle double-click on map list
// Matches C++ MapSelectMenuSystem GLM_DOUBLE_CLICKED case (MapSelectMenu.cpp lines 421-444)
pub fn handle_map_double_click(
    state: &mut MapSelectMenuState,
    row_selected: i32,
    map_name: String,
) -> MapSelectMenuAction {
    if state.button_pushed {
        return MapSelectMenuAction::None;
    }

    if row_selected >= 0 {
        state.selected_map = Some(map_name);
        // Simulate OK button click
        return MapSelectMenuAction::StartGame;
    }

    MapSelectMenuAction::None
}

// Action results from menu interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSelectMenuAction {
    None,
    RefreshMapList,
    StartGame,
    Back,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_state_creation() {
        let state = MapSelectMenuState::new();
        assert!(state.show_solo_maps);
        assert!(!state.is_shutting_down);
        assert!(!state.start_game);
        assert_eq!(state.difficulty, GameDifficulty::Normal);
        assert!(state.selected_map.is_none());
    }

    #[test]
    fn test_difficulty_selection() {
        let mut state = MapSelectMenuState::new();
        let mut campaign_manager = CampaignManager::new();

        // Select easy
        let action = handle_button_selected(&mut state, 104, &mut campaign_manager);
        assert_eq!(action, MapSelectMenuAction::None);
        assert_eq!(state.difficulty, GameDifficulty::Easy);

        // Select hard
        let action = handle_button_selected(&mut state, 106, &mut campaign_manager);
        assert_eq!(action, MapSelectMenuAction::None);
        assert_eq!(state.difficulty, GameDifficulty::Hard);
    }

    #[test]
    fn test_map_selection() {
        let mut state = MapSelectMenuState::new();

        state.set_selected_map(Some("Maps/TestMap.map".to_string()));
        assert_eq!(state.get_selected_map(), Some("Maps/TestMap.map"));

        let mut campaign_manager = CampaignManager::new();

        // Click OK with map selected
        let action = handle_button_selected(&mut state, 108, &mut campaign_manager);
        assert_eq!(action, MapSelectMenuAction::StartGame);
        assert!(state.button_pushed);
        assert!(state.start_game);
    }

    #[test]
    fn test_map_double_click() {
        let mut state = MapSelectMenuState::new();

        let action = handle_map_double_click(
            &mut state,
            0,
            "Maps/DoubleClickMap.map".to_string(),
        );

        assert_eq!(action, MapSelectMenuAction::StartGame);
        assert_eq!(state.get_selected_map(), Some("Maps/DoubleClickMap.map"));
    }

    #[test]
    fn test_solo_multiplayer_toggle() {
        let mut state = MapSelectMenuState::new();
        let mut campaign_manager = CampaignManager::new();

        assert!(state.is_solo_maps_shown());

        // Switch to multiplayer
        let action = handle_button_selected(&mut state, 101, &mut campaign_manager);
        assert_eq!(action, MapSelectMenuAction::RefreshMapList);
        assert!(!state.is_solo_maps_shown());

        // Switch back to solo
        let action = handle_button_selected(&mut state, 100, &mut campaign_manager);
        assert_eq!(action, MapSelectMenuAction::RefreshMapList);
        assert!(state.is_solo_maps_shown());
    }

    #[test]
    fn test_back_button() {
        let mut state = MapSelectMenuState::new();
        let mut campaign_manager = CampaignManager::new();

        let action = handle_button_selected(&mut state, 107, &mut campaign_manager);
        assert_eq!(action, MapSelectMenuAction::Back);
        assert!(state.button_pushed);
    }
}
