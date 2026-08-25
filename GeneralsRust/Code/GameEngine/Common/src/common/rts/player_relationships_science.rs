use super::*;

impl Player {
    // Relationship System (C++ Player.cpp lines 540-590)
    // =========================================================

    /// Get the relationship with another player by their player index.
    /// C++ Reference: Player::getRelationship() for player index lookup
    /// Returns NEUTRAL if no relationship is explicitly set.
    ///
    /// # Arguments
    /// * `player_index` - The index of the other player
    ///
    /// # Returns
    /// The relationship type (Allies, Enemies, or Neutral)
    pub fn get_relationship(&self, player_index: i32) -> Relationship {
        self.player_relations
            .get(player_index)
            .unwrap_or(Relationship::Neutral)
    }

    /// Set the relationship with another player.
    /// C++ Reference: Player::setPlayerRelationship() lines 582-588
    ///
    /// # Arguments
    /// * `player_index` - The index of the other player
    /// * `relationship` - The relationship to set
    pub fn set_player_relationship(&mut self, player_index: i32, relationship: Relationship) {
        self.player_relations.set(player_index, relationship);
    }

    /// Remove all relationships, or a specific player relationship.
    /// Returns true if relationships were removed.
    ///
    /// # Arguments
    /// * `player_index` - If Some, remove only that player's relationship. If None, clear all.
    pub fn remove_player_relationship(&mut self, player_index: Option<i32>) -> bool {
        self.player_relations.remove(player_index)
    }

    /// Get a reference to the player relations map
    pub fn get_player_relations(&self) -> &PlayerRelationMap {
        &self.player_relations
    }

    /// Get a mutable reference to the player relations map
    pub fn get_player_relations_mut(&mut self) -> &mut PlayerRelationMap {
        &mut self.player_relations
    }

    // =========================================================
    // Science System (C++ Player.h lines 325-327)
    // =========================================================

    /// Get skill points
    /// C++ Reference: Player::getSkillPoints() (Player.h line 330)
    pub fn get_skill_points(&self) -> i32 {
        self.skill_points
    }

    /// Add skill points, returns true if player gained/lost levels
    /// C++ Reference: Player::addSkillPoints() (Player.cpp lines 3041-3084)
    pub fn add_skill_points(&mut self, delta: i32) -> bool {
        // C++ line 3045: Apply modifier
        let adjusted_delta = (delta as f32 * self.skill_points_modifier).ceil() as i32;

        // C++ lines 3050-3052: Check for no change
        if adjusted_delta == 0 {
            return false;
        }

        // C++ line 3054: Apply the change
        let old_rank = self.rank_level;
        self.skill_points += adjusted_delta;

        // C++ addSkillPoints only advances ranks. Rank loss is handled by setRankLevel.
        let new_rank = self.calculate_rank_from_skill_points();
        if new_rank > old_rank {
            return self.set_rank_level(new_rank);
        }
        false
    }

    /// Calculate rank level from current skill points
    /// C++ Reference: RankInfoStore::getRankLevelForSkillPoints()
    fn calculate_rank_from_skill_points(&self) -> i32 {
        let rank_store = get_rank_info_store();
        if !rank_store.is_empty() {
            return rank_store.get_rank_level_for_skill_points(self.skill_points);
        }

        let points = self.skill_points;
        if points >= 5000 {
            8
        } else if points >= 4000 {
            7
        } else if points >= 3000 {
            6
        } else if points >= 2000 {
            5
        } else if points >= 1000 {
            4
        } else if points >= 500 {
            3
        } else if points >= 100 {
            2
        } else {
            1
        }
    }

    /// Get rank level
    /// C++ Reference: Player::getRankLevel() (Player.h line 332)
    pub fn get_rank_level(&self) -> i32 {
        self.rank_level
    }

