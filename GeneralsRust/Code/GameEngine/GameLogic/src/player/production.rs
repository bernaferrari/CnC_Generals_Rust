use super::*;

impl Player {
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

    pub(super) fn is_production_prerequisite_satisfied(
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

    pub(super) fn can_build_more_of_type(
        &self,
        template: &dyn crate::common::ThingTemplate,
    ) -> Bool {
        // Wave 268: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let max_simultaneous = template.get_max_simultaneous_of_type();
        if max_simultaneous == 0 {
            return true;
        }

        let link_key = template.get_max_simultaneous_link_key();
        let check_production_queue = !template.is_kind_of(crate::common::KindOf::Structure);
        let mut count = 0u32;
        for &object_id in &self.owned_objects {
            let Some(at_cap) =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() {
                        return false;
                    }

                    let object_template = object_guard.get_template();
                    if template.is_equivalent_to(object_template.as_ref())
                        || (link_key != 0
                            && link_key == object_template.get_max_simultaneous_link_key())
                    {
                        count += 1;
                        if count >= max_simultaneous {
                            return true;
                        }
                    }

                    if check_production_queue {
                        let Some(production_behavior) =
                            object_guard.get_production_update_interface()
                        else {
                            return false;
                        };
                        let Ok(mut behavior_guard) = production_behavior.lock() else {
                            return false;
                        };
                        let Some(production) = behavior_guard.get_production_update_interface()
                        else {
                            return false;
                        };

                        for entry in production.get_queue_entries() {
                            if entry.production_type
                                != crate::object::production::queue::ProductionType::Unit
                            {
                                continue;
                            }
                            let Some(queued_template) =
                                crate::helpers::TheThingFactory::find_template(
                                    &entry.template_name,
                                )
                            else {
                                continue;
                            };
                            if template.is_equivalent_to(queued_template.as_ref())
                                || (link_key != 0
                                    && link_key == queued_template.get_max_simultaneous_link_key())
                            {
                                count += 1;
                                if count >= max_simultaneous {
                                    return true;
                                }
                            }
                        }
                    }
                    false
                })
            else {
                continue;
            };
            if at_cap {
                return false;
            }
        }

