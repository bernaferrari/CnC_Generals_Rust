//! Academy Statistics System
//!
//! Keeps track of various statistics in order to provide advice to
//! the player about how to improve playing. This system tracks player
//! behavior and provides tiered advice based on skill level.
//!
//! Based on C++ AcademyStats.cpp/h from GeneralsMD codebase.

use crate::common::time;

use super::handles::{CommandSetHandle, FrameNumber, PlayerHandle, ThingTemplateHandle};

/// Maximum number of advice tips to provide at once (C++ AcademyStats.h:39)
pub const MAX_ADVICE_TIPS: usize = 1;

/// Frames between updates (C++ AcademyStats.cpp:55)
const FRAMES_BETWEEN_UPDATES: u32 = 30;

/// Logic frames per second (C++ GameCommon.h)
const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Academy advice information structure
#[derive(Debug, Clone)]
pub struct AcademyAdviceInfo {
    /// Array of advice strings
    pub advice: [String; MAX_ADVICE_TIPS],
    /// Number of active tips
    pub num_tips: u32,
}

impl AcademyAdviceInfo {
    pub fn new() -> Self {
        Self {
            advice: Default::default(),
            num_tips: 0,
        }
    }

    pub fn add_tip(&mut self, tip: String) {
        if (self.num_tips as usize) < MAX_ADVICE_TIPS {
            self.advice[self.num_tips as usize] = tip;
            self.num_tips += 1;
        }
    }

    pub fn clear(&mut self) {
        for advice in &mut self.advice {
            advice.clear();
        }
        self.num_tips = 0;
    }
}

impl Default for AcademyAdviceInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Academy classification types for advice categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcademyClassificationType {
    None,
    UpgradeRadar,
    Superpower,
}

impl AcademyClassificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::UpgradeRadar => "UPGRADE_RADAR",
            Self::Superpower => "SUPERPOWER",
        }
    }
}

/// Academy statistics tracker
///
/// Tracks various player behaviors and provides advice for improvement.
/// Organizes advice into three tiers: basic, intermediate, and advanced.
#[derive(Debug)]
pub struct AcademyStats {
    /// Player this stats tracker belongs to
    player: PlayerHandle,
    /// Frame number for next update
    next_update_frame: FrameNumber,
    /// Last frame any academy-relevant event was recorded
    last_event_frame: FrameNumber,
    /// Whether this is the first update
    first_update: bool,
    /// Dozer command set for analysis
    dozer_command_set: CommandSetHandle,
    /// Whether the side is unknown
    unknown_side: bool,
    /// Command center template for this player's faction
    command_center_template: ThingTemplateHandle,

    // Tier 1 (Basic advice) statistics
    spent_cash_before_building_supply_center: bool,
    supply_centers_built: u32,
    supply_center_template: ThingTemplateHandle,
    supply_center_cost: u32,

    researched_radar: bool,
    peons_built: u32,
    structures_captured: u32,
    generals_points_spent: u32,
    special_powers_used: u32,
    structures_garrisoned: u32,

    idle_building_units_max_frames: u32,
    last_unit_built_frame: u32,
    drag_select_units: u32,
    upgrades_purchased: u32,

    power_out_max_frames: u32,
    oldest_power_out_frame: u32,
    had_power_last_check: bool,

    gatherers_built: u32,
    heroes_built: u32,

    // Tier 2 (Intermediate advice) statistics
    had_a_strategy_center: bool,
    chose_a_strategy_for_center: bool,
    units_entered_tunnel_network: u32,
    had_a_tunnel_network: bool,
    control_groups_used: u32,
    secondary_income_units_built: u32,
    cleared_garrisoned_buildings: u32,
    salvage_collected: u32,
    guard_ability_used_count: u32,

    // Tier 3 (Advanced advice) statistics
    double_click_attack_move_orders_given: u32,
    built_barracks_within_five_minutes: bool,
    built_war_factory_within_ten_minutes: bool,
    built_tech_structure_within_fifteen_minutes: bool,
    last_income_frame: u32,
    max_frames_between_income: u32,

    // Neutral player stats (special tracking)
    mines: u32,
    mines_cleared: u32,
    vehicles_recovered: u32,
    vehicles_sniped: u32,
    disguisable_vehicles_built: u32,
    vehicles_disguised: u32,
    firestorms_created: u32,
}

