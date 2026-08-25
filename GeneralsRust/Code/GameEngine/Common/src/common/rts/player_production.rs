use super::*;

impl Player {
    // =========================================================
    // Production Cost/Time Changes (C++ Player.h lines 351-353)
    // =========================================================

    /// Set production cost change for a thing
    /// C++ Reference: Player production cost modifiers
    pub fn set_production_cost_change(&mut self, thing_name: String, percent: f32) {
        self.production_cost_changes.insert(thing_name, percent);
    }

    /// Get production cost change for a thing
    /// C++ Reference: Player::getProductionCostChangePercent() (Player.cpp)
    pub fn get_production_cost_change(&self, thing_name: &str) -> f32 {
        self.production_cost_changes
            .get(thing_name)
            .copied()
            .unwrap_or(0.0)
    }

    /// Set production time change for a thing
    pub fn set_production_time_change(&mut self, thing_name: String, percent: f32) {
        self.production_time_changes.insert(thing_name, percent);
    }

    /// Get production time change for a thing
    /// C++ Reference: Player::getProductionTimeChangePercent() (Player.cpp)
    pub fn get_production_time_change(&self, thing_name: &str) -> f32 {
        self.production_time_changes
            .get(thing_name)
            .copied()
            .unwrap_or(0.0)
    }

    /// Get production cost change based on KindOf mask.
    /// C++ Reference: Player::getProductionCostChangeBasedOnKindOf (Player.cpp lines 3842-3859)
    ///
    /// Iterates the KindOf-based production cost changes. For each entry whose
    /// KindOf mask overlaps with the provided `kindof`, the modifier is applied
    /// multiplicatively: `result *= (1 + percent)`.
    pub fn get_production_cost_change_based_on_kind_of(&self, kindof: u64) -> f32 {
        let mut result = 1.0f32;
        for entry in &self.kind_of_production_cost_changes {
            if (kindof as u128 & entry.kind_of.bits()) != 0 {
                result *= 1.0 + entry.percent;
            }
        }
        result
    }

    /// Add a KindOf-based production cost change entry.
    pub fn add_kind_of_production_cost_change(&mut self, kindof: u64, percent: f32) {
        self.kind_of_production_cost_changes
            .push(KindOfPercentProductionChange {
                kind_of: KindOfMask::from_bits_truncate(kindof as u128),
                percent,
                refs: 1,
            });
    }

    /// Override a team relationship (C++ Player::setTeamRelationship).
    pub fn set_team_relationship(&mut self, team_id: TeamID, relationship: Relationship) {
        self.team_relations.set_relationship(team_id, relationship);
    }

    /// Get an override team relationship if one exists.
    pub fn get_team_relationship(&self, team_id: TeamID) -> Option<Relationship> {
        self.team_relations.get_relationship(team_id)
    }

    #[cfg(test)]
    pub(super) fn set_battle_plan_bonuses_for_test(&mut self, bonuses: BattlePlanBonuses) {
        self.battle_plan_bonuses = Some(bonuses);
    }

    // =========================================================
    // Special Power Timers (C++ Player.h line 392)
    // =========================================================

    /// Set special power ready frame
    pub fn set_special_power_ready_frame(&mut self, template_id: u32, ready_frame: u32) {
        self.special_power_timers.insert(template_id, ready_frame);
    }

    /// Get special power ready frame
    pub fn get_special_power_ready_frame(&self, template_id: u32) -> Option<u32> {
        self.special_power_timers.get(&template_id).copied()
    }

    /// Remove special power timer
    pub fn remove_special_power_timer(&mut self, template_id: u32) {
        self.special_power_timers.remove(&template_id);
    }

    // =========================================================
    // Vision Spied (C++ Player.cpp lines 3138-3152)
    // =========================================================

    /// Set units vision spied status
    /// C++ Reference: Player::setUnitsVisionSpied() (Player.cpp lines 3138-3152)
    pub fn set_units_vision_spied(&mut self, _setting: bool, _by_whom: i32) {
        // Would iterate all objects and set their vision spied status
        // Simplified: no-op
    }

    // =========================================================
    // Retaliation Mode (C++ Player.cpp lines 573-590)
    // =========================================================

