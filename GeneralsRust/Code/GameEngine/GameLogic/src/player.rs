//! Player system - Complete Rust conversion of C++ Player class
//!
//! A "Player" is an entity that contains the persistent info of the Player, as well as containing
//! transient mission data. Some attributes persist between missions, whereas others are "transient" and only
//! have meaning in a mission, wherein they change a lot (current tech tree state, current buildings
//! built, units trained, money, etc).
//!
//! A "Player" consists of an entity controlling a single set of units in a mission.
//! A Player may be human or computer controlled.

use crate::ai::AIGroup;
use crate::build_list_info::BuildListInfo;
use crate::common::ThingTemplate;
use crate::common::*;
use crate::helpers::TheGameLogic;
use crate::modules::AIUpdateInterfaceExt;
use crate::object::behavior::battle_plan_update::BattlePlanBonuses;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::Object;
use crate::object_manager::get_object_manager;
use crate::special_power_module::integration::{FrameCount, PlayerInterface};
use crate::special_power_module::types::SpecialPowerID;
use crate::squad::Squad;
use crate::supply_system::ResourceGatheringManager;
use crate::team::{Team, TeamID, TeamPrototype, TeamRelationMap};
use crate::tunnel_tracker::TunnelTracker;
use crate::upgrade::{PlayerUpgradeManager, Upgrade, UpgradeTemplate};
use game_engine::common::global_data;
use game_engine::common::ini::ensure_player_templates_loaded;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::get_player_template_store;
use game_engine::common::rts::science::get_science_store;
use game_engine::common::rts::{Money, ScienceAccess, ScienceType, SCIENCE_INVALID};
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::{Xfer, XferMode, XferVersion};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Player index type (matching C++ PlayerIndex)
pub type PlayerIndex = Int;
pub const PLAYER_INDEX_INVALID: PlayerIndex = -1;

/// Money interface (matching C++ MoneyInterface usage).
pub trait MoneyInterface: Send + Sync {
    fn count_money(&self) -> i32;
}

/// Maximum number of hotkey squads
pub const NUM_HOTKEY_SQUADS: usize = 10;

/// Invalid hotkey squad constant
pub const NO_HOTKEY_SQUAD: PlayerIndex = -1;

/// Player types (matching C++ PlayerType: HUMAN=0, COMPUTER=1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayerType {
    Human = 0,
    Computer = 1,
    Observer = 2,
    Neutral = 3,
}

/// Game difficulty levels (matching C++ GameDifficulty: EASY=0, NORMAL=1, HARD=2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GameDifficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
    Brutal = 3,
}

/// Science availability types (matching C++ ScienceAvailabilityType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScienceAvailabilityType {
    Available,
    Disabled,
    Hidden,
}

/// Battle plan types (matching C++ battle plan system)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePlanType {
    Bombard,
    HoldTheLine,
    SearchAndDestroy,
}

/// Science vector type
pub type ScienceVec = Vec<ScienceType>;

/// Command source constant for AI commands (matching C++ CMD_FROM_AI)
pub const CMD_FROM_AI: CommandSourceType = CommandSourceType::FromAi;

/// Player money/resource management (matching C++ Money class)
#[derive(Debug, Clone)]
pub struct PlayerMoney {
    amount: Int,
    income_rate: Real,
    last_update_frame: UnsignedInt,
    player_index: PlayerIndex,
}

impl PlayerMoney {
    pub fn new(player_index: PlayerIndex) -> Self {
        Self {
            amount: 0,
            income_rate: 0.0,
            last_update_frame: 0,
            player_index,
        }
    }

    pub fn get_money(&self) -> Int {
        self.amount
    }

    pub fn add_money(&mut self, amount: Int) {
        if amount >= 0 {
            let _ = self.deposit(amount as u32);
        } else {
            let _ = self.withdraw((-amount) as u32);
        }
    }

    /// Set money to an exact amount (matching C++ Player::setMoney)
    pub fn set_money(&mut self, amount: Int) {
        self.amount = amount;
    }

    pub fn subtract_money(&mut self, amount: Int) -> bool {
        if amount <= 0 {
            return true;
        }
        if self.amount >= amount {
            let _ = self.withdraw(amount as u32);
            true
        } else {
            false
        }
    }

    pub fn can_afford(&self, cost: Int) -> bool {
        self.amount >= cost
    }

    pub fn set_income_rate(&mut self, rate: Real) {
        self.income_rate = rate;
    }

    pub fn get_income_rate(&self) -> Real {
        self.income_rate
    }

    pub fn set_player_index(&mut self, player_index: PlayerIndex) {
        self.player_index = player_index;
    }

    /// Returns the currently available cash (non-negative) as an unsigned amount.
    pub fn count_money(&self) -> u32 {
        self.amount.max(0) as u32
    }

    /// Withdraw money from the player's reserves.
    pub fn withdraw(&mut self, amount: u32) -> Result<u32, GameError> {
        self.withdraw_with_sound(amount, true)
    }

    /// Withdraw money from the player's reserves, optionally playing a sound.
    /// Matches C++ Money::withdraw(amount, playSound).
    pub fn withdraw_with_sound(&mut self, amount: u32, play_sound: bool) -> Result<u32, GameError> {
        let available = self.count_money();
        let actual = amount.min(available);
        if actual == 0 {
            return Ok(0);
        }

        if play_sound {
            if let Some(audio) = crate::helpers::TheAudio::get() {
                let event = crate::helpers::TheAudio::get_misc_audio()
                    .money_withdraw
                    .clone();
                let mut audio_event = crate::common::audio::AudioEventRts::new(event.sound_type);
                audio_event.set_player_index(self.player_index as u32);
                audio.add_audio_event(&audio_event);
            }
        }

        self.amount = self.amount.saturating_sub(actual as Int);
        Ok(actual)
    }

    /// Deposit money into the player's reserves.
    pub fn deposit(&mut self, amount: u32) -> Result<(), GameError> {
        self.deposit_with_sound(amount, true)
    }

    /// Deposit money into the player's reserves, optionally playing a sound.
    /// Matches C++ Money::deposit(amount, playSound).
    pub fn deposit_with_sound(&mut self, amount: u32, play_sound: bool) -> Result<(), GameError> {
        if amount == 0 {
            return Ok(());
        }

        if play_sound {
            if let Some(audio) = crate::helpers::TheAudio::get() {
                let event = crate::helpers::TheAudio::get_misc_audio()
                    .money_deposit
                    .clone();
                let mut audio_event = crate::common::audio::AudioEventRts::new(event.sound_type);
                audio_event.set_player_index(self.player_index as u32);
                audio.add_audio_event(&audio_event);
            }
        }

        self.amount = self.amount.saturating_add(amount as Int);
        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_index) {
                if let Ok(mut player_guard) = player.write() {
                    player_guard
                        .get_academy_stats_mut()
                        .record_income(amount as Int);
                }
            }
        }
        Ok(())
    }

    /// Deposit money from Int amount (alternative interface).
    pub fn deposit_money(&mut self, amount: Int) {
        self.amount = self.amount.saturating_add(amount);
    }

    /// Track money earned for statistics (currently just adds to total).
    pub fn add_money_earned(&mut self, amount: Int) {
        if amount <= 0 {
            return;
        }

        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_index) {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.score_keeper.add_money_earned(amount as u32);
                }
            }
        }
    }
}

impl MoneyInterface for PlayerMoney {
    fn count_money(&self) -> i32 {
        self.amount
    }
}

/// Player energy/power management (matching C++ Energy class)
#[derive(Debug, Clone)]
pub struct PlayerEnergy {
    production: Int,
    consumption: Int,
    power_sabotaged_till_frame: UnsignedInt,
}

impl PlayerEnergy {
    pub fn new() -> Self {
        Self {
            production: 0,
            consumption: 0,
            power_sabotaged_till_frame: 0,
        }
    }

    /// Reset energy bookkeeping to defaults (matches C++ Energy::init).
    pub fn reset(&mut self) {
        self.production = 0;
        self.consumption = 0;
    }

    pub fn get_power(&self) -> Int {
        self.production() - self.consumption
    }

    pub fn is_low_power(&self) -> bool {
        !self.has_sufficient_power()
    }

    pub fn production(&self) -> Int {
        if TheGameLogic::get_frame() < self.power_sabotaged_till_frame {
            0
        } else {
            self.production
        }
    }

    pub fn consumption(&self) -> Int {
        self.consumption
    }

    pub fn supply_ratio(&self) -> Real {
        if TheGameLogic::get_frame() < self.power_sabotaged_till_frame {
            return 0.0;
        }

        if self.consumption <= 0 {
            return self.production() as Real;
        }

        (self.production() as Real) / (self.consumption as Real)
    }

    pub fn add_power_production(&mut self, amount: Int) {
        self.production += amount;
        debug_assert!(
            self.production >= 0 && self.consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.production,
            self.consumption
        );
    }

    pub fn add_power_consumption(&mut self, amount: Int) {
        self.consumption += amount;
        debug_assert!(
            self.production >= 0 && self.consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.production,
            self.consumption
        );
    }

    /// Adjust power based on a delta and whether we're adding/removing (matches C++ Energy::adjustPower).
    pub fn adjust_power(&mut self, power_delta: Int, adding: Bool) {
        if power_delta == 0 {
            return;
        }

        if power_delta > 0 {
            if adding {
                self.add_power_production(power_delta);
            } else {
                self.add_power_production(-power_delta);
            }
        } else if adding {
            self.add_power_consumption(-power_delta);
        } else {
            self.add_power_consumption(power_delta);
        }
    }

    /// Register a newly influenced object to adjust production/consumption (matches C++ Energy::objectEnteringInfluence).
    pub fn object_entering_influence(&mut self, obj: &Object) {
        let energy = obj.get_template().get_energy_production();
        if energy < 0 {
            self.add_power_consumption(-energy);
        } else if energy > 0 {
            self.add_power_production(energy);
        }
    }

    /// Remove influence from an object (matches C++ Energy::objectLeavingInfluence).
    pub fn object_leaving_influence(&mut self, obj: &Object) {
        let energy = obj.get_template().get_energy_production();
        if energy < 0 {
            self.add_power_consumption(energy);
        } else if energy > 0 {
            self.add_power_production(-energy);
        }
    }

    pub fn add_power_bonus(&mut self, obj: ObjectID) {
        if let Some(object) = crate::object::registry::OBJECT_REGISTRY.get_object(obj) {
            if let Ok(object_guard) = object.read() {
                let bonus = object_guard.get_template().get_energy_bonus();
                if bonus != 0 {
                    self.add_power_production(bonus);
                }
            }
        }
        self.touch();
    }

    pub fn remove_power_bonus(&mut self, obj: ObjectID) {
        if let Some(object) = crate::object::registry::OBJECT_REGISTRY.get_object(obj) {
            if let Ok(object_guard) = object.read() {
                let bonus = object_guard.get_template().get_energy_bonus();
                if bonus != 0 {
                    self.add_power_production(-bonus);
                }
            }
        }
    }

    pub fn touch(&mut self) {}

    /// Set sabotage timer for the player's power supply
    /// Matches C++ Energy::setPowerSabotagedTillFrame
    pub fn set_power_sabotaged_till_frame(&mut self, frame: UnsignedInt) {
        self.power_sabotaged_till_frame = frame;
    }

    pub fn get_power_sabotaged_till_frame(&self) -> UnsignedInt {
        self.power_sabotaged_till_frame
    }

    pub fn is_power_sabotaged(&self) -> bool {
        TheGameLogic::get_frame() < self.power_sabotaged_till_frame
    }

    pub fn has_sufficient_power(&self) -> bool {
        if self.is_power_sabotaged() {
            false
        } else {
            self.production >= self.consumption
        }
    }
}

/// Resource snapshot exposed to high-level managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerResources {
    pub supplies: Int,
    pub power_available: Int,
    pub power_used: Int,
}

/// Player handicap system (matching C++ Handicap class)
#[derive(Debug, Clone)]
pub struct PlayerHandicap {
    damage_multiplier: Real,
    cost_multiplier: Real,
    build_time_multiplier: Real,
    vision_multiplier: Real,
    build_cost_generic: Real,
    build_cost_buildings: Real,
    build_time_generic: Real,
    build_time_buildings: Real,
}

impl PlayerHandicap {
    pub fn new() -> Self {
        Self {
            damage_multiplier: 1.0,
            cost_multiplier: 1.0,
            build_time_multiplier: 1.0,
            vision_multiplier: 1.0,
            build_cost_generic: 1.0,
            build_cost_buildings: 1.0,
            build_time_generic: 1.0,
            build_time_buildings: 1.0,
        }
    }

    pub fn get_damage_multiplier(&self) -> Real {
        self.damage_multiplier
    }

    pub fn get_cost_multiplier(&self) -> Real {
        self.cost_multiplier
    }

    pub fn get_build_time_multiplier(&self) -> Real {
        self.build_time_multiplier
    }

    pub fn get_vision_multiplier(&self) -> Real {
        self.vision_multiplier
    }

    pub fn set_all(&mut self, value: Real) {
        let value = value.max(0.0);
        self.damage_multiplier = value;
        self.cost_multiplier = value;
        self.build_time_multiplier = value;
        self.vision_multiplier = value;
        self.build_cost_generic = value;
        self.build_cost_buildings = value;
        self.build_time_generic = value;
        self.build_time_buildings = value;
    }

    pub fn read_from_dict(&mut self, dict: &crate::common::Dict) {
        let keys = [
            ("HANDICAP_BUILDCOST_GENERIC", true, true),
            ("HANDICAP_BUILDCOST_BUILDINGS", true, false),
            ("HANDICAP_BUILDTIME_GENERIC", false, true),
            ("HANDICAP_BUILDTIME_BUILDINGS", false, false),
        ];

        for (name, is_cost, is_generic) in keys {
            let key = NameKeyGenerator::name_to_key(name);
            if dict.get_type(key).is_some() {
                let value = dict.get_real(key);
                if is_cost {
                    if is_generic {
                        self.build_cost_generic = value;
                    } else {
                        self.build_cost_buildings = value;
                    }
                } else if is_generic {
                    self.build_time_generic = value;
                } else {
                    self.build_time_buildings = value;
                }
            }
        }

        self.cost_multiplier = self.build_cost_generic;
        self.build_time_multiplier = self.build_time_generic;
    }

    pub fn get_cost_multiplier_for_template(&self, template: &dyn ThingTemplate) -> Real {
        if template.is_kind_of(KindOf::Structure) {
            self.build_cost_buildings
        } else {
            self.build_cost_generic
        }
    }

    pub fn get_build_time_multiplier_for_template(&self, template: &dyn ThingTemplate) -> Real {
        if template.is_kind_of(KindOf::Structure) {
            self.build_time_buildings
        } else {
            self.build_time_generic
        }
    }
}

/// Academy statistics tracking (matching C++ AcademyStats)
#[derive(Debug, Clone)]
pub struct AcademyStats {
    units_built: HashMap<String, Int>,
    units_killed: HashMap<String, Int>,
    buildings_built: HashMap<String, Int>,
    buildings_destroyed: HashMap<String, Int>,
    /// Track total units that have entered tunnel network
    tunnel_entries: Int,
    /// Total generals points spent on sciences.
    generals_points_spent: Int,
    researched_radar: Bool,
    upgrades_purchased: Int,
    cleared_garrisoned_buildings: Int,
    salvage_collected: Int,
    special_powers_used: Int,
    /// Total money earned (for scoreboard / academy stats).
    total_income: Int,
}

impl AcademyStats {
    pub fn new() -> Self {
        Self {
            units_built: HashMap::new(),
            units_killed: HashMap::new(),
            buildings_built: HashMap::new(),
            buildings_destroyed: HashMap::new(),
            tunnel_entries: 0,
            generals_points_spent: 0,
            researched_radar: false,
            upgrades_purchased: 0,
            cleared_garrisoned_buildings: 0,
            salvage_collected: 0,
            special_powers_used: 0,
            total_income: 0,
        }
    }

    /// Record that a unit entered the tunnel network
    /// Matches C++ AcademyStats::recordUnitEnteredTunnelNetwork
    pub fn record_unit_entered_tunnel_network(&mut self) {
        self.tunnel_entries += 1;
    }

    /// Get total tunnel entries for statistics/achievements
    pub fn get_tunnel_entries(&self) -> Int {
        self.tunnel_entries
    }

    /// Record generals points spent (matches C++ AcademyStats::recordGeneralsPointsSpent).
    pub fn record_generals_points_spent(&mut self, cost: Int) {
        if cost > 0 {
            self.generals_points_spent = self.generals_points_spent.saturating_add(cost);
        }
    }

    /// Get total generals points spent.
    pub fn get_generals_points_spent(&self) -> Int {
        self.generals_points_spent
    }

    /// Record unit built with type tracking
    pub fn record_unit_built(&mut self, unit_type: &str) {
        *self.units_built.entry(unit_type.to_string()).or_insert(0) += 1;
    }

    /// Record unit killed with type tracking
    pub fn record_unit_killed(&mut self, unit_type: &str) {
        *self.units_killed.entry(unit_type.to_string()).or_insert(0) += 1;
    }