impl AcademyStats {
    /// Create a new AcademyStats instance
    pub fn new() -> Self {
        Self {
            player: PlayerHandle::INVALID,
            next_update_frame: 0,
            last_event_frame: 0,
            first_update: true,
            dozer_command_set: CommandSetHandle::INVALID,
            unknown_side: false,
            command_center_template: ThingTemplateHandle::INVALID,

            spent_cash_before_building_supply_center: false,
            supply_centers_built: 0,
            supply_center_template: ThingTemplateHandle::INVALID,
            supply_center_cost: 0,

            researched_radar: false,
            peons_built: 0,
            structures_captured: 0,
            generals_points_spent: 0,
            special_powers_used: 0,
            structures_garrisoned: 0,

            idle_building_units_max_frames: 0,
            last_unit_built_frame: 0,
            drag_select_units: 0,
            upgrades_purchased: 0,

            power_out_max_frames: 0,
            oldest_power_out_frame: 0,
            had_power_last_check: false,

            gatherers_built: 0,
            heroes_built: 0,

            had_a_strategy_center: false,
            chose_a_strategy_for_center: false,
            units_entered_tunnel_network: 0,
            had_a_tunnel_network: false,
            control_groups_used: 0,
            secondary_income_units_built: 0,
            cleared_garrisoned_buildings: 0,
            salvage_collected: 0,
            guard_ability_used_count: 0,

            double_click_attack_move_orders_given: 0,
            built_barracks_within_five_minutes: false,
            built_war_factory_within_ten_minutes: false,
            built_tech_structure_within_fifteen_minutes: false,
            last_income_frame: 0,
            max_frames_between_income: 0,

            mines: 0,
            mines_cleared: 0,
            vehicles_recovered: 0,
            vehicles_sniped: 0,
            disguisable_vehicles_built: 0,
            vehicles_disguised: 0,
            firestorms_created: 0,
        }
    }

    fn mark_event(&mut self) {
        self.last_event_frame = time::frame();
    }

    pub fn last_event_frame(&self) -> u32 {
        self.last_event_frame
    }

    /// Initialize for a specific player
    /// Based on C++ AcademyStats.cpp:78-261
    pub fn init(&mut self, player: PlayerHandle) {
        // C++ line 85-86: Set next update frame
        self.next_update_frame = time::frame() + FRAMES_BETWEEN_UPDATES;
        self.first_update = true;
        self.unknown_side = false;

        // C++ line 90: Init the player
        self.player = player;

        // Note: Template initialization (C++ lines 91-135) would require PlayerTemplate
        // and CommandSet systems to be available. For now, we initialize with defaults.
        // When these systems are available, we should:
        // 1. Get player template
        // 2. Check if side is USA/China/GLA (otherwise set unknown_side = true)
        // 3. Find dozer command set
        // 4. Extract supply center and command center templates from command set

        self.dozer_command_set = CommandSetHandle::INVALID;
        self.command_center_template = ThingTemplateHandle::INVALID;
        self.supply_center_template = ThingTemplateHandle::INVALID;

        // C++ line 147-150: Default supply center cost
        self.supply_center_cost = 1000;

        // Tier 1 (Basic advice) - C++ lines 145-189
        self.spent_cash_before_building_supply_center = false;
        self.supply_centers_built = 0;
        self.researched_radar = false;
        self.peons_built = 0;
        self.structures_captured = 0;
        self.generals_points_spent = 0;
        self.special_powers_used = 0;
        self.structures_garrisoned = 0;
        self.idle_building_units_max_frames = 0;
        self.last_unit_built_frame = 0;
        self.drag_select_units = 0;
        self.upgrades_purchased = 0;
        self.power_out_max_frames = 0;
        self.oldest_power_out_frame = 0;
        self.had_power_last_check = false;
        self.gatherers_built = 0;
        self.heroes_built = 0;

        // Tier 2 (Intermediate advice) - C++ lines 195-220
        self.had_a_strategy_center = false;
        self.chose_a_strategy_for_center = false;
        self.had_a_tunnel_network = false;
        self.units_entered_tunnel_network = 0;
        self.control_groups_used = 0;
        self.secondary_income_units_built = 0;
        self.cleared_garrisoned_buildings = 0;
        self.salvage_collected = 0;
        self.guard_ability_used_count = 0;

        // Tier 3 (Advanced advice) - C++ lines 229-261
        self.double_click_attack_move_orders_given = 0;
        self.built_barracks_within_five_minutes = false;
        self.built_war_factory_within_ten_minutes = false;
        self.built_tech_structure_within_fifteen_minutes = false;
        self.last_income_frame = 0;
        self.max_frames_between_income = 0;
        self.mines = 0;
        self.mines_cleared = 0;
        self.vehicles_recovered = 0;
        self.vehicles_sniped = 0;
        self.disguisable_vehicles_built = 0;
        self.vehicles_disguised = 0;
        self.firestorms_created = 0;
    }

    pub fn set_player_handle(&mut self, player: PlayerHandle) {
        self.player = player;
    }

