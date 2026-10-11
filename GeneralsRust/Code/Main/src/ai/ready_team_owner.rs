//! CPP AIPlayer::checkReadyTeams (2729-2803), using canonical session teams.
use super::*;

impl GameLogic {
    fn ready_object_is_idle(&self, id: ObjectId) -> bool {
        self.host_object(id).is_some_and(|object| {
            object.has_ai_update_interface()
                && (object.ai_state == AIState::Idle
                    || (matches!(
                        object.ai_state,
                        AIState::GuardingArea | AIState::GuardingObject
                    ) && object.unit_ai_runtime.guard_phase()
                        == Some(crate::game_logic::object::unit_ai_runtime::GuardPhase::Idle)))
        })
    }

    pub(crate) fn check_owned_ai_ready_teams(&mut self, player: u32) {
        let mut index = 0;
        loop {
            let Some(node) = self
                .ai_manager
                .ai_players
                .get(&player)
                .and_then(|ai| ai.team_ready_queue.get(index))
                .cloned()
            else {
                return;
            };
            let team = node.team_id.and_then(|id| {
                self.team_factory
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .find_team_by_id(id)
            });
            // C++ always has m_team here. A stale reference cannot authorize
            // manufacturing another instance or executing a creation callback.
            let Some(team) = team else {
                index += 1;
                continue;
            };
            let members: Vec<ObjectId> = team
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .get_members()
                .iter()
                .copied()
                .map(ObjectId)
                .collect();
            let (mut all_idle, any_idle) = if node.reinforcement {
                match node.reinforcement_id.filter(|id| {
                    self.host_object(*id)
                        .is_some_and(|object| object.has_ai_update_interface())
                }) {
                    Some(id) => {
                        let idle = self.ready_object_is_idle(id);
                        (idle, idle)
                    }
                    None => (true, false),
                }
            } else {
                // Team::isIdle ignores effectively dead and non-AI members;
                // the independent anyIdle probe does not filter dead objects.
                (
                    members.iter().all(|id| {
                        self.host_object(*id).is_none_or(|object| {
                            !object.has_ai_update_interface()
                                || object.status.effectively_dead
                                || self.ready_object_is_idle(*id)
                        })
                    }),
                    members.iter().any(|id| self.ready_object_is_idle(*id)),
                )
            };
            let prototype = self
                .team_factory
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .find_team_prototype(&node.name);
            if any_idle
                && prototype
                    .as_ref()
                    .is_some_and(|prototype| prototype.get_execute_actions_on_create())
            {
                let name = prototype
                    .as_ref()
                    .unwrap()
                    .get_production_condition()
                    .to_string();
                if self
                    .owned_script_engine
                    .as_ref()
                    .and_then(|engine| engine.find_script_clone_by_name(&name))
                    .is_some_and(|script| script.get_action().is_some())
                {
                    all_idle = true;
                }
            }
            // Original unsigned logic-frame deadline is strict, not >= and
            // not elapsed floating-point seconds.
            if node.frame_started.wrapping_add(60 * 30) < self.frame {
                all_idle = true;
            }
            if !all_idle {
                index += 1;
                continue;
            }
            let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
                return;
            };
            ai.team_ready_queue.remove(index);
            ai.activity_count = ai.activity_count.saturating_add(1);
            if node.reinforcement {
                if let Some(id) = node.reinforcement_id.filter(|id| {
                    self.host_object(*id)
                        .is_some_and(|object| object.has_ai_update_interface())
                }) {
                    AIPlayer::join_team_reinforcement_host(self, id, &members);
                }
            } else {
                team.write()
                    .unwrap_or_else(|error| error.into_inner())
                    .set_active();
                if self.game_mode() == GameMode::Skirmish {
                    if let Some(engine) = &self.owned_script_engine {
                        engine.clear_team_flags();
                    }
                }
                // Team::updateState executes OnCreate at the subsequent script
                // phase. No script-name classification or immediate host order.
            }
            // C++ restarts the linked-list iterator after deleting its node.
            index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, ThingTemplate};
    use gamelogic::scripting::engine::ScriptEngine;