    /// Record building built with type tracking
    pub fn record_building_built(&mut self, building_type: &str) {
        *self
            .buildings_built
            .entry(building_type.to_string())
            .or_insert(0) += 1;
    }

    /// Record building destroyed with type tracking
    pub fn record_building_destroyed(&mut self, building_type: &str) {
        *self
            .buildings_destroyed
            .entry(building_type.to_string())
            .or_insert(0) += 1;
    }

    /// Record income earned (for scoreboard). Matches C++ AcademyStats::recordIncome.
    pub fn record_income(&mut self, amount: Int) {
        self.total_income = self.total_income.saturating_add(amount);
    }

    /// Record upgrade acquisition (matches C++ AcademyStats::recordUpgrade)
    pub fn record_upgrade(&mut self, upgrade: &UpgradeTemplate, granted: Bool) {
        if upgrade.get_academy_classification() == 1 {
            self.researched_radar = true;
        }

        if !granted {
            self.upgrades_purchased += 1;
        }
    }

    /// Record clearing a garrisoned building (matches C++ AcademyStats::recordClearedGarrisonedBuilding).
    pub fn record_cleared_garrisoned_building(&mut self) {
        self.cleared_garrisoned_buildings += 1;
    }

    /// Record collecting a salvage crate (matches C++ AcademyStats::recordSalvageCollected).
    pub fn record_salvage_collected(&mut self) {
        self.salvage_collected += 1;
    }

    pub fn get_salvage_collected(&self) -> Int {
        self.salvage_collected
    }

    /// Record special power use (matches C++ AcademyStats::recordSpecialPowerUsed).
    pub fn record_special_power_used(
        &mut self,
        _classification: game_engine::common::rts::academy_stats::AcademyClassificationType,
    ) {
        self.special_powers_used = self.special_powers_used.saturating_add(1);
    }
}

/// Score keeping system (matching C++ ScoreKeeper)
#[derive(Debug, Clone)]
pub struct ScoreKeeper {
    units_built: Int,
    units_killed: Int,
    units_lost: Int,
    buildings_built: Int,
    buildings_destroyed: Int,
    buildings_lost: Int,
    supplies_collected: Int,
    supplies_spent: Int,
    experience_points: Int,
}

impl ScoreKeeper {
    pub fn new() -> Self {
        Self {
            units_built: 0,
            units_killed: 0,
            units_lost: 0,
            buildings_built: 0,
            buildings_destroyed: 0,
            buildings_lost: 0,
            supplies_collected: 0,
            supplies_spent: 0,
            experience_points: 0,
        }
    }

    pub fn add_unit_built(&mut self) {
        self.units_built += 1;
    }

    pub fn add_unit_killed(&mut self) {
        self.units_killed += 1;
    }

    pub fn add_unit_lost(&mut self) {
        self.units_lost += 1;
    }

    pub fn get_total_score(&self) -> Int {
        self.units_built * 10 + self.units_killed * 20 + self.buildings_built * 50
    }

    pub fn add_money_earned(&mut self, amount: u32) {
        self.supplies_collected = self.supplies_collected.saturating_add(amount as Int);
    }

    pub fn add_money_spent(&mut self, amount: u32) {
        self.supplies_spent = self.supplies_spent.saturating_add(amount as Int);
    }

    pub fn get_total_units_built(&self) -> Int {
        self.units_built
    }

    pub fn get_total_units_destroyed(&self) -> Int {
        self.units_killed
    }

    pub fn get_total_units_lost(&self) -> Int {
        self.units_lost
    }

    pub fn get_total_buildings_built(&self) -> Int {
        self.buildings_built
    }

    pub fn get_total_buildings_destroyed(&self) -> Int {
        self.buildings_destroyed
    }

    pub fn get_total_buildings_lost(&self) -> Int {
        self.buildings_lost
    }

    pub fn get_total_money_earned(&self) -> Int {
        self.supplies_collected
    }

    pub fn get_total_money_spent(&self) -> Int {
        self.supplies_spent
    }

    pub fn get_units_lost(&self) -> Int {
        self.units_lost
    }

    pub fn get_buildings_destroyed(&self) -> Int {
        self.buildings_destroyed
    }

    pub fn get_buildings_lost(&self) -> Int {
        self.buildings_lost
    }

    pub fn add_building_destroyed(&mut self) {
        self.buildings_destroyed += 1;
    }

    pub fn add_building_built(&mut self) {
        self.buildings_built += 1;
    }

    // Trait-based methods for ScoreableObject integration
    // These allow Object to pass itself directly to ScoreKeeper

    /// Add an object that was lost by this player.
    /// Convenience method that extracts information from the object.
    /// C++ Reference: ScoreKeeper::addObjectLost(const Object* o)
    pub fn add_object_lost_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        // Check if under construction - under construction objects don't count
        if object.is_score_under_construction() {
            return;
        }

        // Check the KindOf mask to determine if it's a unit or building
        let mask = object.get_score_kindof_mask();
        use game_engine::common::rts::score_keeper::KindOf;

        if mask.is_set(KindOf::Structure) {
            self.buildings_lost += 1;
        } else if mask.is_set(KindOf::Infantry) || mask.is_set(KindOf::Vehicle) {
            self.units_lost += 1;
        }
    }

    /// Add an object that was destroyed by this player.
    /// Convenience method that extracts information from the object.
    /// C++ Reference: ScoreKeeper::addObjectDestroyed(const Object* o)
    pub fn add_object_destroyed_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        // Check if under construction - under construction objects don't count
        if object.is_score_under_construction() {
            return;
        }

        // Check the KindOf mask to determine if it's a unit or building
        let mask = object.get_score_kindof_mask();
        use game_engine::common::rts::score_keeper::KindOf;

        if mask.is_set(KindOf::Structure) {
            self.buildings_destroyed += 1;
        } else if mask.is_set(KindOf::Infantry) || mask.is_set(KindOf::Vehicle) {
            self.units_killed += 1;
        }
    }
}

/// Player relation map (matching C++ PlayerRelationMap)
pub type PlayerRelationMapType = HashMap<PlayerIndex, Relationship>;

#[derive(Debug, Clone)]
pub struct PlayerRelationMap {
    pub map: PlayerRelationMapType,
}

impl PlayerRelationMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

/// Special power ready timer (matching C++ SpecialPowerReadyTimerType)
#[derive(Debug, Clone)]
pub struct SpecialPowerReadyTimer {
    template_id: UnsignedInt,
    ready_frame: UnsignedInt,
}

impl SpecialPowerReadyTimer {
    pub fn new() -> Self {
        Self {
            template_id: INVALID_ID,
            ready_frame: 0xffffffff,
        }
    }

    pub fn clear(&mut self) {
        self.ready_frame = 0xffffffff;
        self.template_id = INVALID_ID;
    }
}

/// Player template interface
#[derive(Debug, Clone)]
pub struct PlayerTemplate {
    pub name: String,
    pub side: String,
    pub base_side: String,
    pub display_name: String,
    pub playable: bool,
    pub is_observer: bool,
    pub old_faction: bool,
    pub starting_money: Money,
    pub preferred_color: u32,
    pub starting_building: String,
    pub starting_units: Vec<String>,
    pub score_screen_image: String,
    pub score_screen_music: String,
    pub load_screen_image: String,
    pub load_screen_music: String,
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
    pub purchase_science_command_set_rank1: String,
    pub purchase_science_command_set_rank3: String,
    pub purchase_science_command_set_rank8: String,
    pub special_power_shortcut_command_set: String,
    pub special_power_shortcut_win_name: String,
    pub special_power_shortcut_button_count: Int,
    pub player_allies: String,
    pub player_enemies: String,
    intrinsic_sciences: ScienceVec,
    intrinsic_science_purchase_points: Int,
    production_cost_changes: HashMap<NameKeyType, Real>,
    production_time_changes: HashMap<NameKeyType, Real>,
    production_veterancy_levels: HashMap<NameKeyType, VeterancyLevel>,
}

impl PlayerTemplate {
    pub fn new(name: String) -> Self {
        Self {
            name,
            side: String::new(),
            base_side: String::new(),
            display_name: String::new(),
            playable: true,
            is_observer: false,
            old_faction: false,
            starting_money: Money::new(),
            preferred_color: 0,
            starting_building: String::new(),
            starting_units: vec![String::new(); 10],
            score_screen_image: String::new(),
            score_screen_music: String::new(),
            load_screen_image: String::new(),
            load_screen_music: String::new(),
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
            purchase_science_command_set_rank1: String::new(),
            purchase_science_command_set_rank3: String::new(),
            purchase_science_command_set_rank8: String::new(),
            special_power_shortcut_command_set: String::new(),
            special_power_shortcut_win_name: String::new(),
            special_power_shortcut_button_count: 0,
            player_allies: String::new(),
            player_enemies: String::new(),
            intrinsic_sciences: Vec::new(),
            intrinsic_science_purchase_points: 0,
            production_cost_changes: HashMap::new(),
            production_time_changes: HashMap::new(),
            production_veterancy_levels: HashMap::new(),
        }
    }

    pub fn from_common(
        template: &game_engine::common::rts::player_template::PlayerTemplate,
    ) -> Self {
        let mut result = Self::new(template.name.clone());
        result.apply_common(template);
        result
    }

    pub fn apply_common(
        &mut self,
        template: &game_engine::common::rts::player_template::PlayerTemplate,
    ) {
        self.name = template.name.clone();
        self.side = template.side.clone();
        self.base_side = template.base_side.clone();
        self.display_name = template.display_name.clone();
        self.playable = template.playable;
        self.is_observer = template.is_observer;
        self.old_faction = template.old_faction;
        self.starting_money = template.starting_money.clone();
        self.preferred_color = template.preferred_color;
        self.starting_building = template.starting_building.clone();
        self.starting_units = template.starting_units.clone();
        self.score_screen_image = template.score_screen_image.clone();
        self.score_screen_music = template.score_screen_music.clone();
        self.load_screen_image = template.load_screen_image.clone();
        self.load_screen_music = template.load_screen_music.clone();
        self.head_water_mark = template.head_water_mark.clone();
        self.flag_water_mark = template.flag_water_mark.clone();
        self.enabled_image = template.enabled_image.clone();
        self.side_icon_image = template.side_icon_image.clone();
        self.general_image = template.general_image.clone();
        self.beacon_name = template.beacon_name.clone();
        self.army_tooltip = template.army_tooltip.clone();
        self.features = template.features.clone();
        self.medallion_regular = template.medallion_regular.clone();
        self.medallion_hilite = template.medallion_hilite.clone();
        self.medallion_select = template.medallion_select.clone();
        self.purchase_science_command_set_rank1 =
            template.purchase_science_command_set_rank1.clone();
        self.purchase_science_command_set_rank3 =
            template.purchase_science_command_set_rank3.clone();
        self.purchase_science_command_set_rank8 =
            template.purchase_science_command_set_rank8.clone();
        self.special_power_shortcut_command_set =
            template.special_power_shortcut_command_set.clone();
        self.special_power_shortcut_win_name = template.special_power_shortcut_win_name.clone();
        self.special_power_shortcut_button_count = template.special_power_shortcut_button_count;
        self.player_allies = template.player_allies.clone();
        self.player_enemies = template.player_enemies.clone();
        self.intrinsic_science_purchase_points = template.intrinsic_science_purchase_points;

        self.production_cost_changes = template.production_cost_changes.clone();
        self.production_time_changes = template.production_time_changes.clone();
        self.production_veterancy_levels = template
            .production_veterancy_levels
            .iter()
            .map(|(name_key, level)| {
                let mapped = match level {
                    game_engine::common::game_common::VeterancyLevel::Regular => {
                        crate::common::VeterancyLevel::Regular
                    }
                    game_engine::common::game_common::VeterancyLevel::Veteran => {
                        crate::common::VeterancyLevel::Veteran
                    }
                    game_engine::common::game_common::VeterancyLevel::Elite => {
                        crate::common::VeterancyLevel::Elite
                    }
                    game_engine::common::game_common::VeterancyLevel::Heroic => {
                        crate::common::VeterancyLevel::Heroic
                    }
                };
                (*name_key, mapped)
            })
            .collect();

        self.intrinsic_sciences.clear();
        if let Some(store) = get_science_store() {
            for name in &template.intrinsic_sciences {
                let science = store.get_science_from_internal_name(name);
                if science != SCIENCE_INVALID {
                    self.intrinsic_sciences.push(science);
                }
            }
        }
    }

    pub fn hydrate_from_common_store(&mut self) {
        ensure_player_templates_loaded();
        let store = get_player_template_store();
        if let Some(found) = store.find_template(&self.name) {
            self.apply_common(found);
        } else if !self.name.is_empty() {
            log::warn!(
                "PlayerTemplate '{}' not found in store (map may be obsolete)",
                self.name
            );
        }
    }

    /// Get the side/faction name
    pub fn get_side(&self) -> &str {
        &self.side
    }

    pub fn get_intrinsic_sciences(&self) -> &ScienceVec {
        &self.intrinsic_sciences
    }

    pub fn get_intrinsic_science_purchase_points(&self) -> Int {
        self.intrinsic_science_purchase_points
    }

    pub fn get_score_screen(&self) -> &str {
        &self.score_screen_image
    }

    pub fn get_score_screen_music(&self) -> &str {
        &self.score_screen_music
    }

    pub fn get_side_icon_image(&self) -> &str {
        &self.side_icon_image
    }

    pub fn production_cost_changes(&self) -> &HashMap<NameKeyType, Real> {
        &self.production_cost_changes
    }

    pub fn production_time_changes(&self) -> &HashMap<NameKeyType, Real> {
        &self.production_time_changes
    }

    pub fn production_veterancy_levels(&self) -> &HashMap<NameKeyType, VeterancyLevel> {
        &self.production_veterancy_levels
    }
}

#[derive(Debug, Clone)]
struct KindOfPercentProductionChange {
    kind_of: KindOfMaskType,
    percent: Real,
    refs: u32,
}

/// Complete Player class (matching C++ Player)
#[derive(Debug)]
pub struct Player {
    // Core identity
    player_index: PlayerIndex,
    player_name_key: NameKeyType,
    player_display_name: String,
    player_template: Option<Arc<PlayerTemplate>>,

    // Gameplay properties
    player_type: PlayerType,
    side: String,
    base_side: String,
    color: Color,
    night_color: Color,
    difficulty: GameDifficulty,

    // Resources and economy
    money: PlayerMoney,
    energy: PlayerEnergy,
    handicap: PlayerHandicap,

    // Research and upgrades
    sciences: ScienceVec,
    sciences_disabled: ScienceVec,
    sciences_hidden: ScienceVec,
    upgrade_list: Vec<Upgrade>,
    upgrades_in_progress: UpgradeMaskType,
    upgrades_completed: UpgradeMaskType,

    // Experience and ranking
    rank_level: Int,
    skill_points: Int,
    science_purchase_points: Int,
    skill_points_modifier: Real,
    general_name: String,

    // Team and relationships
    default_team: Option<Arc<RwLock<Team>>>,
    player_team_prototypes: Vec<Arc<TeamPrototype>>,
    player_relations: PlayerRelationMap,
    team_relations: Option<TeamRelationMap>,

    // Production cost modifiers
    kind_of_percent_production_change_list: Vec<KindOfPercentProductionChange>,

    // Radar and intelligence
    radar_count: Int,
    disable_proof_radar_count: Int,
    radar_disabled: Bool,

    // Battle plans and bonuses
    bombard_battle_plans: Int,
    hold_the_line_battle_plans: Int,
    search_and_destroy_battle_plans: Int,
    battle_plan_bonuses: Option<BattlePlanBonuses>,

    // Special powers
    special_power_ready_timers: RwLock<Vec<SpecialPowerReadyTimer>>,

    // Statistics and tracking
    academy_stats: AcademyStats,
    score_keeper: ScoreKeeper,

    // Control and AI
    can_build_units: Bool,
    can_build_base: Bool,
    is_observer: Bool,
    is_preorder: Bool,
    is_player_dead: Bool,
    list_in_score_screen: Bool,
    units_should_hunt: Bool,
    attacked_by: [Bool; MAX_PLAYER_COUNT],
    attacked_frame: UnsignedInt,

    // Multiplayer
    mp_start_index: Int,

    // Special properties
    cash_bounty_percent: Real,

    // Hotkey squads
    squads: [Option<Squad>; NUM_HOTKEY_SQUADS],
    current_selection: Option<Squad>,

    // Cheats and debug
    #[cfg(any(debug_assertions, feature = "internal"))]
    demo_ignore_prereqs: Bool,
    #[cfg(any(debug_assertions, feature = "internal"))]
    demo_free_build: Bool,
    #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
    demo_instant_build: Bool,

    // Retaliation mode
    logical_retaliation_mode_enabled: Bool,

    // Tunnel network system (for GLA faction)
    tunnel_tracker: Option<TunnelTracker>,
    resource_manager: Option<ResourceGatheringManager>,

    // Player upgrade manager
    upgrade_manager: PlayerUpgradeManager,

    // Objects owned by this player
    owned_objects: Vec<ObjectID>,

    // AI build list (skirmish plans)
    build_list: Option<Box<BuildListInfo>>,