    /// Update statistics (called periodically)
    /// Based on C++ AcademyStats.cpp:279-339
    pub fn update(&mut self) {
        // C++ line 281-284: Early exit for unknown side
        if self.unknown_side {
            return;
        }

        let now = time::frame();

        // C++ line 288: Check if it's time to update
        if self.next_update_frame >= now {
            self.next_update_frame = now + FRAMES_BETWEEN_UPDATES;

            // Note: C++ line 291 iterates over player objects to call updateAcademyStats
            // This would require object iteration system to be available.
            // The callback (C++ lines 264-276) would call recordProduction on first update.

            // C++ lines 293-305: Check if player ran out of money before building supply center
            if self.supply_centers_built == 0 && !self.spent_cash_before_building_supply_center {
                // Note: Would need Money system to check actual cash amount
                // For now, we skip this check as it requires external dependency
                // When Money system is available:
                // if money.count_money() < self.supply_center_cost {
                //     self.spent_cash_before_building_supply_center = true;
                // }
            }

            // C++ lines 307-331: Track power outage duration
            // Note: Would need Energy system to check power status
            // When Energy system is available, implement power tracking:
            // let has_power = energy.has_sufficient_power();
            // if has_power != self.had_power_last_check {
            //     if !has_power {
            //         self.oldest_power_out_frame = now;
            //     } else {
            //         let frames = now - self.oldest_power_out_frame;
            //         if frames > self.power_out_max_frames {
            //             self.power_out_max_frames = frames;
            //         }
            //     }
            //     self.had_power_last_check = has_power;
            // }

            // C++ line 333-336: Clear first update flag
            if self.is_first_update() {
                self.set_first_update(false);
            }
        }
    }

    /// Check if this is the first update
    pub fn is_first_update(&self) -> bool {
        self.first_update
    }

    /// Set the first update flag
    pub fn set_first_update(&mut self, set: bool) {
        self.first_update = set;
    }

    // Recording methods for various game events

    /// Record that an object was produced
    /// Based on C++ AcademyStats.cpp:342-441
    ///
    /// # Arguments
    /// * `kindof_flags` - KINDOF flags from the produced object (bitmask)
    /// * `has_tunnel_contain` - Whether object has tunnel contain module
    ///
    /// Note: Original C++ takes Object* and checks isKindOf() and getContain().
    /// We simplify by accepting pre-computed flags since Object system isn't available yet.
    pub fn record_production(&mut self, kindof_flags: u64, has_tunnel_contain: bool) {
        self.mark_event();
        let now = time::frame();

        // KINDOF flag constants (these should match C++ KINDOF definitions)
        const KINDOF_FS_SUPPLY_CENTER: u64 = 1 << 0;
        const KINDOF_DOZER: u64 = 1 << 1;
        const KINDOF_INFANTRY: u64 = 1 << 2;
        const KINDOF_VEHICLE: u64 = 1 << 3;
        const KINDOF_HARVESTER: u64 = 1 << 4;
        const KINDOF_HERO: u64 = 1 << 5;
        const KINDOF_FS_STRATEGY_CENTER: u64 = 1 << 6;
        const KINDOF_MONEY_HACKER: u64 = 1 << 7;
        const KINDOF_FS_BLACK_MARKET: u64 = 1 << 8;
        const KINDOF_FS_SUPPLY_DROPZONE: u64 = 1 << 9;
        const KINDOF_FS_BARRACKS: u64 = 1 << 10;
        const KINDOF_FS_WARFACTORY: u64 = 1 << 11;
        const KINDOF_FS_ADVANCED_TECH: u64 = 1 << 12;
        const KINDOF_DISGUISER: u64 = 1 << 13;

        // C++ lines 346-351: Track supply centers built
        if (kindof_flags & KINDOF_FS_SUPPLY_CENTER) != 0 {
            self.supply_centers_built += 1;
        }

        // C++ lines 353-357: Track dozers/workers built
        if (kindof_flags & KINDOF_DOZER) != 0 {
            self.peons_built += 1;
        }

        // C++ lines 359-376: Track military unit production idle time
        if ((kindof_flags & KINDOF_INFANTRY) != 0 || (kindof_flags & KINDOF_VEHICLE) != 0)
            && (kindof_flags & KINDOF_DOZER) == 0
            && (kindof_flags & KINDOF_HARVESTER) == 0
        {
            // How long has it been since we built our last unit?
            let idle_frames = if self.last_unit_built_frame > 0 {
                now - self.last_unit_built_frame
            } else {
                0
            };

            // If it was longer than our max time, record it
            if idle_frames > self.idle_building_units_max_frames {
                self.idle_building_units_max_frames = idle_frames;
            }

            // Record the frame we built our unit
            self.last_unit_built_frame = now;
        }

        // C++ lines 378-382: Track extra gatherers built
        if (kindof_flags & KINDOF_HARVESTER) != 0 {
            self.gatherers_built += 1;
        }

        // C++ lines 384-388: Track heroes built
        if (kindof_flags & KINDOF_HERO) != 0 {
            self.heroes_built += 1;
        }

        // C++ lines 390-394: Track strategy center
        if (kindof_flags & KINDOF_FS_STRATEGY_CENTER) != 0 {
            self.had_a_strategy_center = true;
        }

        // C++ lines 396-400: Track tunnel network
        if has_tunnel_contain {
            self.had_a_tunnel_network = true;
        }

        // C++ lines 402-406: Track secondary income buildings
        if (kindof_flags & KINDOF_MONEY_HACKER) != 0
            || (kindof_flags & KINDOF_FS_BLACK_MARKET) != 0
            || (kindof_flags & KINDOF_FS_SUPPLY_DROPZONE) != 0
        {
            self.secondary_income_units_built += 1;
        }

        // C++ lines 408-415: Track barracks built within 5 minutes
        if (kindof_flags & KINDOF_FS_BARRACKS) != 0 {
            if time::frame() <= 300 * LOGICFRAMES_PER_SECOND {
                self.built_barracks_within_five_minutes = true;
            }
        }

        // C++ lines 417-424: Track war factory built within 10 minutes
        if (kindof_flags & KINDOF_FS_WARFACTORY) != 0 {
            if time::frame() <= 600 * LOGICFRAMES_PER_SECOND {
                self.built_war_factory_within_ten_minutes = true;
            }
        }

        // C++ lines 426-433: Track tech structure built within 15 minutes
        if (kindof_flags & KINDOF_FS_ADVANCED_TECH) != 0 {
            if time::frame() <= 900 * LOGICFRAMES_PER_SECOND {
                self.built_tech_structure_within_fifteen_minutes = true;
            }
        }

        // C++ lines 435-439: Track disguisable vehicles
        if (kindof_flags & KINDOF_DISGUISER) != 0 {
            self.disguisable_vehicles_built += 1;
        }
    }

