//! Host script unit movement, command, and waypoint behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// C++ ScriptActions::doSetCaveIndex live drain.
    pub fn apply_host_set_cave_index_requests(&mut self) {
        for (cave_name, index) in gamelogic::scripting::take_host_set_cave_index_requests() {
            let _ = self.set_named_cave_index(&cave_name, index);
        }
    }

    /// C++ ScriptActions TEAM/NAMED move and attack live drain.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptMoveAttackRequest`].
    pub(super) fn apply_host_move_attack_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptMoveAttackRequest;
        for req in gamelogic::scripting::take_host_script_move_attack_requests() {
            match req {
                HostScriptMoveAttackRequest::TeamMove { team, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        let _ = self.unit_command_move_to(id, dest);
                    }
                }
                HostScriptMoveAttackRequest::NamedMove { unit, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_move_to(id, dest);
                }
                HostScriptMoveAttackRequest::TeamAttackTeam { attacker, victim } => {
                    let members = self.host_script_team_member_ids(&attacker);
                    for id in members {
                        self.host_script_attack_team(id, &victim);
                    }
                }
                HostScriptMoveAttackRequest::NamedAttackNamed { attacker, victim } => {
                    let Some(aid) = self.host_object_id_by_script_name(&attacker) else {
                        continue;
                    };
                    let Some(vid) = self.host_object_id_by_script_name(&victim) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(aid, "normal");
                    let _ = self.unit_command_force_attack(aid, vid);
                }
                HostScriptMoveAttackRequest::NamedAttackArea { unit, area } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    self.host_script_attack_area(id, &area);
                }
                HostScriptMoveAttackRequest::NamedAttackTeam { unit, team } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    self.host_script_attack_team(id, &team);
                }
                HostScriptMoveAttackRequest::TeamAttackArea { team, area } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        self.host_script_attack_area(id, &area);
                    }
                }
                HostScriptMoveAttackRequest::TeamAttackNamed { team, unit } => {
                    let Some(vid) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        let _ = self.unit_command_attack(id, vid);
                    }
                }
                HostScriptMoveAttackRequest::NamedMoveTowardsNearest {
                    unit,
                    object_type,
                    trigger,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(target) = self.host_script_closest_object_of_type_in_trigger(
                        id,
                        &object_type,
                        &trigger,
                    ) else {
                        continue;
                    };
                    let Some(dest) = self.host_object(target).map(|o| o.get_position()) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_move_to(id, dest);
                }
                HostScriptMoveAttackRequest::TeamMoveTowardsNearest {
                    team,
                    object_type,
                    trigger,
                } => {
                    let members = self.host_script_team_member_ids(&team);
                    let Some(&source) = members.first() else {
                        continue;
                    };
                    let Some(target) = self.host_script_closest_object_of_type_in_trigger(
                        source,
                        &object_type,
                        &trigger,
                    ) else {
                        continue;
                    };
                    let Some(dest) = self.host_object(target).map(|o| o.get_position()) else {
                        continue;
                    };
                    for id in members {
                        let _ = self.apply_unit_locomotor_set(id, "normal");
                        let _ = self.unit_command_move_to(id, dest);
                    }
                }
            }
        }
    }

    /// C++ ScriptActions TEAM/NAMED HUNT, TEAM/NAMED GUARD, PLAYER_HUNT.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptHuntGuardRequest`].
    pub(super) fn apply_host_hunt_guard_script_requests(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::HostScriptHuntGuardRequest;
        for req in gamelogic::scripting::take_host_script_hunt_guard_requests() {
            match req {
                HostScriptHuntGuardRequest::TeamHunt { team } => {
                    for id in self.host_script_hunt_guard_team_member_ids(&team) {
                        let _ = self.unit_command_patrol(id);
                    }
                }
                HostScriptHuntGuardRequest::NamedHunt { unit } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_patrol(id);
                }
                HostScriptHuntGuardRequest::TeamGuard { team } => {
                    // C++ doTeamGuard: leftover getTeamNamed instance, every member with AI.
                    let members = self.host_script_hunt_guard_team_member_ids(&team);
                    for id in members {
                        if !self.host_script_unit_can_guard(id) {
                            continue;
                        }
                        let Some(pos) = self.host_object(id).map(|u| u.get_position()) else {
                            continue;
                        };
                        let _ = self.unit_command_guard_position(id, pos);
                    }
                }
                HostScriptHuntGuardRequest::NamedGuard { unit } => {
                    // C++ doNamedGuard: AIUpdateInterface only (Stinger/stun still guard).
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    if !self.host_script_unit_can_guard(id) {
                        continue;
                    }
                    let Some(pos) = self.host_object(id).map(|u| u.get_position()) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_guard_position(id, pos);
                }
                HostScriptHuntGuardRequest::PlayerHunt { player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(player) = self.players.get_mut(&pid) {
                        player.units_should_hunt = true;
                    }

                    let team = self.players.get(&pid).map(|p| p.team);
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            if !obj.is_alive() || obj.status.destroyed {
                                return false;
                            }
                            if obj.is_kind_of(KindOf::Dozer)
                                || obj.is_kind_of(KindOf::Harvester)
                                || obj.is_kind_of(KindOf::IgnoresSelectAll)
                            {
                                return false;
                            }
                            match obj.owner_player_id {
                                Some(oid) => oid == pid,
                                None => team.map(|t| obj.team == t).unwrap_or(false),
                            }
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        // C++ Player::setUnitsShouldHunt: leaveGroup then aiHunt.
                        self.host_object_leave_group(id);
                        let _ = self.unit_command_patrol(id);
                    }
                }
                HostScriptHuntGuardRequest::TeamHuntWithCommandButton { team, button } => {
                    self.host_script_team_hunt_with_command_button(&team, &button);
                }
            }
        }
    }

    /// C++ ScriptActions NAMED_STOP / TEAM_STOP / TEAM_STOP_AND_DISBAND.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptIdleRequest`] (`aiIdle` / `groupIdle`).
    pub(super) fn apply_host_idle_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptIdleRequest;
        for req in gamelogic::scripting::take_host_script_idle_requests() {
            match req {
                HostScriptIdleRequest::NamedStop { unit } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.unit_command_stop(id);
                }
                HostScriptIdleRequest::TeamStop { team, disband } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let _ = self.unit_command_stop(id);
                    }
                    if !disband {
                        continue;
                    }
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let Some((owner, faction)) = self
                            .host_object(id)
                            .map(|obj| (obj.owner_player_id, obj.team))
                        else {
                            continue;
                        };
                        let default = self.default_host_team_instance_name(owner, faction);
                        if let Some(obj) = self.host_object_mut(id) {
                            obj.team_instance_name = default;
                        }
                    }
                }
                HostScriptIdleRequest::IdleAll { player } => {
                    let pids = self.host_script_idle_or_resume_player_ids(&player);
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            obj.is_alive()
                                && !obj.status.destroyed
                                && !obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                && obj
                                    .owner_player_id
                                    .map(|pid| pids.contains(&pid))
                                    .unwrap_or(false)
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        let pos = self
                            .host_object(id)
                            .map(|o| o.get_position())
                            .unwrap_or(glam::Vec3::ZERO);
                        // C++ aiMoveToPosition(self) — stop in place.
                        let _ = self.unit_command_move_to(id, pos);
                    }
                }
                HostScriptIdleRequest::ResumeSupply { player } => {
                    let pids = self.host_script_idle_or_resume_player_ids(&player);
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            obj.is_alive()
                                && !obj.status.destroyed
                                && !obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                && obj.ai_state == crate::game_logic::AIState::Idle
                                && (obj.is_kind_of(crate::game_logic::KindOf::Harvester)
                                    || obj.is_kind_of(crate::game_logic::KindOf::Dozer))
                                && obj
                                    .owner_player_id
                                    .map(|pid| pids.contains(&pid))
                                    .unwrap_or(false)
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        if let Some(obj) = self.host_object_mut(id) {
                            obj.supply_truck_force_pending = true;
                        }
                    }
                }
            }
        }
    }

    /// C++ `doIdleAllPlayerUnits` / `doResumeSupplyTruckingForIdleUnits`.
    /// Empty name walks every local/human player (dispatch always passes empty).
    pub(super) fn host_script_idle_or_resume_player_ids(&self, player: &str) -> Vec<u32> {
        if let Some(pid) = self.host_player_id_for_script_token(player) {
            return vec![pid];
        }
        let locals: Vec<u32> = self
            .players
            .values()
            .filter(|p| p.is_local && !p.is_observer)
            .map(|p| p.id)
            .collect();
        if !locals.is_empty() {
            return locals;
        }
        self.players
            .values()
            .filter(|p| !p.is_observer && p.is_alive)
            .map(|p| p.id)
            .collect()
    }

    /// C++ `doNamedUseCommandButtonAbility*` / `doTeamUseCommandButtonAbility*`.
    pub(super) fn apply_host_use_command_button_script_requests(&mut self) {
        use crate::command_executor::CommandExecutor;
        use gamelogic::scripting::HostScriptUseCommandButtonRequest;
        for req in gamelogic::scripting::take_host_script_use_command_button_requests() {
            match req {
                HostScriptUseCommandButtonRequest::Named { unit, button } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &[id],
                        &button,
                        None,
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::NamedOnNamed {
                    unit,
                    button,
                    target,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &[id],
                        &button,
                        None,
                        Some(tid),
                    );
                }
                HostScriptUseCommandButtonRequest::NamedAtWaypoint {
                    unit,
                    button,
                    waypoint,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &[id],
                        &button,
                        Some(pos),
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::NamedUsingWaypointPath {
                    unit,
                    button,
                    path,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let from = self
                        .host_object(id)
                        .map(|o| o.get_position())
                        .unwrap_or(glam::Vec3::ZERO);
                    let Some(wps) = self.host_script_waypoint_path_from(&path, from) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid)
                        .execute_do_command_button_using_waypoints(&[id], &button, &wps);
                }
                HostScriptUseCommandButtonRequest::Team { team, button } => {
                    let ids = self.host_script_team_member_ids(&team);
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid)
                        .execute_do_command_button(&ids, &button, None, None);
                }
                HostScriptUseCommandButtonRequest::TeamOnNamed {
                    team,
                    button,
                    target,
                } => {
                    let ids = self.host_script_team_member_ids(&team);
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &ids,
                        &button,
                        None,
                        Some(tid),
                    );
                }
                HostScriptUseCommandButtonRequest::TeamAtWaypoint {
                    team,
                    button,
                    waypoint,
                } => {
                    let ids = self.host_script_team_member_ids(&team);
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
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &ids,
                        &button,
                        Some(pos),
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemy { team, button } => {
                    self.host_script_team_use_command_on_nearest(
                        &team,
                        &button,
                        |s, viewer, obj| s.host_script_affiliation_allows(viewer, obj, true, false),
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestGarrisonedBuilding {
                    team,
                    button,
                } => {
                    self.host_script_team_use_command_on_nearest(
                        &team,
                        &button,
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
                    self.host_script_team_use_command_on_nearest(
                        &team,
                        &button,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, false)
                                && obj.is_kind_of(kind)
                        },
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuilding { team, button } => {
                    self.host_script_team_use_command_on_nearest(
                        &team,
                        &button,
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
                    self.host_script_team_use_command_on_nearest(
                        &team,
                        &button,
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
                    self.host_script_team_use_command_on_nearest(
                        &team,
                        &button,
                        |s, viewer, obj| {
                            s.host_script_affiliation_allows(viewer, obj, true, true)
                                && obj.template_name.eq_ignore_ascii_case(&object_type)
                        },
                    );
                }
            }
        }
        self.apply_host_team_partial_command_button_requests();
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
        use gamelogic::common::Relationship;
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
        if n <= 0.0 {
            None
        } else {
            Some(acc / n)
        }
    }

    pub(super) fn host_script_team_use_command_on_nearest(
        &mut self,
        team: &str,
        button: &str,
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
        for obj in self.objects.values() {
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
        let _ = CommandExecutor::new(self, pid).execute_do_command_button(
            &ids,
            button,
            None,
            Some(tid),
        );
    }

    pub(super) fn apply_host_team_partial_command_button_requests(&mut self) {
        use crate::command_executor::CommandExecutor;
        use crate::command_system::command_type_from_button_name;
        for req in gamelogic::scripting::take_host_team_partial_command_button_requests() {
            let mut ids = self.host_script_team_member_ids(&req.team);
            if ids.is_empty() || command_type_from_button_name(&req.button).is_none() {
                continue;
            }
            let mut num_to_use = ((req.percentage / 100.0) * ids.len() as f32) as i32;
            if num_to_use <= 0 {
                continue;
            }
            if num_to_use > ids.len() as i32 {
                num_to_use = ids.len() as i32;
            }
            ids.truncate(num_to_use as usize);
            let pid = self
                .host_object(ids[0])
                .and_then(|o| o.owner_player_id)
                .unwrap_or(0);
            for id in ids {
                let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                    &[id],
                    &req.button,
                    None,
                    None,
                );
            }
        }
    }

    /// C++ ScriptActions NAMED/TEAM DELETE / KILL / DAMAGE.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptKillDeleteDamageRequest`].
    pub(super) fn apply_host_kill_delete_damage_script_requests(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::HostScriptKillDeleteDamageRequest;
        const HUGE_DAMAGE_AMOUNT: f32 = 999999.0;
        for req in gamelogic::scripting::take_host_script_kill_delete_damage_requests() {
            match req {
                HostScriptKillDeleteDamageRequest::NamedDelete { unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.destroy_object(id);
                    }
                }
                HostScriptKillDeleteDamageRequest::NamedKill { unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_kill_object(id, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::NamedDamage { unit, amount } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_apply_unresistable(id, amount as f32, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::TeamDelete { team, ignore_dead } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        if ignore_dead {
                            let skip = self
                                .host_object(id)
                                .map(|o| !o.is_alive() || o.status.destroyed)
                                .unwrap_or(true);
                            if skip {
                                continue;
                            }
                        }
                        self.destroy_object(id);
                    }
                }
                HostScriptKillDeleteDamageRequest::TeamKill { team } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let is_tech = self
                            .host_object(id)
                            .map(|o| o.is_kind_of(KindOf::TechBuilding))
                            .unwrap_or(false);
                        if is_tech {
                            if let Some(obj) = self.host_object_mut(id) {
                                obj.team = Team::Neutral;
                            }
                            continue;
                        }
                        self.host_script_kill_object(id, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::TeamDamage { team, amount } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let skip = self
                            .host_object(id)
                            .map(|o| !o.is_alive() || o.status.destroyed)
                            .unwrap_or(true);
                        if skip {
                            continue;
                        }
                        self.host_script_apply_unresistable(id, amount, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::DestroyAllContained { unit } => {
                    let Some(container) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let mut occupants = self
                        .host_object(container)
                        .map(|o| o.contained_units())
                        .unwrap_or_default();
                    occupants.extend(
                        self.objects
                            .values()
                            .filter(|o| o.contained_by == Some(container) && o.is_alive())
                            .map(|o| o.id),
                    );
                    occupants.sort_by_key(|id| id.0);
                    occupants.dedup();
                    for occ in occupants {
                        self.host_script_kill_object(occ, HUGE_DAMAGE_AMOUNT);
                    }
                    if let Some(obj) = self.host_object_mut(container) {
                        if let Some(building) = obj.building_data.as_mut() {
                            building.garrisoned_units.clear();
                        }
                        obj.occupants.clear();
                    }
                }
            }
        }
    }

    /// C++ `Object::kill()` — HUGE unresistable damage with death effects.
    pub(super) fn host_script_kill_object(&mut self, id: ObjectId, huge: f32) {
        let dead = self
            .host_object_mut(id)
            .map(|obj| obj.take_damage_from(huge, None))
            .unwrap_or(false);
        if dead {
            self.destroy_object(id);
        }
    }

    /// C++ `attemptDamage` UNRESISTABLE; amount < 0 is `Object::kill()`.
    pub(super) fn host_script_apply_unresistable(&mut self, id: ObjectId, amount: f32, huge: f32) {
        if amount < 0.0 {
            self.host_script_kill_object(id, huge);
            return;
        }
        let dead = self
            .host_object_mut(id)
            .map(|obj| obj.take_damage_from(amount, None))
            .unwrap_or(false);
        if dead {
            self.destroy_object(id);
        }
    }

    /// C++ ScriptActions TEAM/NAMED FOLLOW_WAYPOINTS and EXACT.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptFollowWaypointsRequest`].
    pub(super) fn apply_host_follow_waypoints_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptFollowWaypointsRequest;
        for req in gamelogic::scripting::take_host_script_follow_waypoints_requests() {
            match req {
                HostScriptFollowWaypointsRequest::NamedFollow {
                    unit,
                    waypoint,
                    exact,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(pos) = self
                        .host_object(id)
                        .filter(|u| u.is_alive())
                        .map(|u| u.get_position())
                    else {
                        continue;
                    };
                    let Some(path) = self.host_script_waypoint_path_from(&waypoint, pos) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    self.host_script_issue_follow_waypoint_path(
                        &[id],
                        &path,
                        exact,
                        false,
                        &waypoint,
                    );
                }
                HostScriptFollowWaypointsRequest::TeamFollow {
                    team,
                    waypoint,
                    as_team,
                    exact,
                } => {
                    let members = self.host_script_team_member_ids(&team);
                    if members.is_empty() {
                        continue;
                    }
                    let mut sx = 0.0f32;
                    let mut sy = 0.0f32;
                    let mut sz = 0.0f32;
                    let mut n = 0u32;
                    for id in &members {
                        if let Some(pos) = self
                            .host_object(*id)
                            .filter(|u| u.is_alive())
                            .map(|u| u.get_position())
                        {
                            sx += pos.x;
                            sy += pos.y;
                            sz += pos.z;
                            n += 1;
                        }
                    }
                    if n == 0 {
                        continue;
                    }
                    let inv = 1.0 / n as f32;
                    let center = glam::Vec3::new(sx * inv, sy * inv, sz * inv);
                    let Some(path) = self.host_script_waypoint_path_from(&waypoint, center) else {
                        continue;
                    };
                    self.host_script_issue_follow_waypoint_path(
                        &members, &path, exact, as_team, &waypoint,
                    );
                }
            }
        }
    }

    /// C++ `doTeamFollowSkirmishApproachPath` / `doTeamMoveToSkirmishApproachPath`.
    /// Path label is `label + (enemy mpStartIndex + 1)`.
    pub(super) fn apply_host_skirmish_approach_path_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_skirmish_approach_path_requests() {
            let members = self.host_script_team_member_ids(&req.team);
            if members.is_empty() {
                continue;
            }
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            let mut sz = 0.0f32;
            let mut n = 0u32;
            for id in &members {
                if let Some(pos) = self
                    .host_object(*id)
                    .filter(|u| u.is_alive())
                    .map(|u| u.get_position())
                {
                    sx += pos.x;
                    sy += pos.y;
                    sz += pos.z;
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let inv = 1.0 / n as f32;
            let center = glam::Vec3::new(sx * inv, sy * inv, sz * inv);
            let mp_index = self.host_skirmish_enemy_mp_index(&members) + 1;
            let path_label = format!("{}{}", req.path_label, mp_index);
            let Some(path) = self.host_script_waypoint_path_from(&path_label, center) else {
                continue;
            };
            // C++ ScriptActions.cpp:1702-1704 checkBridges(firstUnit, way).
            if let Some(&first) = members.first() {
                if let Some(wid) = self.host_script_closest_waypoint_id(&path_label, center) {
                    let pid = self
                        .host_object(first)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let mut ai_mgr = std::mem::take(&mut self.ai_manager);
                    if let Some(ai) = ai_mgr.ai_players.get_mut(&pid) {
                        let _ = ai.check_bridges(self, first, wid);
                    }
                    self.ai_manager = ai_mgr;
                }
            }
            if req.follow {
                self.host_script_issue_follow_waypoint_path(
                    &members,
                    &path,
                    false,
                    req.as_team,
                    &path_label,
                );
            } else if let Some(&dest) = path.first() {
                for id in members {
                    let _ = self.unit_command_move_to(id, dest);
                }
            }
        }
    }

    /// C++ `TheScriptEngine->getSkirmishEnemyPlayer()->getMpStartIndex()`.
    pub(super) fn host_skirmish_enemy_mp_index(&self, members: &[ObjectId]) -> i32 {
        let owner = members
            .first()
            .and_then(|id| self.host_object(*id))
            .and_then(|obj| obj.owner_player_id);
        for player in self.players.values() {
            if player.is_local && Some(player.id) != owner {
                return player.start_position.max(0);
            }
        }
        for player in self.players.values() {
            if Some(player.id) != owner && player.is_alive && !player.is_observer {
                return player.start_position.max(0);
            }
        }
        0
    }

    /// C++ ScriptActions NAMED/TEAM FACE_NAMED / FACE_WAYPOINT live drain.
    /// Leftover queues [`gamelogic::scripting::HostScriptFaceRequest`].
    pub(super) fn apply_host_face_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptFaceRequest;
        for req in gamelogic::scripting::take_host_script_face_requests() {
            match req {
                HostScriptFaceRequest::NamedFaceNamed { unit, target } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    let Some(pos) = self
                        .host_object(tid)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        continue;
                    };
                    self.host_script_face_unit(id, pos);
                }
                HostScriptFaceRequest::NamedFaceWaypoint { unit, waypoint } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    self.host_script_face_unit(id, pos);
                }
                HostScriptFaceRequest::TeamFaceNamed { team, target } => {
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    let Some(pos) = self
                        .host_object(tid)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_face_unit(id, pos);
                    }
                }
                HostScriptFaceRequest::TeamFaceWaypoint { team, waypoint } => {
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_face_unit(id, pos);
                    }
                }
            }
        }
    }

    /// C++ `clearWaypointQueue` + `leaveGroup` + `chooseLocomotorSet(NORMAL)` +
    /// `aiFacePosition` (`CMD_FROM_SCRIPT`).
    pub(super) fn host_script_face_unit(&mut self, id: ObjectId, pos: glam::Vec3) {
        let _ = self.unit_command_stop(id);
        let _ = self.apply_unit_locomotor_set(id, "normal");
        let _ = self.private_face_position(id, pos);
    }
}