    // Skirmish AI tracking
    is_skirmish_ai: Bool,
    current_enemy_player_index: Option<PlayerIndex>,
}

impl Player {
    /// Create a new player with the given index
    pub fn new(player_index: PlayerIndex) -> Self {
        Self {
            player_index,
            player_name_key: 0,
            player_display_name: String::new(),
            player_template: None,

            player_type: PlayerType::Human,
            side: String::new(),
            base_side: String::new(),
            color: Color::default(),
            night_color: Color::default(),
            difficulty: GameDifficulty::Normal,

            money: PlayerMoney::new(player_index),
            energy: PlayerEnergy::new(),
            handicap: PlayerHandicap::new(),

            sciences: Vec::new(),
            sciences_disabled: Vec::new(),
            sciences_hidden: Vec::new(),
            upgrade_list: Vec::new(),
            upgrades_in_progress: UpgradeMaskType::none(),
            upgrades_completed: UpgradeMaskType::none(),

            rank_level: 1,
            skill_points: 0,
            science_purchase_points: 0,
            skill_points_modifier: 1.0,
            general_name: String::new(),

            default_team: None,
            player_team_prototypes: Vec::new(),
            player_relations: PlayerRelationMap::new(),
            team_relations: None,

            kind_of_percent_production_change_list: Vec::new(),

            radar_count: 0,
            disable_proof_radar_count: 0,
            radar_disabled: false,

            bombard_battle_plans: 0,
            hold_the_line_battle_plans: 0,
            search_and_destroy_battle_plans: 0,
            battle_plan_bonuses: None,

            special_power_ready_timers: RwLock::new(Vec::new()),

            academy_stats: AcademyStats::new(),
            score_keeper: ScoreKeeper::new(),

            can_build_units: true,
            can_build_base: true,
            is_observer: false,
            is_preorder: false,
            is_player_dead: false,
            list_in_score_screen: true,
            units_should_hunt: false,
            attacked_by: [false; MAX_PLAYER_COUNT],
            attacked_frame: 0,

            mp_start_index: 0,

            cash_bounty_percent: 0.0,

            squads: Default::default(),
            current_selection: None,

            #[cfg(any(debug_assertions, feature = "internal"))]
            demo_ignore_prereqs: false,
            #[cfg(any(debug_assertions, feature = "internal"))]
            demo_free_build: false,
            #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
            demo_instant_build: false,

            logical_retaliation_mode_enabled: false,

            tunnel_tracker: None,
            resource_manager: None,

            upgrade_manager: PlayerUpgradeManager::new(player_index as u32),

            owned_objects: Vec::new(),
            build_list: None,

            is_skirmish_ai: false,
            current_enemy_player_index: None,
        }
    }

    /// Get the player ID (player index)
    pub fn get_id(&self) -> PlayerIndex {
        self.player_index
    }

    pub fn get_build_list(&self) -> Option<&BuildListInfo> {
        self.build_list.as_deref()
    }