    /// Record that an upgrade was purchased
    /// Based on C++ AcademyStats.cpp:444-457
    ///
    /// # Arguments
    /// * `classification_type` - The academy classification type of the upgrade
    /// * `granted` - Whether the upgrade was granted (true) or purchased (false)
    pub fn record_upgrade(
        &mut self,
        classification_type: AcademyClassificationType,
        granted: bool,
    ) {
        self.mark_event();

        // C++ lines 446-450: Check if this is a radar upgrade
        if classification_type == AcademyClassificationType::UpgradeRadar {
            self.researched_radar = true;
        }

        // C++ lines 452-456: Only count purchased upgrades (not granted ones)
        if !granted {
            self.upgrades_purchased += 1;
        }
    }

    /// Record that a special power was used
    /// Based on C++ AcademyStats.cpp:460-466
    ///
    /// # Arguments
    /// * `classification_type` - The academy classification type of the special power
    pub fn record_special_power_used(&mut self, classification_type: AcademyClassificationType) {
        self.mark_event();

        // C++ lines 462-465: Only count superpowers
        if classification_type == AcademyClassificationType::Superpower {
            self.special_powers_used += 1;
        }
    }

    /// Record income received
    /// Based on C++ AcademyStats.cpp:469-480
    pub fn record_income(&mut self) {
        self.mark_event();
        let now = time::frame();

        // C++ line 473: Calculate delta from last income
        let delta = if self.last_income_frame > 0 {
            now.saturating_sub(self.last_income_frame)
        } else {
            0
        };

        // C++ lines 474-477: Track max time between income
        if delta > self.max_frames_between_income {
            self.max_frames_between_income = delta;
        }

        // C++ line 479: Update last income frame
        self.last_income_frame = now;
    }

    // Simple recording methods (these increment counters)

    pub fn record_building_capture(&mut self) {
        self.mark_event();
        self.structures_captured += 1;
    }

    pub fn record_generals_points_spent(&mut self, points: i32) {
        self.mark_event();
        self.generals_points_spent += points as u32;
    }

    pub fn record_building_garrisoned(&mut self) {
        self.mark_event();
        self.structures_garrisoned += 1;
    }

    pub fn record_drag_selection(&mut self) {
        self.mark_event();
        self.drag_select_units += 1;
    }

    pub fn record_strategy_center(&mut self) {
        self.mark_event();
        self.had_a_strategy_center = true;
    }

    pub fn record_battle_plan_selected(&mut self) {
        self.mark_event();
        self.chose_a_strategy_for_center = true;
    }

    pub fn record_unit_entered_tunnel_network(&mut self) {
        self.mark_event();
        self.units_entered_tunnel_network += 1;
    }

    pub fn record_control_groups_used(&mut self) {
        self.mark_event();
        self.control_groups_used += 1;
    }

    pub fn record_cleared_garrisoned_building(&mut self) {
        self.mark_event();
        self.cleared_garrisoned_buildings += 1;
    }

    pub fn record_vehicle_disguised(&mut self) {
        self.mark_event();
        self.vehicles_disguised += 1;
    }

    pub fn record_firestorm_created(&mut self) {
        self.mark_event();
        self.firestorms_created += 1;
    }

