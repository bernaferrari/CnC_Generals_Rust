// FILE: skirmish_map_select_menu.rs
// Author: Chris Brue, August 2002 (original C++), Rust port
// Description: Skirmish Map Select Menu
//
// Ported from: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishMapSelectMenu.cpp

use std::collections::HashMap;
use super::skirmish_game_options_menu::{MapMetaData, MAX_SLOTS, ICoord2D};

// Map difficulty levels for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapDifficulty {
    NoSuccess = 0,
    EasySuccess = 1,
    MediumSuccess = 2,
    HardSuccess = 3,
    MaxBrutalSuccess = 4,
}

impl MapDifficulty {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => MapDifficulty::NoSuccess,
            1 => MapDifficulty::EasySuccess,
            2 => MapDifficulty::MediumSuccess,
            3 => MapDifficulty::HardSuccess,
            4 => MapDifficulty::MaxBrutalSuccess,
            _ => MapDifficulty::NoSuccess,
        }
    }

    pub fn to_tooltip(&self) -> &'static str {
        match self {
            MapDifficulty::NoSuccess => "TOOLTIP:MapNoSuccess",
            MapDifficulty::EasySuccess => "TOOLTIP:MapEasySuccess",
            MapDifficulty::MediumSuccess => "TOOLTIP:MapMediumSuccess",
            MapDifficulty::HardSuccess => "TOOLTIP:MapHardSuccess",
            MapDifficulty::MaxBrutalSuccess => "TOOLTIP:MapMaxBrutalSuccess",
        }
    }
}

// Map list entry
#[derive(Debug, Clone)]
pub struct MapListEntry {
    pub display_name: String,
    pub filename: String,
    pub difficulty: MapDifficulty,
    pub is_official: bool,
    pub num_players: usize,
}

impl MapListEntry {
    pub fn new(display_name: String, filename: String, is_official: bool, num_players: usize) -> Self {
        MapListEntry {
            display_name,
            filename,
            difficulty: MapDifficulty::NoSuccess,
            is_official,
            num_players,
        }
    }

    pub fn set_difficulty(&mut self, difficulty: MapDifficulty) {
        self.difficulty = difficulty;
    }
}

// Map cache - stores all available maps
#[derive(Debug, Clone)]
pub struct MapCache {
    maps: HashMap<String, MapMetaData>,
}

impl MapCache {
    pub fn new() -> Self {
        MapCache {
            maps: HashMap::new(),
        }
    }

    pub fn update_cache(&mut self) {
        // Load map metadata from disk
        // This would scan the maps directory and load .map files
    }

    pub fn find_map(&self, name: &str) -> Option<&MapMetaData> {
        let lower_name = name.to_lowercase();
        self.maps.get(&lower_name)
    }

    pub fn add_map(&mut self, name: String, metadata: MapMetaData) {
        let lower_name = name.to_lowercase();
        self.maps.insert(lower_name, metadata);
    }

    pub fn get_official_maps(&self) -> Vec<(&str, &MapMetaData)> {
        self.maps.iter()
            .filter(|(_, mmd)| mmd.is_official)
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }

    pub fn get_user_maps(&self) -> Vec<(&str, &MapMetaData)> {
        self.maps.iter()
            .filter(|(_, mmd)| !mmd.is_official)
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &MapMetaData)> {
        self.maps.iter()
    }
}

impl Default for MapCache {
    fn default() -> Self {
        Self::new()
    }
}

// Map select menu state
pub struct SkirmishMapSelectMenu {
    selected_map: String,
    use_system_map_dir: bool,
    map_list: Vec<MapListEntry>,
    preview_positions: Vec<Option<ICoord2D>>,
}

impl SkirmishMapSelectMenu {
    pub fn new() -> Self {
        SkirmishMapSelectMenu {
            selected_map: String::new(),
            use_system_map_dir: true,
            map_list: Vec::new(),
            preview_positions: vec![None; MAX_SLOTS],
        }
    }

    // Initialize menu
    pub fn init(&mut self, current_map: &str, use_system_maps: bool) {
        self.use_system_map_dir = use_system_maps;
        self.selected_map = current_map.to_string();
    }

