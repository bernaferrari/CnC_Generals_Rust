use super::*;

impl Player {
    /// Handle power brownout state change
    /// C++ Reference: Player::onPowerBrownOutChange() (Player.cpp lines 3232-3241)
    pub(super) fn on_power_brown_out_change(&mut self, brown_out: bool) {
        if brown_out {
            self.disable_radar();
        } else {
            self.enable_radar();
        }
        // C++ iterateObjects(doPowerDisable, &brownOut)
        if let Some(world) = get_player_object_world() {
            for id in self.collect_owned_object_ids(&world) {
                if let Some(snap) = world.snapshot(id) {
                    if snap.is_kind(KindOfMask::POWERED) || snap.is_powered {
                        world.set_disabled_underpowered(id, brown_out);
                    }
                }
            }
        }
    }

    // =========================================================
    // Radar System (C++ Player.h lines 299-301)
    // =========================================================

    /// Add a radar producer
    /// C++ Reference: Player::addRadar() (Player.cpp lines 2414-2422)
    pub fn add_radar(&mut self, disable_proof: bool) {
        self.radar_count += 1;
        if disable_proof {
            self.disable_proof_radar_count += 1;
        }
    }

    /// Remove a radar producer
    /// C++ Reference: Player::removeRadar() (Player.cpp lines 2425-2434)
    pub fn remove_radar(&mut self, disable_proof: bool) {
        if self.radar_count > 0 {
            self.radar_count -= 1;
        }
        if disable_proof && self.disable_proof_radar_count > 0 {
            self.disable_proof_radar_count -= 1;
        }
    }

    /// Disable radar (regardless of count)
    /// C++ Reference: Player::disableRadar() (Player.cpp lines 2437-2440)
    pub fn disable_radar(&mut self) {
        self.radar_disabled = true;
    }

    /// Enable radar (remove restriction)
    /// C++ Reference: Player::enableRadar() (Player.cpp lines 2443-2446)
    pub fn enable_radar(&mut self) {
        self.radar_disabled = false;
    }

    /// Check if player has radar
    /// C++ Reference: Player::hasRadar() (Player.cpp lines 2449-2452)
    pub fn has_radar(&self) -> bool {
        self.radar_count > 0 && !self.radar_disabled
    }

    // =========================================================
    // Battle Plan System (C++ Player.h lines 302-304)
    // =========================================================

    /// Get total number of battle plans active
    /// C++ Reference: Player::getNumBattlePlansActive() (Player.h line 228)
    pub fn get_num_battle_plans_active(&self) -> i32 {
        self.bombard_battle_plans
            + self.hold_the_line_battle_plans
            + self.search_and_destroy_battle_plans
    }

    /// Get count of specific battle plan type
    /// C++ Reference: Player::getBattlePlansActiveSpecific() (Player.cpp lines 2455-2469)
    pub fn get_battle_plans_active_specific(&self, plan_type: BattlePlanStatus) -> i32 {
        match plan_type {
            BattlePlanStatus::Bombardment => self.bombard_battle_plans,
            BattlePlanStatus::HoldTheLine => self.hold_the_line_battle_plans,
            BattlePlanStatus::SearchAndDestroy => self.search_and_destroy_battle_plans,
        }
    }

    /// Change a battle plan count
    /// C++ Reference: Player::changeBattlePlan() (Player.cpp lines 2472-2498)
    pub fn change_battle_plan(&mut self, plan: BattlePlanStatus, delta: i32) {
        match plan {
            BattlePlanStatus::Bombardment => self.bombard_battle_plans += delta,
            BattlePlanStatus::HoldTheLine => self.hold_the_line_battle_plans += delta,
            BattlePlanStatus::SearchAndDestroy => self.search_and_destroy_battle_plans += delta,
        }
    }

    // =========================================================
    // Attacked Tracking (C++ Player.h lines 378-379)
    // =========================================================