    pub fn get_build_list_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.build_list.as_deref_mut()
    }

    pub fn set_build_list(&mut self, build_list: Option<BuildListInfo>) {
        self.build_list = build_list.map(Box::new);
    }

    pub fn is_skirmish_ai(&self) -> Bool {
        self.is_skirmish_ai
    }

    pub fn set_is_skirmish_ai(&mut self, value: Bool) {
        self.is_skirmish_ai = value;
    }

    pub fn get_current_enemy_player_index(&self) -> Option<PlayerIndex> {
        self.current_enemy_player_index
    }

    pub fn set_current_enemy_player_index(&mut self, index: Option<PlayerIndex>) {
        self.current_enemy_player_index = index;
    }

    /// Get all objects owned by this player
    /// Matches C++ Player::getObjectList
    pub fn get_all_objects(&self) -> Vec<crate::common::ObjectID> {
        self.owned_objects.clone()
    }

    /// Count objects by thing template, matching C++ Player::countObjectsByThingTemplate.
    pub fn count_objects_by_thing_template(
        &self,
        templates: &[Arc<dyn ThingTemplate>],
        ignore_dead: Bool,
        ignore_under_construction: Bool,
        counts: &mut [Int],
    ) {
        counts.fill(0);
        let max_templates = templates.len().min(counts.len());
        if max_templates == 0 {
            return;
        }

        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };

            if ignore_dead && object_guard.is_effectively_dead() {
                continue;
            }
            if ignore_under_construction
                && object_guard.test_status(ObjectStatusTypes::UnderConstruction)
            {
                continue;
            }

            let obj_template = object_guard.get_template();
            for i in 0..max_templates {
                if !obj_template.is_equivalent_to(templates[i].as_ref()) {
                    continue;
                }
                counts[i] += 1;
                break;
            }
        }
    }

    /// Count player-owned structures, matching C++ Player::countBuildings.
    pub fn count_buildings(&self) -> Int {
        let mut count = 0;
        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.get_template().is_kind_of(KindOf::Structure) {
                count += 1;
            }
        }
        count
    }

    /// Count player-owned objects by KindOf masks, matching C++ Player::countObjects.
    pub fn count_objects_by_kindof(
        &self,
        required: KindOfMaskType,
        forbidden: KindOfMaskType,
    ) -> Int {
        let mut count = 0;
        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.is_kind_of_multi(required, forbidden) {
                count += 1;
            }
        }
        count
    }

    /// Add an object to this player's ownership
    /// Matches C++ Player::addObject
    pub fn add_owned_object(&mut self, object_id: ObjectID) {
        if !self.owned_objects.contains(&object_id) {
            self.owned_objects.push(object_id);
        }

        let Some(object) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(object_guard) = object.read() else {
            return;
        };

        if !object_guard.test_status(ObjectStatusTypes::UnderConstruction) {
            let power = object_guard.get_template().get_energy_production();
            if power > 0 {
                if !object_guard.is_disabled() {
                    self.add_power_production(power);
                }
            } else if power < 0 {
                self.add_power_consumption(-power);
            }
        }

        if object_guard.is_kind_of(crate::common::KindOf::Dozer) {
            if let Some(ai) = object_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if ai_guard.is_idle() {
                        crate::helpers::TheInGameUI::add_idle_worker(
                            &*object_guard,
                            self.player_index,
                        );
                    }
                }
            }
        }
    }

    /// Find a drone owned by this player that was produced by the given object ID.
    pub fn find_drone_by_producer_id(
        &self,
        producer_id: ObjectID,
    ) -> Result<Option<Arc<RwLock<Object>>>, String> {
        for object_id in &self.owned_objects {
            let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(*object_id) else {
                continue;
            };
            let matches = {
                let Ok(obj_ref) = obj.read() else {
                    continue;
                };
                obj_ref.get_producer_id() == producer_id
                    && obj_ref.is_kind_of(crate::common::KindOf::Drone)
            };
            if matches {
                return Ok(Some(obj.clone()));
            }
        }
        Ok(None)
    }

    /// Remove an object from this player's ownership
    /// Matches C++ Player::removeObject
    pub fn remove_owned_object(&mut self, object_id: ObjectID) {
        self.owned_objects.retain(|&id| id != object_id);

        let Some(object) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(object_guard) = object.read() else {
            return;
        };

        if !object_guard.test_status(ObjectStatusTypes::UnderConstruction) {
            let power = object_guard.get_template().get_energy_production();
            if power > 0 {
                if !object_guard.is_disabled() {
                    self.add_power_production(-power);
                }
            } else if power < 0 {
                self.add_power_consumption(power);
            }
        }

        if object_guard.is_kind_of(crate::common::KindOf::Dozer) {
            if let Some(ai) = object_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if ai_guard.is_idle() {
                        crate::helpers::TheInGameUI::remove_idle_worker(
                            &*object_guard,
                            self.player_index,
                        );
                    }
                }
            }
        }
    }

    /// Get the number of objects owned by this player
    pub fn get_owned_object_count(&self) -> usize {
        self.owned_objects.len()
    }

    /// Get the upgrade manager for this player
    /// Matches C++ Player::getUpgradeManager
    pub fn get_upgrade_manager(&self) -> Option<&PlayerUpgradeManager> {
        Some(&self.upgrade_manager)
    }

    /// Get mutable upgrade manager for this player
    pub fn get_upgrade_manager_mut(&mut self) -> Option<&mut PlayerUpgradeManager> {
        Some(&mut self.upgrade_manager)
    }

    /// Update player state each frame
    pub fn update(&mut self) {
        let sabotage_frame = self.energy.get_power_sabotaged_till_frame();
        if sabotage_frame != 0 && TheGameLogic::get_frame() > sabotage_frame {
            self.energy.set_power_sabotaged_till_frame(0);
            let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
        }
    }

    /// Called when a new map is loaded
    pub fn new_map(&mut self) {
        // Reset transient state for new map
        self.radar_count = 0;
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            timers.clear();
        }
        self.attacked_by = [false; MAX_PLAYER_COUNT];
        self.attacked_frame = 0;
    }

    fn add_new_shared_special_power_timer(
        &mut self,
        template: &SpecialPowerTemplate,
        frame: UnsignedInt,
    ) {
        let mut timer = SpecialPowerReadyTimer::new();
        timer.template_id = template.get_id();
        timer.ready_frame = frame;
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            timers.push(timer);
        }
    }

    pub fn reset_or_start_special_power_ready_frame(&mut self, template: &SpecialPowerTemplate) {
        let now = TheGameLogic::get_frame();
        let lookup_id = template.get_id();
        let mut needs_insert = true;

        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == lookup_id {
                    timer.ready_frame = now + template.get_reload_time();
                    needs_insert = false;
                    break;
                }
            }
        }

        if needs_insert {
            self.add_new_shared_special_power_timer(template, now);
        }
    }

    pub fn express_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        frame: UnsignedInt,
    ) {
        let lookup_id = template.get_id();
        let mut needs_insert = true;
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == lookup_id {
                    timer.ready_frame = frame;
                    needs_insert = false;
                    break;
                }
            }
        }

        if needs_insert {
            self.add_new_shared_special_power_timer(template, frame);
        }
    }

    pub fn get_or_start_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
    ) -> UnsignedInt {
        let now = TheGameLogic::get_frame();
        let lookup_id = template.get_id();
        let mut ready_frame = None;

        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == lookup_id {
                    ready_frame = Some(timer.ready_frame);
                    break;
                }
            }
        }

        if let Some(frame) = ready_frame {
            frame
        } else {
            self.add_new_shared_special_power_timer(template, now);
            now
        }
    }

    pub fn set_display_name<S: Into<String>>(&mut self, name: S) {
        let name = name.into();
        self.player_display_name = name.clone();
        if self.player_name_key == 0 && !name.is_empty() {
            self.player_name_key = NameKeyGenerator::name_to_key(&name);
        }
    }

    pub fn set_player_name_key(&mut self, key: NameKeyType) {
        self.player_name_key = key;
    }

    pub fn set_side<S: Into<String>>(&mut self, side: S) {
        self.side = side.into();
    }

    pub fn set_base_side<S: Into<String>>(&mut self, base_side: S) {
        self.base_side = base_side.into();
    }

    pub fn set_colors(&mut self, primary: Color, night: Color) {
        self.color = primary;
        self.night_color = night;
    }

    pub fn set_observer(&mut self, observer: Bool) {
        self.is_observer = observer;
    }

    /// Initialize from player template
    pub fn init(&mut self, player_template: Arc<PlayerTemplate>) {
        self.energy.reset();
        let mut template = (*player_template).clone();
        if template.production_cost_changes.is_empty()
            && template.production_time_changes.is_empty()
            && template.production_veterancy_levels.is_empty()
        {
            template.hydrate_from_common_store();
        }
        self.player_template = Some(Arc::new(template.clone()));
        self.side = template.side.clone();
        self.base_side = template.base_side.clone();
        self.player_display_name = template.display_name.clone();
        if self.player_name_key == 0 {
            let key_source = if !template.name.is_empty() {
                template.name.as_str()
            } else {
                self.player_display_name.as_str()
            };
            if !key_source.is_empty() {
                self.player_name_key = NameKeyGenerator::name_to_key(key_source);
            }
        }
        self.is_observer = template.is_observer;

        // Apply starting money from the player template.
        // In C++ this is set during Player::init() via the PlayerTemplate's
        // StartingMoney field.  When the template has not been populated from
        // INI yet (Money::count_money() == 0) we fall back to the standard
        // skirmish default of $10,000 so that players always start with money.
        let starting = template.starting_money.count_money();
        let amount = if starting > 0 {
            starting as i32
        } else {
            10_000
        };
        self.money.set_money(amount);

        self.reset_rank_impl();
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();
    }

    pub fn init_from_dict_defaults(&mut self) {
        for slot in &mut self.squads {
            *slot = Some(Squad::new());
        }
        self.current_selection = Some(Squad::new());
        self.tunnel_tracker = Some(TunnelTracker::new());
        self.resource_manager = Some(ResourceGatheringManager::new());
        self.player_relations.map.clear();
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.clear();
        }
        self.attacked_by = [false; MAX_PLAYER_COUNT];
        self.attacked_frame = 0;
    }

    /// Set default team
    pub fn set_default_team(&mut self, team: Option<Arc<RwLock<Team>>>) {
        self.default_team = team;
    }

    pub fn get_default_team(&self) -> Option<Arc<RwLock<Team>>> {
        self.default_team.as_ref().map(Arc::clone)
    }

    /// Get the default team ID for this player
    pub fn get_default_team_id(&self) -> Option<TeamID> {
        self.default_team
            .as_ref()
            .and_then(|team| team.read().ok().map(|t| t.get_id()))
    }

    // Getters for core properties
    pub fn get_player_display_name(&self) -> &String {
        &self.player_display_name
    }

    pub fn get_player_name_key(&self) -> NameKeyType {
        self.player_name_key
    }

    pub fn get_mp_start_index(&self) -> Int {
        self.mp_start_index
    }

    pub fn set_mp_start_index(&mut self, index: Int) {
        self.mp_start_index = index;
    }

    pub fn set_is_preorder(&mut self, value: Bool) {
        self.is_preorder = value;
    }

    pub fn get_side(&self) -> &String {
        &self.side
    }

    pub fn get_base_side(&self) -> &String {
        &self.base_side
    }

    pub fn get_player_template(&self) -> Option<&Arc<PlayerTemplate>> {
        self.player_template.as_ref()
    }

    pub fn get_objects(&self) -> Vec<Arc<RwLock<Object>>> {
        let mut objects = Vec::new();
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let object_ids = manager.get_objects_owned_by_player(self.player_index as UnsignedInt);
            for obj_id in object_ids {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj_instance) = obj_arc.read() {
                        objects.push(obj_instance.base.clone());
                    }
                }
            }
        }
        objects
    }

    /// Check if player has any objects at all.
    /// C++ Reference: Player::hasAnyObjects()
    pub fn has_any_objects(&self) -> Bool {
        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                continue;
            }
            if object_guard.is_kind_of(KindOf::Projectile)
                || object_guard.is_kind_of(KindOf::Inert)
                || object_guard.is_kind_of(KindOf::Mine)
            {
                continue;
            }
            return true;
        }
        false
    }

    /// Check if player has any units (non-structure objects)
    /// C++ Reference: Player::hasAnyUnits() - checks for non-structure units
    pub fn has_any_units(&self) -> Bool {
        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                continue;
            }
            if object_guard.is_kind_of(KindOf::Structure)
                || object_guard.is_kind_of(KindOf::Projectile)
                || object_guard.is_kind_of(KindOf::Mine)
            {
                continue;
            }
            return true;
        }
        false
    }

    /// Check if player has any buildings that count for victory.
    /// C++ Reference: Player::hasAnyBuildings(KINDOF_MP_COUNT_FOR_VICTORY)
    pub fn has_any_buildings_counts_for_victory(&self) -> Bool {
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let object_ids = manager.get_objects_owned_by_player(self.player_index as UnsignedInt);
            for obj_id in object_ids {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj_instance) = obj_arc.read() {
                        if let Ok(base_obj) = obj_instance.base.read() {
                            if base_obj.is_kind_of(KindOf::Structure)
                                && base_obj.is_kind_of(KindOf::CountsForVictory)
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if player has any build facilities (structures that can produce units)
    /// C++ Reference: Player::hasAnyBuildFacility() - checks for buildings with production capability
    pub fn has_any_build_facility(&self) -> Bool {
        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.get_template().is_build_facility() {
                return true;
            }
        }
        false
    }

    /// Called when a unit is created by this player
    /// Matches C++ Player::onUnitCreated
    pub fn on_unit_created(&mut self, producer: &Arc<RwLock<Object>>, unit: &Arc<RwLock<Object>>) {
        // Update score keeper
        if let Ok(unit_guard) = unit.read() {
            // Check if it's a structure or unit
            if unit_guard.is_kind_of(KindOf::Structure) {
                self.score_keeper.add_building_built();

                // Track in academy stats
                let type_name = unit_guard.get_template().get_name().as_str();
                self.academy_stats.record_building_built(type_name);
            } else {
                self.score_keeper.add_unit_built();

                // Track in academy stats
                let type_name = unit_guard.get_template().get_name().as_str();
                self.academy_stats.record_unit_built(type_name);
            }
        }

        // In full implementation, would also:
        // - Update production queues
        // - Trigger AI notifications
        // - Update veterancy on producer if applicable
        // - Check for achievement/challenge progress
        let _ = producer; // Mark as used for future implementation
    }

    /// Called when a structure under construction is completed.
    /// Matches C++ Player::onStructureConstructionComplete.
    pub fn on_structure_construction_complete(
        &mut self,
        builder: Option<&Arc<RwLock<Object>>>,
        structure: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
    ) {
        crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();

        let (
            structure_id,
            structure_pos,
            structure_layer,
            is_superweapon_particle,
            is_superweapon_nuke,
            is_superweapon_scud,
        ) = {
            let Ok(structure_guard) = structure.read() else {
                return;
            };
            (
                structure_guard.get_id(),
                *structure_guard.get_position(),
                structure_guard.get_layer(),
                structure_guard.has_special_power(
                    crate::object::special_power_types::SpecialPowerType::ParticleUplinkCannon,
                ),
                structure_guard.has_special_power(
                    crate::object::special_power_types::SpecialPowerType::NeutronMissile,
                ),
                structure_guard.has_special_power(
                    crate::object::special_power_types::SpecialPowerType::ScudStorm,
                ),
            )
        };

        if let Ok(ai_guard) = crate::ai::THE_AI.read() {
            if let Some(pathfinding) = ai_guard.pathfinding_system() {
                if let Ok(mut system) = pathfinding.write() {
                    let layer =
                        crate::ai::pathfinding_system::PathfindLayerEnum::from(structure_layer);
                    let positions = [structure_pos];
                    system.remove_obstacle(structure_id, &positions, layer);
                    system.add_obstacle(structure_id, &positions, layer);
                }
            }
        }

        if !is_rebuild {
            if let Ok(structure_guard) = structure.read() {
                self.score_keeper.add_building_built();
                let cost = structure_guard
                    .get_template()
                    .calc_cost_to_build(Some(self))
                    .max(0) as u32;
                self.score_keeper.add_money_spent(cost);
                self.academy_stats
                    .record_building_built(structure_guard.get_template().get_name().as_str());
            }
        }

        if let Ok(structure_guard) = structure.read() {
            structure_guard.adjust_power_for_player(true);
        }

        if let Some(builder_arc) = builder {
            let player_id = self.player_index as u32;
            let factory_id = builder_arc.read().map(|b| b.get_id()).unwrap_or(INVALID_ID);
            let structure_id = structure.read().map(|s| s.get_id()).unwrap_or(INVALID_ID);
            let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
                manager.with_ai_player_mut(player_id, |ai_player| {
                    let _ = ai_player.on_structure_produced(factory_id, structure_id);
                })
            });
        }

        crate::control_bar::mark_ui_dirty();

        let local_player = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        if let (Some(local_player), Ok(structure_guard)) = (local_player, structure.read()) {
            let relation = structure_guard
                .get_team()
                .and_then(|team| {
                    team.read().ok().map(|team_guard| {
                        local_player
                            .read()
                            .ok()
                            .map(|p| p.get_relationship_with_team(&team_guard))
                    })
                })
                .flatten()
                .unwrap_or(Relationship::Neutral);

            if is_superweapon_particle {
                if local_player.read().ok().map(|p| p.get_player_index()) == Some(self.player_index)
                {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnParticleCannon,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyParticleCannon,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyParticleCannon,
                    );
                }
            }

            if is_superweapon_nuke {
                if local_player.read().ok().map(|p| p.get_player_index()) == Some(self.player_index)
                {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnNuke,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyNuke,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyNuke,
                    );
                }
            }

            if is_superweapon_scud {
                if local_player.read().ok().map(|p| p.get_player_index()) == Some(self.player_index)
                {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnScudStorm,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyScudStorm,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyScudStorm,
                    );
                }
            }
        }
    }

    /// Set units vision spied state
    /// Matches C++ Player::setUnitsVisionSpied
    pub fn set_units_vision_spied(
        &mut self,
        on: Bool,
        spy_on_kind_of: crate::common::KindOfMaskType,
        spying_player_index: PlayerIndex,
    ) {
        use crate::common::{ALL_KIND_OF, KIND_OF_MASK_ALL, KIND_OF_MASK_NONE};
        use crate::object::registry::OBJECT_REGISTRY;

        fn matches_any_kind_of(object: &Object, mask: crate::common::KindOfMaskType) -> bool {
            if mask == KIND_OF_MASK_ALL {
                return true;
            }
            if mask == KIND_OF_MASK_NONE {
                return false;
            }

            for &kind in ALL_KIND_OF {
                let bit = 1u64 << (kind as u32);
                if (mask & bit) != 0 && object.is_kind_of(kind) {
                    return true;
                }
            }

            false
        }

        for &object_id in &self.owned_objects {
            let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };

            let object_lock = &*object_arc;
            if let Ok(mut obj_guard) = object_lock.write() {
                if matches_any_kind_of(&*obj_guard, spy_on_kind_of) {
                    obj_guard.set_vision_spied_by_player(spying_player_index, on);
                }
            };
        }
    }

    /// Called when a unit owned by this player is destroyed
    /// Matches C++ Player::onUnitDestroyed
    pub fn on_unit_destroyed(
        &mut self,
        unit: &Arc<RwLock<Object>>,
        _by_player: Option<PlayerIndex>,
    ) {
        if let Ok(unit_guard) = unit.read() {
            // Update score keeper
            if unit_guard.is_kind_of(KindOf::Structure) {
                self.score_keeper.buildings_lost += 1;
            } else {
                self.score_keeper.add_unit_lost();
            }
        }
    }

    /// Called when this player destroys an enemy unit
    /// Matches C++ Player::onEnemyUnitKilled
    pub fn on_enemy_unit_killed(&mut self, killed_unit: &Arc<RwLock<Object>>) {
        if let Ok(unit_guard) = killed_unit.read() {
            // Update score keeper
            if unit_guard.is_kind_of(KindOf::Structure) {
                self.score_keeper.add_building_destroyed();

                // Track in academy stats
                let type_name = unit_guard.get_template().get_name().as_str();
                self.academy_stats.record_building_destroyed(type_name);
            } else {
                self.score_keeper.add_unit_killed();

                // Track in academy stats
                let type_name = unit_guard.get_template().get_name().as_str();
                self.academy_stats.record_unit_killed(type_name);
            }
        }
    }

    pub fn is_playable_side(&self) -> bool {
        !self.is_observer
    }

    pub fn get_handicap(&self) -> &PlayerHandicap {
        &self.handicap
    }

    pub fn apply_handicap_from_dict(&mut self, dict: &crate::common::Dict) {
        self.handicap.read_from_dict(dict);
    }

    pub fn set_handicap(&mut self, value: Real) {
        self.handicap.set_all(value);
    }

    pub fn get_money(&self) -> &PlayerMoney {
        &self.money
    }

    pub fn get_money_mut(&mut self) -> &mut PlayerMoney {
        &mut self.money
    }

    /// C++ Player::getSupplyBoxValue hook. Today it returns the global base value,
    /// but callers should go through Player so later economy modifiers stay local.
    pub fn get_supply_box_value(&self) -> UnsignedInt {
        global_data::read_safe()
            .map(|data| data.base_value_per_supply_box.max(0) as UnsignedInt)
            .unwrap_or(0)
    }

    pub fn get_energy(&self) -> &PlayerEnergy {
        &self.energy
    }

    pub fn get_energy_mut(&mut self) -> &mut PlayerEnergy {
        &mut self.energy
    }

    /// Called when power brown-out state changes for this player
    /// Brown-out occurs when power consumption exceeds production
    /// Matches C++ Player::onPowerBrownOutChange
    pub fn on_power_brown_out_change(&mut self, is_brown_out: bool) -> Result<(), GameError> {
        if is_brown_out {
            self.disable_radar();
        } else {
            self.enable_radar();
        }

        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let object_ids = manager.get_objects_owned_by_player(self.player_index as UnsignedInt);

            for obj_id in object_ids {
                let Some(obj_arc) = manager.get_object(obj_id) else {
                    continue;
                };
                let Ok(obj_instance) = obj_arc.write() else {
                    continue;
                };
                let Ok(mut base_obj) = obj_instance.base.write() else {
                    continue;
                };
                if base_obj.is_kind_of(KindOf::Powered) {
                    if is_brown_out {
                        base_obj.set_disabled(DisabledType::DisabledUnderpowered);
                    } else {
                        base_obj.clear_disabled(DisabledType::DisabledUnderpowered);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_player_color(&self) -> Color {
        self.color
    }

    pub fn get_player_night_color(&self) -> Color {
        self.night_color
    }

    pub fn get_player_type(&self) -> PlayerType {
        self.player_type
    }

    pub fn set_player_type(&mut self, player_type: PlayerType, skirmish: Bool) {
        self.player_type = player_type;
        self.is_skirmish_ai = match player_type {
            PlayerType::Computer => skirmish,
            _ => false,
        };
    }

    pub fn get_player_index(&self) -> PlayerIndex {
        self.player_index
    }

    /// Return a bitmask that is unique to this player
    pub fn get_player_mask(&self) -> PlayerMaskType {
        PlayerMaskType::from_bits_truncate(1 << self.player_index)
    }

    pub fn get_player_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    /// Check if player has the given science
    pub fn has_science(&self, science: ScienceType) -> Bool {
        science != SCIENCE_INVALID && self.sciences.contains(&science)
    }

    /// Check if science is disabled
    pub fn is_science_disabled(&self, science: ScienceType) -> Bool {
        science != SCIENCE_INVALID && self.sciences_disabled.contains(&science)
    }

    /// Check if science is hidden
    pub fn is_science_hidden(&self, science: ScienceType) -> Bool {
        science != SCIENCE_INVALID && self.sciences_hidden.contains(&science)
    }

    /// Set science availability
    pub fn set_science_availability(
        &mut self,
        science: ScienceType,
        availability_type: ScienceAvailabilityType,
    ) {
        if science == SCIENCE_INVALID {
            return;
        }
        match availability_type {
            ScienceAvailabilityType::Available => {
                self.sciences_disabled.retain(|&s| s != science);
                self.sciences_hidden.retain(|&s| s != science);
            }
            ScienceAvailabilityType::Disabled => {
                if !self.sciences_disabled.contains(&science) {
                    self.sciences_disabled.push(science);
                }
                self.sciences_hidden.retain(|&s| s != science);
            }
            ScienceAvailabilityType::Hidden => {
                if !self.sciences_hidden.contains(&science) {
                    self.sciences_hidden.push(science);
                }
                self.sciences_disabled.retain(|&s| s != science);
            }
        }
    }

    /// Parse science availability from script text.
    /// Matches C++ Player::getScienceAvailabilityTypeFromString.
    pub fn get_science_availability_type_from_string(
        name: &str,
    ) -> Option<ScienceAvailabilityType> {
        if name.eq_ignore_ascii_case("Available") {
            Some(ScienceAvailabilityType::Available)
        } else if name.eq_ignore_ascii_case("Disabled") {
            Some(ScienceAvailabilityType::Disabled)
        } else if name.eq_ignore_ascii_case("Hidden") {
            Some(ScienceAvailabilityType::Hidden)
        } else {
            None
        }
    }

    /// Check if player has upgrade complete
    /// Matches C++ Player::hasUpgradeComplete
    pub fn has_upgrade_complete(&self, upgrade_template: &UpgradeTemplate) -> Bool {
        let upgrade_name = upgrade_template.get_name();
        let mask_bit = crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str());
        let mask_value = UpgradeMaskType::from_bits_retain(mask_bit.bits());
        (self.upgrades_completed & mask_value).bits() != 0
    }

    /// Check if upgrade is in production
    /// Matches C++ Player::hasUpgradeInProduction
    pub fn has_upgrade_in_production(&self, upgrade_template: &UpgradeTemplate) -> Bool {
        let upgrade_name = upgrade_template.get_name();
        let mask_bit = crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str());
        let mask_value = UpgradeMaskType::from_bits_retain(mask_bit.bits());
        (self.upgrades_in_progress & mask_value).bits() != 0
    }

    /// Get completed upgrade mask
    pub fn get_completed_upgrade_mask(&self) -> UpgradeMaskType {
        self.upgrades_completed
    }

    /// Add KindOf production cost change (matches C++ Player::addKindOfProductionCostChange)
    pub fn add_kind_of_production_cost_change(&mut self, kind_of: KindOfMaskType, percent: Real) {
        for entry in &mut self.kind_of_percent_production_change_list {
            if entry.kind_of == kind_of && (entry.percent - percent).abs() < f32::EPSILON {
                entry.refs = entry.refs.saturating_add(1);
                return;
            }
        }

        self.kind_of_percent_production_change_list
            .push(KindOfPercentProductionChange {
                kind_of,
                percent,
                refs: 1,
            });
    }

    /// Remove KindOf production cost change (matches C++ Player::removeKindOfProductionCostChange)
    pub fn remove_kind_of_production_cost_change(
        &mut self,
        kind_of: KindOfMaskType,
        percent: Real,
    ) {
        let mut idx = None;
        for (i, entry) in self
            .kind_of_percent_production_change_list
            .iter_mut()
            .enumerate()
        {
            if entry.kind_of == kind_of && (entry.percent - percent).abs() < f32::EPSILON {
                if entry.refs > 0 {
                    entry.refs -= 1;
                }
                if entry.refs == 0 {
                    idx = Some(i);
                }
                break;
            }
        }

        if let Some(i) = idx {
            self.kind_of_percent_production_change_list.remove(i);
        } else if idx.is_none() {
            log::warn!(
                "remove_kind_of_production_cost_change missing entry kind_of={} percent={} ",
                kind_of,
                percent
            );
        }
    }

    fn lookup_production_change(map: &HashMap<NameKeyType, Real>, template_name: &str) -> Real {
        let key = NameKeyGenerator::name_to_key(template_name);
        map.get(&key).copied().unwrap_or(0.0)
    }

    /// Production cost change percent for this template name (matches C++ Player::getProductionCostChangePercent).
    pub fn get_production_cost_change_percent(&self, template_name: &str) -> Real {
        let Some(template) = self.player_template.as_ref() else {
            return 0.0;
        };

        Self::lookup_production_change(&template.production_cost_changes, template_name)
    }

    /// Production time change percent for this template name (matches C++ Player::getProductionTimeChangePercent).
    pub fn get_production_time_change_percent(&self, template_name: &str) -> Real {
        let Some(template) = self.player_template.as_ref() else {
            return 0.0;
        };

        Self::lookup_production_change(&template.production_time_changes, template_name)
    }

    /// Get production cost change based on KindOf mask (matches C++ Player::getProductionCostChangeBasedOnKindOf)
    pub fn get_production_cost_change_based_on_kind_of(&self, kind_of: KindOfMaskType) -> Real {
        let mut result: Real = 1.0;
        for entry in &self.kind_of_percent_production_change_list {
            if (kind_of & entry.kind_of) != KIND_OF_MASK_NONE {
                result *= 1.0 + entry.percent;
            }
        }
        result
    }

    /// Power management
    pub fn add_power_bonus(&mut self, obj: ObjectID) {
        self.energy.add_power_bonus(obj);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    pub fn remove_power_bonus(&mut self, obj: ObjectID) {
        self.energy.remove_power_bonus(obj);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    /// Adjust power production/consumption (matches C++ Energy::adjustPower).
    pub fn adjust_power(&mut self, power_delta: Int, adding: Bool) {
        self.energy.adjust_power(power_delta, adding);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    /// New object influences the power grid (matches C++ Energy::objectEnteringInfluence).
    pub fn object_entering_influence(&mut self, obj: &Object) {
        self.energy.object_entering_influence(obj);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    /// Object no longer influences the power grid (matches C++ Energy::objectLeavingInfluence).
    pub fn object_leaving_influence(&mut self, obj: &Object) {
        self.energy.object_leaving_influence(obj);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    /// Update sabotage timer for the power grid (matches C++ Energy::setPowerSabotagedTillFrame).
    pub fn set_power_sabotaged_till_frame(&mut self, frame: UnsignedInt) {
        self.energy.set_power_sabotaged_till_frame(frame);
    }

    /// Direct production adjustment with brown-out handling.
    pub fn add_power_production(&mut self, amount: Int) {
        self.energy.add_power_production(amount);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    /// Direct consumption adjustment with brown-out handling.
    pub fn add_power_consumption(&mut self, amount: Int) {
        self.energy.add_power_consumption(amount);
        let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
    }

    /// Radar management
    pub fn add_radar(&mut self, disable_proof: Bool) {
        self.radar_count += 1;
        if disable_proof {
            self.disable_proof_radar_count += 1;
        }
    }

    pub fn remove_radar(&mut self, disable_proof: Bool) {
        self.radar_count = (self.radar_count - 1).max(0);
        if disable_proof {
            self.disable_proof_radar_count = (self.disable_proof_radar_count - 1).max(0);
        }
    }

    pub fn disable_radar(&mut self) {
        self.radar_disabled = true;
    }

    pub fn enable_radar(&mut self) {
        self.radar_disabled = false;
    }

    pub fn has_radar(&self) -> Bool {
        self.radar_count > 0 && !self.radar_disabled
    }

    /// Player state checks
    pub fn is_local_player(&self) -> Bool {
        let Ok(list) = player_list().read() else {
            return false;
        };
        list.get_local_player_index() == self.player_index
    }

    pub fn is_player_observer(&self) -> Bool {
        self.is_observer
    }

    pub fn is_player_dead(&self) -> Bool {
        self.is_player_dead
    }

    /// Check if player is defeated
    /// Matches C++ Player::isDefeated
    pub fn is_defeated(&self) -> Bool {
        self.is_player_dead
    }

    /// Set player defeated state
    /// Matches C++ Player::setDefeated
    pub fn set_defeated(&mut self, defeated: Bool) {
        self.is_player_dead = defeated;
    }

    pub fn is_player_active(&self) -> Bool {
        !self.is_player_dead && !self.is_observer
    }

    pub fn did_player_preorder(&self) -> Bool {
        self.is_preorder
    }

    pub fn get_list_in_score_screen(&self) -> Bool {
        self.list_in_score_screen
    }

    pub fn set_list_in_score_screen(&mut self, value: Bool) {
        self.list_in_score_screen = value;
    }

    /// Score keeping
    pub fn get_score_keeper(&self) -> &ScoreKeeper {
        &self.score_keeper
    }

    pub fn get_score_keeper_mut(&mut self) -> &mut ScoreKeeper {
        &mut self.score_keeper
    }

    /// Iterate over the objects owned by this player
    /// Matches C++ Player::iterateObjects
    pub fn iterate_objects<F>(&self, mut func: F) -> Result<(), GameError>
    where
        F: FnMut(Arc<RwLock<Object>>) -> Result<(), GameError>,
    {
        // Get all objects owned by this player from the object manager
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let object_ids = manager.get_objects_owned_by_player(self.player_index as UnsignedInt);

            // Iterate through each object and call the function
            for obj_id in object_ids {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    // Call the function with the object
                    // Note: We need to get the GameObjectInstance's base Object
                    if let Ok(obj_instance) = obj_arc.read() {
                        let base_obj = obj_instance.base.clone();
                        func(base_obj)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Academy stats
    pub fn get_academy_stats(&self) -> &AcademyStats {
        &self.academy_stats
    }

    pub fn get_academy_stats_mut(&mut self) -> &mut AcademyStats {
        &mut self.academy_stats
    }

    /// Experience and ranking
    pub fn get_skill_points(&self) -> Int {
        self.skill_points
    }

    pub fn get_science_purchase_points(&self) -> Int {
        self.science_purchase_points
    }

    pub fn get_skill_points_modifier(&self) -> Real {
        self.skill_points_modifier
    }

    pub fn set_skill_points_modifier(&mut self, modifier: Real) {
        self.skill_points_modifier = modifier;
    }

    pub fn get_rank_level(&self) -> Int {
        self.rank_level
    }

    pub fn get_general_name(&self) -> &String {
        &self.general_name
    }

    pub fn set_general_name(&mut self, name: String) {
        self.general_name = name;
    }

    /// Set rank level, returns true if rank actually changed
    ///
    /// Delegates to science_management module for full implementation
    pub fn set_rank_level(&mut self, level: Int) -> Bool {
        self.set_rank_level_impl(level)
    }

    /// Add skill points, returns true if player gained/lost levels
    ///
    /// Delegates to science_management module for full implementation
    pub fn add_skill_points(&mut self, delta: Int) -> Bool {
        self.add_skill_points_impl(delta)
    }

    /// Add skill points for killing an object
    ///
    /// Delegates to science_management module for full implementation
    pub fn add_skill_points_for_kill(
        &mut self,
        killer: Option<ObjectID>,
        victim_under_construction: bool,
        victim_skill_value: Int,
    ) -> Bool {
        self.add_skill_points_for_kill_impl(killer, victim_under_construction, victim_skill_value)
    }

    /// Add science purchase points
    ///
    /// Delegates to science_management module for full implementation
    pub fn add_science_purchase_points(&mut self, delta: Int) {
        self.add_science_purchase_points_impl(delta);
    }

    /// Add a science to the player
    ///
    /// Delegates to science_management module for full implementation
    pub fn add_science(&mut self, science: ScienceType) -> Bool {
        self.add_science_impl(science)
    }

    /// Grant a science for free
    ///
    /// Delegates to science_management module for full implementation
    pub fn grant_science(&mut self, science: ScienceType) -> Bool {
        self.grant_science_impl(science)
    }

    /// Attempt to purchase a science
    ///
    /// Delegates to science_management module for full implementation
    pub fn attempt_to_purchase_science(&mut self, science: ScienceType) -> Bool {
        self.attempt_to_purchase_science_impl(science)
    }

    /// Check if player can purchase a science
    ///
    /// Delegates to science_management module for full implementation
    pub fn is_capable_of_purchasing_science(&self, science: ScienceType) -> Bool {
        self.is_capable_of_purchasing_science_impl(science)
    }

    /// Check if player has prerequisites for a science
    ///
    /// Delegates to science_management module for full implementation
    pub fn has_prereqs_for_science(&self, science: ScienceType) -> Bool {
        self.has_prereqs_for_science_impl(science)
    }

    /// Get purchasable sciences
    ///
    /// Delegates to science_management module for full implementation
    pub fn get_purchasable_sciences(&self) -> (ScienceVec, ScienceVec) {
        self.get_purchasable_sciences_impl()
    }

    /// Reset sciences to intrinsic + rank-granted
    ///
    /// Delegates to science_management module for full implementation
    pub fn reset_sciences(&mut self) {
        self.reset_sciences_impl();
    }

    /// Unit and building control
    pub fn get_can_build_units(&self) -> Bool {
        self.can_build_units
    }

    pub fn set_can_build_units(&mut self, can_build: Bool) {
        self.can_build_units = can_build;
    }

    pub fn get_can_build_base(&self) -> Bool {
        self.can_build_base
    }

    pub fn set_can_build_base(&mut self, can_build: Bool) {
        self.can_build_base = can_build;
    }

    /// Enable/disable all owned objects of a specific template type.
    /// Matches C++ Player::setObjectsEnabled.
    pub fn set_objects_enabled(&mut self, template_type_to_affect: &str, enable: Bool) {
        let object_ids = self.owned_objects.clone();
        for object_id in object_ids {
            let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut object_guard) = object_arc.write() else {
                continue;
            };
            if object_guard.get_template().get_name().as_str() == template_type_to_affect {
                object_guard.set_script_status(
                    crate::object::ObjectScriptStatusBit::ScriptDisabled,
                    !enable,
                );
            }
        }
    }

    /// Check whether the player is allowed to build the given template.
    pub fn can_build_template(&self, template: &dyn crate::common::ThingTemplate) -> Bool {
        if template.is_kind_of(crate::common::KindOf::Structure) {
            if !self.can_build_base {
                return false;
            }
        } else if !self.can_build_units {
            return false;
        }

        let buildable_status = crate::helpers::TheGameLogic::find_buildable_status_override(
            template.get_name().as_str(),
        );
        if let Some(status) = buildable_status {
            // BuildableStatus values mirror C++:
            // 0=Yes, 1=Ignore_Prerequisites, 2=No, 3=Only_By_AI.
            if status == 2 {
                return false;
            }
            if status == 1 {
                return true;
            }
            if status == 3 && self.player_type != PlayerType::Computer {
                return false;
            }
        } else if let Some(status) = template.get_buildable_status() {
            use game_engine::common::thing::BuildableStatus;

            match status {
                BuildableStatus::No => return false,
                BuildableStatus::IgnorePrerequisites => return true,
                BuildableStatus::OnlyByAi if self.player_type != PlayerType::Computer => {
                    return false;
                }
                BuildableStatus::Yes | BuildableStatus::OnlyByAi => {}
            }
        }

        if !self.ignores_prereqs() {
            for prereq in template.get_production_prerequisites() {
                if !self.is_production_prerequisite_satisfied(prereq) {
                    return false;
                }
            }
        }

        if !self.can_build_more_of_type(template) {
            return false;
        }

        true
    }

    fn is_production_prerequisite_satisfied(
        &self,
        prereq: &game_engine::common::rts::ProductionPrerequisite,
    ) -> Bool {
        prereq.is_satisfied_with_counter(
            |science| self.has_science(science),
            |handles, ignore_dead, counts| {
                let templates: Vec<_> = handles
                    .iter()
                    .filter_map(|handle| {
                        crate::helpers::TheThingFactory::find_template_by_id(handle.value())
                    })
                    .collect();

                if templates.len() != handles.len() {
                    counts.fill(0);
                    return;
                }

                self.count_objects_by_thing_template(&templates, ignore_dead, false, counts);
            },
        )
    }

    fn can_build_more_of_type(&self, template: &dyn crate::common::ThingTemplate) -> Bool {
        let max_simultaneous = template.get_max_simultaneous_of_type();
        if max_simultaneous == 0 {
            return true;
        }

        let link_key = template.get_max_simultaneous_link_key();
        let check_production_queue = !template.is_kind_of(crate::common::KindOf::Structure);
        let mut count = 0u32;
        for &object_id in &self.owned_objects {
            let Some(object_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.is_effectively_dead() {
                continue;
            }

            let object_template = object_guard.get_template();
            if template.is_equivalent_to(object_template.as_ref())
                || (link_key != 0 && link_key == object_template.get_max_simultaneous_link_key())
            {
                count += 1;
                if count >= max_simultaneous {
                    return false;
                }
            }

            if check_production_queue {
                let Some(production_behavior) = object_guard.get_production_update_interface()
                else {
                    continue;
                };
                let Ok(mut behavior_guard) = production_behavior.lock() else {
                    continue;
                };
                let Some(production) = behavior_guard.get_production_update_interface() else {
                    continue;
                };

                for entry in production.get_queue_entries() {
                    if entry.production_type
                        != crate::object::production::queue::ProductionType::Unit
                    {
                        continue;
                    }
                    let Some(queued_template) =
                        crate::helpers::TheThingFactory::find_template(&entry.template_name)
                    else {
                        continue;
                    };
                    if template.is_equivalent_to(queued_template.as_ref())
                        || (link_key != 0
                            && link_key == queued_template.get_max_simultaneous_link_key())
                    {
                        count += 1;
                        if count >= max_simultaneous {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// Hunting behavior
    pub fn get_units_should_hunt(&self) -> Bool {
        self.units_should_hunt
    }

    pub fn set_units_should_hunt(&mut self, should_hunt: Bool, _source: CommandSourceType) {
        self.units_should_hunt = should_hunt;
    }

    /// Forward scripted repair requests to this player's AI controller.
    /// Matches C++ Player::repairStructure.
    pub fn repair_structure(&mut self, structure_id: ObjectID) {
        let player_id = self.player_index as u32;
        let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                let _ = ai_player.repair_structure(structure_id);
            })
        });
    }

    /// Set the current AI skillset selector for this player.
    /// Matches C++ Player::friend_setSkillset.
    pub fn friend_set_skillset(&mut self, skill_set: Int) {
        let player_id = self.player_index as u32;
        let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                ai_player.select_skillset(skill_set);
            })
        });
    }

    /// Set AI team build delay in seconds for this player.
    /// Matches C++ Player::setTeamDelaySeconds.
    pub fn set_team_delay_seconds(&mut self, delay: Int) {
        let player_id = self.player_index as u32;
        let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                ai_player.set_team_delay_seconds(delay as Real);
            })
        });
    }

    /// Force units to idle in place or resume supply trucking.
    /// Matches C++ Player::setUnitsShouldIdleOrResume.
    pub fn set_units_should_idle_or_resume(&mut self, idle: Bool, source: CommandSourceType) {
        for object_id in &self.owned_objects {
            let Some(object) = crate::object::registry::OBJECT_REGISTRY.get_object(*object_id)
            else {
                continue;
            };
            let Ok(object_guard) = object.read() else {
                continue;
            };

            if object_guard.is_kind_of(crate::common::KindOf::Structure) {
                continue;
            }

            let Some(ai) = object_guard.get_ai_update_interface() else {
                continue;
            };

            let Ok(mut ai_guard) = ai.lock() else {
                continue;
            };

            if idle {
                let pos = *object_guard.get_position();
                drop(ai_guard);
                ai.ai_move_to_position(&pos, false, source);
            } else if ai_guard.is_idle() {
                if let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() {
                    truck.set_force_wanting_state(true);
                }
            }
        }
    }

    /// Attack tracking
    pub fn set_attacked_by(&mut self, player_index: Int) {
        if player_index >= 0 && (player_index as usize) < MAX_PLAYER_COUNT {
            self.attacked_by[player_index as usize] = true;
            self.attacked_frame = TheGameLogic::get_frame();
        }
    }

    pub fn get_attacked_by(&self, player_index: Int) -> Bool {
        if player_index >= 0 && (player_index as usize) < MAX_PLAYER_COUNT {
            self.attacked_by[player_index as usize]
        } else {
            false
        }
    }

    pub fn get_attacked_frame(&self) -> UnsignedInt {
        self.attacked_frame
    }

    /// Cash bounty system
    pub fn get_cash_bounty(&self) -> Real {
        self.cash_bounty_percent
    }

    pub fn set_cash_bounty(&mut self, percentage: Real) {
        self.cash_bounty_percent = percentage;
    }

    /// Do bounty for kill - awards cash when player kills an enemy
    /// C++ Reference: Player::doBountyForKill() (Player.cpp lines 1963-1989)
    ///
    /// # Arguments
    /// * `killer_cost` - The cost of the victim object (used for bounty calculation)
    ///
    /// # Returns
    /// The bounty amount awarded.
    pub fn do_bounty_for_kill(&mut self, killer_cost: Int) -> Int {
        // Calculate bounty based on victim's cost and our cash bounty percent
        let bounty = ((killer_cost as Real) * self.cash_bounty_percent).ceil() as Int;

        // Award the bounty
        if bounty > 0 {
            let _ = self.money.deposit(bounty as u32);
        }

        bounty
    }

    /// Do bounty for kill using object references.
    /// C++ Reference: Player::doBountyForKill() with object parameters
    ///
    /// # Arguments
    /// * `_killer` - The object that made the kill (unused in basic implementation)
    /// * `victim` - The object that was killed
    ///
    /// Returns the bounty amount awarded.
    pub fn do_bounty_for_kill_obj(
        &mut self,
        _killer: &dyn game_engine::common::rts::player::BountyObject,
        victim: &dyn game_engine::common::rts::player::BountyObject,
    ) -> Int {
        // C++ line 1972: Get victim's build cost for bounty calculation
        let killer_cost = victim.get_build_cost();

        // C++ line 1973: Under construction objects don't give bounty
        if victim.is_under_construction() {
            return 0;
        }

        self.do_bounty_for_kill(killer_cost)
    }

    /// Add skill points for kill using object references.
    /// C++ Reference: Player::addSkillPointsForKill() with object parameters
    ///
    /// # Arguments
    /// * `killer` - The object that made the kill
    /// * `victim` - The object that was killed
    ///
    /// Returns true if player gained/lost levels.
    pub fn add_skill_points_for_kill_obj(
        &mut self,
        killer: &dyn game_engine::common::rts::player::SkillPointObject,
        victim: &dyn game_engine::common::rts::player::SkillPointObject,
    ) -> Bool {
        let _victim_level = victim.get_veterancy_level();
        let skill_value = victim.get_skill_point_value(killer);
        self.add_skill_points_for_kill(None, false, skill_value)
    }

    /// Retaliation mode
    pub fn is_logical_retaliation_mode_enabled(&self) -> Bool {
        self.logical_retaliation_mode_enabled
    }

    pub fn set_logical_retaliation_mode_enabled(&mut self, enabled: Bool) {
        self.logical_retaliation_mode_enabled = enabled;
    }

    /// Hotkey squad management
    pub fn get_hotkey_squad(&mut self, squad_number: Int) -> Option<&mut Squad> {
        if squad_number >= 0 && (squad_number as usize) < NUM_HOTKEY_SQUADS {
            self.squads[squad_number as usize].as_mut()
        } else {
            None
        }
    }

    /// Return the current selection as an AIGroup (matches C++ Player::getCurrentSelectionAsAIGroup).
    pub fn get_current_selection_as_ai_group(&mut self, group: &mut AIGroup) {
        if let Some(selection) = &mut self.current_selection {
            let _ = selection.ai_group_from_squad(group);
        }
    }

    /// Return the current selection as a list of object IDs.
    /// Matches C++ selection iteration that operates on selected object IDs.
    pub fn get_current_selection_ids(&self) -> Vec<ObjectID> {
        self.current_selection
            .as_ref()
            .map(|selection| selection.get_object_ids().clone())
            .unwrap_or_default()
    }

    /// Set the current selection from an AIGroup (matches C++ Player::setCurrentlySelectedAIGroup).
    pub fn set_currently_selected_ai_group(&mut self, group: Option<&AIGroup>) {
        if self.current_selection.is_none() {
            self.current_selection = Some(Squad::new());
        }

        if let Some(selection) = &mut self.current_selection {
            selection.clear_squad();
            if let Some(group) = group {
                selection.squad_from_ai_group(group, true);
            }
        }
    }

    /// Add members of an AIGroup to the current selection (matches C++ Player::addAIGroupToCurrentSelection).
    pub fn add_ai_group_to_current_selection(&mut self, group: &AIGroup) {
        if self.current_selection.is_none() {
            self.current_selection = Some(Squad::new());
        }

        if let Some(selection) = &mut self.current_selection {
            let ids = group.get_all_ids_snapshot();
            for object_id in ids {
                selection.add_object_id(object_id);
            }
        }
    }

    /// Add a single object to the current selection.
    pub fn add_object_to_current_selection(&mut self, object_id: ObjectID) {
        if self.current_selection.is_none() {
            self.current_selection = Some(Squad::new());
        }

        if let Some(selection) = &mut self.current_selection {
            selection.add_object_id(object_id);
        }
    }

    /// Replace current selection with a single object.
    pub fn set_current_selection_to_object(&mut self, object_id: ObjectID) {
        if self.current_selection.is_none() {
            self.current_selection = Some(Squad::new());
        }

        if let Some(selection) = &mut self.current_selection {
            selection.clear_squad();
            selection.add_object_id(object_id);
        }
    }

    /// Remove a single object from current selection.
    pub fn remove_object_from_current_selection(&mut self, object_id: ObjectID) -> Bool {
        let Some(selection) = &mut self.current_selection else {
            return false;
        };

        let before = selection.get_object_ids().len();
        selection.remove_object_id(object_id);
        let after = selection.get_object_ids().len();

        if after == 0 {
            self.current_selection = None;
        }

        before != after
    }

    // Debug/cheat functions
    #[cfg(any(debug_assertions, feature = "internal"))]
    pub fn toggle_ignore_prereqs(&mut self) {
        self.demo_ignore_prereqs = !self.demo_ignore_prereqs;
    }

    #[cfg(any(debug_assertions, feature = "internal"))]
    pub fn ignores_prereqs(&self) -> Bool {
        self.demo_ignore_prereqs
    }

    #[cfg(any(debug_assertions, feature = "internal"))]
    pub fn toggle_free_build(&mut self) {
        self.demo_free_build = !self.demo_free_build;
    }

    #[cfg(any(debug_assertions, feature = "internal"))]
    pub fn builds_for_free(&self) -> Bool {
        self.demo_free_build
    }

    #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
    pub fn toggle_instant_build(&mut self) {
        self.demo_instant_build = !self.demo_instant_build;
    }

    #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
    pub fn builds_instantly(&self) -> Bool {
        self.demo_instant_build
    }

    /// Player relationship management
    ///
    /// Get relationship between this player and another player
    /// Matches C++ Player.cpp:542 Player::getRelationship
    pub fn get_relationship(&self, that_player: &Player) -> Relationship {
        self.player_relations
            .map
            .get(&that_player.get_player_index())
            .copied()
            .unwrap_or(Relationship::Neutral)
    }

    /// Get relationship between this player and a team
    /// Checks team override first, then player override, then neutral
    /// Matches C++ Player.cpp:542-572 Player::getRelationship(const Team*)
    pub fn get_relationship_with_team(&self, that_team: &Team) -> Relationship {
        // Check for team-specific relationship override
        if let Some(ref team_relations) = self.team_relations {
            if let Some(&relationship) = team_relations.map.get(&that_team.get_id()) {
                return relationship;
            }
        }

        // Check for player relationship override
        if let Some(controlling_player_id) = that_team.get_controlling_player_id() {
            if controlling_player_id as PlayerIndex == self.player_index {
                return Relationship::Allies;
            }
            if let Some(&relationship) = self
                .player_relations
                .map
                .get(&(controlling_player_id as PlayerIndex))
            {
                return relationship;
            }
        }

        Relationship::Neutral
    }

    /// Set player-to-player relationship
    /// Matches C++ Player.cpp:575 Player::setPlayerRelationship
    pub fn set_player_relationship(&mut self, that_player: &Player, relationship: Relationship) {
        self.player_relations
            .map
            .insert(that_player.get_player_index(), relationship);
    }

    /// Set player-to-player relationship by player index.
    /// Thin helper for script actions that only carry resolved player IDs.
    pub fn set_player_relationship_by_index(
        &mut self,
        that_player_index: PlayerIndex,
        relationship: Relationship,
    ) {
        self.player_relations
            .map
            .insert(that_player_index, relationship);
    }

    /// Remove player-to-player relationship override
    /// Matches C++ Player.cpp:585 Player::removePlayerRelationship
    pub fn remove_player_relationship(&mut self, that_player: &Player) -> Bool {
        self.player_relations
            .map
            .remove(&that_player.get_player_index())
            .is_some()
    }

    /// Set player-to-team relationship override
    /// Matches C++ Player.cpp:608 Player::setTeamRelationship
    pub fn set_team_relationship(&mut self, that_team: &Team, relationship: Relationship) {
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.insert(that_team.get_id(), relationship);
        }
    }

    /// Remove player-to-team relationship override
    /// Matches C++ Player.cpp:618 Player::removeTeamRelationship
    pub fn remove_team_relationship(&mut self, that_team: &Team) -> Bool {
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.remove(&that_team.get_id()).is_some()
        } else {
            false
        }
    }

    /// Check if this player is allied with another player
    pub fn is_allied_with_player(&self, that_player: &Player) -> Bool {
        matches!(self.get_relationship(that_player), Relationship::Allies)
    }

    /// Check if this player is allied with a team
    pub fn is_allied_with_team(&self, that_team: &Team) -> Bool {
        matches!(
            self.get_relationship_with_team(that_team),
            Relationship::Allies
        )
    }

    /// Check if this player is enemy with another player
    pub fn is_enemy_with_player(&self, that_player: &Player) -> Bool {
        matches!(self.get_relationship(that_player), Relationship::Enemies)
    }

    /// Check if this player is enemy with a team
    pub fn is_enemy_with_team(&self, that_team: &Team) -> Bool {
        matches!(
            self.get_relationship_with_team(that_team),
            Relationship::Enemies
        )
    }

    /// Get all allies of this player
    pub fn get_allied_players(&self) -> Vec<PlayerIndex> {
        self.player_relations
            .map
            .iter()
            .filter_map(|(&index, &rel)| {
                if rel == Relationship::Allies {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all enemies of this player
    pub fn get_enemy_players(&self) -> Vec<PlayerIndex> {
        self.player_relations
            .map
            .iter()
            .filter_map(|(&index, &rel)| {
                if rel == Relationship::Enemies {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if this player shares vision with another player (due to alliance)
    pub fn shares_vision_with(&self, that_player: &Player) -> Bool {
        self.is_allied_with_player(that_player)
    }

    /// Check if this player shares radar with another player (due to alliance)
    pub fn shares_radar_with(&self, that_player: &Player) -> Bool {
        self.is_allied_with_player(that_player) && self.has_radar() && that_player.has_radar()
    }

    /// Get tunnel system for this player (GLA faction tunnels)
    /// Returns reference to the tunnel network for this player
    pub fn get_tunnel_system(&self) -> Option<&TunnelTracker> {
        self.tunnel_tracker.as_ref()
    }

    /// Get mutable reference to tunnel system for this player
    pub fn get_tunnel_system_mut(&mut self) -> Option<&mut TunnelTracker> {
        self.tunnel_tracker.as_mut()
    }

    pub fn get_resource_manager(&self) -> Option<&ResourceGatheringManager> {
        self.resource_manager.as_ref()
    }

    pub fn get_resource_manager_mut(&mut self) -> Option<&mut ResourceGatheringManager> {
        self.resource_manager.as_mut()
    }

    /// Initialize tunnel tracker for this player
    /// Should be called when a player builds their first tunnel entrance
    pub fn init_tunnel_tracker(&mut self) {
        if self.tunnel_tracker.is_none() {
            self.tunnel_tracker = Some(TunnelTracker::new());
        }
    }

    /// Change battle plan count for this player
    /// Battle plans are strategic bonuses that affect units
    pub fn change_battle_plan(
        &mut self,
        plan_type: BattlePlanType,
        delta: Int,
        bonus: &BattlePlanBonuses,
    ) {
        let mut add_bonus = false;
        let mut remove_bonus = false;

        match plan_type {
            BattlePlanType::Bombard => {
                self.bombard_battle_plans += delta;
                if self.bombard_battle_plans == 1 && delta == 1 {
                    add_bonus = true;
                } else if self.bombard_battle_plans == 0 && delta == -1 {
                    remove_bonus = true;
                }
            }
            BattlePlanType::HoldTheLine => {
                self.hold_the_line_battle_plans += delta;
                if self.hold_the_line_battle_plans == 1 && delta == 1 {
                    add_bonus = true;
                } else if self.hold_the_line_battle_plans == 0 && delta == -1 {
                    remove_bonus = true;
                }
            }
            BattlePlanType::SearchAndDestroy => {
                self.search_and_destroy_battle_plans += delta;
                if self.search_and_destroy_battle_plans == 1 && delta == 1 {
                    add_bonus = true;
                } else if self.search_and_destroy_battle_plans == 0 && delta == -1 {
                    remove_bonus = true;
                }
            }
        }

        if add_bonus {
            self.apply_battle_plan_bonuses_for_player_objects(bonus);
        } else if remove_bonus {
            let mut inverted = bonus.clone();
            inverted.armor_scalar = 1.0 / inverted.armor_scalar.max(0.01);
            inverted.sight_range_scalar = 1.0 / inverted.sight_range_scalar.max(0.01);
            if inverted.bombardment > 0 {
                inverted.bombardment = -1;
            }
            if inverted.hold_the_line > 0 {
                inverted.hold_the_line = -1;
            }
            if inverted.search_and_destroy > 0 {
                inverted.search_and_destroy = -1;
            }
            self.apply_battle_plan_bonuses_for_player_objects(&inverted);
        }
    }

    /// Get battle plan count
    pub fn get_battle_plan_count(&self, plan_type: BattlePlanType) -> Int {
        match plan_type {
            BattlePlanType::Bombard => self.bombard_battle_plans,
            BattlePlanType::HoldTheLine => self.hold_the_line_battle_plans,
            BattlePlanType::SearchAndDestroy => self.search_and_destroy_battle_plans,
        }
    }

    /// Total number of active battle plans (matching C++ getNumBattlePlansActive).
    pub fn get_num_battle_plans_active(&self) -> Int {
        self.bombard_battle_plans
            + self.hold_the_line_battle_plans
            + self.search_and_destroy_battle_plans
    }

    fn local_apply_battle_plan_bonuses_to_object(
        &self,
        obj: &mut Object,
        bonus: &BattlePlanBonuses,
    ) {
        let mut object_to_validate_id = obj.get_object_id();
        let is_projectile = obj.is_kind_of(KindOf::Projectile);
        if is_projectile {
            let producer_id = obj.get_producer_id();
            if producer_id != INVALID_ID {
                object_to_validate_id = producer_id;
            }
        }

        let kind_mask = if object_to_validate_id == obj.get_object_id() {
            obj.get_kind_of()
        } else {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_to_validate_id) else {
                return;
            };
            let Ok(guard) = obj_arc.read() else {
                return;
            };
            guard.get_kind_of()
        };
        if (kind_mask & bonus.valid_kind_of) == 0 {
            return;
        }
        if (kind_mask & bonus.invalid_kind_of) != 0 {
            return;
        }

        if !is_projectile {
            if (bonus.armor_scalar - 1.0).abs() > f32::EPSILON {
                if let Some(body) = obj.get_body_module() {
                    if let Ok(mut body_guard) = body.lock() {
                        let _ = body_guard.apply_damage_scalar(bonus.armor_scalar);
                    }
                }
            }
            if (bonus.sight_range_scalar - 1.0).abs() > f32::EPSILON {
                let new_range = obj.get_vision_range() * bonus.sight_range_scalar;
                let new_shroud = obj.get_shroud_clearing_range() * bonus.sight_range_scalar;
                obj.set_vision_range(new_range);
                obj.set_shroud_clearing_range(new_shroud);
            }
        }

        if bonus.bombardment > 0 {
            obj.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanBombardment,
            );
        } else {
            obj.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanBombardment,
            );
        }
        if bonus.hold_the_line > 0 {
            obj.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanHoldTheLine,
            );
        } else {
            obj.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanHoldTheLine,
            );
        }
        if bonus.search_and_destroy > 0 {
            obj.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanSearchAndDestroy,
            );
        } else {
            obj.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanSearchAndDestroy,
            );
        }
    }

    /// New object or converted object gaining our current battle plan bonuses.
    pub fn apply_battle_plan_bonuses_for_object(&self, obj: &mut Object) {
        if let Some(bonuses) = &self.battle_plan_bonuses {
            self.local_apply_battle_plan_bonuses_to_object(obj, bonuses);
        }
    }

    /// Object has just left our team, so remove its bonuses.
    pub fn remove_battle_plan_bonuses_for_object(&self, obj: &mut Object) {
        let Some(bonuses) = &self.battle_plan_bonuses else {
            return;
        };

        let mut inverted = bonuses.clone();
        inverted.armor_scalar = 1.0 / inverted.armor_scalar.max(0.01);
        inverted.sight_range_scalar = 1.0 / inverted.sight_range_scalar.max(0.01);
        inverted.bombardment = -1;
        inverted.search_and_destroy = -1;
        inverted.hold_the_line = -1;

        self.local_apply_battle_plan_bonuses_to_object(obj, &inverted);
    }

    /// Battle plan bonuses changing, so apply to all of our objects.
    pub fn apply_battle_plan_bonuses_for_player_objects(&mut self, bonus: &BattlePlanBonuses) {
        if let Some(existing) = &mut self.battle_plan_bonuses {
            existing.armor_scalar *= bonus.armor_scalar;
            existing.sight_range_scalar *= bonus.sight_range_scalar;
            existing.bombardment = (existing.bombardment + bonus.bombardment).max(0);
            existing.hold_the_line = (existing.hold_the_line + bonus.hold_the_line).max(0);
            existing.search_and_destroy =
                (existing.search_and_destroy + bonus.search_and_destroy).max(0);
        } else {
            self.battle_plan_bonuses = Some(bonus.clone());
        }

        let owned_objects = self.owned_objects.clone();
        for object_id in owned_objects {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut guard) = obj_arc.write() {
                    self.local_apply_battle_plan_bonuses_to_object(&mut guard, bonus);
                }
            }
        }
    }
}

/// Save/load support for Player.
/// Matches C++ Player::xfer (Player.cpp:3975, version 8).
impl Snapshotable for Player {
    /// C++ Player::crc is intentionally much narrower than Player::xfer.
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut has_battle_plan_bonus = self.battle_plan_bonuses.is_some();
        xfer.xfer_bool(&mut has_battle_plan_bonus)
            .map_err(|e| e.to_string())?;
        if let Some(bonuses) = &self.battle_plan_bonuses {
            let mut armor_scalar = bonuses.armor_scalar;
            xfer.xfer_real(&mut armor_scalar)
                .map_err(|e| e.to_string())?;
            let mut sight_range_scalar = bonuses.sight_range_scalar;
            xfer.xfer_real(&mut sight_range_scalar)
                .map_err(|e| e.to_string())?;
            let mut bombardment = bonuses.bombardment;
            xfer.xfer_int(&mut bombardment).map_err(|e| e.to_string())?;
            let mut hold_the_line = bonuses.hold_the_line;
            xfer.xfer_int(&mut hold_the_line)
                .map_err(|e| e.to_string())?;
            let mut search_and_destroy = bonuses.search_and_destroy;
            xfer.xfer_int(&mut search_and_destroy)
                .map_err(|e| e.to_string())?;
            let mut valid_kind_of = bonuses.valid_kind_of;
            xfer.xfer_u64(&mut valid_kind_of)
                .map_err(|e| e.to_string())?;
            let mut invalid_kind_of = bonuses.invalid_kind_of;
            xfer.xfer_u64(&mut invalid_kind_of)
                .map_err(|e| e.to_string())?;
        }

        let mut skill_points = self.skill_points;
        xfer.xfer_int(&mut skill_points)
            .map_err(|e| e.to_string())?;
        let mut science_purchase_points = self.science_purchase_points;
        xfer.xfer_int(&mut science_purchase_points)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Player::xfer version 8
        let current_version: XferVersion = 8;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // Money (inline, matching C++ Money::xfer v1: just the amount)
        {
            let mut money_version: XferVersion = 1;
            xfer.xfer_version(&mut money_version, 1)
                .map_err(|e| e.to_string())?;
            let mut money_amount = self.money.amount as u32;
            xfer.xfer_u32(&mut money_amount)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.money.amount = money_amount as Int;
            }
        }

        // Upgrade list count
        let mut upgrade_count = self.upgrade_list.len() as u16;
        xfer.xfer_unsigned_short(&mut upgrade_count)
            .map_err(|e| e.to_string())?;

        // Version 7: preorder
        if version >= 7 {
            xfer.xfer_bool(&mut self.is_preorder)
                .map_err(|e| e.to_string())?;
        }

        // Version 8: disabled/hidden science vectors
        if version >= 8 {
            xfer.xfer_science_vec(&mut self.sciences_disabled)
                .map_err(|e| e.to_string())?;
            xfer.xfer_science_vec(&mut self.sciences_hidden)
                .map_err(|e| e.to_string())?;
        }

        // Upgrade instances
        if xfer.get_xfer_mode() == XferMode::Save {
            for upgrade in &mut self.upgrade_list {
                let mut upgrade_name = upgrade.get_template().get_name().to_string();
                xfer.xfer_ascii_string(&mut upgrade_name)
                    .map_err(|e| e.to_string())?;
                upgrade.xfer(xfer)?;
            }
        } else {
            self.upgrade_list.clear();
            for _ in 0..upgrade_count {
                let mut upgrade_name = String::new();
                xfer.xfer_ascii_string(&mut upgrade_name)
                    .map_err(|e| e.to_string())?;

                let template = crate::upgrade::center::get_upgrade_center()
                    .read()
                    .ok()
                    .and_then(|center| center.find_upgrade(&upgrade_name));
                if template.is_none() {
                    log::warn!("Player::xfer - Unable to find upgrade '{}'", upgrade_name);
                    // Skip the upgrade data by reading a dummy
                    let mut dummy_upgrade = crate::upgrade::Upgrade::new(Arc::new(
                        crate::upgrade::UpgradeTemplate::new(crate::common::AsciiString::from(
                            "__dummy__",
                        )),
                    ));
                    dummy_upgrade.xfer(xfer)?;
                    continue;
                }

                let template_arc = template.unwrap();
                let mut upgrade = crate::upgrade::Upgrade::new(template_arc);
                upgrade.xfer(xfer)?;
                self.upgrade_list.push(upgrade);
            }
        }

        // Radar info
        xfer.xfer_int(&mut self.radar_count)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_player_dead)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.disable_proof_radar_count)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.radar_disabled)
            .map_err(|e| e.to_string())?;

        // Upgrade masks
        {
            let mut in_progress = self.upgrades_in_progress.bits();
            xfer.xfer_u128(&mut in_progress)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.upgrades_in_progress = UpgradeMaskType::from_bits_truncate(in_progress);
            }
        }
        {
            let mut completed = self.upgrades_completed.bits();
            xfer.xfer_u128(&mut completed).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.upgrades_completed = UpgradeMaskType::from_bits_truncate(completed);
            }
        }

        // Energy (inline, matching C++ Energy::xfer v3)
        {
            let mut energy_version: XferVersion = 3;
            xfer.xfer_version(&mut energy_version, 3)
                .map_err(|e| e.to_string())?;
            if energy_version < 2 {
                let mut production: Int = self.energy.production;
                xfer.xfer_int(&mut production).map_err(|e| e.to_string())?;
                let mut consumption: Int = self.energy.consumption;
                xfer.xfer_int(&mut consumption).map_err(|e| e.to_string())?;
                if xfer.get_xfer_mode() == XferMode::Load {
                    self.energy.production = 0; // rebuilt from objects
                    self.energy.consumption = 0; // rebuilt from objects
                }
            }
            let mut owning_player_index = self.player_index;
            xfer.xfer_int(&mut owning_player_index)
                .map_err(|e| e.to_string())?;
            if energy_version >= 3 {
                xfer.xfer_u32(&mut self.energy.power_sabotaged_till_frame)
                    .map_err(|e| e.to_string())?;
            }
        }

        // Team prototypes (count + IDs, resolved on load via TeamFactory)
        {
            let mut prototype_count = self.player_team_prototypes.len() as u16;
            xfer.xfer_unsigned_short(&mut prototype_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for prototype in &self.player_team_prototypes {
                    let mut proto_id = prototype.get_id();
                    xfer.xfer_u32(&mut proto_id).map_err(|e| e.to_string())?;
                }
            } else {
                self.player_team_prototypes.clear();
                let factory = crate::team::get_team_factory();
                let Ok(factory_guard) = factory.lock() else {
                    return Err("Player::xfer - cannot lock TeamFactory".to_string());
                };
                for _ in 0..prototype_count {
                    let mut proto_id: UnsignedInt = 0;
                    xfer.xfer_u32(&mut proto_id).map_err(|e| e.to_string())?;
                    if let Some(prototype) = factory_guard.find_team_prototype_by_id(proto_id) {
                        self.player_team_prototypes.push(prototype);
                    }
                }
            }
        }

        // Build list info (count + snapshots)
        {
            let mut build_list_count: UnsignedShort = 0;
            if xfer.get_xfer_mode() == XferMode::Save {
                let mut entry = self.build_list.as_deref();
                while let Some(info) = entry {
                    build_list_count = build_list_count.saturating_add(1);
                    entry = info.get_next();
                }
            }

            xfer.xfer_unsigned_short(&mut build_list_count)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Save {
                let mut entry = self.build_list.as_deref_mut();
                while let Some(info) = entry {
                    info.xfer(xfer);
                    entry = info.get_next_mut();
                }
            } else {
                let mut entries = Vec::with_capacity(build_list_count as usize);
                for _ in 0..build_list_count {
                    let mut info = BuildListInfo::new();
                    info.xfer(xfer);
                    entries.push(info);
                }

                self.build_list = None;
                for mut info in entries.into_iter().rev() {
                    info.set_next_build_list_boxed(self.build_list.take());
                    self.build_list = Some(Box::new(info));
                }
            }
        }

        // AI player data. C++ writes a presence bool, then the AIPlayer/AISkirmishPlayer snapshot.
        {
            let player_id = self.player_index as u32;
            let mut ai_present = if xfer.get_xfer_mode() == XferMode::Save {
                crate::ai::integration::with_ai_integration(|manager| {
                    manager.has_ai_player(player_id)
                })
                .unwrap_or(false)
            } else {
                false
            };
            xfer.xfer_bool(&mut ai_present).map_err(|e| e.to_string())?;

            if ai_present {
                let xfer_result = crate::ai::integration::with_ai_integration_mut(|manager| {
                    manager.xfer_ai_player(player_id, self.is_skirmish_ai, xfer)
                });

                match xfer_result {
                    Some(Ok(())) => {}
                    Some(Err(err)) => return Err(err),
                    None if xfer.get_xfer_mode() == XferMode::Load => {
                        log::warn!(
                            "Player::xfer - consuming AI snapshot for player {} without integration manager",
                            player_id
                        );
                        if self.is_skirmish_ai {
                            let mut ai_player =
                                crate::ai::skirmish_player::AISkirmishPlayer::new(player_id);
                            ai_player.xfer(xfer);
                        } else {
                            let mut ai_player = crate::ai::ai_player::AIPlayer::new(player_id);
                            ai_player.xfer(xfer);
                        }
                    }
                    None => {
                        return Err(format!(
                            "Player::xfer - AI integration manager unavailable for player {}",
                            player_id
                        ));
                    }
                }
            }
        }

        // Resource gathering manager
        {
            let has_rgm = self.resource_manager.is_some();
            let mut rgm_present = has_rgm;
            xfer.xfer_bool(&mut rgm_present)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.resource_manager = if rgm_present {
                    Some(ResourceGatheringManager::new())
                } else {
                    None
                };
            }
            if let Some(manager) = self.resource_manager.as_mut() {
                manager.xfer(xfer)?;
            }
        }

        // Tunnel tracker
        {
            let has_tunnel = self.tunnel_tracker.is_some();
            let mut tunnel_present = has_tunnel;
            xfer.xfer_bool(&mut tunnel_present)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.tunnel_tracker = if tunnel_present {
                    Some(TunnelTracker::new())
                } else {
                    None
                };
            }
            if let Some(tracker) = self.tunnel_tracker.as_mut() {
                tracker.xfer(xfer)?;
            }
        }

        // Default team ID
        {
            let mut team_id: UnsignedInt = self
                .default_team
                .as_ref()
                .and_then(|t| t.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::team::TEAM_ID_INVALID);
            xfer.xfer_u32(&mut team_id).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                let factory = crate::team::get_team_factory();
                if let Ok(mut factory_guard) = factory.lock() {
                    self.default_team = factory_guard.find_team_by_id(team_id);
                }
            }
        }

        // Sciences (version >= 5)
        if version >= 5 {
            if xfer.get_xfer_mode() == XferMode::Load {
                self.sciences.clear();
            }
            xfer.xfer_science_vec(&mut self.sciences)
                .map_err(|e| e.to_string())?;
        }

        // Rank/skill
        xfer.xfer_int(&mut self.rank_level)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.skill_points)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.science_purchase_points)
            .map_err(|e| e.to_string())?;

        // Level up/down (C++ has these, Rust may not track them separately)
        let mut level_up: Int = 0;
        xfer.xfer_int(&mut level_up).map_err(|e| e.to_string())?;
        let mut level_down: Int = 0;
        xfer.xfer_int(&mut level_down).map_err(|e| e.to_string())?;

        // General name (C++ Player::xfer writes UnicodeString)
        xfer.xfer_unicode_string(&mut self.general_name)
            .map_err(|e| e.to_string())?;

        // Player relations (inline, matching C++ PlayerRelationMap::xfer v1)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;
            let mut rel_count = self.player_relations.map.len() as u16;
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for (&pidx, &rel) in &self.player_relations.map {
                    let mut player_idx = pidx;
                    let mut rel_raw = rel as Int;
                    xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                }
            } else {
                self.player_relations.map.clear();
                for _ in 0..rel_count {
                    let mut player_idx: Int = 0;
                    let mut rel_raw: Int = 0;
                    xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    let rel = match rel_raw {
                        0 => Relationship::Enemies,
                        1 => Relationship::Neutral,
                        2 => Relationship::Allies,
                        _ => Relationship::Neutral,
                    };
                    self.player_relations.map.insert(player_idx, rel);
                }
            }
        }

        // Team relations (inline, matching C++ TeamRelationMap::xfer v1)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;
            let mut rel_count = self
                .team_relations
                .as_ref()
                .map(|r| r.map.len() as u16)
                .unwrap_or(0);
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                if let Some(ref relations) = self.team_relations {
                    for (&tid, &rel) in &relations.map {
                        let mut team_id_val = tid;
                        let mut rel_raw = rel as Int;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                self.team_relations = None;
                if rel_count > 0 {
                    let mut map = crate::team::TeamRelationMap::new();
                    for _ in 0..rel_count {
                        let mut team_id_val: UnsignedInt = 0;
                        let mut rel_raw: Int = 0;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                        let rel = match rel_raw {
                            0 => Relationship::Enemies,
                            1 => Relationship::Neutral,
                            2 => Relationship::Allies,
                            _ => Relationship::Neutral,
                        };
                        map.map.insert(team_id_val, rel);
                    }
                    self.team_relations = Some(map);
                }
            }
        }

        // Build flags
        xfer.xfer_bool(&mut self.can_build_units)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.can_build_base)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_observer)
            .map_err(|e| e.to_string())?;

        // Version 2: skill points modifier
        if version >= 2 {
            xfer.xfer_real(&mut self.skill_points_modifier)
                .map_err(|e| e.to_string())?;
        }

        // Version 3: list in score screen
        if version >= 3 {
            xfer.xfer_bool(&mut self.list_in_score_screen)
                .map_err(|e| e.to_string())?;
        }

        // Attacked by array (raw bytes matching C++ xferUser)
        for i in 0..MAX_PLAYER_COUNT {
            let mut val = self.attacked_by[i];
            xfer.xfer_bool(&mut val).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.attacked_by[i] = val;
            }
        }

        // Cash bounty percent
        xfer.xfer_real(&mut self.cash_bounty_percent)
            .map_err(|e| e.to_string())?;

        // ScoreKeeper (inline, matching C++ ScoreKeeper::xfer v1)
        {
            let mut sk_version: XferVersion = 1;
            xfer.xfer_version(&mut sk_version, 1)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.supplies_collected)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.supplies_spent)
                .map_err(|e| e.to_string())?;
            // units destroyed per player (C++ has array, we skip)
            let mut _dummy: Int;
            for _ in 0..MAX_PLAYER_COUNT {
                _dummy = 0;
                xfer.xfer_int(&mut _dummy).map_err(|e| e.to_string())?;
            }
            xfer.xfer_int(&mut self.score_keeper.units_built)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.units_lost)
                .map_err(|e| e.to_string())?;
            // buildings destroyed per player
            for _ in 0..MAX_PLAYER_COUNT {
                _dummy = 0;
                xfer.xfer_int(&mut _dummy).map_err(|e| e.to_string())?;
            }
            xfer.xfer_int(&mut self.score_keeper.buildings_built)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.buildings_lost)
                .map_err(|e| e.to_string())?;
            // tech buildings captured, faction buildings captured, current score, player index
            _dummy = 0;
            xfer.xfer_int(&mut _dummy).map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut _dummy).map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut _dummy).map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut _dummy).map_err(|e| e.to_string())?;
            // objects built map (count + entries)
            {
                let mut obj_map_count: u16 = 0;
                xfer.xfer_unsigned_short(&mut obj_map_count)
                    .map_err(|e| e.to_string())?;
                for _ in 0..obj_map_count {
                    let mut _key: u32 = 0;
                    let mut _val: Int = 0;
                    xfer.xfer_u32(&mut _key).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut _val).map_err(|e| e.to_string())?;
                }
            }
            // objects destroyed per-player array
            let mut destroyed_array_size = MAX_PLAYER_COUNT as u16;
            xfer.xfer_unsigned_short(&mut destroyed_array_size)
                .map_err(|e| e.to_string())?;
            for _ in 0..destroyed_array_size {
                let mut obj_map_count: u16 = 0;
                xfer.xfer_unsigned_short(&mut obj_map_count)
                    .map_err(|e| e.to_string())?;
                for _ in 0..obj_map_count {
                    let mut _key: u32 = 0;
                    let mut _val: Int = 0;
                    xfer.xfer_u32(&mut _key).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut _val).map_err(|e| e.to_string())?;
                }
            }
            // objects lost, objects captured
            for _ in 0..2 {
                let mut obj_map_count: u16 = 0;
                xfer.xfer_unsigned_short(&mut obj_map_count)
                    .map_err(|e| e.to_string())?;
                for _ in 0..obj_map_count {
                    let mut _key: u32 = 0;
                    let mut _val: Int = 0;
                    xfer.xfer_u32(&mut _key).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut _val).map_err(|e| e.to_string())?;
                }
            }
        }

        // KindOf percent production change list
        {
            let mut change_count = self.kind_of_percent_production_change_list.len() as u16;
            xfer.xfer_unsigned_short(&mut change_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for entry in &self.kind_of_percent_production_change_list {
                    let mut kind_of_raw = entry.kind_of;
                    xfer.xfer_u64(&mut kind_of_raw).map_err(|e| e.to_string())?;
                    let mut percent = entry.percent;
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                    let mut refs = entry.refs;
                    xfer.xfer_u32(&mut refs).map_err(|e| e.to_string())?;
                }
            } else {
                self.kind_of_percent_production_change_list.clear();
                for _ in 0..change_count {
                    let mut kind_of_raw: u64 = 0;
                    xfer.xfer_u64(&mut kind_of_raw).map_err(|e| e.to_string())?;
                    let mut percent: Real = 0.0;
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                    let mut refs: UnsignedInt = 0;
                    xfer.xfer_u32(&mut refs).map_err(|e| e.to_string())?;
                    self.kind_of_percent_production_change_list.push(
                        KindOfPercentProductionChange {
                            kind_of: kind_of_raw,
                            percent,
                            refs,
                        },
                    );
                }
            }
        }

        // Version 4+: special power ready timer list
        if version >= 4 {
            let mut timer_count: u16 = 0;
            if let Ok(timers) = self.special_power_ready_timers.read() {
                timer_count = timers.len() as u16;
            }
            xfer.xfer_unsigned_short(&mut timer_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                if let Ok(timers) = self.special_power_ready_timers.read() {
                    for timer in timers.iter() {
                        let mut template_id = timer.template_id;
                        let mut ready_frame = timer.ready_frame;
                        xfer.xfer_u32(&mut template_id).map_err(|e| e.to_string())?;
                        xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                    }
                }
            } else if let Ok(mut timers) = self.special_power_ready_timers.write() {
                timers.clear();
                for _ in 0..timer_count {
                    let mut template_id: UnsignedInt = 0;
                    let mut ready_frame: UnsignedInt = 0;
                    xfer.xfer_u32(&mut template_id).map_err(|e| e.to_string())?;
                    xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                    timers.push(SpecialPowerReadyTimer {
                        template_id,
                        ready_frame,
                    });
                }
            }
        }

        // Squads
        {
            let mut squad_count = NUM_HOTKEY_SQUADS as u16;
            xfer.xfer_unsigned_short(&mut squad_count)
                .map_err(|e| e.to_string())?;
            if squad_count as usize != NUM_HOTKEY_SQUADS {
                return Err("Player::xfer - squad count mismatch".to_string());
            }
            for slot in &mut self.squads {
                if slot.is_none() {
                    *slot = Some(Squad::new());
                }
                if let Some(ref mut squad) = slot {
                    squad.xfer(xfer)?;
                }
            }
        }

        // Current selection (present bool + snapshot)
        {
            let mut selection_present = self.current_selection.is_some();
            xfer.xfer_bool(&mut selection_present)
                .map_err(|e| e.to_string())?;
            if selection_present {
                if self.current_selection.is_none() {
                    self.current_selection = Some(Squad::new());
                }
                if let Some(ref mut selection) = self.current_selection {
                    selection.xfer(xfer)?;
                }
            } else {
                self.current_selection = None;
            }
        }

        // Battle plan bonuses
        {
            let mut has_bonus = self.battle_plan_bonuses.is_some();
            xfer.xfer_bool(&mut has_bonus).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.battle_plan_bonuses = None;
                if has_bonus {
                    self.battle_plan_bonuses = Some(BattlePlanBonuses {
                        armor_scalar: 1.0,
                        sight_range_scalar: 1.0,
                        bombardment: 0,
                        hold_the_line: 0,
                        search_and_destroy: 0,
                        valid_kind_of: crate::common::KIND_OF_MASK_NONE,
                        invalid_kind_of: crate::common::KIND_OF_MASK_NONE,
                    });
                }
            }
            if let Some(ref mut bonuses) = self.battle_plan_bonuses {
                xfer.xfer_real(&mut bonuses.armor_scalar)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_real(&mut bonuses.sight_range_scalar)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut bonuses.bombardment)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut bonuses.hold_the_line)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut bonuses.search_and_destroy)
                    .map_err(|e| e.to_string())?;
                let mut valid_kind_of = bonuses.valid_kind_of;
                xfer.xfer_u64(&mut valid_kind_of)
                    .map_err(|e| e.to_string())?;
                bonuses.valid_kind_of = valid_kind_of;
                let mut invalid_kind_of = bonuses.invalid_kind_of;
                xfer.xfer_u64(&mut invalid_kind_of)
                    .map_err(|e| e.to_string())?;
                bonuses.invalid_kind_of = invalid_kind_of;
            }
        }

        // Battle plan counts
        xfer.xfer_int(&mut self.bombard_battle_plans)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.hold_the_line_battle_plans)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.search_and_destroy_battle_plans)
            .map_err(|e| e.to_string())?;

        // Version 6: units_should_hunt
        if version >= 6 {
            xfer.xfer_bool(&mut self.units_should_hunt)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let player_id = self.player_index as u32;
        let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.load_post_process_ai_player(player_id);
        });
        if let Some(manager) = self.resource_manager.as_mut() {
            manager.load_post_process()?;
        }
        if let Some(tracker) = self.tunnel_tracker.as_mut() {
            tracker.load_post_process()?;
        }
        Ok(())
    }
}

