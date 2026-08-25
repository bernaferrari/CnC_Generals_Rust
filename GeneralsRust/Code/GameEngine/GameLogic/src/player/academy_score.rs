use super::*;

#[derive(Debug, Clone)]
pub struct AcademyStats {
    pub(super) units_built: HashMap<String, Int>,
    pub(super) units_killed: HashMap<String, Int>,
    pub(super) buildings_built: HashMap<String, Int>,
    pub(super) buildings_destroyed: HashMap<String, Int>,
    /// Track total units that have entered tunnel network
    pub(super) tunnel_entries: Int,
    /// Total generals points spent on sciences.
    pub(super) generals_points_spent: Int,
    pub(super) researched_radar: Bool,
    pub(super) upgrades_purchased: Int,
    pub(super) cleared_garrisoned_buildings: Int,
    pub(super) salvage_collected: Int,
    pub(super) special_powers_used: Int,
    pub(super) vehicles_sniped: Int,
    /// C++ AcademyStats::m_vehiclesDisguised
    pub(super) vehicles_disguised: Int,
    /// Total money earned (for scoreboard / academy stats).
    pub(super) total_income: Int,
    pub(super) mines: Int,
    pub(super) mines_cleared: Int,
    /// C++ AcademyStats::m_choseAStrategyForCenter
    pub(super) chose_a_strategy_for_center: Bool,
    /// C++ AcademyStats::m_doubleClickAttackMoveOrdersGiven
    pub(super) double_click_attack_move_orders_given: Int,
    /// C++ AcademyStats::m_structuresGarrisoned
    pub(super) structures_garrisoned: Int,
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
            vehicles_sniped: 0,
            vehicles_disguised: 0,
            total_income: 0,
            mines: 0,
            mines_cleared: 0,
            chose_a_strategy_for_center: false,
            double_click_attack_move_orders_given: 0,
            structures_garrisoned: 0,
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

    /// C++ AcademyStats::m_researchedRadar.
    pub fn has_researched_radar(&self) -> bool {
        self.researched_radar
    }

    /// C++ AcademyStats::m_upgradesPurchased.
    pub fn get_upgrades_purchased(&self) -> Int {
        self.upgrades_purchased
    }

    /// Record clearing a garrisoned building (matches C++ AcademyStats::recordClearedGarrisonedBuilding).
    pub fn record_cleared_garrisoned_building(&mut self) {
        self.cleared_garrisoned_buildings += 1;
    }

    /// C++ AcademyStats::recordBuildingGarrisoned — m_structuresGarrisoned++.
    pub fn record_building_garrisoned(&mut self) {
        self.structures_garrisoned = self.structures_garrisoned.saturating_add(1);
    }

    pub fn get_structures_garrisoned(&self) -> Int {
        self.structures_garrisoned
    }

    /// Record a vehicle snipe (C++ AcademyStats::recordVehicleSniped).
    pub fn record_vehicle_sniped(&mut self) {
        self.vehicles_sniped = self.vehicles_sniped.saturating_add(1);
    }

    pub fn get_vehicles_sniped(&self) -> Int {
        self.vehicles_sniped
    }

    /// Record a vehicle disguise (C++ AcademyStats::recordVehicleDisguised).
    pub fn record_vehicle_disguised(&mut self) {
        self.vehicles_disguised = self.vehicles_disguised.saturating_add(1);
    }

