//! CPP AIPlayer::checkQueuedTeams and buildSpecificAITeam callback phases.
use super::*;
use gamelogic::scripting::engine::ScriptEngine;

impl GameLogic {
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
        if !ai.is_active || ai.skirmish_new_map_applied {
            return;
        }
        ai.skirmish_new_map_applied = true;
        let fallback = ai.base_center;
        let has_definition = ai.has_skirmish_new_map_definition(data);
        if self
            .get_player(player)
            .is_some_and(|p| !p.map_side.build_list.is_empty())
        {
            return;
        }
        if !has_definition {
            return;
        }
        let center = self.host_objects().iter().find_map(|(&id, object)| {
            (object.is_alive()
                && object.is_kind_of(KindOf::CommandCenter)
                && self.object_owned_by_player(object, player))
            .then_some((id, object.get_position()))
        });
        let start = if let Some((id, pos)) = center {
            self.destroy_object(id);
            self.process_destroy_list();
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
    }

    pub(crate) fn update_owned_ai_military_with_effects(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use super::teams::OwnedTeamSelection;
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        let due = now >= ai.next_team_queue_time;
        if due {
            ai.process_team_queue_with_ai_data(self, now, data);
        }
        self.ai_manager.ai_players.insert(player, ai);
        let candidates: Vec<(String, i32)> = self
            .ai_manager
            .ai_players
            .get(&player)
            .filter(|ai| {
                now >= ai.next_team_queue_time
                    && now >= ai.next_team_time
                    && ai.team_queue.len() < 3
                    && self.get_player(player).is_none_or(|p| p.can_build_units)
            })
            .map(|ai| {
                ai.team_factory
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .owned_team_prototypes_for_controller(player)
                    .into_iter()
                    .map(|prototype| {
                        (
                            prototype.get_name().to_string(),
                            prototype.get_production_priority(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let conditions = candidates
            .iter()
            .cloned()
            .map(|(name, _)| {
                let result = self.evaluate_owned_team_condition(player, &name);
                (name, result)
            })
            .collect::<HashMap<_, _>>();
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        let selection = if due {
            if now >= ai.next_team_time
                && ai.should_build_new_team_after_owned_conditions(
                    self,
                    data,
                    &conditions,
                    &candidates,
                )
            {
                ai.prepare_team_selection_with_conditions(
                    self,
                    now,
                    data,
                    Some(&conditions),
                    Some(candidates),
                )
            } else {
                OwnedTeamSelection::None
            }
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
            return;
        };
        if due {
            if new_pick {
                ai.arm_team_timer_after_build_with_ai_data(self, now, data);
            }
            if selected {
                ai.process_team_queue_with_ai_data(self, now, data);
            } else if now >= ai.next_team_time {
                ai.next_team_time = now + AIPlayer::TEAM_QUEUE_RETRY_SECONDS;
            }
            ai.next_team_queue_time = now + AIPlayer::TEAM_QUEUE_RETRY_SECONDS;
        }
        ai.evaluate_attack_opportunities(self, now);
        self.ai_manager.ai_players.insert(player, ai);
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
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
            view: None,
        };
        self.update_owned_ai_military_with_effects(player, now, data, &mut effects)
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
            if orders.is_empty() || !ai.team_factories_exist(self, &orders) {
                return None;
            }
            Some((factory, orders, execute))
        })();
        let Some((factory, orders, execute)) = prepared else {
            return false;
        };
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
        for script in effects {
            if let Some(engine) = interpreter {
                self.execute_owned_ai_condition_with_effects(engine, &script, None, ui);
            } else {
                self.with_owned_script_engine(|engine, world| {
                    world.execute_owned_ai_condition_with_effects(engine, &script, None, ui);
                });
            }
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
        ai.next_team_queue_time = 0.0;
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
        if let Some(action) = engine
            .find_script_clone_by_name(name)
            .and_then(|script| script.get_action().cloned())
        {
            self.execute_script_team_actions_with_effects(engine, &action, team, ui);
        }
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
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return;
        };
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
        let skirmish = ai.leftover_is_skirmish_ai();
        loop {
            let decision = self.ai_manager.ai_players.get(&player).and_then(|ai| {
                ai.team_queue.iter().enumerate().find_map(|(index, team)| {
                    let expired = team.is_build_time_expired(&ai.team_factory, now);
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
                        .is_some_and(|o| o.has_ai_update_interface() && (o.ai_state == AIState::Idle
                            || (matches!(o.ai_state, AIState::GuardingArea | AIState::GuardingObject)
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
                unit_thing_name: "AmericaInfantryRanger",
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
        let mut world = GameLogic::new();
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
        let mut engine = ScriptEngine::new().unwrap();
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
    fn automatic_military_selection_runs_owned_condition_and_nested_build_immediately() {
        let mut world = callback_world();
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
        let mut engine = ScriptEngine::new().unwrap();
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
        let mut world = callback_world();
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
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        world.templates.insert(cc.name.clone(), cc);
        let mut supply = ThingTemplate::new("AmericaSupplyCenter");
        supply
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::SupplyCenter);
        world.templates.insert(supply.name.clone(), supply);
        let mut collector = ThingTemplate::new("AmericaVehicleChinook");
        collector.add_kind_of(KindOf::Harvester);
        world.templates.insert(collector.name.clone(), collector);
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.skirmish_new_map_applied = false;
        ai.next_team_queue_time = 999.0;
        let peer_timer = world.ai_manager.ai_players[&2].next_team_queue_time;
        let data = AiDataView::from_world(&world);
        world.initialize_owned_skirmish_ai(1, &data);
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
        assert!(world.host_objects().values().any(|o| {
            o.template_name == "AmericaVehicleChinook"
                && o.producer_id == pad.object_id
                && o.owner_player_id == Some(1)
        }));
        assert_eq!(world.ai_manager.ai_players.len(), 2);
    }

    #[test]
    fn factory_null_team_then_admitted_team_callbacks_do_not_inherit_caller_team() {
        let mut world = callback_world();
        let parent = world.ai_manager.ai_players[&1].team_queue.front().unwrap();
        let parent_id = parent.team_id.unwrap();
        let unit = parent.work_orders[0].observed_unit_ids[0];
        world
            .host_object_mut(unit)
            .unwrap()
            .set_ai_state(AIState::Moving);
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
        let mut engine = ScriptEngine::new().unwrap();
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
        assert_eq!(world.host_object(unit).unwrap().ai_state, AIState::Moving);
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
        let mut engine = ScriptEngine::new().unwrap();
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
        object.ai_state = AIState::GuardingArea;
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
            world.host_object(unit).unwrap().ai_state,
            AIState::GuardingArea,
            "script name must not invent Hunt orders"
        );
    }
    include!("native_ai_ui_tests.rs");
}
