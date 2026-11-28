//! Host script unit movement, command, and waypoint behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// C++ Player::setUnitsShouldHunt for the already resolved player.
    pub(super) fn apply_script_player_hunt(&mut self, player_id: u32) {
        use crate::game_logic::KindOf;

        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        player.units_should_hunt = true;
        let teams = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .owned_team_instances_for_controller(player_id);
        for team in teams {
            let members = team
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .get_members()
                .to_vec();
            for id in members.into_iter().map(ObjectId) {
                let Some(object) = self.host_object(id) else {
                    continue;
                };
                if object.is_kind_of(KindOf::Dozer)
                    || object.is_kind_of(KindOf::Harvester)
                    || object.is_kind_of(KindOf::IgnoresSelectAll)
                {
                    continue;
                }
                let can_hunt = object.is_mobile_for_ai_command()
                    && !object.is_kind_of(KindOf::Projectile);
                // Player.cpp visits its Team roster and leaves each AI group
                // before AIUpdateInterface admits the script command.
                self.host_object_leave_group(id);
                if can_hunt {
                    let _ = self.unit_command_script_hunt(id);
                }
            }
        }
    }

    /// ScriptActions.cpp:3215–3259 dispatches both actions to every human.
    pub(super) fn apply_owned_player_units_idle(&mut self, idle: bool) {
        let mut humans: Vec<_> = self
            .players
            .values()
            .filter(|player| player.is_human && !player.is_observer)
            .map(|player| player.id)
            .collect();
        humans.sort_unstable();
        for player in humans {
            let teams = self
                .team_factory
                .with_read_without_flushing(|factory| {
                    factory.owned_team_instances_for_controller(player)
                })
                .unwrap_or_else(|error| error.into_inner());
            for team in teams {
                let members = team
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_members()
                    .to_vec();
                for id in members.into_iter().map(ObjectId) {
                    let Some(object) = self.host_object(id).filter(|object| {
                        object.owner_player_id == Some(player)
                            && object.has_ai_update_interface()
                            && !object.is_kind_of(KindOf::Structure)
                    }) else {
                        continue;
                    };
                    if idle {
                        let position = object.get_position();
                        self.unit_command_script_move_to(id, position);
                    } else if object.ai_state() == AIState::Idle
                        && object.get_template().supply_truck_metadata.is_some()
                        && let Some(object) = self.host_object_mut(id)
                    {
                        object.supply_truck_force_pending = true;
                    }
                }
            }
        }
    }

    /// C++ `doNamedUseCommandButtonAbility*` / `doTeamUseCommandButtonAbility*`.
    pub(super) fn apply_host_use_command_button_script_requests(&mut self) {
        self.with_owned_script_engine(|engine, world| {
            world.apply_host_use_command_button_script_requests_with_policy(engine);
        })
        .expect("command-button drain owns its receiving interpreter");
    }

    pub(super) fn apply_host_use_command_button_script_requests_with_policy(
        &mut self,
        interpreter: &gamelogic::scripting::engine::ScriptEngine,
    ) {
        use crate::command_executor::CommandExecutor;
        use generals_sim::scripting::world::HostScriptUseCommandButtonRequest;
        for req in gamelogic::scripting::take_host_script_use_command_button_requests() {
            self.apply_receiving_command_button_request(
                req,
                None,
                None,
                None,
                interpreter,
                &mut Default::default(),
            );
        }
        self.apply_host_team_partial_command_button_requests();
    }

    pub(super) fn apply_receiving_command_button_request(
        &mut self,
        req: generals_sim::scripting::world::HostScriptUseCommandButtonRequest,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: &gamelogic::scripting::engine::ScriptEngine,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::command_executor::CommandExecutor;
        use generals_sim::scripting::world::HostScriptUseCommandButtonRequest;
        // One request is supplied directly by the action dispatcher. Keeping
        // the original request loop preserves its branch-local continues.
        for req in std::iter::once(req) {
            match req {
                HostScriptUseCommandButtonRequest::Named { unit, button } => {
                    let Some(id) = self.owned_named_script_object(&unit, this_object) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button(&[id], &button, None, None);
                }
                HostScriptUseCommandButtonRequest::NamedOnNamed {
                    unit,
                    button,
                    target,
                } => {
                    let Some(id) = self.owned_named_script_object(&unit, this_object) else {
                        continue;
                    };
                    let Some(tid) = self.owned_named_script_object(&target, this_object) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button(&[id], &button, None, Some(tid));
                }
                HostScriptUseCommandButtonRequest::NamedAtWaypoint {
                    unit,
                    button,
                    waypoint,
                } => {
                    let Some(id) = self.owned_named_script_object(&unit, this_object) else {
                        continue;
                    };
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button(&[id], &button, Some(pos), None);
                }
                HostScriptUseCommandButtonRequest::NamedUsingWaypointPath {
                    unit,
                    button,
                    path,
                } => {
                    let Some(id) = self.owned_named_script_object(&unit, this_object) else {
                        continue;
                    };
                    let from = self
                        .host_object(id)
                        .map(|o| o.get_position())
                        .unwrap_or(glam::Vec3::ZERO);
                    let Some(route) = self.host_script_waypoint_route_from(&path, from) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button_using_waypoints(
                        &[id],
                        &button,
                        &route.points,
                    );
                }
                HostScriptUseCommandButtonRequest::Team { team, button } => {
                    let ids: Vec<_> = self
                        .resolve_owned_script_team(&team, calling, condition)
                        .expect("receiving command-button team")
                        .map(|(_, ids)| ids.into_iter().map(ObjectId).collect())
                        .unwrap_or_default();
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button(&ids, &button, None, None);
                }
                HostScriptUseCommandButtonRequest::TeamOnNamed {
                    team,
                    button,
                    target,
                } => {
                    let ids: Vec<_> = self
                        .resolve_owned_script_team(&team, calling, condition)
                        .expect("receiving command-button team")
                        .map(|(_, ids)| ids.into_iter().map(ObjectId).collect())
                        .unwrap_or_default();
                    let Some(tid) = self.owned_named_script_object(&target, this_object) else {
                        continue;
                    };
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button(&ids, &button, None, Some(tid));
                }
                HostScriptUseCommandButtonRequest::TeamAtWaypoint {
                    team,
                    button,
                    waypoint,
                } => {
                    let ids: Vec<_> = self
                        .resolve_owned_script_team(&team, calling, condition)
                        .expect("receiving command-button team")
                        .map(|(_, ids)| ids.into_iter().map(ObjectId).collect())
                        .unwrap_or_default();
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new_with_target_policy_and_effects(
                        self,
                        pid,
                        interpreter,
                        effects.reborrow(),
                    )
                    .execute_do_command_button(&ids, &button, Some(pos), None);
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemy { team, button } => {
                    self.host_script_team_use_command_on_nearest_with_policy(
                        &team,
                        &button,
                        interpreter,
                        |s, viewer, obj| s.host_script_affiliation_allows(viewer, obj, true, false),
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestGarrisonedBuilding {
                    team,
                    button,
                } => {
                    self.host_script_team_use_command_on_nearest_with_policy(
                        &team,
                        &button,
                        interpreter,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, false)
                                && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                && obj.is_garrison_contain()
                        },
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestKindof {
                    team,
                    button,
                    kindof,
                } => {
                    let Some(kind) = Self::host_script_kind_from_token(&kindof) else {
                        continue;
                    };
                    self.host_script_team_use_command_on_nearest_with_policy(
                        &team,
                        &button,
                        interpreter,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, false)
                                && obj.is_kind_of(kind)
                        },
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuilding { team, button } => {
                    self.host_script_team_use_command_on_nearest_with_policy(
                        &team,
                        &button,
                        interpreter,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, false)
                                && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                        },
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuildingClass {
                    team,
                    button,
                    kindof,
                } => {
                    let Some(kind) = Self::host_script_kind_from_token(&kindof) else {
                        continue;
                    };
                    self.host_script_team_use_command_on_nearest_with_policy(
                        &team,
                        &button,
                        interpreter,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, false)
                                && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                && obj.is_kind_of(kind)
                        },
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestObjectType {
                    team,
                    button,
                    object_type,
                } => {
                    self.host_script_team_use_command_on_nearest_with_policy(
                        &team,
                        &button,
                        interpreter,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, true)
                                && obj.template_name.eq_ignore_ascii_case(&object_type)
                        },
                    );
                }
            }
        }
    }

    pub(super) fn host_script_kind_from_token(token: &str) -> Option<crate::game_logic::KindOf> {
        use crate::game_logic::KindOf;
        let t = token.trim();
        let t = t
            .strip_prefix("KINDOF_")
            .or_else(|| t.strip_prefix("KINDOF"))
            .unwrap_or(t);
        let u = t.to_ascii_uppercase();
        match u.as_str() {
            "INFANTRY" => Some(KindOf::Infantry),
            "VEHICLE" => Some(KindOf::Vehicle),
            "STRUCTURE" | "BUILDING" => Some(KindOf::Structure),
            "AIRCRAFT" => Some(KindOf::Aircraft),
            "HERO" => Some(KindOf::Hero),
            "DOZER" => Some(KindOf::Dozer),
            "HARVESTER" => Some(KindOf::Harvester),
            "MINE" => Some(KindOf::Mine),
            "PROJECTILE" => Some(KindOf::Projectile),
            "COMMANDCENTER" | "COMMAND_CENTER" => Some(KindOf::CommandCenter),
            "FSBARRACKS" | "FS_BARRACKS" => Some(KindOf::FSBarracks),
            "FSWARFACTORY" | "FS_WARFACTORY" => Some(KindOf::FSWarFactory),
            "FSAIRFIELD" | "FS_AIRFIELD" => Some(KindOf::FSAirfield),
            "FSBASEDEFENSE" | "FS_BASE_DEFENSE" | "BASEDEFENSE" => Some(KindOf::FSBaseDefense),
            "TECHBUILDING" | "TECH_BUILDING" => Some(KindOf::TechBuilding),
            other => KindOf::from_ini_token(other),
        }
    }

    pub(super) fn host_script_affiliation_allows(
        &self,
        viewer: u32,
        candidate: &crate::game_logic::Object,
        allow_enemies: bool,
        allow_neutral: bool,
    ) -> bool {
        use crate::game_logic::Team;
        use game_engine::common::game_common::Relationship;
        let Some(oid) = candidate.owner_player_id else {
            let vt = self
                .players
                .get(&viewer)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            if candidate.team == Team::Neutral || vt == Team::Neutral {
                return allow_neutral;
            }
            if candidate.team == vt {
                return false;
            }
            return allow_enemies;
        };
        let rel = self
            .players
            .get(&viewer)
            .and_then(|p| p.map_relationship(oid))
            .unwrap_or_else(|| {
                let vt = self
                    .players
                    .get(&viewer)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                let ot = self
                    .players
                    .get(&oid)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                if vt == ot {
                    Relationship::Allies
                } else if vt == Team::Neutral || ot == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            });
        match rel {
            Relationship::Enemies => allow_enemies,
            Relationship::Neutral => allow_neutral,
            Relationship::Allies => false,
        }
    }

    pub(super) fn host_script_team_center(
        &self,
        ids: &[crate::game_logic::ObjectId],
    ) -> Option<glam::Vec3> {
        let mut acc = glam::Vec3::ZERO;
        let mut n = 0.0;
        for id in ids {
            if let Some(obj) = self.host_object(*id) {
                acc += obj.get_position();
                n += 1.0;
            }
        }
        if n <= 0.0 { None } else { Some(acc / n) }
    }

    pub(super) fn host_script_team_use_command_on_nearest(
        &mut self,
        team: &str,
        button: &str,
        pred: impl Fn(&Self, u32, &crate::game_logic::Object) -> bool,
    ) {
        self.with_owned_script_engine(|engine, world| {
            world.host_script_team_use_command_on_nearest_with_policy(team, button, engine, pred);
        })
        .expect("nearest command-button operation owns its receiving interpreter");
    }

    fn host_script_team_use_command_on_nearest_with_policy(
        &mut self,
        team: &str,
        button: &str,
        interpreter: &gamelogic::scripting::engine::ScriptEngine,
        pred: impl Fn(&Self, u32, &crate::game_logic::Object) -> bool,
    ) {
        use crate::command_executor::CommandExecutor;
        let ids = self.host_script_team_member_ids(team);
        if ids.is_empty() {
            return;
        }
        let pid = self
            .host_object(ids[0])
            .and_then(|o| o.owner_player_id)
            .unwrap_or(0);
        let Some(center) = self.host_script_team_center(&ids) else {
            return;
        };
        let team_set: std::collections::HashSet<_> = ids.iter().copied().collect();
        let mut best = None;
        let mut best_d = f32::MAX;
        for obj in self.objects.registered_values() {
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            if team_set.contains(&obj.id) {
                continue;
            }
            if !pred(self, pid, obj) {
                continue;
            }
            let p = obj.get_position();
            let dx = p.x - center.x;
            let dz = p.z - center.z;
            let d = dx * dx + dz * dz;
            if d < best_d {
                best_d = d;
                best = Some(obj.id);
            }
        }
        let Some(tid) = best else {
            return;
        };
        let _ = CommandExecutor::new_with_target_policy(self, pid, interpreter)
            .execute_do_command_button(&ids, button, None, Some(tid));
    }

    pub(super) fn apply_host_team_partial_command_button_requests(&mut self) {
        for req in gamelogic::scripting::take_host_team_partial_command_button_requests() {
            let _ = self.apply_owned_team_partial_button(
                &req.team,
                &req.button,
                req.percentage,
                None,
                None,
            );
        }
    }
    pub(super) fn owned_partial_button_members(
        &self,
        members: &[u32],
        button: &game_engine::common::ini::ini_command_button::CommandButton,
        percentage: f32,
    ) -> Vec<ObjectId> {
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption as Options;
        let options = Options::from_bits_truncate(button.options_bits);
        let needs_target = options.intersects(
            Options::NEED_TARGET_ENEMY_OBJECT
                | Options::NEED_TARGET_NEUTRAL_OBJECT
                | Options::NEED_TARGET_ALLY_OBJECT
                | Options::NEED_TARGET_PRISONER
                | Options::NEED_TARGET_POS,
        );
        let mut valid = Vec::new();
        for &id in members {
            let Some(object) = self.host_object(ObjectId(id)) else {
                continue;
            };
            let accepted = if !button.upgrade.is_empty() {
                object.building_data.as_ref().is_some_and(|production| {
                    !production
                        .production_queue
                        .iter()
                        .any(|entry| entry.is_upgrade())
                }) && object.affected_by_object_upgrade(&button.upgrade)
                    && !object.has_object_upgrade_complete(&button.upgrade)
            } else if needs_target {
                false
            } else {
                button
                    .special_power_template
                    .as_deref()
                    .and_then(|name| {
                        object
                            .thing()
                            .template
                            .special_power_modules
                            .iter()
                            .find(|module| module.special_power_template.eq_ignore_ascii_case(name))
                    })
                    .and_then(|module| module.command_power.as_ref())
                    .is_some_and(generals_sim::command::special_power_is_no_target)
            };
            if accepted {
                valid.push(object.id);
            }
        }
        // CPP5793 truncates (despite the commented CEIL label) AFTER validity.
        let count = ((percentage / 100.0) * valid.len() as f32) as i32;
        valid.truncate(count.max(0) as usize);
        valid
    }
    pub(super) fn apply_owned_team_partial_button(
        &mut self,
        team: &str,
        name: &str,
        percentage: f32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> generals_sim::GameLogicResult<()> {
        // Definitions are immutable during a match; the catalog guard does not
        // cross production callbacks or command execution.
        let Some(button) = self.world_ini_control_bar()
            .and_then(|catalog| catalog.find_command_button_resolved(name).cloned())
        else {
            return Ok(());
        };
        self.apply_owned_team_partial_button_definition(
            team, &button, percentage, calling, condition,
        )
    }
    pub(super) fn apply_owned_team_partial_button_definition(
        &mut self,
        team: &str,
        button: &game_engine::common::ini::ini_command_button::CommandButton,
        percentage: f32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        let selected = self.owned_partial_button_members(&members, button, percentage);
        for id in selected {
            let Some(owner) = self.host_object(id).and_then(|o| o.owner_player_id) else {
                continue;
            };
            let _ = crate::command_executor::CommandExecutor::new(self, owner)
                .execute_authored_command_button(&[id], button, None, None);
        }
        Ok(())
    }

    pub(crate) fn host_script_damage_with_interpreter(
        &mut self,
        id: ObjectId,
        amount: f32,
        kill: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> generals_sim::GameLogicResult<()> {
        self.host_script_damage_with_interpreter_with_effects(
            id,
            amount,
            kill,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn host_script_damage_with_interpreter_with_effects(
        &mut self,
        id: ObjectId,
        amount: f32,
        kill: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let context = if kill {
            crate::game_logic::object::DamageHitContext::for_kill()
        } else {
            crate::game_logic::object::DamageHitContext::default()
        }
        .with_topple_interpreter(interpreter);
        let amount = if kill {
            self.host_object(id)
                .map(|object| {
                    if object.health.maximum > 0.0 {
                        object.health.maximum
                    } else {
                        object.max_health.max(1.0)
                    }
                })
                .unwrap_or(0.0)
        } else {
            amount
        };
        self.apply_owned_damage_with_effects(
            id,
            amount,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
            effects,
        )?;
        Ok(())
    }

    /// ScriptActions.cpp:1656–1763 qualifies the path using the running
    /// script's player and forms the receiving Team's group before path lookup.
    pub(super) fn apply_owned_skirmish_approach_path(
        &mut self,
        team: &str,
        path_label: &str,
        as_team: bool,
        follow: bool,
        player: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some((_, roster)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        let group = self.create_team_ai_group(&roster);
        let Some(center) = self.script_team_center(&roster) else {
            return Ok(());
        };
        let Some(player) = player else {
            return Ok(());
        };
        let Some(enemy) = self.owned_skirmish_script_enemy(player) else {
            return Ok(());
        };
        let path_label = format!("{path_label}{}", self.players[&enemy].start_position + 1);
        let Some(path) = self.host_script_waypoint_route_from(&path_label, center) else {
            return Ok(());
        };
        if follow {
            if let Some(&first) = roster.first()
                && let Some(waypoint) = self.host_script_closest_waypoint_id(&path_label, center)
                && let Some(mut ai) = self.ai_manager.ai_players.remove(&player)
            {
                let _ = ai.check_bridges(self, ObjectId(first), waypoint);
                self.ai_manager.ai_players.insert(player, ai);
            }
            self.host_script_issue_follow_waypoint_path(
                &self.ai_group_members(group),
                &path.points,
                false,
                as_team,
                path.completion(false),
            );
        } else if let Some(&destination) = path.points.first() {
            let members = self.ai_group_members(group);
            let mut executor = crate::command_executor::CommandExecutor::new(self, 0);
            let _ = executor.execute_script_team_move(&members, destination)?;
        }
        Ok(())
    }
}