    /// Set rank level, returns true if changed
    /// C++ Reference: Player::setRankLevel() (Player.cpp lines 3090-3115)
    pub fn set_rank_level(&mut self, level: i32) -> bool {
        let rank_count = {
            let rank_store = get_rank_info_store();
            rank_store.get_rank_level_count()
        };

        if rank_count == 0 {
            let level = level.max(1);
            if level == self.rank_level {
                return false;
            }

            self.rank_level = level;
            return true;
        }

        let level = level.clamp(1, rank_count);
        if level == self.rank_level {
            return false;
        }

        if level < self.rank_level {
            self.reset_rank();
        }

        let start_level = self.rank_level + 1;
        let rank_store = get_rank_info_store();
        for rank_level in start_level..=level {
            if let Some(rank) = rank_store.get_rank_info(rank_level) {
                self.science_purchase_points += rank.science_purchase_points_granted as i32;
                if self.science_purchase_points < 0 {
                    self.science_purchase_points = 0;
                }

                if self.skill_points < rank.skill_points_needed {
                    self.skill_points = rank.skill_points_needed;
                }

                for &science in &rank.sciences_granted {
                    self.grant_science(science);
                }

                self.level_down = rank.skill_points_needed;
            }
        }

        self.level_up = rank_store
            .get_rank_info(level + 1)
            .map(|rank| rank.skill_points_needed)
            .unwrap_or(i32::MAX);
        self.rank_level = level;
        true
    }

    /// Get skill points modifier
    /// C++ Reference: Player::getSkillPointsModifier() (Player.h line 342)
    pub fn get_skill_points_modifier(&self) -> f32 {
        self.skill_points_modifier
    }

    /// Set skill points modifier
    /// C++ Reference: Player::setSkillPointsModifier() (Player.h line 341)
    pub fn set_skill_points_modifier(&mut self, modifier: f32) {
        self.skill_points_modifier = modifier;
    }

    /// Get skill points to level up
    /// C++ Reference: Player::getSkillPointsLevelUp() (Player.h line 333)
    pub fn get_skill_points_level_up(&self) -> i32 {
        self.level_up
    }

    /// Get skill points to level down
    /// C++ Reference: Player::getSkillPointsLevelDown() (Player.h line 334)
    pub fn get_skill_points_level_down(&self) -> i32 {
        self.level_down
    }

    /// Get general name
    /// C++ Reference: Player::getGeneralName() (Player.h line 335)
    pub fn get_general_name(&self) -> &str {
        &self.general_name
    }

    /// Set general name
    /// C++ Reference: Player::setGeneralName() (Player.h line 336)
    pub fn set_general_name(&mut self, name: String) {
        self.general_name = name;
    }

    // =========================================================
    // Science Purchase Points (C++ Player.h lines 337-340)
    // =========================================================

    /// Get science purchase points
    /// C++ Reference: Player::getSciencePurchasePoints() (Player.h line 331)
    pub fn get_science_purchase_points(&self) -> i32 {
        self.science_purchase_points
    }

    /// Add science purchase points
    /// C++ Reference: Player::addSciencePurchasePoints() (Player.h line 339)
    pub fn add_science_purchase_points(&mut self, delta: i32) {
        let old_points = self.science_purchase_points;
        self.science_purchase_points += delta;
        if self.science_purchase_points < 0 {
            self.science_purchase_points = 0;
        }

        // Notify UI if changed (would notify control bar in full impl)
        let _ = old_points; // Just to note we track the change
    }

    /// Add skill points for kill
    /// C++ Reference: Player::addSkillPointsForKill() (Player.cpp lines 2104-2115)
    pub fn add_skill_points_for_kill(&mut self, victim_level: i32, skill_value: i32) -> bool {
        let _ = victim_level; // Would affect calculation based on victim's veterancy
        self.add_skill_points(skill_value)
    }

    /// Add skill points for kill using trait objects.
    /// C++ Reference: Player::addSkillPointsForKill(const Object* killer, const Object* victim)
    ///
    /// # Arguments
    /// * `killer` - The object that made the kill (unused in basic implementation)
    /// * `victim` - The object that was killed
    pub fn add_skill_points_for_kill_obj(
        &mut self,
        killer: &dyn SkillPointObject,
        victim: &dyn SkillPointObject,
    ) -> bool {
        let victim_level = victim.get_veterancy_level();
        let skill_value = victim.get_skill_point_value(killer);
        self.add_skill_points_for_kill(victim_level, skill_value)
    }

    /// Complete rank reset to initial state
    /// C++ Reference: Player::resetRank() (Player.cpp lines 2142-2163)
    pub fn reset_rank_full(&mut self) {
        self.reset_rank();
        self.general_name = "General".to_string();
    }

