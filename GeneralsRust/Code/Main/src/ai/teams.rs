use super::*;

impl AIPlayer {
    /// Process team production queue
    pub(super) fn process_team_queue(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        self.reconcile_produced_units(game_logic);
        // Retail queueUnits invokes queueSupplyTruck before walking the usual
        // TeamInQueue work orders.  Its priority order therefore gets an idle
        // SupplyCenter and pays through ordinary ProductionUpdate immediately.
        self.queue_supply_truck(game_logic, current_time);
        let can_build_units = game_logic
            .get_player(self.player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true);
        if can_build_units {
            // C++ queueUnits: tryToRecruit existing map units before startTraining.
            self.recruit_waiting_work_orders(game_logic);
        }

        // Collect all factory assignments needed
        let mut factory_assignments = Vec::new();
        let mut completed_teams = Vec::new();

        for (team_index, team) in self.team_queue.iter_mut().enumerate() {
            let mut all_complete = true;

            for (order_index, work_order) in team.work_orders.iter().enumerate() {
                if work_order.num_completed < work_order.num_required {
                    // Try to queue more units
                    if work_order.factory_id.is_none()
                        && (can_build_units
                            || Self::is_dozer_work_order_template(&work_order.template_name)
                            || work_order.is_resource_gatherer)
                    {
                        factory_assignments.push((
                            team_index,
                            order_index,
                            work_order.template_name.clone(),
                            team.priority_build,
                            work_order.is_resource_gatherer,
                            work_order.supply_center_id,
                        ));
                    }

                    all_complete = false;
                }
            }

            if all_complete && !team.completed {
                team.completed = true;
                completed_teams.push(team_index);
            }
        }

        // Process factory assignments and enqueue production on the host path.
        let mut produced = 0u64;
        for (
            team_index,
            order_index,
            template_name,
            priority_build,
            is_resource_gatherer,
            supply_center_id,
        ) in factory_assignments
        {
            // C++ `startTraining(order, team->m_priorityBuild, ...)` only
            // permits a busy factory for a priority build.  Normal skirmish
            // teams must wait for an idle producer; otherwise a pre-existing
            // matching unit could be misattributed to this work order.
            let factory_id = if is_resource_gatherer {
                supply_center_id.and_then(|center_id| {
                    Self::supply_center_factory_for_collector(
                        game_logic,
                        center_id,
                        &template_name,
                        self.team,
                    )
                })
            } else {
                Self::find_factory_for_unit_ex(
                    game_logic,
                    &template_name,
                    self.team,
                    priority_build,
                )
            };
            if let Some(factory_id) = factory_id {
                // Snapshot matching output before the request is submitted.
                // A priority order may join an already-busy factory, whose
                // earlier output belongs to another order.
                let mut preexisting: Vec<ObjectId> = game_logic
                    .host_objects()
                    .iter()
                    .filter_map(|(&unit_id, unit)| {
                        (unit.team == self.team
                            && unit.producer_id == Some(factory_id)
                            && unit.template_name.eq_ignore_ascii_case(&template_name))
                        .then_some(unit_id)
                    })
                    .collect();
                preexisting.sort_by_key(|id| id.0);

                let queued = game_logic.enqueue_production(factory_id, template_name.clone());
                if let Some(team) = self.team_queue.get_mut(team_index) {
                    if let Some(work_order) = team.work_orders.get_mut(order_index) {
                        // Only bind factory on success — failed enqueue (wrong type,
                        // full queue, cash) must retry next military tick.  Record
                        // pre-existing matching output before the enqueue so a unit
                        // made for an older queue cannot complete this order.
                        if queued {
                            for unit_id in preexisting {
                                if !work_order.observed_unit_ids.contains(&unit_id) {
                                    work_order.observed_unit_ids.push(unit_id);
                                }
                            }
                            work_order.factory_id = Some(factory_id);
                            work_order.queued_count = work_order.queued_count.saturating_add(1);
                            produced = produced.saturating_add(1);
                        } else {
                            work_order.factory_id = None;
                        }
                    }
                }
            }
        }
        self.activity_count = self.activity_count.saturating_add(produced);

        // C++ checkQueuedTeams (not queueUnits) promotes completed teams.
        let _ = completed_teams;
    }

    /// C++ `isPossibleToBuildTeam` unit-cost residual:
    /// `cost += thingCost * ((minUnits+maxUnits)/2.0f)` then Int-truncate.
    pub(super) fn estimate_team_unit_cost(&self, game_logic: &GameLogic, team_name: &str) -> u32 {
        let units = Self::prototype_unit_infos(team_name);
        if !units.is_empty() {
            let mut cost: i32 = 0;
            for (name, min_u, max_u) in units {
                let unit_cost = game_logic
                    .templates
                    .get(&name)
                    .map(|t| t.build_cost.supplies)
                    .unwrap_or(0) as i32;
                // C++: cost += thingCost * ((maxUnits+minUnits)/2.0f);
                cost += (unit_cost as f32 * ((max_u as f32 + min_u as f32) / 2.0)) as i32;
            }
            return cost.max(0) as u32;
        }
        let orders = self.create_work_orders_for_team(team_name);
        let mut cost = 0u32;
        for order in orders {
            let unit_cost = game_logic
                .templates
                .get(&order.template_name)
                .map(|t| t.build_cost.supplies)
                .unwrap_or(0);
            cost = cost.saturating_add(unit_cost.saturating_mul(order.num_required as u32));
        }
        cost
    }

