// multiplayer_settings.rs - Port of Common/MultiplayerSettings.cpp
// Original: GeneralsMD/Code/GameEngine/Source/Common/MultiplayerSettings.cpp

/// RGBA color packed as 0xAARRGGBB.
pub type MultiplayerColor = u32;

/// C++ `PLAYERTEMPLATE_RANDOM` / `PLAYERTEMPLATE_OBSERVER` from GameInfo.h.
pub const PLAYERTEMPLATE_RANDOM: i32 = -1;
pub const PLAYERTEMPLATE_OBSERVER: i32 = -2;

/// C++ MultiplayerColorDefinition ctor: RGB + packed color are white.
pub const MULTIPLAYER_COLOR_WHITE: MultiplayerColor = 0xFFFF_FFFF;

/// Per-player color definition with tooltip, day, and night variants.
/// Matches C++ MultiplayerColorDefinition.
#[derive(Debug, Clone)]
pub struct MultiplayerColorDefinition {
    pub tooltip_name: String,
    pub color: MultiplayerColor,
    pub color_night: MultiplayerColor,
}

impl Default for MultiplayerColorDefinition {
    fn default() -> Self {
        Self {
            tooltip_name: String::new(),
            color: MULTIPLAYER_COLOR_WHITE,
            color_night: MULTIPLAYER_COLOR_WHITE,
        }
    }
}

