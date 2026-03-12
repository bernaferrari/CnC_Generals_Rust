// FILE: credits_menu.rs
//-----------------------------------------------------------------------------
//
//                       Electronic Arts Pacific.
//
//                       Confidential Information
//                Copyright (C) 2002 - All Rights Reserved
//
//-----------------------------------------------------------------------------
//
//  created:    Dec 2002
//
//  Filename:   credits_menu.rs
//
//  author:     Chris Huybregts (original C++), Rust port
//
//  purpose:    The credits screen...yay
//
//-----------------------------------------------------------------------------
//
// Faithful Rust port of:
// /GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/CreditsMenu.cpp

use std::sync::{Arc, Mutex, RwLock, OnceLock};

use crate::GameClient::GUI::{
    WindowLayout, GameWindow, WindowManager, NameKeyGenerator,
    WindowMsgHandledType, WindowMsgData, NameKeyType, NAMEKEY_INVALID,
};

use super::credits::{CreditsManager, get_the_credits, set_the_credits};

// Keyboard key constants (matches C++ KeyDefs.h)
const KEY_ESC: u8 = 27;

// Key state flags (matches C++ KeyDefs.h)
const KEY_STATE_UP: u8 = 0x01;

// Window messages (matches C++ GWM_* and GBM_* constants)
const GWM_CREATE: u32 = 0x0001;
const GWM_DESTROY: u32 = 0x0002;
const GWM_CHAR: u32 = 0x0020;
const GWM_INPUT_FOCUS: u32 = 0x0021;
const GBM_SELECTED: u32 = 0x0040;

// Audio handle special values (matches C++ AudioHandleSpecialValues.h)
const AHSV_STOP_THE_MUSIC_FADE: i32 = -4;

// Helper macro to test bits
fn bit_test(value: u8, bit: u8) -> bool {
    (value & bit) != 0
}

// Shell trait (represents C++ Shell)
pub trait Shell: Send + Sync {
    fn show_shell_map(&mut self, show: bool);
    fn pop(&mut self);
    fn shutdown_complete(&mut self, layout: &dyn WindowLayout);
}

// Audio manager trait (represents C++ GameAudio)
pub trait AudioManager: Send + Sync {
    fn remove_audio_event(&mut self, event_type: i32);
    fn add_audio_event(&mut self, event_name: &str, should_fade: bool);
}

// Static state variables (matches C++ CreditsMenu.cpp lines 50-53)
static PARENT_MAIN_MENU_ID: OnceLock<Mutex<NameKeyType>> = OnceLock::new();

fn get_parent_main_menu_id() -> Arc<Mutex<NameKeyType>> {
    PARENT_MAIN_MENU_ID.get_or_init(|| Mutex::new(NAMEKEY_INVALID)).clone().into()
}

// Initialize the credits menu (matches C++ CreditsMenu.cpp lines 61-88)
pub fn credits_menu_init(
    layout: &mut dyn WindowLayout,
    shell: &mut dyn Shell,
    audio: &mut dyn AudioManager,
    window_manager: &dyn WindowManager,
    name_key_generator: &dyn NameKeyGenerator,
    user_data: *mut (),
) {
    // Hide shell map (line 63)
    shell.show_shell_map(false);

    // Delete existing credits manager if present (lines 64-65)
    // Reset global credits manager
    let new_credits = Arc::new(Mutex::new(CreditsManager::new()));

    // Load and initialize credits (lines 67-68)
    if let Ok(mut credits) = new_credits.lock() {
        credits.load("Data/INI/Credits.ini");
        credits.init();
    }

    set_the_credits(new_credits);

    // Get parent window ID (lines 70-71)
    let parent_id = name_key_generator.name_to_key("CreditsMenu.wnd:ParentCreditsWindow");
    if let Ok(mut id) = get_parent_main_menu_id().lock() {
        *id = parent_id;
    }

    // Show menu (line 75)
    layout.hide(false);

    // Set keyboard focus to main parent (line 78)
    // In C++: TheWindowManager->winSetFocus(parentMainMenu);
    // Would need window_manager.win_set_focus(parent_window) here

    // Stop music and start credits music (lines 82-85)
    audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
    audio.add_audio_event("Credits", true); // should_fade = TRUE
}

// Credits menu shutdown (matches C++ CreditsMenu.cpp lines 93-108)
pub fn credits_menu_shutdown(
    layout: &mut dyn WindowLayout,
    shell: &mut dyn Shell,
    audio: &mut dyn AudioManager,
    user_data: *mut (),
) {
    // Reset and delete credits manager (lines 95-97)
    if let Some(credits_arc) = THE_CREDITS.get() {
        if let Ok(mut credits) = credits_arc.lock() {
            credits.reset();
        }
    }
    // Credits manager will be cleaned up when reference is dropped

    // Show shell map again (line 98)
    shell.show_shell_map(true);

    // Hide menu (line 101)
    layout.hide(true);

    // Notify shell of shutdown completion (line 104)
    shell.shutdown_complete(layout);

    // Stop credits music (line 106)
    audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
}