    pub fn record_guard_ability_used(&mut self) {
        self.mark_event();
        self.guard_ability_used_count += 1;
    }

    pub fn record_salvage_collected(&mut self) {
        self.mark_event();
        self.salvage_collected += 1;
    }

    pub fn record_double_click_attack_move_order_given(&mut self) {
        self.mark_event();
        self.double_click_attack_move_orders_given += 1;
    }

    pub fn record_mine_cleared(&mut self) {
        self.mark_event();
        self.mines_cleared += 1;
    }

    // Methods for neutral player tracking

    pub fn record_vehicle_sniped(&mut self) {
        self.mark_event();
        self.vehicles_sniped += 1;
    }

    pub fn get_vehicles_sniped(&self) -> u32 {
        self.vehicles_sniped
    }

    pub fn record_mine(&mut self) {
        self.mark_event();
        self.mines += 1;
    }

    pub fn get_mines(&self) -> u32 {
        self.mines
    }

    // Query methods

    pub fn get_player(&self) -> PlayerHandle {
        self.player
    }

    pub fn had_a_supply_center(&self) -> bool {
        self.supply_centers_built > 0
    }

    pub fn get_command_center_template(&self) -> ThingTemplateHandle {
        self.command_center_template
    }

    /// Calculate and provide academy advice based on player statistics
    /// Based on C++ AcademyStats.cpp:1034-1082
    pub fn calculate_academy_advice(&self, info: &mut AcademyAdviceInfo) -> bool {
        // C++ lines 1044-1047: Early exit for unknown side
        if self.unknown_side {
            return false;
        }

        // C++ lines 1049-1063: Initialize advice info
        info.clear();
        for _advice in &mut info.advice.iter_mut() {
            // C++ line 1062: Build header for each string (empty for now, would be "\n\n")
        }

        // C++ lines 1065-1078: Evaluate all tiers progressively
        self.evaluate_tier1_advice(info, -1);

        if (info.num_tips as usize) < MAX_ADVICE_TIPS {
            self.evaluate_tier2_advice(info, -1);

            if (info.num_tips as usize) < MAX_ADVICE_TIPS {
                self.evaluate_tier3_advice(info, -1);
            }
        }

        // C++ line 1081: Return whether we have any advice to give
        info.num_tips > 0
    }

    // Private helper methods for advice evaluation

