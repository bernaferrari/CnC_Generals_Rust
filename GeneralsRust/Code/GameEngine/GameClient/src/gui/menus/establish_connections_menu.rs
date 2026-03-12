//! EstablishConnectionsMenu
//!
//! This module handles the connection establishment menu that appears when
//! setting up network games. It manages player connection status and
//! displays connection progress information.

use crate::gui::{GameWindow, WindowManager, WindowMessage, WindowMsgData, WindowMsgHandled};
use gamelogic::helpers::TheGameText;
use log::{debug, error, warn};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};

/// Maximum number of player slots supported
const MAX_SLOTS: usize = 8;

/// Connection state types for network players
#[derive(Debug, Clone, PartialEq)]
pub enum NATConnectionState {
    WaitingForManglerResponse,
    WaitingForMangledPort,
    WaitingForResponse,
    Done,
    Failed,
    WaitingToBegin,
}

/// Menu state for the EstablishConnectionsMenu
#[derive(Debug, Clone, PartialEq)]
pub enum EstablishConnectionsMenuState {
    ScreenOn,
    ScreenOff,
}

/// The EstablishConnectionsMenu manages the network connection setup screen
pub struct EstablishConnectionsMenu {
    /// Root window for the menu.
    window: Option<Rc<RefCell<GameWindow>>>,

    /// Window manager reference for loading/destroying windows.
    window_manager: Option<Arc<Mutex<WindowManager>>>,

    /// Cached player name controls.
    player_name_controls: Vec<Option<Rc<RefCell<GameWindow>>>>,

    /// Cached player status controls.
    player_status_controls: Vec<Option<Rc<RefCell<GameWindow>>>>,

    /// Quit button control.
    quit_button: Option<Rc<RefCell<GameWindow>>>,

    /// Current menu state
    menu_state: EstablishConnectionsMenuState,

    /// Control names for player ready buttons
    player_ready_control_names: [&'static str; MAX_SLOTS],

    /// Control names for player name displays
    player_name_control_names: [&'static str; MAX_SLOTS],

    /// Control names for player status displays
    player_status_control_names: [&'static str; MAX_SLOTS],

    /// Cached player display names for slots
    player_names: Vec<String>,

    /// Cached player connection status for slots
    player_statuses: Vec<NATConnectionState>,

    /// Whether the menu initiated a local abort
    aborted: bool,
}

impl EstablishConnectionsMenu {
    /// Create a new EstablishConnectionsMenu instance
    pub fn new() -> Self {
        debug!("Creating new EstablishConnectionsMenu");

        Self {
            window: None,
            window_manager: None,
            player_name_controls: Vec::with_capacity(MAX_SLOTS),
            player_status_controls: Vec::with_capacity(MAX_SLOTS),
            quit_button: None,
            menu_state: EstablishConnectionsMenuState::ScreenOff,
            player_ready_control_names: [
                "EstablishConnectionsScreen.wnd:ButtonAccept1",
                "EstablishConnectionsScreen.wnd:ButtonAccept2",
                "EstablishConnectionsScreen.wnd:ButtonAccept3",
                "EstablishConnectionsScreen.wnd:ButtonAccept4",
                "EstablishConnectionsScreen.wnd:ButtonAccept5",
                "EstablishConnectionsScreen.wnd:ButtonAccept6",
                "EstablishConnectionsScreen.wnd:ButtonAccept7",
                "",
            ],
            player_name_control_names: [
                "EstablishConnectionsScreen.wnd:StaticPlayer1Name",
                "EstablishConnectionsScreen.wnd:StaticPlayer2Name",
                "EstablishConnectionsScreen.wnd:StaticPlayer3Name",
                "EstablishConnectionsScreen.wnd:StaticPlayer4Name",
                "EstablishConnectionsScreen.wnd:StaticPlayer5Name",
                "EstablishConnectionsScreen.wnd:StaticPlayer6Name",
                "EstablishConnectionsScreen.wnd:StaticPlayer7Name",
                "",
            ],
            player_status_control_names: [
                "EstablishConnectionsScreen.wnd:StaticPlayer1Status",
                "EstablishConnectionsScreen.wnd:StaticPlayer2Status",
                "EstablishConnectionsScreen.wnd:StaticPlayer3Status",
                "EstablishConnectionsScreen.wnd:StaticPlayer4Status",
                "EstablishConnectionsScreen.wnd:StaticPlayer5Status",
                "EstablishConnectionsScreen.wnd:StaticPlayer6Status",
                "EstablishConnectionsScreen.wnd:StaticPlayer7Status",
                "",
            ],
            player_names: vec![String::new(); MAX_SLOTS],
            player_statuses: vec![NATConnectionState::WaitingToBegin; MAX_SLOTS],
            aborted: false,
        }
    }

    /// Attach a window manager for loading the menu layout.
    pub fn set_window_manager(&mut self, window_manager: Arc<Mutex<WindowManager>>) {
        self.window_manager = Some(window_manager);
    }

    /// Initialize the menu and show the connections window
    pub fn init_menu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Initializing EstablishConnectionsMenu");
        self.menu_state = EstablishConnectionsMenuState::ScreenOn;
        self.aborted = false;

        if let Some(manager) = &self.window_manager {
            let mut manager = manager.lock().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "WindowManager lock poisoned")
            })?;
            if self.window.is_none() {
                self.window = Some(manager.load_window("EstablishConnectionsScreen.wnd")?);
            }
        }

