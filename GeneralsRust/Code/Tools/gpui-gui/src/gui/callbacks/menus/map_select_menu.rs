use crate::gui::callbacks::menus::main_menu::GameDifficultyPort;
use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/MapSelectMenu.cpp",
    "crate::gui::callbacks::menus::map_select_menu",
    "Map Select Menu",
    "Map selection screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "MapSelectMenu",
    "Map Select",
    "Browse and choose a scenario map.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapEntryPort {
    pub name: String,
    pub display_name: String,
    pub player_count: u8,
    pub official: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapSelectMenuPort {
    pub shell_map_visible: bool,
    pub visible: bool,
    pub show_solo_maps: bool,
    pub uses_system_map_dir: bool,
    pub is_shutting_down: bool,
    pub start_game: bool,
    pub button_pushed: bool,
    pub ai_difficulty: GameDifficultyPort,
    pub maps: Vec<MapEntryPort>,
    pub selected_index: Option<usize>,
    pub pending_file: Option<String>,
    pub pop_requested: bool,
}

impl Default for MapSelectMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl MapSelectMenuPort {
    pub fn init(maps: Vec<MapEntryPort>, uses_system_map_dir: bool) -> Self {
        Self {
            shell_map_visible: true,
            visible: true,
            show_solo_maps: true,
            uses_system_map_dir,
            is_shutting_down: false,
            start_game: false,
            button_pushed: false,
            ai_difficulty: GameDifficultyPort::Normal,
            selected_index: (!maps.is_empty()).then_some(0),
            maps,
            pending_file: None,
            pop_requested: false,
        }
    }

    pub fn set_map_directory(&mut self, uses_system_map_dir: bool) {
        self.uses_system_map_dir = uses_system_map_dir;
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficultyPort) {
        self.ai_difficulty = difficulty;
    }

    pub fn select_map(&mut self, index: usize) -> bool {
        if index >= self.maps.len() {
            return false;
        }
        self.selected_index = Some(index);
        true
    }

    pub fn confirm(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        let Some(map) = self.maps.get(index) else {
            return false;
        };
        self.start_game = true;
        self.pending_file = Some(map.name.clone());
        self.button_pushed = true;
        true
    }

    pub fn update(&mut self, shell_anim_finished: bool) -> bool {
        if self.start_game && shell_anim_finished {
            self.start_game = false;
            self.is_shutting_down = true;
            return true;
        }
        if self.is_shutting_down && shell_anim_finished {
            self.visible = false;
            self.is_shutting_down = false;
            return true;
        }
        false
    }

    pub fn handle_escape(&mut self, key_up: bool) -> bool {
        if self.button_pushed || !key_up {
            return false;
        }
        self.pop_requested = true;
        self.button_pushed = true;
        true
    }

    pub fn sample() -> Self {
        Self::init(
            vec![
                MapEntryPort {
                    name: "TournamentDesert".to_string(),
                    display_name: "Tournament Desert".to_string(),
                    player_count: 2,
                    official: true,
                },
                MapEntryPort {
                    name: "Defcon6".to_string(),
                    display_name: "Defcon 6".to_string(),
                    player_count: 6,
                    official: true,
                },
                MapEntryPort {
                    name: "TwilightFlame".to_string(),
                    display_name: "Twilight Flame".to_string(),
                    player_count: 8,
                    official: false,
                },
            ],
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirming_selected_map_starts_shell_reverse() {
        let mut menu = MapSelectMenuPort::sample();
        menu.set_difficulty(GameDifficultyPort::Hard);
        assert!(menu.select_map(1));

        assert!(menu.confirm());
        assert_eq!(menu.pending_file.as_deref(), Some("Defcon6"));
        assert_eq!(menu.ai_difficulty, GameDifficultyPort::Hard);
    }

    #[test]
    fn escape_requests_pop_once() {
        let mut menu = MapSelectMenuPort::sample();

        assert!(menu.handle_escape(true));
        assert!(menu.pop_requested);
        assert!(!menu.handle_escape(true));
    }
}
