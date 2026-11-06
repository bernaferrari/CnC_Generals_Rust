//! Shared LAN setup state for menu coordination.

use game_network::GameInfo;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct LanSetup {
    selected_map: String,
    use_system_maps: bool,
    game_info: GameInfo,
}

impl LanSetup {
    pub fn selected_map(&self) -> &str {
        &self.selected_map
    }

    pub fn set_selected_map(&mut self, map: String) {
        self.selected_map = map;
    }

    pub fn use_system_maps(&self) -> bool {
        self.use_system_maps
    }

    pub fn set_use_system_maps(&mut self, value: bool) {
        self.use_system_maps = value;
    }

    pub fn game_info(&self) -> &GameInfo {
        &self.game_info
    }

    pub fn game_info_mut(&mut self) -> &mut GameInfo {
        &mut self.game_info
    }
}

#[cfg(test)]
thread_local! {
    static TEST_LAN_SETUP: std::cell::RefCell<Option<Arc<Mutex<LanSetup>>>> =
        const { std::cell::RefCell::new(None) };
}

fn lan_setup_handle() -> Arc<Mutex<LanSetup>> {
    if let Some(handle) = crate::core::game_client::with_live_game_client_mut(|client| {
        client.ensure_lan_setup()
    }) {
        return handle;
    }
    #[cfg(test)]
    {
        return TEST_LAN_SETUP.with(|slot| {
            slot.borrow_mut()
                .get_or_insert_with(|| Arc::new(Mutex::new(LanSetup::default())))
                .clone()
        });
    }
    #[cfg(not(test))]
    {
        Arc::new(Mutex::new(LanSetup::default()))
    }
}

/// Handle to the LAN setup for this client (or test/ephemeral fallback).
pub fn get_lan_setup() -> Arc<Mutex<LanSetup>> {
    lan_setup_handle()
}

pub fn with_lan_setup<R>(f: impl FnOnce(&mut LanSetup) -> R) -> R {
    let handle = lan_setup_handle();
    let mut guard = handle.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}
