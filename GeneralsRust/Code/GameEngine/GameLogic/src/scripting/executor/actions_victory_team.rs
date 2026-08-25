//! Victory/defeat, basic team, and basic named-object script actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptActionDispatcher {
    // ============================================================================
    // VICTORY/DEFEAT ACTIONS
    // C++ Reference: ScriptActions.cpp lines 215-276
    // ============================================================================

    /// C++ Reference: ScriptActions::doVictory() ScriptActions.cpp:191-210
    pub(crate) fn do_victory(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("VICTORY!");

        {
            let mut ctx = self.context.write().unwrap();
            ctx.suppress_new_windows = false;
        }

        // C++ ScriptActions.cpp:193-209: closeWindows, TheGameLogic->closeWindows,
        // doDisableInput, winCreateFromScript(Victorious/ObserverQuit),
        // SetVictorious(TRUE), startEndGameTimer.
        let _ = with_script_engine_mut(|engine| {
            engine.close_windows(false);
            engine.close_game_windows();
        });
        self.do_disable_input()?;
        let _ = with_script_engine_mut(|engine| {
            let layout = if engine.should_show_observer_quit_window() {
                "Menus/ObserverQuit.wnd"
            } else {
                "Menus/Victorious.wnd"
            };
            engine.create_win_lose_window(layout);
            engine.set_campaign_victorious(true);
            engine.start_end_game_timer();
        });

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doQuickVictory() ScriptActions.cpp:169-177
    pub(crate) fn do_quick_victory(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("QUICK VICTORY!");

        {
            let mut ctx = self.context.write().unwrap();
            ctx.suppress_new_windows = false;
        }

        // C++ ScriptActions.cpp:171-176: closeWindows + GameLogic::closeWindows,
        // doDisableInput, SetVictorious, startQuickEndGameTimer. No new window.
        let _ = with_script_engine_mut(|engine| {
            engine.close_windows(false);
            engine.close_game_windows();
        });
        self.do_disable_input()?;
        let _ = with_script_engine_mut(|engine| {
            engine.set_campaign_victorious(true);
            engine.start_quick_end_game_timer();
        });

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doDefeat() ScriptActions.cpp:215-234
    pub(crate) fn do_defeat(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("DEFEAT!");

        {
            let mut ctx = self.context.write().unwrap();
            ctx.suppress_new_windows = false;
        }

        // C++ ScriptActions.cpp:217-233: closeWindows, GameLogic::closeWindows,
        // doDisableInput, winCreateFromScript(Defeat/ObserverQuit),
        // SetVictorious(FALSE), startEndGameTimer.
        let _ = with_script_engine_mut(|engine| {
            engine.close_windows(false);
            engine.close_game_windows();
        });
        self.do_disable_input()?;
        let _ = with_script_engine_mut(|engine| {
            let layout = if engine.should_show_observer_quit_window() {
                "Menus/ObserverQuit.wnd"
            } else {
                "Menus/Defeat.wnd"
            };
            engine.create_win_lose_window(layout);
            engine.set_campaign_victorious(false);
            engine.start_end_game_timer();
        });

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doLocalDefeat() ScriptActions.cpp:239-252
    pub(crate) fn do_local_defeat(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("LOCAL DEFEAT (multiplayer)");

        // C++ ScriptActions.cpp:241-251: markMPLocalDefeatWindowShown,
        // closeWindows, GameLogic::closeWindows, LocalDefeat.wnd when not
        // observer, SetVictorious(FALSE), startCloseWindowTimer.
        let _ = with_script_engine_mut(|engine| {
            engine.set_shown_mp_local_defeat_window(true);
            engine.close_windows(false);
            engine.close_game_windows();
            if engine.should_show_local_defeat_window() {
                engine.create_win_lose_window("Menus/LocalDefeat.wnd");
            }
            engine.set_campaign_victorious(false);
            engine.start_close_window_timer();
        });

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // TEAM ACTIONS
    // C++ Reference: ScriptActions.cpp lines 413-435 (move team)
    // ============================================================================

    /// C++ Reference: ScriptActions::doMoveToWaypoint() line 413
    pub(crate) fn do_move_team_to(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let waypoint_name = self.get_string_param(action, 1)?;

        log::info!(
            "Moving team '{}' to waypoint '{}'",
            team_name,
            waypoint_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(super::HostScriptMoveAttackRequest::TeamMove {
                team: team_name,
                waypoint: waypoint_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let destination = self.get_waypoint_position(&waypoint_name)?;
        let group_arc = self.create_ai_group_from_team(&team_name)?;

        if let Ok(group) = group_arc.read() {
            group.group_move_to_position(&destination, false, CommandSourceType::FromScript);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamAttackNamed() line (in header)
    /// C++ Reference: ScriptActions::doTeamAttackTeam()
    /// Creates AI group from attacker team and issues attack command on victim team
    pub(crate) fn do_team_attack_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let attacker_team = self.get_string_param(action, 0)?;
        let victim_team = self.get_string_param(action, 1)?;

        log::info!("Team '{}' attacking team '{}'", attacker_team, victim_team);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::TeamAttackTeam {
                    attacker: self.resolve_team_name_token(&attacker_team),
                    victim: self.resolve_team_name_token(&victim_team),
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let victim_team = self.resolve_team_name_token(&victim_team);
        if self.get_team_by_name(&victim_team).is_err() {
            log::warn!("Victim team '{}' not found for team attack", victim_team);
            return Ok(ScriptActionResult::Success);
        }

        // Create AI group from attacker team
        let group_arc = self.create_ai_group_from_team(&attacker_team)?;

        // Issue attack command to group targeting victim team
        // C++: aiGroup->groupAttackTeam(victimTeam, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT)
        if let Ok(mut group) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackTeam, CommandSourceType::FromScript);
            params.team = Some(victim_team);
            params.int_value = -1; // NO_MAX_SHOTS_LIMIT
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamHunt() lines 1985-1999
    /// Creates AI group from team and issues hunt command
    pub(crate) fn do_team_hunt(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);

        log::info!("Team '{}' hunting", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::TeamHunt {
                team: team_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        // Create AI group from team and issue hunt command
        // C++: theGroup->groupHunt(CMD_FROM_SCRIPT)
        let group_arc = self.create_ai_group_from_team(&team_name)?;

        if let Ok(mut group) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Hunt, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGuard() lines 1882-1900
    /// Orders team members to guard at their current positions
    pub(crate) fn do_team_guard(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' guarding at current positions", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::TeamGuard {
                team: team_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = team_arc
            .read()
            .map_err(|e| ScriptError::ExecutionFailed(format!("Failed to read team: {}", e)))?
            .get_members()
            .to_vec();

        for object_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            let position = *obj.get_position();
            let Some(ai_arc) = obj.get_ai_update_interface() else {
                continue;
            };
            drop(obj);

            if let Ok(mut ai) = ai_arc.lock() {
                let mut params = AiCommandParams::new(
                    AiCommandType::GuardPosition,
                    CommandSourceType::FromScript,
                );
                params.pos = position;
                params.int_value = GuardMode::Normal.as_i32();
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamDelete() line (in header)
    pub(crate) fn do_team_delete(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);

        log::info!("Deleting team '{}'", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::TeamDelete {
                    team: team_name,
                    ignore_dead: false,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // C++ parity: TeamDelete delegates to Team::deleteTeam(ignoreDead=false).
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.delete_team(false);
                    log::info!("Team '{}' deleted successfully", team_name);
                }
            } else {
                log::warn!("Team '{}' not found for deletion", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamKill() line (in header)
    pub(crate) fn do_team_kill(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);

        log::info!("Killing team '{}'", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::TeamKill { team: team_name },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Get team by name and kill all members
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    // Kill all team members (with death effects)
                    team_guard.kill_team();
                    log::info!("Team '{}' killed successfully", team_name);
                }
            } else {
                log::warn!("Team '{}' not found for kill", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doDamageTeamMembers() / Team::damageTeamMembers()
    pub(crate) fn do_damage_team_members(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let damage_amount = self.get_real_param(action, 1)?;

        log::info!("Damaging team '{}' for {} points", team_name, damage_amount);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::TeamDamage {
                    team: team_name,
                    amount: damage_amount,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
            .and_then(|team| team.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();
        if members.is_empty() {
            log::warn!("Team '{}' not found for damage", team_name);
            return Ok(ScriptActionResult::Success);
        }

        for object_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                continue;
            }
            if damage_amount < 0.0 {
                obj_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
            } else {
                let mut damage_info = DamageInfo::with_simple(
                    damage_amount,
                    INVALID_ID,
                    DamageType::Unresistable,
                    DeathType::Normal,
                );
                let _ = obj_guard.attempt_damage(&mut damage_info);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetTeamState() line 492
    pub(crate) fn do_set_team_state(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let state_name = self.get_string_param(action, 1)?;

        log::info!("Setting team '{}' state to '{}'", team_name, state_name);

        // Get team by name and set its state
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.set_state(state_name.clone().into());
                    log::info!("Team '{}' state set to '{}'", team_name, state_name);
                }
            } else {
                log::warn!("Team '{}' not found for state change", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFollowWaypoints() line (in header)
    /// Creates AI group from team and issues follow waypoint path command
    pub(crate) fn do_team_follow_waypoints(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let waypoint_path_name = self.get_string_param(action, 1)?;
        // C++ ScriptActions.cpp:1767 doTeamFollowWaypoints(..., Bool asTeam)
        let as_team = self.get_int_param(action, 2).unwrap_or(1) != 0;

        log::debug!(
            "Team '{}' following waypoint path '{}' as_team={}",
            team_name,
            waypoint_path_name,
            as_team
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_follow_waypoints(
                super::HostScriptFollowWaypointsRequest::TeamFollow {
                    team: team_name,
                    waypoint: waypoint_path_name,
                    as_team,
                    exact: false,
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
        let waypoint_id = self.resolve_follow_waypoint_id(&waypoint_path_name, team_center);

        if let Some(wid) = waypoint_id {
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            {
                if let Ok(mut group) = group_arc.write() {
                    let cmd = if as_team {
                        AiCommandType::FollowWaypointPathAsTeam
                    } else {
                        AiCommandType::FollowWaypointPath
                    };
                    let mut params = AiCommandParams::new(cmd, CommandSourceType::FromScript);
                    params.waypoint = Some(wid);
                    let _ = group.ai_do_command(&params);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // UNIT CREATION/DELETION ACTIONS
    // C++ Reference: ScriptActions.cpp line (create object)
    // ============================================================================

    /// C++ Reference: ScriptActions::doCreateObject() line (in header)
    pub(crate) fn do_create_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // C++ Reference: ScriptActions.cpp switch case ScriptAction::CREATE_OBJECT
        // Parameters:
        //  0: object type (template name)
        //  1: team name
        //  2: coord3d position
        //  3: angle
        let object_type = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let position = self.get_coord_param(action, 2)?;
        let position = crate::common::Coord3D::new(position.x, position.y, position.z);
        let angle = self.get_real_param(action, 3)?;

        log::info!(
            "Creating object of type '{}' on team '{}' at ({}, {}, {}) angle {}",
            object_type,
            team_name,
            position.x,
            position.y,
            position.z,
            angle
        );

        if super::dual_world_registry_unavailable() {
            super::request_host_script_create(super::HostScriptCreateRequest::Object {
                name: None,
                thing: object_type,
                team: team_name,
                x: position.x,
                y: position.y,
                z: position.z,
                angle,
            });
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = if team_name.trim().is_empty() {
            None
        } else {
            self.get_or_create_team_by_name(&team_name).ok()
        };

        let object_id = {
            let manager_arc = get_object_manager();
            let Ok(mut manager) = manager_arc.write() else {
                log::warn!("CREATE_OBJECT: failed to lock ObjectManager");
                return Ok(ScriptActionResult::Success);
            };

            match manager.create_object(
                &object_type,
                position,
                team_arc.clone(),
                crate::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(err) => {
                    log::warn!(
                        "CREATE_OBJECT: failed to create '{}' on team '{}': {}",
                        object_type,
                        team_name,
                        err
                    );
                    return Ok(ScriptActionResult::Success);
                }
            }
        };

        if let Some(team_arc) = &team_arc {
            if let Ok(mut team) = team_arc.write() {
                team.add_member(object_id);
            }
        }

        if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut obj) = obj_arc.write() {
                let _ = obj.set_orientation(angle);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_create_named_on_team_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let team_name = self.get_string_param(action, 2)?;
        let waypoint_name = self.get_string_param(action, 3)?;

        log::debug!(
            "Creating named unit '{}' of type '{}' on team '{}' at waypoint '{}'",
            unit_name,
            object_type,
            team_name,
            waypoint_name
        );

        let _ = self.create_unit_on_team_at_waypoint(
            Some(&unit_name),
            &object_type,
            &team_name,
            &waypoint_name,
        )?;

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doNamedDelete() line (in header)
    pub(crate) fn do_named_delete(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Deleting named unit '{}'", unit_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::NamedDelete { unit: unit_name },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name and delete
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object manager and destroy the object
            let manager_arc = get_object_manager();
            let _ = manager_arc.write().ok().map(|mut mgr_guard| {
                mgr_guard.destroy_object(object_id);
                log::info!("Named unit '{}' deleted (ID: {})", unit_name, object_id);
            });
        } else {
            log::warn!("Named unit '{}' not found for deletion", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doNamedKill() line (in header)
    pub(crate) fn do_named_kill(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Killing named unit '{}'", unit_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::NamedKill { unit: unit_name },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name and kill
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // C++ ScriptActions::doNamedKill: pUnit->kill().
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if obj_arc
                    .write()
                    .ok()
                    .map(|mut obj_guard| {
                        obj_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                    })
                    .is_some()
                {
                    log::info!("Named unit '{}' killed (ID: {})", unit_name, object_id);
                }
            }
        } else {
            log::warn!("Named unit '{}' not found for kill", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doNamedDamage() line (in header)
    pub(crate) fn do_named_damage(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let damage_amount = self.get_int_param(action, 1)?;

        log::info!(
            "Damaging named unit '{}' for {} points",
            unit_name,
            damage_amount
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_kill_delete_damage(
                super::HostScriptKillDeleteDamageRequest::NamedDamage {
                    unit: unit_name,
                    amount: damage_amount,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name and apply damage
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object from manager and apply damage
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let _ = obj_arc.write().ok().map(|mut obj_guard| {
                    // Create damage info with script damage (unresistable type)
                    let mut damage_info = DamageInfo::with_simple(
                        damage_amount as f32,
                        INVALID_ID,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    let _ = obj_guard.attempt_damage(&mut damage_info);
                    log::info!(
                        "Named unit '{}' damaged for {} points (ID: {})",
                        unit_name,
                        damage_amount,
                        object_id
                    );
                });
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for damage", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // NAMED UNIT ACTIONS
    // C++ Reference: ScriptActions.cpp line 438 (named move)
    // ============================================================================

    /// C++ Reference: ScriptActions::doNamedMoveToWaypoint()
    pub(crate) fn do_named_move_to_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;

        log::info!(
            "Moving named unit '{}' to waypoint '{}'",
            unit_name,
            waypoint_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(super::HostScriptMoveAttackRequest::NamedMove {
                unit: unit_name,
                waypoint: waypoint_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(&unit_name).ok().flatten() else {
            log::warn!("Named unit '{}' not found for move to waypoint", unit_name);
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!("Named unit '{}' not found in object registry", unit_name);
            return Ok(ScriptActionResult::Success);
        };

        let waypoint_name_ascii = AsciiString::from(waypoint_name.as_str());
        let Some(position) = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_name_ascii)
                .map(|waypoint| *waypoint.get_location())
        }) else {
            log::warn!("Waypoint '{}' not found for move", waypoint_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut obj_guard) = obj_arc.write() {
            let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                log::warn!("Named unit '{}' has no AI update interface", unit_name);
                return Ok(ScriptActionResult::Success);
            };
            obj_guard.leave_group();
            if let Ok(mut ai_guard) = ai_arc.lock() {
                // C++ ScriptActions.cpp:433-436 clearWaypointQueue first.
                ai_guard.clear_waypoint_queue();
                let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params = AiCommandParams::new(
                    AiCommandType::MoveToPosition,
                    CommandSourceType::FromScript,
                );
                params.pos = position;
                let _ = ai_guard.execute_command(&params);
                log::info!(
                    "Named unit '{}' moving to waypoint '{}' at ({:.1}, {:.1}, {:.1})",
                    unit_name,
                    waypoint_name,
                    position.x,
                    position.y,
                    position.z
                );
            };
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_attack_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let attacker_name = self.get_string_param(action, 0)?;
        let victim_name = self.get_string_param(action, 1)?;

        log::info!("Named unit '{}' attacking '{}'", attacker_name, victim_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::NamedAttackNamed {
                    attacker: attacker_name,
                    victim: victim_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up attacker and victim object IDs by name
        let tracker = get_named_object_tracker();
        let attacker_id = tracker.get_object_id(&attacker_name).ok().flatten();
        let victim_id = tracker.get_object_id(&victim_name).ok().flatten();

        match (attacker_id, victim_id) {
            (Some(attacker), Some(target)) => {
                if TheGameLogic::find_object_by_id(target).is_none() {
                    log::warn!("Victim '{}' not found in object registry", victim_name);
                    return Ok(ScriptActionResult::Success);
                }

                let Some(obj_arc) = TheGameLogic::find_object_by_id(attacker) else {
                    log::warn!("Attacker '{}' not found in object registry", attacker_name);
                    return Ok(ScriptActionResult::Success);
                };

                if let Ok(mut obj_guard) = obj_arc.write() {
                    let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                        log::warn!("Attacker '{}' has no AI update interface", attacker_name);
                        return Ok(ScriptActionResult::Success);
                    };
                    obj_guard.leave_group();
                    if let Ok(mut ai_guard) = ai_arc.lock() {
                        let _ =
                            ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let mut params = AiCommandParams::new(
                            AiCommandType::ForceAttackObject,
                            CommandSourceType::FromScript,
                        );
                        params.obj = Some(target);
                        params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                        let _ = ai_guard.execute_command(&params);
                        log::info!(
                            "Named unit '{}' (ID: {}) force attacking '{}' (ID: {})",
                            attacker_name,
                            attacker,
                            victim_name,
                            target
                        );
                    };
                };
            }
            (None, _) => {
                log::warn!("Attacker '{}' not found for attack", attacker_name);
            }
            (_, None) => {
                log::warn!("Victim '{}' not found for attack", victim_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_hunt(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Named unit '{}' hunting", unit_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::NamedHunt {
                unit: unit_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object and issue hunt command via AI interface
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    if let Ok(mut ai) = ai_arc.lock() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let hunt_params = AiCommandParams::new(
                            AiCommandType::Hunt,
                            CommandSourceType::FromScript,
                        );
                        let _ = ai.execute_command(&hunt_params);
                        log::info!(
                            "Named unit '{}' hunt command issued (ID: {})",
                            unit_name,
                            object_id
                        );
                    };
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for hunt", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_guard(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Named unit '{}' guarding", unit_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::NamedGuard {
                unit: unit_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object and issue guard command via AI interface
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                // Get object's current position for guard position
                let position = obj_arc
                    .read()
                    .ok()
                    .map(|obj| obj.get_position().clone())
                    .unwrap_or_default();

                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.leave_group();
                    }
                    if let Ok(mut ai) = ai_arc.lock() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    }

                    let mut guard_params = AiCommandParams::new(
                        AiCommandType::GuardPosition,
                        CommandSourceType::FromScript,
                    );
                    guard_params.pos = position;
                    guard_params.int_value = GuardMode::Normal.as_i32();
                    let _ = ai_arc.lock().ok().map(|mut ai| {
                        let _ = ai.execute_command(&guard_params);
                        log::info!(
                            "Named unit '{}' guard command issued (ID: {}) at ({:.1}, {:.1}, {:.1})",
                            unit_name, object_id, position.x, position.y, position.z
                        );
                    });
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for guard", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_stop(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Named unit '{}' stopping", unit_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_idle(super::HostScriptIdleRequest::NamedStop {
                unit: unit_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());

                if let Some(ai_arc) = ai_result {
                    if let Ok(mut ai) = ai_arc.lock() {
                        let params = AiCommandParams::new(
                            AiCommandType::Idle,
                            CommandSourceType::FromScript,
                        );
                        let _ = ai.execute_command(&params);
                        log::info!("Named unit '{}' stopped (ID: {})", unit_name, object_id);
                    };
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for stop", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }
}
