//! Shared skirmish setup state for menu coordination.

use game_network::SkirmishGameInfo;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Default)]
pub struct SkirmishSetup {
    selected_map: String,
    use_system_maps: bool,
    game_info: SkirmishGameInfo,
}

impl SkirmishSetup {
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

    pub fn game_info(&self) -> &SkirmishGameInfo {
        &self.game_info
    }

    pub fn game_info_mut(&mut self) -> &mut SkirmishGameInfo {
        &mut self.game_info
    }
}

static SKIRMISH_SETUP: OnceLock<Mutex<SkirmishSetup>> = OnceLock::new();

pub fn get_skirmish_setup() -> std::sync::MutexGuard<'static, SkirmishSetup> {
    SKIRMISH_SETUP
        .get_or_init(|| Mutex::new(SkirmishSetup::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