    pub fn get_vehicles_disguised(&self) -> Int {
        self.vehicles_disguised
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

    /// C++ AcademyStats::m_specialPowersUsed.
    pub fn get_special_powers_used(&self) -> Int {
        self.special_powers_used
    }

    /// C++ `AcademyStats::recordMine`.
    pub fn record_mine(&mut self) {
        self.mines = self.mines.saturating_add(1);
    }

    /// Record a mine/booby-trap/demotrap disarm (C++ AcademyStats::recordMineCleared).
    pub fn record_mine_cleared(&mut self) {
        self.mines_cleared = self.mines_cleared.saturating_add(1);
    }

    /// C++ AcademyStats::recordDoubleClickAttackMoveOrderGiven.
    pub fn record_double_click_attack_move_order_given(&mut self) {
        self.double_click_attack_move_orders_given =
            self.double_click_attack_move_orders_given.saturating_add(1);
    }

    /// C++ AcademyStats::recordBattlePlanSelected — sets m_choseAStrategyForCenter.
    pub fn record_battle_plan_selected(&mut self) {
        self.chose_a_strategy_for_center = true;
    }

    /// ScoreScreen War School: leftover academy_stats stay empty unless leftover
    /// live notify (radar / sciences / upgrades / battle plan) is applied here.
    pub fn apply_live_notify_snapshot(
        &mut self,
        researched_radar: bool,
        generals_points_spent: Int,
        upgrades_purchased: Int,
        chose_a_strategy: bool,
    ) {
        if researched_radar {
            self.researched_radar = true;
        }
        if generals_points_spent > self.generals_points_spent {
            self.generals_points_spent = generals_points_spent;
        }
        if upgrades_purchased > self.upgrades_purchased {
            self.upgrades_purchased = upgrades_purchased;
        }
        if chose_a_strategy {
            self.chose_a_strategy_for_center = true;
        }
    }

    /// C++ AcademyStats::calculateAcademyAdvice — fill ScoreScreen war-school tips.
    pub fn calculate_academy_advice(
        &self,
        info: &mut game_engine::common::rts::AcademyAdviceInfo,
    ) -> bool {
        info.clear();
        if !self.researched_radar {
            info.add_tip("ACADEMY:TryBuildingRadar".to_string());
        } else if self.generals_points_spent == 0 {
            info.add_tip("ACADEMY:SpendGeneralsPoints".to_string());
        } else if self.special_powers_used == 0 {
            info.add_tip("ACADEMY:TryUsingSuperweapons".to_string());
        } else if self.upgrades_purchased == 0 {
            info.add_tip("ACADEMY:ResearchUpgrades".to_string());
        } else if self.cleared_garrisoned_buildings == 0 {
            info.add_tip("ACADEMY:ClearBuildings".to_string());
        } else if self.tunnel_entries == 0 {
            info.add_tip("ACADEMY:UseTunnelNetwork".to_string());
        } else if !self.chose_a_strategy_for_center {
            info.add_tip("ACADEMY:PickStrategyCenterPlan".to_string());
        } else if self.salvage_collected == 0 {
            info.add_tip("ACADEMY:PickUpSalvage".to_string());
        }
        info.num_tips > 0
    }
}

/// Score keeping system (matching C++ ScoreKeeper)
#[derive(Debug, Clone)]
pub struct ScoreKeeper {
    pub(super) units_built: Int,
    pub(super) units_killed: Int,
    pub(super) units_lost: Int,
    pub(super) buildings_built: Int,
    pub(super) buildings_destroyed: Int,
    pub(super) buildings_lost: Int,
    pub(super) units_destroyed_by_player: [Int; MAX_PLAYER_COUNT],
    pub(super) buildings_destroyed_by_player: [Int; MAX_PLAYER_COUNT],
    pub(super) objects_built: HashMap<String, Int>,
    pub(super) objects_destroyed: [HashMap<String, Int>; MAX_PLAYER_COUNT],
    pub(super) objects_lost: HashMap<String, Int>,
    pub(super) objects_captured: HashMap<String, Int>,
    pub(super) tech_buildings_captured: Int,
    pub(super) faction_buildings_captured: Int,
    pub(super) current_score: Int,
    pub(super) my_player_idx: PlayerIndex,
    pub(super) supplies_collected: Int,
    pub(super) supplies_spent: Int,
    pub(super) experience_points: Int,
}

impl ScoreKeeper {
    pub fn new() -> Self {
        Self::new_for_player(0)
    }

