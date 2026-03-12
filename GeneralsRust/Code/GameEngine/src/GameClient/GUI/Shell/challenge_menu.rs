// FILE: challenge_menu.rs
// Ported from: ChallengeMenu.cpp
// Author: Steve Copeland (original C++), Rust port 2025
// Purpose: General's Challenge Mode Menu UI logic

use super::challenge_generals::{ChallengeGenerals, GeneralPersona, GameDifficulty, NUM_GENERALS};
use super::campaign_manager::CampaignManager;
use std::time::{Duration, Instant};

// Constants from C++ (ChallengeMenu.cpp lines 43-44)
const DEFAULT_GENERAL: usize = 0;
const TELETYPE_SKIP: usize = 2;

// Window state tracking
pub struct ChallengeMenuState {
    // Window visibility
    is_shutting_down: bool,
    just_entered: bool,
    initial_gadget_delay: i32,
    button_pushed: bool,

    // General selection
    last_button_index: i32,
    last_hilited_index: i32,
    is_auto_selecting: bool,

    // Button sequence animation
    button_sequence_step: usize,

    // Bio display state (teletype effect)
    bio_line_1: String,
    bio_line_2: String,
    bio_line_3: String,
    bio_line_4: String,
    bio_line_1_readout: String,
    bio_line_2_readout: String,
    bio_line_3_readout: String,
    bio_line_4_readout: String,
    bio_text_position: usize,
    bio_total_length: usize,

    // Audio state
    intro_audio_magic_number: i32,
    has_played_intro_audio: bool,
}

impl ChallengeMenuState {
    pub fn new() -> Self {
        Self {
            is_shutting_down: false,
            just_entered: false,
            initial_gadget_delay: 2,
            button_pushed: false,
            last_button_index: -1,
            last_hilited_index: -1,
            is_auto_selecting: false,
            button_sequence_step: 0,
            bio_line_1: String::new(),
            bio_line_2: String::new(),
            bio_line_3: String::new(),
            bio_line_4: String::new(),
            bio_line_1_readout: String::new(),
            bio_line_2_readout: String::new(),
            bio_line_3_readout: String::new(),
            bio_line_4_readout: String::new(),
            bio_text_position: 0,
            bio_total_length: 0,
            intro_audio_magic_number: 0,
            has_played_intro_audio: false,
        }
    }

    // Find which general button was clicked
    // Matches C++ findPositionButton (ChallengeMenu.cpp lines 110-118)
    pub fn find_position_button(&self, button_id: i32) -> Option<usize> {
        if button_id >= 0 && (button_id as usize) < NUM_GENERALS {
            Some(button_id as usize)
        } else {
            None
        }
    }

    // Set the bio text for a general (for teletype display)
    // Matches C++ setGeneralBio (ChallengeMenu.cpp lines 178-205)
    pub fn set_general_bio(&mut self, button_index: usize, generals: &[GeneralPersona; NUM_GENERALS]) {
        if button_index >= NUM_GENERALS {
            return;
        }

        let general = &generals[button_index];

        // Reset teletype position
        self.bio_text_position = 0;

        // Set bio lines (in real implementation these would be fetched from TheGameText)
        self.bio_line_1 = general.bio_name().to_string();
        self.bio_line_2 = general.bio_rank().to_string();
        self.bio_line_3 = general.bio_branch().to_string();
        self.bio_line_4 = general.bio_strategy().to_string();

        self.bio_total_length = self.bio_line_1.len() + self.bio_line_2.len()
            + self.bio_line_3.len() + self.bio_line_4.len();

        // Clear readout (updateBio will fill it)
        self.bio_line_1_readout.clear();
        self.bio_line_2_readout.clear();
        self.bio_line_3_readout.clear();
        self.bio_line_4_readout.clear();
    }

