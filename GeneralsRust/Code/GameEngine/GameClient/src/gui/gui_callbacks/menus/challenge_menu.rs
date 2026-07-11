// FILE: challenge_menu.rs
// Ported from: ChallengeMenu.cpp
// Author: Steve Copeland (original C++), Rust port 2025
// Purpose: General's Challenge Mode Menu UI logic

use crate::gui::campaign_manager::CampaignManager;
use crate::gui::challenge_generals::{
    ChallengeGenerals, GameDifficulty, GeneralPersona, NUM_GENERALS,
};
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

impl Default for ChallengeMenuState {
    fn default() -> Self {
        Self::new()
    }
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
    pub fn set_general_bio(
        &mut self,
        button_index: usize,
        generals: &[GeneralPersona; NUM_GENERALS],
    ) {
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

        self.bio_total_length = self.bio_line_1.len()
            + self.bio_line_2.len()
            + self.bio_line_3.len()
            + self.bio_line_4.len();

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
    pub fn update_button_sequence(
        &mut self,
        steps_per_update: usize,
        generals: &[GeneralPersona; NUM_GENERALS],
    ) {
        const CLEANUP_STATES: usize = 2;

        if self.button_sequence_step > NUM_GENERALS + CLEANUP_STATES {
            return;
        }

        for _ in 0..steps_per_update {
            let _pos = self.button_sequence_step;
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
// Matches C++ ChallengeMenuUpdate (ChallengeMenu.cpp lines 384-447)
pub fn challenge_menu_update(state: &mut ChallengeMenuState, generals: &ChallengeGenerals) {
    if state.just_entered {
        state.just_entered = false;
        return;
    }

    if state.initial_gadget_delay > 0 {
        state.initial_gadget_delay -= 1;
        return;
    }

    state.update_button_sequence(1, generals.challenge_generals());
    state.update_bio(TELETYPE_SKIP);
}

// Handle button selection in challenge menu
// Matches C++ ChallengeMenuSystem GBM_SELECTED (ChallengeMenu.cpp lines 499-565)
pub fn challenge_menu_button_selected(
    state: &mut ChallengeMenuState,
    button_id: i32,
    generals: &ChallengeGenerals,
    campaign_manager: &mut CampaignManager,
) -> ChallengeMenuAction {
    if let Some(index) = state.find_position_button(button_id) {
        state.last_button_index = index as i32;
        state.set_general_bio(index, generals.challenge_generals());
        return ChallengeMenuAction::SelectGeneral(index);
    }

    match button_id {
        1000 => {
            state.is_shutting_down = true;
            ChallengeMenuAction::Back
        }
        1001 => {
            if state.last_button_index >= 0 {
                let general = &generals.challenge_generals()[state.last_button_index as usize];
                campaign_manager.set_campaign(general.campaign());
                ChallengeMenuAction::StartChallenge
            } else {
                ChallengeMenuAction::None
            }
        }
        _ => ChallengeMenuAction::None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeMenuAction {
    None,
    SelectGeneral(usize),
    StartChallenge,
    Back,
}