impl PlayerInterface for Player {
    fn get_or_start_special_power_ready_frame(
        &self,
        power_id: SpecialPowerID,
        current_frame: FrameCount,
    ) -> FrameCount {
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == power_id {
                    return timer.ready_frame;
                }
            }

            let mut timer = SpecialPowerReadyTimer::new();
            timer.template_id = power_id;
            timer.ready_frame = current_frame;
            timers.push(timer);
        }

        current_frame
    }

    fn express_special_power_ready_frame(&mut self, power_id: SpecialPowerID, frame: FrameCount) {
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == power_id {
                    timer.ready_frame = frame;
                    return;
                }
            }

            let mut timer = SpecialPowerReadyTimer::new();
            timer.template_id = power_id;
            timer.ready_frame = frame;
            timers.push(timer);
        }
    }

    fn reset_or_start_special_power_ready_frame(
        &mut self,
        power_id: SpecialPowerID,
        current_frame: FrameCount,
        reload_time: FrameCount,
    ) {
        let ready_frame = current_frame.saturating_add(reload_time);
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == power_id {
                    timer.ready_frame = ready_frame;
                    return;
                }
            }

            let mut timer = SpecialPowerReadyTimer::new();
            timer.template_id = power_id;
            timer.ready_frame = ready_frame;
            timers.push(timer);
        }
    }

    fn has_science(&self, science_name: &str) -> bool {
        let Some(store) = get_science_store() else {
            return false;
        };
        let science = store.get_science_from_internal_name(science_name);
        ScienceAccess::has_science(self, science)
    }

    fn get_player_index(&self) -> UnsignedInt {
        self.player_index as UnsignedInt
    }

    #[cfg(any(debug_assertions, feature = "allow_debug_cheats"))]
    fn builds_instantly(&self) -> bool {
        self.builds_instantly()
    }

    #[cfg(not(any(debug_assertions, feature = "allow_debug_cheats")))]
    fn builds_instantly(&self) -> bool {
        false
    }

    fn get_money(&self) -> &dyn MoneyInterface {
        &self.money
    }

    fn get_build_time_modifier(&self) -> f32 {
        let mut modifier = self.handicap.get_build_time_multiplier();
        let energy_ratio = self.energy.supply_ratio();

        let (low_energy_penalty_modifier, min_speed, max_speed) =
            if let Some(data) = game_engine::common::ini::get_global_data() {
                let guard = data.read();
                (
                    guard.low_energy_penalty_modifier,
                    guard.min_low_energy_production_speed,
                    guard.max_low_energy_production_speed,
                )
            } else {
                (0.5_f32, 0.5_f32, 1.0_f32)
            };

        let energy_percent = energy_ratio.min(1.0);
        let energy_short = (1.0 - energy_percent) * low_energy_penalty_modifier;
        let mut penalty_rate = 1.0 - energy_short;
        penalty_rate = penalty_rate.max(min_speed);
        if energy_percent < 1.0 {
            penalty_rate = penalty_rate.min(max_speed);
        }
        let penalty_rate = if penalty_rate <= 0.0 {
            0.01
        } else {
            penalty_rate
        };

        modifier /= penalty_rate;
        modifier
    }

    fn get_cost_modifier(&self) -> f32 {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            if self.builds_for_free() {
                return 0.0;
            }
        }

        self.handicap.get_cost_multiplier()
    }
}

