// multiplayer_settings.rs - Port of Common/MultiplayerSettings.cpp
// Original: GeneralsMD/Code/GameEngine/Source/Common/MultiplayerSettings.cpp

/// RGBA color packed as 0xAARRGGBB.
pub type MultiplayerColor = u32;

/// Per-player color definition with tooltip, day, and night variants.
/// Matches C++ MultiplayerColorDefinition.
#[derive(Debug, Clone)]
pub struct MultiplayerColorDefinition {
    pub tooltip_name: String,
    pub color: MultiplayerColor,
    pub color_night: MultiplayerColor,
}

impl MultiplayerColorDefinition {
    pub fn new(tooltip_name: &str, r: u8, g: u8, b: u8, nr: u8, ng: u8, nb: u8) -> Self {
        Self {
            tooltip_name: tooltip_name.to_string(),
            color: ((0xFFu32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
            color_night: ((0xFFu32) << 24) | ((nr as u32) << 16) | ((ng as u32) << 8) | (nb as u32),
        }
    }
}

/// Multiplayer game settings singleton data.
/// Matches C++ MultiplayerSettings (TheMultiplayerSettings).
#[derive(Debug, Clone)]
pub struct MultiplayerSettings {
    pub max_players: u32,
    pub game_name: String,
    pub password: Option<String>,
    pub map_name: String,
    pub use_random_seed: bool,
    pub seed: u32,

    // C++ INI fields
    pub start_countdown_timer: i32,
    pub max_beacons_per_player: i32,
    pub use_shroud: bool,
    pub show_random_player_template: bool,
    pub show_random_start_pos: bool,
    pub show_random_color: bool,

    pub colors: Vec<MultiplayerColorDefinition>,
    pub starting_money_choices: Vec<i32>,
    pub default_starting_money: i32,
}

const DEFAULT_MAX_BEACONS: i32 = 3;

impl Default for MultiplayerSettings {
    fn default() -> Self {
        Self {
            max_players: 8,
            game_name: "New Game".to_string(),
            password: None,
            map_name: "Default".to_string(),
            use_random_seed: true,
            seed: 0,
            start_countdown_timer: 5,
            max_beacons_per_player: DEFAULT_MAX_BEACONS,
            use_shroud: true,
            show_random_player_template: true,
            show_random_start_pos: true,
            show_random_color: true,
            colors: Vec::new(),
            starting_money_choices: vec![5000, 10000, 20000, 30000, 40000, 50000],
            default_starting_money: 10000,
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

    /// Matches C++ MultiplayerSettings::getColor(Int which).
    pub fn get_color(&self, index: i32) -> Option<MultiplayerColor> {
        if index < 0 {
            return None;
        }
        let idx = index as usize;
        self.colors.get(idx).map(|c| c.color)
    }

    /// Matches C++ MultiplayerSettings::getNightColor(Int which).
    pub fn get_night_color(&self, index: i32) -> Option<MultiplayerColor> {
        if index < 0 {
            return None;
        }
        let idx = index as usize;
        self.colors.get(idx).map(|c| c.color_night)
    }

    /// Matches C++ MultiplayerSettings::addColor.
    pub fn add_color(&mut self, def: MultiplayerColorDefinition) {
        self.colors.push(def);
    }

    /// Matches C++ MultiplayerSettings::addStartingMoneyChoice.
    pub fn add_starting_money_choice(&mut self, amount: i32, is_default: bool) {
        self.starting_money_choices.push(amount);
        if is_default {
            self.default_starting_money = amount;
        }
    }
}