    // Update the bio display with teletype effect
    // Matches C++ updateBio (ChallengeMenu.cpp lines 257-302)
    pub fn update_bio(&mut self, frames: usize) -> bool {
        let mut updated = false;

        for _ in 0..frames {
            if self.bio_text_position < self.bio_total_length {
                let line1_len = self.bio_line_1.len();
                let line2_len = self.bio_line_2.len();
                let line3_len = self.bio_line_3.len();

                // Determine which line we're currently typing
                if self.bio_text_position < line1_len {
                    // Line 1
                    if let Some(ch) = self.bio_line_1.chars().nth(self.bio_text_position) {
                        self.bio_line_1_readout.push(ch);
                    }
                } else if self.bio_text_position < line1_len + line2_len {
                    // Line 2
                    let pos = self.bio_text_position - line1_len;
                    if let Some(ch) = self.bio_line_2.chars().nth(pos) {
                        self.bio_line_2_readout.push(ch);
                    }
                } else if self.bio_text_position < line1_len + line2_len + line3_len {
                    // Line 3
                    let pos = self.bio_text_position - line1_len - line2_len;
                    if let Some(ch) = self.bio_line_3.chars().nth(pos) {
                        self.bio_line_3_readout.push(ch);
                    }
                } else {
                    // Line 4
                    let pos = self.bio_text_position - line1_len - line2_len - line3_len;
                    if let Some(ch) = self.bio_line_4.chars().nth(pos) {
                        self.bio_line_4_readout.push(ch);
                    }
                }

                self.bio_text_position += 1;
                updated = true;
            }
        }

        updated
    }

    // Update button intro animation sequence
    // Matches C++ updateButtonSequence (ChallengeMenu.cpp lines 210-250)
    pub fn update_button_sequence(&mut self, steps_per_update: usize, generals: &[GeneralPersona; NUM_GENERALS]) {
        const CLEANUP_STATES: usize = 2;

        if self.button_sequence_step > NUM_GENERALS + CLEANUP_STATES {
            return;
        }

        for _ in 0..steps_per_update {
            let pos = self.button_sequence_step;

            // In real implementation, this would update button images
            // based on medallion states (selected, hilite, normal)

            self.button_sequence_step += 1;
        }
    }
}

// Campaign Menu logic handlers

// Initialize the challenge mode menu
// Matches C++ ChallengeMenuInit (ChallengeMenu.cpp lines 308-382)
pub fn challenge_menu_init(state: &mut ChallengeMenuState) {
    // Reset state
    state.is_auto_selecting = false;
    state.button_sequence_step = 0;
    state.just_entered = true;
    state.initial_gadget_delay = 2;
    state.is_shutting_down = false;
    state.intro_audio_magic_number = 0;
    state.has_played_intro_audio = false;
    state.last_button_index = -1;

    // In real implementation would:
    // - Initialize TheChallengeGameInfo
    // - Set up window IDs and pointers
    // - Set enabled buttons based on general availability
    // - Hide bio parent initially
    // - Set keyboard focus
    // - Initialize video manager
}

// Update the challenge mode menu each frame
// Matches C++ ChallengeMenuUpdate (ChallengeMenu.cpp lines 388-430)
pub fn challenge_menu_update(state: &mut ChallengeMenuState, generals: &[GeneralPersona; NUM_GENERALS]) {
    // Handle initial transition delay
    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            // In real implementation: TheTransitionHandler->setGroup("ChallengeMenuFade")
            state.initial_gadget_delay = 2;
            state.just_entered = false;
        } else {
            state.initial_gadget_delay -= 1;
        }
    }

    // Delay intro audio
    if !state.has_played_intro_audio {
        // In C++ checks TheTransitionHandler->isFinished()
        state.intro_audio_magic_number += 1;
        if state.intro_audio_magic_number == 10 {
            // In real implementation: play "Choose your general" audio
            state.has_played_intro_audio = true;
        }
    }

    // Update bio teletype effect
    state.update_bio(TELETYPE_SKIP);

    // Check if shutdown animation complete
    if state.is_shutting_down {
        // In real implementation would check TheShell->isAnimFinished() && TheTransitionHandler->isFinished()
        // then call TheShell->shutdownComplete(layout)
    }

    // Update video manager (in real implementation)
}

