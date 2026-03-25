use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::map_select_menu::{MapEntryPort, MapSelectMenuPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SkirmishMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::skirmish_map_select_menu",
    "Skirmish Map Select Menu",
    "Skirmish map selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SkirmishMapSelectMenu",
    "Skirmish Maps",
    "Select a skirmish battleground.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapSortOrder {
    ByName,
    ByPlayerCount,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkirmishMapSelectAction {
    Back,
    Ok,
    SelectMap { index: usize },
    SystemMapsRadio,
    UserMapsRadio,
    DoubleClickMap { index: usize },
    Escape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapSelectionResult {
    pub map_file: String,
    pub display_name: String,
    pub crc: u32,
    pub size: u32,
    pub num_players: i32,
    pub is_official: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishMapSelectPort {
    pub map_select: MapSelectMenuPort,
    pub official_maps_only: bool,
    pub sort_order: MapSortOrder,
    pub selected_map_file: Option<String>,
    pub selected_map_display_name: Option<String>,
    pub selected_map_crc: u32,
    pub selected_map_size: u32,
    pub confirmed_result: Option<MapSelectionResult>,
    pub back_requested: bool,
    pub button_pushed: bool,
}

impl Default for SkirmishMapSelectPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl SkirmishMapSelectPort {
    pub fn sample() -> Self {
        let mut map_select = MapSelectMenuPort::sample();
        map_select.uses_system_map_dir = true;

        Self {
            map_select,
            official_maps_only: true,
            sort_order: MapSortOrder::ByPlayerCount,
            selected_map_file: None,
            selected_map_display_name: None,
            selected_map_crc: 0,
            selected_map_size: 0,
            confirmed_result: None,
            back_requested: false,
            button_pushed: false,
        }
    }

    pub fn init(
        maps: Vec<MapEntryPort>,
        uses_system_map_dir: bool,
        current_map_name: &str,
    ) -> Self {
        let mut port = Self::sample();
        port.map_select = MapSelectMenuPort::init(maps, uses_system_map_dir);
        port.official_maps_only = uses_system_map_dir;

        if !current_map_name.is_empty() {
            if let Some(idx) = port
                .map_select
                .maps
                .iter()
                .position(|m| m.name.eq_ignore_ascii_case(current_map_name))
            {
                port.map_select.selected_index = Some(idx);
                port.select_map_internal(idx);
            }
        }

        port
    }

    pub fn set_sort_order(&mut self, order: MapSortOrder) {
        self.sort_order = order;
        self.sort_maps();
    }

    fn sort_maps(&mut self) {
        match self.sort_order {
            MapSortOrder::ByName => {
                self.map_select.maps.sort_by(|a, b| {
                    a.display_name
                        .to_lowercase()
                        .cmp(&b.display_name.to_lowercase())
                });
            }
            MapSortOrder::ByPlayerCount => {
                self.map_select.maps.sort_by(|a, b| {
                    let players = a.player_count.cmp(&b.player_count);
                    if players != std::cmp::Ordering::Equal {
                        return players;
                    }
                    a.display_name
                        .to_lowercase()
                        .cmp(&b.display_name.to_lowercase())
                });
            }
        }
        self.map_select.selected_index = if self.map_select.maps.is_empty() {
            None
        } else {
            Some(0)
        };
        if let Some(idx) = self.map_select.selected_index {
            self.select_map_internal(idx);
        }
    }

    fn select_map_internal(&mut self, index: usize) {
        if let Some(entry) = self.map_select.maps.get(index) {
            self.selected_map_file = Some(entry.name.clone());
            self.selected_map_display_name = Some(entry.display_name.clone());
            self.selected_map_crc = 0;
            self.selected_map_size = 0;
        } else {
            self.clear_selection();
        }
    }

    fn clear_selection(&mut self) {
        self.selected_map_file = None;
        self.selected_map_display_name = None;
        self.selected_map_crc = 0;
        self.selected_map_size = 0;
    }

    pub fn set_map_metadata(&mut self, crc: u32, size: u32) {
        self.selected_map_crc = crc;
        self.selected_map_size = size;
    }

    pub fn update_map_metadata_from_cache(&mut self, find_fn: impl Fn(&str) -> Option<(u32, u32)>) {
        if let Some(ref map_file) = self.selected_map_file {
            if let Some((crc, size)) = find_fn(map_file) {
                self.selected_map_crc = crc;
                self.selected_map_size = size;
            }
        }
    }

    pub fn select_map(&mut self, index: usize) -> bool {
        if self.button_pushed {
            return false;
        }
        if !self.map_select.select_map(index) {
            return false;
        }
        self.select_map_internal(index);
        true
    }

    pub fn confirm(&mut self) -> bool {
        if self.button_pushed {
            return false;
        }
        let Some(ref map_file) = self.selected_map_file else {
            return false;
        };
        let Some(entry) = self
            .map_select
            .maps
            .get(self.map_select.selected_index.unwrap_or(usize::MAX))
        else {
            return false;
        };

        self.button_pushed = true;
        self.confirmed_result = Some(MapSelectionResult {
            map_file: map_file.clone(),
            display_name: entry.display_name.clone(),
            crc: self.selected_map_crc,
            size: self.selected_map_size,
            num_players: entry.player_count as i32,
            is_official: entry.official,
        });
        true
    }

    pub fn back(&mut self) -> bool {
        if self.button_pushed {
            return false;
        }
        self.button_pushed = true;
        self.back_requested = true;
        true
    }

    pub fn switch_to_system_maps(&mut self, maps: Vec<MapEntryPort>, current_map_name: &str) {
        self.map_select.maps = maps;
        self.official_maps_only = true;
        self.map_select.uses_system_map_dir = true;
        self.sort_maps();

        if !current_map_name.is_empty() {
            if let Some(idx) = self
                .map_select
                .maps
                .iter()
                .position(|m| m.name.eq_ignore_ascii_case(current_map_name))
            {
                self.map_select.selected_index = Some(idx);
                self.select_map_internal(idx);
            }
        }
    }

    pub fn switch_to_user_maps(
        &mut self,
        solo_maps: Vec<MapEntryPort>,
        multi_maps: Vec<MapEntryPort>,
        current_map_name: &str,
    ) {
        let mut maps = solo_maps;
        maps.extend(multi_maps);
        self.map_select.maps = maps;
        self.official_maps_only = false;
        self.map_select.uses_system_map_dir = false;
        self.sort_maps();

        if !current_map_name.is_empty() {
            if let Some(idx) = self
                .map_select
                .maps
                .iter()
                .position(|m| m.name.eq_ignore_ascii_case(current_map_name))
            {
                self.map_select.selected_index = Some(idx);
                self.select_map_internal(idx);
            }
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selected_map_file.is_some()
    }

    pub fn selected_entry(&self) -> Option<&MapEntryPort> {
        let idx = self.map_select.selected_index?;
        self.map_select.maps.get(idx)
    }

    pub fn handle_button(&mut self, action: SkirmishMapSelectAction) -> bool {
        match action {
            SkirmishMapSelectAction::Back => self.back(),
            SkirmishMapSelectAction::Ok => self.confirm(),
            SkirmishMapSelectAction::SelectMap { index } => self.select_map(index),
            SkirmishMapSelectAction::DoubleClickMap { index } => {
                if self.select_map(index) {
                    self.confirm()
                } else {
                    false
                }
            }
            SkirmishMapSelectAction::SystemMapsRadio => {
                if self.button_pushed {
                    return false;
                }
                self.map_select.set_map_directory(true);
                self.official_maps_only = true;
                true
            }
            SkirmishMapSelectAction::UserMapsRadio => {
                if self.button_pushed {
                    return false;
                }
                self.map_select.set_map_directory(false);
                self.official_maps_only = false;
                true
            }
            SkirmishMapSelectAction::Escape => {
                if self.button_pushed {
                    return false;
                }
                self.back()
            }
        }
    }

    pub fn take_result(&mut self) -> Option<MapSelectionResult> {
        self.confirmed_result.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_maps() -> Vec<MapEntryPort> {
        vec![
            MapEntryPort {
                name: "maps/tournament desert/tournament desert.map".to_string(),
                display_name: "Tournament Desert (2)".to_string(),
                player_count: 2,
                official: true,
            },
            MapEntryPort {
                name: "maps/defcon6/defcon6.map".to_string(),
                display_name: "Defcon 6 (6)".to_string(),
                player_count: 6,
                official: true,
            },
            MapEntryPort {
                name: "maps/twilight flame/twilight flame.map".to_string(),
                display_name: "Twilight Flame (8)".to_string(),
                player_count: 8,
                official: false,
            },
        ]
    }

    #[test]
    fn init_selects_current_map() {
        let port = SkirmishMapSelectPort::init(sample_maps(), true, "maps/defcon6/defcon6.map");
        assert_eq!(
            port.selected_map_file.as_deref(),
            Some("maps/defcon6/defcon6.map")
        );
    }

    #[test]
    fn select_map_updates_fields() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        assert!(port.select_map(2));
        assert_eq!(
            port.selected_map_display_name.as_deref(),
            Some("Twilight Flame (8)")
        );
    }

    #[test]
    fn confirm_sets_result() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.select_map(0);
        port.set_map_metadata(12345, 67890);
        assert!(port.confirm());

        let result = port.take_result().unwrap();
        assert_eq!(result.crc, 12345);
        assert_eq!(result.size, 67890);
        assert_eq!(result.num_players, 2);
        assert!(result.is_official);
    }

    #[test]
    fn confirm_without_selection_fails() {
        let mut port = SkirmishMapSelectPort::init(Vec::new(), true, "");
        assert!(!port.confirm());
    }

    #[test]
    fn back_sets_flag() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        assert!(port.back());
        assert!(port.back_requested);
    }

    #[test]
    fn double_click_selects_and_confirms() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        assert!(port.handle_button(SkirmishMapSelectAction::DoubleClickMap { index: 1 }));
        assert!(port.confirmed_result.is_some());
    }

    #[test]
    fn handle_button_ignores_when_pushed() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.button_pushed = true;
        assert!(!port.handle_button(SkirmishMapSelectAction::Ok));
        assert!(!port.handle_button(SkirmishMapSelectAction::Back));
        assert!(!port.handle_button(SkirmishMapSelectAction::SelectMap { index: 0 }));
    }

    #[test]
    fn escape_triggers_back() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        assert!(port.handle_button(SkirmishMapSelectAction::Escape));
        assert!(port.back_requested);
    }

    #[test]
    fn sort_by_name_reorders() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.set_sort_order(MapSortOrder::ByName);
        let names: Vec<&str> = port
            .map_select
            .maps
            .iter()
            .map(|m| m.display_name.as_str())
            .collect();
        assert_eq!(names[0], "Defcon 6 (6)");
        assert_eq!(names[1], "Tournament Desert (2)");
        assert_eq!(names[2], "Twilight Flame (8)");
    }

    #[test]
    fn sort_by_player_count_reorders() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.set_sort_order(MapSortOrder::ByPlayerCount);
        let counts: Vec<u8> = port
            .map_select
            .maps
            .iter()
            .map(|m| m.player_count)
            .collect();
        assert_eq!(counts[0], 2);
        assert_eq!(counts[1], 6);
        assert_eq!(counts[2], 8);
    }

    #[test]
    fn switch_to_system_maps() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        let system_maps = vec![MapEntryPort {
            name: "maps/official/a.map".to_string(),
            display_name: "Official A (4)".to_string(),
            player_count: 4,
            official: true,
        }];
        port.switch_to_system_maps(system_maps, "");
        assert!(port.official_maps_only);
        assert_eq!(port.map_select.maps.len(), 1);
    }

    #[test]
    fn switch_to_user_maps_appends_multiplayer() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        let solo = vec![MapEntryPort {
            name: "solo.map".to_string(),
            display_name: "Solo".to_string(),
            player_count: 1,
            official: false,
        }];
        let multi = vec![MapEntryPort {
            name: "multi.map".to_string(),
            display_name: "Multi".to_string(),
            player_count: 4,
            official: false,
        }];
        port.switch_to_user_maps(solo, multi, "");
        assert!(!port.official_maps_only);
        assert_eq!(port.map_select.maps.len(), 2);
    }

    #[test]
    fn update_metadata_from_cache() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.select_map(0);
        port.update_map_metadata_from_cache(|name| {
            if name.contains("tournament") {
                Some((42, 9999))
            } else {
                None
            }
        });
        assert_eq!(port.selected_map_crc, 42);
        assert_eq!(port.selected_map_size, 9999);
    }

    #[test]
    fn take_result_clears() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.select_map(0);
        port.confirm();
        let _ = port.take_result();
        assert!(port.confirmed_result.is_none());
    }

    #[test]
    fn selected_entry_returns_current() {
        let mut port = SkirmishMapSelectPort::init(sample_maps(), true, "");
        port.select_map(1);
        let entry = port.selected_entry().unwrap();
        assert_eq!(entry.player_count, 6);
    }
}
