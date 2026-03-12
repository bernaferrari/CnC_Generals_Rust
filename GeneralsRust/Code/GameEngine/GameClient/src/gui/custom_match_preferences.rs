//! Custom match preferences storage for custom multiplayer lobbies.

use game_engine::common::preferences::CustomMatchPreferences;

#[derive(Debug, Default)]
pub struct CustomMatchPreferencesStore {
    prefs: CustomMatchPreferences,
}

impl CustomMatchPreferencesStore {
    pub fn new() -> Self {
        Self {
            prefs: CustomMatchPreferences::new(),
        }
    }

    pub fn prefs(&self) -> &CustomMatchPreferences {
        &self.prefs
    }

    pub fn prefs_mut(&mut self) -> &mut CustomMatchPreferences {
        &mut self.prefs
    }

    pub fn load(&mut self) {
        self.prefs = CustomMatchPreferences::new();
    }

    pub fn write(&mut self) {
        let _ = self.prefs.write();
    }
}