// Shutdown the challenge mode menu
// Matches C++ ChallengeMenuShutdown (ChallengeMenu.cpp lines 436-466)
pub fn challenge_menu_shutdown(state: &mut ChallengeMenuState, immediate: bool) {
    state.last_button_index = -1;
    state.button_sequence_step = 0;

    if immediate {
        // Immediate shutdown - skip animations
        // In real implementation: layout->hide(TRUE), TheShell->shutdownComplete(layout)
        return;
    }

    // Animated shutdown
    // In real implementation: TheTransitionHandler->reverse("ChallengeMenuFade")
    state.is_shutting_down = true;

    // Cleanup audio
    // In real implementation: TheAudio->removeAudioEvent(lastSelectionSound/lastPreviewSound)
    state.intro_audio_magic_number = 0;
}

// Handle button selection
// Matches C++ ChallengeMenuSystem GBM_SELECTED case (ChallengeMenu.cpp lines 578-679)
pub fn handle_button_selected(
    state: &mut ChallengeMenuState,
    button_id: i32,
    generals: &[GeneralPersona; NUM_GENERALS],
    campaign_manager: &mut CampaignManager,
) -> ChallengeMenuAction {
    // Skip if auto-selecting (radio button behavior)
    if state.is_auto_selecting {
        state.is_auto_selecting = false;
        return ChallengeMenuAction::None;
    }

    // Check if it's a general position button
    if let Some(button_index) = state.find_position_button(button_id) {
        // Deselect previous button (radio button behavior)
        if state.last_button_index != -1 {
            state.is_auto_selecting = true;
            // In real implementation: GadgetCheckBoxToggle(lastControl)
        }

        // Play audio for this general
        // In real implementation: TheAudio->removeAudioEvent(...) then addAudioEvent(PreviewSound)

        state.last_button_index = button_index as i32;

        // Show play button
        // In real implementation: buttonPlay->winHide(FALSE)

        return ChallengeMenuAction::None;
    }

    // Check if it's the Play button
    if button_id == -100 {  // Placeholder ID for Play button
        if state.last_button_index == -1 {
            return ChallengeMenuAction::None;
        }

        let button_index = state.last_button_index as usize;
        let general = &generals[button_index];

        // Set campaign and template
        campaign_manager.set_campaign(general.campaign());

        // In real implementation would:
        // - Set TheChallengeGameInfo slot
        // - Set pending file to current map
        // - Clear game data if in game
        // - Set difficulty
        // - Post MSG_NEW_GAME message

        // Reset selection
        state.last_button_index = -1;
        state.button_sequence_step = 0;

        return ChallengeMenuAction::StartMission;
    }

    // Check if it's the Back button
    if button_id == -101 {  // Placeholder ID for Back button
        return ChallengeMenuAction::Back;
    }

    ChallengeMenuAction::None
}

// Handle mouse entering a button
// Matches C++ ChallengeMenuSystem GBM_MOUSE_ENTERING case (ChallengeMenu.cpp lines 537-555)
pub fn handle_mouse_entering(
    state: &mut ChallengeMenuState,
    button_id: i32,
    generals: &[GeneralPersona; NUM_GENERALS],
) {
    if let Some(button_index) = state.find_position_button(button_id) {
        if button_index != state.last_button_index as usize {
            state.set_general_bio(button_index, generals);

            // In real implementation: play hover sound
            // AudioEventRTS event("GUILogoMouseOver");
            // TheAudio->addAudioEvent(&event);

            state.last_hilited_index = button_index as i32;
        }
    }
}

// Handle mouse leaving a button
// Matches C++ ChallengeMenuSystem GBM_MOUSE_LEAVING case (ChallengeMenu.cpp lines 558-576)
pub fn handle_mouse_leaving(
    state: &mut ChallengeMenuState,
    button_id: i32,
    generals: &[GeneralPersona; NUM_GENERALS],
) {
    if let Some(button_index) = state.find_position_button(button_id) {
        if button_index != state.last_button_index as usize {
            // Restore bio to selected general (if any)
            if state.last_button_index >= 0 {
                state.set_general_bio(state.last_button_index as usize, generals);
            }
        }
    }
}