        self.cache_controls();
        self.show_window();
        self.refresh_player_controls();

        Ok(())
    }

    /// End the menu and hide the connections window
    pub fn end_menu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Ending EstablishConnectionsMenu");
        self.menu_state = EstablishConnectionsMenuState::ScreenOff;

        self.hide_window();

        Ok(())
    }

    /// Abort the game gracefully
    ///
    /// As noted in the original C++ code: "It's really sad that this game isn't going to be played
    /// considering how difficult it is to even get a game going in the first place,
    /// especially one with more than two players."
    pub fn abort_game(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        warn!("Aborting game from EstablishConnectionsMenu - this is unfortunate!");

        self.aborted = true;
        self.menu_state = EstablishConnectionsMenuState::ScreenOff;

        self.hide_window();

        Ok(())
    }

    /// Set the player name for a given slot
    pub fn set_player_name(
        &mut self,
        slot: i32,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if slot < 0 || slot as usize >= MAX_SLOTS {
            error!("Invalid slot number {} for setPlayerName", slot);
            return Err(format!("Invalid slot number: {}", slot).into());
        }

        debug!("Setting player name for slot {}: {}", slot, name);

        let control_name = self.player_name_control_names[slot as usize];
        if control_name.is_empty() {
            error!("No control name defined for slot {}", slot);
            return Err(format!("No control name for slot: {}", slot).into());
        }

        self.player_names[slot as usize] = name.to_string();
        self.update_player_name_control(slot as usize);

        Ok(())
    }

    /// Set the connection status for a given player slot
    pub fn set_player_status(
        &mut self,
        slot: i32,
        state: NATConnectionState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if slot < 0 || slot as usize >= MAX_SLOTS {
            error!("Invalid slot number {} for setPlayerStatus", slot);
            return Err(format!("Invalid slot number: {}", slot).into());
        }

        debug!("Setting player status for slot {}: {:?}", slot, state);

        let control_name = self.player_status_control_names[slot as usize];
        if control_name.is_empty() {
            error!("No control name defined for slot {}", slot);
            return Err(format!("No control name for slot: {}", slot).into());
        }

        let status_text = match state {
            NATConnectionState::WaitingForManglerResponse => {
                TheGameText::fetch("GUI:WaitingForManglerResponse")
            }
            NATConnectionState::WaitingForMangledPort => {
                TheGameText::fetch("GUI:WaitingForMangledPort")
            }
            NATConnectionState::WaitingForResponse => TheGameText::fetch("GUI:WaitingForResponse"),
            NATConnectionState::Done => TheGameText::fetch("GUI:ConnectionDone"),
            NATConnectionState::Failed => TheGameText::fetch("GUI:ConnectionFailed"),
            NATConnectionState::WaitingToBegin => {
                TheGameText::fetch("GUI:WaitingToBeginConnection")
            }
        };

        self.player_statuses[slot as usize] = state.clone();

        if log::log_enabled!(log::Level::Debug) {
            debug!(
                "Status text for slot {}: {} ({})",
                slot, status_text, control_name
            );
        }

        self.update_player_status_control(slot as usize, &status_text);

        Ok(())
    }

    /// Handle system messages for the establish connections window.
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        let target_name = if data1 != 0 {
            window
                .find_child_by_id(data1 as i32)
                .map(|child| child.borrow().get_name().to_string())
        } else {
            None
        };
        let name = target_name.as_deref().unwrap_or_else(|| window.get_name());

        if msg == WindowMessage::GadgetSelected
            && name == "EstablishConnectionsScreen.wnd:ButtonQuit"
        {
            if let Err(err) = self.abort_game() {
                warn!("Failed to abort establish connections menu: {}", err);
            }
            return WindowMsgHandled::Handled;
        }

        WindowMsgHandled::Ignored
    }

    fn cache_controls(&mut self) {
        self.player_name_controls.clear();
        self.player_status_controls.clear();
        self.quit_button = None;

        if let Some(window) = &self.window {
            for name in &self.player_name_control_names {
                let control = window.borrow().find_child_window(name);
                self.player_name_controls.push(control);
            }

            for name in &self.player_status_control_names {
                let control = window.borrow().find_child_window(name);
                self.player_status_controls.push(control);
            }

            self.quit_button = window
                .borrow()
                .find_child_window("EstablishConnectionsScreen.wnd:ButtonQuit");
        }
    }

    fn show_window(&mut self) {
        if let Some(window) = &self.window {
            let mut window = window.borrow_mut();
            let _ = window.show();
            let _ = window.bring_to_front();
        }

        if let (Some(manager), Some(window)) = (&self.window_manager, &self.window) {
            if let Ok(mut manager) = manager.lock() {
                let _ = manager.set_focus(Some(window));
            }
        }
    }

    fn hide_window(&mut self) {
        if let Some(window) = &self.window {
            let _ = window.borrow_mut().hide(true);
        }

        if let (Some(manager), Some(window)) = (&self.window_manager, self.window.take()) {
            if let Ok(mut manager) = manager.lock() {
                let _ = manager.destroy_window(window);
            }
        }

        self.player_name_controls.clear();
        self.player_status_controls.clear();
        self.quit_button = None;
    }

    fn refresh_player_controls(&mut self) {
        for slot in 0..MAX_SLOTS {
            self.update_player_name_control(slot);
            let status = self.player_statuses.get(slot).cloned();
            if let Some(status) = status {
                let status_text = match status {
                    NATConnectionState::WaitingForManglerResponse => {
                        "GUI:WaitingForManglerResponse"
                    }
                    NATConnectionState::WaitingForMangledPort => "GUI:WaitingForMangledPort",
                    NATConnectionState::WaitingForResponse => "GUI:WaitingForResponse",
                    NATConnectionState::Done => "GUI:ConnectionDone",
                    NATConnectionState::Failed => "GUI:ConnectionFailed",
                    NATConnectionState::WaitingToBegin => "GUI:WaitingToBeginConnection",
                };
                self.update_player_status_control(slot, status_text);
            }
        }
    }

    fn update_player_name_control(&mut self, slot: usize) {
        if slot >= self.player_names.len() {
            return;
        }
        let name = self.player_names[slot].clone();
        if let Some(control) = self
            .player_name_controls
            .get(slot)
            .and_then(|control| control.as_ref())
        {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(name.clone());
            }
            let _ = control.set_text(&name);
        }
    }

    fn update_player_status_control(&mut self, slot: usize, status_text: &str) {
        if let Some(control) = self
            .player_status_controls
            .get(slot)
            .and_then(|control| control.as_ref())
        {
            let mut control = control.borrow_mut();
            if let Some(text) = control.static_text_mut() {
                text.set_text(status_text.to_string());
            }
            let _ = control.set_text(status_text);
        }
    }

    /// Get cached player name for a slot
    pub fn get_player_name(&self, slot: usize) -> Option<&str> {
        if slot >= MAX_SLOTS {
            return None;
        }
        let name = &self.player_names[slot];
        if name.is_empty() {
            None
        } else {
            Some(name.as_str())
        }
    }

    /// Get cached status for a slot
    pub fn get_player_status(&self, slot: usize) -> Option<&NATConnectionState> {
        self.player_statuses.get(slot)
    }

    /// Check whether the menu initiated an abort
    pub fn was_aborted(&self) -> bool {
        self.aborted
    }

    /// Get the current menu state
    pub fn get_menu_state(&self) -> &EstablishConnectionsMenuState {
        &self.menu_state
    }

    /// Check if the menu is currently active (screen on)
    pub fn is_active(&self) -> bool {
        matches!(self.menu_state, EstablishConnectionsMenuState::ScreenOn)
    }

    /// Get the control name for a player ready button
    pub fn get_player_ready_control_name(&self, slot: usize) -> Option<&str> {
        if slot >= MAX_SLOTS {
            return None;
        }
        let name = self.player_ready_control_names[slot];
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// Get the control name for a player name display
    pub fn get_player_name_control_name(&self, slot: usize) -> Option<&str> {
        if slot >= MAX_SLOTS {
            return None;
        }
        let name = self.player_name_control_names[slot];
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// Get the control name for a player status display
    pub fn get_player_status_control_name(&self, slot: usize) -> Option<&str> {
        if slot >= MAX_SLOTS {
            return None;
        }
        let name = self.player_status_control_names[slot];
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}

impl Default for EstablishConnectionsMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Global instance of the EstablishConnectionsMenu
///
/// This mirrors the C++ TheEstablishConnectionsMenu singleton pattern.
/// In a real implementation, you might want to use a more sophisticated
/// dependency injection system instead of a global static.
thread_local! {
    static THE_ESTABLISH_CONNECTIONS_MENU: Arc<RwLock<EstablishConnectionsMenu>> =
        Arc::new(RwLock::new(EstablishConnectionsMenu::new()));
}

/// Helper function to get a reference to the global EstablishConnectionsMenu
pub fn get_establish_connections_menu() -> Arc<RwLock<EstablishConnectionsMenu>> {
    THE_ESTABLISH_CONNECTIONS_MENU.with(|menu| menu.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_establish_connections_menu_creation() {
        let menu = EstablishConnectionsMenu::new();
        assert_eq!(
            menu.get_menu_state(),
            &EstablishConnectionsMenuState::ScreenOff
        );
        assert!(!menu.is_active());
    }

    #[test]
    fn test_menu_state_transitions() {
        let mut menu = EstablishConnectionsMenu::new();

        // Start with screen off
        assert_eq!(
            menu.get_menu_state(),
            &EstablishConnectionsMenuState::ScreenOff
        );

        // Initialize menu
        menu.init_menu().unwrap();
        assert_eq!(
            menu.get_menu_state(),
            &EstablishConnectionsMenuState::ScreenOn
        );
        assert!(menu.is_active());

        // End menu
        menu.end_menu().unwrap();
        assert_eq!(
            menu.get_menu_state(),
            &EstablishConnectionsMenuState::ScreenOff
        );
        assert!(!menu.is_active());
    }

    #[test]
    fn test_set_player_name_valid_slot() {
        let mut menu = EstablishConnectionsMenu::new();
        let result = menu.set_player_name(0, "TestPlayer");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_player_name_invalid_slot() {
        let mut menu = EstablishConnectionsMenu::new();
        let result = menu.set_player_name(-1, "TestPlayer");
        assert!(result.is_err());

        let result = menu.set_player_name(MAX_SLOTS as i32, "TestPlayer");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_player_status_valid_slot() {
        let mut menu = EstablishConnectionsMenu::new();
        let result = menu.set_player_status(0, NATConnectionState::Done);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_player_status_invalid_slot() {
        let mut menu = EstablishConnectionsMenu::new();
        let result = menu.set_player_status(-1, NATConnectionState::Done);
        assert!(result.is_err());

        let result = menu.set_player_status(MAX_SLOTS as i32, NATConnectionState::Done);
        assert!(result.is_err());
    }

    #[test]
    fn test_control_name_getters() {
        let menu = EstablishConnectionsMenu::new();

        // Test valid slots
        assert!(menu.get_player_ready_control_name(0).is_some());
        assert!(menu.get_player_name_control_name(0).is_some());
        assert!(menu.get_player_status_control_name(0).is_some());

        // Test invalid slots
        assert!(menu.get_player_ready_control_name(MAX_SLOTS).is_none());
        assert!(menu.get_player_name_control_name(MAX_SLOTS).is_none());
        assert!(menu.get_player_status_control_name(MAX_SLOTS).is_none());

        // Test last valid slot (which has empty string)
        assert!(menu.get_player_ready_control_name(MAX_SLOTS - 1).is_none());
        assert!(menu.get_player_name_control_name(MAX_SLOTS - 1).is_none());
        assert!(menu.get_player_status_control_name(MAX_SLOTS - 1).is_none());
    }

    #[test]
    fn test_cached_player_fields() {
        let mut menu = EstablishConnectionsMenu::new();

        assert_eq!(menu.get_player_name(0), None);
        assert_eq!(
            menu.get_player_status(0),
            Some(&NATConnectionState::WaitingToBegin)
        );

        menu.set_player_name(0, "Tester").unwrap();
        menu.set_player_status(0, NATConnectionState::Done).unwrap();

        assert_eq!(menu.get_player_name(0), Some("Tester"));
        assert_eq!(menu.get_player_status(0), Some(&NATConnectionState::Done));
    }

    #[test]
    fn test_abort_sets_state() {
        let mut menu = EstablishConnectionsMenu::new();
        menu.init_menu().unwrap();
        assert!(!menu.was_aborted());
        assert!(menu.is_active());

        menu.abort_game().unwrap();
        assert!(menu.was_aborted());
        assert!(!menu.is_active());
    }

    #[test]
    fn test_connection_states() {
        use NATConnectionState::*;

        // Test all connection states can be created
        let states = vec![
            WaitingForManglerResponse,
            WaitingForMangledPort,
            WaitingForResponse,
            Done,
            Failed,
            WaitingToBegin,
        ];

        for state in states {
            let mut menu = EstablishConnectionsMenu::new();
            assert!(menu.set_player_status(0, state).is_ok());
        }
    }

    #[test]
    fn test_global_menu_instance() {
        let menu1 = get_establish_connections_menu();
        let menu2 = get_establish_connections_menu();

        // Both should point to the same instance
        assert!(Arc::ptr_eq(&menu1, &menu2));

        // Test that we can lock and use the menu
        {
            let mut menu = menu1.write().unwrap();
            menu.init_menu().unwrap();
            assert!(menu.is_active());
        }

        // Check state through the second reference
        {
            let menu = menu2.read().unwrap();
            assert!(menu.is_active());
        }
    }
}