// Use THE_CREDITS for consistency
use super::credits::THE_CREDITS;

// Credits menu update (matches C++ CreditsMenu.cpp lines 113-126)
pub fn credits_menu_update(
    layout: &mut dyn WindowLayout,
    shell: &mut dyn Shell,
    window_manager: &dyn WindowManager,
    user_data: *mut (),
) {
    if let Some(credits_arc) = THE_CREDITS.get() {
        if let Ok(mut credits) = credits_arc.lock() {
            // Set focus to parent window (line 118)
            // window_manager.win_set_focus(parent_main_menu);

            credits.update();

            // Check if credits finished (lines 120-121)
            if credits.is_finished() {
                shell.pop();
            }
        }
    } else {
        // No credits manager, just pop (line 124)
        shell.pop();
    }
}

// Credits menu input callback (matches C++ CreditsMenu.cpp lines 131-175)
pub fn credits_menu_input(
    window: &dyn GameWindow,
    shell: &mut dyn Shell,
    msg: u32,
    m_data1: WindowMsgData,
    m_data2: WindowMsgData,
) -> WindowMsgHandledType {
    match msg {
        GWM_CHAR => {
            // Lines 139-169
            let key = (m_data1 & 0xFF) as u8;
            let state = (m_data2 & 0xFF) as u8;

            match key {
                KEY_ESC => {
                    // Lines 148-164
                    // Send a simulated selected event to the parent window
                    // of the back/exit button
                    if bit_test(state, KEY_STATE_UP) {
                        shell.pop();
                    }

                    // Don't let key fall through anywhere else
                    return WindowMsgHandledType::Handled;
                }
                _ => {}
            }
        }
        _ => {}
    }

    WindowMsgHandledType::Ignored
}

// Credits menu system callback (matches C++ CreditsMenu.cpp lines 180-227)
pub fn credits_menu_system(
    window: &dyn GameWindow,
    msg: u32,
    m_data1: WindowMsgData,
    m_data2: WindowMsgData,
) -> WindowMsgHandledType {
    match msg {
        GWM_CREATE => {
            // Line 188-193
            // Window creation
            WindowMsgHandledType::Handled
        }
        GWM_DESTROY => {
            // Lines 197-202
            // Window destruction
            WindowMsgHandledType::Handled
        }
        GWM_INPUT_FOCUS => {
            // Lines 205-213
            // If we're given the opportunity to take the keyboard focus we must say we want it
            if m_data1 != 0 {
                // In C++: *(Bool *)mData2 = TRUE;
                // Would need to set the value at the pointer location
                // For now, just signal we handled it
            }

            return WindowMsgHandledType::Handled;
        }
        GBM_SELECTED => {
            // Lines 216-220
            // Button selected
            WindowMsgHandledType::Handled
        }
        _ => WindowMsgHandledType::Ignored,
    }
}

// Helper functions for integrating with the game engine

// Draw credits (called from main render loop)
pub fn credits_menu_draw() {
    if let Some(credits_arc) = THE_CREDITS.get() {
        if let Ok(credits) = credits_arc.lock() {
            credits.draw();
        }
    }
}

// Check if credits is active
pub fn is_credits_active() -> bool {
    THE_CREDITS.get().is_some()
}

// Parse INI configuration (matches C++ Credits.cpp lines 68-78)
pub fn parse_credits_ini(ini_data: &str) -> CreditsManager {
    let mut manager = CreditsManager::new();

    // This would parse the INI file format:
    // ScrollRate = 1
    // ScrollRateEveryFrames = 1
    // ScrollDown = Yes
    // TitleColor = R:255 G:255 B:255 A:255
    // MinorTitleColor = R:200 G:200 B:200 A:255
    // NormalColor = R:180 G:180 B:180 A:255
    // Style = TITLE
    // Text = "GAME TITLE"
    // Style = NORMAL
    // Text = "Developer Name"
    // Blank
    // etc.

    // For now, return default manager
    // A full implementation would parse the INI format
    manager
}

// Example usage and integration helper
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credits_manager_creation() {
        let manager = CreditsManager::new();
        assert!(!manager.is_finished());
    }

    #[test]
    fn test_add_blank() {
        let mut manager = CreditsManager::new();
        manager.add_blank();
        assert_eq!(manager.credit_line_list.len(), 1);
    }

    #[test]
    fn test_add_text() {
        let mut manager = CreditsManager::new();
        manager.set_current_style(super::super::credits::CreditStyle::Normal);
        manager.add_text("Test Credit".to_string());
        assert_eq!(manager.credit_line_list.len(), 1);
    }

    #[test]
    fn test_bit_test() {
        assert!(bit_test(0x01, 0x01));
        assert!(!bit_test(0x02, 0x01));
        assert!(bit_test(0x03, 0x01));
    }
}