    /// Get logical retaliation mode enabled
    /// C++ Reference: Player::isLogicalRetaliationModeEnabled() (Player.h line 391)
    pub fn is_logical_retaliation_mode_enabled(&self) -> bool {
        self.logical_retaliation_mode_enabled
    }

    /// Set logical retaliation mode enabled
    /// C++ Reference: Player::setLogicalRetaliationModeEnabled()
    pub fn set_logical_retaliation_mode_enabled(&mut self, enabled: bool) {
        self.logical_retaliation_mode_enabled = enabled;
    }

    // =========================================================
    // Default Team (C++ Player.h line 321)
    // =========================================================

    /// Get default team
    /// C++ Reference: Player::getDefaultTeam() (Player.h line 322)
    pub fn get_default_team(&self) -> Option<TeamID> {
        self.default_team
    }

    /// Set default team
    /// C++ Reference: Player::setDefaultTeam() (Player.cpp lines 715-725)
    pub fn set_default_team(&mut self, team_id: TeamID) {
        self.default_team = Some(team_id);
    }

    // =========================================================
    // Side Information (C++ Player.h lines 289-290)
    // =========================================================

    /// Set player side
    pub fn set_side(&mut self, side: String) {
        self.side = side;
        self.refresh_academy_base_side_context();
    }

    /// Set player base side
    pub fn set_base_side(&mut self, base_side: String) {
        self.base_side = base_side;
        self.refresh_academy_base_side_context();
    }

    /// Set player display name
    pub fn set_player_display_name(&mut self, name: String) {
        self.player_display_name = name;
    }

    /// Set player name
    pub fn set_player_name(&mut self, name: String) {
        self.player_name = name;
    }

    // =========================================================
    // Debug/Cheat Methods (C++ #if _DEBUG sections)
    // =========================================================

    /// Check if ignores prereqs (debug only)
    /// C++ Reference: Player::ignoresPrereqs() (Player.cpp)
    #[cfg(debug_assertions)]
    pub fn ignores_prereqs(&self) -> bool {
        // Would return m_DEMO_ignorePrereqs in debug builds
        false
    }

    /// Check if free build (debug only)
    /// C++ Reference: Player::isFreeBuild() (Player.cpp)
    #[cfg(debug_assertions)]
    pub fn is_free_build(&self) -> bool {
        // Would return m_DEMO_freeBuild in debug builds
        false
    }

    /// Check if instant build (debug only)
    /// C++ Reference: Player::isInstantBuild() (Player.cpp)
    #[cfg(debug_assertions)]
    pub fn is_instant_build(&self) -> bool {
        // Would return m_DEMO_instantBuild in debug builds
        false
    }

    // =========================================================
    // Skillset (C++ Player.cpp line 1928)
    // =========================================================

    /// Set AI skillset (friend function for AI)
    /// C++ Reference: Player::friend_setSkillset() (Player.cpp line 1928)
    pub fn set_skillset(&mut self, skillset: i32) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, skillset); // Would call ai.selectSkillset()
        }
    }

    // =========================================================
    // Score Methods (C++ ScoreKeeper integration)
    // =========================================================

    /// Add object built to score
    pub fn score_add_object_built(&mut self, cost: i32) {
        self.score_keeper.add_money_spent(cost);
    }

    /// Get score keeper reference
    pub fn get_score_keeper_mut_ref(&mut self) -> &mut ScoreKeeper {
        &mut self.score_keeper
    }

    // =========================================================
    // Supply Box Value (C++ Player.cpp lines 1928-1933)
    // =========================================================

    /// Get supply box value
    /// C++ Reference: Player::getSupplyBoxValue() (Player.cpp lines 1928-1933)
    pub fn get_supply_box_value(&self) -> u32 {
        global_data::read_safe()
            .map(|data| data.base_value_per_supply_box.max(0) as u32)
            .unwrap_or(0)
    }

    // =========================================================
    // New Map (C++ Player.cpp lines 592-595)
    // =========================================================

    /// Called when a new map is loaded
    /// C++ Reference: Player::newMap() (Player.cpp lines 592-595)
    pub fn new_map(&mut self) {
        if let Some(ai) = self.get_ai() {
            ai.new_map();
        }
    }
}