impl Default for Player {
    fn default() -> Self {
        Player::new(0)
    }
}

impl ScienceAccess for Player {
    fn has_science(&self, science: ScienceType) -> bool {
        science != SCIENCE_INVALID && self.sciences.contains(&science)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_crc::XferCRC;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn player_crc(player: &Player) -> u32 {
        let sink = Cursor::new(Vec::<u8>::new());
        let inner = XferSave::new(sink, 1);
        let mut xfer = XferCRC::new(inner);
        Snapshotable::crc(player, &mut xfer).unwrap();
        xfer.get_crc()
    }

    #[test]
    fn player_crc_matches_cpp_skill_and_science_surface() {
        let mut base = Player::new(0);
        base.skill_points = 10;
        base.science_purchase_points = 3;
        let base_crc = player_crc(&base);

        let mut save_only_change = Player::new(0);
        save_only_change.skill_points = 10;
        save_only_change.science_purchase_points = 3;
        save_only_change.money.set_money(50_000);
        save_only_change.general_name = "General AΩ".to_string();
        save_only_change.radar_count = 7;
        save_only_change.bombard_battle_plans = 2;

        assert_eq!(player_crc(&save_only_change), base_crc);

        let mut skill_change = Player::new(0);
        skill_change.skill_points = 11;
        skill_change.science_purchase_points = 3;

        assert_ne!(player_crc(&skill_change), base_crc);
    }

    #[test]
    fn player_crc_includes_battle_plan_bonus_payload_like_cpp() {
        let base = Player::new(0);
        let mut with_bonus = Player::new(0);
        with_bonus.battle_plan_bonuses = Some(BattlePlanBonuses {
            armor_scalar: 1.25,
            sight_range_scalar: 1.5,
            bombardment: 1,
            hold_the_line: 0,
            search_and_destroy: 1,
            valid_kind_of: 0x12,
            invalid_kind_of: 0x40,
        });

        assert_ne!(player_crc(&with_bonus), player_crc(&base));
    }
}

/// Global player management
static PLAYER_LIST: OnceLock<RwLock<PlayerList>> = OnceLock::new();

/// Player list management (matching C++ PlayerList functionality)
#[derive(Debug)]
pub struct PlayerList {
    players: Vec<Arc<RwLock<Player>>>,
    local_player_index: PlayerIndex,
}

impl PlayerList {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            local_player_index: PLAYER_INDEX_INVALID,
        }
    }

    pub fn add_player(&mut self, player: Arc<RwLock<Player>>) {
        self.players.push(player);
    }

    pub fn get_player(&self, index: PlayerIndex) -> Option<&Arc<RwLock<Player>>> {
        self.players.get(index as usize)
    }

    pub fn get_player_count(&self) -> usize {
        self.players.len()
    }

    pub fn set_local_player_index(&mut self, index: PlayerIndex) {
        self.local_player_index = index;
    }

    pub fn get_local_player_index(&self) -> PlayerIndex {
        self.local_player_index
    }

    pub fn get_local_player(&self) -> Option<&Arc<RwLock<Player>>> {
        if self.local_player_index != PLAYER_INDEX_INVALID {
            self.get_player(self.local_player_index)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.players.clear();
        self.local_player_index = PLAYER_INDEX_INVALID;
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Arc<RwLock<Player>>> {
        self.players.iter()
    }

    pub fn get_neutral_player(&self) -> Option<Arc<RwLock<Player>>> {
        self.players.iter().find_map(|player| {
            let guard = player.read().ok()?;
            if guard.get_player_type() == PlayerType::Neutral {
                Some(Arc::clone(player))
            } else {
                None
            }
        })
    }

    /// Find a player by name key (from player name)
    /// Matches C++ PlayerList::findPlayerWithNameKey()
    pub fn find_player_by_name(&self, name: &str) -> Option<Arc<RwLock<Player>>> {
        let key = NameKeyGenerator::name_to_key(name);
        self.players.iter().find_map(|player| {
            let guard = player.read().ok()?;
            if guard.get_player_name_key() == key {
                Some(Arc::clone(player))
            } else {
                None
            }
        })
    }
}

// Provide PlayerManager operations directly on PlayerList for systems that hold the list lock.
impl crate::commands::command_processor::PlayerManager for PlayerList {
    fn get_player_resources(
        &self,
        player_id: Int,
    ) -> Option<crate::commands::command_processor::PlayerResources> {
        let player_arc = self.get_player(player_id).cloned()?;
        let player = player_arc.read().ok()?;
        Some(crate::commands::command_processor::PlayerResources {
            supplies: player.get_money().get_money(),
            power_available: player.get_energy().production(),
            power_used: player.get_energy().consumption(),
        })
    }

    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int) {
        if let Some(player_arc) = self.get_player(player_id).cloned() {
            if let Ok(mut player) = player_arc.write() {
                player.get_money_mut().add_money(supplies);
                if power > 0 {
                    player.add_power_production(power);
                } else if power < 0 {
                    player.add_power_consumption(-power);
                }
            }
        }
    }

    fn can_player_afford(
        &self,
        player_id: Int,
        cost: &crate::commands::command_processor::ResourceCost,
    ) -> bool {
        if let Some(player_arc) = self.get_player(player_id).cloned() {
            if let Ok(player) = player_arc.read() {
                return player.get_money().can_afford(cost.supplies);
            }
        }
        false
    }
}