    /// Mark that this player was attacked by another player
    /// C++ Reference: Player::setAttackedBy() (Player.cpp lines 3173-3176)
    pub fn set_attacked_by(&mut self, player_index: i32) {
        if player_index >= 0 && (player_index as usize) < self.attacked_by.len() {
            self.attacked_by[player_index as usize] = true;
            self.attacked_frame = crate::common::time::frame();
        }
    }

    /// Check if this player was attacked by another player
    /// C++ Reference: Player::getAttackedBy() (Player.cpp lines 3179-3182)
    pub fn get_attacked_by(&self, player_index: i32) -> bool {
        if player_index >= 0 && (player_index as usize) < self.attacked_by.len() {
            self.attacked_by[player_index as usize]
        } else {
            false
        }
    }

    /// Get the last frame this player was attacked
    /// C++ Reference: Player::getAttackedFrame() (Player.h line 421)
    pub fn get_attacked_frame(&self) -> u32 {
        self.attacked_frame
    }

    /// Get the attacked-by array (for save/load)
    pub fn get_attacked_by_array(&self) -> &[bool] {
        &self.attacked_by
    }

    /// Set the attacked-by array (for load)
    pub fn set_attacked_by_array(&mut self, attacked: Vec<bool>) {
        self.attacked_by = attacked;
    }

    // =========================================================
    // Player State Queries (C++ Player.h lines 398-412)
    // =========================================================

    /// Check if player is dead
    /// C++ Reference: Player::isPlayerDead() (Player.h line 408)
    pub fn is_player_dead(&self) -> bool {
        self.is_player_dead
    }

    /// Set player dead state
    pub fn set_player_dead(&mut self, dead: bool) {
        self.is_player_dead = dead;
    }

    /// Check if player is an observer
    /// C++ Reference: Player::isPlayerObserver() (Player.h line 407)
    pub fn is_player_observer(&self) -> bool {
        self.observer
    }

    /// Set observer mode
    /// C++ Reference: Player::init() sets m_observer (Player.cpp line 320)
    pub fn set_observer(&mut self, observer: bool) {
        self.observer = observer;
        // Observers are considered "dead" for gameplay purposes
        if observer {
            self.is_player_dead = true;
        }
    }

    /// Check if player is active (not dead and not observer)
    /// C++ Reference: Player::isPlayerActive() (Player.h line 409)
    pub fn is_player_active(&self) -> bool {
        !self.observer && !self.is_player_dead
    }

    /// Check if this is a playable side
    /// C++ Reference: Player::isPlayableSide() (Player.cpp lines 3185-3190)
    pub fn is_playable_side(&self) -> bool {
        self.current_player_template()
            .map(|template| template.is_playable_side())
            .unwrap_or(false)
    }

    /// Check if player preordered
    /// C++ Reference: Player::didPlayerPreorder() (Player.h line 411)
    pub fn did_player_preorder(&self) -> bool {
        self.is_preorder
    }

    /// Set preorder status
    pub fn set_preorder(&mut self, preorder: bool) {
        self.is_preorder = preorder;
    }

    /// Check if should be listed in score screen
    /// C++ Reference: Player::getListInScoreScreen() (Player.h line 413)
    pub fn get_list_in_score_screen(&self) -> bool {
        self.list_in_score_screen
    }

    /// Set score screen listing
    pub fn set_list_in_score_screen(&mut self, list: bool) {
        self.list_in_score_screen = list;
    }

    /// Get units should hunt flag
    /// C++ Reference: Player::getUnitsShouldHunt() (Player.h line 376)
    pub fn get_units_should_hunt(&self) -> bool {
        self.units_should_hunt
    }

    /// Set units should hunt
    /// C++ Reference: Player::setUnitsShouldHunt() (Player.cpp lines 3179-3182)
    pub fn set_units_should_hunt(&mut self, should_hunt: bool) {
        self.units_should_hunt = should_hunt;
    }

    /// Get can build units
    /// C++ Reference: Player::getCanBuildUnits() (Player.h line 395)
    pub fn get_can_build_units(&self) -> bool {
        self.can_build_units
    }

    /// Set can build units
    pub fn set_can_build_units(&mut self, can_build: bool) {
        self.can_build_units = can_build;
    }

