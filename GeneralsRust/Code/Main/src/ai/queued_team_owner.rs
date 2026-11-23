//! CPP AIPlayer::checkQueuedTeams and buildSpecificAITeam callback phases.
use super::*;
use gamelogic::scripting::engine::ScriptEngine;

impl GameLogic {
    /// Resolve Player::getCurrentEnemy against this world's skirmish controllers.
    pub(crate) fn query_owned_ai_enemy(&mut self, player: u32) -> Option<u32> {
        if !self.get_player(player).is_some_and(|p| p.map_side.is_skirmish_ai) {
            return None;
        }
        AIPlayer::query_peer_enemy(self, player, &mut Vec::new())
    }

    /// C++ GameLogic::startNewGame final step (GameLogic.cpp:2069-2071):
    /// PlayerList::newMap runs each Player's AI newMap before the first
    /// script frame, so script build requests mark the AIData build list.
    pub(crate) fn new_map_owned_ai_with_effects(
        &mut self,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let data = AiDataView::from_world(self);
        let mut ids: Vec<u32> = self.ai_manager.ai_players.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let Some(ai) = self.ai_manager.ai_players.get(&id) else {
                continue;
            };
            if ai.ordinary_timing.is_some() {
                self.new_map_owned_ordinary_ai(id, ui)?;
            } else {
                self.initialize_owned_skirmish_ai_with_effects(id, &data, ui);
            }
        }
        Ok(())
    }

    /// AIPlayer.cpp:3010–3027 and Player.cpp:653–663. Admit the
    /// preplaced producers for each installed ordinary campaign controller.
    /// Two prepends in C++ leave oldest Object birth first in the BuildList.
    #[cfg(test)]
    pub(crate) fn admit_preplaced_campaign_factories(&mut self) {
        if !self.is_single_player_scenario()
            || self.ai_definitions.data().force_skirmish_ai
        {
            return;
        }
        let owners: Vec<_> = self
            .ai_manager
            .ai_players
            .keys()
            .copied()
            .filter(|&id| self.get_player(id).is_some_and(|player| {
                !player.is_human && !player.is_reserved_neutral() && !player.map_side.is_skirmish_ai
            }))
            .collect();
        for owner in owners {
            self.prepend_owned_campaign_factories(owner);
        }
    }

    pub(super) fn prepend_owned_campaign_factories(&mut self, owner: u32) {
        let mut preplaced = Vec::new();
        for object in self.host_objects().registered_values().rev() {
            if self.object_owned_by_player(object, owner)
                && AIPlayer::has_factory_production_interface(object)
            {
                preplaced.push(AIBuildingInfo::preplaced_campaign_factory(object));
            }
        }
        let has_authored_rows = self.get_player(owner)
            .is_some_and(|player| !player.map_side.build_list.is_empty());
        if let Some(ai) = self.ai_manager.ai_players.get_mut(&owner) {
            if has_authored_rows || ai.ordinary_timing.is_some() {
                preplaced.append(&mut ai.building_queue);
            }
            ai.building_queue = preplaced;
            ai.skirmish_new_map_applied = true;
        }
        self.compute_owned_ai_base_extent(owner);
    }

    /// Initial-map destruction and factory callbacks observe the installed AI.
    pub(crate) fn initialize_owned_skirmish_ai(&mut self, player: u32, data: &AiDataView) {
        self.initialize_owned_skirmish_ai_with_effects(player, data, &mut Default::default())
    }

    pub(crate) fn initialize_owned_skirmish_ai_with_effects(
        &mut self,
        player: u32,
        data: &AiDataView,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return;
        };
        if !ai.is_active || ai.skirmish_new_map_applied || ai.ordinary_timing.is_some() {
            return;
        }
        ai.skirmish_new_map_applied = true;
        let fallback = ai.base_center;
        let has_definition = ai.has_skirmish_new_map_definition(data);
        if self
            .get_player(player)
            .is_some_and(|p| !p.map_side.build_list.is_empty())
        {
            self.compute_owned_ai_base_extent(player);
            return;
        }
        if !has_definition {
            return;
        }
        let center = self.host_objects().registered().find_map(|(&id, object)| {
            (object.is_alive()
                && object.is_kind_of(KindOf::CommandCenter)
                && self.object_owned_by_player(object, player))
            .then_some((id, object.get_position()))
        });
        let start = if let Some((id, pos)) = center {
            self.destroy_object_with_effects(id, ui);
            // AISkirmishPlayer::adjustBuildList queues physical deletion;
            // the original end-of-frame phase retires this installed Object.
            pos
        } else {
            fallback
        };
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        let pads = ai.prepare_skirmish_new_map(self, data, start);
        self.ai_manager.ai_players.insert(player, ai);
        let Some(pads) = pads else {
            return;
        };
        for (mut building, initially_built) in pads {
            if initially_built {
                building.is_built = true;
                building.object_id = self.create_object_for_player_with_effects(
                    &building.template_name,
                    player,
                    building.position,
                    None,
                    ui,
                );
            }
            if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                ai.building_queue.push(building);
            }
        }
        self.compute_owned_ai_base_extent(player);
    }

    fn compute_owned_ai_base_extent(&mut self, player: u32) {
        if let Some(mut ai) = self.ai_manager.ai_players.remove(&player) {
            ai.compute_center_and_radius_of_base(self);
            self.ai_manager.ai_players.insert(player, ai);
        }
    }

    fn evaluate_owned_team_condition(&mut self, player: u32, team: &str) -> bool {
        let Some((condition, difficulty)) = self
            .ai_manager
            .ai_players
            .get(&player)
            .map(|ai| (ai.prototype_production_condition(team), ai.difficulty))
        else {
            return false;
        };
        let mut state = self
            .ai_manager
            .ai_players
            .get_mut(&player)
            .unwrap()
            .production_condition_states
            .remove(team)
            .unwrap_or_default();
        if state.condition_name != condition {
            state = OwnedProductionConditionState {
                condition_name: condition.clone(),
                ..Default::default()
            };
        }
        let result = if state.always_false {
            false
        } else if condition.is_empty() {
            state.always_false = true;
            false
        } else {
            self.with_owned_script_engine(|engine, world| {
                let cached = state.script.is_some();
                if !cached {
                    state.script = engine.find_script_clone_by_name(&condition);
                    let allowed = state
                        .script
                        .as_ref()
                        .is_some_and(|script| match difficulty {
                            AIDifficulty::Easy => script.easy,
                            AIDifficulty::Medium => script.normal,
                            AIDifficulty::Hard | AIDifficulty::Brutal => script.hard,
                        });
                    if !allowed {
                        state.always_false = true;
                        return false;
                    }
                }
                let script = state.script.as_mut().unwrap();
                if cached {
                    if world.frame < script.frame_to_evaluate_at {
                        return false;
                    }
                    if script.delay_evaluation_seconds > 0 {
                        script.frame_to_evaluate_at = world.frame.wrapping_add(
                            (script.delay_evaluation_seconds as u32).wrapping_mul(30),
                        );
                    }
                }
                let owner_name = world.get_player(player).map(|p| p.name.clone());
                world.evaluate_owned_script_conditions(engine, script, owner_name.as_deref())
            })
            .unwrap_or(false)
        };
        if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
            ai.production_condition_states
                .insert(team.to_owned(), state);
        }
        result
    }

    /// CPP queueUnits → selectTeamToBuild → immediate queueUnits. The
    /// selection script runs with all canonical controllers installed.
    pub(crate) fn update_owned_ai_military(&mut self, player: u32, now: f32, data: &AiDataView) {
        self.update_owned_ai_military_with_effects(player, now, data, &mut Default::default())
            .expect("owned AI military update failed");
    }

    pub(crate) fn update_owned_ai_military_with_effects(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use super::teams::OwnedTeamSelection;
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return Ok(());
        };
        let due = now >= ai.next_team_queue_time;
        let queue_result = if due {
            ai.process_team_queue_with_ai_data(self, now, data)
        } else {
            Ok(())
        };
        self.ai_manager.ai_players.insert(player, ai);
        queue_result?;
        let candidates = self
            .ai_manager
            .ai_players
            .get(&player)
            .filter(|ai| {
                now >= ai.next_team_queue_time
                    && now >= ai.next_team_time
                    && ai.can_attempt_team_selection(self)
            })
            .map_or_else(Vec::new, AIPlayer::player_team_prototype_candidates);
        let conditions = self.evaluate_owned_ai_team_conditions(player, &candidates);
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return Ok(());
        };
        // selectTeamToBuild considers reinforcement even when every new
        // prototype fails its production condition or instance limit.
        let selection = if due && now >= ai.next_team_time && ai.can_attempt_team_selection(self) {
            ai.prepare_team_selection_with_conditions(
                self,
                now,
                data,
                Some(&conditions),
                Some(candidates),
            )
        } else {
            OwnedTeamSelection::None
        };
        self.ai_manager.ai_players.insert(player, ai);
        let (selected, new_pick) = match selection {
            OwnedTeamSelection::None => (false, false),
            OwnedTeamSelection::Reinforced => (true, false),
            OwnedTeamSelection::New(name) => {
                let selected =
                    self.build_owned_specific_ai_team_with_effects(player, &name, false, None, ui);
                (selected, selected)
            }
        };
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return Ok(());
        };
        let queue_result = if due {
            if new_pick {
                ai.arm_team_timer_after_build_with_ai_data(self, now, data);
            }
            let result = if selected {
                ai.process_team_queue_with_ai_data(self, now, data)
            } else {
                Ok(())
            };
            if !selected && now >= ai.next_team_time {
                ai.next_team_time = now + AIPlayer::TEAM_QUEUE_RETRY_SECONDS;
            }
            ai.next_team_queue_time = now + AIPlayer::TEAM_QUEUE_RETRY_SECONDS;
            result
        } else {
            Ok(())
        };
        if queue_result.is_ok() {
            ai.evaluate_attack_opportunities(self, now);
        }
        self.ai_manager.ai_players.insert(player, ai);
        queue_result?;
        Ok(())
    }
    fn evaluate_owned_ai_team_conditions(
        &mut self,
        player: u32,
        candidates: &[(String, i32)],
    ) -> HashMap<String, bool> {
        candidates
            .iter()
            .map(|(name, _)| {
                let result = self.evaluate_owned_team_condition(player, name);
                (name.clone(), result)
            })
            .collect()
    }

    fn run_owned_ordinary_ai_queue_units(
        &mut self,
        player: u32,
        now: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return Ok(());
        };
        let epoch = self.host_visual_world_epoch();
        let result = ai.process_ordinary_team_queue(self, now, effects);
        if self.host_visual_world_epoch() == epoch {
            self.ai_manager.ai_players.insert(player, ai);
        }
        result
    }

    /// AIPlayer.cpp:2620–2626, 2877–2902. Player permission pauses both
    /// countdowns; only the ordinary controller runs this 150-frame cadence.
    pub(super) fn update_owned_ordinary_ai_team_building_with_effects(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use super::teams::OwnedTeamSelection;

        if !self.get_player(player).is_some_and(|p| p.can_build_units) {
            return Ok(());
        }
        let Some(timing) = self
            .ai_manager
            .ai_players
            .get_mut(&player)
            .and_then(|ai| ai.ordinary_timing.as_mut())
        else {
            return Ok(());
        };
        if !timing.ready_to_build_team {
            timing.team_timer = timing.team_timer.wrapping_sub(1);
            if timing.team_timer <= 0 {
                timing.ready_to_build_team = true;
                timing.team_delay = 0;
            }
        }
        timing.team_delay = timing.team_delay.wrapping_sub(1);
        if timing.team_delay >= 1 {
            return Ok(());
        }

        self.run_owned_ordinary_ai_queue_units(player, now, ui)?;
        let candidates = self
            .ai_manager
            .ai_players
            .get(&player)
            .filter(|ai| {
                ai.ordinary_timing
                    .is_some_and(|timing| timing.ready_to_build_team)
            })
            .map(AIPlayer::player_team_prototype_candidates);
        let selection = if let Some(candidates) = candidates {
            // Evaluate authored conditions with the canonical controllers installed.
            let conditions = self.evaluate_owned_ai_team_conditions(player, &candidates);
            let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
                return Ok(());
            };
            let selection = ai.prepare_team_selection_with_conditions(
                self,
                now,
                data,
                Some(&conditions),
                Some(candidates),
            );
            self.ai_manager.ai_players.insert(player, ai);
            selection
        } else {
            OwnedTeamSelection::None
        };
        let selected = match selection {
            OwnedTeamSelection::None => false,
            OwnedTeamSelection::Reinforced => true,
            OwnedTeamSelection::New(name) => {
                // AIPlayer.cpp:1699–1707 arms the timer after the void build
                // call even when the admission or an authored action fails.
                self.build_owned_specific_ai_team_with_effects(player, &name, false, None, ui);
                let money = self.get_player(player).map_or(0, |p| p.resources.supplies);
                let definitions = self.ai_definitions.data();
                if let Some(timing) = self
                    .ai_manager
                    .ai_players
                    .get_mut(&player)
                    .and_then(|ai| ai.ordinary_timing.as_mut())
                {
                    timing.ready_to_build_team = false;
                    let timer = timing.team_seconds.wrapping_mul(30);
                    timing.team_timer = if money < definitions.resources_poor as u32 {
                        (timer as f32 / definitions.team_poor_mod) as i32
                    } else if money > definitions.resources_wealthy as u32 {
                        (timer as f32 / definitions.team_wealthy_mod) as i32
                    } else {
                        timer
                    };
                }
                true
            }
        };
        let result = if selected {
            self.run_owned_ordinary_ai_queue_units(player, now, ui)
        } else {
            Ok(())
        };

        // The final write wins over any synchronous team admission shortcut.
        if let Some(timing) = self
            .ai_manager
            .ai_players
            .get_mut(&player)
            .and_then(|ai| ai.ordinary_timing.as_mut())
        {
            timing.team_delay = 5 * 30;
        }
        result
    }

    // Explicit UI-only adapter for standalone callers; native roots lend both domains.
    pub(crate) fn update_owned_ai_military_with_ui(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) {
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: None,
            script_events: None,
            partition: None,
            trigger_world: None,
            audio: None,
            terrain: None,
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
            view: None,
        };
        self.update_owned_ai_military_with_effects(player, now, data, &mut effects)
            .expect("owned AI military UI update failed");
    }

    /// Commit queue admission before its synchronous production-condition action.
    pub(crate) fn build_owned_specific_ai_team(
        &mut self,
        player: u32,
        name: &str,
        priority: bool,
        interpreter: Option<&ScriptEngine>,
    ) -> bool {
        self.build_owned_specific_ai_team_with_effects(
            player,
            name,
            priority,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn build_owned_specific_ai_team_with_effects(
        &mut self,
        player: u32,
        name: &str,
        priority: bool,
        interpreter: Option<&ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let prepared = (|| {
            let ai = self.ai_manager.ai_players.get(&player)?;
            if !self.get_player(player).is_some_and(|p| p.can_build_units) {
                return None;
            }
            let factory = ai.team_factory.clone();
            let execute = {
                let mut definitions = factory.lock().ok()?;
                let prototype = definitions.find_team_prototype(name)?;
                if priority
                    && prototype.is_singleton()
                    && definitions
                        .find_team(name)
                        .is_some_and(|team| team.read().ok().is_some_and(|t| t.has_any_objects()))
                {
                    return None;
                }
                prototype.get_execute_actions_on_create()
            };
            let orders = ai.build_team_work_orders(self, name);
            Some((factory, orders, execute))
        })();
        let Some((factory, orders, execute)) = prepared else {
            return false;
        };
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return false;
        };
        let available = ai.team_factories_exist(self, &orders);
        self.ai_manager.ai_players.insert(player, ai);
        if !available {
            return false;
        }
        if let Err(error) = self.require_owned_team_creation_interpreter(name, true, interpreter) {
            log::error!("AI queued Team creation failed: {error}");
            return false;
        }
        // CPP AIPlayer.cpp2480 admits the work-order node before the
        // factory's null-team action. Nested actions must observe that node.
        let mut team = AITeamQueue::new(name.to_owned(), orders, priority, self.frame);
        team.execute_actions = execute;
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return false;
        };
        let token = ai.next_queue_visit_token;
        team.visit_token = token;
        ai.next_queue_visit_token = token
            .checked_add(1)
            .expect("AI queue node token exhaustion");
        if priority {
            ai.team_queue.push_front(team);
        } else {
            ai.team_queue.push_back(team);
        }
        let creation = factory
            .lock()
            .ok()
            .and_then(|mut definitions| definitions.create_inactive_team_with_owned_effects(name));
        let Some((created, effects)) = creation else {
            if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                ai.team_queue.retain(|node| node.visit_token != token);
            }
            return false;
        };
        let Some(team_id) = created.read().ok().map(|team| team.get_id()) else {
            return false;
        };
        if let Err(error) = self.run_owned_team_creation_actions(&created, effects, interpreter, ui)
        {
            // C++ assigns TeamInQueue::m_team only after createInactiveTeam's
            // null-Team action returns. Keep that ordering here: after the
            // callback has returned (even with an error), bind the exact Team
            // receipt so a later queue pass cannot recreate it through the
            // legacy ambient TeamFactory path. A reentrant callback may have
            // removed/replaced the Team or its queue node; only bind when both
            // original identities still exist.
            let still_admitted = factory
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .find_team_by_id(team_id)
                .filter(|current| std::sync::Arc::ptr_eq(current, &created));
            if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                if let Some(index) = ai
                    .team_queue
                    .iter()
                    .position(|node| node.visit_token == token)
                {
                    if still_admitted.is_some() {
                        ai.team_queue[index].team_id = Some(team_id);
                    } else {
                        ai.team_queue.retain(|node| node.visit_token != token);
                    }
                }
            }
            log::error!("AI queued Team creation failed: {error}");
            return false;
        }
        // Never attach a removed/replaced team or another node after a callback.
        let admitted_actions = factory.lock().ok().and_then(|definitions| {
            let current = definitions.find_team_by_id(team_id)?;
            if !std::sync::Arc::ptr_eq(&current, &created) {
                return None;
            }
            let prototype = definitions.find_team_prototype(name)?;
            Some((
                prototype.get_execute_actions_on_create(),
                prototype.get_production_condition().to_owned(),
            ))
        });
        let still_admitted = admitted_actions.is_some();
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return false;
        };
        let Some(node) = ai
            .team_queue
            .iter_mut()
            .find(|node| node.visit_token == token)
        else {
            return false;
        };
        if !still_admitted {
            ai.team_queue.retain(|node| node.visit_token != token);
            return false;
        }
        node.team_id = Some(team_id);
        ai.wake_team_queue();
        ai.activity_count = ai.activity_count.saturating_add(1);
        // CPP rereads the admitted prototype after the null-team callback.
        let (execute, condition) = admitted_actions.unwrap();
        if execute && !condition.is_empty() {
            if let Some(engine) = interpreter {
                self.execute_owned_ai_condition_with_effects(engine, &condition, Some(team_id), ui);
            } else {
                self.with_owned_script_engine(|engine, world| {
                    world.execute_owned_ai_condition_with_effects(
                        engine,
                        &condition,
                        Some(team_id),
                        ui,
                    );
                });
            }
        }
        true
    }

    pub(crate) fn execute_owned_ai_condition(
        &mut self,
        engine: &ScriptEngine,
        name: &str,
        team: Option<u32>,
    ) {
        self.execute_owned_ai_condition_with_effects(engine, name, team, &mut Default::default())
    }

    pub(crate) fn execute_owned_ai_condition_with_effects(
        &mut self,
        engine: &ScriptEngine,
        name: &str,
        team: Option<u32>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) =
            self.execute_owned_ai_condition_with_effects_checked(engine, name, team, ui)
        {
            log::error!("{error}");
        }
    }

    pub(crate) fn execute_owned_ai_condition_with_effects_checked(
        &mut self,
        engine: &ScriptEngine,
        name: &str,
        team: Option<u32>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(action) = engine
            .find_script_clone_by_name(name)
            .and_then(|script| script.get_action().cloned())
        {
            self.execute_script_team_actions_with_effects_checked(engine, &action, team, ui)?;
        }

        Ok(())
    }

    /// Only the queue-decision borrow survives within one phase. Scripts and
    /// destruction execute with every controller installed in its session.
    pub(crate) fn check_owned_ai_queued_teams(&mut self, player: u32, now: f32) {
        self.check_owned_ai_queued_teams_with_effects(player, now, &mut Default::default())
    }

    pub(crate) fn check_owned_ai_queued_teams_with_effects(
        &mut self,
        player: u32,
        now: f32,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let skirmish = AiWorldView::new(self).player_is_skirmish_ai(player);
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return;
        };
        let frame = self.frame;
        let ordinary = ai.ordinary_timing.is_some();
        let skirmish = skirmish && !ordinary;
        for node in ai.team_queue.iter_mut() {
            if node.visit_token == 0 {
                node.visit_token = ai.next_queue_visit_token;
                ai.next_queue_visit_token = ai
                    .next_queue_visit_token
                    .checked_add(1)
                    .expect("AI queue node token exhaustion");
            }
        }
        // Node identities survive synchronous insertion/removal. Promotion
        // restarts at the head, exactly as the original linked-list iterator.
        loop {
            let decision = self.ai_manager.ai_players.get(&player).and_then(|ai| {
                ai.team_queue.iter().enumerate().find_map(|(index, team)| {
                    let expired = if ordinary {
                        team.is_ordinary_build_time_expired(&ai.team_factory, frame)
                    } else {
                        team.is_build_time_expired(&ai.team_factory, now)
                    };
                    let minimum = team.is_minimum_built();
                    (expired && (!minimum || team.are_builds_complete()))
                        .then_some((index, minimum))
                })
            });
            let Some((index, minimum)) = decision else {
                break;
            };
            let ai = self.ai_manager.ai_players.get_mut(&player).unwrap();
            let team = ai.team_queue.remove(index).unwrap();
            if minimum {
                ai.team_ready_queue.push_front(team);
            } else {
                self.disband_owned_ai_queued_team(player, &team, skirmish);
            }
            // CPP explicitly restarts the iterator after each removal.
        }
        let mut cursor = self
            .ai_manager
            .ai_players
            .get(&player)
            .and_then(|ai| ai.team_queue.front().map(|node| node.visit_token));
        while let Some(token) = cursor {
            let Some((index, complete)) = self.ai_manager.ai_players.get(&player).and_then(|ai| {
                ai.team_queue
                    .iter()
                    .position(|n| n.visit_token == token)
                    .map(|index| {
                        (
                            index,
                            ai.team_queue[index].is_all_built() || ai.team_queue[index].completed,
                        )
                    })
            }) else {
                break;
            };
            if complete {
                let ai = self.ai_manager.ai_players.get_mut(&player).unwrap();
                let team = ai.team_queue.remove(index).unwrap();
                ai.team_ready_queue.push_front(team);
                cursor = ai.team_queue.front().map(|node| node.visit_token);
                continue;
            }
            let successors: Vec<_> = self.ai_manager.ai_players[&player]
                .team_queue
                .iter()
                .skip(index + 1)
                .map(|node| node.visit_token)
                .collect();
            let prepared = {
                let ai = &self.ai_manager.ai_players[&player];
                let team = &ai.team_queue[index];
                let members: Vec<_> = ai
                    .leftover_instance_member_ids(team.team_id)
                    .into_iter()
                    .map(ObjectId)
                    .collect();
                let idle = members.iter().any(|id| {
                    self.host_object(*id)
                        .is_some_and(|o| o.has_ai_update_interface() && (o.ai_state() == AIState::Idle
                            || (matches!(o.ai_state(), AIState::GuardingArea | AIState::GuardingObject)
                                && o.unit_ai_runtime.guard_phase() == Some(crate::game_logic::object::unit_ai_runtime::GuardPhase::Idle))))
                });
                (idle && ai.prototype_execute_actions_on_create(&team.name))
                    .then(|| (ai.prototype_production_condition(&team.name), team.team_id))
            };
            if let Some((condition, team)) = prepared {
                self.with_owned_script_engine(|engine, world| {
                    world.execute_owned_ai_condition_with_effects(engine, &condition, team, ui)
                });
            }
            cursor = self.ai_manager.ai_players.get(&player).and_then(|ai| {
                match ai
                    .team_queue
                    .iter()
                    .position(|node| node.visit_token == token)
                {
                    Some(index) => ai.team_queue.get(index + 1).map(|node| node.visit_token),
                    None => successors
                        .iter()
                        .copied()
                        .find(|next| ai.team_queue.iter().any(|node| node.visit_token == *next)),
                }
            });
        }
    }

    fn disband_owned_ai_queued_team(&mut self, player: u32, team: &AITeamQueue, skirmish: bool) {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return;
        };
        let default = self.default_host_team_instance_name(Some(player), ai.team);
        // CPP transferUnitsTo walks the live linked roster, in its current
        // order. Work-order observations are not team membership.
        let members = ai.leftover_instance_member_ids(team.team_id);
        let singleton = ai.leftover_prototype_is_singleton(&team.name);
        let factory = ai.team_factory.clone();
        if let Some(source) = team.team_id {
            let destination = factory
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .find_team_instances(&default)
                .into_iter()
                .next();
            if destination
                .as_ref()
                .and_then(|t| t.read().ok().map(|t| t.get_id()))
                == Some(source)
            {
                return;
            }
            let Some(destination) = destination else {
                return;
            };
            let destination_id = destination.read().ok().map(|t| t.get_id());
            let source_team = factory
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .find_team_by_id(source);
            let Some(source_team) = source_team else {
                return;
            };
            for id in members {
                let Some(object) = self.host_object_mut(ObjectId(id)) else {
                    continue;
                };
                object.team_instance_name = default.clone();
                // C++ Team::transferUnitsTo -> Object::setTeam retargets m_team.
                object.team_instance_id = destination_id;
                object.apply_named_team_ai_profile(&factory, true);

                source_team
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove_member(id);
                destination
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .add_member(id);
            }
            destination
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .set_active();
            if !singleton {
                self.delete_canonical_ai_team(source);
            }
        }
        if skirmish {
            if let Some(engine) = &self.owned_script_engine {
                engine.clear_team_flags();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_factory_campaign_map(path: &std::path::Path) {
        use game_engine::common::{
            dict::Dict, name_key_generator::NameKeyGenerator, system::DataChunkOutput,
            well_known_keys::*,
        };

        let mut map = DataChunkOutput::new();
        map.open_data_chunk("HeightMapData", 3);
        for value in [51, 51, 0, 2601] {
            map.write_int(value);
        }
        for _ in 0..2601 {
            map.write_byte(0);
        }
        map.close_data_chunk();
        map.open_data_chunk("SidesList", 2);
        map.write_int(3);
        for (name, human) in [("Human", true), ("Commander", false), ("Captor", true)] {
            let mut player = Dict::new();
            player.set_ascii_string(key_player_name(), name);
            player.set_unicode_string(key_player_display_name(), name);
            player.set_ascii_string(key_player_faction(), "FactionAmerica");
            player.set_bool(key_player_is_human(), human);
            map.write_dict(&player);
            map.write_int(0); // no authored BuildList before preplaced admission
        }
        map.write_int(0); // normal map admission supplies the default Teams
        map.close_data_chunk();
        map.open_data_chunk("ObjectsList", 3);
        for x in [90.0, 210.0, 330.0] {
            map.open_data_chunk("Object", 3);
            for component in [x, 120.0, 0.0, 0.0] {
                map.write_real(component);
            }
            map.write_int(0);
            map.write_ascii_string("AmericaBarracks");
            let mut object = Dict::new();
            object.set_ascii_string(
                NameKeyGenerator::name_to_key("originalOwner"),
                "teamCommander",
            );
            map.write_dict(&object);
            map.close_data_chunk();
        }
        map.close_data_chunk();
        std::fs::write(path, map.into_ckmp_bytes()).unwrap();
    }

    #[test]
    fn mounted_campaign_factory_lookup_uses_preplaced_rows_and_clears_only_visited_captures() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "mounted_campaign_factory_lookup_uses_preplaced_rows_and_clears_only_visited_captures",
            || {
                const RANGER: &str = "AmericaInfantryRanger";
                let startup = crate::parallel_harness::RetailWorldDefinitions::load().unwrap();
                let directory = tempfile::tempdir().unwrap();
                let map = directory.path().join("FactoryOrderCampaign.map");
                let mut world = GameLogic::new();
                world.with_own_services_mut(|world| {
                    startup.admit(world).unwrap();
                    write_factory_campaign_map(&map);
                });
                world
                    .start_new_game_in_campaign_session(GameMode::SinglePlayer, true, false)
                    .unwrap();
                // The current Main campaign bootstrap does not install the
                // base AI controller. Supply the receiving controller here;
                // the real map loader must build its factory rows.
                world
                    .ai_manager
                    .add_ai_player(2, Team::USA, AIDifficulty::Medium);
                assert!(world.load_map(map.to_str().unwrap()));
                assert_eq!(
                    world.get_player(2).unwrap().script_identity(),
                    Some("Commander")
                );
                assert!(world.get_player(2).unwrap().map_side.build_list.is_empty());
                let mut ai = world.ai_manager.ai_players.remove(&2).unwrap();
                let factories: Vec<_> = world
                    .host_objects()
                    .registered_values()
                    .rev()
                    .filter_map(|object| {
                        (object.owner_player_id == Some(2)
                            && object.template_name == "AmericaBarracks")
                            .then_some(object.id)
                    })
                    .collect();
                assert_eq!(factories.len(), 3);
                assert_eq!(
                    ai.building_queue
                        .iter()
                        .map(|row| row.object_id.unwrap())
                        .collect::<Vec<_>>(),
                    factories,
                    "ordinary newMap prepends the preplaced factories in birth order"
                );
                assert!(ai.building_queue.iter().all(|row| row.max_rebuilds == 0));
                let [first, middle, last]: [ObjectId; 3] = factories.try_into().unwrap();
                let owner = world.get_player_mut(2).unwrap();
                owner.resources.supplies = 50_000;
                owner.set_can_build_units(true);
                assert_eq!(
                    ai.find_factory_for_unit_ex(&world, RANGER, false),
                    Some(first)
                );

                for factory in [first, middle, last] {
                    assert!(world.enqueue_production(factory, RANGER.into()));
                }
                let unlisted = world
                    .create_object_for_player("AmericaBarracks", 2, Vec3::new(440.0, 0.0, 120.0))
                    .unwrap();
                assert_eq!(ai.find_factory_for_unit_ex(&world, RANGER, false), None);
                assert_eq!(
                    ai.find_factory_for_unit_ex(&world, RANGER, true),
                    Some(last)
                );
                assert!(AIPlayer::factory_is_idle(
                    world.host_object(unlisted).unwrap()
                ));

                assert!(world.transfer_object_to_player(first, 3).unwrap());
                assert!(world.transfer_object_to_player(last, 3).unwrap());
                world
                    .host_object_mut(middle)
                    .unwrap()
                    .building_data
                    .as_mut()
                    .unwrap()
                    .production_queue
                    .clear();
                let order = AIWorkOrder::new(RANGER.into(), 1, 100);
                ai.team_queue.push_front(AITeamQueue::new(
                    "Admitted factory".into(),
                    vec![order],
                    true,
                    0,
                ));
                ai.process_team_queue(&mut world, 0.0);
                assert_eq!(ai.team_queue[0].work_orders[0].factory_id, Some(middle));
                assert_eq!(ai.building_queue[0].object_id, None);
                assert_eq!(ai.building_queue[0].destroyed_at_time, None);
                assert_eq!(
                    ai.building_queue[2].object_id,
                    Some(last),
                    "first idle ends the walk"
                );
                assert!(AIPlayer::factory_is_idle(
                    world.host_object(unlisted).unwrap()
                ));

                let second = AIWorkOrder::new(RANGER.into(), 1, 100);
                ai.team_queue.push_front(AITeamQueue::new(
                    "Idle producer required".into(),
                    vec![second],
                    false,
                    0,
                ));
                ai.process_team_queue(&mut world, 0.1);
                assert_eq!(ai.team_queue[0].work_orders[0].factory_id, None);
                assert_eq!(ai.building_queue[2].object_id, None);
                assert_eq!(ai.building_queue[2].destroyed_at_time, None);
            },
        );
    }


    use crate::game_logic::{Player, ThingTemplate};
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };

    fn build(name: &str) -> ScriptAction {
        let mut action = ScriptAction::new(ScriptActionType::BuildTeam);
        action
            .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
            .unwrap();
        action
    }
    fn prototype(world: &mut GameLogic, name: &str, player: &str, condition: &str) {
        let controller = world
            .get_players()
            .iter()
            .find_map(|(&id, owner)| (owner.name == player).then_some(id));
        let mut factory = world.team_factory.lock().unwrap();
        factory.init_team_owned(name.into(), player.into(), false, None, controller);
        let mut proto = (*factory.find_team_prototype(name).unwrap()).clone();
        // These callback fixtures author the receiving prototype directly;
        // name-key dictionary parsing is a separate ownership boundary.
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 2,
                max_units: 2,
                unit_thing_name: "AmericaInfantryRanger".into(),
            },
        );
        proto.set_production_condition(condition.into());
        proto.set_execute_actions_on_create(!condition.is_empty());
        proto.set_initial_idle_frames(30_000);
        factory.replace_team_prototype(proto);
        let admitted = factory.find_team_prototype(name).unwrap();
        let units = admitted.units_info();
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].unit_thing_name, "AmericaInfantryRanger");
        assert_eq!(units[0].min_units, 2);
        assert_eq!(units[0].max_units, 2);
    }

    fn callback_world() -> GameLogic {
        callback_world_with(&[])
    }

    /// `callback_world` whose Object content is authored and admitted before
    /// any Object exists (GameEngine.cpp:428-458). Once a world has admitted
    /// content its ThingFactory serves only declared templates, so the
    /// roster's Ranger and Barracks are declared alongside `content`.
    fn callback_world_with(content: &[crate::fixture_content::FixtureObject]) -> GameLogic {
        use crate::fixture_content::{FixtureObject, admit_fixture_objects};
        let mut world = GameLogic::new();
        if !content.is_empty() {
            let mut declared = vec![
                FixtureObject::new("AmericaInfantryRanger").kind_of("INFANTRY"),
                FixtureObject::new("AmericaBarracks").kind_of("STRUCTURE"),
            ];
            declared.extend(content.iter().cloned());
            admit_fixture_objects(&mut world, &declared);
        }
        for (id, name) in [(1, "CurrentOwner"), (2, "PeerOwner")] {
            let mut player = Player::new(id, Team::USA, name, false);
            player.resources.supplies = 10_000;
            world.add_player(player);
            world.add_ai_opponent(id, Team::USA, AIDifficulty::Medium);
            world.get_player_mut(id).unwrap().set_can_build_units(true);
            let ai = world.ai_manager.ai_players.get_mut(&id).unwrap();
            ai.building_queue.clear();
            ai.next_building_time = 100_000.0;
            ai.next_team_time = 100_000.0;
            ai.next_team_queue_time = 100_000.0;
        }
        world.ai_manager.ai_players.get_mut(&2).unwrap().is_active = false;
        let mut infantry = ThingTemplate::new("AmericaInfantryRanger");
        infantry.add_kind_of(KindOf::Infantry);
        world.templates.insert(infantry.name.clone(), infantry);
        let mut factory = ThingTemplate::new("AmericaBarracks");
        factory.add_kind_of(KindOf::Structure);
        world.templates.insert(factory.name.clone(), factory);
        for id in [1, 2] {
            world
                .create_object_for_player("AmericaBarracks", id, Vec3::ZERO)
                .unwrap();
        }
        let unit = world
            .create_object_for_player("AmericaInfantryRanger", 1, Vec3::ZERO)
            .unwrap();
        world.host_object_mut(unit).unwrap().is_recruitable = false;
        prototype(&mut world, "Parent", "CurrentOwner", "BuildChild");
        prototype(&mut world, "Child", "CurrentOwner", "BuildPeer");
        prototype(&mut world, "PeerChild", "PeerOwner", "NestedReceipt");
        let parent_id = world
            .team_factory
            .lock()
            .unwrap()
            .create_inactive_team_with_owned_effects("Parent")
            .unwrap()
            .0
            .read()
            .unwrap()
            .get_id();
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(parent_id)
            .unwrap()
            .write()
            .unwrap()
            .add_member(unit.0);
        world.host_object_mut(unit).unwrap().team_instance_name = "Parent".into();
        let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
        order.num_completed = 1;
        order.observed_unit_ids.push(unit);
        let mut parent = AITeamQueue::new("Parent".into(), vec![order], false, 0);
        parent.team_id = Some(parent_id);
        parent.execute_actions = true;
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_queue
            .push_back(parent);
        let mut list = ScriptList::new();
        for (name, action) in [
            ("BuildChild", build("Child")),
            ("BuildPeer", build("PeerChild")),
        ] {
            let mut script = Script::new();
            script.script_name = name.into();
            script.is_subroutine = true;
            let mut condition = gamelogic::scripting::OrCondition::new();
            condition.set_first_and_condition(Some(Box::new(
                gamelogic::scripting::Condition::new(
                    gamelogic::scripting::ConditionType::ConditionTrue,
                ),
            )));
            script.condition = Some(Box::new(condition));
            script.action = Some(Box::new(action));
            list.append_script(Box::new(script));
        }
        let mut receipt = Script::new();
        receipt.script_name = "NestedReceipt".into();
        receipt.is_subroutine = true;
        let mut display = ScriptAction::new(ScriptActionType::DisplayText);
        display
            .add_parameter(Parameter::with_string(
                ParameterType::TextString,
                "nested:current-and-peer".into(),
            ))
            .unwrap();
        receipt.action = Some(Box::new(display));
        list.append_script(Box::new(receipt));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        world.install_owned_script_engine(engine);
        world
    }

    #[test]
    fn actual_ai_pass_accepts_nested_current_and_peer_team_builds_immediately() {
        let mut world = callback_world();
        AIManager::update_owned(&mut world, 0.0);
        assert!(
            world.ai_manager.ai_players[&1]
                .team_queue
                .iter()
                .any(|n| n.name == "Child")
        );
        assert!(
            world.ai_manager.ai_players[&2]
                .team_queue
                .iter()
                .any(|n| n.name == "PeerChild")
        );
        assert_eq!(world.ai_manager.ai_players[&2].next_team_queue_time, 0.0);
        assert_eq!(
            world.peek_new_script_messages(),
            vec!["Transmission: nested:current-and-peer"; 4]
        );
        assert!(world.owned_script_engine.is_some());
        assert_eq!(world.ai_manager.ai_players.len(), 2);
    }

    #[test]
    fn failed_reentrant_create_callback_keeps_receipt_and_queue_retry_does_not_recreate() {
        let mut world = callback_world();
        world.admit_challenge_session(true);

        // Child's null-Team callback reenters AI team creation, then fails on
        // the challenge-only ThePlayer alias because this fixture has no local
        // player. The first action's nested queue admission must remain visible.
        let mut nested = build("PeerChild");
        let mut failing = ScriptAction::new(ScriptActionType::MoveTeamTo);
        failing
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                gamelogic::scripting::core::TEAM_THE_PLAYER.into(),
            ))
            .unwrap();
        failing
            .add_parameter(Parameter::with_string(
                ParameterType::Waypoint,
                "UnusedWaypoint".into(),
            ))
            .unwrap();
        nested.set_next_action(Some(Box::new(failing)));

        let mut create = Script::new();
        // Child's creation hook is named BuildPeer in callback_world().
        create.script_name = "BuildPeer".into();
        create.is_subroutine = true;
        create.action = Some(Box::new(nested));
        let mut count = ScriptAction::new(ScriptActionType::IncrementCounter);
        count
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        count
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "PeerCreateVisits".into(),
            ))
            .unwrap();
        let mut peer_create = Script::new();
        peer_create.script_name = "NestedReceipt".into();
        peer_create.is_subroutine = true;
        peer_create.action = Some(Box::new(count));
        let mut scripts = ScriptList::new();
        scripts.append_script(Box::new(create));
        scripts.append_script(Box::new(peer_create));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine.set_counter("PeerCreateVisits", 0).unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(scripts)))
            .unwrap();
        world.install_owned_script_engine(engine);

        assert!(!world.build_owned_specific_ai_team_with_effects(
            1,
            "Child",
            false,
            None,
            &mut Default::default(),
        ));

        let (child_id, child_identity) = {
            let factory = world.team_factory.lock().unwrap();
            let child = factory.find_team_instances("Child").pop().unwrap();
            let child_id = child.read().unwrap().get_id();
            (child_id, child)
        };
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(
            ai.team_queue
                .iter()
                .find(|node| node.name == "Child")
                .unwrap()
                .team_id,
            Some(child_id)
        );
        assert_eq!(
            world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("PeerCreateVisits")
                .unwrap()
                .value,
            2
        );
        assert!(world.team_creation_error().is_some());

        // The production queue processor is the retry boundary. It must keep
        // the completed callback receipt and must not enter the legacy binder.
        world.ai_manager.ai_players.get_mut(&1).unwrap().next_team_queue_time = 0.0;
        let data = AiDataView::from_world(&world);
        world
            .update_owned_ai_military_with_effects(1, 0.0, &data, &mut Default::default())
            .unwrap();
        assert_eq!(
            world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("PeerCreateVisits")
                .unwrap()
                .value,
            2
        );
        let factory = world.team_factory.lock().unwrap();
        let child_now = factory.find_team_by_id(child_id).unwrap();
        assert!(std::sync::Arc::ptr_eq(&child_identity, &child_now));
        assert_eq!(factory.find_team_instances("Child").len(), 1);
        assert_eq!(factory.find_team_instances("PeerChild").len(), 1);
        assert_eq!(
            world.ai_manager.ai_players[&1]
                .team_queue
                .iter()
                .filter(|node| node.name == "Child")
                .count(),
            1
        );
    }

    #[test]
    fn automatic_military_selection_runs_owned_condition_and_nested_build_immediately() {
        let mut world = callback_world();
        // Parent starts training before automatic team selection. C++
        // isPossibleToBuildTeam still needs an idle factory of this player.
        world
            .create_object_for_player("AmericaBarracks", 1, Vec3::new(80.0, 0.0, 0.0))
            .unwrap();
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.next_team_queue_time = 0.0;
        ai.next_team_time = 0.0;
        let data = AiDataView::from_world(&world);
        world.update_owned_ai_military(1, 0.0, &data);
        assert!(
            world.ai_manager.ai_players[&1]
                .team_queue
                .iter()
                .any(|q| q.name == "Child")
        );
        assert!(
            world.ai_manager.ai_players[&2]
                .team_queue
                .iter()
                .any(|q| q.name == "PeerChild")
        );
        assert_eq!(
            world.peek_new_script_messages(),
            vec!["Transmission: nested:current-and-peer"; 4]
        );
        assert!(world.ai_manager.ai_players[&1].next_team_time > 0.0);
        assert!(
            world.ai_manager.ai_players[&1].production_condition_states["Child"]
                .script
                .is_some()
        );
        assert!(world.owned_script_engine.is_some());
    }

    #[test]
    fn ordinary_team_cadence_pauses_and_selects_with_the_signed_clock() {
        let mut world = callback_world();
        world
            .create_object_for_player("AmericaBarracks", 1, Vec3::new(80.0, 0.0, 0.0))
            .unwrap();
        // Parent holds an existing work order. These two pending receipts put
        // the ordinary controller at the same count as the Skirmish guard.
        let initial = OrdinaryAiTiming {
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 2,
            structure_timer: 2,
            build_delay: 31,
            team_delay: 0,
            team_seconds: 17,
            frame_last_building_built: 0,
            bridge_timer: 0,
        };
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.ordinary_timing = Some(initial);
        ai.set_team_delay_seconds(-3);
        let initial = ai.ordinary_timing.unwrap();
        for name in ["PendingOne", "PendingTwo"] {
            ai.team_queue.push_back(AITeamQueue::new(
                name.into(),
                vec![AIWorkOrder::new("MissingCampaignUnit".into(), 1, 100)],
                false,
                0,
            ));
        }
        world
            .ai_manager
            .ai_players
            .get_mut(&2)
            .unwrap()
            .ordinary_timing = Some(OrdinaryAiTiming {
            team_delay: 77,
            ..initial
        });
        let mut definitions = world.ai_definitions.data().clone();
        definitions.resources_poor = 0;
        definitions.resources_wealthy = 7_000;
        definitions.team_wealthy_mod = 2.0;
        world.set_ai_definition_base(definitions);
        world.frame = (1 << 24) + 1;
        let data = AiDataView::from_world(&world);

        world.get_player_mut(1).unwrap().set_can_build_units(false);
        tick_ordinary_team(&mut world, &data);
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(ai.ordinary_timing, Some(initial));
        assert_eq!(ai.team_queue.len(), 3);
        assert_eq!(ai.team_queue[0].work_orders[0].factory_id, None);

        world.get_player_mut(1).unwrap().set_can_build_units(true);
        tick_ordinary_team(&mut world, &data);
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(
            ai.ordinary_timing,
            Some(OrdinaryAiTiming {
                team_timer: 1,
                team_delay: 150,
                ..initial
            })
        );
        assert_eq!(ai.team_queue.len(), 3);
        assert!(ai.team_queue[0].work_orders[0].factory_id.is_some());

        tick_ordinary_team(&mut world, &data);
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(
            ai.ordinary_timing,
            Some(OrdinaryAiTiming {
                team_timer: -45,
                team_delay: 150,
                ..initial
            })
        );
        let child = ai
            .team_queue
            .iter()
            .find(|node| node.name == "Child")
            .unwrap();
        assert_eq!(child.frame_started, world.frame);
        assert!(child.work_orders[0].factory_id.is_some());
        assert_eq!(ai.next_team_time, 100_000.0);
        assert_eq!(ai.next_team_queue_time, 100_000.0);
        let peer = &world.ai_manager.ai_players[&2];
        assert!(peer.team_queue.iter().any(|node| node.name == "PeerChild"));
        assert_eq!(peer.ordinary_timing.unwrap().team_delay, 0);
        assert_eq!(peer.ordinary_timing.unwrap().team_timer, 2);
        assert_eq!(peer.next_team_queue_time, 100_000.0);
    }

    fn tick_ordinary_team(world: &mut GameLogic, data: &AiDataView) {
        world.frame += 1;
        world
            .update_owned_ordinary_ai_team_building_with_effects(
                1,
                world.frame as f32 / LOGIC_FRAMES_PER_SECOND,
                data,
                &mut Default::default(),
            )
            .unwrap();
    }

    #[test]
    fn ordinary_work_orders_recruit_again_after_an_active_singleton_trains() {
        const RANGER: &str = "AmericaInfantryRanger";
        const LOW: &str = "CampaignSingleton";
        const HIGH: &str = "CampaignUrgent";
        let mut world = callback_world();
        world
            .create_object_for_player("AmericaBarracks", 1, Vec3::new(80., 0., 0.))
            .unwrap();
        let template = world.templates.get_mut(RANGER).unwrap();
        template.set_cost(225, 0);
        template.buildable_status =
            crate::game_logic::host_production_buildable_command_residual::BSTATUS_ONLY_BY_AI;
        template.set_authored_ai_update_interface(Some(true));
        let recruit = world
            .create_object_for_player(RANGER, 1, Vec3::new(25., 0., 30.))
            .unwrap();
        let unit = world.host_object_mut(recruit).unwrap();
        unit.is_recruitable = true;
        unit.last_command_source = crate::game_logic::HUNT_CMD_FROM_SCRIPT;

        prototype(&mut world, LOW, "CurrentOwner", "");
        prototype(&mut world, HIGH, "CurrentOwner", "");
        let low_id = {
            let mut factory = world.team_factory.lock().unwrap();
            for (name, priority, count) in [(LOW, 10, 2), (HIGH, 20, 1)] {
                let mut prototype = (*factory.find_team_prototype(name).unwrap()).clone();
                assert!(!prototype.has_home_location());
                prototype.set_singleton(name == LOW);
                prototype.set_ai_recruitable(name == LOW);
                prototype.set_production_priority(priority);
                prototype.set_units_info(
                    0,
                    gamelogic::team::CreateUnitsInfo {
                        min_units: count,
                        max_units: count,
                        unit_thing_name: RANGER.into(),
                    },
                );
                factory.replace_team_prototype(prototype);
            }
            let low = factory
                .create_inactive_team_with_owned_effects(LOW)
                .unwrap()
                .0;
            low.write().unwrap().set_active();
            let id = low.read().unwrap().get_id();
            id
        };

        let base = Vec3::new(113., 0., 62.);
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.team_queue.clear();
        ai.base_center = base;
        ai.base_center_set = true;
        ai.ordinary_timing = Some(OrdinaryAiTiming {
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 500,
            structure_timer: 500,
            build_delay: 500,
            team_delay: 0,
            team_seconds: 20,
            frame_last_building_built: 0,
            bridge_timer: 0,
        });
        assert!(world.build_owned_specific_ai_team(1, HIGH, false, None));
        assert!(world.build_owned_specific_ai_team(1, LOW, true, None));
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(ai.team_queue[0].team_id, Some(low_id));
        let high_id = ai.team_queue[1].team_id.unwrap();
        let cash = world.host_authoritative_cash(1).unwrap();

        let mut definitions = world.ai_definitions.data().clone();
        definitions.max_recruit_distance = 200.;
        world.set_ai_definition_base(definitions);
        let data = AiDataView::from_world(&world);
        tick_ordinary_team(&mut world, &data);

        let ai = &world.ai_manager.ai_players[&1];
        let low = &ai.team_queue[0].work_orders[0];
        let high = &ai.team_queue[1].work_orders[0];
        assert_eq!(low.num_completed, 1);
        let producing = low
            .factory_id
            .expect("the first order starts its one remaining unit");
        assert_eq!(
            world
                .host_object(producing)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue
                .len(),
            1
        );
        assert_eq!(high.num_completed, 1);
        assert_eq!(high.factory_id, None);
        assert_eq!(world.host_authoritative_cash(1), Some(cash - 225));
        assert!(ai.leftover_instance_member_ids(Some(low_id)).is_empty());
        assert_eq!(ai.leftover_instance_member_ids(Some(high_id)), [recruit.0]);
        let unit = world.host_object(recruit).unwrap();
        assert_eq!(unit.team_instance_id, Some(high_id));
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::HUNT_CMD_FROM_AI
        );
        assert_eq!(unit.capture_base_ai_continuation().goals.position, base);
    }

    #[test]
    fn ordinary_work_order_validation_yields_the_idle_factory_to_waiting_orders() {
        const RANGER: &str = "AmericaInfantryRanger";
        const MISSILE_DEFENDER: &str = "AmericaInfantryMissileDefender";

        let mut world = callback_world();
        let captured = world
            .host_objects()
            .registered_values()
            .find(|object| {
                object.template_name == "AmericaBarracks" && object.owner_player_id == Some(1)
            })
            .unwrap()
            .id;
        let removed = world
            .create_object_for_player("AmericaBarracks", 1, Vec3::new(80.0, 0.0, 0.0))
            .unwrap();
        let replacement = world
            .create_object_for_player("AmericaBarracks", 1, Vec3::new(160.0, 0.0, 0.0))
            .unwrap();
        world.templates.get_mut(RANGER).unwrap().set_cost(225, 0);
        let mut missile_defender = ThingTemplate::new(MISSILE_DEFENDER);
        missile_defender
            .add_kind_of(KindOf::Infantry)
            .set_cost(300, 0);
        world
            .templates
            .insert(MISSILE_DEFENDER.into(), missile_defender);

        assert!(world.enqueue_production(captured, RANGER.into()));
        assert!(world.enqueue_production(removed, RANGER.into()));
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.ordinary_timing = Some(OrdinaryAiTiming {
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 500,
            structure_timer: 500,
            build_delay: 500,
            team_delay: 0,
            team_seconds: 20,
            frame_last_building_built: 0,
            bridge_timer: 0,
        });
        let [first, second] = [captured, removed].map(|factory| {
            let mut order = AIWorkOrder::new(RANGER.into(), 1, 100);
            order.factory_id = Some(factory);
            order.queued_count = 1;
            order
        });
        let waiting = AIWorkOrder::new(MISSILE_DEFENDER.into(), 1, 100);
        ai.team_queue[0].work_orders = vec![first, second, waiting];

        assert!(world.transfer_object_to_player(captured, 2).unwrap());
        world.destroy_object(removed).unwrap();
        world.process_destroy_list();
        let captured_object = world.host_object(captured).unwrap();
        assert_eq!(captured_object.owner_player_id, Some(2));
        assert_eq!(captured_object.team, Team::USA);
        assert!(world.host_object(removed).is_none());
        let cash = world.host_authoritative_cash(1).unwrap();
        let peer_cash = world.host_authoritative_cash(2).unwrap();

        let data = AiDataView::from_world(&world);
        tick_ordinary_team(&mut world, &data);

        let orders = &world.ai_manager.ai_players[&1].team_queue[0].work_orders;
        assert_eq!(
            orders
                .iter()
                .map(|o| (o.factory_id, o.queued_count))
                .collect::<Vec<_>>(),
            [(None, 0), (None, 0), (Some(replacement), 1)]
        );
        let queue = &world
            .host_object(replacement)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue;
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].template_name, MISSILE_DEFENDER);
        assert_eq!(world.host_authoritative_cash(1), Some(cash - 300));
        assert_eq!(world.host_authoritative_cash(2), Some(peer_cash));
    }

    #[test]
    fn owned_condition_cache_preserves_delay_missing_and_difficulty_isolation() {
        let mut world = callback_world();
        assert!(world.evaluate_owned_team_condition(1, "Child"));
        let state = world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .production_condition_states
            .get_mut("Child")
            .unwrap();
        state.script.as_mut().unwrap().delay_evaluation_seconds = 2;
        world.frame = 10;
        assert!(world.evaluate_owned_team_condition(1, "Child"));
        assert_eq!(
            world.ai_manager.ai_players[&1].production_condition_states["Child"]
                .script
                .as_ref()
                .unwrap()
                .frame_to_evaluate_at,
            70
        );
        let mut bytes = Vec::new();
        crate::save_load::snapshot::ai_player_queue_persist::append_to_lifecycle_tail(
            &mut bytes, &world,
        );
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .production_condition_states
            .clear();
        crate::save_load::snapshot::ai_player_queue_persist::apply_from_lifecycle_tail(
            &bytes, &mut world,
        )
        .unwrap();
        world.frame = 69;
        assert!(!world.evaluate_owned_team_condition(1, "Child"));
        world.frame = 70;
        assert!(world.evaluate_owned_team_condition(1, "Child"));
        assert!(!world.evaluate_owned_team_condition(1, "Absent"));
        assert!(world.ai_manager.ai_players[&1].production_condition_states["Absent"].always_false);
        assert!(
            world.ai_manager.ai_players[&2]
                .production_condition_states
                .is_empty()
        );
        let mut rejected = callback_world();
        let mut script = rejected
            .owned_script_engine
            .as_ref()
            .unwrap()
            .find_script_clone_by_name("BuildPeer")
            .unwrap();
        script.normal = false;
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        rejected.install_owned_script_engine(engine);
        rejected
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .difficulty = AIDifficulty::Medium;
        assert!(!rejected.evaluate_owned_team_condition(1, "Child"));
        assert!(
            rejected.ai_manager.ai_players[&1].production_condition_states["Child"].always_false
        );
    }

    #[test]
    fn startup_factory_collector_callback_observes_its_canonical_ai_owner() {
        // Retail AmericaSupplyCenter authors its free Chinook through
        // SpawnBehavior plus SupplyCenterProductionExitUpdate; all of it is
        // admitted before NewGame (GameEngine.cpp:428-458).
        use crate::fixture_content::FixtureObject;
        let mut world = callback_world_with(&[
            FixtureObject::new("AmericaCommandCenter")
                .kind_of("STRUCTURE COMMANDCENTER"),
            FixtureObject::new("AmericaSupplyCenter")
                .kind_of("STRUCTURE FS_SUPPLY_CENTER")
                .module("Behavior = SpawnBehavior SpawnTag\n OneShot = Yes\n SpawnNumber = 1\n SpawnTemplateName = AmericaVehicleChinook\nEnd")
                .module("Behavior = SupplyCenterProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:30 Y:0 Z:0\nEnd"),
            FixtureObject::new("AmericaVehicleChinook").kind_of("HARVESTER VEHICLE"),
        ]);
        let mut definitions = game_engine::common::ini::AIData::default();
        definitions.side_build_lists.clear();
        let mut list = game_engine::common::ini::AiSideBuildList::new("America".into());
        for (name, template, built, position) in [
            ("CC", "AmericaCommandCenter", true, (0.0, 0.0)),
            ("Supply", "AmericaSupplyCenter", true, (50.0, 0.0)),
        ] {
            list.entries.push(game_engine::common::ini::BuildListEntry {
                building_name: name.into(),
                template_name: template.into(),
                location: position,
                rebuilds: 0,
                angle_radians: 0.0,
                initially_built: built,
                rally_point_offset: (0.0, 0.0),
                automatically_build: true,
            });
        }
        definitions.side_build_lists.push(list);
        world.set_ai_definition_base(definitions);
        let original_cc = world
            .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(10., 0., 20.))
            .unwrap();
        let unrelated = world
            .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(90., 0., 0.))
            .unwrap();
        let mut peer = GameLogic::new();
        crate::fixture_content::admit_fixture_objects(
            &mut peer,
            &[FixtureObject::new("AmericaCommandCenter").kind_of("STRUCTURE COMMANDCENTER")],
        );
        peer.add_player(Player::new(1, Team::USA, "SameIdPeer", false));
        peer.set_next_object_id_for_restore(original_cc);
        let peer_cc = peer
            .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(110., 0., 120.))
            .unwrap();
        assert_eq!(original_cc, peer_cc);
        let original_health = world.objects[&original_cc].health.current;
        world.destroy_object(unrelated);
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.skirmish_new_map_applied = false;
        ai.next_team_queue_time = 999.0;
        let peer_timer = world.ai_manager.ai_players[&2].next_team_queue_time;
        let data = AiDataView::from_world(&world);
        world.initialize_owned_skirmish_ai(1, &data);
        // SpawnBehavior creates the free collector on its first update
        // (SpawnBehavior.cpp:186-247), not inside newMap.
        world.frame += 1;
        world
            .update_spawn_behavior_with_effects(&mut Default::default())
            .unwrap();
        assert_eq!(world.ai_manager.ai_players[&1].next_team_queue_time, 0.0);
        assert_eq!(
            world.ai_manager.ai_players[&2].next_team_queue_time,
            peer_timer
        );
        let pad = world.ai_manager.ai_players[&1]
            .building_queue
            .iter()
            .find(|p| p.template_name == "AmericaSupplyCenter")
            .unwrap();
        assert!(world.host_objects().registered_values().any(|o| {
            o.template_name == "AmericaVehicleChinook"
                && o.producer_id == pad.object_id
                && o.owner_player_id == Some(1)
        }));
        assert_eq!(world.ai_manager.ai_players.len(), 2);
        for id in [original_cc, unrelated] {
            assert!(
                world.objects.contains_key(&id),
                "startup callbacks cannot flush queued physical deletion"
            );
            assert!(world.objects.ordered_ids().contains(&id));
            assert!(world.objects[&id].status.destroyed);
        }
        assert_eq!(world.objects[&original_cc].health.current, original_health);
        assert!(world.has_pending_object_removals());
        assert!(!peer.objects[&peer_cc].status.destroyed);
        world
            .process_destroy_list_with_effects_checked(&mut Default::default())
            .unwrap();
        assert!(!world.objects.contains_key(&original_cc));
        assert!(!world.objects.contains_key(&unrelated));
        assert!(peer.objects.contains_key(&peer_cc));
        assert!(!peer.objects[&peer_cc].status.destroyed);
    }

    #[test]
    fn factory_null_team_then_admitted_team_callbacks_do_not_inherit_caller_team() {
        let mut world = callback_world();
        let parent = world.ai_manager.ai_players[&1].team_queue.front().unwrap();
        let parent_id = parent.team_id.unwrap();
        let unit = parent.work_orders[0].observed_unit_ids[0];
        let frame = world.frame;
        world.host_object_mut(unit).unwrap().set_ai_state(
            AIState::Moving,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        prototype(&mut world, "Leaf", "CurrentOwner", "LeafReceipt");
        let mut display = ScriptAction::new(ScriptActionType::DisplayText);
        display
            .add_parameter(Parameter::with_string(
                ParameterType::TextString,
                "leaf:phase".into(),
            ))
            .unwrap();
        let mut stop = ScriptAction::new(ScriptActionType::TeamStop);
        stop.add_parameter(Parameter::with_string(
            ParameterType::Team,
            gamelogic::scripting::core::THIS_TEAM.into(),
        ))
        .unwrap();
        display.next_action = Some(Box::new(stop));
        let mut script = Script::new();
        script.script_name = "LeafReceipt".into();
        script.action = Some(Box::new(display));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        world.install_owned_script_engine(engine);
        world.with_owned_script_engine(|engine, world| {
            world.execute_script_team_actions(engine, &build("Leaf"), Some(parent_id));
        });
        // CPP createInactiveTeam first passes null, then buildSpecificAITeam
        // passes the admitted Leaf. Neither callback can stop the caller Parent.
        assert_eq!(
            world.peek_new_script_messages(),
            vec!["Transmission: leaf:phase"; 2]
        );
        assert_eq!(world.host_object(unit).unwrap().ai_state(), AIState::Moving);
        let leaf = world.ai_manager.ai_players[&1]
            .team_queue
            .iter()
            .find(|q| q.name == "Leaf")
            .unwrap();
        assert!(leaf.team_id.is_some());
        assert!(world.owned_script_engine.is_some());
    }

    #[test]
    fn promotion_restarts_queue_walk_after_synchronous_prepend() {
        let mut world = callback_world();
        let complete = AITeamQueue::new("Completed".into(), Vec::new(), false, 0);
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_queue
            .push_back(complete);
        world.check_owned_ai_queued_teams(1, 0.0);
        assert_eq!(
            world.peek_new_script_messages(),
            vec!["Transmission: nested:current-and-peer"; 8]
        );
        assert_eq!(world.ai_manager.ai_players[&2].team_queue.len(), 4);
        assert!(
            world.ai_manager.ai_players[&1]
                .team_ready_queue
                .iter()
                .any(|node| node.name == "Completed")
        );
    }

    #[test]
    fn queued_condition_reads_actual_team_members_and_ai_idle_without_name_heuristics() {
        use crate::game_logic::object::unit_ai_runtime::GuardPhase;
        let mut world = callback_world();
        let unit =
            world.ai_manager.ai_players[&1].team_queue[0].work_orders[0].observed_unit_ids[0];
        world.ai_manager.ai_players.get_mut(&1).unwrap().team_queue[0].work_orders[0]
            .observed_unit_ids
            .clear();
        let mut factory = world.team_factory.lock().unwrap();
        let mut parent = (*factory.find_team_prototype("Parent").unwrap()).clone();
        parent.set_production_condition("HuntNameOnly".into());
        factory.replace_team_prototype(parent);
        drop(factory);
        let mut script = Script::new();
        script.script_name = "HuntNameOnly".into();
        script.is_subroutine = true;
        let mut action = ScriptAction::new(ScriptActionType::DisplayText);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::TextString,
                "roster:idle".into(),
            ))
            .unwrap();
        script.action = Some(Box::new(action));
        let mut scripts = ScriptList::new();
        scripts.append_script(Box::new(script));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(scripts)))
            .unwrap();
        world.install_owned_script_engine(engine);
        let mut absent = ThingTemplate::new("NoAiMember");
        absent.add_kind_of(KindOf::Infantry);
        absent.set_authored_ai_update_interface(Some(false));
        world.templates.insert(absent.name.clone(), absent);
        let no_ai = world
            .create_object_for_player("NoAiMember", 1, Vec3::ZERO)
            .unwrap();
        let team_id = world.ai_manager.ai_players[&1].team_queue[0]
            .team_id
            .unwrap();
        let team = world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(team_id)
            .unwrap();
        team.write().unwrap().remove_member(unit.0);
        team.write().unwrap().add_member(no_ai.0);
        world.check_owned_ai_queued_teams(1, 0.0);
        assert!(world.peek_new_script_messages().is_empty());
        team.write().unwrap().remove_member(no_ai.0);
        team.write().unwrap().add_member(unit.0);
        let object = world.host_object_mut(unit).unwrap();
        object.restore_ai_state(AIState::GuardingArea);
        object
            .unit_ai_runtime
            .set_guard_phase(Some(GuardPhase::Return { goal: Vec3::ZERO }));
        world.check_owned_ai_queued_teams(1, 0.0);
        assert!(world.peek_new_script_messages().is_empty());
        world
            .host_object_mut(unit)
            .unwrap()
            .unit_ai_runtime
            .enter_guard_idle(30, Some(Vec3::ZERO));
        world.check_owned_ai_queued_teams(1, 0.0);
        assert_eq!(
            world.peek_new_script_messages(),
            ["Transmission: roster:idle"]
        );
        assert_eq!(
            world.host_object(unit).unwrap().ai_state(),
            AIState::GuardingArea,
            "script name must not invent Hunt orders"
        );
    }
    include!("native_ai_ui_tests.rs");
}

#[cfg(test)]
impl GameLogic {
    pub(crate) fn initialize_owned_skirmish_ai_from_definitions_for_test(
        &mut self,
        player: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let data = AiDataView::from_world(self);
        self.initialize_owned_skirmish_ai_with_effects(player, &data, effects);
    }
}