/// Global access to player list (matching C++ ThePlayerList)
pub fn player_list() -> &'static RwLock<PlayerList> {
    PLAYER_LIST.get_or_init(|| RwLock::new(PlayerList::new()))
}

/// Convenience alias for C++ compatibility
pub use player_list as ThePlayerList;

pub mod manager;
pub mod science_management;
pub mod science_ui;

// Re-export UI types for convenience
pub use science_ui::{
    LevelUpNotification, PurchasableScienceInfo, RankProgressInfo, ScienceTreeUIData,
};

// Type aliases and constants for compatibility
pub type NameKeyType = game_engine::common::thing::module::NameKeyType;

/// Extension trait for Arc<RwLock<Player>> to provide helper methods
pub trait PlayerArcExt {
    fn change_battle_plan(&self, plan_type: BattlePlanType, delta: Int, bonus: &BattlePlanBonuses);
    fn has_upgrade_complete(&self, upgrade_template: &UpgradeTemplate) -> Bool;
    fn has_upgrade_in_production(&self, upgrade_template: &UpgradeTemplate) -> Bool;
    fn add_upgrade(
        &self,
        upgrade_template: &UpgradeTemplate,
        status: crate::upgrade::UpgradeStatus,
    );
    fn remove_upgrade(&self, upgrade_template: &UpgradeTemplate);
    fn iterate_objects<F>(&self, func: F) -> Result<(), GameError>
    where
        F: FnMut(Arc<RwLock<Object>>) -> Result<(), GameError>;
    fn get_player_template(&self) -> Option<Arc<PlayerTemplate>>;
    fn allowed_to_build(&self, template: &dyn crate::common::ThingTemplate) -> Bool;
}