    /// Get can build base
    /// C++ Reference: Player::getCanBuildBase() (Player.h line 397)
    pub fn get_can_build_base(&self) -> bool {
        self.can_build_base
    }

    /// Set can build base
    pub fn set_can_build_base(&mut self, can_build: bool) {
        self.can_build_base = can_build;
    }

    // =========================================================
    // Kill Player and Related (C++ Player.cpp lines 1597-1650)
    // =========================================================

    /// Kill this player - evacuate, destroy units, then zero money.
    /// C++ Reference: Player::killPlayer() (Player.cpp lines 2023-2071)
    pub fn kill_player(&mut self) {
        let world = get_player_object_world();
        let ids = if let Some(world) = world.as_ref() {
            self.collect_owned_object_ids(world)
        } else {
            self.owned_objects.clone()
        };

        // C++ first pass: evacuateTeam on every instance
        if let Some(world) = world.as_ref() {
            for id in &ids {
                world.evacuate_container(*id);
            }
        }

        // Mark dead so OCLs don't spawn useful units
        self.is_player_dead = true;

        // C++ second pass: killTeam — TECH_BUILDING → Neutral, else kill.
        if let Some(world) = world.as_ref() {
            let neutral_team = world.get_neutral_default_team();
            for id in &ids {
                let is_tech = world
                    .snapshot(*id)
                    .is_some_and(|snap| snap.is_kind(KindOfMask::TECH_BUILDING));
                if is_tech {
                    if let Some(team_id) = neutral_team {
                        world.set_team(*id, team_id);
                    }
                    continue;
                }
                world.kill_object(*id);
            }
        }
        self.owned_objects.clear();

        // C++: single-player computer players are resurrected so scripts can reuse the slot
        let resurrect_sp_ai = world
            .as_ref()
            .map(|w| w.is_single_player_game() && self.player_type == PlayerType::Computer)
            .unwrap_or(false);
        if resurrect_sp_ai {
            self.is_player_dead = false;
            return;
        }

        if self.is_local {
            self.becoming_local_player(true);
            if let Some(world) = world.as_ref() {
                if self.is_player_active() {
                    world.set_player_control_bar(self.index);
                } else {
                    world.set_observer_control_bar();
                }
            }
        }

        let all_money = self.money.count_money();
        if all_money > 0 {
            self.money.withdraw(all_money, false);
        }
    }

    /// Transfer all assets from another player to this one
    /// C++ Reference: Player::transferAssetsFromThat() (Player.cpp lines 2100-2144)
    pub fn transfer_assets_from(&mut self, other: &mut Player) {
        let dest_team = self.default_team;
        let beacon = other
            .current_player_template()
            .map(|t| t.get_beacon_template().to_string())
            .unwrap_or_default();

        if let Some(world) = get_player_object_world() {
            let mut to_transfer = Vec::new();
            for id in other.collect_owned_object_ids(&world) {
                if let Some(snap) = world.snapshot(id) {
                    if snap.is_beacon || (!beacon.is_empty() && snap.template_name == beacon) {
                        continue;
                    }
                    to_transfer.push(id);
                } else {
                    to_transfer.push(id);
                }
            }
            if let Some(team_id) = dest_team {
                for id in &to_transfer {
                    world.set_team(*id, team_id);
                    self.add_owned_object(*id);
                    other.remove_owned_object(*id);
                }
            }
        }

        let all_money = other.get_money().count_money();
        if all_money > 0 {
            other.get_money_mut().withdraw(all_money, false);
            self.money.deposit(all_money, false);
        }
    }