    /// Evaluate tier 1 (basic) advice
    /// Based on C++ AcademyStats.cpp:483-705
    fn evaluate_tier1_advice(&self, info: &mut AcademyAdviceInfo, num_available_tips: i32) {
        let max_advice_tips = MAX_ADVICE_TIPS;
        let mut _num_available = num_available_tips;
        let choosing = num_available_tips != -1;
        let mut available_tips = 0;

        // Note: C++ uses GameClientRandomValue for random selection.
        // For now, we implement deterministic selection (first available tip).
        // When random system is available, use: rand::thread_rng().gen_range(0, num_available)

        // C++ lines 502-514: Advice #2 - Ran out of money before building supply center
        if self.spent_cash_before_building_supply_center {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildSupplyCenterEarlier".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 516-528: Advice #3 - Build radar
        if !self.researched_radar {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:TryBuildingRadar".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 530-542: Advice #4 - Build more dozers/workers
        if self.peons_built < 2 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildMorePeons".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 544-556: Advice #5 - Capture structures
        if self.structures_captured == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:TryCapturingStructures".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 558-570: Advice #6 - Spend generals points
        if self.generals_points_spent == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:SpendGeneralsPoints".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 572-584: Advice #7 - Use special powers
        if self.special_powers_used == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:TryUsingSuperweapons".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 586-598: Advice #8 - Garrison structures
        if self.structures_garrisoned == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:TryGarrisoningAStructure".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 600-617: Advice #9 - Build military units more frequently
        let now = time::frame();
        let idle_frames = now.saturating_sub(self.last_unit_built_frame);
        let mut max_idle = self.idle_building_units_max_frames;
        if idle_frames > max_idle {
            max_idle = idle_frames;
        }
        if max_idle > 300 * LOGICFRAMES_PER_SECOND || self.last_unit_built_frame == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:IdleBuildingUnits".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 619-631: Advice #10 - Drag select units
        if self.drag_select_units == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:TryDragSelectingUnits".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 633-645: Advice #11 - Research upgrades
        if self.upgrades_purchased == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:ResearchUpgrades".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 647-668: Advice #12 - Ran out of power for too long
        let mut max_power_out = self.power_out_max_frames;
        if !self.had_power_last_check {
            let frames = now.saturating_sub(self.oldest_power_out_frame);
            if frames > max_power_out {
                max_power_out = frames;
            }
        }
        if max_power_out > 600 * LOGICFRAMES_PER_SECOND {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:RanOutOfPower".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 670-683: Advice #13 - Build more gatherers
        if self.gatherers_built == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildMoreGatherers".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 685-697: Advice #14 - Build a hero
        if self.heroes_built == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildAHero".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 699-704: Recursive call to randomly choose if we were just counting
        if !choosing && available_tips > 0 {
            self.evaluate_tier1_advice(info, available_tips);
        }
    }

    /// Evaluate tier 2 (intermediate) advice
    /// Based on C++ AcademyStats.cpp:708-851
    fn evaluate_tier2_advice(&self, info: &mut AcademyAdviceInfo, num_available_tips: i32) {
        let max_advice_tips = MAX_ADVICE_TIPS;
        let mut _num_available = num_available_tips;
        let choosing = num_available_tips != -1;
        let mut available_tips = 0;

        // C++ lines 723-735: Advice #15 - Select a strategy center battle plan
        if self.had_a_strategy_center && !self.chose_a_strategy_for_center {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:PickStrategyCenterPlan".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 737-749: Advice #16 - Use tunnel network
        if self.had_a_tunnel_network && self.units_entered_tunnel_network == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:UseTunnelNetwork".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 751-763: Advice #17 - Use control groups
        if self.control_groups_used == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:UseControlGroups".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 765-777: Advice #18 - Build secondary income buildings
        if self.secondary_income_units_built == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:UseSecondaryIncomeMethods".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 779-791: Advice #19 - Clear garrisoned buildings
        if self.cleared_garrisoned_buildings == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:ClearBuildings".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 793-808: Advice #20 - Pick up salvage (GLA only)
        // Note: Would need PlayerTemplate to check if player is GLA
        // For now, we skip faction-specific checks
        if self.salvage_collected == 0 {
            // Only show for GLA players when faction system is available
        }

        // C++ lines 810-822: Advice #21 - Use guard ability
        if self.guard_ability_used_count == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:UseGuardAbility".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 824-836: Advice #22 - Build multiple supply centers
        if self.supply_centers_built < 2 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:MultipleSupplyCenters".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 844-849: Recursive call to randomly choose if we were just counting
        if !choosing && available_tips > 0 {
            self.evaluate_tier2_advice(info, available_tips);
        }
    }

    /// Evaluate tier 3 (advanced) advice
    /// Based on C++ AcademyStats.cpp:854-1031
    fn evaluate_tier3_advice(&self, info: &mut AcademyAdviceInfo, num_available_tips: i32) {
        let max_advice_tips = MAX_ADVICE_TIPS;
        let mut _num_available = num_available_tips;
        let choosing = num_available_tips != -1;
        let mut available_tips = 0;
        let now = time::frame();

        // C++ lines 871-883: Advice #25 - Use alternate mouse interface
        // Note: Would need TheGlobalData->m_useAlternateMouse
        // Skipped for now as it requires global data system

        // C++ lines 885-897: Advice #26 - Use double-click attack move/guard
        if self.double_click_attack_move_orders_given == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:DoubleClickAttackMoveGuard".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 899-911: Advice #27 - Build barracks sooner
        if !self.built_barracks_within_five_minutes {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildBarracksSooner".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 913-925: Advice #28 - Build war factory sooner
        if !self.built_war_factory_within_ten_minutes {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildWarFactorySooner".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 927-939: Advice #29 - Build tech structure sooner
        if !self.built_tech_structure_within_fifteen_minutes {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:BuildTechStructureSooner".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 941-958: Advice #30 - No income for too long
        let delta = now.saturating_sub(self.last_income_frame);
        let mut max_between_income = self.max_frames_between_income;
        if delta > max_between_income {
            max_between_income = delta;
        }
        if max_between_income > LOGICFRAMES_PER_SECOND * 120 || self.last_income_frame == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:NoIncome".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 960-972: Advice #31 - Clear mines with dozers
        // Note: Would need ThePlayerList->getLocalPlayer()->getAcademyStats()->getMines()
        // Skipped for now as it requires player list system

        // C++ lines 974-989: Advice #32 - Capture sniped vehicles
        // Note: Would need ThePlayerList->getNeutralPlayer()->getAcademyStats()->getVehiclesSniped()
        // Skipped for now as it requires player list system

        // C++ lines 991-1006: Advice #33 - Use disguise ability
        if self.disguisable_vehicles_built > 0 && self.vehicles_disguised == 0 {
            available_tips += 1;
            if choosing && (info.num_tips as usize) < max_advice_tips {
                info.add_tip("ACADEMY:DisguisedUnits".to_string());
            }
            _num_available -= 1;
        }

        // C++ lines 1008-1023: Advice #35 - Create firestorms (China only)
        // Note: Would need PlayerTemplate to check if player is China
        // For now, we skip faction-specific checks
        if self.firestorms_created == 0 {
            // Only show for China players when faction system is available
        }

        // C++ lines 1025-1030: Recursive call to randomly choose if we were just counting
        if !choosing && available_tips > 0 {
            self.evaluate_tier3_advice(info, available_tips);
        }
    }
}

impl Default for AcademyStats {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization support (for save/load functionality)
// Based on C++ AcademyStats.cpp:1087-1245

impl AcademyStats {
    /// Calculate CRC for network synchronization
    /// Based on C++ AcademyStats.cpp:1087-1090
    pub fn crc(&self) -> u32 {
        // C++ implementation is empty, so we return 0
        // When full CRC system is available, this should compute
        // a checksum over all stats fields
        0
    }

    /// Serialize/deserialize academy stats
    /// Based on C++ AcademyStats.cpp:1097-1237
    ///
    /// This would be used for save/load functionality.
    /// In the C++ version, this uses an Xfer object that handles
    /// bidirectional serialization (can read or write).
    ///
    /// Fields are serialized in version 1 format (C++ line 1101-1102).
    /// All statistics are saved to preserve player advice state across
    /// game sessions.
    ///
    /// When a full serialization system is available, implement:
    /// - Version header (currentVersion = 1)
    /// - All tier 1, tier 2, and tier 3 statistics
    /// - Frame counters and timestamps
    /// - Boolean flags
    pub fn serialize(&self) -> Vec<u8> {
        // Placeholder for serialization
        // When implemented, should serialize all fields in order:
        // 1. Version number
        // 2. m_nextUpdateFrame, m_firstUpdate, m_unknownSide
        // 3. All Tier 1 stats (lines 1117-1158)
        // 4. All Tier 2 stats (lines 1164-1186)
        // 5. All Tier 3 stats (lines 1204-1235)
        Vec::new()
    }

    /// Deserialize academy stats from bytes
    /// Based on C++ AcademyStats.cpp:1097-1237
    pub fn deserialize(&mut self, _data: &[u8]) {
        // Placeholder for deserialization
        // When implemented, should restore all fields from serialized data
    }

    /// Post-process after loading from save file
    /// Based on C++ AcademyStats.cpp:1242-1245
    pub fn load_post_process(&mut self) {
        // C++ implementation is empty
        // This hook is available for any initialization needed
        // after deserializing from a save file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_academy_stats_creation() {
        let stats = AcademyStats::new();

        assert!(stats.is_first_update());
        assert_eq!(stats.get_vehicles_sniped(), 0);
        assert_eq!(stats.get_mines(), 0);
        assert!(!stats.had_a_supply_center());
    }

    #[test]
    fn test_academy_stats_init() {
        let mut stats = AcademyStats::new();
        let player = PlayerHandle::new(1);

        stats.init(player);

        assert_eq!(stats.get_player(), player);
        assert!(stats.is_first_update());
        assert_eq!(stats.supply_centers_built, 0);
        assert_eq!(stats.supply_center_cost, 1000);
        assert!(!stats.researched_radar);
        assert_eq!(stats.peons_built, 0);
    }

    #[test]
    fn test_academy_stats_recording() {
        let mut stats = AcademyStats::new();

        // Test basic recording
        stats.record_building_capture();
        assert_eq!(stats.structures_captured, 1);

        stats.record_generals_points_spent(5);
        assert_eq!(stats.generals_points_spent, 5);

        stats.record_vehicle_sniped();
        assert_eq!(stats.get_vehicles_sniped(), 1);

        stats.record_mine();
        assert_eq!(stats.get_mines(), 1);

        stats.record_building_garrisoned();
        assert_eq!(stats.structures_garrisoned, 1);

        stats.record_drag_selection();
        assert_eq!(stats.drag_select_units, 1);
    }

    #[test]
    fn test_production_tracking() {
        let mut stats = AcademyStats::new();

        // Supply center
        const KINDOF_FS_SUPPLY_CENTER: u64 = 1 << 0;
        stats.record_production(KINDOF_FS_SUPPLY_CENTER, false);
        assert_eq!(stats.supply_centers_built, 1);
        assert!(stats.had_a_supply_center());

        // Dozer
        const KINDOF_DOZER: u64 = 1 << 1;
        stats.record_production(KINDOF_DOZER, false);
        assert_eq!(stats.peons_built, 1);

        // Hero
        const KINDOF_HERO: u64 = 1 << 5;
        stats.record_production(KINDOF_HERO, false);
        assert_eq!(stats.heroes_built, 1);
    }

    #[test]
    fn test_upgrade_recording() {
        let mut stats = AcademyStats::new();

        // Record radar upgrade
        stats.record_upgrade(AcademyClassificationType::UpgradeRadar, false);
        assert!(stats.researched_radar);
        assert_eq!(stats.upgrades_purchased, 1);

        // Record granted upgrade (shouldn't count as purchased)
        stats.record_upgrade(AcademyClassificationType::None, true);
        assert_eq!(stats.upgrades_purchased, 1);

        // Record normal upgrade
        stats.record_upgrade(AcademyClassificationType::None, false);
        assert_eq!(stats.upgrades_purchased, 2);
    }

    #[test]
    fn test_special_power_recording() {
        let mut stats = AcademyStats::new();

        // Record superpower
        stats.record_special_power_used(AcademyClassificationType::Superpower);
        assert_eq!(stats.special_powers_used, 1);

        // Record non-superpower (shouldn't count)
        stats.record_special_power_used(AcademyClassificationType::None);
        assert_eq!(stats.special_powers_used, 1);
    }

    #[test]
    fn test_income_tracking() {
        let mut stats = AcademyStats::new();

        stats.record_income();
        assert!(stats.last_income_frame > 0);

        // Simulate time passing
        time::advance();
        time::advance();
        stats.record_income();

        // Should have tracked the gap
        assert!(stats.max_frames_between_income > 0);
    }

    #[test]
    fn test_tier2_tracking() {
        let mut stats = AcademyStats::new();

        stats.record_strategy_center();
        assert!(stats.had_a_strategy_center);

        stats.record_battle_plan_selected();
        assert!(stats.chose_a_strategy_for_center);

        stats.record_unit_entered_tunnel_network();
        assert_eq!(stats.units_entered_tunnel_network, 1);

        stats.record_control_groups_used();
        assert_eq!(stats.control_groups_used, 1);

        stats.record_cleared_garrisoned_building();
        assert_eq!(stats.cleared_garrisoned_buildings, 1);
    }

    #[test]
    fn test_tier3_tracking() {
        let mut stats = AcademyStats::new();

        stats.record_double_click_attack_move_order_given();
        assert_eq!(stats.double_click_attack_move_orders_given, 1);

        stats.record_mine_cleared();
        assert_eq!(stats.mines_cleared, 1);

        stats.record_vehicle_disguised();
        assert_eq!(stats.vehicles_disguised, 1);

        stats.record_firestorm_created();
        assert_eq!(stats.firestorms_created, 1);
    }

    #[test]
    fn test_advice_generation() {
        let stats = AcademyStats::new();
        let mut info = AcademyAdviceInfo::new();

        // New player should get advice
        let has_advice = stats.calculate_academy_advice(&mut info);
        assert!(has_advice);
        assert!(info.num_tips > 0);
    }

    #[test]
    fn test_tier1_advice_peons() {
        let mut stats = AcademyStats::new();
        stats.unknown_side = false;
        let mut info = AcademyAdviceInfo::new();

        // Should get advice about building peons
        stats.evaluate_tier1_advice(&mut info, -1);

        // Should have at least one tip
        assert!(info.num_tips > 0);
    }

    #[test]
    fn test_tier2_advice_control_groups() {
        let mut stats = AcademyStats::new();
        stats.unknown_side = false;
        let mut info = AcademyAdviceInfo::new();

        // Should get advice about using control groups
        stats.evaluate_tier2_advice(&mut info, -1);

        assert!(info.num_tips > 0);
    }

    #[test]
    fn test_tier3_advice_timing() {
        let mut stats = AcademyStats::new();
        stats.unknown_side = false;
        let mut info = AcademyAdviceInfo::new();

        // Should get advice about building barracks sooner
        stats.evaluate_tier3_advice(&mut info, -1);

        assert!(info.num_tips > 0);
    }

    #[test]
    fn test_academy_advice_info() {
        let mut info = AcademyAdviceInfo::new();

        assert_eq!(info.num_tips, 0);

        info.add_tip("Test advice".to_string());
        assert_eq!(info.num_tips, 1);
        assert_eq!(info.advice[0], "Test advice");

        info.clear();
        assert_eq!(info.num_tips, 0);
        assert!(info.advice[0].is_empty());
    }

    #[test]
    fn test_academy_classification_type() {
        assert_eq!(AcademyClassificationType::None.as_str(), "NONE");
        assert_eq!(
            AcademyClassificationType::UpgradeRadar.as_str(),
            "UPGRADE_RADAR"
        );
        assert_eq!(AcademyClassificationType::Superpower.as_str(), "SUPERPOWER");
    }

    #[test]
    fn test_serialization_stubs() {
        let stats = AcademyStats::new();

        // Test CRC
        assert_eq!(stats.crc(), 0);

        // Test serialize
        let data = stats.serialize();
        assert!(data.is_empty()); // Placeholder implementation

        // Test load_post_process doesn't crash
        let mut stats2 = AcademyStats::new();
        stats2.load_post_process();
    }

    #[test]
    fn test_unknown_side_no_advice() {
        let mut stats = AcademyStats::new();
        stats.unknown_side = true;

        let mut info = AcademyAdviceInfo::new();
        let has_advice = stats.calculate_academy_advice(&mut info);

        // Unknown side should not generate advice
        assert!(!has_advice);
        assert_eq!(info.num_tips, 0);
    }
}