impl PlayerArcExt for Arc<RwLock<Player>> {
    /// Change battle plan count for this player
    fn change_battle_plan(&self, plan_type: BattlePlanType, delta: Int, bonus: &BattlePlanBonuses) {
        if let Ok(mut guard) = self.write() {
            guard.change_battle_plan(plan_type, delta, bonus);
        }
    }

    /// Check if player has upgrade complete
    fn has_upgrade_complete(&self, upgrade_template: &UpgradeTemplate) -> Bool {
        if let Ok(guard) = self.read() {
            guard.has_upgrade_complete(upgrade_template)
        } else {
            false
        }
    }

    /// Check if upgrade is in production
    fn has_upgrade_in_production(&self, upgrade_template: &UpgradeTemplate) -> Bool {
        if let Ok(guard) = self.read() {
            guard.has_upgrade_in_production(upgrade_template)
        } else {
            false
        }
    }

    /// Add upgrade to player
    /// Matches C++ Player::addUpgrade
    fn add_upgrade(
        &self,
        upgrade_template: &UpgradeTemplate,
        status: crate::upgrade::UpgradeStatus,
    ) {
        if let Ok(mut guard) = self.write() {
            // Create new upgrade instance
            let upgrade = Upgrade::new(Arc::new(upgrade_template.clone()));

            // Set the status
            let mut upgrade_mut = upgrade;
            upgrade_mut.set_status(status);

            // Get the upgrade mask bit for this upgrade
            let upgrade_name = upgrade_template.get_name();
            let mask_bit = UpgradeMaskType::from_bits_retain(
                crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str()).bits(),
            );
            // Update the appropriate mask based on status
            match status {
                crate::upgrade::UpgradeStatus::InProduction => {
                    guard.upgrades_in_progress = guard.upgrades_in_progress | mask_bit;
                }
                crate::upgrade::UpgradeStatus::Complete => {
                    guard.upgrades_completed = guard.upgrades_completed | mask_bit;
                    // Remove from in-progress if it was there
                    guard.upgrades_in_progress = guard.upgrades_in_progress & !mask_bit;
                }
                crate::upgrade::UpgradeStatus::Invalid => {
                    // Do nothing for invalid status
                }
            }

            // Add to upgrade list if not already present
            if !guard
                .upgrade_list
                .iter()
                .any(|u| u.get_template().get_name() == upgrade_template.get_name())
            {
                guard.upgrade_list.push(upgrade_mut);
            }
        }
    }

    /// Remove upgrade from player
    /// Matches C++ Player::removeUpgrade
    fn remove_upgrade(&self, upgrade_template: &UpgradeTemplate) {
        if let Ok(mut guard) = self.write() {
            // Remove from upgrade list
            let upgrade_name = upgrade_template.get_name();
            guard
                .upgrade_list
                .retain(|u| u.get_template().get_name() != upgrade_name);

            // Clear from masks
            let mask_bit = UpgradeMaskType::from_bits_retain(
                crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str()).bits(),
            );
            guard.upgrades_in_progress = guard.upgrades_in_progress & !mask_bit;
            guard.upgrades_completed = guard.upgrades_completed & !mask_bit;
        }
    }

    /// Iterate over the objects owned by this player
    fn iterate_objects<F>(&self, mut func: F) -> Result<(), GameError>
    where
        F: FnMut(Arc<RwLock<Object>>) -> Result<(), GameError>,
    {
        if let Ok(guard) = self.read() {
            // Get all objects owned by this player from the object manager
            let obj_manager = get_object_manager();
            if let Ok(manager) = obj_manager.read() {
                let object_ids =
                    manager.get_objects_owned_by_player(guard.player_index as UnsignedInt);

                // Iterate through each object and call the function
                for obj_id in object_ids {
                    if let Some(obj_arc) = manager.get_object(obj_id) {
                        // Call the function with the object
                        if let Ok(obj_instance) = obj_arc.read() {
                            let base_obj = obj_instance.base.clone();
                            func(base_obj)?;
                        }
                    }
                }
            }
            Ok(())
        } else {
            Err(GameLogicError::LockError)
        }
    }

    /// Get the player template
    fn get_player_template(&self) -> Option<Arc<PlayerTemplate>> {
        if let Ok(guard) = self.read() {
            guard.get_player_template().cloned()
        } else {
            None
        }
    }

    /// Check if player is allowed to build the given template
    fn allowed_to_build(&self, template: &dyn crate::common::ThingTemplate) -> Bool {
        if let Ok(guard) = self.read() {
            guard.can_build_template(template)
        } else {
            false
        }
    }
}

// Note: rhai::Locked<T> is an alias for RwLock<T>, so the impl above covers both
