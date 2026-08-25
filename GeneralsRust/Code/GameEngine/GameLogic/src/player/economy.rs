use super::*;

impl Player {
    pub fn is_playable_side(&self) -> bool {
        self.player_template
            .as_ref()
            .map(|template| template.is_playable_side())
            .unwrap_or(false)
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
                let __base_arc = obj_instance.base();
                let Ok(mut base_obj) = __base_arc.write() else {
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

    pub(super) fn lookup_production_change(
        map: &HashMap<NameKeyType, Real>,
        template_name: &str,
    ) -> Real {
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
    /// C++ `Player::getProductionVeterancyLevel`. Defaults to LEVEL_FIRST/Regular.
    pub fn get_production_veterancy_level(&self, build_template_name: &str) -> VeterancyLevel {
        let Some(template) = self.player_template.as_ref() else {
            return VeterancyLevel::Regular;
        };
        let key = NameKeyGenerator::name_to_key(build_template_name);
        template
            .production_veterancy_levels
            .get(&key)
            .copied()
            .unwrap_or(VeterancyLevel::Regular)
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

    /// C++ Player::setPlayerDead — same latch as setDefeated.
    pub fn set_player_dead(&mut self, dead: Bool) {
        self.is_player_dead = dead;
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
                        let base_obj = obj_instance.base();
                        func(base_obj)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// ID-first owned-object iteration (no Arc retention at the callback boundary).
    pub fn iterate_object_ids<F>(&self, mut func: F) -> Result<(), GameError>
    where
        F: FnMut(ObjectID) -> Result<(), GameError>,
    {
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let object_ids = manager.get_objects_owned_by_player(self.player_index as UnsignedInt);
            for obj_id in object_ids {
                // Only yield ids that still resolve.
                if manager.get_object(obj_id).is_some() {
                    func(obj_id)?;
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

    /// C++ `Player::crc` dumped onto GameLogic `getCRC` (one XferCRC addCRC path).
    pub fn crc_into_logic_xfer(&self, xfer: &mut dyn crate::common::xfer::Xfer) {
        let _ = Snapshotable::crc(self, xfer);
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

    pub fn get_radar_count(&self) -> Int {
        self.radar_count
    }

    pub fn get_disable_proof_radar_count(&self) -> Int {
        self.disable_proof_radar_count
    }

    pub fn is_radar_disabled(&self) -> Bool {
        self.radar_disabled
    }
    pub fn restore_radar_state(&mut self, radar: Int, proof: Int, disabled: Bool) {
        self.radar_count = radar;
        self.disable_proof_radar_count = proof;
        self.radar_disabled = disabled;
    }

    pub fn kind_of_production_change_entries(&self) -> Vec<(KindOfMaskType, Real, u32)> {
        self.kind_of_percent_production_change_list
            .iter()
            .map(|entry| (entry.kind_of, entry.percent, entry.refs))
            .collect()
    }

    pub fn replace_kind_of_production_changes(&mut self, entries: &[(KindOfMaskType, Real, u32)]) {
        self.kind_of_percent_production_change_list = entries
            .iter()
            .map(|&(kind_of, percent, refs)| KindOfPercentProductionChange {
                kind_of,
                percent,
                refs,
            })
            .collect();
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
}