// Action results from menu interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeMenuAction {
    None,
    StartMission,
    Back,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::challenge_generals::GeneralPersona;

    fn create_test_generals() -> [GeneralPersona; NUM_GENERALS] {
        let mut generals = [
            GeneralPersona::new(), GeneralPersona::new(), GeneralPersona::new(),
            GeneralPersona::new(), GeneralPersona::new(), GeneralPersona::new(),
            GeneralPersona::new(), GeneralPersona::new(), GeneralPersona::new(),
            GeneralPersona::new(), GeneralPersona::new(), GeneralPersona::new(),
        ];

        generals[0].set_bio_name("General Townes".to_string());
        generals[0].set_bio_rank("General".to_string());
        generals[0].set_bio_branch("USAF".to_string());
        generals[0].set_bio_strategy("Laser Technology".to_string());
        generals[0].set_campaign("USA01".to_string());

        generals
    }

    #[test]
    fn test_menu_state_creation() {
        let state = ChallengeMenuState::new();
        assert_eq!(state.last_button_index, -1);
        assert!(!state.is_shutting_down);
        assert_eq!(state.bio_text_position, 0);
    }

    #[test]
    fn test_find_position_button() {
        let state = ChallengeMenuState::new();

        assert_eq!(state.find_position_button(0), Some(0));
        assert_eq!(state.find_position_button(11), Some(11));
        assert_eq!(state.find_position_button(12), None);
        assert_eq!(state.find_position_button(-1), None);
    }

    #[test]
    fn test_set_general_bio() {
        let mut state = ChallengeMenuState::new();
        let generals = create_test_generals();

        state.set_general_bio(0, &generals);

        assert_eq!(state.bio_line_1, "General Townes");
        assert_eq!(state.bio_line_2, "General");
        assert_eq!(state.bio_line_3, "USAF");
        assert_eq!(state.bio_line_4, "Laser Technology");
        assert_eq!(state.bio_text_position, 0);
        assert!(state.bio_total_length > 0);
    }

    #[test]
    fn test_update_bio_teletype() {
        let mut state = ChallengeMenuState::new();
        let generals = create_test_generals();

        state.set_general_bio(0, &generals);

        // Update a few characters
        let updated = state.update_bio(5);
        assert!(updated);
        assert!(state.bio_text_position > 0);
        assert!(!state.bio_line_1_readout.is_empty());

        // Continue until complete
        while state.bio_text_position < state.bio_total_length {
            state.update_bio(10);
        }

        // Should be fully displayed
        assert_eq!(state.bio_line_1_readout, "General Townes");
        assert_eq!(state.bio_line_2_readout, "General");
    }

    #[test]
    fn test_handle_mouse_events() {
        let mut state = ChallengeMenuState::new();
        let generals = create_test_generals();

        // Mouse enter should set bio
        handle_mouse_entering(&mut state, 0, &generals);
        assert_eq!(state.last_hilited_index, 0);
        assert!(!state.bio_line_1.is_empty());

        // Mouse leave should restore
        state.last_button_index = 1;
        handle_mouse_leaving(&mut state, 0, &generals);
        // Bio should be set to button 1 now (if it were implemented)
    }

    #[test]
    fn test_button_selection_flow() {
        let mut state = ChallengeMenuState::new();
        let generals = create_test_generals();
        let mut campaign_manager = CampaignManager::new();

        // Select a general
        let action = handle_button_selected(&mut state, 0, &generals, &mut campaign_manager);
        assert_eq!(action, ChallengeMenuAction::None);
        assert_eq!(state.last_button_index, 0);

        // Click play button
        let action = handle_button_selected(&mut state, -100, &generals, &mut campaign_manager);
        assert_eq!(action, ChallengeMenuAction::StartMission);
        assert_eq!(state.last_button_index, -1);

        // Click back button
        let action = handle_button_selected(&mut state, -101, &generals, &mut campaign_manager);
        assert_eq!(action, ChallengeMenuAction::Back);
    }
}