    pub fn new_for_player(player_index: PlayerIndex) -> Self {
        Self {
            units_built: 0,
            units_killed: 0,
            units_lost: 0,
            buildings_built: 0,
            buildings_destroyed: 0,
            buildings_lost: 0,
            units_destroyed_by_player: [0; MAX_PLAYER_COUNT],
            buildings_destroyed_by_player: [0; MAX_PLAYER_COUNT],
            objects_built: HashMap::new(),
            objects_destroyed: [(); MAX_PLAYER_COUNT].map(|_| HashMap::new()),
            objects_lost: HashMap::new(),
            objects_captured: HashMap::new(),
            tech_buildings_captured: 0,
            faction_buildings_captured: 0,
            current_score: 0,
            my_player_idx: player_index,
            supplies_collected: 0,
            supplies_spent: 0,
            experience_points: 0,
        }
    }

    pub(super) fn player_slot(player_index: Option<Int>) -> usize {
        match player_index {
            Some(index) if index >= 0 && (index as usize) < MAX_PLAYER_COUNT => index as usize,
            _ => 0,
        }
    }

    pub(super) fn total_units_destroyed_by_array(&self) -> Int {
        self.units_destroyed_by_player.iter().sum()
    }

    pub(super) fn total_buildings_destroyed_by_array(&self) -> Int {
        self.buildings_destroyed_by_player.iter().sum()
    }

    pub(super) fn add_unit_destroyed_for_player(&mut self, player_index: Option<Int>) {
        let slot = Self::player_slot(player_index);
        self.units_destroyed_by_player[slot] += 1;
        self.units_killed = self.total_units_destroyed_by_array();
    }

    pub(super) fn add_building_destroyed_for_player(&mut self, player_index: Option<Int>) {
        let slot = Self::player_slot(player_index);
        self.buildings_destroyed_by_player[slot] += 1;
        self.buildings_destroyed = self.total_buildings_destroyed_by_array();
    }

    pub(super) fn increment_object_count(map: &mut HashMap<String, Int>, template_name: &str) {
        let entry = map.entry(template_name.to_string()).or_insert(0);
        *entry += 1;
    }

    pub(super) fn decrement_object_count(map: &mut HashMap<String, Int>, template_name: &str) {
        let entry = map.entry(template_name.to_string()).or_insert(0);
        *entry -= 1;
    }

    pub(super) fn recompute_destroyed_aggregates(&mut self) {
        self.units_killed = self.total_units_destroyed_by_array();
        self.buildings_destroyed = self.total_buildings_destroyed_by_array();
    }

    pub(super) fn xfer_object_count_map(
        xfer: &mut dyn Xfer,
        map: &mut HashMap<String, Int>,
    ) -> Result<(), String> {
        let mut map_version: XferVersion = 1;
        xfer.xfer_version(&mut map_version, 1)
            .map_err(|e| e.to_string())?;

        let mut map_size = map.len() as u16;
        xfer.xfer_unsigned_short(&mut map_size)
            .map_err(|e| e.to_string())?;

        match xfer.get_xfer_mode() {
            XferMode::Save => {
                let mut entries: Vec<(String, Int)> = map
                    .iter()
                    .map(|(name, count)| (name.clone(), *count))
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                for (mut template_name, mut count) in entries {
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
                }
            }
            XferMode::Load => {
                map.clear();
                for _ in 0..map_size {
                    let mut template_name = String::new();
                    let mut count: Int = 0;
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
                    map.insert(template_name, count);
                }
            }
            XferMode::Crc => {
                for _ in 0..map_size {
                    let mut template_name = String::new();
                    let mut count: Int = 0;
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
                }
            }
            XferMode::Invalid => {
                return Err("ScoreKeeper::xferObjectCountMap called with invalid xfer mode".into());
            }
        }

        Ok(())
    }

    fn scoring_enabled() -> bool {
        // C++ ScoreKeeper.cpp:101 — TheGameLogic->isScoringEnabled().
        TheGameLogic::is_scoring_enabled()
    }

