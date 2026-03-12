// multiplayer_settings.rs - Multiplayer game settings placeholder

/// Multiplayer game settings
#[derive(Debug, Clone)]
pub struct MultiplayerSettings {
    pub max_players: u32,
    pub game_name: String,
    pub password: Option<String>,
    pub map_name: String,
    pub use_random_seed: bool,
    pub seed: u32,
}

impl Default for MultiplayerSettings {
    fn default() -> Self {
        Self {
            max_players: 8,
            game_name: "New Game".to_string(),
            password: None,
            map_name: "Default".to_string(),
            use_random_seed: true,
            seed: 0,
        }
    }
}

impl MultiplayerSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_max_players(&mut self, max_players: u32) {
        self.max_players = max_players;
    }

    pub fn set_password(&mut self, password: Option<String>) {
        self.password = password;
    }

    pub fn has_password(&self) -> bool {
        self.password.is_some()
    }
}