    /// Get all sciences
    pub fn get_sciences(&self) -> &HashSet<ScienceType> {
        &self.sciences
    }

    /// Get all disabled sciences
    pub fn get_sciences_disabled(&self) -> &HashSet<ScienceType> {
        &self.sciences_disabled
    }

    /// Get all hidden sciences
    pub fn get_sciences_hidden(&self) -> &HashSet<ScienceType> {
        &self.sciences_hidden
    }

    /// Set sciences directly (for save/load)
    pub fn set_sciences(&mut self, sciences: HashSet<ScienceType>) {
        self.sciences = sciences;
    }

    /// Set disabled sciences directly (for save/load)
    pub fn set_sciences_disabled(&mut self, sciences: HashSet<ScienceType>) {
        self.sciences_disabled = sciences;
    }

    /// Set hidden sciences directly (for save/load)
    pub fn set_sciences_hidden(&mut self, sciences: HashSet<ScienceType>) {
        self.sciences_hidden = sciences;
    }

    // =========================================================
    // Bounty System (C++ Player.h lines 373-376)
    // =========================================================

    /// Get cash bounty percent
    /// C++ Reference: Player::getCashBounty() (Player.h line 423)
    pub fn get_cash_bounty_percent(&self) -> f32 {
        self.cash_bounty_percent
    }

    /// Set cash bounty percent
    /// C++ Reference: Player::setCashBounty() (Player.h line 424)
    pub fn set_cash_bounty_percent(&mut self, percent: f32) {
        self.cash_bounty_percent = percent;
    }

    /// Do bounty for kill - awards cash when player kills an enemy
    /// C++ Reference: Player::doBountyForKill() (Player.cpp lines 1963-1989)
    pub fn do_bounty_for_kill(&mut self, killer_cost: i32) -> i32 {
        // Calculate bounty based on victim's cost and our cash bounty percent
        let bounty = ((killer_cost as f32) * self.cash_bounty_percent).ceil() as i32;

        if bounty > 0 {
            if let Ok(amount) = u32::try_from(bounty) {
                self.money.deposit(amount, false);
            }
            self.score_keeper.add_money_earned(bounty);
        }

        bounty
    }

    /// Do bounty for kill using trait objects.
    /// C++ Reference: Player::doBountyForKill(const Object* killer, const Object* victim)
    ///
    /// # Arguments
    /// * `_killer` - The object that made the kill (unused in basic implementation)
    /// * `victim` - The object that was killed
    ///
    /// Returns the bounty amount awarded.
    pub fn do_bounty_for_kill_obj(
        &mut self,
        _killer: &dyn BountyObject,
        victim: &dyn BountyObject,
    ) -> i32 {
        // C++ Player.cpp:2406-2407: no bounty for under-construction victims.
        if victim.is_under_construction() {
            return 0;
        }

        // C++ Player.cpp:2409 calcCostToBuild(victim controlling player).
        let killer_cost = victim.calc_cost_to_build();

        self.do_bounty_for_kill(killer_cost)
    }

    // =========================================================
    // CRC for networking (C++ Player.cpp lines 3939-3960)
    // =========================================================

    /// Compute CRC for network synchronization.
    /// C++ Reference: Player::crc(Xfer* xfer) - used for network game state validation
    /// This method computes a simple CRC hash of the player's critical state
    /// for network synchronization purposes.
    pub fn crc(&self) -> u32 {
        // Simple CRC computation based on key player state
        // This mirrors the C++ approach of xfer'ing key values for CRC
        let mut result: u32 = 0;

        // Hash player index
        result = result.wrapping_add(self.index as u32);

        // Hash skill points
        result = result.wrapping_add(self.skill_points as u32);

        // Hash science purchase points
        result = result.wrapping_add(self.science_purchase_points as u32);

        // Hash rank level
        result = result.wrapping_add(self.rank_level as u32);

        // Hash cash bounty (convert to bits for deterministic hashing)
        result = result.wrapping_add(self.cash_bounty_percent.to_bits());

        // Hash relationships using PlayerRelationMap (deterministic order)
        let mut indices: Vec<_> = self.player_relations.iter().map(|(k, _)| *k).collect();
        indices.sort();
        for idx in indices {
            result = result.wrapping_add(idx as u32);
            if let Some(rel) = self.player_relations.get(idx) {
                result = result.wrapping_add(rel.clone() as i32 as u32);
            }
        }

        // Hash sciences count (for state consistency)
        result = result.wrapping_add(self.sciences.len() as u32);
        result = result.wrapping_add(self.sciences_disabled.len() as u32);
        result = result.wrapping_add(self.sciences_hidden.len() as u32);

        result
    }

