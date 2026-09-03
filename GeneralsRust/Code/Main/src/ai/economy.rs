use super::*;

impl AIPlayer {
    /// Update economic management (base building, resource optimization)
    pub(super) fn update_economic_management(
        &mut self,
        game_logic: &mut GameLogic,
        current_time: f32,
    ) {
        if current_time < self.next_building_time {
            return;
        }

        // Check if we need more resources
        // Check resources first
        let should_build_supply = if let Some(player) = game_logic.get_player(self.player_id) {
            let resource_threshold = match self.difficulty {
                AIDifficulty::Easy => 500,
                AIDifficulty::Medium => 800,
                AIDifficulty::Hard => 1200,
                AIDifficulty::Brutal => 1500,
            };
            player.resources.supplies < resource_threshold
        } else {
            false
        };

        let should_build_power = if let Some(player) = game_logic.get_player(self.player_id) {
            player.power_available < 0
        } else {
            false
        };

        // C++ processBaseBuilding never invents extra supply/power pads.
        // Scripts stamp priority; emergency power picks an existing FS_POWER
        // list entry. Extra-pad invention is leftover of try_build_*.
        let _ = (should_build_supply, should_build_power);

        // `AISkirmishPlayer::processBaseBuilding` selects and starts one
        // structure per economic pass.  Starting every eligible entry here
        // spends the AI's money in a burst and leaves most scaffolds without a
        // dozer, which is not a viable skirmish base build lifecycle.
        self.process_building_queue(game_logic, current_time);

        // StructureSeconds residual + wealth/poor rate (AIData Structures*Rate).
        let interval = self.scaled_interval_seconds(game_logic, Self::STRUCTURE_SECONDS, true);
        self.next_building_time = current_time + interval;
    }

    /// Update military management (unit production, attack coordination).
    ///
    /// Retail `AISkirmishPlayer::doTeamBuilding` calls `queueUnits` every
    /// `m_teamDelay` (2 seconds), but only selects a *new* team after its
    /// `m_teamTimer` (`TeamSeconds`) expires.  Coupling both to TeamSeconds
    /// leaves a just-selected skirmish team idle for ten seconds.
    pub(super) fn update_military_management(
        &mut self,
        game_logic: &mut GameLogic,
        current_time: f32,
    ) {
        let queue_due = current_time >= self.next_team_queue_time;
        // C++ `AIPlayer::onUnitProduced` sets m_teamDelay to zero after the
        // factory has actually created the unit.  Poll the producer link on
        // each host-AI update so a completed unit wakes its waiting order on
        // the next frame instead of idling until the normal two-second pass.
        // `process_team_queue` reconciles again when it runs; that second
        // observation is empty and keeps the queue mutation in one place.
        let unit_completed = !queue_due && self.reconcile_produced_units(game_logic);

        if queue_due || unit_completed {
            // C++ queueUnits runs before selection, so established orders get
            // first use of an idle factory.
            self.process_team_queue(game_logic, current_time);

            let selected_team =
                if current_time >= self.next_team_time && self.should_build_new_team(game_logic) {
                    self.select_team_to_build(game_logic, current_time)
                } else {
                    false
                };

            if selected_team {
                // C++ processTeamBuilding invokes queueUnits immediately after
                // a successful selectTeamToBuild. Timer is armed only on a new
                // pick inside selectTeamToBuild; reinforce returns with ready set.
                self.process_team_queue(game_logic, current_time);
            } else if current_time >= self.next_team_time {
                // A failed selection leaves m_readyToBuildTeam set.  Retry on
                // the short m_teamDelay cadence rather than sleeping 10 sec.
                self.next_team_time = current_time + Self::TEAM_QUEUE_RETRY_SECONDS;
            }

            self.next_team_queue_time = current_time + Self::TEAM_QUEUE_RETRY_SECONDS;
        }

        // The host attack policy has its own 60-second guard.  Evaluate it on
        // manager cadence so a newly produced force is not held behind the
        // new-team selection timer.
        self.evaluate_attack_opportunities(game_logic, current_time);
    }

    /// Update strategic decisions and long-term planning
    pub(super) fn update_strategic_decisions(
        &mut self,
        game_logic: &mut GameLogic,
        current_time: f32,
    ) {
        // Update strategy based on game state
        self.update_strategy_phase(game_logic, current_time);

        // Update build phase
        self.update_build_phase(game_logic, current_time);
    }

