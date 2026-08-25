//! Team flash, transfer, relations, garrison, guard, attack, and sequential-script actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;
use crate::modules::AIUpdateInterfaceExt;

impl ScriptActionDispatcher {
    pub(crate) fn do_team_flash(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing team '{}' for {}s", team_name, time_in_seconds);
        super::request_host_script_flash(super::HostScriptFlashRequest::Team {
            team: team_name.clone(),
            seconds: time_in_seconds,
            white: false,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
            .and_then(|team| team.read().ok().map(|t| t.get_members().to_vec()))
            .unwrap_or_default();

        for member_id in members {
            self.flash_object_by_id(member_id, time_in_seconds, None);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_flash_white(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!(
            "Flashing team '{}' white for {}s",
            team_name,
            time_in_seconds
        );
        super::request_host_script_flash(super::HostScriptFlashRequest::Team {
            team: team_name.clone(),
            seconds: time_in_seconds,
            white: true,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
            .and_then(|team| team.read().ok().map(|t| t.get_members().to_vec()))
            .unwrap_or_default();

        for member_id in members {
            self.flash_object_by_id(member_id, time_in_seconds, Some(Color::white()));
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTransferTeamToPlayer()
    /// Reassigns the team's controlling player; members stay on the same team.
    pub(crate) fn do_team_transfer_to_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::info!(
            "Transferring team '{}' to player '{}'",
            team_name,
            player_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_transfer(super::HostScriptTransferRequest::Team {
                team: team_name.clone(),
                player: player_name.clone(),
            });
        }

        let Some(target_player) = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
        else {
            log::warn!("Player '{}' not found for team transfer", player_name);
            return Ok(ScriptActionResult::Success);
        };
        let Some(player_id) = target_player
            .read()
            .ok()
            .map(|player| player.get_player_index() as u32)
        else {
            return Ok(ScriptActionResult::Success);
        };

        let Some(team_arc) = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
        else {
            log::warn!("Team '{}' not found for transfer", team_name);
            return Ok(ScriptActionResult::Success);
        };

        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();
        if let Ok(mut team_guard) = team_arc.write() {
            // Team::set_controlling_player_id walks members and calls
            // Object::handle_partition_cell_maintenance, which re-locks the team
            // via get_controlling_player(). Detach first so the owner swap cannot
            // deadlock, then restore membership (C++ never captures the units).
            for object_id in &members {
                team_guard.remove_member(*object_id);
            }
            team_guard.set_controlling_player_id(Some(player_id));
            for object_id in &members {
                team_guard.add_member(*object_id);
            }
        }

        let night_time = global_data::read().time_of_day == global_data::TimeOfDay::Night;
        for object_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            obj_guard.handle_partition_cell_maintenance();
            obj_guard.update_upgrade_modules_from_player();
            let color = if night_time {
                obj_guard.get_night_indicator_color()
            } else {
                obj_guard.get_indicator_color()
            };
            if let Some(drawable) = obj_guard.get_drawable() {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.set_indicator_color(color);
                }
            }
        }

        log::info!(
            "Team '{}' transferred to player '{}'",
            team_name,
            player_name
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let target_team = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Team '{}' override relation to team '{}' ({})",
            team_name,
            target_team,
            relation
        );

        let (team_arc, target_team_id) = if let Ok(mut factory) = get_team_factory().lock() {
            (
                factory.find_team(&team_name),
                factory
                    .find_team(&target_team)
                    .and_then(|team| team.read().ok().map(|team| team.get_id())),
            )
        } else {
            (None, None)
        };
        if let (Some(team_arc), Some(target_team_id)) = (team_arc, target_team_id) {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.set_override_team_relationship(target_team_id, relationship);
            }
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::SetTeam {
                source: team_name,
                dest_team: target_team,
                relationship,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_remove_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let target_team = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Team '{}' remove override relation to team '{}'",
            team_name,
            target_team
        );

        let (team_arc, target_team_id) = if let Ok(mut factory) = get_team_factory().lock() {
            (
                factory.find_team(&team_name),
                factory
                    .find_team(&target_team)
                    .and_then(|team| team.read().ok().map(|team| team.get_id())),
            )
        } else {
            (None, None)
        };
        if let (Some(team_arc), Some(target_team_id)) = (team_arc, target_team_id) {
            if let Ok(mut team_guard) = team_arc.write() {
                let _ = team_guard.remove_override_team_relationship(target_team_id);
            }
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::RemoveTeam {
                source: team_name,
                dest_team: target_team,
            },
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_remove_all_override_relations(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' remove all override relations", team_name);

        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        if let Some(team_arc) = team_arc {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.clear_override_team_relationships();
                team_guard.clear_override_player_relationships();
            }
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::RemoveAll {
                source: team_name,
            },
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_override_relation_to_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Team '{}' override relation to player '{}' ({})",
            team_name,
            player_name,
            relation
        );

        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        let player_index = if let Ok(players) = player_list().read() {
            players
                .find_player_by_name(&player_name)
                .and_then(|player| player.read().ok().map(|player| player.get_player_index()))
        } else {
            None
        };
        if let (Some(team_arc), Some(player_index)) = (team_arc, player_index) {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.set_override_player_relationship(player_index, relationship);
            }
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::SetPlayer {
                source: team_name,
                dest_player: player_name,
                relationship,
            },
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_remove_override_relation_to_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Team '{}' remove override relation to player '{}'",
            team_name,
            player_name
        );

        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        let player_index = if let Ok(players) = player_list().read() {
            players
                .find_player_by_name(&player_name)
                .and_then(|player| player.read().ok().map(|player| player.get_player_index()))
        } else {
            None
        };
        if let (Some(team_arc), Some(player_index)) = (team_arc, player_index) {
            if let Ok(mut team_guard) = team_arc.write() {
                let _ = team_guard.remove_override_player_relationship(player_index);
            }
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::RemovePlayer {
                source: team_name,
                dest_player: player_name,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamLoadTransports() / PartitionSolver
    pub(crate) fn do_team_load_transports(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::info!("Team '{}' loading transports", team_name);
        // Live leftover Team members are host IDs, but leftover TheGameLogic
        // lookup is empty on the player path. Queue leftover BinPartitionSolver
        // + aiEnter for the live host drain.
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_load_transports(&team_name);
            return Ok(ScriptActionResult::Success);
        }
        let team_arc = self.get_team_by_name(&team_name)?;
        let members = team_arc
            .read()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();

        let mut units = game_engine::common::partition_solver::EntriesVec::new();
        let mut transports = game_engine::common::partition_solver::SpacesVec::new();

        for member_id in members {
            let Some(obj) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if guard.is_kind_of(crate::common::KindOf::Transport) {
                let capacity = match guard.get_contain() {
                    Some(contain) => contain
                        .lock()
                        .ok()
                        .map(|c| c.get_contain_max().max(0) as u32)
                        .unwrap_or(0),
                    None => 0,
                };
                transports.push((member_id, capacity));
            } else {
                let slots = guard.get_transport_slot_count() as u32;
                units.push((member_id, slots));
            }
        }

        let mut solver = game_engine::common::partition_solver::BinPartitionSolver::new(
            units,
            transports,
            game_engine::common::partition_solver::SolutionType::PreferFastSolution,
        );
        solver.solve();

        for (unit_id, transport_id) in solver.get_solution() {
            let Some(unit) = TheGameLogic::find_object_by_id(*unit_id) else {
                continue;
            };
            if let Ok(unit_guard) = unit.read() {
                if let Some(ai) = unit_guard.get_ai_update_interface() {
                    ai.ai_enter(*transport_id, crate::ai::CommandSourceType::FromScript);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamEnterNamed()
    /// Team enters a specific named object (building/transport)
    pub(crate) fn do_team_enter_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' entering '{}'", team_name, target_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::TeamEnter {
                    team: team_name,
                    dest: target_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Get the target object ID
        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&target_name).ok().flatten();

        if let Some(tid) = target_id {
            // Create group first, then use it
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(tid);
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Target '{}' not found for team enter", target_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamExitAll()
    /// All team members exit from containers/transports
    pub(crate) fn do_team_exit_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' exiting all", team_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::TeamExitAll {
                    team: team_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let write_result = group_arc.write();
        if let Ok(mut group) = write_result {
            let params =
                AiCommandParams::new(AiCommandType::Evacuate, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGarrisonSpecificBuilding()
    /// Team garrisons a specific named building
    pub(crate) fn do_team_garrison_specific_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let building_name = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' garrisoning building '{}'",
            team_name,
            building_name
        );
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::TeamGarrisonSpecific {
                    team: team_name,
                    building: building_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let team_player_mask = self
            .get_team_by_name(&team_name)
            .ok()
            .and_then(|team| team.read().ok().and_then(|t| t.get_controlling_player_id()))
            .and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as i32).cloned())
            })
            .and_then(|player| player.read().ok().map(|p| p.get_player_mask()))
            .unwrap_or_else(crate::common::PlayerMaskType::none);

        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&building_name).ok().flatten();

        if let Some(tid) = target_id {
            let Some(building_obj) = TheGameLogic::find_object_by_id(tid) else {
                return Ok(ScriptActionResult::Success);
            };
            let can_garrison = if let Ok(building_guard) = building_obj.read() {
                if !building_guard.is_kind_of(crate::common::KindOf::Structure) {
                    false
                } else if let Some(contain) = building_guard.get_contain() {
                    let entered_mask = contain
                        .lock()
                        .ok()
                        .map(|c| c.get_player_who_entered())
                        .unwrap_or_else(crate::common::PlayerMaskType::none);
                    entered_mask == crate::common::PlayerMaskType::none()
                        || entered_mask == team_player_mask
                } else {
                    false
                }
            } else {
                false
            };
            if !can_garrison {
                return Ok(ScriptActionResult::Success);
            }

            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(tid);
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Building '{}' not found for team garrison", building_name);
        }

        Ok(ScriptActionResult::Success)
    }

    // TEAM_GARRISON_NEAREST_BUILDING lives in `actions_garrison.rs`.

    /// C++ Reference: ScriptActions::doTeamExitAllBuildings()
    /// Team exits from all garrisoned buildings
    pub(crate) fn do_team_exit_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::info!("Team '{}' exiting all buildings", team_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::TeamExitAllBuildings {
                    team: team_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let Some(team_arc) = self.get_team_by_name(&team_name).ok() else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();

        for member_id in members {
            let Some(member_obj) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            if let Ok(mut member_guard) = member_obj.write() {
                let Some(ai_arc) = member_guard.get_ai_update_interface() else {
                    continue;
                };
                member_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromScript);
                    let _ = ai_guard.execute_command(&params);
                };
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGuardPosition()
    /// Team guards at a specified waypoint position
    pub(crate) fn do_team_guard_position(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' guarding position at '{}'",
            team_name,
            waypoint_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_guard_variant(
                super::HostScriptGuardVariantRequest::TeamGuardPosition {
                    team: self.resolve_team_name_token(&team_name),
                    waypoint: waypoint_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Get waypoint position
        let waypoint_name_ascii = AsciiString::from(waypoint_name.as_str());
        let waypoint_pos = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_name_ascii)
                .map(|w| w.get_location().clone())
        });

        if let Some(position) = waypoint_pos {
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params = AiCommandParams::new(
                    AiCommandType::GuardPosition,
                    CommandSourceType::FromScript,
                );
                params.pos = position;
                params.int_value = 0; // GUARDMODE_NORMAL
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!(
                "Waypoint '{}' not found for team guard position",
                waypoint_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGuardObject()
    /// Team guards a specific named object
    pub(crate) fn do_team_guard_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_name = self.get_string_param(action, 1)?;
        log::debug!("Team '{}' guarding object '{}'", team_name, object_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_guard_variant(
                super::HostScriptGuardVariantRequest::TeamGuardObject {
                    team: self.resolve_team_name_token(&team_name),
                    unit: object_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Get the object ID from name tracker
        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&object_name).ok().flatten();

        if let Some(tid) = target_id {
            if TheGameLogic::find_object_by_id(tid).is_none() {
                log::warn!("Object '{}' object {} no longer exists", object_name, tid);
                return Ok(ScriptActionResult::Success);
            }

            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params =
                    AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
                params.obj = Some(tid);
                params.int_value = GuardMode::Normal.as_i32();
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Object '{}' not found for team guard", object_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_guard_area(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let area_name = self.get_string_param(action, 1)?;
        log::debug!("Team '{}' guarding area '{}'", team_name, area_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_guard_variant(
                super::HostScriptGuardVariantRequest::TeamGuardArea {
                    team: team_name,
                    area: area_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let (area_center, trigger_id) = match self.get_trigger_area(&area_name) {
            Ok(trigger) => (trigger.get_center_point(), trigger.get_id()),
            Err(_) => {
                log::warn!("Trigger area '{}' not found for guard", area_name);
                return Ok(ScriptActionResult::Success);
            }
        };

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardArea, CommandSourceType::FromScript);
            params.pos = area_center;
            params.polygon = Some(trigger_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_guard_supply_center(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // C++ ScriptActions::doGuardSupplyCenter:
        // Team *team = getTeamNamed; Player *player = team->getControllingPlayer();
        // player->guardSupplyCenter(team, supplies).
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let min_supplies = self.get_int_param(action, 1)?;
        log::debug!(
            "Team '{}' guarding supply center with >= {} supplies",
            team_name,
            min_supplies
        );

        if dual_world_registry_unavailable() {
            // Live host never populates leftover OBJECT_REGISTRY / leftover AI.
            super::request_host_guard_supply_center(&team_name, min_supplies);
            return Ok(ScriptActionResult::Success);
        }

        let Ok(team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let controlling_player_id = team_arc
            .read()
            .ok()
            .and_then(|team| team.get_controlling_player_id());
        let Some(player_id) = controlling_player_id else {
            return Ok(ScriptActionResult::Success);
        };
        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                let _ = ai_player.guard_supply_center(&team_name, min_supplies);
            })
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_guard_in_tunnel_network(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' guarding in tunnel network", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_guard_variant(
                super::HostScriptGuardVariantRequest::TeamGuardTunnel { team: team_name },
            );
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    let ai_arc = obj_arc
                        .read()
                        .ok()
                        .and_then(|obj| obj.get_ai_update_interface());
                    let Some(ai_arc) = ai_arc else {
                        continue;
                    };
                    if let Ok(mut ai_guard) = ai_arc.lock() {
                        let mut params = AiCommandParams::new(
                            AiCommandType::GuardTunnelNetwork,
                            CommandSourceType::FromScript,
                        );
                        params.int_value = GuardMode::Normal.as_i32();
                        let _ = ai_guard.execute_command(&params);
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    #[allow(dead_code)]
    pub(crate) fn do_team_guard_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Team '{}' guarding for {} frames", team_name, frames);

        // Unused C++ helper is not dispatched, but leftover-queue if called.
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::TeamGuard {
                team: team_name.clone(),
            });
        }

        // C++ parity: issue guard-at-current-position to each member.
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    for &member_id in team.get_members() {
                        let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            continue;
                        };
                        let Ok(obj) = obj_arc.read() else {
                            continue;
                        };
                        let pos = *obj.get_position();
                        let Some(ai_arc) = obj.get_ai_update_interface() else {
                            continue;
                        };
                        let mut guard_params = AiCommandParams::new(
                            AiCommandType::GuardPosition,
                            CommandSourceType::FromScript,
                        );
                        guard_params.pos = pos;
                        if let Ok(mut ai) = ai_arc.lock() {
                            let _ = ai.execute_command(&guard_params);
                        };
                    }
                }
            }
        }

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn do_team_idle_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Team '{}' idling for {} frames", team_name, frames);

        // Live host objects are not in leftover OBJECT_REGISTRY. Queue
        // C++ doTeamIdleForFramecount: groupIdle(CMD_FROM_SCRIPT) + sequential timer.
        // TEAM_GUARD_FOR_FRAMECOUNT also dispatches here (C++ executeAction).
        if super::dual_world_registry_unavailable() {
            super::request_host_script_idle(super::HostScriptIdleRequest::TeamStop {
                team: team_name.clone(),
                disband: false,
            });
        }

        // C++ parity: idle the team through an AI group.
        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let params =
                    AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                let _ = group.ai_do_command(&params);
            }
        }

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn do_team_spin_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Team '{}' spinning for {} frames", team_name, frames);

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn do_team_increase_priority(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(priority) = factory.increase_team_prototype_priority_for_success(&team_name)
            {
                log::debug!(
                    "Increased production priority for team '{}' to {}",
                    team_name,
                    priority
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_decrease_priority(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(priority) = factory.decrease_team_prototype_priority_for_failure(&team_name)
            {
                log::debug!(
                    "Decreased production priority for team '{}' to {}",
                    team_name,
                    priority
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFollowWaypointsExact()
    /// Team follows waypoints in exact formation (no pathfinding deviation)
    pub(crate) fn do_team_follow_waypoints_exact(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path = self.get_string_param(action, 1)?;
        // C++ ScriptActions.cpp:1814 doTeamFollowWaypointsExact(..., Bool asTeam)
        let as_team = self.get_int_param(action, 2).unwrap_or(1) != 0;
        log::info!(
            "Team '{}' following waypoints exact '{}' as_team={}",
            team_name,
            waypoint_path,
            as_team
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_follow_waypoints(
                super::HostScriptFollowWaypointsRequest::TeamFollow {
                    team: self.resolve_team_name_token(&team_name),
                    waypoint: waypoint_path,
                    as_team,
                    exact: true,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some(team_center) = self
            .compute_team_center_and_first(&team_arc)
            .map(|(center, _)| center)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let waypoint_id = self.resolve_follow_waypoint_id(&waypoint_path, team_center);

        if let Some(wid) = waypoint_id {
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let cmd = if as_team {
                    AiCommandType::FollowWaypointPathAsTeamExact
                } else {
                    AiCommandType::FollowWaypointPathExact
                };
                let mut params = AiCommandParams::new(cmd, CommandSourceType::FromScript);
                params.waypoint = Some(wid);
                let _ = group.ai_do_command(&params);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamAttackArea()
    /// Team attacks at a trigger area
    pub(crate) fn do_team_attack_area(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let area_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' attacking area '{}'", team_name, area_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::TeamAttackArea {
                    team: self.resolve_team_name_token(&team_name),
                    area: area_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let (area_center, trigger_id) = match self.get_trigger_area(&area_name) {
            Ok(trigger) => (trigger.get_center_point(), trigger.get_id()),
            Err(_) => {
                log::warn!("Trigger area '{}' not found", area_name);
                return Ok(ScriptActionResult::Success);
            }
        };

        // Issue AttackArea command to team AI group
        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let write_result = group_arc.write();
        if let Ok(mut group) = write_result {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackArea, CommandSourceType::FromScript);
            params.pos = area_center;
            params.polygon = Some(trigger_id);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamAttackNamed()
    /// Team attacks a specific named object
    pub(crate) fn do_team_attack_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' attacking '{}'", team_name, target_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::TeamAttackNamed {
                    team: self.resolve_team_name_token(&team_name),
                    unit: target_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Get the object ID from name tracker
        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&target_name).ok().flatten();

        if let Some(tid) = target_id {
            if TheGameLogic::find_object_by_id(tid).is_none() {
                log::warn!("Target '{}' object {} no longer exists", target_name, tid);
                return Ok(ScriptActionResult::Success);
            }

            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params = AiCommandParams::new(
                    AiCommandType::AttackObject,
                    CommandSourceType::FromScript,
                );
                params.obj = Some(tid);
                params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Target '{}' not found for team attack", target_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamApplyAttackPrioritySet()
    pub(crate) fn do_team_apply_attack_priority_set(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let priority_set = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' applying attack priority set '{}'",
            team_name,
            priority_set
        );

        let info_name = with_script_engine_ref(|engine| {
            engine
                .get_attack_info(&priority_set)
                .map(|info| info.get_name().to_string())
        })
        .flatten()
        .unwrap_or_default();

        let mut prototype_updated = false;
        let mut team_members = Vec::new();
        if let Ok(mut factory) = get_team_factory().lock() {
            prototype_updated =
                factory.set_team_prototype_attack_priority_name(&team_name, info_name.as_str());
            if !prototype_updated {
                if let Some(team_arc) = factory.find_team(&team_name) {
                    if let Ok(team) = team_arc.read() {
                        team_members = team.get_members().to_vec();
                    }
                }
            }
        }

        if !prototype_updated {
            if team_members.is_empty() {
                log::debug!(
                    "Team '{}' has no prototype and no live members for attack priority set '{}'",
                    team_name,
                    info_name
                );
            } else {
                let _ = with_script_engine_mut(|engine| {
                    for member_id in team_members {
                        if info_name.is_empty() {
                            engine.clear_object_attack_priority_set(member_id);
                        } else {
                            engine.set_object_attack_priority_set(member_id, info_name.as_str());
                        }
                    }
                });
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamSetAttitude()
    /// Set team's combat attitude (Aggressive, Normal, Defensive, Passive)
    pub(crate) fn do_team_set_attitude(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let mood = self.get_int_param(action, 1)?;
        let attitude = self.group_attitude_from_script_int(mood);
        let module_attitude = self.attitude_from_script_int(mood);
        log::info!("Team '{}' setting attitude to {:?}", team_name, attitude);

        if super::dual_world_registry_unavailable() {
            super::request_host_team_attitude(&team_name, mood);
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let _ = group.set_attitude(attitude);
            }
        }

        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let object_manager = get_object_manager();
                    if let Ok(obj_manager) = object_manager.read() {
                        for obj_id in team.get_members() {
                            if let Some(obj) = obj_manager.get_object(*obj_id) {
                                if let Ok(obj_read) = obj.read() {
                                    if let Some(ai) = obj_read.get_ai_update_interface() {
                                        if let Ok(mut ai_write) = ai.lock() {
                                            let _ = ai_write.set_attitude(module_attitude);
                                        }
                                    }
                                }
                            }
                        }
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_execute_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let script_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' executing sequential script '{}'",
            team_name,
            script_name
        );

        let Ok(_team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };

        // C++ resolves the script before it idles the team.  Take an owned
        // clone through the lexically active engine, then leave engine state
        // unlocked while issuing the AI command.
        let Some(script) =
            with_script_engine_ref(|engine| engine.find_script_clone_by_name(&script_name))
                .flatten()
        else {
            return Ok(ScriptActionResult::Success);
        };

        // C++ parity: idle team before queueing sequential script.
        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let params =
                    AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                let _ = group.ai_do_command(&params);
            }
        }

        let _ = with_script_engine_mut(|engine| {
            let mut seq_script = crate::scripting::engine::SequentialScript::new();
            seq_script.team_to_exec_on = Some(team_name.clone());
            seq_script.object_id = INVALID_ID;
            seq_script.script_to_execute_sequentially = Some(Box::new(script));
            seq_script.times_to_loop = 0;
            engine.append_sequential_script(seq_script);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_execute_sequential_script_looping(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let script_name = self.get_string_param(action, 1)?;
        let loop_val = self.get_int_param(action, 2)? - 1;
        log::debug!(
            "Team '{}' executing sequential script '{}' looping ({})",
            team_name,
            script_name,
            loop_val
        );

        let Ok(_team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };

        // Preserve C++ lookup-before-idle order without holding an engine
        // lock across the AI command.
        let Some(script) =
            with_script_engine_ref(|engine| engine.find_script_clone_by_name(&script_name))
                .flatten()
        else {
            return Ok(ScriptActionResult::Success);
        };

        // C++ parity: idle team before queueing sequential script.
        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let params =
                    AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                let _ = group.ai_do_command(&params);
            }
        }

        let _ = with_script_engine_mut(|engine| {
            let mut seq_script = crate::scripting::engine::SequentialScript::new();
            seq_script.team_to_exec_on = Some(team_name.clone());
            seq_script.object_id = INVALID_ID;
            seq_script.script_to_execute_sequentially = Some(Box::new(script));
            seq_script.times_to_loop = loop_val;
            engine.append_sequential_script(seq_script);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_stop_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' stopping sequential script", team_name);

        let Ok(_team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let _ = with_script_engine_mut(|engine| {
            engine.remove_all_sequential_scripts_for_team(&team_name);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_emoticon(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let emoticon = self.get_string_param(action, 1)?;
        let duration_seconds = self.get_real_param(action, 2)?;
        let duration_frames = (duration_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        log::debug!(
            "Team '{}' setting emoticon '{}' for {}s ({}f)",
            team_name,
            emoticon,
            duration_seconds,
            duration_frames
        );
        super::request_host_script_emoticon(super::HostScriptEmoticonRequest::Team {
            team: team_name.clone(),
            emoticon: emoticon.clone(),
            duration_frames,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();
        for object_id in members {
            self.emoticon_object_by_id(object_id, &emoticon, duration_frames);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_stealth_enabled(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Team '{}' stealth enabled: {}", team_name, enabled);

        let team_name = self.resolve_team_name_token(&team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_stealth_enabled(
                super::HostScriptStealthEnabledRequest::Team {
                    team: team_name,
                    enabled,
                },
            );
            return Ok(ScriptActionResult::Success);
        }
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.set_script_status(
                            crate::object::ObjectScriptStatusBit::ScriptUnstealthed,
                            !enabled,
                        );
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_repulsor(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Team '{}' repulsor: {}", team_name, enabled);

        let team_name = self.resolve_team_name_token(&team_name);
        super::request_host_script_repulsor(super::HostScriptRepulsorRequest::Team {
            team: team_name.clone(),
            enabled,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard
                            .set_status(crate::common::ObjectStatusMaskType::REPULSOR, enabled);
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_create_radar_event(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let event_type = self.get_int_param(action, 1)?;
        log::debug!(
            "Creating radar event for team '{}' (type {})",
            team_name,
            event_type
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_radar_event(super::HostScriptRadarEventRequest::Team {
                team: team_name,
                event_type,
            });
            return Ok(ScriptActionResult::Success);
        }
        let team_arc = self.get_team_by_name(&team_name)?;
        let pos = {
            let Ok(team) = team_arc.read() else {
                return Ok(ScriptActionResult::Success);
            };
            if !team.has_any_units() {
                return Ok(ScriptActionResult::Success);
            }
            team.get_estimate_team_position()
        };
        let Some(pos) = pos else {
            return Ok(ScriptActionResult::Success);
        };

        let radar_event = Self::radar_event_type_from_int(event_type);
        if let Ok(mut radar) = get_radar_system().write() {
            let radar_pos = to_radar_coord(&pos);
            radar.create_event(&radar_pos, radar_event, 4.0);
        }
        // The host callback can execute nested script/UI work.  Clone it
        // first so neither the team nor ScriptEngine lock spans that call.
        let handler =
            with_script_engine_ref(|script_engine| script_engine.action_handler()).flatten();
        if let Some(handler) = handler {
            if let Err(err) = handler.create_radar_event(pos.x, pos.y, pos.z, event_type) {
                log::warn!("Script action handler create_radar_event failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_delete_living(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::debug!("Deleting living members of team '{}'", team_name);

        // C++ parity: TEAM_DELETE_LIVING -> doTeamDelete(team, TRUE).
        let team_name = self.resolve_team_name_token(&team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::TeamDelete {
                    team: team_name,
                    ignore_dead: true,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.delete_team(true);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_wait_for_not_contained_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' waiting for not contained (all)", team_name);
        let all_contained = self.evaluate_team_is_contained(&team_name, true);
        if all_contained {
            Ok(ScriptActionResult::Pending(1.0))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn do_team_wait_for_not_contained_partial(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' waiting for not contained (partial)", team_name);
        let any_contained = self.evaluate_team_is_contained(&team_name, false);
        if any_contained {
            Ok(ScriptActionResult::Pending(1.0))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn evaluate_team_is_contained(&self, team_name: &str, all_contained: bool) -> bool {
        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(team_name))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();
        if members.is_empty() {
            if crate::scripting::host_script_query_has_any() {
                return crate::scripting::host_eval_team_is_contained(team_name, all_contained);
            }
            return false;
        }

        let mut any_considered = false;
        for member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            let mut is_contained = obj.get_contained_by().is_some();
            if !is_contained {
                if let Some(ai_arc) = obj.get_ai_update_interface() {
                    if let Ok(ai) = ai_arc.lock() {
                        is_contained = ai.get_current_state_id()
                            == Some(crate::ai::states::AIStateType::Exit as u32);
                    }
                }
            }

            if is_contained {
                if !all_contained {
                    return true;
                }
            } else if all_contained {
                return false;
            }

            any_considered = true;
        }

        if any_considered {
            return all_contained;
        }

        // Leftover TeamFactory members are live host ids, but leftover
        // OBJECT_REGISTRY is empty so find_object_by_id misses. Use the
        // host snapshot census (contained_by / ai_exiting).
        if crate::scripting::host_script_query_has_any() {
            return crate::scripting::host_eval_team_is_contained(team_name, all_contained);
        }
        false
    }

    pub(crate) fn do_team_move_towards_nearest_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let trigger_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' moving towards nearest '{}' in trigger '{}'",
            team_name,
            object_type,
            trigger_name
        );

        // Leftover partition / leftover crate objects are empty on the player path.
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::TeamMoveTowardsNearest {
                    team: self.resolve_team_name_token(&team_name),
                    object_type: object_type.clone(),
                    trigger: trigger_name.clone(),
                },
            );
        }

        let team_name = self.resolve_team_name_token(&team_name);
        let (members, estimate_team_pos) = if let Ok(mut factory_guard) = get_team_factory().lock()
        {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(team_guard) = team_arc.read() {
                    (
                        team_guard.get_members().to_vec(),
                        team_guard.get_estimate_team_position(),
                    )
                } else {
                    (Vec::new(), None)
                }
            } else {
                (Vec::new(), None)
            }
        } else {
            (Vec::new(), None)
        };
        if members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let mut source_object_id = INVALID_ID;
        let mut source_off_map = false;
        let mut source_pos = estimate_team_pos.unwrap_or(Coord3D::new(0.0, 0.0, 0.0));
        for &member_id in &members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.get_ai_update_interface().is_some() {
                source_object_id = member_id;
                source_off_map = obj.is_off_map();
                if estimate_team_pos.is_none() {
                    source_pos = *obj.get_position();
                }
                break;
            }
        }
        if source_object_id == INVALID_ID {
            return Ok(ScriptActionResult::Success);
        }

        let Some(target_id) = self.find_closest_object_of_type_in_trigger(
            source_object_id,
            &source_pos,
            source_off_map,
            &object_type,
            &trigger_name,
        ) else {
            return Ok(ScriptActionResult::Success);
        };

        for &member_id in &members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let ai_arc = {
                let Ok(obj) = obj_arc.read() else {
                    continue;
                };
                let Some(ai_arc) = obj.get_ai_update_interface() else {
                    continue;
                };
                ai_arc
            };
            if let Ok(mut ai) = ai_arc.lock() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params = AiCommandParams::new(
                    AiCommandType::MoveToObject,
                    CommandSourceType::FromScript,
                );
                params.obj = Some(target_id);
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }
}