    fn fixture() -> (GameLogic, ObjectId, u32) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "ReadyOwner", false));
        world.add_ai_opponent(1, Team::USA, AIDifficulty::Medium);
        let mut template = ThingTemplate::new("ReadyInfantry");
        template.add_kind_of(KindOf::Infantry);
        world.templates.insert(template.name.clone(), template);
        let unit = world
            .create_object_for_player("ReadyInfantry", 1, Vec3::ZERO)
            .unwrap();
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "ReadyTeam".into(),
                "ReadyOwner".into(),
                false,
                None,
                Some(1),
            )
            .unwrap();
        let team = factory
            .create_inactive_team_with_owned_effects("ReadyTeam")
            .unwrap()
            .0;
        drop(factory);
        let id = {
            let mut team = team.write().unwrap();
            team.add_member(unit.0);
            team.get_id()
        };
        world.host_object_mut(unit).unwrap().team_instance_name = "ReadyTeam".into();
        world.install_owned_script_engine(ScriptEngine::new().unwrap());
        let mut node = AITeamQueue::new("ReadyTeam".into(), Vec::new(), false, 100);
        node.team_id = Some(id);
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_ready_queue
            .push_back(node);
        (world, unit, id)
    }
    fn active(world: &GameLogic, id: u32) -> bool {
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(id)
            .unwrap()
            .read()
            .unwrap()
            .is_active()
    }

    #[test]
    fn ready_deadline_is_strict_logic_frames_and_does_not_invent_host_orders() {
        let (mut world, unit, id) = fixture();
        world.host_object_mut(unit).unwrap().ai_state = AIState::Moving;
        // A name alone is never permission to issue a Hunt order.
        let mut prototype = (*world
            .team_factory
            .lock()
            .unwrap()
            .find_team_prototype("ReadyTeam")
            .unwrap())
        .clone();
        prototype.set_script_on_create("HuntLikeNameWithoutScript".into());
        world
            .team_factory
            .lock()
            .unwrap()
            .replace_team_prototype(prototype);
        world.frame = 1900;
        world.check_owned_ai_ready_teams(1);
        assert!(!active(&world, id));
        assert_eq!(world.ai_manager.ai_players[&1].team_ready_queue.len(), 1);
        world.frame = 1901;
        world.check_owned_ai_ready_teams(1);
        assert!(active(&world, id));
        assert!(world.ai_manager.ai_players[&1].team_ready_queue.is_empty());
        assert_eq!(world.host_object(unit).unwrap().ai_state, AIState::Moving);
        assert_eq!(world.ai_manager.ai_players.len(), 1);
    }

    #[test]
    fn ready_team_reads_live_roster_and_ignores_dead_or_non_ai_members() {
        let (mut world, unit, id) = fixture();
        world.host_object_mut(unit).unwrap().ai_state = AIState::Moving;
        world.host_object_mut(unit).unwrap().status.effectively_dead = true;
        world.frame = 101;
        world.check_owned_ai_ready_teams(1);
        assert!(active(&world, id));
        assert!(world.ai_manager.ai_players[&1].team_ready_queue.is_empty());
    }

    #[test]
    fn missing_ready_team_does_not_create_replacement_or_run_callback() {
        let (mut world, unit, _) = fixture();
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_ready_queue
            .front_mut()
            .unwrap()
            .team_id = Some(99_999);
        let before = world
            .team_factory
            .lock()
            .unwrap()
            .find_team_instances("ReadyTeam")
            .len();
        world.frame = 9999;
        world.check_owned_ai_ready_teams(1);
        assert_eq!(
            world
                .team_factory
                .lock()
                .unwrap()
                .find_team_instances("ReadyTeam")
                .len(),
            before
        );
        assert_eq!(world.ai_manager.ai_players[&1].team_ready_queue.len(), 1);
        assert_eq!(world.host_object(unit).unwrap().ai_state, AIState::Idle);
    }
    #[test]
    fn ready_reinforcement_joins_actual_team_order_without_activating_team() {
        let (mut world, leader, id) = fixture();
        world.host_object_mut(leader).unwrap().ai_state = AIState::Moving;
        let reinforcement = world
            .create_object_for_player("ReadyInfantry", 1, Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(id)
            .unwrap()
            .write()
            .unwrap()
            .add_member(reinforcement.0);
        let node = world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_ready_queue
            .front_mut()
            .unwrap();
        node.reinforcement = true;
        node.reinforcement_id = Some(reinforcement);
        world.frame = 101;
        world.check_owned_ai_ready_teams(1);
        assert_eq!(
            world.host_object(reinforcement).unwrap().ai_state,
            AIState::Moving
        );
        assert!(!active(&world, id));
        assert!(world.ai_manager.ai_players[&1].team_ready_queue.is_empty());
        assert_eq!(world.ai_manager.ai_players.len(), 1);
    }
    #[test]
    fn activation_runs_real_on_create_once_at_next_owned_script_tick() {
        use gamelogic::scripting::core::{
            Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
        };
        let (mut world, unit, id) = fixture();
        let (mut peer, _, peer_id) = fixture();
        let mut prototype = (*world
            .team_factory
            .lock()
            .unwrap()
            .find_team_prototype("ReadyTeam")
            .unwrap())
        .clone();
        prototype.set_script_on_create("ActualCreate".into());
        world
            .team_factory
            .lock()
            .unwrap()
            .replace_team_prototype(prototype);
        // Existing instance copied its prototype scripts at construction, so
        // recreate the actual team after definitions are admitted.
        let deletion = world
            .team_factory
            .lock()
            .unwrap()
            .prepare_host_team_deletion(id)
            .unwrap();
        world
            .team_factory
            .lock()
            .unwrap()
            .finalize_host_team_deletion(deletion);
        let team = world
            .team_factory
            .lock()
            .unwrap()
            .create_inactive_team_with_owned_effects("ReadyTeam")
            .unwrap()
            .0;
        let id = {
            let mut team = team.write().unwrap();
            team.add_member(unit.0);
            team.get_id()
        };
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_ready_queue
            .front_mut()
            .unwrap()
            .team_id = Some(id);
        let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "ActualCreateReceipt".into(),
            ))
            .unwrap();
        let mut script = Script::new();
        script.script_name = "ActualCreate".into();
        script.is_subroutine = true;
        let mut condition = gamelogic::scripting::OrCondition::new();
        condition.set_first_and_condition(Some(Box::new(gamelogic::scripting::Condition::new(
            gamelogic::scripting::ConditionType::ConditionTrue,
        ))));
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_counter("ActualCreateReceipt", 0)
            .unwrap();
        peer.owned_script_engine
            .as_ref()
            .unwrap()
            .set_counter("ActualCreateReceipt", 91)
            .unwrap();
        world.check_owned_ai_ready_teams(1);
        assert!(active(&world, id));
        assert_eq!(
            world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("ActualCreateReceipt")
                .unwrap()
                .value,
            0
        );
        world.ai_manager.ai_players.get_mut(&1).unwrap().is_active = false;
        world.scripts_loaded = true;
        world.update();
        assert_eq!(
            world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("ActualCreateReceipt")
                .unwrap()
                .value,
            1
        );
        world.update();
        assert_eq!(
            world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("ActualCreateReceipt")
                .unwrap()
                .value,
            1
        );
        assert_eq!(
            peer.owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("ActualCreateReceipt")
                .unwrap()
                .value,
            91
        );
        assert!(!active(&peer, peer_id));
    }
}
