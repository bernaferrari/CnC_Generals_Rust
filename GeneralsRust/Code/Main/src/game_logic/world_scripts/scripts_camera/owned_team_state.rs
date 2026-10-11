//! CPP ScriptEngine5573 -> Player::updateTeamStates -> Team::updateState.
use super::*;
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::team::{TeamStateObservation, TeamStateStep};

impl GameLogic {
    fn observe_owned_team_state(&self, members: &[u32]) -> TeamStateObservation {
        let (min, max) = self.world_bounds();
        let off_map = |position: Vec3| {
            crate::game_logic::host_deliver_payload::is_off_map_residual(
                position, min.x, min.z, max.x, max.z,
            )
        };
        let mut observation = TeamStateObservation {
            member_count: members.len() as i32,
            all_ai_idle: true,
            ..Default::default()
        };
        for id in members {
            let Some(object) = self.host_object(ObjectId(*id)) else {
                continue;
            };
            if object.status.effectively_dead {
                continue;
            }
            observation.live_count += 1;
            observation.any_live_member = true;
            if object.has_ai_update_interface() {
                observation.any_live_ai = true;
                observation.all_ai_idle &= object.ai_state == AIState::Idle
                    || (matches!(
                        object.ai_state,
                        AIState::GuardingArea | AIState::GuardingObject
                    ) && object.unit_ai_runtime.guard_phase()
                        == Some(crate::game_logic::object::unit_ai_runtime::GuardPhase::Idle));
            }
            if !observation.enemy_sighted {
                let position = object.get_position();
                let radius_sq = object.vision_range * object.vision_range;
                observation.enemy_sighted =
                    self.host_objects()
                        .iter()
                        .any(|(&candidate_id, candidate)| {
                            if candidate_id.0 == *id
                                || !candidate.is_alive()
                                || candidate.status.effectively_dead
                                || off_map(candidate.get_position()) != off_map(position)
                                || self.script_sighting_relationship(object, candidate)
                                    != gamelogic::common::Relationship::Enemies
                            {
                                return false;
                            }
                            let delta = candidate.get_position() - position;
                            delta.x * delta.x + delta.z * delta.z <= radius_sq
                        });
            }
        }
        observation
    }

    pub(crate) fn update_owned_script_team_states(
        &mut self,
        engine: &ScriptEngine,
    ) -> gamelogic::GameLogicResult<()> {
        // CPP PlayerList executes numeric slots, each player's prototype list
        // retains admission order, and each prototype visits its live instances.
        let players: Vec<_> = self.get_players().keys().copied().collect();
        let mut players = players;
        players.sort_unstable();
        for player in players {
            let prototypes = self
                .team_factory
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .owned_team_prototypes_for_controller(player);
            for prototype in prototypes {
                let name = prototype.get_name().to_string();
                let mut cursor = self
                    .team_factory
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .condition_team_after(&name, None);
                while let Some(id) = cursor {
                    let Some(team) = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .find_team_by_id(id)
                    else {
                        break;
                    };
                    let successor = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .condition_team_after(&name, Some(id));
                    let mut step = TeamStateStep::default();
                    while step != TeamStateStep::Done {
                        let members = team
                            .read()
                            .unwrap_or_else(|error| error.into_inner())
                            .get_members()
                            .to_vec();
                        let observation = self.observe_owned_team_state(&members);
                        let script = team
                            .write()
                            .unwrap_or_else(|error| error.into_inner())
                            .advance_owned_state(&mut step, observation);
                        if let Some(script) = script {
                            let player_name =
                                self.get_player(player).map(|player| player.name.clone());
                            let context = gamelogic::scripting::executor::ScriptContext {
                                game_logic_id: 0,
                                object_manager_id: 0,
                                player_manager_id: 0,
                                event_system_id: 0,
                                camera_system_id: 0,
                                audio_system_id: 0,
                                partition_manager_id: 0,
                                special_powers_id: 0,
                                current_frame: self.frame,
                                suppress_new_windows: false,
                                host_trigger_world: Arc::clone(&self.host_trigger_world),
                            };
                            let mut driver = super::script_execution_driver::HostScriptExecutionDriver::with_interpreter(self, engine);
                            engine.run_team_script_with_driver(
                                &script,
                                id,
                                player_name.as_deref(),
                                context,
                                &mut driver,
                            )?;
                        }
                        let current = self
                            .team_factory
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .find_team_by_id(id);
                        if !current.is_some_and(|current| Arc::ptr_eq(&current, &team)) {
                            break;
                        }
                    }
                    let factory = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    cursor = if factory.find_team_by_id(id).is_some() {
                        factory.condition_team_after(&name, Some(id))
                    } else {
                        successor.filter(|id| factory.find_team_by_id(*id).is_some())
                    };
                }
                // TeamPrototype::updateState removes empty active non-singletons,
                // restarting after each synchronous deletion notification.
                loop {
                    let default = self.get_player(player).map(|owner| {
                        self.default_host_team_instance_name(Some(player), owner.team)
                    });
                    let empty = {
                        let factory = self
                            .team_factory
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        let default = default
                            .as_deref()
                            .and_then(|name| factory.find_team_instances(name).first().cloned())
                            .map(|team| {
                                team.read()
                                    .unwrap_or_else(|error| error.into_inner())
                                    .get_id()
                            });
                        factory.empty_active_team_for_prototype(&name, default)
                    };
                    let Some(empty) = empty else {
                        break;
                    };
                    if !self.delete_canonical_ai_team_with_interpreter(empty, engine) {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}