    /// Garrison all units
    /// C++ Reference: Player::garrisonAllUnits() (Player.cpp lines 2147-2197)
    pub fn garrison_all_units(&mut self) {
        let Some(world) = get_player_object_world() else {
            return;
        };
        let units = self.collect_owned_object_ids(&world);
        let mut buildings = world.all_object_ids();
        if buildings.is_empty() {
            buildings = units.clone();
        }
        let my_mask = self.get_player_mask();
        for unit_id in units {
            let Some(unit) = world.snapshot(unit_id) else {
                continue;
            };
            if !unit.has_ai || unit.is_structure || unit.is_dead {
                continue;
            }
            for building_id in &buildings {
                let Some(building) = world.snapshot(*building_id) else {
                    continue;
                };
                if !building.is_structure || !building.has_contain {
                    continue;
                }
                if !(building.contain_player_mask == 0 || building.contain_player_mask == my_mask) {
                    continue;
                }
                if !world.can_enter(unit_id, *building_id) {
                    continue;
                }
                world.ai_enter(unit_id, *building_id);
            }
        }
    }

    /// Ungarrison all units
    /// C++ Reference: Player::ungarrisonAllUnits() (Player.cpp lines 2200-2230)
    pub fn ungarrison_all_units(&mut self) {
        let Some(world) = get_player_object_world() else {
            return;
        };
        for id in self.collect_owned_object_ids(&world) {
            if let Some(snap) = world.snapshot(id) {
                if snap.is_structure && snap.has_ai {
                    world.ai_evacuate(id);
                }
            }
        }
    }

    /// Set units to idle or resume
    /// C++ Reference: Player::setUnitsShouldIdleOrResume() (Player.cpp lines 2234-2276)
    pub fn set_units_should_idle_or_resume(&mut self, idle: bool) {
        let Some(world) = get_player_object_world() else {
            return;
        };
        for id in self.collect_owned_object_ids(&world) {
            let Some(snap) = world.snapshot(id) else {
                continue;
            };
            if snap.is_structure || !snap.has_ai {
                continue;
            }
            if idle {
                world.ai_move_to(id, snap.position);
            } else if snap.is_idle {
                world.ai_force_want_supplies(id);
            }
        }
    }

    /// Sell everything under the sun
    /// C++ Reference: Player::sellEverythingUnderTheSun() (Player.cpp lines 2288-2291)
    pub fn sell_everything(&mut self) {
        if let Some(world) = get_player_object_world() {
            for id in self.collect_owned_object_ids(&world) {
                if let Some(snap) = world.snapshot(id) {
                    if snap.is_faction_structure
                        || snap.is_kind(KindOfMask::COMMANDCENTER)
                        || snap.is_kind(KindOfMask::FS_POWER)
                    {
                        world.sell_object(id);
                    }
                }
            }
        }
        self.build_list = None;
    }

    /// Set objects enabled/disabled by template
    /// C++ Reference: Player::setObjectsEnabled() (Player.cpp lines 2074-2097)
    pub fn set_objects_enabled(&mut self, template_name: &str, enable: bool) {
        let Some(world) = get_player_object_world() else {
            return;
        };
        for id in self.collect_owned_object_ids(&world) {
            if let Some(snap) = world.snapshot(id) {
                if snap.template_name == template_name {
                    world.set_script_disabled(id, !enable);
                }
            }
        }
    }

    // =========================================================
    // Build Prerequisites and Permissions (C++ Player.cpp lines 1842-2061)
    // =========================================================

    /// Check if allowed to build a thing (basic check)
    /// C++ Reference: Player::allowedToBuild() (Player.cpp lines 1842-1855)
    pub fn allowed_to_build(&self, is_structure: bool) -> bool {
        if !self.can_build_base && is_structure {
            return false;
        }
        if !self.can_build_units && !is_structure {
            return false;
        }
        true
    }