impl MultiplayerColorDefinition {
    pub fn new(tooltip_name: &str, r: u8, g: u8, b: u8, nr: u8, ng: u8, nb: u8) -> Self {
        let mut def = Self {
            tooltip_name: tooltip_name.to_string(),
            color: 0,
            color_night: 0,
        };
        def.set_color(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        def.set_night_color(((nr as u32) << 16) | ((ng as u32) << 8) | (nb as u32));
        def
    }

    /// C++ MultiplayerColorDefinition::setColor: `rgb.getAsInt() | (0xFF << 24)`.
    pub fn set_color(&mut self, rgb: MultiplayerColor) {
        self.color = rgb | 0xFF00_0000;
    }

    /// C++ MultiplayerColorDefinition::setNightColor.
    pub fn set_night_color(&mut self, rgb: MultiplayerColor) {
        self.color_night = rgb | 0xFF00_0000;
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
    pub got_default_starting_money: bool,
    /// C++ `m_randomColor` — returned by `getColor(PLAYERTEMPLATE_RANDOM)`.
    pub random_color: MultiplayerColorDefinition,
    /// C++ `m_observerColor` — returned by `getColor(PLAYERTEMPLATE_OBSERVER)`.
    pub observer_color: MultiplayerColorDefinition,
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
            // C++ MultiplayerSettings ctor: countdown 0, starting-money list empty
            // until INI addStartingMoneyChoice. Extra lobby fields are Rust-only.
            start_countdown_timer: 0,
            max_beacons_per_player: DEFAULT_MAX_BEACONS,
            use_shroud: true,
            show_random_player_template: true,
            show_random_start_pos: true,
            show_random_color: true,
            colors: Vec::new(),
            starting_money_choices: Vec::new(),
            default_starting_money: 0,
            got_default_starting_money: false,
            random_color: MultiplayerColorDefinition::default(),
            observer_color: MultiplayerColorDefinition::default(),
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

    /// C++ MultiplayerSettings::getColor — RANDOM/OBSERVER specials, else list.
    pub fn get_color_definition(&self, which: i32) -> Option<&MultiplayerColorDefinition> {
        if which == PLAYERTEMPLATE_RANDOM {
            Some(&self.random_color)
        } else if which == PLAYERTEMPLATE_OBSERVER {
            Some(&self.observer_color)
        } else if which < 0 || which as usize >= self.colors.len() {
            None
        } else {
            self.colors.get(which as usize)
        }
    }

    /// Packed day color from `getColor(which)`.
    pub fn get_color(&self, which: i32) -> Option<MultiplayerColor> {
        self.get_color_definition(which).map(|c| c.color)
    }

    /// Packed night color from the same definition `getColor` would return.
    pub fn get_night_color(&self, which: i32) -> Option<MultiplayerColor> {
        self.get_color_definition(which).map(|c| c.color_night)
    }

    /// C++ MultiplayerSettings::findMultiplayerColorDefinitionByName — tooltip only.
    pub fn find_multiplayer_color_definition_by_name(
        &self,
        name: &str,
    ) -> Option<&MultiplayerColorDefinition> {
        self.colors.iter().find(|def| def.tooltip_name == name)
    }

    /// C++ MultiplayerSettings::newMultiplayerColorDefinition — name is unused;
    /// a default-white definition is inserted at the next index.
    pub fn new_multiplayer_color_definition(&mut self, _name: &str) -> &mut MultiplayerColorDefinition {
        self.colors.push(MultiplayerColorDefinition::default());
        self.colors.last_mut().unwrap()
    }

    /// Matches C++ MultiplayerSettings::addColor (append a prepared definition).
    pub fn add_color(&mut self, def: MultiplayerColorDefinition) {
        self.colors.push(def);
    }

    /// C++ MultiplayerSettings::addStartingMoneyChoice. A second default still
    /// overwrites (DEBUG_ASSERTCRASH then continues).
    pub fn add_starting_money_choice(&mut self, amount: i32, is_default: bool) {
        self.starting_money_choices.push(amount);
        if is_default {
            self.default_starting_money = amount;
            self.got_default_starting_money = true;
        }
    }

    pub fn get_num_colors(&self) -> i32 {
        self.colors.len() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_color_random_and_observer_are_ctor_white() {
        // C++ MultiplayerSettings.cpp:75-91 + ColorDefinition ctor 66-72.
        let settings = MultiplayerSettings::new();
        assert_eq!(
            settings.get_color(PLAYERTEMPLATE_RANDOM),
            Some(MULTIPLAYER_COLOR_WHITE)
        );
        assert_eq!(
            settings.get_color(PLAYERTEMPLATE_OBSERVER),
            Some(MULTIPLAYER_COLOR_WHITE)
        );
        assert_eq!(
            settings.get_night_color(PLAYERTEMPLATE_RANDOM),
            Some(MULTIPLAYER_COLOR_WHITE)
        );
        assert_eq!(settings.get_color(-3), None);
        assert_eq!(settings.get_color(0), None);
    }

    #[test]
    fn set_color_forces_opaque_alpha() {
        // C++ setColor: rgb.getAsInt() | (0xFF << 24)
        let mut def = MultiplayerColorDefinition::default();
        def.set_color(0x00FF_0000);
        assert_eq!(def.color, 0xFFFF_0000);
        def.set_night_color(0x0000_00FF);
        assert_eq!(def.color_night, 0xFF00_00FF);
    }

    #[test]
    fn find_color_is_tooltip_only() {
        let mut settings = MultiplayerSettings::new();
        let def = settings.new_multiplayer_color_definition("Red");
        def.tooltip_name = "GUI:Red".to_string();
        def.set_color(0x00FF_0000);
        assert!(
            settings
                .find_multiplayer_color_definition_by_name("GUI:Red")
                .is_some()
        );
        assert!(
            settings
                .find_multiplayer_color_definition_by_name("Red")
                .is_none()
        );
        assert_eq!(settings.get_color(0), Some(0xFFFF_0000));
    }

    #[test]
    fn second_default_starting_money_still_overwrites() {
        let mut settings = MultiplayerSettings::new();
        settings.add_starting_money_choice(10_000, true);
        assert_eq!(settings.default_starting_money, 10_000);
        settings.add_starting_money_choice(5_000, true);
        assert_eq!(settings.default_starting_money, 5_000);
        assert_eq!(settings.starting_money_choices, vec![10_000, 5_000]);
        assert!(settings.got_default_starting_money);
    }
}