    /// Check whether this player already owns the specified science
    pub fn has_science(&self, science: ScienceType) -> bool {
        science != SCIENCE_INVALID && self.sciences.contains(&science)
    }

    /// Grant a science to the player
    pub fn grant_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences_disabled.remove(&science);
        self.sciences_hidden.remove(&science);
        self.sciences.insert(science);
    }

    /// Disable a science (remains known but unusable)
    pub fn disable_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences.remove(&science);
        self.sciences_hidden.remove(&science);
        self.sciences_disabled.insert(science);
    }

    /// Hide a science (used by UI gating, retains knowledge state)
    pub fn hide_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences_disabled.remove(&science);
        self.sciences_hidden.insert(science);
    }

    /// Check if a science is disabled
    pub fn is_science_disabled(&self, science: ScienceType) -> bool {
        self.sciences_disabled.contains(&science)
    }

    /// Check if a science is hidden
    pub fn is_science_hidden(&self, science: ScienceType) -> bool {
        self.sciences_hidden.contains(&science)
    }

    /// Set science availability
    /// C++ Reference: Player::setScienceAvailability() (Player.cpp lines 2273-2307)
    pub fn set_science_availability(&mut self, science: ScienceType, available: bool) {
        if available {
            // Remove from disabled and hidden lists
            self.sciences_disabled.remove(&science);
            self.sciences_hidden.remove(&science);
        } else {
            // Add to disabled list
            self.sciences_disabled.insert(science);
        }
    }

    /// Check if has prerequisites for science
    /// C++ Reference: Player::hasPrereqsForScience() (Player.cpp lines 1992-1995)
    pub fn has_prereqs_for_science(&self, science: ScienceType) -> bool {
        if science == SCIENCE_INVALID {
            return false;
        }

        get_science_store()
            .map(|store| store.player_has_prereqs_for_science(self, science))
            .unwrap_or(false)
    }

    /// Check if capable of purchasing science
    /// C++ Reference: Player::isCapableOfPurchasingScience() (Player.cpp lines 2226-2254)
    pub fn is_capable_of_purchasing_science(&self, science: ScienceType) -> bool {
        if science == SCIENCE_INVALID {
            return false;
        }

        // Already have it?
        if self.has_science(science) {
            return false;
        }

        // Is it disabled or hidden?
        if self.is_science_disabled(science) || self.is_science_hidden(science) {
            return false;
        }

        // Has prereqs?
        if !self.has_prereqs_for_science(science) {
            return false;
        }

        let Some(store) = get_science_store() else {
            return false;
        };

        let cost = store.get_science_purchase_cost(science);
        if cost == 0 || cost > self.science_purchase_points {
            return false;
        }

        true
    }

    /// Attempt to purchase a science
    /// C++ Reference: Player::attemptToPurchaseScience() (Player.cpp lines 2204-2223)
    pub fn attempt_to_purchase_science(&mut self, science: ScienceType) -> bool {
        if !self.is_capable_of_purchasing_science(science) {
            return false;
        }

        let cost = get_science_store()
            .map(|store| store.get_science_purchase_cost(science))
            .unwrap_or(0);
        self.add_science_purchase_points(-cost);

        // Add the science
        self.grant_science(science);

        // Record in academy stats
        self.academy_stats.record_generals_points_spent(cost);

        true
    }

    /// Grant a science (bypassing purchase system)
    /// C++ Reference: Player::grantScience() (Player.cpp lines 2195-2201)
    pub fn grant_science_with_check(&mut self, science: ScienceType) -> bool {
        if !get_science_store()
            .map(|store| store.is_science_grantable(science))
            .unwrap_or(false)
        {
            return false;
        }

        self.grant_science(science);
        true
    }

    /// Reset sciences to default state
    /// C++ Reference: Player::resetSciences() (Player.cpp lines 2118-2140)
    pub fn reset_sciences_full(&mut self) {
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();
        self.reset_sciences();
    }
}