    /// Pick a real, available construction unit as C++ `AIPlayer::findDozer`
    /// does before `AISkirmishPlayer::processBaseBuilding` starts a structure.
    /// Prefer an idle dozer, then the nearest other eligible one. Never steal
    /// `m_repairDozer`.
    pub(super) fn find_available_dozer(
        game_logic: &GameLogic,
        team: Team,
        target: Vec3,
        skip: Option<ObjectId>,
    ) -> Option<ObjectId> {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                if object.team != team || !object.is_alive() || !object.can_construct() {
                    return false;
                }
                if skip == Some(object.id) {
                    return false;
                }
                !matches!(
                    object.ai_state,
                    AIState::Constructing
                        | AIState::Repairing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Docking
                )
            })
            .map(|object| {
                let position = object.get_position();
                let dx = position.x - target.x;
                let dz = position.z - target.z;
                let busy_rank = u8::from(object.ai_state != AIState::Idle);
                (busy_rank, dx * dx + dz * dz, object.id)
            })
            .min_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            })
            .map(|(_, _, id)| id)
    }

    pub(super) fn team_has_any_dozer(game_logic: &GameLogic, team: Team) -> bool {
        game_logic
            .host_objects()
            .values()
            .any(|object| object.team == team && object.is_alive() && object.can_construct())
    }

    pub(super) fn is_dozer_work_order_template(template_name: &str) -> bool {
        let n = template_name.to_ascii_lowercase();
        n.contains("dozer") || n.contains("infantryworker") || n.ends_with("worker")
    }

    pub(super) fn dozer_already_queued(&self) -> bool {
        self.team_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| Self::is_dozer_work_order_template(&order.template_name))
        })
    }

    pub(super) fn faction_dozer_template(team: Team) -> Option<&'static str> {
        match team {
            Team::USA => Some("AmericaVehicleDozer"),
            Team::China => Some("ChinaVehicleDozer"),
            Team::GLA => Some("GLAInfantryWorker"),
            _ => None,
        }
    }

    /// C++ `AIPlayer::queueDozer` (AIPlayer.cpp:3128-3171): prepend a priority
    /// dozer work order and startTraining immediately when no KINDOF_DOZER exists.
    pub(super) fn queue_dozer(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.dozer_already_queued() {
            return;
        }
        let Some(template_name) = Self::faction_dozer_template(self.team) else {
            return;
        };
        // C++ findFactory(dozer, busyOk=true) — a busy Command Center is allowed.
        let Some(factory_id) =
            Self::find_factory_for_unit_ex(game_logic, template_name, self.team, true)
        else {
            return;
        };

        let mut preexisting: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&unit_id, unit)| {
                (unit.team == self.team
                    && unit.producer_id == Some(factory_id)
                    && unit.template_name.eq_ignore_ascii_case(template_name))
                .then_some(unit_id)
            })
            .collect();
        preexisting.sort_by_key(|id| id.0);

        let mut order = AIWorkOrder::new(template_name.to_string(), 1, 100);
        let queued = Self::with_can_build_units(game_logic, self.player_id, true, |gl| {
            gl.enqueue_production(factory_id, template_name.to_string())
        });
        if queued {
            order.factory_id = Some(factory_id);
            order.queued_count = 1;
            order.observed_unit_ids = preexisting;
        }
        self.team_queue.push_front(AITeamQueue::new(
            "DOZER - building one at the factory".to_string(),
            vec![order],
            true,
            (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
        ));
        // C++ m_teamDelay = 0 so queueUnits retries immediately.
        self.next_team_queue_time = current_time;
        if queued {
            self.activity_count = self.activity_count.saturating_add(1);
        }
    }

    /// Reattach a live dozer to every queued scaffold that lost its builder.
    ///
    /// C++ refreshes existing `BuildListInfo` entries before considering a new
    /// structure, and calls `aiResumeConstruction` whenever the remembered
    /// dozer was killed/captured or no longer has a build task.  Without this,
    /// one interrupted construction permanently blocks its planned base slot.
    pub(super) fn resume_interrupted_construction(&self, game_logic: &mut GameLogic) {
        let unfinished: Vec<(ObjectId, Vec3)> = self
            .building_queue
            .iter()
            .filter_map(|building| {
                let object_id = building.object_id?;
                let object = game_logic.host_object(object_id)?;
                (object.team == self.team && object.is_alive() && object.status.under_construction)
                    .then_some((object_id, object.get_position()))
            })
            .collect();

        for (structure_id, position) in unfinished {
            let has_live_builder = game_logic.host_objects().values().any(|object| {
                object.team == self.team
                    && object.is_alive()
                    && object.can_construct()
                    && object.target == Some(structure_id)
                    && object.ai_state == AIState::Constructing
            });
            if has_live_builder {
                continue;
            }

            if let Some(dozer_id) =
                Self::find_available_dozer(game_logic, self.team, position, self.repair_dozer)
            {
                if game_logic.resume_construction(&[dozer_id], structure_id) {
                    // `resume_construction` creates the authoritative dock;
                    // this command restores the corresponding movement/path.
                    let _ = game_logic.unit_command_begin_construct(dozer_id, position);
                }
            }
        }
    }

    /// Process one building construction start, matching
    /// `AISkirmishPlayer::processBaseBuilding`.
    ///
    /// C++ does not mass-place scaffolds and later invent construction work:
    /// it chooses one legal plan, finds a dozer, and gives that dozer the live
    /// build task immediately.  Keep an unstartable plan in the queue instead
    /// of fabricating a builder or charging the player.
    pub(super) fn process_building_queue(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        self.resume_interrupted_construction(game_logic);
        // C++ findDozer → queueDozer when no KINDOF_DOZER exists (AIPlayer.cpp:3254-3256).
        if !Self::team_has_any_dozer(game_logic, self.team) {
            self.queue_dozer(game_logic, current_time);
        }

        let is_under_powered = game_logic
            .get_player(self.player_id)
            .is_some_and(|player| player.power_available < 0);
        let build_index =
            self.select_priority_or_power_build(game_logic, current_time, is_under_powered);
        if let Some(index) = build_index {
            self.relocate_defense_if_illegal(game_logic, index);
            if let Some((template_name, position, mut build_cost)) =
                self.building_queue.get(index).and_then(|building| {
                    game_logic
                        .templates
                        .get(&building.template_name)
                        .map(|template| {
                            (
                                building.template_name.clone(),
                                building.position,
                                template.build_cost,
                            )
                        })
                })
            {
                build_cost.supplies = game_logic.modified_build_cost_supplies(
                    self.player_id,
                    &template_name,
                    build_cost.supplies,
                );
                let started =
                    Self::find_available_dozer(game_logic, self.team, position, self.repair_dozer)
                        .filter(|&dozer_id| {
                            game_logic.is_location_legal_to_build_for_builder(
                                self.team,
                                position,
                                &template_name,
                                Some(dozer_id),
                            )
                        })
                        .and_then(|dozer_id| {
                            let structure_id = game_logic.create_object_under_construction(
                                &template_name,
                                self.team,
                                position,
                            )?;

                            // The authoritative construction APIs carry the exact live
                            // association that `update_construction` consumes: target is
                            // the scaffold and the dozer receives its construct path.
                            let assigned =
                                game_logic.resume_construction(&[dozer_id], structure_id);
                            let commanded = assigned
                                && game_logic.unit_command_begin_construct(dozer_id, position);
                            let paid = commanded
                                && game_logic
                                    .get_player_mut(self.player_id)
                                    .is_some_and(|player| player.spend_resources(&build_cost));
                            if paid {
                                Some(structure_id)
                            } else {
                                // This is only an invariant failure after the earlier
                                // affordability/path checks.  Do not leave an unpaid,
                                // unassigned scaffold behind.
                                game_logic.cancel_dozers_building(structure_id);
                                game_logic.destroy_object(structure_id);
                                None
                            }
                        });

                if let Some(object_id) = started {
                    let building = &mut self.building_queue[index];
                    building.object_id = Some(object_id);
                    building.decrement_num_rebuilds();
                    // C++ setObjectTimestamp(0) once rebuild is enabled/started.
                    building.destroyed_at_time = None;
                    self.activity_count = self.activity_count.saturating_add(1);
                    log::debug!(
                        "AI Player {} building {} at {:?}",
                        self.player_id,
                        template_name,
                        position
                    );
                }
            }
        }

        // C++ processBaseBuilding: captured pads unbind; missing pads scan GLA holes.
        self.sync_build_list_object_status(game_logic, current_time);
    }

    pub(super) fn pad_object_still_ours(
        object: &crate::game_logic::Object,
        player_id: u32,
        team: Team,
    ) -> bool {
        match object.owner_player_id {
            Some(owner) => owner == player_id,
            None => object.team == team,
        }
    }

    pub(super) fn find_rebuild_hole_for_spawner(
        game_logic: &GameLogic,
        prior_id: ObjectId,
    ) -> Option<ObjectId> {
        game_logic.host_objects().iter().find_map(|(&id, object)| {
            (object.is_rebuild_hole && object.rebuild_spawner_id == Some(prior_id)).then_some(id)
        })
    }

    pub(super) fn sync_build_list_object_status(
        &mut self,
        game_logic: &GameLogic,
        current_time: f32,
    ) {
        for building in &mut self.building_queue {
            let Some(object_id) = building.object_id else {
                continue;
            };
            let prior_id = object_id;
            match game_logic
                .host_object(object_id)
                .filter(|object| object.is_alive())
            {
                // C++ findObjectByID is an existence test on the live map; a
                // destroyed-but-not-yet-reaped husk must take the destroyed path
                // (unbind + timestamp + GLA-hole scan), not the captured path.
                Some(object) => {
                    if Self::pad_object_still_ours(object, self.player_id, self.team) {
                        building.is_built = object.is_constructed() && !object.is_rebuild_hole;
                    } else {
                        // C++: captured — clear objectID + stamp timestamp.
                        building.object_id = None;
                        building.is_built = false;
                        if building.destroyed_at_time.is_none() {
                            building.destroyed_at_time = Some(current_time);
                        }
                    }
                }
                None => {
                    building.object_id = None;
                    building.is_built = false;
                    if building.destroyed_at_time.is_none() {
                        building.destroyed_at_time = Some(current_time);
                    }
                    if let Some(hole_id) = Self::find_rebuild_hole_for_spawner(game_logic, prior_id)
                    {
                        building.object_id = Some(hole_id);
                    }
                }
            }
        }
    }

    /// C++ `KINDOF_FS_POWER && !KINDOF_CASH_GENERATOR`.
    pub(super) fn template_is_power_plan(game_logic: &GameLogic, template_name: &str) -> bool {
        let Some(template) = game_logic.templates.get(template_name) else {
            return template_name.contains("PowerPlant");
        };
        let is_power =
            template.is_kind_of(KindOf::FSPower) || template.is_kind_of(KindOf::PowerPlant);
        let is_cash = template.is_kind_of(KindOf::SupplyCenter)
            || template.is_kind_of(KindOf::FSSupplyCenter);
        is_power && !is_cash
    }

    /// C++ `AISkirmishPlayer::processBaseBuilding` pick: first priority pad,
    /// then underpowered / automatic FS_POWER. Automatic pads never win
    /// (`canMakeUnit(dozer, NULL)` → `CANMAKE_NO_PREREQ`).
    pub(super) fn select_priority_or_power_build(
        &self,
        game_logic: &GameLogic,
        current_time: f32,
        is_under_powered: bool,
    ) -> Option<usize> {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return None;
        };
        let mut selected: Option<usize> = None;
        let mut selected_priority = false;
        let mut power_idx: Option<usize> = None;
        let mut power_under_construction = false;

        for (index, building) in self.building_queue.iter().enumerate() {
            if let Some(object_id) = building.object_id {
                if let Some(object) = game_logic.host_object(object_id) {
                    if object.status.under_construction
                        && Self::template_is_power_plan(game_logic, &building.template_name)
                    {
                        power_under_construction = true;
                    }
                    continue;
                }
            }
            if building.is_built
                || !building.is_buildable()
                || !building.rebuild_delay_elapsed(current_time, Self::REBUILD_DELAY_SECONDS)
            {
                continue;
            }
            let Some(template) = game_logic.templates.get(&building.template_name) else {
                continue;
            };
            if !player.can_afford(&{
                let mut cost = template.build_cost;
                cost.supplies = game_logic.modified_build_cost_supplies(
                    self.player_id,
                    &building.template_name,
                    template.build_cost.supplies,
                );
                cost
            }) {
                continue;
            }
            if !self.is_location_safe(game_logic, building.position, Some(template)) {
                continue;
            }

            if building.is_priority && !selected_priority {
                selected = Some(index);
                selected_priority = true;
            }
            if power_idx.is_none()
                && Self::template_is_power_plan(game_logic, &building.template_name)
                && (is_under_powered || building.automatic_build)
            {
                power_idx = Some(index);
            }
        }
        // C++: `if (powerPlan && powerInfo && !powerPlan->isEquivalentTo(bldgPlan))`
        // — while no FS_POWER scaffold is under construction, the power plan
        // overrides ANY selection (the isEquivalentTo check only skips an
        // identical replacement when the selection is already this power
        // plan, which changes nothing).  Automatic pads alone never win
        // (`canMakeUnit(dozer, NULL) -> CANMAKE_NO_PREREQ`).
        if let Some(power) = power_idx {
            if !power_under_construction {
                selected = Some(power);
            }
        }
        selected
    }
    /// C++ `AISkirmishPlayer::buildSpecificAIBuilding` — mark an existing
    /// unbuilt list entry as priority. Does not invent a new pad.
    pub fn build_specific_ai_building(&mut self, thing_name: &str) -> bool {
        let mut found = false;
        for building in &mut self.building_queue {
            if building.template_name != thing_name {
                continue;
            }
            found = true;
            if building.object_id.is_some() || building.is_built {
                continue;
            }
            if building.is_priority {
                continue;
            }
            building.is_priority = true;
            building.automatic_build = false;
            return true;
        }
        let _ = found;
        false
    }

    /// C++ `AIPlayer::buildBySupplies` — named template near `findSupplyCenter`.
    /// Never invents a random offset around base center.
    pub fn build_by_supplies(
        &mut self,
        game_logic: &GameLogic,
        minimum_cash: i32,
        thing_name: &str,
    ) -> bool {
        let is_cash = game_logic
            .templates
            .get(thing_name)
            .map(|t| t.is_kind_of(KindOf::SupplyCenter) || t.is_kind_of(KindOf::FSSupplyCenter))
            .unwrap_or_else(|| {
                thing_name.contains("SupplyCenter") || thing_name.contains("SupplyStash")
            });
        // C++ always findSupplyCenter first; non-cash may then use m_curWarehouseID.
        let mut warehouse_id = self.find_supply_center(game_logic, minimum_cash);
        if !is_cash {
            if let Some(id) = self.current_warehouse_id {
                if game_logic.host_object(id).is_some() {
                    warehouse_id = Some(id);
                }
            }
        }
        let Some(warehouse_id) = warehouse_id else {
            return false;
        };
        let Some(warehouse) = game_logic.host_object(warehouse_id) else {
            return false;
        };
        let warehouse_pos = warehouse.get_position();
        let mut offset = warehouse_pos - self.base_center;
        let mut radius = 30.0;
        if !is_cash {
            if let Some(enemy_id) = self.enemy_player_id {
                if let Some(enemy) = game_logic.get_player(enemy_id) {
                    let enemy_center = self.find_enemy_base_center(game_logic, enemy.team);
                    offset = warehouse_pos - enemy_center;
                }
            }
            radius = warehouse.selection_radius.max(20.0);
        }
        let len = Vec3::new(offset.x, 0.0, offset.z).length();
        if len > 0.0001 {
            offset.x /= len;
            offset.z /= len;
        } else {
            offset = Vec3::new(1.0, 0.0, 0.0);
        }
        let position = Vec3::new(
            warehouse_pos.x - offset.x * radius,
            0.0,
            warehouse_pos.z - offset.z * radius,
        );
        self.add_building(thing_name, position, 2);
        self.current_warehouse_id = Some(warehouse_id);
        true
    }

    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam`.
    /// Place a priority pad at the team's estimate position (first live member).
    pub fn build_specific_building_nearest_team(
        &mut self,
        game_logic: &GameLogic,
        thing_name: &str,
        team_name: &str,
    ) -> bool {
        if thing_name.trim().is_empty() {
            return false;
        }
        let Some(position) = Self::estimate_team_position(game_logic, team_name) else {
            return false;
        };
        self.add_building(thing_name, position, 2);
        true
    }

    /// C++ `Team::getEstimateTeamPosition` — first living member's pose.
    pub(super) fn estimate_team_position(game_logic: &GameLogic, team_name: &str) -> Option<Vec3> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return None;
        }
        let mut best: Option<(u32, Vec3)> = None;
        for (&id, object) in game_logic.host_objects() {
            if !object.is_alive() || object.status.destroyed {
                continue;
            }
            let instance = !object.team_instance_name.is_empty()
                && object.team_instance_name.eq_ignore_ascii_case(needle);
            let faction = object.team.get_name().eq_ignore_ascii_case(needle);
            if !instance && !faction {
                continue;
            }
            if best.map(|(oid, _)| id.0 < oid).unwrap_or(true) {
                best = Some((id.0, object.get_position()));
            }
        }
        best.map(|(_, pos)| pos)
    }

    /// C++ `AIPlayer::buildUpgrade` — named player upgrade at a ready factory.
    pub fn build_upgrade(&mut self, game_logic: &mut GameLogic, upgrade_name: &str) -> bool {
        if upgrade_name.trim().is_empty() {
            return false;
        }
        if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name) {
            return false;
        }
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        if !player.is_alive
            || player.has_unlocked_upgrade(upgrade_name)
            || player.has_queued_upgrade(upgrade_name)
        {
            return false;
        }
        let leftover_cost = gamelogic::upgrade::center::with_upgrade_center(|center| {
            center
                .find_upgrade(upgrade_name)
                .map(|template| template.get_cost().max(0) as u32)
        });
        let cost_supplies = leftover_cost.unwrap_or_else(|| {
            crate::game_logic::host_upgrades::resolve_upgrade_retail_cost_supplies(upgrade_name)
        });
        let cost = crate::game_logic::Resources {
            supplies: cost_supplies,
            power: 0,
        };
        if cost_supplies > 0 && !player.can_afford(&cost) {
            return false;
        }
        let Some(producer_id) = self.find_upgrade_producer(game_logic, upgrade_name) else {
            return false;
        };
        let Some(player) = game_logic.get_player_mut(self.player_id) else {
            return false;
        };
        if !player.queue_upgrade(upgrade_name, &cost) {
            return false;
        }
        let kind = crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade_name);
        let secs = kind
            .retail_build_time_secs()
            .max(1.0 / LOGIC_FRAMES_PER_SECOND);
        if !game_logic.unit_command_building_add_upgrade_to_queue(
            producer_id,
            upgrade_name,
            secs,
            cost,
        ) {
            if let Some(player) = game_logic.get_player_mut(self.player_id) {
                let _ = player.cancel_queued_upgrade(upgrade_name, &cost);
            }
            return false;
        }
        game_logic.record_host_upgrade_queued(
            self.player_id,
            self.team,
            upgrade_name,
            Some(producer_id),
        );
        game_logic.host_upgrades_mut().set_build_cost_paid(
            upgrade_name,
            self.player_id,
            cost.supplies,
        );
        let frames = (secs * LOGIC_FRAMES_PER_SECOND).round().max(1.0) as u32;
        game_logic.host_upgrades_mut().set_resolved_research_frames(
            upgrade_name,
            self.player_id,
            frames,
        );
        self.activity_count = self.activity_count.saturating_add(1);
        true
    }

    /// C++ `AIPlayer::findSupplyCenter`.
    /// Leftover-calls leftover warehouse-dock / own cash-gen / 60/40 / cash-floor pick.
    pub(crate) fn find_supply_center(
        &self,
        game_logic: &GameLogic,
        minimum_cash: i32,
    ) -> Option<ObjectId> {
        let enemy_center = self
            .enemy_structure_bounds_midpoint(game_logic)
            .map(|p| (p.x, p.z));
        let mut candidates: Vec<gamelogic::ai::ai_player::LeftoverSupplyCenterCandidate> =
            Vec::new();
        let mut own_cash_gens: Vec<gamelogic::ai::ai_player::LeftoverOwnedCashGenerator> =
            Vec::new();
        for (&id, source) in game_logic.host_objects() {
            if !source.is_alive() {
                continue;
            }
            if Self::is_host_cash_generator(source) && self.host_owned_by_us(source) {
                let p = source.get_position();
                own_cash_gens
                    .push(gamelogic::ai::ai_player::LeftoverOwnedCashGenerator { x: p.x, y: p.z });
            }
            let p = source.get_position();
            candidates.push(gamelogic::ai::ai_player::LeftoverSupplyCenterCandidate {
                id: id.0,
                x: p.x,
                y: p.z,
                bounding_circle: Self::host_object_bounding_circle(source),
                available_cash: source.stored_resources.supplies as i32,
                is_structure: source.is_kind_of(KindOf::Structure),
                is_supply_source: source.is_kind_of(KindOf::SupplySource),
                has_warehouse_dock: source.thing.template.dock_kind == DockKind::SupplyWarehouse,
                is_enemy: source.team != Team::Neutral
                    && source.team != self.team
                    && self.team != Team::Neutral,
            });
        }
        gamelogic::ai::ai_player::leftover_find_supply_center(
            &candidates,
            &own_cash_gens,
            self.base_center.x,
            self.base_center.z,
            enemy_center,
            minimum_cash,
        )
        .map(ObjectId)
    }

    pub(super) fn host_object_bounding_circle(obj: &crate::game_logic::Object) -> f32 {
        crate::game_logic::host_supply_gather::host_bounding_circle_radius(
            obj.thing.template.geometry_info.authored,
            obj.thing.template.geometry_info.bounding_circle_radius(),
            obj.thing.geometry.radius.max(obj.selection_radius),
        )
    }

    pub(super) fn host_owned_by_us(&self, obj: &crate::game_logic::Object) -> bool {
        match obj.owner_player_id {
            Some(pid) => pid == self.player_id,
            None => obj.team == self.team,
        }
    }

    pub(super) fn is_host_cash_generator(obj: &crate::game_logic::Object) -> bool {
        obj.is_kind_of(KindOf::SupplyCenter) || obj.is_kind_of(KindOf::FSSupplyCenter)
    }

    pub(super) fn own_cash_generator_near(
        &self,
        game_logic: &GameLogic,
        warehouse_pos: Vec3,
        radius: f32,
    ) -> bool {
        game_logic.host_objects().values().any(|cand| {
            if !cand.is_alive()
                || !Self::is_host_cash_generator(cand)
                || !self.host_owned_by_us(cand)
            {
                return false;
            }
            let other_r = Self::host_object_bounding_circle(cand);
            let limit = radius + other_r;
            let p = cand.get_position();
            let dx = p.x - warehouse_pos.x;
            let dz = p.z - warehouse_pos.z;
            dx * dx + dz * dz <= limit * limit
        })
    }

    pub(super) fn enemy_structure_bounds_midpoint(&self, game_logic: &GameLogic) -> Option<Vec3> {
        let enemy_team = self.skirmish_enemy_team(game_logic)?;
        let mut lo_x = f32::MAX;
        let mut lo_z = f32::MAX;
        let mut hi_x = f32::MIN;
        let mut hi_z = f32::MIN;
        let mut any = false;
        for obj in game_logic.host_objects().values() {
            if !obj.is_alive() || obj.team != enemy_team || !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let p = obj.get_position();
            lo_x = lo_x.min(p.x);
            lo_z = lo_z.min(p.z);
            hi_x = hi_x.max(p.x);
            hi_z = hi_z.max(p.z);
            any = true;
        }
        any.then_some(Vec3::new((lo_x + hi_x) * 0.5, 0.0, (lo_z + hi_z) * 0.5))
    }

    /// C++ `AIPlayer::isLocationSafe`.
    /// Leftover-calls leftover AIData radius + enemy/alive/stealth/harvester/dozer filters.
    pub fn is_location_safe(
        &self,
        game_logic: &GameLogic,
        pos: Vec3,
        template: Option<&ThingTemplate>,
    ) -> bool {
        let Some(template) = template else {
            return false;
        };
        let template_r = if template.geometry_info.authored {
            template.geometry_info.bounding_circle_radius()
        } else {
            0.0
        };
        let radius = gamelogic::ai::ai_player::leftover_is_location_safe_radius(
            Self::aidata_supply_center_safe_radius(),
            template_r,
        );
        let candidates = game_logic.host_objects().values().map(|other| {
            let p = other.get_position();
            gamelogic::ai::ai_player::LeftoverLocationSafeCandidate {
                x: p.x,
                y: p.z,
                is_destroyed: other.status.destroyed,
                is_effectively_dead: other.status.effectively_dead || !other.is_alive(),
                is_harvester: other.is_kind_of(KindOf::Harvester),
                is_dozer: other.is_kind_of(KindOf::Dozer),
                stealthed: other.status.stealthed,
                detected: other.status.detected,
                disguised: other.status.disguised,
                is_enemy: other.team != self.team && other.team != Team::Neutral,
                is_bridge: other.is_kind_of(KindOf::Bridge),
                is_bridge_tower: other.is_kind_of(KindOf::BridgeTower),
            }
        });
        gamelogic::ai::ai_player::leftover_is_location_safe(pos.x, pos.z, radius, candidates)
    }

    /// C++ `AIPlayer::isSupplySourceSafe` — find + isLocationSafe.
    pub fn is_supply_source_safe(&self, game_logic: &GameLogic, min_supplies: i32) -> bool {
        let Some(warehouse_id) = self.find_supply_center(game_logic, min_supplies) else {
            return true;
        };
        let Some(warehouse) = game_logic.host_object(warehouse_id) else {
            return true;
        };
        let template = game_logic.templates.get(&warehouse.template_name);
        self.is_location_safe(game_logic, warehouse.get_position(), template)
    }

    pub(super) fn aidata_supply_center_safe_radius() -> Option<f32> {
        let store = game_engine::common::ini::get_ai_data_store();
        let store = store.read().expect("AI data store read lock");
        if let Some(radius) = store.get_active().map(|d| d.supply_center_safe_radius) {
            return Some(radius);
        }
        drop(store);
        gamelogic::ai::the_ai().read().ok().and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|d| d.supply_center_safe_radius)
        })
    }

    /// C++ `AIPlayer::isSupplySourceAttacked`.
    ///
    /// Rate-limited (`SCAN_RATE = 10` frames). After a recent player attack,
    /// scan cash generators / dozers / harvesters for recent damage and latch
    /// `m_attackedSupplyCenter`.
    pub fn is_supply_source_attacked(&mut self, game_logic: &GameLogic) -> bool {
        const SCAN_RATE: u32 = 10;
        let cur_frame = game_logic.get_frame();
        if cur_frame == 0 {
            self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);
            return false;
        }
        self.attacked_supply_center = None;
        if cur_frame < self.supply_source_attack_check_frame {
            return false;
        }
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        if player.get_attacked_frame().saturating_add(SCAN_RATE) < cur_frame {
            return false;
        }
        self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);

        for (&id, obj) in game_logic.host_objects() {
            if obj.team != self.team || !obj.is_alive() {
                continue;
            }
            // C++ KINDOF_CASH_GENERATOR | DOZER | HARVESTER.
            let is_econ = obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || obj.is_kind_of(KindOf::Dozer)
                || obj.is_kind_of(KindOf::Harvester)
                || obj.is_kind_of(KindOf::Worker);
            if !is_econ {
                continue;
            }
            let Some(timestamp) = obj.last_damage_timestamp else {
                continue;
            };
            if timestamp.saturating_add(SCAN_RATE) > cur_frame {
                self.attacked_supply_center = Some(id);
                return true;
            }
        }
        false
    }

    pub(super) fn skirmish_enemy_team(&self, game_logic: &GameLogic) -> Option<Team> {
        if let Some(enemy_id) = self.enemy_player_id {
            if let Some(enemy) = game_logic.get_player(enemy_id) {
                if enemy.team != Team::Neutral {
                    return Some(enemy.team);
                }
            }
        }
        game_logic
            .get_players()
            .values()
            .find(|player| player.is_local && player.team != Team::Neutral)
            .map(|player| player.team)
    }

    pub(super) fn named_team_member_ids(
        &self,
        game_logic: &GameLogic,
        team_name: &str,
    ) -> Vec<ObjectId> {
        let needle = team_name.trim();
        game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() || obj.team != self.team {
                    return None;
                }
                if needle.is_empty() {
                    return obj.is_mobile().then_some(id);
                }
                (!obj.team_instance_name.is_empty()
                    && obj.team_instance_name.eq_ignore_ascii_case(needle))
                .then_some(id)
            })
            .collect()
    }

    /// C++ `AIPlayer::guardSupplyCenter`.
    ///
    /// Force attack check; prefer attacked warehouse else `findSupplyCenter`;
    /// `groupGuardPosition` toward enemy structure-bounds offset by
    /// warehouse radius * 0.8 (`CMD_FROM_SCRIPT` / `GUARDMODE_NORMAL`).
    pub fn guard_supply_center(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        min_supplies: i32,
    ) {
        self.supply_source_attack_check_frame = 0;
        let mut warehouse_id = None;
        if self.is_supply_source_attacked(game_logic) {
            warehouse_id = self.attacked_supply_center;
        }
        if warehouse_id.is_none() {
            warehouse_id = self.find_supply_center(game_logic, min_supplies);
        }
        let Some(warehouse_id) = warehouse_id else {
            return;
        };
        let Some(warehouse) = game_logic.host_object(warehouse_id) else {
            return;
        };
        let mut location = warehouse.get_position();
        let radius = warehouse.selection_radius.max(0.0) * 0.8;
        let enemy_team = self.skirmish_enemy_team(game_logic);
        if let Some(enemy_team) = enemy_team {
            let (lo_x, lo_z, hi_x, hi_z) = self.player_structure_bounds(game_logic, enemy_team);
            let mut ox = location.x - (lo_x + hi_x) * 0.5;
            let mut oz = location.z - (lo_z + hi_z) * 0.5;
            let len = (ox * ox + oz * oz).sqrt();
            if len > 0.0001 {
                ox /= len;
                oz /= len;
                location.x -= ox * radius;
                location.z -= oz * radius;
            }
        }

        let members = self.named_team_member_ids(game_logic, team_name);
        for unit_id in members {
            let mobile = game_logic
                .host_object(unit_id)
                .map(|unit| unit.is_alive() && unit.is_mobile())
                .unwrap_or(false);
            if mobile {
                let _ = game_logic.unit_command_guard_position(unit_id, location);
            }
        }
    }

    /// Default/AIData.ini `SideInfo::* ResourceGatherers*` plus the free
    /// SupplyCenter/Stash SpawnBehavior collector.  All three difficulty
    /// entries use these same values in the retail data.
    pub(super) fn desired_gatherers_per_supply_center(&self) -> u32 {
        match self.team {
            Team::USA | Team::China => 3,
            Team::GLA => 6,
            Team::Neutral => 0,
        }
    }

    /// C++ `SUPPLY_CENTER_CLOSE_DIST` = 20 * PATHFIND_CELL_SIZE_F (10).
    const SUPPLY_CENTER_CLOSE_DISTANCE: f32 = 200.0;

    /// Completed allied SupplyCenterDockUpdate owners, in stable object-id
    /// order.  C++ discovers these through its BuildListInfo records; Main's
    /// live objects are the authoritative equivalent after construction.
    pub(super) fn live_supply_centers(&self, game_logic: &GameLogic) -> Vec<ObjectId> {
        let mut centers: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == self.team
                    && object.is_alive()
                    && object.is_constructed()
                    && !object.status.sold
                    && (object.is_kind_of(KindOf::SupplyCenter)
                        || object.is_kind_of(KindOf::FSSupplyCenter)))
                .then_some(id)
            })
            .collect();
        centers.sort_by_key(|id| id.0);
        centers
    }

    /// Typed `KINDOF_SUPPLY_SOURCE` lookup near a concrete supply center.
    /// The host stores this source capability as `Resource` + `Harvestable`;
    /// do not infer sources from object names.  A finite source must still
    /// hold supplies, matching C++ SupplyWarehouseDockUpdate's available-cash
    /// check before the AI assigns additional collectors.
    pub(super) fn nearest_supply_source_for_center(
        &self,
        game_logic: &GameLogic,
        center_id: ObjectId,
    ) -> Option<ObjectId> {
        let center = game_logic.host_object(center_id)?;
        let center_position = center.get_position();
        let maximum_distance = Self::SUPPLY_CENTER_CLOSE_DISTANCE + center.selection_radius;
        let maximum_distance_squared = maximum_distance * maximum_distance;

        game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, source)| {
                if !source.is_alive()
                    || source.status.under_construction
                    || (source.team != Team::Neutral && source.team != self.team)
                    || !(source.is_kind_of(KindOf::Resource)
                        || source.is_kind_of(KindOf::Harvestable))
                    || source.stored_resources.supplies == 0
                {
                    return None;
                }
                let delta = source.get_position() - center_position;
                let distance_squared = delta.x * delta.x + delta.z * delta.z;
                (distance_squared <= maximum_distance_squared).then_some((distance_squared, id))
            })
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1 .0.cmp(&right.1 .0))
            })
            .map(|(_, id)| id)
    }

    pub(super) fn collector_count_for_supply_center(
        &self,
        game_logic: &GameLogic,
        center_id: ObjectId,
    ) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                object.team == self.team
                    && object.is_alive()
                    // C++ `AIPlayer::queueSupplyTruck` counts each
                    // SupplyTruckAIUpdate::getPreferredDockID(), not the
                    // factory/spawner that happened to create the unit.  A
                    // collector rescued after its old center dies must count
                    // toward its newly assigned depot immediately.
                    && object.preferred_dock_id == Some(center_id)
                    && object.is_resource_collector()
            })
            .count() as u32
    }

    pub(super) fn total_live_collectors(&self, game_logic: &GameLogic) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                object.team == self.team && object.is_alive() && object.is_resource_collector()
            })
            .count() as u32
    }

    /// C++ `AIPlayer::onUnitProduced`: a resource-gatherer work order makes
    /// the newly created collector want supplies and docks it at the matching
    /// SupplyCenter.  Main's typed gather state carries the source target;
    /// `preferred_dock_id` retains the C++ CMD_FROM_PLAYER dock assignment
    /// for the later ReturningResources leg.
    pub(super) fn route_collector_to_supply_center(
        &self,
        game_logic: &mut GameLogic,
        collector_id: ObjectId,
        center_id: ObjectId,
    ) -> bool {
        let Some(source_id) = self.nearest_supply_source_for_center(game_logic, center_id) else {
            return false;
        };
        let valid_collector = game_logic
            .host_object(collector_id)
            .is_some_and(|collector| {
                collector.team == self.team
                    && collector.is_alive()
                    && collector.is_resource_collector()
            });
        if !valid_collector {
            return false;
        }
        let routed = game_logic
            .unit_command_stop_moving_order_target(collector_id, Some(source_id))
            && game_logic.unit_command_set_ai_state(collector_id, AIState::Gathering);
        if routed {
            if let Some(collector) = game_logic.host_object_mut(collector_id) {
                // C++ AIPlayer::queueSupplyTruck uses aiDock(...,
                // CMD_FROM_PLAYER) specifically so SupplyTruckAIUpdate stores
                // this center in m_preferredDock.
                collector.preferred_dock_id = Some(center_id);
            }
        }
        routed
    }

    /// The SpawnBehavior collector has no AI work order, so give an idle
    /// producer-linked collector the same SupplyTruckAI wanting/dock handoff
    /// that paid collector output receives in C++ `onUnitProduced`.
    pub(super) fn route_idle_supply_center_collectors(
        &self,
        game_logic: &mut GameLogic,
        center_id: ObjectId,
    ) {
        let mut collectors: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                let spawned_here_without_a_dock =
                    object.producer_id == Some(center_id) && object.preferred_dock_id.is_none();
                let already_assigned_to_this_center = object.preferred_dock_id == Some(center_id);
                (object.team == self.team
                    && object.is_alive()
                    && object.is_resource_collector()
                    // The one-shot SpawnBehavior collector initially has a
                    // producer link but no preferred dock.  Once assigned,
                    // C++ only reissues aiDock to a non-ferrying collector
                    // whose preferred dock is this center.
                    && (spawned_here_without_a_dock || already_assigned_to_this_center)
                    && !Self::collector_is_currently_ferrying_supplies(object))
                .then_some(id)
            })
            .collect();
        collectors.sort_by_key(|id| id.0);
        for collector_id in collectors {
            let _ = self.route_collector_to_supply_center(game_logic, collector_id, center_id);
        }
    }

    /// C++ `SupplyTruckAIUpdate::isCurrentlyFerryingSupplies` returns true
    /// only in its Wanting or Docking substates.  Main's economy state owns
    /// those live legs as Gathering, ReturningResources, and Docking; do not
    /// use a generic "not idle" test here because GLA workers also carry
    /// `KINDOF_HARVESTER` while building or repairing.
    #[inline]
    pub(super) fn collector_is_currently_ferrying_supplies(collector: &Object) -> bool {
        matches!(
            collector.ai_state,
            AIState::Gathering | AIState::ReturningResources | AIState::Docking
        )
    }

    /// C++ `AIPlayer::queueSupplyTruck` first rescues one active collector
    /// whose preferred dock no longer resolves (typically after its supply
    /// center was destroyed), assigns it to the currently-understaffed center,
    /// and returns before it spends money on a replacement truck.
    pub(super) fn reattach_one_loose_supply_collector(
        &self,
        game_logic: &mut GameLogic,
        center_id: ObjectId,
    ) -> bool {
        let mut candidates: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, collector)| {
                let missing_preferred_dock = collector
                    .preferred_dock_id
                    .is_none_or(|dock_id| game_logic.host_object(dock_id).is_none());
                (collector.team == self.team
                    && collector.is_alive()
                    && collector.is_resource_collector()
                    && missing_preferred_dock
                    && Self::collector_is_currently_ferrying_supplies(collector))
                .then_some(id)
            })
            .collect();
        candidates.sort_by_key(|id| id.0);

        let Some(collector_id) = candidates.into_iter().next() else {
            return false;
        };
        let Some(collector) = game_logic.host_object_mut(collector_id) else {
            return false;
        };
        // `aiDock(center, CMD_FROM_PLAYER)` persists m_preferredDock while
        // preserving the supply sub-brain's active ferry leg.  Do not require
        // a new local warehouse here: retail reattaches before its subsequent
        // resource scan, and an already-full collector must be able to return
        // directly to the replacement center.
        collector.preferred_dock_id = Some(center_id);
        true
    }

    /// A collector work order is tied to the concrete SupplyCenter/Stash whose
    /// authored SpawnBehavior identified the collector template.  This avoids
    /// the old unit-name factory guessing and preserves C++'s real typed
    /// ProductionUpdate authorization/queue/money path.
    pub(super) fn supply_center_factory_for_collector(
        game_logic: &GameLogic,
        center_id: ObjectId,
        collector_template: &str,
        team: Team,
    ) -> Option<ObjectId> {
        let center = game_logic.host_object(center_id)?;
        if center.team != team
            || !center.is_alive()
            || !center.is_constructed()
            || !Self::factory_is_idle(center)
            || (!center.is_kind_of(KindOf::SupplyCenter)
                && !center.is_kind_of(KindOf::FSSupplyCenter))
            || !game_logic
                .templates
                .get(collector_template)
                .is_some_and(|template| template.is_kind_of(KindOf::Harvester))
        {
            return None;
        }
        (game_logic.can_make_unit(center_id, collector_template)
            == crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK)
            .then_some(center_id)
    }

    /// C++ `AIPlayer::queueSupplyTruck`: retain the free SpawnBehavior
    /// collector separately, then prepend one paid, priority collector work
    /// order when a completed supply center has a nearby live source and is
    /// below its SideInfo gatherer target.
    pub(super) fn queue_supply_truck(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.team_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| order.is_resource_gatherer)
        }) {
            return;
        }

        let desired = self.desired_gatherers_per_supply_center();
        if desired == 0 {
            return;
        }
        let total_collectors = self.total_live_collectors(game_logic);
        let centers = self.live_supply_centers(game_logic);
        for center_id in centers {
            self.route_idle_supply_center_collectors(game_logic, center_id);

            let current = self.collector_count_for_supply_center(game_logic, center_id);
            if current >= desired {
                continue;
            }
            // Do not create a paid replacement while a surviving active
            // harvester from a destroyed center can fill this slot.  C++
            // returns after one such rescue, which also preserves its
            // one-collector-per-economic-tick reassignment cadence.
            if self.reattach_one_loose_supply_collector(game_logic, center_id) {
                return;
            }
            // The first collector must be the actual one-shot SpawnBehavior
            // result, not an unpaid factory item.  Once that behavior has
            // fired, a later dead starter is eligible for normal paid
            // replacement just as C++ does after its initial `current = -1`
            // handoff becomes a real counted gatherer.
            let spawn_behavior_fired = game_logic
                .host_object(center_id)
                .is_some_and(|center| center.supply_center_spawn_behavior_fired);
            if !spawn_behavior_fired {
                continue;
            }
            if total_collectors >= desired.saturating_mul(3)
                || self
                    .nearest_supply_source_for_center(game_logic, center_id)
                    .is_none()
            {
                continue;
            }
            let Some(collector_template) =
                game_logic.supply_center_one_shot_collector_template(center_id)
            else {
                continue;
            };
            if Self::supply_center_factory_for_collector(
                game_logic,
                center_id,
                &collector_template,
                self.team,
            )
            .is_none()
            {
                continue;
            }

            let mut order = AIWorkOrder::new(collector_template, 1, 100);
            order.is_resource_gatherer = true;
            order.supply_center_id = Some(center_id);
            self.team_queue.push_front(AITeamQueue::new(
                "Supply truck".to_string(),
                vec![order],
                true,
                (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
            ));
            // C++ sets m_teamDelay=0 and proceeds through queueUnits in this
            // same pass, so process_team_queue will enqueue it immediately.
            // Do not return: retail keeps walking completed centers and may
            // begin one collector order for each in the same economic tick.
        }
    }

    /// Account for units that have physically left their producing factory.
    ///
    /// C++ `AIPlayer::onUnitProduced` receives the factory and newly-created
    /// unit directly, increments `m_numCompleted`, and clears
    /// `m_factoryID`.  Main owns the production simulation, so its equivalent
    /// is the stable `producer_id` stamped during the live completion path.
    /// Never treat a successful enqueue as a completed work order: doing so
    /// made teams disappear before any of their units existed.
    pub(super) fn reconcile_produced_units(&mut self, game_logic: &mut GameLogic) -> bool {
        let mut observed_completion = false;
        let mut completed_resource_collectors: Vec<(ObjectId, ObjectId)> = Vec::new();
        for team in &mut self.team_queue {
            for order in &mut team.work_orders {
                let Some(factory_id) = order.factory_id else {
                    continue;
                };

                // The only outstanding request for a normal work order is
                // bound to this factory.  If the producer died, C++ can no
                // longer deliver that request; allow a future queue pass to
                // find a replacement factory instead of retaining a dead ID.
                let factory_alive = game_logic
                    .host_object(factory_id)
                    .map(|factory| factory.is_alive())
                    .unwrap_or(false);
                if !factory_alive {
                    order.factory_id = None;
                    order.queued_count = 0;
                    continue;
                }

                let remaining = order.num_required.saturating_sub(order.num_completed);
                if remaining == 0 {
                    order.factory_id = None;
                    order.queued_count = 0;
                    continue;
                }

                // `producer_id` is applied before the spawned unit enters the
                // normal AI update, so this observes the real factory exit
                // rather than inferring a completion from a queue mutation.
                let mut produced: Vec<ObjectId> = game_logic
                    .host_objects()
                    .iter()
                    .filter_map(|(&unit_id, unit)| {
                        (unit.team == self.team
                            && unit.producer_id == Some(factory_id)
                            && unit
                                .template_name
                                .eq_ignore_ascii_case(&order.template_name)
                            && !order.observed_unit_ids.contains(&unit_id))
                        .then_some(unit_id)
                    })
                    .collect();
                // HashMap iteration is deliberately not an ordering contract;
                // preserve the C++ one-unit-at-a-time work-order progression.
                produced.sort_by_key(|id| id.0);

                let mut accepted = 0u32;
                for unit_id in produced.into_iter().take(remaining as usize) {
                    order.observed_unit_ids.push(unit_id);
                    if order.is_resource_gatherer {
                        if let Some(center_id) = order.supply_center_id {
                            completed_resource_collectors.push((unit_id, center_id));
                        }
                    }
                    if self.dozer_queued_for_repair
                        && Self::is_dozer_work_order_template(&order.template_name)
                    {
                        self.repair_dozer = Some(unit_id);
                        self.dozer_queued_for_repair = false;
                        if let Some(unit) = game_logic.host_object(unit_id) {
                            self.repair_dozer_origin = unit.get_position();
                        }
                    }
                    accepted = accepted.saturating_add(1);
                }
                if accepted == 0 {
                    continue;
                }

                order.num_completed = order
                    .num_completed
                    .saturating_add(accepted)
                    .min(order.num_required);
                order.queued_count = order.queued_count.saturating_sub(accepted);
                observed_completion = true;
                // `onUnitProduced` releases the factory association for the
                // next required unit.  Host queues one at a time as C++ does.
                order.factory_id = None;
            }
        }
        // C++ `onUnitProduced` performs the SupplyTruckAI wanting/dock setup
        // immediately after matching the work order.  Do it after releasing
        // the queue borrow so these are real typed gather commands, not a
        // synthetic resource credit.
        for (collector_id, center_id) in completed_resource_collectors {
            let _ = self.route_collector_to_supply_center(game_logic, collector_id, center_id);
        }
        observed_completion
    }
}