    /// Retail ZH KindOf.h bits used by ThingTemplate::get_kindof_mask().
    /// ScoreKindOf local discriminants are not the same positions.
    pub(super) fn score_kindof_retail_bit(kind: ScoreKindOf) -> u32 {
        match kind {
            ScoreKindOf::Structure => 7,
            ScoreKindOf::Score => 35,
            ScoreKindOf::ScoreCreate => 36,
            ScoreKindOf::ScoreDestroy => 37,
            ScoreKindOf::Infantry => 8,
            ScoreKindOf::Vehicle => 9,
            ScoreKindOf::Aircraft => 10,
        }
    }

    pub fn add_unit_built(&mut self) {
        if !Self::scoring_enabled() {
            return;
        }
        self.units_built += 1;
    }

    pub fn add_unit_killed(&mut self) {
        if !Self::scoring_enabled() {
            return;
        }
        self.add_unit_destroyed_for_player(Some(self.my_player_idx));
    }

    pub fn add_unit_lost(&mut self) {
        if !Self::scoring_enabled() {
            return;
        }
        self.units_lost += 1;
    }

    pub fn get_total_score(&self) -> Int {
        let mut score =
            self.units_built * 100 + self.supplies_collected + self.buildings_built * 100;
        for (index, units_destroyed) in self.units_destroyed_by_player.iter().enumerate() {
            if index as PlayerIndex == self.my_player_idx {
                continue;
            }
            score += *units_destroyed * 100;
            score += self.buildings_destroyed_by_player[index] * 100;
        }
        score
    }

    pub fn get_current_score(&self) -> Int {
        self.current_score
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
        self.total_units_destroyed_by_array()
    }

    pub fn get_total_units_lost(&self) -> Int {
        self.units_lost
    }

    pub fn get_total_buildings_built(&self) -> Int {
        self.buildings_built
    }

    pub fn get_total_buildings_destroyed(&self) -> Int {
        self.total_buildings_destroyed_by_array()
    }

    pub fn get_total_buildings_lost(&self) -> Int {
        self.buildings_lost
    }

    /// C++ ScoreKeeper::getTotalObjectsBuilt — count by template name.
    pub fn get_total_objects_built(&self, template_name: &str) -> Int {
        self.objects_built.get(template_name).copied().unwrap_or(0)
    }