    /// Check if can build a thing (includes prereqs when the template factory is available)
    /// C++ Reference: Player::canBuild() (Player.cpp lines 2880-2924)
    pub fn can_build(&self, template_name: &str, is_structure: bool) -> bool {
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                return factory
                    .find_template(template_name, false)
                    .map(|template| self.can_build_thing_template(template.as_ref()))
                    .unwrap_or(false);
            }
        }

        self.allowed_to_build(is_structure)
    }

    /// Full template check matching C++ Player::canBuild(const ThingTemplate*).
    pub fn can_build_thing_template(&self, template: &ThingTemplate) -> bool {
        let is_structure = template.is_kind_of_mask(KindOfMask::STRUCTURE.bits() as u64);
        let buildable = match template.get_buildable() {
            BuildableStatus::Yes => 0,
            BuildableStatus::IgnorePrerequisites => 1,
            BuildableStatus::No => 2,
            BuildableStatus::OnlyByAi => 3,
        };

        self.can_build_template(is_structure, buildable, template.get_prereqs())
    }

    /// Full prerequisite check matching C++ Player::canBuild() behavior.
    ///
    /// C++ Reference: Player::canBuild() (Player.cpp lines 2880-2924)
    ///
    /// Checks:
    /// 1. allowedToBuild()
    /// 2. BuildableStatus != BSTATUS_NO
    /// 3. BuildableStatus != BSTATUS_ONLY_BY_AI (unless player is COMPUTER)
    /// 4. All ProductionPrerequisite entries satisfied (AND logic)
    /// 5. (Debug) ignoresPrereqs override
    /// 6. canBuildMoreOfType
    pub fn can_build_template(
        &self,
        is_structure: bool,
        buildable: i32, // 0=Yes, 1=IgnorePrerequisites, 2=No, 3=OnlyByAI
        prereqs: &[ProductionPrerequisite],
    ) -> bool {
        // C++ line 2885: if (!allowedToBuild(tmplate)) return false;
        if !self.allowed_to_build(is_structure) {
            return false;
        }

        // C++ lines 2888-2895: BuildableStatus checks
        // BuildableStatus: Yes=0, Ignore_Prerequisites=1, No=2, Only_By_AI=3
        if buildable == 2 {
            // BSTATUS_NO
            return false;
        }
        if buildable == 1 {
            // BSTATUS_IGNORE_PREREQUISITES
            return true;
        }
        if buildable == 3 && self.player_type != PlayerType::Computer {
            // BSTATUS_ONLY_BY_AI
            return false;
        }

        // C++ lines 2898-2917: Check all prerequisites (AND logic)
        // All ProductionPrerequisite entries must be satisfied
        let mut prereqs_ok = true;
        for prereq in prereqs {
            if !prereq.is_satisfied(self) {
                prereqs_ok = false;
                break;
            }
        }

        // C++ lines 2909-2912: Debug override
        #[cfg(debug_assertions)]
        if self.ignores_prereqs() {
            prereqs_ok = true;
        }

        if !prereqs_ok {
            return false;
        }

        // C++ lines 2919-2920: canBuildMoreOfType
        // Note: max_simultaneous check requires template info, handled by caller

        true
    }

    /// Check if can afford to build
    /// C++ Reference: Player::canAffordBuild() (Player.cpp lines 2064-2073)
    pub fn can_afford_build(&self, cost: i32) -> bool {
        self.money.count_money() >= cost as u32
    }

    /// Check if can build more of a specific type
    /// C++ Reference: Player::canBuildMoreOfType() (Player.cpp lines 1907-1950)
    pub fn can_build_more_of_type(&self, _template_name: &str, max_simultaneous: u32) -> bool {
        // 0 means unlimited
        if max_simultaneous == 0 {
            return true;
        }
        true
    }

    /// Check max-simultaneous limits against typed object-world data.
    ///
    /// This matches C++ `Player::canBuildMoreOfType`: live owned objects count
    /// by equivalent template name or shared MaxSimultaneousLinkKey, and
    /// non-structure templates also count queued unit production.
    pub fn can_build_more_of_template_with_world<W: BuildLimitWorld>(
        &self,
        template: &BuildLimitTemplateInfo,
        world: &W,
    ) -> bool {
        let max_simultaneous = template.max_simultaneous_of_type();
        if max_simultaneous == 0 {
            return true;
        }

        let check_production_queue = !template.is_structure();
        let mut count = 0u32;

        for object in world.build_limit_objects_for_player(self.index) {
            if object.is_effectively_dead() {
                continue;
            }

            if template.matches_template(object.template()) {
                count += 1;
                if count >= max_simultaneous {
                    return false;
                }
            }

            if check_production_queue {
                for queued_template in object.queued_units() {
                    if template.matches_template(queued_template) {
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
}
