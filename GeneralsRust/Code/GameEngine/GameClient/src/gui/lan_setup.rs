//! Shared LAN setup state for menu coordination.

use game_network::GameInfo;
use std::sync::{Mutex, OnceLock};

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

static LAN_SETUP: OnceLock<Mutex<LanSetup>> = OnceLock::new();

pub fn get_lan_setup() -> std::sync::MutexGuard<'static, LanSetup> {
    LAN_SETUP
        .get_or_init(|| Mutex::new(LanSetup::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