    /// C++ ScoreKeeper::getTotalUnitsBuilt(valid, invalid) — honor filters.
    pub fn get_total_units_built_filtered(
        &self,
        valid_mask: &ScoreKindOfMaskType,
        invalid_mask: &ScoreKindOfMaskType,
    ) -> Int {
        if let Ok(factory_guard) = game_engine::common::thing::thing_factory::get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                let mut total: Int = 0;
                for (template_name, count) in &self.objects_built {
                    if let Some(template) = factory.find_template(template_name, false) {
                        if Self::kindof_matches_multi(
                            template.get_kindof_mask(),
                            valid_mask,
                            invalid_mask,
                        ) {
                            total += *count;
                        }
                    }
                }
                return total;
            }
        }
        // C++ has no units_built fallback. Factory-less tests still walk nothing.
        0
    }

    pub(super) fn kindof_matches_multi(
        template_bits: u64,
        valid_mask: &ScoreKindOfMaskType,
        invalid_mask: &ScoreKindOfMaskType,
    ) -> bool {
        const KINDS: [ScoreKindOf; 7] = [
            ScoreKindOf::Structure,
            ScoreKindOf::Score,
            ScoreKindOf::ScoreCreate,
            ScoreKindOf::ScoreDestroy,
            ScoreKindOf::Infantry,
            ScoreKindOf::Vehicle,
            ScoreKindOf::Aircraft,
        ];
        for kind in KINDS {
            let bit = 1u64 << Self::score_kindof_retail_bit(kind);
            if invalid_mask.is_set(kind) && (template_bits & bit) != 0 {
                return false;
            }
            if valid_mask.is_set(kind) && (template_bits & bit) == 0 {
                return false;
            }
        }
        true
    }

    pub(super) fn score_mask_from_retail_bits(bits: u64) -> ScoreKindOfMaskType {
        let mut mask = ScoreKindOfMaskType::new();
        const KINDS: [ScoreKindOf; 7] = [
            ScoreKindOf::Structure,
            ScoreKindOf::Score,
            ScoreKindOf::ScoreCreate,
            ScoreKindOf::ScoreDestroy,
            ScoreKindOf::Infantry,
            ScoreKindOf::Vehicle,
            ScoreKindOf::Aircraft,
        ];
        for kind in KINDS {
            if bits & (1u64 << Self::score_kindof_retail_bit(kind)) != 0 {
                mask.set(kind);
            }
        }
        mask
    }

    /// C++ addObjectBuilt when only a ThingTemplate name is available.
    pub fn add_object_built_template(&mut self, template_name: &str, retail_kindof_bits: u64) {
        if !Self::scoring_enabled() {
            return;
        }
        let mask = Self::score_mask_from_retail_bits(retail_kindof_bits);
        if Self::counts_as_score_building_create(mask) {
            self.buildings_built += 1;
            Self::increment_object_count(&mut self.objects_built, template_name);
        } else if Self::counts_as_score_unit_create(mask) {
            self.units_built += 1;
            Self::increment_object_count(&mut self.objects_built, template_name);
        }
    }

    /// C++ addObjectDestroyed when only a ThingTemplate name is available.
    pub fn add_object_destroyed_template(
        &mut self,
        template_name: &str,
        retail_kindof_bits: u64,
        owner_player_index: Int,
        under_construction: bool,
    ) {
        if !Self::scoring_enabled() || under_construction {
            return;
        }
        let mask = Self::score_mask_from_retail_bits(retail_kindof_bits);
        let slot = Self::player_slot(Some(owner_player_index));
        if Self::counts_as_score_building_destroy(mask) {
            self.add_building_destroyed_for_player(Some(owner_player_index));
            Self::increment_object_count(&mut self.objects_destroyed[slot], template_name);
        } else if Self::counts_as_score_unit_destroy(mask) {
            self.add_unit_destroyed_for_player(Some(owner_player_index));
            Self::increment_object_count(&mut self.objects_destroyed[slot], template_name);
        }
    }

    /// C++ addObjectLost when only a ThingTemplate name is available.
    pub fn add_object_lost_template(
        &mut self,
        template_name: &str,
        retail_kindof_bits: u64,
        under_construction: bool,
    ) {
        if !Self::scoring_enabled() || under_construction {
            return;
        }
        let mask = Self::score_mask_from_retail_bits(retail_kindof_bits);
        if Self::counts_as_score_building_destroy(mask) {
            self.buildings_lost += 1;
            Self::increment_object_count(&mut self.objects_lost, template_name);
        } else if Self::counts_as_score_unit_destroy(mask) {
            self.units_lost += 1;
            Self::increment_object_count(&mut self.objects_lost, template_name);
        }
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
        self.get_total_buildings_destroyed()
    }

    pub fn get_buildings_lost(&self) -> Int {
        self.buildings_lost
    }

    pub fn add_building_destroyed(&mut self) {
        if !Self::scoring_enabled() {
            return;
        }
        self.add_building_destroyed_for_player(Some(self.my_player_idx));
    }

    pub fn add_building_built(&mut self) {
        if !Self::scoring_enabled() {
            return;
        }
        self.buildings_built += 1;
    }

    pub(super) fn counts_as_score_building_create(mask: ScoreKindOfMaskType) -> bool {
        mask.is_set(ScoreKindOf::Structure)
            && (mask.is_set(ScoreKindOf::Score) || mask.is_set(ScoreKindOf::ScoreCreate))
    }

    pub(super) fn counts_as_score_unit_create(mask: ScoreKindOfMaskType) -> bool {
        (mask.is_set(ScoreKindOf::Infantry) || mask.is_set(ScoreKindOf::Vehicle))
            && (mask.is_set(ScoreKindOf::Score) || mask.is_set(ScoreKindOf::ScoreCreate))
    }

    pub(super) fn counts_as_score_building_destroy(mask: ScoreKindOfMaskType) -> bool {
        mask.is_set(ScoreKindOf::Structure)
            && (mask.is_set(ScoreKindOf::Score) || mask.is_set(ScoreKindOf::ScoreDestroy))
    }

    pub(super) fn counts_as_score_unit_destroy(mask: ScoreKindOfMaskType) -> bool {
        (mask.is_set(ScoreKindOf::Infantry) || mask.is_set(ScoreKindOf::Vehicle))
            && (mask.is_set(ScoreKindOf::Score) || mask.is_set(ScoreKindOf::ScoreDestroy))
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
        if !Self::scoring_enabled() {
            return;
        }
        // Check if under construction - under construction objects don't count
        if object.is_score_under_construction() {
            return;
        }

        // Check the KindOf mask to determine if it's a unit or building
        let mask = object.get_score_kindof_mask();
        let template_name = object.get_score_template_name();

        if Self::counts_as_score_building_destroy(mask) {
            self.buildings_lost += 1;
            Self::increment_object_count(&mut self.objects_lost, template_name);
        } else if Self::counts_as_score_unit_destroy(mask) {
            self.units_lost += 1;
            Self::increment_object_count(&mut self.objects_lost, template_name);
        }
    }

    /// Add an object that was destroyed by this player.
    /// Convenience method that extracts information from the object.
    /// C++ Reference: ScoreKeeper::addObjectDestroyed(const Object* o)
    pub fn add_object_destroyed_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        if !Self::scoring_enabled() {
            return;
        }
        // Check if under construction - under construction objects don't count
        if object.is_score_under_construction() {
            return;
        }

        // Check the KindOf mask to determine if it's a unit or building
        let mask = object.get_score_kindof_mask();
        let template_name = object.get_score_template_name();
        let slot = Self::player_slot(object.get_score_controlling_player_index());

        if Self::counts_as_score_building_destroy(mask) {
            self.add_building_destroyed_for_player(object.get_score_controlling_player_index());
            Self::increment_object_count(&mut self.objects_destroyed[slot], template_name);
        } else if Self::counts_as_score_unit_destroy(mask) {
            self.add_unit_destroyed_for_player(object.get_score_controlling_player_index());
            Self::increment_object_count(&mut self.objects_destroyed[slot], template_name);
        }
    }

    /// Add an object that was built by this player.
    /// C++ Reference: ScoreKeeper::addObjectBuilt(const Object* o)
    pub fn add_object_built_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        if !Self::scoring_enabled() {
            return;
        }
        let mask = object.get_score_kindof_mask();
        let template_name = object.get_score_template_name();
        if Self::counts_as_score_building_create(mask) {
            self.buildings_built += 1;
            Self::increment_object_count(&mut self.objects_built, template_name);
        } else if Self::counts_as_score_unit_create(mask) {
            self.units_built += 1;
            Self::increment_object_count(&mut self.objects_built, template_name);
        }
    }

    /// Remove an object from the built score map.
    /// C++ Reference: ScoreKeeper::removeObjectBuilt(const Object* o)
    pub fn remove_object_built_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        if !Self::scoring_enabled() {
            return;
        }
        let mask = object.get_score_kindof_mask();
        let template_name = object.get_score_template_name();
        if Self::counts_as_score_building_create(mask) {
            self.buildings_built -= 1;
            Self::decrement_object_count(&mut self.objects_built, template_name);
        } else if Self::counts_as_score_unit_create(mask) {
            self.units_built -= 1;
            Self::decrement_object_count(&mut self.objects_built, template_name);
        }
    }

    /// Add an object captured by this player.
    /// C++ Reference: ScoreKeeper::addObjectCaptured(const Object* o)
    pub fn add_object_captured_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        if !Self::scoring_enabled() {
            return;
        }
        let mask = object.get_score_kindof_mask();
        if mask.is_set(ScoreKindOf::Structure) {
            if mask.is_set(ScoreKindOf::Score) {
                self.faction_buildings_captured += 1;
            } else {
                self.tech_buildings_captured += 1;
            }
            Self::increment_object_count(
                &mut self.objects_captured,
                object.get_score_template_name(),
            );
        }
    }
}
