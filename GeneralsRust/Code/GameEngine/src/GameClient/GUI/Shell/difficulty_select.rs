// FILE: difficulty_select.rs
// Ported from: DifficultySelect.cpp
// Author: Chris Huybregts (original C++), Rust port 2025
// Purpose: Campaign difficulty selection popup

use super::campaign_manager::{CampaignManager, GameDifficulty};

// Difficulty selection state
pub struct DifficultySelectState {
    selected_difficulty: GameDifficulty,
}

impl DifficultySelectState {
    pub fn new() -> Self {
        Self {
            selected_difficulty: GameDifficulty::Normal,
        }
    }

    pub fn get_difficulty(&self) -> GameDifficulty {
        self.selected_difficulty
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        self.selected_difficulty = difficulty;
    }
}

// Initialize difficulty select popup
// Matches C++ DifficultySelectInit (DifficultySelect.cpp lines 102-131)
pub fn difficulty_select_init(state: &mut DifficultySelectState) {
    // Load difficulty from user preferences
    // In C++ this reads from OptionPreferences::getCampaignDifficulty()
    state.selected_difficulty = GameDifficulty::Normal;

    // In real implementation would:
    // - Get window IDs for buttons (OK, Cancel, radio buttons)
    // - Set radio button selection based on loaded difficulty
    // - Set window as modal: TheWindowManager->winSetModal(parent)
    // - Bring window to top: parent->winBringToTop()
}

// Set the difficulty radio button based on current selection
// Matches C++ SetDifficultyRadioButton (DifficultySelect.cpp lines 61-99)
pub fn set_difficulty_radio_button(state: &DifficultySelectState) -> i32 {
    // Returns the button ID that should be selected
    match state.selected_difficulty {
        GameDifficulty::Easy => 200,    // RadioButtonEasy
        GameDifficulty::Normal => 201,  // RadioButtonMedium
        GameDifficulty::Hard => 202,    // RadioButtonHard
    }
}

// Handle button selections
// Matches C++ DifficultySelectSystem GBM_SELECTED case (DifficultySelect.cpp lines 224-266)
pub fn handle_button_selected(
    state: &mut DifficultySelectState,
    button_id: i32,
    campaign_manager: &mut CampaignManager,
) -> DifficultySelectAction {
    // OK button - confirm and start game
    if button_id == 203 {  // ButtonOk
        // Save difficulty preference
        // In C++ this calls: pref.setCampaignDifficulty(s_AIDiff); pref.write();

        // In real implementation would:
        // - Destroy layout window
        // - Call setupGameStart(TheCampaignManager->getCurrentMap(), difficulty)

        return DifficultySelectAction::StartGame(state.selected_difficulty);
    }

    // Cancel button - abort campaign start
    if button_id == 204 {  // ButtonCancel
        // Reset campaign
        campaign_manager.set_campaign("");

        // In real implementation would:
        // - Unset modal: TheWindowManager->winUnsetModal(window)
        // - Destroy layout window

        return DifficultySelectAction::Cancel;
    }

    // Difficulty radio buttons
    if button_id == 200 {  // RadioButtonEasy
        state.selected_difficulty = GameDifficulty::Easy;
        return DifficultySelectAction::None;
    } else if button_id == 201 {  // RadioButtonMedium
        state.selected_difficulty = GameDifficulty::Normal;
        return DifficultySelectAction::None;
    } else if button_id == 202 {  // RadioButtonHard
        state.selected_difficulty = GameDifficulty::Hard;
        return DifficultySelectAction::None;
    }

    DifficultySelectAction::None
}

// Action results from difficulty selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifficultySelectAction {
    None,
    StartGame(GameDifficulty),
    Cancel,
}

// Setup game start with selected difficulty
// Referenced in C++ DifficultySelect.cpp line 239: setupGameStart(mapName, difficulty)
// This function signature appears in MapSelectMenu.cpp
pub fn setup_game_start_with_difficulty(
    map_name: String,
    difficulty: GameDifficulty,
    campaign_manager: &mut CampaignManager,
) {
    // Set the difficulty in campaign manager
    campaign_manager.set_game_difficulty(difficulty);

    // In real implementation would:
    // - Set TheWritableGlobalData->m_pendingFile = mapName
    // - Clear game data if in-game: TheGameLogic->clearGameData()
    // - Post MSG_NEW_GAME with GAME_SINGLE_PLAYER mode
    // - Pass difficulty and rank points
    // - Initialize random seed: InitRandom(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_state_creation() {
        let state = DifficultySelectState::new();
        assert_eq!(state.get_difficulty(), GameDifficulty::Normal);
    }

    #[test]
    fn test_set_difficulty() {
        let mut state = DifficultySelectState::new();

        state.set_difficulty(GameDifficulty::Easy);
        assert_eq!(state.get_difficulty(), GameDifficulty::Easy);

        state.set_difficulty(GameDifficulty::Hard);
        assert_eq!(state.get_difficulty(), GameDifficulty::Hard);
    }

    #[test]
    fn test_radio_button_selection() {
        let mut state = DifficultySelectState::new();

        state.set_difficulty(GameDifficulty::Easy);
        assert_eq!(set_difficulty_radio_button(&state), 200);

        state.set_difficulty(GameDifficulty::Normal);
        assert_eq!(set_difficulty_radio_button(&state), 201);

        state.set_difficulty(GameDifficulty::Hard);
        assert_eq!(set_difficulty_radio_button(&state), 202);
    }

    #[test]
    fn test_difficulty_button_selection() {
        let mut state = DifficultySelectState::new();
        let mut campaign_manager = CampaignManager::new();

        // Select easy
        let action = handle_button_selected(&mut state, 200, &mut campaign_manager);
        assert_eq!(action, DifficultySelectAction::None);
        assert_eq!(state.get_difficulty(), GameDifficulty::Easy);

        // Select hard
        let action = handle_button_selected(&mut state, 202, &mut campaign_manager);
        assert_eq!(action, DifficultySelectAction::None);
        assert_eq!(state.get_difficulty(), GameDifficulty::Hard);

        // Select normal
        let action = handle_button_selected(&mut state, 201, &mut campaign_manager);
        assert_eq!(action, DifficultySelectAction::None);
        assert_eq!(state.get_difficulty(), GameDifficulty::Normal);
    }

    #[test]
    fn test_ok_button() {
        let mut state = DifficultySelectState::new();
        let mut campaign_manager = CampaignManager::new();

        state.set_difficulty(GameDifficulty::Hard);

        let action = handle_button_selected(&mut state, 203, &mut campaign_manager);

        match action {
            DifficultySelectAction::StartGame(diff) => {
                assert_eq!(diff, GameDifficulty::Hard);
            }
            _ => panic!("Expected StartGame action"),
        }
    }

    #[test]
    fn test_cancel_button() {
        let mut state = DifficultySelectState::new();
        let mut campaign_manager = CampaignManager::new();

        // Set a campaign first
        campaign_manager.set_campaign("TestCampaign");

        let action = handle_button_selected(&mut state, 204, &mut campaign_manager);
        assert_eq!(action, DifficultySelectAction::Cancel);

        // Campaign should be cleared
        assert!(campaign_manager.get_current_campaign().is_none());
    }

    #[test]
    fn test_setup_game_with_difficulty() {
        let mut campaign_manager = CampaignManager::new();

        setup_game_start_with_difficulty(
            "Maps/TestMap.map".to_string(),
            GameDifficulty::Hard,
            &mut campaign_manager,
        );

        assert_eq!(campaign_manager.get_game_difficulty(), GameDifficulty::Hard);
    }
}
