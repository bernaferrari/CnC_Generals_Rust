//! Player Template System - Placeholder implementation
//!
//! Defines player faction templates and their properties.

use crate::common::game_common::VeterancyLevel;
use crate::common::rts::{Money, NameKeyType};
use once_cell::sync::OnceCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Player template defining faction characteristics
#[derive(Debug, Clone)]
pub struct PlayerTemplate {
    pub name: String,
    pub display_name: String,
    pub side: String,
    pub base_side: String,
    pub playable: bool,
    pub is_observer: bool,
    pub old_faction: bool,
    pub starting_money: Money,
    pub preferred_color: u32,
    pub starting_building: String,
    pub starting_units: Vec<String>,
    pub intrinsic_sciences: Vec<String>,
    pub purchase_science_command_set_rank1: String,
    pub purchase_science_command_set_rank3: String,
    pub purchase_science_command_set_rank8: String,
    pub special_power_shortcut_command_set: String,
    pub special_power_shortcut_win_name: String,
    pub special_power_shortcut_button_count: i32,
    pub intrinsic_science_purchase_points: i32,
    pub score_screen_image: String,
    pub load_screen_image: String,
    pub load_screen_music: String,
    pub score_screen_music: String,
    pub head_water_mark: String,
    pub flag_water_mark: String,
    pub enabled_image: String,
    pub side_icon_image: String,
    pub general_image: String,
    pub beacon_name: String,
    pub army_tooltip: String,
    pub features: String,
    pub medallion_regular: String,
    pub medallion_hilite: String,
    pub medallion_select: String,
    pub production_cost_changes: std::collections::HashMap<NameKeyType, f32>,
    pub production_time_changes: std::collections::HashMap<NameKeyType, f32>,
    pub production_veterancy_levels: std::collections::HashMap<NameKeyType, VeterancyLevel>,
    pub player_allies: String,
    pub player_enemies: String,
}

impl PlayerTemplate {
    pub fn new(name: String) -> Self {
        Self {
            name,
            display_name: String::new(),
            side: String::new(),
            base_side: String::new(),
            playable: true,
            is_observer: false,
            old_faction: false,
            starting_money: Money::new(),
            preferred_color: 0,
            starting_building: String::new(),
            starting_units: vec![String::new(); 10],
            intrinsic_sciences: Vec::new(),
            purchase_science_command_set_rank1: String::new(),
            purchase_science_command_set_rank3: String::new(),
            purchase_science_command_set_rank8: String::new(),
            special_power_shortcut_command_set: String::new(),
            special_power_shortcut_win_name: String::new(),
            special_power_shortcut_button_count: 0,
            intrinsic_science_purchase_points: 0,
            score_screen_image: String::new(),
            load_screen_image: String::new(),
            load_screen_music: String::new(),
            score_screen_music: String::new(),
            head_water_mark: String::new(),
            flag_water_mark: String::new(),
            enabled_image: String::new(),
            side_icon_image: String::new(),
            general_image: String::new(),
            beacon_name: String::new(),
            army_tooltip: String::new(),
            features: String::new(),
            medallion_regular: String::new(),
            medallion_hilite: String::new(),
            medallion_select: String::new(),
            production_cost_changes: std::collections::HashMap::new(),
            production_time_changes: std::collections::HashMap::new(),
            production_veterancy_levels: std::collections::HashMap::new(),
            player_allies: String::new(),
            player_enemies: String::new(),
        }
    }

    pub fn get_display_name(&self) -> &str {
        if self.display_name.is_empty() {
            &self.name
        } else {
            &self.display_name
        }
    }

    /// Get the side/faction name
    pub fn get_side(&self) -> &str {
        &self.side
    }

    pub fn get_side_icon_image(&self) -> &str {
        &self.side_icon_image
    }

    pub fn is_playable_side(&self) -> bool {
        self.playable && self.side != "Boss"
    }
}

/// Player template store
#[derive(Debug)]
pub struct PlayerTemplateStore {
    templates: Vec<PlayerTemplate>,
}

impl PlayerTemplateStore {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    pub fn find_template(&self, name: &str) -> Option<&PlayerTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    pub fn get_nth_player_template(&self, index: usize) -> Option<&PlayerTemplate> {
        self.templates.get(index)
    }

    pub fn get_nth_player_template_mut(&mut self, index: usize) -> Option<&mut PlayerTemplate> {
        self.templates.get_mut(index)
    }

    pub fn add_template(&mut self, template: PlayerTemplate) {
        self.templates.push(template);
    }

    pub fn find_template_index(&self, name: &str) -> Option<usize> {
        self.templates
            .iter()
            .position(|template| template.name == name)
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, PlayerTemplate> {
        self.templates.iter()
    }

    pub fn clear(&mut self) {
        self.templates.clear();
    }
}

impl Default for PlayerTemplateStore {
    fn default() -> Self {
        Self::new()
    }
}

static PLAYER_TEMPLATE_STORE: OnceCell<RwLock<PlayerTemplateStore>> = OnceCell::new();

pub fn get_player_template_store() -> RwLockReadGuard<'static, PlayerTemplateStore> {
    PLAYER_TEMPLATE_STORE
        .get_or_init(|| RwLock::new(PlayerTemplateStore::new()))
        .read()
        .expect("PlayerTemplateStore poisoned")
}

pub fn get_player_template_store_mut() -> RwLockWriteGuard<'static, PlayerTemplateStore> {
    PLAYER_TEMPLATE_STORE
        .get_or_init(|| RwLock::new(PlayerTemplateStore::new()))
        .write()
        .expect("PlayerTemplateStore poisoned")
}
