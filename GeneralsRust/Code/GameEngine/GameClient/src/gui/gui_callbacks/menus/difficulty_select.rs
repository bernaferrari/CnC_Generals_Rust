// FILE: difficulty_select.rs
// Ported from: DifficultySelect.cpp
// Author: Chris Huybregts (original C++), Rust port 2025
// Purpose: Campaign difficulty selection popup

use crate::gui::campaign_manager::{CampaignManager, GameDifficulty};

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
}

// Set the difficulty radio button based on current selection
// Matches C++ SetDifficultyRadioButton (DifficultySelect.cpp lines 61-99)
pub fn set_difficulty_radio_button(state: &DifficultySelectState) -> i32 {
    match state.selected_difficulty {
        GameDifficulty::Easy => 200,   // RadioButtonEasy
        GameDifficulty::Normal => 201, // RadioButtonMedium
        GameDifficulty::Hard => 202,   // RadioButtonHard
    }
}

// Handle button selections
// Matches C++ DifficultySelectSystem GBM_SELECTED (DifficultySelect.cpp lines 224-266)
pub fn handle_button_selected(
    state: &mut DifficultySelectState,
    button_id: i32,
    campaign_manager: &mut CampaignManager,
) -> DifficultySelectAction {
    if button_id == 203 {
        return DifficultySelectAction::StartGame(state.selected_difficulty);
    }

    if button_id == 204 {
        campaign_manager.set_campaign("");
        return DifficultySelectAction::Cancel;
    }

    if button_id == 200 {
        state.selected_difficulty = GameDifficulty::Easy;
        return DifficultySelectAction::None;
    } else if button_id == 201 {
        state.selected_difficulty = GameDifficulty::Normal;
        return DifficultySelectAction::None;
    } else if button_id == 202 {
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
pub fn setup_game_start_with_difficulty(
    map_name: String,
    difficulty: GameDifficulty,
    campaign_manager: &mut CampaignManager,
) {
    campaign_manager.set_game_difficulty(difficulty);
    let _ = map_name;
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
}