    /// AIData `TeamResourcesToStart` (`m_teamResourcesToBuild`). Store first,
    /// leftover `the_ai`, then the retail 0.1 residual.
    pub(super) fn team_resources_to_start_frac() -> f32 {
        let from_store = game_engine::common::ini::get_ai_data_store()
            .get_active()
            .map(|d| d.team_resources_to_build);
        let ai_store = gamelogic::ai::the_ai();let leftover = ai_store.read().ok().and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|d| d.team_resources_to_build)
        });
        from_store
            .or(leftover)
            .filter(|m| *m > 0.0)
            .unwrap_or(Self::TEAM_RESOURCES_TO_START)
    }

    /// C++ `isPossibleToBuildTeam` money residual:
    /// `cost *= m_teamResourcesToBuild` then require `money >= cost`.
    pub(super) fn can_afford_team_start(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        let full = self.estimate_team_unit_cost(game_logic, team_name) as f32;
        // C++: `cost *= m_teamResourcesToBuild` (Int *= Real truncates).
        let required = (full * Self::team_resources_to_start_frac()) as u32;
        player.resources.supplies >= required
    }

    /// C++ `isPossibleToBuildTeam` residual (money + factory existence + any idle).
    /// Production-condition scripts / maxInstances remain unported.
    pub(super) fn is_possible_to_build_team(
        &self,
        game_logic: &GameLogic,
        team_name: &str,
    ) -> bool {
        self.can_afford_team_start(game_logic, team_name)
            && self.team_factories_ready(game_logic, team_name)
    }

    /// C++ `AIPlayer::aiPreTeamDestroy(const Team *deletedTeam)`.
    /// Drop TeamInQueue entries whose `m_team` is the deleted instance.
    pub fn ai_pre_team_destroy(&mut self, deleted_team_id: Option<u32>, deleted_name: &str) {
        let keep = |q: &AITeamQueue| -> bool {
            if let (Some(qid), Some(did)) = (q.team_id, deleted_team_id) {
                // C++: team->m_team == deletedTeam
                return qid != did;
            }
            // Fallback: name compare when m_team handle was never stamped.
            if q.team_id.is_none() && !deleted_name.is_empty() {
                return q.name != deleted_name;
            }
            true
        };
        self.team_queue.retain(keep);
        self.team_ready_queue.retain(keep);
    }

    pub(super) fn leftover_team_instance_gone(team_id: Option<u32>) -> bool {
        let Some(id) = team_id else {
            return false;
        };
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .map(|factory| factory.find_team_by_id(id).is_none())
            .unwrap_or(false)
    }

    pub(super) fn leftover_instance_member_ids(team_id: Option<u32>) -> Vec<u32> {
        let Some(id) = team_id else {
            return Vec::new();
        };
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_by_id(id))
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_members().to_vec()))
            .unwrap_or_default()
    }

    /// C++ `Team::hasAnyUnits` on one instance, using live host objects.
    pub(super) fn leftover_instance_has_any_host_units(
        game_logic: &GameLogic,
        team_id: u32,
    ) -> bool {
        Self::leftover_instance_member_ids(Some(team_id))
            .into_iter()
            .any(|id| {
                game_logic.host_object(ObjectId(id)).is_some_and(|o| {
                    o.is_alive()
                        && !o.is_kind_of(KindOf::Structure)
                        && !o.is_kind_of(KindOf::Projectile)
                        && !o.is_kind_of(KindOf::Mine)
                })
            })
    }

    /// C++ `Team::countObjectsByThingTemplate` on one instance.
    pub(super) fn leftover_instance_count_template(
        game_logic: &GameLogic,
        team_id: u32,
        template_name: &str,
    ) -> u32 {
        Self::leftover_instance_member_ids(Some(team_id))
            .into_iter()
            .filter(|&id| {
                game_logic.host_object(ObjectId(id)).is_some_and(|o| {
                    Self::recruit_template_matches(template_name, &o.template_name)
                })
            })
            .count() as u32
    }

    pub(super) fn leftover_instance_first_member_pos(
        game_logic: &GameLogic,
        team_id: u32,
    ) -> Option<Vec3> {
        for id in Self::leftover_instance_member_ids(Some(team_id)) {
            if let Some(obj) = game_logic.host_object(ObjectId(id)) {
                return Some(obj.get_position());
            }
        }
        None
    }

    pub(super) fn leftover_prototype_is_singleton(team_name: &str) -> bool {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(team_name)
                    .map(|p| p.is_singleton())
            })
            .unwrap_or(false)
    }

    pub(super) fn leftover_default_team_arc(
        player_id: u32,
    ) -> Option<std::sync::Arc<std::sync::RwLock<gamelogic::team::Team>>> {
        gamelogic::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()))
    }

    pub(super) fn leftover_is_skirmish_ai(&self) -> bool {
        gamelogic::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|player| player.read().ok().map(|p| p.is_skirmish_ai()))
            .unwrap_or(false)
    }

    /// C++ `Object::setTeam` onto the destination leftover instance.
    pub(super) fn assign_host_unit_to_leftover_team(
        game_logic: &mut GameLogic,
        unit_id: ObjectId,
        dest_team_id: Option<u32>,
        dest_team_name: &str,
    ) {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            for team in factory.get_all_teams() {
                if let Ok(mut tg) = team.write() {
                    if dest_team_id != Some(tg.get_id()) {
                        tg.remove_member(unit_id.0);
                    }
                }
            }
            if let Some(id) = dest_team_id {
                if let Some(arc) = factory.find_team_by_id(id) {
                    if let Ok(mut tg) = arc.write() {
                        tg.add_member(unit_id.0);
                    }
                }
            }
        }
        if let Some(obj) = game_logic.host_object_mut(unit_id) {
            obj.team_instance_name = dest_team_name.to_string();
        }
    }

    /// C++ `TeamInQueue::disband` (`AIPlayer.cpp:3554-3566`).
    pub(super) fn disband_queued_team(&self, game_logic: &mut GameLogic, team: &AITeamQueue) {
        let default_name =
            game_logic.default_host_team_instance_name(Some(self.player_id), self.team);
        let mut member_ids: HashSet<u32> = Self::leftover_instance_member_ids(team.team_id)
            .into_iter()
            .collect();
        for order in &team.work_orders {
            member_ids.extend(order.observed_unit_ids.iter().map(|id| id.0));
        }
        for mid in &member_ids {
            if let Some(obj) = game_logic.host_object_mut(ObjectId(*mid)) {
                obj.team_instance_name = default_name.clone();
            }
        }

        let Some(src_id) = team.team_id else {
            if self.leftover_is_skirmish_ai() {
                Self::clear_leftover_team_flags();
            }
            return;
        };
        let default_arc = Self::leftover_default_team_arc(self.player_id);
        let default_id = default_arc
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_id()));
        if default_id == Some(src_id) {
            return;
        }
        if let Some(default_arc) = default_arc {
            if let Ok(mut dg) = default_arc.write() {
                for mid in &member_ids {
                    dg.add_member(*mid);
                }
            }
        }
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(src) = factory.find_team_by_id(src_id) {
                if let Ok(mut sg) = src.write() {
                    for mid in &member_ids {
                        sg.remove_member(*mid);
                    }
                }
            }
        }
        if !Self::leftover_prototype_is_singleton(&team.name) {
            if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
                factory.team_about_to_be_deleted(src_id);
            }
        }
        if self.leftover_is_skirmish_ai() {
            Self::clear_leftover_team_flags();
        }
    }

    pub(super) fn clear_leftover_team_flags() {
        if let Ok(mut eng) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(e) = eng.as_mut() {
                e.clear_team_flags();
            }
        }
    }

    pub(super) fn queue_team_members_wiped(game_logic: &GameLogic, team: &AITeamQueue) -> bool {
        let mut any = false;
        for order in &team.work_orders {
            for &id in &order.observed_unit_ids {
                any = true;
                if game_logic.host_object(id).is_some_and(|o| o.is_alive()) {
                    return false;
                }
            }
        }
        any
    }

    /// C++ Team::~Team → Player::preTeamDestroy: free the AI slot immediately.
    pub(super) fn purge_destroyed_or_wiped_queued_teams(&mut self, game_logic: &GameLogic) {
        let mut doomed: Vec<(Option<u32>, String)> = Vec::new();
        for team in self.team_ready_queue.iter() {
            if Self::leftover_team_instance_gone(team.team_id)
                || Self::queue_team_members_wiped(game_logic, team)
            {
                doomed.push((team.team_id, team.name.clone()));
            }
        }
        for team in self.team_queue.iter() {
            if Self::leftover_team_instance_gone(team.team_id)
                || ((team.completed || team.is_all_built())
                    && Self::queue_team_members_wiped(game_logic, team))
            {
                doomed.push((team.team_id, team.name.clone()));
            }
        }
        for (id, name) in doomed {
            self.ai_pre_team_destroy(id, &name);
        }
    }

    /// C++ `TeamInQueue::m_team = TheTeamFactory->createInactiveTeam(...)`.
    pub(super) fn bind_inactive_team_handle(team: &mut AITeamQueue) {
        if team.team_id.is_some() {
            return;
        }
        team.team_id = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.create_inactive_team(&team.name))
            .and_then(|arc| arc.read().ok().map(|t| t.get_id()));
    }

    /// C++ `AIPlayer::checkReadyTeams` (`AIPlayer.cpp:2729-2803`).
    pub(super) fn check_ready_teams(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        const READY_TEAM_FORCE_SECONDS: f32 = 60.0;
        let mut i = 0;
        while i < self.team_ready_queue.len() {
            let time_expired = {
                let started =
                    self.team_ready_queue[i].frame_started as f32 / LOGIC_FRAMES_PER_SECOND;
                current_time - started >= READY_TEAM_FORCE_SECONDS
            };
            let (mut all_idle, mut any_idle) = (true, false);
            if self.team_ready_queue[i].reinforcement {
                // C++ AIPlayer.cpp:2740-2745 — only the reinforcement object.
                if let Some(obj_id) = self.team_ready_queue[i].reinforcement_id {
                    if let Some(obj) = game_logic.host_object(obj_id) {
                        let idle = obj.is_alive() && obj.ai_state == AIState::Idle;
                        all_idle = idle;
                        any_idle = idle;
                    }
                }
            } else {
                let member_ids: Vec<ObjectId> = self.team_ready_queue[i]
                    .work_orders
                    .iter()
                    .flat_map(|order| order.observed_unit_ids.iter().copied())
                    .collect();
                if member_ids.is_empty() {
                    all_idle = false;
                } else {
                    for id in &member_ids {
                        // C++ guard-at-post settles into AIGuardMachine's
                        // AI_GUARD_IDLE ("Wait till something shows up to
                        // attack", AIGuard.h:33) — behaviorally idle for
                        // checkReadyTeams readiness, so the team activates and
                        // its setActive counts as player activity.
                        let idle = game_logic.host_object(*id).map(|o| {
                            o.is_alive()
                                && matches!(
                                    o.ai_state,
                                    AIState::Idle
                                        | AIState::GuardingArea
                                        | AIState::GuardingObject
                                )
                        })
                        .unwrap_or(false);
                        if idle {
                            any_idle = true;
                        } else {
                            all_idle = false;
                        }
                    }
                }
            }
            // C++ AIPlayer.cpp:2755-2761 — anyIdle shortcut only when
            // ExecutesActionsOnCreate and ProductionCondition has an Action.
            if any_idle && self.team_ready_queue[i].execute_actions {
                if Self::production_condition_has_action(&self.team_ready_queue[i].name) {
                    all_idle = true;
                }
            }
            if time_expired {
                all_idle = true;
            }
            if !all_idle {
                i += 1;
                continue;
            }
            let mut team = self.team_ready_queue.remove(i).expect("ready idx");
            team.sent_to_start_location = true;
            team.activated = true;
            team.completed = true;
            // C++ Team::setActive() → m_created; OnCreate Hunt/Guard/AttackMove
            // scripts run for this team's members only (not the whole army).
            self.activate_ready_team(game_logic, &team, current_time);
            log::debug!(
                "AI Player {} activated ready team: {}",
                self.player_id,
                team.name
            );
        }
    }

    /// C++ `Team::setActive` + OnCreate + `joinTeam` / `clearTeamFlags`.
    /// Host orders come from the OnCreate script, never a forced AttackMove.
    pub(super) fn activate_ready_team(
        &mut self,
        game_logic: &mut GameLogic,
        team: &AITeamQueue,
        current_time: f32,
    ) {
        let members: Vec<ObjectId> = team
            .work_orders
            .iter()
            .flat_map(|order| order.observed_unit_ids.iter().copied())
            .collect();

        if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
            let team_arc = factory
                .find_team_instances(&team.name)
                .into_iter()
                .next()
                .or_else(|| factory.create_inactive_team(&team.name));
            if let Some(team_arc) = team_arc {
                if let Ok(mut tg) = team_arc.write() {
                    for id in &members {
                        tg.add_member(id.0);
                    }
                    tg.set_active();
                }
            }
        }

        let on_create = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(&team.name)
                    .map(|proto| proto.get_script_on_create().to_string())
            })
            .unwrap_or_default();
        // C++ AIPlayer.cpp:2788-2792 only Team::setActive. OnCreate runs once
        // from Team::updateState on the next ScriptEngine tick.

        if team.reinforcement {
            if let Some(obj_id) = team.reinforcement_id {
                self.join_team_reinforcement_host(game_logic, obj_id, &members);
            }
        } else {
            if let Ok(mut eng) = gamelogic::scripting::engine::get_script_engine().write() {
                if let Some(e) = eng.as_mut() {
                    e.clear_team_flags();
                }
            }
            self.apply_on_create_host_orders(game_logic, &members, &on_create, current_time);
        }
        self.activity_count = self.activity_count.saturating_add(1);
    }

    /// C++ `AIUpdateInterface::joinTeam` (`AIUpdate.cpp:2516`) — copy a
    /// teammate's current order. Reinforcements never invent AttackMove.
    pub(super) fn join_team_reinforcement_host(
        &mut self,
        game_logic: &mut GameLogic,
        obj_id: ObjectId,
        members: &[ObjectId],
    ) {
        let mobile = game_logic
            .host_object(obj_id)
            .map(|o| o.is_alive() && o.is_mobile())
            .unwrap_or(false);
        if !mobile {
            return;
        }
        let other_id = members.iter().copied().find(|&id| {
            id != obj_id
                && game_logic
                    .host_object(id)
                    .map(|o| o.is_alive() && !o.is_kind_of(crate::game_logic::KindOf::Immobile))
                    .unwrap_or(false)
        });
        let Some(other_id) = other_id else {
            return;
        };
        let (other_idle, other_pos, other_state, other_target) =
            match game_logic.host_object(other_id) {
                Some(other) => (
                    other.ai_state == AIState::Idle,
                    other.get_position(),
                    other.ai_state.clone(),
                    other.target,
                ),
                None => return,
            };
        if other_idle {
            if game_logic.assign_unit_path(obj_id, other_pos, &[]) {
                game_logic.set_ai_state_decision_aware_for_ai(obj_id, AIState::Moving);
            } else if let Some(unit) = game_logic.host_object_mut(obj_id) {
                unit.move_to(other_pos);
                game_logic.set_ai_state_decision_aware_for_ai(obj_id, AIState::Moving);
            }
            return;
        }
        if let Some(unit) = game_logic.host_object_mut(obj_id) {
            unit.target = other_target;
        }
        game_logic.set_ai_state_decision_aware_for_ai(obj_id, other_state);
    }

    /// Apply leftover OnCreate script intent to live host members.
    /// C++ `checkReadyTeams` only `setActive`; Hunt/Guard/AttackMove come
    /// from the team's OnCreate script actions.
    pub(super) fn apply_on_create_host_orders(
        &mut self,
        game_logic: &mut GameLogic,
        members: &[ObjectId],
        on_create: &str,
        current_time: f32,
    ) {
        if members.is_empty() {
            return;
        }
        match Self::classify_on_create_script(on_create) {
            OnCreateIntent::Hunt => self.hunt_units(game_logic, members),
            OnCreateIntent::HuntWithCommandButton => {
                self.hunt_units_with_command_button(game_logic, members, on_create)
            }
            OnCreateIntent::Guard => self.guard_units(game_logic, members),
            OnCreateIntent::AttackMove => self.attack_move_units(game_logic, members, current_time),
            OnCreateIntent::None => {}
        }
    }

    pub(super) fn classify_on_create_script(on_create: &str) -> OnCreateIntent {
        if on_create.is_empty() || on_create == "<none>" {
            return OnCreateIntent::None;
        }
        use gamelogic::scripting::core::ScriptActionType;
        if let Ok(eng) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(e) = eng.as_ref() {
                if let Some(script) = e.find_script_clone_by_name(on_create) {
                    let mut act = script.get_action();
                    while let Some(a) = act {
                        match a.get_action_type() {
                            ScriptActionType::TeamHunt
                            | ScriptActionType::NamedHunt
                            | ScriptActionType::PlayerHunt => {
                                return OnCreateIntent::Hunt;
                            }
                            ScriptActionType::TeamHuntWithCommandButton => {
                                // C++ doTeamHuntWithCommandButton arms
                                // CommandButtonHuntUpdate, not groupHunt.
                                return OnCreateIntent::HuntWithCommandButton;
                            }
                            ScriptActionType::TeamGuard | ScriptActionType::NamedGuard => {
                                return OnCreateIntent::Guard;
                            }
                            ScriptActionType::TeamGuardArea
                            | ScriptActionType::TeamGuardPosition
                            | ScriptActionType::TeamGuardObject
                            | ScriptActionType::TeamGuardSupplyCenter
                            | ScriptActionType::TeamGuardInTunnelNetwork => {
                                // Leftover run_script queues the scripted anchor.
                                // Do not collapse to guard-at-current-position.
                                return OnCreateIntent::None;
                            }
                            ScriptActionType::TeamAttackArea
                            | ScriptActionType::TeamAttackTeam
                            | ScriptActionType::TeamAttackNamed => {
                                return OnCreateIntent::AttackMove;
                            }
                            _ => {}
                        }
                        act = a.get_next();
                    }
                }
            }
        }
        let n = on_create.to_ascii_lowercase();
        if n.contains("guardposition")
            || n.contains("guard_position")
            || n.contains("guardobject")
            || n.contains("guard_object")
            || n.contains("guardarea")
            || n.contains("guard_area")
            || n.contains("tunnel")
            || n.contains("supplycenter")
            || n.contains("supply_center")
        {
            OnCreateIntent::None
        } else if n.contains("guard") {
            OnCreateIntent::Guard
        } else if n.contains("attackmove") || n.contains("attack_move") || n.contains("team_attack")
        {
            OnCreateIntent::AttackMove
        } else if n.contains("huntwithcommand")
            || n.contains("hunt_with_command")
            || (n.contains("hunt") && n.contains("command"))
        {
            OnCreateIntent::HuntWithCommandButton
        } else if n.contains("hunt") {
            OnCreateIntent::Hunt
        } else {
            OnCreateIntent::None
        }
    }

    /// C++ `AIGroup::groupHunt` residual (`ScriptActions::doTeamHunt`).
    pub(super) fn hunt_units(&mut self, game_logic: &mut GameLogic, units: &[ObjectId]) {
        // C++ AIGroup::groupHunt: every member with AIUpdateInterface → aiHunt.
        // No can_move / Immobile / Structure gate.
        for &unit_id in units {
            let alive = game_logic
                .host_object(unit_id)
                .is_some_and(|u| u.is_alive());
            if alive {
                let _ = game_logic.unit_command_patrol(unit_id);
            }
        }
    }

    /// C++ `ScriptActions::doTeamHuntWithCommandButton` — arm CommandButtonHuntUpdate.
    pub(super) fn hunt_units_with_command_button(
        &mut self,
        game_logic: &mut GameLogic,
        units: &[ObjectId],
        on_create: &str,
    ) {
        let button = Self::on_create_command_button_name(on_create);
        for &unit_id in units {
            if game_logic.unit_can_team_hunt_with_command_button(unit_id, button.as_deref()) {
                let _ = game_logic.start_command_button_hunt_named(unit_id, button.as_deref());
            }
        }
    }

    pub(super) fn on_create_command_button_name(on_create: &str) -> Option<String> {
        use gamelogic::scripting::core::ScriptActionType;
        let engine = gamelogic::scripting::engine::get_script_engine();
        let Ok(eng) = engine.read() else {
            return None;
        };
        let e = eng.as_ref()?;
        let script = e.find_script_clone_by_name(on_create)?;
        let mut act = script.get_action();
        while let Some(a) = act {
            if a.get_action_type() == ScriptActionType::TeamHuntWithCommandButton {
                if let Some(name) = a.get_parameter(1).map(|p| p.get_string().to_string()) {
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
            act = a.get_next();
        }
        None
    }

    /// C++ `ScriptActions::doTeamGuard` — guard at current positions.
    pub(super) fn guard_units(&mut self, game_logic: &mut GameLogic, units: &[ObjectId]) {
        for &unit_id in units {
            let pos = game_logic.host_object(unit_id).and_then(|u| {
                game_logic
                    .host_unit_can_guard(unit_id)
                    .then_some(u.get_position())
            });
            if let Some(pos) = pos {
                let _ = game_logic.unit_command_guard_position(unit_id, pos);
            }
        }
    }

    /// C++ `AIPlayer::checkQueuedTeams` (`AIPlayer.cpp:2810-2870`).
    pub(super) fn check_queued_teams(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        let mut i = 0;
        while i < self.team_queue.len() {
            if !self.team_queue[i].is_build_time_expired(current_time) {
                i += 1;
                continue;
            }
            if self.team_queue[i].is_minimum_built() {
                if self.team_queue[i].are_builds_complete() {
                    if let Some(team) = self.team_queue.remove(i) {
                        self.team_ready_queue.push_front(team);
                    }
                } else {
                    i += 1;
                }
            } else if let Some(team) = self.team_queue.remove(i) {
                self.disband_queued_team(game_logic, &team);
                log::debug!(
                    "AI Player {} disbanded expired team: {}",
                    self.player_id,
                    team.name
                );
            }
        }

        let mut i = 0;
        while i < self.team_queue.len() {
            if self.team_queue[i].is_all_built() || self.team_queue[i].completed {
                if let Some(team) = self.team_queue.remove(i) {
                    self.team_ready_queue.push_front(team);
                }
                continue;
            }
            let member_ids: Vec<ObjectId> = self.team_queue[i]
                .work_orders
                .iter()
                .flat_map(|order| order.observed_unit_ids.iter().copied())
                .collect();
            let any_idle = member_ids.iter().any(|id| {
                game_logic
                    .host_object(*id)
                    .map(|o| o.is_alive() && o.ai_state == AIState::Idle)
                    .unwrap_or(false)
            });
            if any_idle {
                let team_name = self.team_queue[i].name.clone();
                let execute = self.team_queue[i].execute_actions
                    || Self::prototype_execute_actions_on_create(&team_name);
                if execute {
                    self.execute_production_condition_actions(
                        game_logic,
                        &team_name,
                        &member_ids,
                        current_time,
                    );
                }
            }
            i += 1;
        }
    }

    pub(super) fn prototype_execute_actions_on_create(team_name: &str) -> bool {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(team_name)
                    .map(|proto| proto.get_execute_actions_on_create())
            })
            .unwrap_or(false)
    }

    pub(super) fn prototype_production_condition(team_name: &str) -> String {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(team_name)
                    .map(|proto| proto.get_production_condition().to_string())
            })
            .unwrap_or_default()
    }

    /// C++ `TheScriptEngine->findScriptByName(m_productionCondition)` + `getAction()`.
    pub(super) fn production_condition_has_action(team_name: &str) -> bool {
        let cond = Self::prototype_production_condition(team_name);
        if cond.is_empty() {
            return false;
        }
        gamelogic::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|eng| {
                eng.as_ref()
                    .and_then(|e| e.find_script_clone_by_name(&cond))
            })
            .and_then(|script| script.get_action().cloned())
            .is_some()
    }

    pub(super) fn execute_production_condition_actions(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        members: &[ObjectId],
        current_time: f32,
    ) {
        let cond = Self::prototype_production_condition(team_name);
        if cond.is_empty() {
            return;
        }
        if let Ok(eng) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(e) = eng.as_ref() {
                if let Some(script) = e.find_script_clone_by_name(&cond) {
                    if let Some(action) = script.get_action().cloned() {
                        drop(eng);
                        if let Ok(mut writer) =
                            gamelogic::scripting::engine::get_script_engine().write()
                        {
                            if let Some(engine) = writer.as_mut() {
                                engine.friend_execute_action(&action, Some(team_name));
                            }
                        }
                    }
                }
            }
        }
        let idle: Vec<ObjectId> = members
            .iter()
            .copied()
            .filter(|id| {
                game_logic
                    .host_object(*id)
                    .map(|o| o.is_alive() && o.ai_state == AIState::Idle)
                    .unwrap_or(false)
            })
            .collect();
        self.apply_on_create_host_orders(game_logic, &idle, &cond, current_time);
    }

    pub(super) fn with_can_build_units<R>(
        game_logic: &mut GameLogic,
        player_id: u32,
        force: bool,
        f: impl FnOnce(&mut GameLogic) -> R,
    ) -> R {
        let prev = game_logic
            .get_player(player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true);
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(force);
        }
        let result = f(game_logic);
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(prev);
        }
        result
    }

    /// C++ `AIPlayer::doUpgradesAndSkills` (AIPlayer.cpp:2908) spends science
    /// points from an `AISideInfo` SkillSet. Host also runs a
    /// residual of `AIPlayer::buildUpgrade` (AIPlayer.cpp:1728).
    pub(super) fn do_upgrades_and_skills(&mut self, game_logic: &mut GameLogic) {
        // C++ AIPlayer.cpp:2910-2912 — can't do updates on the first few frames.
        if game_logic.get_frame() < 2 {
            return;
        }
        self.try_queue_structure_upgrade(game_logic);
        self.try_purchase_skillset_science(game_logic);
    }

    /// Retail `AIData.ini` `SideInfo` SkillSet1–5 sciences for the live team.
    pub(super) fn side_skillsets(&self) -> [&'static [&'static str]; 5] {
        match self.team {
            Team::USA => [
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_A10ThunderboltMissileStrike1",
                    "SCIENCE_A10ThunderboltMissileStrike2",
                    "SCIENCE_A10ThunderboltMissileStrike3",
                    "SCIENCE_SpectreGunshipSolo",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_Paradrop1",
                    "SCIENCE_Paradrop2",
                    "SCIENCE_Paradrop3",
                    "SCIENCE_SpyDrone",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_Pathfinder",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_A10ThunderboltMissileStrike1",
                    "SCIENCE_A10ThunderboltMissileStrike2",
                    "SCIENCE_A10ThunderboltMissileStrike3",
                    "SCIENCE_LeafletDrop",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_Pathfinder",
                    "SCIENCE_Paradrop1",
                    "SCIENCE_Paradrop2",
                    "SCIENCE_Paradrop3",
                    "SCIENCE_SpyDrone",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_A10ThunderboltMissileStrike1",
                    "SCIENCE_A10ThunderboltMissileStrike2",
                    "SCIENCE_A10ThunderboltMissileStrike3",
                    "SCIENCE_SpectreGunshipSolo",
                    "SCIENCE_DaisyCutter",
                ],
            ],
            Team::China => [
                &[
                    "SCIENCE_NukeLauncher",
                    "SCIENCE_ArtilleryTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
                &[
                    "SCIENCE_RedGuardTraining",
                    "SCIENCE_BattlemasterTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
                &[
                    "SCIENCE_NukeLauncher",
                    "SCIENCE_RedGuardTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_CarpetBomb",
                ],
                &[
                    "SCIENCE_BattlemasterTraining",
                    "SCIENCE_ArtilleryTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
                &[
                    "SCIENCE_NukeLauncher",
                    "SCIENCE_ArtilleryTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
            ],
            Team::GLA => [
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_CashBounty1",
                    "SCIENCE_CashBounty2",
                    "SCIENCE_CashBounty3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_RebelAmbush1",
                    "SCIENCE_RebelAmbush2",
                    "SCIENCE_RebelAmbush3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_TechnicalTraining",
                    "SCIENCE_CashBounty1",
                    "SCIENCE_CashBounty2",
                    "SCIENCE_CashBounty3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_CashBounty1",
                    "SCIENCE_RebelAmbush1",
                    "SCIENCE_RebelAmbush2",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_RebelAmbush1",
                    "SCIENCE_RebelAmbush2",
                    "SCIENCE_RebelAmbush3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
            ],
            _ => [&[][..], &[], &[], &[], &[]],
        }
    }

    /// C++ `AIPlayer::selectSkillset`.
    pub fn select_skillset(&mut self, skillset: i32) {
        self.skillset_selector = skillset;
    }

    /// C++ `AIPlayer::m_skillsetSelector` after `selectSkillset`.
    pub fn selected_skillset(&self) -> i32 {
        self.skillset_selector
    }

    pub(super) fn try_purchase_skillset_science(&mut self, game_logic: &mut GameLogic) {
        let points = game_logic
            .get_player(self.player_id)
            .map(|p| p.science_purchase_points)
            .unwrap_or(0);
        if points <= 0 {
            return;
        }
        let sets = self.live_side_skillsets(game_logic);
        if self.skillset_selector == INVALID_SKILLSET_SELECTION {
            let mut limit = 0i32;
            if !sets[1].is_empty() {
                limit = 1;
                if !sets[2].is_empty() {
                    limit = 2;
                    if !sets[3].is_empty() {
                        limit = 3;
                        if !sets[4].is_empty() {
                            limit = 4;
                        }
                    }
                }
            }
            // C++ AIPlayer.cpp:2944-2948 — only AISkirmishPlayer randomizes.
            // Campaign/script AIPlayer always buys SkillSet1 (selector 0).
            if self.leftover_is_skirmish_ai() {
                self.skillset_selector = self.placement_rng.next_int(0, limit);
            } else {
                self.skillset_selector = 0;
            }
        }
        let idx = self.skillset_selector.clamp(0, 4) as usize;
        let candidates = sets[idx].clone();
        let purchased: Vec<String> = {
            let Some(player) = game_logic.get_player_mut(self.player_id) else {
                return;
            };
            let mut purchased = Vec::new();
            for name in candidates {
                if player.is_capable_of_purchasing_science(&name)
                    && player.attempt_to_purchase_science(&name)
                {
                    log::debug!(
                        "AI Player {} purchases from SkillSet{} {}",
                        self.player_id,
                        idx + 1,
                        name
                    );
                    purchased.push(name);
                }
            }
            purchased
        };
        // C++ Player::addScience → onSpecialPowerCreation + setReadyFrame(now)
        // for modules whose required science matches. Human/script purchase
        // already calls this; AI skillset buys must too.
        for name in purchased {
            game_logic.on_special_power_science_creation(self.player_id, &name);
        }
    }

    /// C++ `AIPlayer::buildUpgrade` residual — queue one structure upgrade.
    pub(super) fn structure_upgrade_candidates(&self) -> &'static [&'static str] {
        use crate::game_logic::host_upgrades::{
            UPGRADE_AMERICA_ADVANCED_TRAINING, UPGRADE_AMERICA_RANGER_CAPTURE,
            UPGRADE_AMERICA_SUPPLY_LINES, UPGRADE_CHINA_NATIONALISM,
            UPGRADE_CHINA_REDGUARD_CAPTURE, UPGRADE_GLA_AP_BULLETS, UPGRADE_GLA_REBEL_CAPTURE,
        };
        match self.team {
            Team::USA => &[
                UPGRADE_AMERICA_SUPPLY_LINES,
                UPGRADE_AMERICA_RANGER_CAPTURE,
                UPGRADE_AMERICA_ADVANCED_TRAINING,
            ],
            Team::China => &[UPGRADE_CHINA_NATIONALISM, UPGRADE_CHINA_REDGUARD_CAPTURE],
            Team::GLA => &[UPGRADE_GLA_REBEL_CAPTURE, UPGRADE_GLA_AP_BULLETS],
            _ => &[],
        }
    }

    pub(super) fn preferred_upgrade_producer_names(upgrade_name: &str) -> &'static [&'static str] {
        let n = upgrade_name.to_ascii_lowercase();
        if n.contains("supplylines") {
            &["AmericaSupplyCenter", "ChinaSupplyCenter", "GLASupplyStash"]
        } else if n.contains("capture") {
            &["AmericaBarracks", "ChinaBarracks", "GLABarracks"]
        } else if n.contains("advancedtraining") {
            &["AmericaStrategyCenter"]
        } else if n.contains("nationalism") {
            &["ChinaPropagandaCenter"]
        } else if n.contains("apbullets") {
            &["GLAPalace"]
        } else {
            &[]
        }
    }

    pub(super) fn building_can_queue_upgrade(
        object: &crate::game_logic::Object,
        upgrade_name: &str,
    ) -> bool {
        if !object.is_alive() || !object.is_constructed() {
            return false;
        }
        let Some(building) = object.building_data.as_ref() else {
            return false;
        };
        if building.production_queue.len() >= crate::game_logic::DEFAULT_PRODUCTION_QUEUE_LIMIT {
            return false;
        }
        if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name)
            && object.refuses_object_upgrade(upgrade_name)
        {
            return false;
        }
        if !crate::game_logic::host_upgrades::object_can_produce_upgrade(object, upgrade_name) {
            return false;
        }

        !building
            .production_queue
            .iter()
            .any(|item| item.is_upgrade() && item.template_name.eq_ignore_ascii_case(upgrade_name))
    }

    /// C++ `AIPlayer::buildUpgrade` walks BuildList factories whose
    /// CommandSet has a button for exactly this upgrade
    /// (`Object::canProduceUpgrade`).  The residual preferred-name table is
    /// that exact CommandSet identity, so a same-team building that passes
    /// only the loose queue checks must NOT be selected.
    pub(super) fn find_upgrade_producer(
        &self,
        game_logic: &GameLogic,
        upgrade_name: &str,
    ) -> Option<ObjectId> {
        let preferred = Self::preferred_upgrade_producer_names(upgrade_name);
        game_logic.host_objects().iter().find_map(|(&id, object)| {
            let name_ok = preferred.iter().any(|name| {
                object.template_name.eq_ignore_ascii_case(name)
                    || object.get_template().name.eq_ignore_ascii_case(name)
            });
            (object.team == self.team
                && name_ok
                && Self::building_can_queue_upgrade(object, upgrade_name))
            .then_some(id)
        })
    }

    pub(super) fn try_queue_structure_upgrade(&mut self, game_logic: &mut GameLogic) {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return;
        };
        if !player.is_alive {
            return;
        }
        let candidates: Vec<&'static str> = self
            .structure_upgrade_candidates()
            .iter()
            .copied()
            .filter(|name| !player.has_unlocked_upgrade(name) && !player.has_queued_upgrade(name))
            .collect();
        for upgrade_name in candidates {
            let kind = HostUpgradeKind::from_name(upgrade_name);
            let cost = Resources {
                supplies: kind.retail_build_cost(),
                power: 0,
            };
            let Some(player) = game_logic.get_player(self.player_id) else {
                return;
            };
            if cost.supplies == 0 || !player.can_afford(&cost) {
                continue;
            }
            let Some(producer_id) = self.find_upgrade_producer(game_logic, upgrade_name) else {
                continue;
            };
            let Some(player) = game_logic.get_player_mut(self.player_id) else {
                return;
            };
            if !player.queue_upgrade(upgrade_name, &cost) {
                continue;
            }
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
                continue;
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
            return;
        }
    }

    /// Pick candidate team name for the current strategy (same as select_team_to_build).
    pub(super) fn candidate_team_name(&self) -> Option<String> {
        match self.current_strategy {
            AIStrategy::EarlyGame => self.select_early_game_team(),
            AIStrategy::MidGame => self.select_mid_game_team(),
            AIStrategy::LateGame => self.select_late_game_team(),
            AIStrategy::Desperate => self.select_desperate_team(),
        }
    }

    /// Check if AI should build a new team
    pub(super) fn should_build_new_team(&self, game_logic: &GameLogic) -> bool {
        if !game_logic
            .get_player(self.player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true)
        {
            return false;
        }
        if self.team_queue.len() >= 3 {
            return false;
        }
        self.player_team_prototype_candidates()
            .iter()
            .any(|(name, _)| self.is_a_good_idea_to_build_team(game_logic, name))
    }

    /// C++ `AIPlayer::selectTeamToBuild` — player TeamPrototypes only.
    /// Reinforce returns before the TeamSeconds arm. A new pick then sets
    /// `next_team_time` from TeamSeconds*FPS / wealth rate (no difficulty).
    pub(super) fn select_team_to_build(
        &mut self,
        game_logic: &mut GameLogic,
        current_time: f32,
    ) -> bool {
        const INVALID_PRI: i32 = -99999;
        let candidates = self.player_team_prototype_candidates();
        let mut good: Vec<(String, i32)> = Vec::new();
        let mut hi_pri = INVALID_PRI;
        for (name, pri) in candidates {
            if self.is_a_good_idea_to_build_team(game_logic, &name) {
                if pri > hi_pri {
                    hi_pri = pri;
                }
                good.push((name, pri));
            }
        }
        if self.select_team_to_reinforce(game_logic, hi_pri, current_time) {
            return true;
        }
        if hi_pri == INVALID_PRI {
            return false;
        }
        let hi: Vec<String> = good
            .into_iter()
            .filter(|(_, p)| *p == hi_pri)
            .map(|(n, _)| n)
            .collect();
        if hi.is_empty() {
            return false;
        }
        let which = if hi.len() == 1 {
            0
        } else {
            self.placement_rng.next_int(0, (hi.len() as i32) - 1) as usize
        };
        let name = &hi[which.min(hi.len() - 1)];
        // C++ `buildSpecificAITeam(teamProto, false)` — auto pick is low
        // priority. Work orders come from TeamPrototype min/max split
        // (optional max-min, then required min including 0), not invented
        // max-as-required compositions.
        if !self.build_specific_ai_team(game_logic, name, false) {
            return false;
        }
        log::debug!("AI Player {} queued team: {}", self.player_id, name);
        // C++ arms m_teamTimer only after a new-team pick, never after reinforce.
        self.arm_team_timer_after_build(game_logic, current_time);
        true
    }

    /// C++ `selectTeamToBuild` new-pick arm: ready=false, teamTimer =
    /// TeamSeconds*FPS divided by TeamsPoorRate / TeamsWealthyRate only.
    /// Difficulty does not rewrite TeamSeconds (`setAIDifficulty` assigns
    /// `m_difficulty` only).
    pub(super) fn arm_team_timer_after_build(&mut self, game_logic: &GameLogic, current_time: f32) {
        let mut timer = (self.team_seconds.max(0.0) * LOGIC_FRAMES_PER_SECOND) as u32;
        let money = game_logic
            .get_player(self.player_id)
            .map(|p| p.resources.supplies as i32)
            .unwrap_or(0);
        let (poor, wealthy, poor_mod, wealthy_mod) = Self::team_wealth_params();
        if money < poor && poor_mod > 0.0 {
            timer = (timer as f32 / poor_mod) as u32;
        } else if money > wealthy && wealthy_mod > 0.0 {
            timer = (timer as f32 / wealthy_mod) as u32;
        }
        self.next_team_time = current_time + (timer as f32 / LOGIC_FRAMES_PER_SECOND);
    }

    /// Leftover `AIPlayer::team_wealth_params`: the_ai AIData with retail
    /// Default/AIData.ini fallbacks when a field is zero / unset.
    pub(super) fn team_wealth_params() -> (i32, i32, f32, f32) {
        gamelogic::ai::the_ai()
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            Self::POOR_RESOURCES as i32
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            Self::WEALTHY_RESOURCES as i32
                        },
                        if data.team_poor_mod > 0.0 {
                            data.team_poor_mod
                        } else {
                            Self::TEAMS_POOR_RATE
                        },
                        if data.team_wealthy_mod > 0.0 {
                            data.team_wealthy_mod
                        } else {
                            Self::TEAMS_WEALTHY_RATE
                        },
                    )
                })
            })
            .unwrap_or((
                Self::POOR_RESOURCES as i32,
                Self::WEALTHY_RESOURCES as i32,
                Self::TEAMS_POOR_RATE,
                Self::TEAMS_WEALTHY_RATE,
            ))
    }

    pub(super) fn player_team_prototype_candidates(&self) -> Vec<(String, i32)> {
        let Ok(list) = gamelogic::player::player_list().read() else {
            return Vec::new();
        };
        let Some(player) = list.get_player(self.player_id as i32) else {
            return Vec::new();
        };
        let Ok(pg) = player.read() else {
            return Vec::new();
        };
        pg.get_player_team_prototypes()
            .iter()
            .map(|proto| {
                (
                    proto.get_name().to_string(),
                    proto.get_production_priority(),
                )
            })
            .collect()
    }

    /// C++ `AIPlayer::isAGoodIdeaToBuildTeam`.
    pub(super) fn is_a_good_idea_to_build_team(
        &self,
        game_logic: &GameLogic,
        team_name: &str,
    ) -> bool {
        let factory = gamelogic::team::get_team_factory();
        let Ok(guard) = factory.lock() else {
            return false;
        };
        let Some(proto) = guard.find_team_prototype(team_name) else {
            return false;
        };
        if !proto.evaluate_production_condition() {
            return false;
        }
        let instances = guard.find_team_instances(team_name).len() as i32;
        if instances >= proto.get_max_instances() {
            return false;
        }
        // C++ AIPlayer.cpp:1487-1492 — only iterate_TeamBuildQueue.
        // Ready-queue teams are idle / 60s force, not a "currently building" veto.
        // countTeamInstances() >= maxInstances is the only instance cap.
        if self.team_queue.iter().any(|t| t.name == team_name) {
            return false;
        }
        drop(guard);
        self.is_possible_to_build_team(game_logic, team_name)
    }

    /// C++ `AIPlayer::selectTeamToReinforce` (`AIPlayer.cpp:1513-1625`).
    pub(super) fn select_team_to_reinforce(
        &mut self,
        game_logic: &mut GameLogic,
        min_priority: i32,
        current_time: f32,
    ) -> bool {
        let candidates = self.collect_auto_reinforce_candidates();
        let mut best: Option<(u32, String, String, i32)> = None;
        let mut cur = min_priority;
        for cand in candidates {
            if cand.priority <= cur {
                continue;
            }
            if self.team_queue.iter().any(|t| t.name == cand.name) {
                continue;
            }
            let instances = gamelogic::team::get_team_factory()
                .lock()
                .ok()
                .map(|factory| factory.find_team_instances(&cand.name))
                .unwrap_or_default();
            for inst in instances {
                let Ok(tg) = inst.read() else {
                    continue;
                };
                let inst_id = tg.get_id();
                drop(tg);
                if !Self::leftover_instance_has_any_host_units(game_logic, inst_id) {
                    continue;
                }
                for unit in &cand.units {
                    if unit.max_units < 1 || unit.thing.is_empty() {
                        continue;
                    }
                    let count =
                        Self::leftover_instance_count_template(game_logic, inst_id, &unit.thing);
                    if count >= unit.max_units as u32 {
                        continue;
                    }
                    if Self::find_factory_for_unit_ex(game_logic, &unit.thing, self.team, false)
                        .is_none()
                    {
                        continue;
                    }
                    best = Some((
                        inst_id,
                        cand.name.clone(),
                        unit.thing.clone(),
                        cand.priority,
                    ));
                    cur = cand.priority;
                }
            }
        }
        let Some((inst_id, team_name, thing, _)) = best else {
            return false;
        };
        let mut order = AIWorkOrder::new(thing.clone(), 1, 100);
        let home = Self::leftover_instance_first_member_pos(game_logic, inst_id)
            .unwrap_or_else(|| self.team_home_or_base(&team_name));
        if let Some(unit_id) = self.try_to_recruit(game_logic, &team_name, &thing, home, None) {
            order.num_completed = 1;
            order.observed_unit_ids.push(unit_id);
            Self::assign_host_unit_to_leftover_team(game_logic, unit_id, Some(inst_id), &team_name);
            if let Some(obj) = game_logic.host_object_mut(unit_id) {
                obj.set_ai_state(AIState::Idle);
            }
        }
        let mut q = AITeamQueue::new(
            team_name,
            vec![order],
            false,
            (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
        );
        q.reinforcement = true;
        if let Some(id) = q
            .work_orders
            .first()
            .and_then(|o| o.observed_unit_ids.first())
        {
            q.reinforcement_id = Some(*id);
        }
        // C++ reinforce uses the existing team instance, not a new inactive team.
        q.team_id = Some(inst_id);
        self.team_queue.push_front(q);
        // C++ m_teamDelay = 0
        self.next_team_queue_time = current_time;
        self.activity_count = self.activity_count.saturating_add(1);
        true
    }

    pub(super) fn collect_auto_reinforce_candidates(&self) -> Vec<ReinforceCandidate> {
        let mut out = Vec::new();
        if let Ok(list) = gamelogic::player::player_list().read() {
            if let Some(player) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player.read() {
                    for proto in pg.get_player_team_prototypes() {
                        if !proto.automatically_reinforce() {
                            continue;
                        }
                        out.push(ReinforceCandidate {
                            name: proto.get_name().to_string(),
                            priority: proto.get_production_priority(),
                            units: proto
                                .units_info()
                                .iter()
                                .map(|u| ReinforceUnit {
                                    thing: u.unit_thing_name.to_string(),
                                    max_units: u.max_units,
                                })
                                .collect(),
                        });
                    }
                }
            }
        }
        if out.is_empty() {
            if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
                // Host tests / unsynced player list: scan factory prototypes.
                for proto in factory.list_team_prototypes() {
                    if !proto.automatically_reinforce() {
                        continue;
                    }
                    out.push(ReinforceCandidate {
                        name: proto.get_name().to_string(),
                        priority: proto.get_production_priority(),
                        units: proto
                            .units_info()
                            .iter()
                            .map(|u| ReinforceUnit {
                                thing: u.unit_thing_name.to_string(),
                                max_units: u.max_units,
                            })
                            .collect(),
                    });
                }
            }
        }
        out
    }

    pub(super) fn count_owned_template_units(
        &self,
        game_logic: &GameLogic,
        template_name: &str,
    ) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                Self::pad_object_still_ours(object, self.player_id, self.team)
                    && object.is_alive()
                    && !object.is_kind_of(KindOf::Structure)
                    && object.template_name.eq_ignore_ascii_case(template_name)
            })
            .count() as u32
    }

    pub(super) fn team_home_or_base(&self, team_name: &str) -> Vec3 {
        // Leftover Coord3D (X east, Y north, Z up) → host Vec3 (X, Y=up, Z=north).
        // Recruit rank uses leftover XY only (`leftover_recruit_dist_sqr`); leftover
        // Z/up must not flip nearest or push a default-team fallback past maxDist.
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(team_name) {
                if proto.has_home_location() {
                    let loc = proto.home_location();
                    return Vec3::new(loc.x, loc.z, loc.y);
                }
            }
        }
        self.base_center
    }

    /// C++/leftover `Team::tryToRecruit`: `dx = home.x-pos.x; dy = home.y-pos.y`.
    /// Host Y is leftover Z/up; leftover Y is host Z.
    pub(super) fn leftover_recruit_dist_sqr(home: Vec3, pos: Vec3) -> f32 {
        let dx = home.x - pos.x;
        let dy = home.z - pos.z;
        dx * dx + dy * dy
    }

    pub(super) fn try_to_recruit(
        &self,
        game_logic: &GameLogic,
        dest_team_name: &str,
        template_name: &str,
        home: Vec3,
        max_dist: Option<f32>,
    ) -> Option<ObjectId> {
        let mut assigned: HashSet<ObjectId> = HashSet::new();
        for team in self.team_queue.iter().chain(self.team_ready_queue.iter()) {
            for order in &team.work_orders {
                assigned.extend(order.observed_unit_ids.iter().copied());
            }
        }
        self.try_to_recruit_excluding(
            game_logic,
            dest_team_name,
            template_name,
            home,
            max_dist.unwrap_or_else(Self::aidata_max_recruit_distance),
            &assigned,
        )
    }

    pub(super) fn dest_team_production_priority(dest_team_name: &str) -> i32 {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(dest_team_name)
                    .map(|proto| proto.get_production_priority())
            })
            .unwrap_or(i32::MAX)
    }

    pub(super) fn source_is_default_team(
        game_logic: &GameLogic,
        object: &crate::game_logic::Object,
    ) -> bool {
        let name = object.team_instance_name.trim();
        if name.is_empty() {
            return true;
        }
        let default =
            game_logic.default_host_team_instance_name(object.owner_player_id, object.team);
        if name.eq_ignore_ascii_case(&default) {
            return true;
        }
        name.eq_ignore_ascii_case(&format!("team{}", object.team.get_name()))
    }

    /// `(active, proto_ai_recruitable, recruitability_set, priority, override_recruitable)`.
    pub(super) fn leftover_source_team_state(name: &str) -> (bool, bool, bool, i32, bool) {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return (true, false, false, 0, false);
        };
        let proto = factory.find_team_prototype(name);
        let priority = proto
            .as_ref()
            .map(|p| p.get_production_priority())
            .unwrap_or(0);
        let proto_ai = proto
            .as_ref()
            .map(|p| p.is_ai_recruitable())
            .unwrap_or(false);
        if let Some(team) = factory.find_team_instances(name).into_iter().next() {
            if let Ok(tg) = team.read() {
                return (
                    tg.is_active(),
                    proto_ai,
                    tg.is_recruitability_set(),
                    priority,
                    tg.is_recruitable(),
                );
            }
        }
        (true, proto_ai, false, priority, false)
    }

    pub(super) fn recruit_template_matches(dest_template: &str, candidate: &str) -> bool {
        crate::game_logic::weapon_bootstrap::splash_templates_equivalent(dest_template, candidate)
    }

    pub(super) fn try_to_recruit_excluding(
        &self,
        game_logic: &GameLogic,
        dest_team_name: &str,
        template_name: &str,
        home: Vec3,
        max_dist: f32,
        assigned: &HashSet<ObjectId>,
    ) -> Option<ObjectId> {
        let dest_priority = Self::dest_team_production_priority(dest_team_name);
        let mut dist_sqr = max_dist * max_dist;
        let mut recruit: Option<ObjectId> = None;
        for (&id, object) in game_logic.host_objects() {
            if !Self::pad_object_still_ours(object, self.player_id, self.team) {
                continue;
            }
            if !object.is_alive() {
                continue;
            }
            // C++ Team::tryToRecruit DISABLED_HELD only (Team.cpp:2353-2356).
            // No KindOf::Structure skip. No contained-by skip beyond HELD.
            if object.status.disabled_held {
                continue;
            }
            // C++ AIUpdateInterface::isRecruitable (Team.cpp:2350-2352).
            if !object.is_recruitable {
                continue;
            }
            if !Self::recruit_template_matches(template_name, &object.template_name) {
                continue;
            }
            if assigned.contains(&id) {
                continue;
            }

            let source_name = if object.team_instance_name.is_empty() {
                game_logic.default_host_team_instance_name(object.owner_player_id, object.team)
            } else {
                object.team_instance_name.clone()
            };
            let is_default = Self::source_is_default_team(game_logic, object);
            let (active, proto_ai, recruitability_set, source_priority, override_recruitable) =
                Self::leftover_source_team_state(&source_name);
            // C++: do not steal from a team still building (Team.cpp:2333-2335).
            if !active {
                continue;
            }
            // C++ source productionPriority < dest (Team.cpp:2336-2338).
            if source_priority >= dest_priority {
                continue;
            }
            let team_is_recruitable = if recruitability_set {
                override_recruitable
            } else {
                is_default || proto_ai
            };
            if !team_is_recruitable {
                continue;
            }

            let pos = object.get_position();
            let d2 = Self::leftover_recruit_dist_sqr(home, pos);
            // C++ default-team fallback even if farther than maxDist (Team.cpp:2361-2370).
            if is_default && recruit.is_none() {
                recruit = Some(id);
                dist_sqr = d2;
            }
            if d2 > dist_sqr {
                continue;
            }
            dist_sqr = d2;
            recruit = Some(id);
        }
        recruit
    }

    pub(super) fn recruit_waiting_work_orders(&mut self, game_logic: &mut GameLogic) {
        let max_dist = Self::aidata_max_recruit_distance();
        for team in self.team_queue.iter_mut() {
            Self::bind_inactive_team_handle(team);
        }
        let mut assigned: HashSet<ObjectId> = HashSet::new();
        for team in self.team_queue.iter().chain(self.team_ready_queue.iter()) {
            for order in &team.work_orders {
                assigned.extend(order.observed_unit_ids.iter().copied());
            }
        }
        let jobs: Vec<(usize, usize, String, Option<u32>, String, Vec3, bool, u32)> = self
            .team_queue
            .iter()
            .enumerate()
            .flat_map(|(ti, team)| {
                let home = self.team_home_or_base(&team.name);
                let dest_name = team.name.clone();
                let dest_id = team.team_id;
                let has_home = gamelogic::team::get_team_factory()
                    .lock()
                    .ok()
                    .and_then(|f| {
                        f.find_team_prototype(&team.name)
                            .map(|p| p.has_home_location())
                    })
                    .unwrap_or(false);
                team.work_orders
                    .iter()
                    .enumerate()
                    .filter_map(move |(oi, order)| {
                        // C++ queueSupplyTruck prepends the paid collector
                        // order and startTraining produces it through the
                        // SupplyCenter; the free SpawnBehavior starter is a
                        // default-team unit that must not be recruited into
                        // it (it is already ferrying for its center, and the
                        // paid order must complete only via real output).
                        // Combat work orders recruit map units as C++ does.
                        if !order.is_resource_gatherer
                            && order.num_completed < order.num_required
                            && order.factory_id.is_none()
                        {
                            Some((
                                ti,
                                oi,
                                dest_name.clone(),
                                dest_id,
                                order.template_name.clone(),
                                home,
                                has_home,
                                order.num_required - order.num_completed,
                            ))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut found: Vec<(usize, usize, ObjectId, Option<u32>, String, Vec3, bool)> = Vec::new();
        for (ti, oi, dest_name, dest_id, thing, home, has_home, need) in jobs {
            let mut got = 0u32;
            while got < need {
                let Some(unit_id) = self.try_to_recruit_excluding(
                    game_logic, &dest_name, &thing, home, max_dist, &assigned,
                ) else {
                    break;
                };
                assigned.insert(unit_id);
                found.push((ti, oi, unit_id, dest_id, dest_name.clone(), home, has_home));
                got = got.saturating_add(1);
            }
        }
        for (ti, oi, unit_id, dest_id, dest_name, home, has_home) in found {
            if let Some(order) = self
                .team_queue
                .get_mut(ti)
                .and_then(|t| t.work_orders.get_mut(oi))
            {
                order.num_completed = order.num_completed.saturating_add(1);
                order.observed_unit_ids.push(unit_id);
            }
            Self::assign_host_unit_to_leftover_team(game_logic, unit_id, dest_id, &dest_name);
            if has_home {
                let _ = game_logic.unit_command_move_to(unit_id, home);
            } else if let Some(obj) = game_logic.host_object_mut(unit_id) {
                obj.set_ai_state(AIState::Idle);
            }
        }
    }

    /// C++ `AIPlayer::buildSpecificAITeam`.
    pub fn build_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        priority_build: bool,
    ) -> bool {
        let can_build = game_logic
            .get_player(self.player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true);
        if !can_build {
            return false;
        }

        let proto = {
            let Ok(mut factory) = gamelogic::team::get_team_factory().lock() else {
                return false;
            };
            let Some(proto) = factory.find_team_prototype(team_name) else {
                return false;
            };
            if priority_build && proto.is_singleton() {
                if let Some(existing) = factory.find_team(team_name) {
                    if existing.read().ok().is_some_and(|eg| eg.has_any_objects()) {
                        return false;
                    }
                }
            }
            proto
        };

        let execute_actions = proto.get_execute_actions_on_create();
        let production_condition = proto.get_production_condition().to_string();
        drop(proto);

        let orders = self.build_team_work_orders(game_logic, team_name);
        if orders.is_empty() {
            return false;
        }
        // C++ `isPossibleToBuildTeam(proto, false)`: factories must exist;
        // missing money still queues.
        if !self.team_factories_exist(game_logic, &orders) {
            return false;
        }

        let team_id = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.create_inactive_team(team_name))
            .and_then(|arc| arc.read().ok().map(|t| t.get_id()));

        let mut team = AITeamQueue::new(
            team_name.to_string(),
            orders,
            priority_build,
            game_logic.frame as u32,
        );
        team.team_id = team_id;
        team.execute_actions = execute_actions;
        if priority_build {
            self.team_queue.push_front(team);
        } else {
            self.team_queue.push_back(team);
        }
        // C++ `m_teamDelay = 0` so `queueUnits` retries immediately.
        self.next_team_queue_time = 0.0;
        self.activity_count = self.activity_count.saturating_add(1);

        if execute_actions && !production_condition.is_empty() {
            if let Ok(eng) = gamelogic::scripting::engine::get_script_engine().read() {
                if let Some(e) = eng.as_ref() {
                    if let Some(script) = e.find_script_clone_by_name(&production_condition) {
                        if let Some(action) = script.get_action().cloned() {
                            drop(eng);
                            if let Ok(mut writer) =
                                gamelogic::scripting::engine::get_script_engine().write()
                            {
                                if let Some(engine) = writer.as_mut() {
                                    engine.friend_execute_action(&action, Some(team_name));
                                }
                            }
                        }
                    }
                }
            }
        }
        true
    }

    /// C++ `AIPlayer::recruitSpecificAITeam`.
    pub fn recruit_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        recruit_radius: f32,
    ) -> bool {
        let radius = if recruit_radius < 1.0 {
            99_999.0
        } else {
            recruit_radius
        };
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(team_name) {
                if proto.is_singleton() {
                    if let Some(existing) =
                        factory.find_team_instances(team_name).into_iter().next()
                    {
                        if let Ok(eg) = existing.read() {
                            if eg.has_any_objects() {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        let mut orders = self.create_work_orders_for_team(team_name);
        if orders.is_empty() {
            if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
                if let Some(proto) = factory.find_team_prototype(team_name) {
                    for unit in proto.units_info() {
                        if unit.max_units < 1 || unit.unit_thing_name.is_empty() {
                            continue;
                        }
                        orders.push(AIWorkOrder::new(
                            unit.unit_thing_name.to_string(),
                            unit.max_units.max(0) as u32,
                            100,
                        ));
                    }
                }
            }
        }
        if orders.is_empty() {
            return false;
        }
        let home = self.team_home_or_base(team_name);
        let mut recruited = 0u32;
        for order in &mut orders {
            while order.num_completed < order.num_required {
                let Some(unit_id) = self.try_to_recruit(
                    game_logic,
                    team_name,
                    &order.template_name,
                    home,
                    Some(radius),
                ) else {
                    break;
                };
                order.num_completed = order.num_completed.saturating_add(1);
                order.observed_unit_ids.push(unit_id);
                recruited = recruited.saturating_add(1);
                let _ = game_logic.unit_command_move_to(unit_id, home);
            }
        }
        if recruited == 0 {
            return false;
        }
        let mut q = AITeamQueue::new(team_name.to_string(), orders, false, 0);
        Self::bind_inactive_team_handle(&mut q);
        self.team_ready_queue.push_back(q);
        self.activity_count = self.activity_count.saturating_add(1);
        true
    }

    /// Select early game team composition
    pub(super) fn select_early_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => match self.personality {
                AIPersonality::Rush => Some("USA_RangerSquad".to_string()),
                _ => Some("USA_BasicForce".to_string()),
            },
            Team::China => match self.personality {
                AIPersonality::Rush => Some("China_RedGuardSquad".to_string()),
                _ => Some("China_BasicForce".to_string()),
            },
            Team::GLA => Some("GLA_TechnicalSquad".to_string()),
            _ => None,
        }
    }

    /// Select mid game team composition
    pub(super) fn select_mid_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_CombinedArms".to_string()),
            Team::China => Some("China_TankSquad".to_string()),
            Team::GLA => Some("GLA_HitAndRun".to_string()),
            _ => None,
        }
    }

    /// Select late game team composition
    pub(super) fn select_late_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_AdvancedStrike".to_string()),
            Team::China => Some("China_HeavyAssault".to_string()),
            Team::GLA => Some("GLA_MassAssault".to_string()),
            _ => None,
        }
    }

    /// Select desperate situation team (cheap, fast units)
    pub(super) fn select_desperate_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_RangerSquad".to_string()),
            Team::China => Some("China_RedGuardSquad".to_string()),
            Team::GLA => Some("GLA_RebelSwarm".to_string()),
            _ => None,
        }
    }

    /// Create work orders for a specific team type.
    ///
    /// Late-game names (`USA_AdvancedStrike`, `China_HeavyAssault`,
    /// `GLA_MassAssault`) keep their C++-style team identity and emit the
    /// matching retail ThingTemplate units. Unknown names stay empty so they
    /// cannot silently become default infantry (C++ `selectTeamToBuild` only
    /// queues `TeamPrototype` unit lists).
    pub(super) fn create_work_orders_for_team(&self, team_name: &str) -> Vec<AIWorkOrder> {
        let mut orders = Vec::new();

        match team_name {
            "USA_RangerSquad" => {
                orders.push(AIWorkOrder::new(
                    "AmericaInfantryRanger".to_string(),
                    4,
                    100,
                ));
            }
            "USA_BasicForce" => {
                orders.push(AIWorkOrder::new("AmericaInfantryRanger".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("AmericaVehicleHumvee".to_string(), 1, 100));
            }
            "USA_CombinedArms" => {
                orders.push(AIWorkOrder::new("AmericaInfantryRanger".to_string(), 3, 80));
                orders.push(AIWorkOrder::new("AmericaVehicleHumvee".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("USA_CrusaderTank".to_string(), 1, 100));
            }
            "USA_AdvancedStrike" => {
                orders.push(AIWorkOrder::new(
                    "AmericaInfantryMissileDefender".to_string(),
                    2,
                    80,
                ));
                orders.push(AIWorkOrder::new("AmericaTankCrusader".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("AmericaJetRaptor".to_string(), 2, 100));
            }
            "China_RedGuardSquad" => {
                orders.push(AIWorkOrder::new(
                    "ChinaInfantryRedguard".to_string(),
                    4,
                    100,
                ));
            }
            "China_BasicForce" => {
                orders.push(AIWorkOrder::new("ChinaInfantryRedguard".to_string(), 2, 90));
                orders.push(AIWorkOrder::new(
                    "ChinaTankBattleMaster".to_string(),
                    1,
                    100,
                ));
            }
            "China_TankSquad" => {
                orders.push(AIWorkOrder::new(
                    "China_BattlemasterTank".to_string(),
                    2,
                    100,
                ));
                orders.push(AIWorkOrder::new("ChinaInfantryRedguard".to_string(), 2, 80));
            }
            "China_HeavyAssault" => {
                orders.push(AIWorkOrder::new("ChinaTankBattleMaster".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("ChinaTankOverlord".to_string(), 1, 90));
                orders.push(AIWorkOrder::new("ChinaJetMIG".to_string(), 2, 100));
            }
            "GLA_TechnicalSquad" => {
                // Barracks first: infantry produces even if ArmsDealer is still building.
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 2, 100));
            }
            "GLA_RebelSwarm" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 4, 100));
            }
            "GLA_HitAndRun" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 2, 100));
            }
            "GLA_MassAssault" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("GLAVehicleTechnical".to_string(), 2, 90));
                orders.push(AIWorkOrder::new(
                    "GLAVehicleScudLauncher".to_string(),
                    1,
                    100,
                ));
            }
            _ => {}
        }

        if orders.is_empty() {
            for (name, _min_u, max_u) in Self::prototype_unit_infos(team_name) {
                if max_u > 0 {
                    orders.push(AIWorkOrder::new(name, max_u as u32, 100));
                }
            }
        }

        orders
    }

    pub(super) fn prototype_unit_infos(team_name: &str) -> Vec<(String, i32, i32)> {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return Vec::new();
        };
        let Some(proto) = factory.find_team_prototype(team_name) else {
            return Vec::new();
        };
        proto
            .units_info()
            .iter()
            .filter(|unit| !unit.unit_thing_name.is_empty())
            .map(|unit| {
                (
                    unit.unit_thing_name.to_string(),
                    unit.min_units,
                    unit.max_units,
                )
            })
            .collect()
    }

    pub(super) fn unit_template_known(&self, game_logic: &GameLogic, name: &str) -> bool {
        if game_logic.templates.contains_key(name) {
            return true;
        }
        if gamelogic::helpers::TheThingFactory::find_template(name).is_some() {
            return true;
        }
        // Live leftover ThingFactory is often empty; allow factory-mapped units.
        Self::factory_template_for_unit(name, self.team).is_some()
    }

    /// C++ `buildSpecificAITeam` work-order construction: optional (max-min)
    /// prepended, then required (min) prepended.
    pub(super) fn build_team_work_orders(
        &self,
        game_logic: &GameLogic,
        team_name: &str,
    ) -> Vec<AIWorkOrder> {
        let units = Self::prototype_unit_infos(team_name);
        if units.is_empty() {
            return self.create_work_orders_for_team(team_name);
        }
        let mut orders = Vec::new();
        for (name, min_u, max_u) in &units {
            if !self.unit_template_known(game_logic, name) {
                continue;
            }
            let count = (*max_u - *min_u).max(0);
            if count <= 0 {
                continue;
            }
            let mut order = AIWorkOrder::new(name.clone(), count as u32, 100);
            order.is_required = false;
            orders.insert(0, order);
        }
        for (name, min_u, _max_u) in &units {
            if !self.unit_template_known(game_logic, name) {
                continue;
            }
            let mut order = AIWorkOrder::new(name.clone(), (*min_u).max(0) as u32, 100);
            order.is_required = true;
            orders.insert(0, order);
        }
        orders
    }

    /// C++ `isPossibleToBuildTeam(..., requireIdleFactory=false)` factory residual.
    pub(super) fn team_factories_exist(
        &self,
        game_logic: &GameLogic,
        orders: &[AIWorkOrder],
    ) -> bool {
        if orders.is_empty() {
            return false;
        }
        orders.iter().all(|order| {
            Self::find_factory_for_unit_ex(game_logic, &order.template_name, self.team, true)
                .is_some()
        })
    }

    /// Find factory that can produce a specific unit
    pub(super) fn find_factory_for_unit(
        &self,
        game_logic: &GameLogic,
        unit_template_name: &str,
    ) -> Option<ObjectId> {
        Self::find_factory_for_unit_static(game_logic, unit_template_name, self.team)
    }

    /// Static version to avoid borrowing conflicts
    pub(super) fn find_factory_for_unit_static(
        game_logic: &GameLogic,
        unit_template_name: &str,
        team: Team,
    ) -> Option<ObjectId> {
        // Prefer idle factory (C++ findFactory(thing, busyOk=false) residual).
        Self::find_factory_for_unit_ex(game_logic, unit_template_name, team, false)
            .or_else(|| Self::find_factory_for_unit_ex(game_logic, unit_template_name, team, true))
    }

    /// Map host unit template → retail factory template residual.
    pub(super) fn factory_template_for_unit(
        unit_template_name: &str,
        team: Team,
    ) -> Option<&'static str> {
        let unit = unit_template_name.to_ascii_lowercase();
        if unit.contains("ranger")
            || unit.contains("redguard")
            || unit.contains("soldier")
            || unit.contains("rebel")
            || unit.contains("missiledefender")
            || unit.contains("pathfinder")
        {
            return match team {
                Team::USA => Some("AmericaBarracks"),
                Team::China => Some("ChinaBarracks"),
                Team::GLA => Some("GLABarracks"),
                _ => None,
            };
        }
        if unit.contains("raptor")
            || unit.contains("stealth")
            || unit.contains("aurora")
            || unit.contains("comanche")
            || unit.contains("chinook")
            || unit.contains("mig")
            || unit.contains("helix")
            || unit.contains("jet")
        {
            return match team {
                Team::USA => Some("AmericaAirfield"),
                Team::China => Some("ChinaAirfield"),
                Team::GLA => None,
                _ => None,
            };
        }
        if unit.contains("humvee")
            || unit.contains("technical")
            || unit.contains("tank")
            || unit.contains("tomahawk")
            || unit.contains("scud")
            || unit.contains("launcher")
        {
            return match team {
                Team::USA => Some("AmericaWarFactory"),
                Team::China => Some("ChinaWarFactory"),
                Team::GLA => Some("GLAArmsDealer"),
                _ => None,
            };
        }
        if unit.contains("dozer")
            || unit.contains("infantryworker")
            || (unit.contains("worker") && !unit.contains("supply"))
        {
            return match team {
                Team::USA => Some("AmericaCommandCenter"),
                Team::China => Some("ChinaCommandCenter"),
                Team::GLA => Some("GLACommandCenter"),
                _ => None,
            };
        }
        None
    }

    /// Retail factory name plus leftover USA_*/China_*/GLA_* aliases used by older tests.
    pub(super) fn factory_name_matches(object_name: &str, retail_factory: &str) -> bool {
        if object_name.eq_ignore_ascii_case(retail_factory) {
            return true;
        }
        let alias = match retail_factory {
            "AmericaBarracks" => "USA_Barracks",
            "AmericaWarFactory" => "USA_WarFactory",
            "AmericaAirfield" => "USA_Airfield",
            "AmericaCommandCenter" => "USA_CommandCenter",
            "ChinaBarracks" => "China_Barracks",
            "ChinaWarFactory" => "China_WarFactory",
            "ChinaAirfield" => "China_Airfield",
            "ChinaCommandCenter" => "China_CommandCenter",
            "GLABarracks" => "GLA_Barracks",
            "GLAArmsDealer" => "GLA_ArmsDealer",
            "GLACommandCenter" => "GLA_CommandCenter",
            _ => return false,
        };
        object_name.eq_ignore_ascii_case(alias)
    }

    /// C++ factory idle residual: empty production_queue ⇒ idle.
    pub(super) fn factory_is_idle(object: &crate::game_logic::Object) -> bool {
        object
            .building_data
            .as_ref()
            .map(|b| b.production_queue.is_empty())
            .unwrap_or(true)
    }

    /// Find constructed factory; `busy_ok=false` requires idle queue (C++ findFactory).
    pub(super) fn find_factory_for_unit_ex(
        game_logic: &GameLogic,
        unit_template_name: &str,
        team: Team,
        busy_ok: bool,
    ) -> Option<ObjectId> {
        let factory_name = Self::factory_template_for_unit(unit_template_name, team)?;
        // Pure residual acquire: prefer idle factories (priority 0) over busy (1)
        // when busy_ok; nearest 3D tiebreak for stable multi-factory choice.
        let cands: Vec<_> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                if object.team != team || !object.is_constructed() || !object.is_alive() {
                    return None;
                }
                let name_ok = Self::factory_name_matches(&object.template_name, factory_name)
                    || Self::factory_name_matches(&object.get_template().name, factory_name);
                if !name_ok {
                    return None;
                }
                let idle = Self::factory_is_idle(object);
                if !busy_ok && !idle {
                    return None;
                }
                let priority = if idle { Some(0u8) } else { Some(1u8) };
                Some(
                    crate::game_logic::host_residual_acquire::PriorityAcquireCandidate {
                        id,
                        position: object.get_position(),
                        is_alive: true,
                        priority,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_best_priority_residual_target(
            ObjectId(0),
            glam::Vec3::ZERO,
            (0.0, 0.0),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ `isPossibleToBuildTeam` factory residual (requireIdleFactory=true):
    /// every unit type has a factory, and at least one factory is idle.
    pub(super) fn team_factories_ready(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let names = self.team_unit_template_names(game_logic, team_name);
        if names.is_empty() {
            return false;
        }
        let mut any_idle = false;
        for name in &names {
            // Must have some factory that can produce this unit (busy ok for existence).
            if Self::find_factory_for_unit_ex(game_logic, name, self.team, true).is_none() {
                return false;
            }
            if Self::find_factory_for_unit_ex(game_logic, name, self.team, false).is_some() {
                any_idle = true;
            }
        }
        any_idle
    }

    /// Prototype unit templates when present; invented compositions only as
    /// a fallback for scripted alias names that have no TeamPrototype units.
    pub(super) fn team_unit_template_names(
        &self,
        game_logic: &GameLogic,
        team_name: &str,
    ) -> Vec<String> {
        let units = Self::prototype_unit_infos(team_name);
        if !units.is_empty() {
            return units
                .into_iter()
                .filter(|(name, _, _)| self.unit_template_known(game_logic, name))
                .map(|(name, _, _)| name)
                .collect();
        }
        self.create_work_orders_for_team(team_name)
            .into_iter()
            .map(|order| order.template_name)
            .collect()
    }
}