    // Shutdown menu
    pub fn shutdown(&mut self) {
        self.map_list.clear();
    }

    // Populate map list from cache
    pub fn populate_map_list(&mut self, cache: &MapCache, reset: bool) {
        if reset {
            self.map_list.clear();
        }

        let maps = if self.use_system_map_dir {
            cache.get_official_maps()
        } else {
            cache.get_user_maps()
        };

        for (name, mmd) in maps {
            let entry = MapListEntry::new(
                mmd.display_name.clone(),
                name.to_string(),
                mmd.is_official,
                mmd.num_players,
            );
            self.map_list.push(entry);
        }

        // Sort by display name
        self.map_list.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    }

    // Populate both system and user maps (without reset)
    pub fn populate_map_list_no_reset(&mut self, cache: &MapCache, official: bool, user: bool) {
        if official {
            let maps = cache.get_official_maps();
            for (name, mmd) in maps {
                if !self.map_list.iter().any(|e| e.filename == name) {
                    let entry = MapListEntry::new(
                        mmd.display_name.clone(),
                        name.to_string(),
                        mmd.is_official,
                        mmd.num_players,
                    );
                    self.map_list.push(entry);
                }
            }
        }

        if user {
            let maps = cache.get_user_maps();
            for (name, mmd) in maps {
                if !self.map_list.iter().any(|e| e.filename == name) {
                    let entry = MapListEntry::new(
                        mmd.display_name.clone(),
                        name.to_string(),
                        mmd.is_official,
                        mmd.num_players,
                    );
                    self.map_list.push(entry);
                }
            }
        }

        // Sort by display name
        self.map_list.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    }

    // Select a map
    pub fn select_map(&mut self, index: usize) -> Option<String> {
        if index < self.map_list.len() {
            self.selected_map = self.map_list[index].filename.clone();
            Some(self.selected_map.clone())
        } else {
            None
        }
    }

    // Get selected map
    pub fn get_selected_map(&self) -> &str {
        &self.selected_map
    }

    // Handle system/user maps radio button
    pub fn set_use_system_maps(&mut self, use_system: bool) {
        self.use_system_map_dir = use_system;
    }

    pub fn uses_system_maps(&self) -> bool {
        self.use_system_map_dir
    }

    // Get map list for display
    pub fn get_map_list(&self) -> &[MapListEntry] {
        &self.map_list
    }

    // Handle map list tooltip
    pub fn get_map_tooltip(&self, row: usize, col: usize) -> Option<String> {
        if col == 1 && row < self.map_list.len() {
            Some(self.map_list[row].difficulty.to_tooltip().to_string())
        } else {
            None
        }
    }

    // Handle double-click on map (same as OK button)
    pub fn handle_double_click(&mut self, row: usize) -> Option<String> {
        self.select_map(row)
    }

    // Position start spots on preview
    pub fn position_start_spots(&mut self, mmd: &MapMetaData, map_window_size: ICoord2D) {
        self.preview_positions = vec![None; MAX_SLOTS];

        if !mmd.is_multiplayer {
            return;
        }

        // Calculate display area for map
        let (ul, lr) = super::skirmish_game_options_menu::find_draw_positions(
            0, 0,
            map_window_size.x, map_window_size.y,
            (mmd.extent_lo_x, mmd.extent_lo_y),
            (mmd.extent_hi_x, mmd.extent_hi_y),
        );

        // Position each player start location
        for i in 0..mmd.num_players.min(MAX_SLOTS) {
            let waypoint_name = format!("Player_{}_Start", i + 1);
            if let Some(pos) = mmd.waypoints.get(&waypoint_name) {
                let gadget_size = ICoord2D { x: 32, y: 32 }; // Standard button size

                let screen_pos = super::skirmish_game_options_menu::position_start_spot_controls(
                    *pos,
                    mmd,
                    map_window_size,
                    gadget_size,
                    ul,
                    lr,
                );

                self.preview_positions[i] = Some(screen_pos);
            }
        }
    }

    // Get start spot positions for rendering
    pub fn get_start_spot_positions(&self) -> &[Option<ICoord2D>] {
        &self.preview_positions
    }