        true
    }

    /// Hunting behavior
    pub fn get_units_should_hunt(&self) -> Bool {
        self.units_should_hunt
    }

    /// C++ `Player::xfer` writes `m_unitsShouldHunt` only. Load must not
    /// re-walk members and re-issue `aiHunt`/`aiIdle`.
    pub fn restore_units_should_hunt(&mut self, should_hunt: Bool) {
        self.units_should_hunt = should_hunt;
    }

    pub fn set_units_should_hunt(&mut self, should_hunt: Bool, source: CommandSourceType) {
        self.units_should_hunt = should_hunt;

        // C++ Player::setUnitsShouldHunt queries getMostValuableLocation first
        // so hunt mood seeds the richest enemy cluster (Player.cpp:1983-1984).
        let _hunt_seed = crate::helpers::ThePartitionManager::get().and_then(|pm| {
            pm.get_most_valuable_location(
                self.player_index,
                !0u32,
                crate::object::collide::partition_manager::ValueOrThreat::CashValue,
            )
        });

        // C++ Player::setUnitsShouldHunt: team prototypes → instances → members.
        let mut member_ids: Vec<ObjectID> = Vec::new();
        if let Ok(factory) = get_team_factory().lock() {
            for prototype in &self.player_team_prototypes {
                for team in factory.find_team_instances(prototype.get_name().as_str()) {
                    if let Ok(team_guard) = team.read() {
                        member_ids.extend_from_slice(team_guard.get_members());
                    }
                }
            }
        }
        if member_ids.is_empty() {
            if let Some(team) = &self.default_team {
                if let Ok(team_guard) = team.read() {
                    member_ids.extend_from_slice(team_guard.get_members());
                }
            }
        }
        if member_ids.is_empty() {
            member_ids.extend_from_slice(&self.owned_objects);
        }

        for object_id in member_ids {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                object_id,
                |object_guard| {
                    if object_guard.is_any_kind_of(&[
                        KindOf::Dozer,
                        KindOf::Harvester,
                        KindOf::IgnoresSelectAll,
                    ]) {
                        return;
                    }
                    object_guard.leave_group();
                    if let Some(ai) = object_guard.get_ai_update_interface() {
                        if should_hunt {
                            ai.ai_hunt(source);
                        } else {
                            ai.ai_idle(source);
                        }
                    }
                },
            );
        }
    }

    /// Kill this player: evacuate, mark dead, kill with death FX, SP-AI resurrect.
    /// C++ Reference: Player::killPlayer() (Player.cpp:2023-2071)
    pub fn kill_player(&mut self) {
        let mut teams: Vec<Arc<RwLock<Team>>> = Vec::new();
        if let Ok(factory) = get_team_factory().lock() {
            for prototype in &self.player_team_prototypes {
                teams.extend(factory.find_team_instances(prototype.get_name().as_str()));
            }
        }
        if teams.is_empty() {
            if let Some(team) = &self.default_team {
                teams.push(Arc::clone(team));
            }
        }

        let mut member_ids: Vec<ObjectID> = Vec::new();
        for team in &teams {
            if let Ok(team_guard) = team.read() {
                member_ids.extend_from_slice(team_guard.get_members());
            }
        }
        if member_ids.is_empty() {
            member_ids.extend_from_slice(&self.owned_objects);
            if let Ok(manager) = get_object_manager().read() {
                member_ids
                    .extend(manager.get_objects_owned_by_player(self.player_index as UnsignedInt));
            }
            member_ids.sort_unstable();
            member_ids.dedup();
        }

        // C++ first pass: evacuateTeam on every instance so dumped cargo exists before kill.
        for object_id in &member_ids {
            let Some(contain_arc) = crate::object::registry::OBJECT_REGISTRY
                .with_object(*object_id, |object_guard| {
                    if object_guard.is_destroyed() || object_guard.is_effectively_dead() {
                        return None;
                    }
                    object_guard.get_contain()
                })
                .flatten()
            else {
                continue;
            };
            if let Ok(mut contain_guard) = contain_arc.lock() {
                if contain_guard.get_contain_count() > 0 {
                    let _ = contain_guard.remove_all_contained(false);
                }
            }
        }

        // Mark dead so OCLs don't spawn useful units.
        self.is_player_dead = true;

        if !teams.is_empty() {
            for team in &teams {
                if let Ok(mut team_guard) = team.write() {
                    team_guard.kill_team();
                }
            }
        } else {
            for object_id in &member_ids {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                    *object_id,
                    |object_guard| {
                        object_guard.kill(
                            Some(crate::damage::DamageType::Unresistable),
                            Some(crate::damage::DeathType::Normal),
                        );
                    },
                );
            }
        }

        self.owned_objects.clear();

        // C++: single-player computer players are resurrected so scripts can reuse the slot.
        let resurrect_sp_ai = crate::system::game_logic::get_game_logic()
            .try_lock()
            .map(|logic| logic.is_in_single_player_game())
            .unwrap_or(false)
            && self.player_type == PlayerType::Computer;
        if resurrect_sp_ai {
            self.is_player_dead = false;
            return;
        }

        let all_money = self.money.count_money();
        if all_money > 0 {
            let _ = self.money.withdraw_with_sound(all_money, false);
        }
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
        // Walk owned ids even when OBJECT_REGISTRY is empty (C++ Player list).
        // An empty registry is not a skip-close: the loop simply finds nothing.
        let _registry_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
        for object_id in &self.owned_objects {
            let Some((is_structure, ai, pos)) = crate::object::registry::OBJECT_REGISTRY
                .with_object(*object_id, |object_guard| {
                    (
                        object_guard.is_kind_of(crate::common::KindOf::Structure),
                        object_guard.get_ai_update_interface(),
                        *object_guard.get_position(),
                    )
                })
            else {
                continue;
            };
            if is_structure {
                continue;
            }
            let Some(ai) = ai else {
                continue;
            };

            if idle {
                ai.ai_move_to_position(&pos, false, source);
            } else if let Ok(mut ai_guard) = ai.lock() {
                if ai_guard.is_idle() {
                    if let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() {
                        truck.set_force_wanting_state(true);
                    }
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
        // C++: Int bounty = REAL_TO_INT_CEIL(costToBuild * m_cashBountyPercent);
        let bounty = ((killer_cost as Real) * self.cash_bounty_percent).ceil() as Int;

        // Award the bounty — C++ deposits + scoreKeeper.addMoneyEarned
        if bounty > 0 {
            let _ = self.money.deposit(bounty as u32);
            self.score_keeper.add_money_earned(bounty as u32);
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
        // C++ Player.cpp:2406-2407: no bounty for under-construction victims.
        if victim.is_under_construction() {
            return 0;
        }

        // C++ Player.cpp:2409 calcCostToBuild(victim controlling player).
        let killer_cost = victim.calc_cost_to_build();

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

    /// Get hotkey squad (const access).
    pub fn get_hotkey_squad_const(&self, squad_number: Int) -> Option<&Squad> {
        if squad_number >= 0 && (squad_number as usize) < NUM_HOTKEY_SQUADS {
            self.squads[squad_number as usize].as_ref()
        } else {
            None
        }
    }

    /// C++ `Player::removeObjectFromHotkeySquad` (`Player.cpp:3756-3767`).
    pub fn remove_object_from_hotkey_squad(&mut self, object_id: ObjectID) {
        for slot in &mut self.squads {
            if let Some(squad) = slot.as_mut() {
                squad.remove_object_id(object_id);
            }
        }
    }

    /// C++ `Player::processCreateTeamGameMessage` (`Player.cpp:3629-3648`).
    pub fn process_create_team_game_message(&mut self, hotkey_num: Int, object_ids: &[ObjectID]) {
        if hotkey_num < 0 || (hotkey_num as usize) >= NUM_HOTKEY_SQUADS {
            return;
        }
        let slot = hotkey_num as usize;
        if self.squads[slot].is_none() {
            self.squads[slot] = Some(Squad::new());
        }
        if let Some(squad) = self.squads[slot].as_mut() {
            squad.clear_squad();
        }
        for &object_id in object_ids {
            self.remove_object_from_hotkey_squad(object_id);
            if let Some(squad) = self.squads[slot].as_mut() {
                squad.add_object_id(object_id);
            }
        }
    }

    /// C++ `Player::processSelectTeamGameMessage` (`Player.cpp:3654-3678`).
    pub fn process_select_team_game_message(&mut self, hotkey_num: Int) {
        if hotkey_num < 0 || (hotkey_num as usize) >= NUM_HOTKEY_SQUADS {
            return;
        }
        let Some(squad) = self.squads[hotkey_num as usize].as_mut() else {
            return;
        };
        let ids = squad.get_live_object_ids();
        let selection = self.current_selection.get_or_insert_with(Squad::new);
        selection.clear_squad();
        for object_id in ids {
            selection.add_object_id(object_id);
        }
    }

    /// C++ `Player::processAddTeamGameMessage` (`Player.cpp:3684-3703`).
    pub fn process_add_team_game_message(&mut self, hotkey_num: Int) {
        if hotkey_num < 0 || (hotkey_num as usize) >= NUM_HOTKEY_SQUADS {
            return;
        }
        let Some(squad) = self.squads[hotkey_num as usize].as_mut() else {
            return;
        };
        let ids = squad.get_live_object_ids();
        let selection = self.current_selection.get_or_insert_with(Squad::new);
        for object_id in ids {
            selection.add_object_id(object_id);
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

    // Debug/cheat functions.
    // Getters must exist in all build profiles: can_build / cost / build-time call sites
    // use them unconditionally. In release without internal features they always return
    // false (production-safe). Toggles remain debug/internal-only.

    #[cfg(any(debug_assertions, feature = "internal"))]
    pub fn toggle_ignore_prereqs(&mut self) {
        self.demo_ignore_prereqs = !self.demo_ignore_prereqs;
    }

    pub fn ignores_prereqs(&self) -> Bool {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            self.demo_ignore_prereqs
        }
        #[cfg(not(any(debug_assertions, feature = "internal")))]
        {
            false
        }
    }

    #[cfg(any(debug_assertions, feature = "internal"))]
    pub fn toggle_free_build(&mut self) {
        self.demo_free_build = !self.demo_free_build;
    }

    pub fn builds_for_free(&self) -> Bool {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            self.demo_free_build
        }
        #[cfg(not(any(debug_assertions, feature = "internal")))]
        {
            false
        }
    }

    #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
    pub fn toggle_instant_build(&mut self) {
        self.demo_instant_build = !self.demo_instant_build;
    }

    pub fn builds_instantly(&self) -> Bool {
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        {
            self.demo_instant_build
        }
        #[cfg(not(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats")))]
        {
            false
        }
    }
}