    // Show/hide underlying GUI elements
    pub fn show_underlying_elements(&self, show: bool) {
        // This would hide/show the main menu elements while map select is open
        // Implementation depends on GUI framework
    }
}

impl Default for SkirmishMapSelectMenu {
    fn default() -> Self {
        Self::new()
    }
}

// Helper function to get default map
pub fn get_default_map(is_skirmish: bool) -> String {
    if is_skirmish {
        "tournament_desert.map".to_string()
    } else {
        "usa01.map".to_string()
    }
}

// Helper function to validate map
pub fn is_valid_map(map_name: &str, is_skirmish: bool, cache: &MapCache) -> bool {
    if let Some(mmd) = cache.find_map(map_name) {
        if is_skirmish {
            mmd.is_multiplayer
        } else {
            true
        }
    } else {
        false
    }
}

// Get map preview image path
pub fn get_map_preview_image_path(map_name: &str) -> String {
    // Remove .map extension if present
    let base_name = if map_name.ends_with(".map") {
        &map_name[..map_name.len() - 4]
    } else {
        map_name
    };

    format!("Maps/{}.tga", base_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_difficulty_conversion() {
        assert_eq!(MapDifficulty::from_i32(0), MapDifficulty::NoSuccess);
        assert_eq!(MapDifficulty::from_i32(1), MapDifficulty::EasySuccess);
        assert_eq!(MapDifficulty::from_i32(4), MapDifficulty::MaxBrutalSuccess);
    }

    #[test]
    fn test_map_difficulty_tooltip() {
        assert_eq!(MapDifficulty::NoSuccess.to_tooltip(), "TOOLTIP:MapNoSuccess");
        assert_eq!(MapDifficulty::HardSuccess.to_tooltip(), "TOOLTIP:MapHardSuccess");
    }

    #[test]
    fn test_map_list_entry_creation() {
        let entry = MapListEntry::new(
            "Tournament Desert".to_string(),
            "tournament_desert.map".to_string(),
            true,
            8,
        );

        assert_eq!(entry.display_name, "Tournament Desert");
        assert_eq!(entry.filename, "tournament_desert.map");
        assert!(entry.is_official);
        assert_eq!(entry.num_players, 8);
    }

    #[test]
    fn test_map_cache_creation() {
        let cache = MapCache::new();
        assert!(cache.maps.is_empty());
    }

    #[test]
    fn test_map_cache_add_and_find() {
        let mut cache = MapCache::new();
        let mut mmd = MapMetaData::new();
        mmd.display_name = "Test Map".to_string();
        mmd.is_official = true;

        cache.add_map("test.map".to_string(), mmd);

        assert!(cache.find_map("test.map").is_some());
        assert!(cache.find_map("TEST.MAP").is_some()); // Case insensitive
        assert!(cache.find_map("nonexistent.map").is_none());
    }

    #[test]
    fn test_map_cache_official_filter() {
        let mut cache = MapCache::new();

        let mut mmd1 = MapMetaData::new();
        mmd1.is_official = true;
        cache.add_map("official.map".to_string(), mmd1);

        let mut mmd2 = MapMetaData::new();
        mmd2.is_official = false;
        cache.add_map("user.map".to_string(), mmd2);

        let official_maps = cache.get_official_maps();
        let user_maps = cache.get_user_maps();

        assert_eq!(official_maps.len(), 1);
        assert_eq!(user_maps.len(), 1);
    }

    #[test]
    fn test_skirmish_map_select_menu_creation() {
        let menu = SkirmishMapSelectMenu::new();
        assert_eq!(menu.get_selected_map(), "");
        assert!(menu.uses_system_maps());
    }

    #[test]
    fn test_map_selection() {
        let mut menu = SkirmishMapSelectMenu::new();
        menu.map_list.push(MapListEntry::new(
            "Map 1".to_string(),
            "map1.map".to_string(),
            true,
            4,
        ));
        menu.map_list.push(MapListEntry::new(
            "Map 2".to_string(),
            "map2.map".to_string(),
            true,
            8,
        ));

        let result = menu.select_map(1);
        assert_eq!(result, Some("map2.map".to_string()));
        assert_eq!(menu.get_selected_map(), "map2.map");
    }

    #[test]
    fn test_map_selection_out_of_bounds() {
        let mut menu = SkirmishMapSelectMenu::new();
        menu.map_list.push(MapListEntry::new(
            "Map 1".to_string(),
            "map1.map".to_string(),
            true,
            4,
        ));

        let result = menu.select_map(5);
        assert_eq!(result, None);
    }

    #[test]
    fn test_populate_map_list() {
        let mut cache = MapCache::new();
        let mut mmd = MapMetaData::new();
        mmd.display_name = "Official Map".to_string();
        mmd.is_official = true;
        cache.add_map("official.map".to_string(), mmd);

        let mut menu = SkirmishMapSelectMenu::new();
        menu.init("", true);
        menu.populate_map_list(&cache, true);

        assert_eq!(menu.get_map_list().len(), 1);
        assert_eq!(menu.get_map_list()[0].display_name, "Official Map");
    }

    #[test]
    fn test_populate_map_list_no_reset() {
        let mut cache = MapCache::new();

        let mut mmd1 = MapMetaData::new();
        mmd1.display_name = "Official Map".to_string();
        mmd1.is_official = true;
        cache.add_map("official.map".to_string(), mmd1);

        let mut mmd2 = MapMetaData::new();
        mmd2.display_name = "User Map".to_string();
        mmd2.is_official = false;
        cache.add_map("user.map".to_string(), mmd2);

        let mut menu = SkirmishMapSelectMenu::new();
        menu.init("", false);
        menu.populate_map_list_no_reset(&cache, true, true);

        assert_eq!(menu.get_map_list().len(), 2);
    }

    #[test]
    fn test_default_map() {
        let skirmish_map = get_default_map(true);
        let campaign_map = get_default_map(false);

        assert_eq!(skirmish_map, "tournament_desert.map");
        assert_eq!(campaign_map, "usa01.map");
    }

    #[test]
    fn test_is_valid_map() {
        let mut cache = MapCache::new();
        let mut mmd = MapMetaData::new();
        mmd.is_multiplayer = true;
        cache.add_map("valid.map".to_string(), mmd);

        assert!(is_valid_map("valid.map", true, &cache));
        assert!(!is_valid_map("invalid.map", true, &cache));
    }

    #[test]
    fn test_map_preview_image_path() {
        let path1 = get_map_preview_image_path("tournament_desert.map");
        assert_eq!(path1, "Maps/tournament_desert.tga");

        let path2 = get_map_preview_image_path("tournament_desert");
        assert_eq!(path2, "Maps/tournament_desert.tga");
    }

    #[test]
    fn test_system_user_maps_toggle() {
        let mut menu = SkirmishMapSelectMenu::new();
        assert!(menu.uses_system_maps());

        menu.set_use_system_maps(false);
        assert!(!menu.uses_system_maps());

        menu.set_use_system_maps(true);
        assert!(menu.uses_system_maps());
    }

    #[test]
    fn test_map_list_sorting() {
        let mut cache = MapCache::new();

        let mut mmd1 = MapMetaData::new();
        mmd1.display_name = "Zebra Map".to_string();
        mmd1.is_official = true;
        cache.add_map("zebra.map".to_string(), mmd1);

        let mut mmd2 = MapMetaData::new();
        mmd2.display_name = "Alpha Map".to_string();
        mmd2.is_official = true;
        cache.add_map("alpha.map".to_string(), mmd2);

        let mut menu = SkirmishMapSelectMenu::new();
        menu.init("", true);
        menu.populate_map_list(&cache, true);

        assert_eq!(menu.get_map_list()[0].display_name, "Alpha Map");
        assert_eq!(menu.get_map_list()[1].display_name, "Zebra Map");
    }

    #[test]
    fn test_handle_double_click() {
        let mut menu = SkirmishMapSelectMenu::new();
        menu.map_list.push(MapListEntry::new(
            "Test Map".to_string(),
            "test.map".to_string(),
            true,
            4,
        ));

        let result = menu.handle_double_click(0);
        assert_eq!(result, Some("test.map".to_string()));
        assert_eq!(menu.get_selected_map(), "test.map");
    }
}
