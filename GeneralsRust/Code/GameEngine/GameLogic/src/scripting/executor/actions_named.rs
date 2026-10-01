//! Named-object actions including color, stealth, emoticon, face, special power, and unit helpers
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use crate::object::registry::OBJECT_REGISTRY;
use super::*;

impl ScriptActionDispatcher {
    // ============================================================================
    // ADDITIONAL NAMED UNIT ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_named_enter_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Unit '{}' entering '{}'", unit_name, target_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::NamedEnter {
                    unit: unit_name,
                    dest: target_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up both objects
        let tracker = get_named_object_tracker();
        let unit_id = tracker.get_object_id(&unit_name).ok().flatten();
        let target_id = tracker.get_object_id(&target_name).ok().flatten();

        if let (Some(uid), Some(tid)) = (unit_id, target_id) {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(uid, |obj| {
                    if let Some(ai) = obj.get_ai_update_interface_mut() {
                        let mut params = AiCommandParams::new(
                            AiCommandType::Enter,
                            CommandSourceType::FromScript,
                        );
                        params.obj = Some(tid);
                        let _ = ai.execute_command(&params);
                        log::info!(
                            "Unit '{}' enter command issued to '{}'",
                            unit_name,
                            target_name
                        );
                    }
                });
        } else {
            log::warn!(
                "Unit '{}' or target '{}' not found for enter command",
                unit_name,
                target_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_exit_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::info!("Unit '{}' exiting all contained", unit_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::NamedExitAll {
                    unit: unit_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up object and issue Evacuate command
        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(oid) = object_id {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(oid, |obj| {
                    if let Some(ai) = obj.get_ai_update_interface_mut() {
                        let params = AiCommandParams::new(
                            AiCommandType::Evacuate,
                            CommandSourceType::FromScript,
                        );
                        let _ = ai.execute_command(&params);
                        log::info!("Unit '{}' evacuate command issued", unit_name);
                    }
                });
        } else {
            log::warn!("Unit '{}' not found for exit command", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_follow_waypoints(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' following waypoints '{}'",
            unit_name,
            waypoint_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_follow_waypoints(
                super::HostScriptFollowWaypointsRequest::NamedFollow {
                    unit: unit_name,
                    waypoint: waypoint_name,
                    exact: false,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(oid) = object_id {
            let reference_pos = OBJECT_REGISTRY.with_object(oid, |obj| *obj.get_position());
            let Some(reference_pos) = reference_pos else {
                log::warn!("Unit '{}' not found for follow waypoints", unit_name);
                return Ok(ScriptActionResult::Success);
            };
            let waypoint_id = self.resolve_follow_waypoint_id(&waypoint_name, reference_pos);
            let Some(waypoint_id) = waypoint_id else {
                return Ok(ScriptActionResult::Success);
            };
            // C++ leaveGroup + NORMAL loco. Return stays outside the checkout.
            let issued = OBJECT_REGISTRY.with_object_mut(oid, |obj_guard| {
                if obj_guard.get_ai_update_interface().is_none() {
                    return false;
                }
                obj_guard.leave_group();
                if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowWaypointPath,
                        CommandSourceType::FromScript,
                    );
                    params.waypoint = Some(waypoint_id);
                    let _ = ai.execute_command(&params);
                }
                true
            });
            if issued == Some(false) {
                return Ok(ScriptActionResult::Success);
            }
        } else {
            log::warn!("Unit '{}' not found for follow waypoints", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_follow_waypoints_exact(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' following waypoints '{}' exact",
            unit_name,
            waypoint_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_follow_waypoints(
                super::HostScriptFollowWaypointsRequest::NamedFollow {
                    unit: unit_name,
                    waypoint: waypoint_name,
                    exact: true,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(oid) = object_id {
            let reference_pos = OBJECT_REGISTRY.with_object(oid, |obj| *obj.get_position());
            let Some(reference_pos) = reference_pos else {
                log::warn!("Unit '{}' not found for follow waypoints exact", unit_name);
                return Ok(ScriptActionResult::Success);
            };
            let waypoint_id = self.resolve_follow_waypoint_id(&waypoint_name, reference_pos);
            let Some(waypoint_id) = waypoint_id else {
                return Ok(ScriptActionResult::Success);
            };
            // C++ leaveGroup + NORMAL loco. Return stays outside the checkout.
            let issued = OBJECT_REGISTRY.with_object_mut(oid, |obj_guard| {
                if obj_guard.get_ai_update_interface().is_none() {
                    return false;
                }
                obj_guard.leave_group();
                if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowWaypointPathExact,
                        CommandSourceType::FromScript,
                    );
                    params.waypoint = Some(waypoint_id);
                    let _ = ai.execute_command(&params);
                }
                true
            });
            if issued == Some(false) {
                return Ok(ScriptActionResult::Success);
            }
        } else {
            log::warn!("Unit '{}' not found for follow waypoints exact", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_attack_area(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let area_name = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' attacking area '{}'", unit_name, area_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::NamedAttackArea {
                    unit: unit_name,
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

        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            let found = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if obj_guard.get_ai_update_interface().is_none() {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                } else {
                    obj_guard.leave_group();
                    if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let mut params = AiCommandParams::new(
                            AiCommandType::AttackArea,
                            CommandSourceType::FromScript,
                        );
                        params.pos = area_center;
                        params.polygon = Some(trigger_id);
                        let _ = ai.execute_command(&params);
                        log::info!(
                            "Named unit '{}' attack area '{}' command issued (ID: {})",
                            unit_name,
                            area_name,
                            object_id
                        );
                    }
                }
            });
            if found.is_none() {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for attack area", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_attack_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 1)?);

        log::info!("Unit '{}' attacking team '{}'", unit_name, team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::NamedAttackTeam {
                    unit: unit_name,
                    team: team_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        if self.get_team_by_name(&team_name).is_err() {
            log::warn!(
                "Target team '{}' not found for named attack team",
                team_name
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            let found = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if obj_guard.get_ai_update_interface().is_none() {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                } else {
                    obj_guard.leave_group();
                    if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let mut params = AiCommandParams::new(
                            AiCommandType::AttackTeam,
                            CommandSourceType::FromScript,
                        );
                        params.team = Some(team_name.clone());
                        params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                        let _ = ai.execute_command(&params);
                        log::info!(
                            "Named unit '{}' attack team '{}' command issued (ID: {})",
                            unit_name,
                            team_name,
                            object_id
                        );
                    }
                }
            });
            if found.is_none() {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for attack team", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_apply_attack_priority_set(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let priority_set = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' applying attack priority set '{}'",
            unit_name,
            priority_set
        );

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(&unit_name).ok().flatten() else {
            log::warn!(
                "Named unit '{}' not found for attack priority set '{}'",
                unit_name,
                priority_set
            );
            return Ok(ScriptActionResult::Success);
        };

        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_none() {
            log::warn!(
                "Named unit '{}' object {} no longer exists for attack priority set '{}'",
                unit_name,
                object_id,
                priority_set
            );
            return Ok(ScriptActionResult::Success);
        }

        let resolved_name = with_script_engine_ref(|engine| {
            engine
                .get_attack_info(&priority_set)
                .map(|info| info.get_name().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();
        let _ = with_script_engine_mut(|engine| {
            if resolved_name.is_empty() {
                engine.clear_object_attack_priority_set(object_id);
            } else {
                engine.set_object_attack_priority_set(object_id, resolved_name.as_str());
            }
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_attitude(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        // C++ ScriptActions.cpp:6585 updateNamedSetAttitude(..., getInt()).
        let mood = self.get_int_param(action, 1)?;
        if super::dual_world_registry_unavailable() {
            super::request_host_script_named_attitude(super::HostScriptNamedAttitudeRequest {
                unit: unit_name,
                mood,
            });
            return Ok(ScriptActionResult::Success);
        }
        let attitude = self.attitude_from_script_int(mood);

        log::info!("Unit '{}' setting attitude to {:?}", unit_name, attitude);

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            let found = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                    if let Err(err) = ai.set_attitude(attitude) {
                        log::debug!(
                            "ScriptActions::do_named_set_attitude failed for object {}: {}",
                            object_id,
                            err
                        );
                    }
                    log::info!(
                        "Named unit '{}' attitude set to {:?} (ID: {})",
                        unit_name,
                        attitude,
                        object_id
                    );
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            });
            if found.is_none() {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for set attitude", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_flash(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing unit '{}' for {}s", unit_name, time_in_seconds);
        // Live host objects are not in leftover OBJECT_REGISTRY. Always queue.
        super::request_host_script_flash(super::HostScriptFlashRequest::Named {
            unit: unit_name.clone(),
            seconds: time_in_seconds,
            white: false,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.flash_object_by_id(object_id, time_in_seconds, None);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_flash_white(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!(
            "Flashing unit '{}' white for {}s",
            unit_name,
            time_in_seconds
        );
        super::request_host_script_flash(super::HostScriptFlashRequest::Named {
            unit: unit_name.clone(),
            seconds: time_in_seconds,
            white: true,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.flash_object_by_id(object_id, time_in_seconds, Some(Color::white()));
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_garrison_specific_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let building_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' garrisoning building '{}'",
            unit_name,
            building_name
        );
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::NamedGarrisonSpecific {
                    unit: unit_name,
                    building: building_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(building_id)) = tracker.get_object_id(&building_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let can_garrison = OBJECT_REGISTRY.with_object(unit_id, |unit_guard| {
            OBJECT_REGISTRY.with_object(building_id, |building_guard| {
                let player_mask = unit_guard
                    .get_controlling_player()
                    .and_then(|index| {
                        crate::player::with_player(index, |player| player.get_player_mask())
                    })
                    .unwrap_or_else(crate::common::PlayerMaskType::none);
                if !building_guard.is_kind_of(crate::common::KindOf::Structure) {
                    false
                } else if let Some(contain) = building_guard.get_contain() {
                    let entered_mask = contain.get_player_who_entered();
                    entered_mask == crate::common::PlayerMaskType::none() || entered_mask == player_mask
                } else {
                    false
                }
            })
        });
        if can_garrison != Some(Some(true)) {
            return Ok(ScriptActionResult::Success);
        }

        let entered = OBJECT_REGISTRY.with_object_mut(unit_id, |unit_guard| {
            unit_guard.leave_group();
            let Some(ai) = unit_guard.get_ai_update_interface_mut() else {
                return false;
            };
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut params =
                AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
            params.obj = Some(building_id);
            let _ = ai.execute_command(&params);
            true
        });
        if entered.is_none() {
            return Ok(ScriptActionResult::Success);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_garrison_nearest_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' garrisoning nearest building", unit_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::NamedGarrisonNearest {
                    unit: unit_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some((unit_pos, unit_off_map, unit_is_hacker, unit_player_mask)) =
            OBJECT_REGISTRY.with_object(unit_id, |unit_guard| {
                (
                    *unit_guard.get_position(),
                    unit_guard.is_off_map(),
                    unit_guard.is_kind_of(crate::common::KindOf::Hacker),
                    unit_guard
                        .get_controlling_player()
                        .and_then(|index| {
                            crate::player::with_player(index, |player| player.get_player_mask())
                        })
                        .unwrap_or_else(crate::common::PlayerMaskType::none),
                )
            })
        else {
            return Ok(ScriptActionResult::Success);
        };

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptActionResult::Success);
        };

        let closest_building_id = partition.get_closest_object_2d(&unit_pos, 1_000_000.0, |obj| {
            if obj.get_id() == unit_id {
                return false;
            }
            if obj.is_effectively_dead() || obj.is_off_map() != unit_off_map {
                return false;
            }
            if !obj.is_kind_of(crate::common::KindOf::Structure) {
                return false;
            }

            let is_internet_center = obj.is_kind_of(crate::common::KindOf::FSInternetCenter);
            if unit_is_hacker {
                if !is_internet_center {
                    return false;
                }
            } else if is_internet_center {
                return false;
            }

            let Some(contain) = obj.get_contain() else {
                return false;
            };
            let entered_mask = contain
                .get_player_who_entered();
            entered_mask == crate::common::PlayerMaskType::none()
                || entered_mask == unit_player_mask
        });

        let Some(target_id) = closest_building_id else {
            return Ok(ScriptActionResult::Success);
        };

        let entered = OBJECT_REGISTRY.with_object_mut(unit_id, |unit_guard| {
            unit_guard.leave_group();
            let Some(ai) = unit_guard.get_ai_update_interface_mut() else {
                return false;
            };
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut params =
                AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            let _ = ai.execute_command(&params);
            true
        });
        if entered != Some(true) {
            return Ok(ScriptActionResult::Success);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_exit_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' exiting building", unit_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::NamedExitBuilding {
                    unit: unit_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let issued = OBJECT_REGISTRY.with_object_mut(unit_id, |unit_guard| {
            if unit_guard.get_ai_update_interface().is_none() {
                return false;
            }
            unit_guard.leave_group();
            if let Some(ai) = unit_guard.get_ai_update_interface_mut() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let params =
                    AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromScript);
                let _ = ai.execute_command(&params);
            }
            true
        });
        if issued.is_none() || issued == Some(false) {
            return Ok(ScriptActionResult::Success);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_stopping_distance(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let distance = self.get_real_param(action, 1)?;
        log::debug!(
            "Unit '{}' setting stopping distance to {}",
            unit_name,
            distance
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_stopping_distance(
                super::HostScriptStoppingDistanceRequest::Named {
                    unit: unit_name,
                    distance,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        if distance < 0.5 {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let mut has_loco = false;
        let updated = OBJECT_REGISTRY.with_object_mut(unit_id, |unit| {
            let Some(ai) = unit.get_ai_update_interface_mut() else {
                return false;
            };
            ai.with_cur_locomotor(&mut |loco| {
                has_loco = true;
                loco.set_close_enough_dist(distance);
            });
            true
        });
        if updated != Some(true) || !has_loco {
            return Ok(ScriptActionResult::Success);
        }


        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_transfer_ownership_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Transferring unit '{}' to player '{}'",
            unit_name,
            player_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_transfer(super::HostScriptTransferRequest::Named {
                unit: unit_name,
                player: player_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(unit_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let destination_team = player_list().read().ok().and_then(|list| {
            list.find_player_by_name(&player_name)
                .and_then(|player| player.get_default_team_id())
        });
        let Some(destination_team) = destination_team else {
            return Ok(ScriptActionResult::Success);
        };

        let _ = OBJECT_REGISTRY.with_object_mut(unit_id, |unit_guard| {
            let old_owner = unit_guard.get_controlling_player();
            let _ = unit_guard.set_team(Some(destination_team));
            let new_owner = unit_guard.get_controlling_player();
            unit_guard.on_capture(old_owner, new_owner);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_hide_special_power_display(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding special power display for '{}'", unit_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            // Copy the host callback out of the active engine before calling
            // it.  The callback may immediately re-enter script execution.
            let handler =
                with_script_engine_ref(|script_engine| script_engine.action_handler()).flatten();
            if let Some(handler) = handler {
                if let Err(err) = handler.hide_object_superweapon_display_by_script(object_id) {
                    log::warn!(
                        "Script action handler hide_object_superweapon_display_by_script failed: {}",
                        err
                    );
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_show_special_power_display(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Showing special power display for '{}'", unit_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            // Copy the host callback out of the active engine before calling
            // it.  The callback may immediately re-enter script execution.
            let handler =
                with_script_engine_ref(|script_engine| script_engine.action_handler()).flatten();
            if let Some(handler) = handler {
                if let Err(err) = handler.show_object_superweapon_display_by_script(object_id) {
                    log::warn!(
                        "Script action handler show_object_superweapon_display_by_script failed: {}",
                        err
                    );
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_stop_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        log::debug!(
            "Stopping special power countdown '{}' for '{}'",
            special_power,
            unit_name
        );

        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            sp_module.pause_countdown(true);
        });
        if let Some(handler) = current_script_action_handler() {
            let _ = handler.pause_named_special_power_countdown(&unit_name, &special_power, true);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_start_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        log::debug!(
            "Starting special power countdown '{}' for '{}'",
            special_power,
            unit_name
        );

        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            sp_module.pause_countdown(false);
        });
        if let Some(handler) = current_script_action_handler() {
            let _ = handler.pause_named_special_power_countdown(&unit_name, &special_power, false);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        let countdown = self.get_int_param(action, 2)?;
        log::debug!(
            "Setting special power countdown '{}' for '{}' to {}s",
            special_power,
            unit_name,
            countdown
        );

        let frames = countdown.saturating_mul(LOGICFRAMES_PER_SECOND as i32);
        let base_frame = TheGameLogic::get_frame();
        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            let ready_frame = base_frame.saturating_add_signed(frames);
            sp_module.set_ready_frame(ready_frame);
        });
        if let Some(handler) = current_script_action_handler() {
            let _ =
                handler.set_named_special_power_countdown(&unit_name, &special_power, countdown);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_add_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        let amount = self.get_int_param(action, 2)?;
        log::debug!(
            "Adding {}s to special power countdown '{}' for '{}'",
            amount,
            special_power,
            unit_name
        );

        let frames = amount.saturating_mul(LOGICFRAMES_PER_SECOND as i32);
        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            let new_ready_frame = sp_module.get_ready_frame().saturating_add_signed(frames);
            sp_module.set_ready_frame(new_ready_frame);
        });
        if let Some(handler) = current_script_action_handler() {
            let _ = handler.add_named_special_power_countdown(&unit_name, &special_power, amount);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_fire_special_power_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let power_name = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' firing special power '{}' at waypoint '{}'",
            unit_name,
            power_name,
            waypoint
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_named_fire_special(
                super::HostScriptNamedFireSpecialPowerRequest::AtWaypoint {
                    unit: unit_name,
                    power: power_name,
                    waypoint,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(source_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(waypoint_pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        let template_name = {
            let Some(store) = get_special_power_store() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(template) = store.find_special_power_template(&power_name) else {
                return Ok(ScriptActionResult::Success);
            };
            template.get_name().to_string()
        };

        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            let _ = source_guard.with_special_power_module_mut_by_name(&template_name, |sp_module| {
                sp_module.do_special_power_at_location(
                    &waypoint_pos,
                    INVALID_ANGLE,
                    SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT,
                );
            });
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_fire_special_power_at_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let power_name = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' firing special power '{}' at '{}'",
            unit_name,
            power_name,
            target_name
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_named_fire_special(
                super::HostScriptNamedFireSpecialPowerRequest::AtNamed {
                    unit: unit_name,
                    power: power_name,
                    target: target_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(source_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }
        if OBJECT_REGISTRY.with_object(target_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let template_name = {
            let Some(store) = get_special_power_store() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(template) = store.find_special_power_template(&power_name) else {
                return Ok(ScriptActionResult::Success);
            };
            template.get_name().to_string()
        };

        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            let _ = source_guard.with_special_power_module_mut_by_name(&template_name, |sp_module| {
                sp_module.do_special_power_at_object(
                    target_id,
                    SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT,
                );
            });
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_fire_weapon_following_waypoint_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_path = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' firing weapon following waypoint path '{}'",
            unit_name,
            waypoint_path
        );
        // Leftover crate forceFire cannot spawn a live projectile. Queue leftover
        // forceFire + follow waypoint path for the live host drain.
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_named_fire_weapon_path(
                &unit_name,
                &waypoint_path,
            );
            return Ok(ScriptActionResult::Success);
        }
        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some((source_pos, waypoint)) = OBJECT_REGISTRY.with_object(source_id, |source_guard| {
            let source_pos = *source_guard.get_position();
            let waypoint = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&source_pos, &waypoint_path)
                    .map(crate::waypoint::Waypoint::from_terrain)
            });
            (source_pos, waypoint)
        }) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(waypoint) = waypoint else {
            return Ok(ScriptActionResult::Success);
        };

        let max_object_id_before_fire = get_object_manager()
            .read()
            .ok()
            .and_then(|mgr| mgr.all_object_ids().into_iter().max())
            .unwrap_or(0);

        let taken = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            source_guard
                .weapon_set
                .take_waypoint_following_capable_weapon()
        });
        let Some(taken) = taken else {
            return Ok(ScriptActionResult::Success);
        };
        let fired = match taken {
            Some((slot, mut weapon)) => {
                // Fire outside the checkout: the projectile cascade re-enters
                // the source id, and a nested same-id lookup is None.
                let _ = weapon.force_fire_weapon(source_id, &source_pos);
                let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
                    source_guard
                        .weapon_set
                        .restore_waypoint_following_weapon(slot, weapon);
                });
                true
            }
            None => false,
        };
        if !fired {
            return Ok(ScriptActionResult::Success);
        }

        let Some(projectile_id) =
            self.find_recent_projectile_fired_by(source_id, max_object_id_before_fire)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let _ = OBJECT_REGISTRY.with_object_mut(projectile_id, |projectile_guard| {
            projectile_guard.leave_group();
            let Some(ai) = projectile_guard.get_ai_update_interface_mut() else {
                return;
            };
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut params = AiCommandParams::new(
                AiCommandType::FollowWaypointPath,
                CommandSourceType::FromScript,
            );
            params.waypoint = Some(waypoint.id);
            let _ = ai.execute_command(&params);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_use_command_button_on_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' using command '{}' on '{}'",
            unit_name,
            command_button,
            target_name
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::NamedOnNamed {
                unit: unit_name.clone(),
                button: command_button.clone(),
                target: target_name.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(target_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }
        let button_ids = OBJECT_REGISTRY.with_object(source_id, |source_guard| {
            self.matching_command_button_ids_for_object(source_guard, &command_button)
        });
        let Some(button_ids) = button_ids else {
            return Ok(ScriptActionResult::Success);
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            for button_id in button_ids {
                let _ = OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                    let _ = source_guard.do_command_button_at_object(
                        button_id,
                        target_guard,
                        CommandSourceType::FromScript,
                    );
                });
            }
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_use_command_button_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' using command '{}' at waypoint '{}'",
            unit_name,
            command_button,
            waypoint
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::NamedAtWaypoint {
                unit: unit_name.clone(),
                button: command_button.clone(),
                waypoint: waypoint.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(source_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }
        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(waypoint_pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        let button_ids = OBJECT_REGISTRY.with_object(source_id, |source_guard| {
            self.matching_command_button_ids_for_object(source_guard, &command_button)
        });
        let Some(button_ids) = button_ids else {
            return Ok(ScriptActionResult::Success);
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            for button_id in button_ids {
                let _ = source_guard.do_command_button_at_position(
                    button_id,
                    &waypoint_pos,
                    CommandSourceType::FromScript,
                );
            }
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_use_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' using command '{}'", unit_name, command_button);
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::Named {
                unit: unit_name.clone(),
                button: command_button.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let button_ids = OBJECT_REGISTRY.with_object(source_id, |source_guard| {
            self.matching_command_button_ids_for_object(source_guard, &command_button)
        });
        let Some(button_ids) = button_ids else {
            return Ok(ScriptActionResult::Success);
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            for button_id in button_ids {
                let _ = source_guard.do_command_button(button_id, CommandSourceType::FromScript);
            }
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_use_command_button_using_waypoint_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let waypoint_path = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' using command '{}' along waypoint path '{}'",
            unit_name,
            command_button,
            waypoint_path
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::NamedUsingWaypointPath {
                unit: unit_name.clone(),
                button: command_button.clone(),
                path: waypoint_path.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let waypoint = OBJECT_REGISTRY.with_object(source_id, |source_guard| {
            let source_pos = *source_guard.get_position();
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&source_pos, &waypoint_path)
                    .map(crate::waypoint::Waypoint::from_terrain)
            })
        }).flatten();
        let Some(waypoint) = waypoint else {
            return Ok(ScriptActionResult::Success);
        };

        let button_ids = OBJECT_REGISTRY
            .with_object(source_id, |source_guard| {
                self.matching_command_button_ids_for_object(source_guard, &command_button)
            })
            .unwrap_or_default();
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let _ = OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
            for button_id in button_ids {
                let _ = source_guard.do_command_button_using_waypoints(
                    button_id,
                    &waypoint,
                    CommandSourceType::FromScript,
                );
            }
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn matching_command_button_ids_for_object(
        &self,
        obj: &crate::object::Object,
        ability: &str,
    ) -> Vec<u32> {
        let Some(control_bar) = get_control_bar_bridge() else {
            return Vec::new();
        };
        let Some(command_set) = control_bar.find_command_set_by_name(obj.get_command_set_string())
        else {
            return Vec::new();
        };

        let matches: Vec<_> = command_set
            .buttons
            .iter()
            .flatten()
            .filter(|command_button| {
                !command_button.get_name().is_empty() && command_button.get_name() == ability
            })
            .map(|command_button| command_button.get_id())
            .collect();
        matches
    }

    pub(crate) fn do_named_receive_upgrade(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let upgrade_name = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' receiving upgrade '{}'", unit_name, upgrade_name);
        // Live host objects are not in leftover OBJECT_REGISTRY. Always queue
        // C++ `doUnitReceiveUpgrade` → `giveUpgrade` for the player path.
        super::request_host_script_named_upgrade(super::HostScriptNamedUpgradeRequest {
            unit: unit_name.clone(),
            upgrade: upgrade_name.clone(),
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let upgrade = get_upgrade_center()
            .read()
            .ok()
            .and_then(|center| center.find_upgrade(upgrade_name.as_str()));
        let Some(upgrade) = upgrade else {
            return Ok(ScriptActionResult::Success);
        };

        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
            if obj_guard.affected_by_upgrade(upgrade.as_ref()) {
                obj_guard.give_upgrade(upgrade.as_ref());
            }
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_held(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let held = self.get_int_param(action, 1)? != 0;
        log::debug!("Unit '{}' held: {}", unit_name, held);
        super::request_host_script_held(super::HostScriptHeldRequest {
            unit: unit_name.clone(),
            held,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                    let _ = obj_guard.set_disabled_held(held);
                });
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_topple_direction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let dir = self.get_coord_param(action, 1)?;
        let direction = crate::common::Coord3D::new(dir.x, dir.y, dir.z);
        let _ = with_script_engine_mut(|engine| {
            engine.set_topple_direction(&unit_name, Some(direction));
        });
        super::request_host_script_topple_direction(&unit_name, dir.x, dir.y);
        log::debug!("Setting topple direction for '{}'", unit_name);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_repulsor(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Unit '{}' repulsor: {}", unit_name, enabled);
        super::request_host_script_repulsor(super::HostScriptRepulsorRequest::Named {
            unit: unit_name.clone(),
            enabled,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                    obj_guard.set_status(crate::common::ObjectStatusMaskType::REPULSOR, enabled);
                });
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_custom_color(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let color_raw = self.get_int_param(action, 1)? as u32;
        // C++ Color.h GameMakeColor: (a<<24)|(r<<16)|(g<<8)|b
        let color = crate::common::Color::new(
            ((color_raw >> 16) & 0xFF) as u8,
            ((color_raw >> 8) & 0xFF) as u8,
            (color_raw & 0xFF) as u8,
            ((color_raw >> 24) & 0xFF) as u8,
        );
        log::debug!(
            "Setting custom color for '{}': 0x{:08X}",
            unit_name,
            color_raw
        );
        super::request_host_script_custom_color(super::HostScriptCustomColorRequest {
            unit: unit_name.clone(),
            color_raw,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                    obj_guard.set_custom_indicator_color(color);
                });
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_stealth_enabled(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Unit '{}' stealth enabled: {}", unit_name, enabled);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_stealth_enabled(
                super::HostScriptStealthEnabledRequest::Named {
                    unit: unit_name,
                    enabled,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                    obj_guard.set_script_status(
                        crate::object::ObjectScriptStatusBit::ScriptUnstealthed,
                        !enabled,
                    );
                });
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_emoticon(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let emoticon = self.get_string_param(action, 1)?;
        let duration_seconds = self.get_real_param(action, 2)?;
        let duration_frames = (duration_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        log::debug!(
            "Unit '{}' emoticon '{}' for {}s ({}f)",
            unit_name,
            emoticon,
            duration_seconds,
            duration_frames
        );
        super::request_host_script_emoticon(super::HostScriptEmoticonRequest::Named {
            unit: unit_name.clone(),
            emoticon: emoticon.clone(),
            duration_frames,
        });
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.emoticon_object_by_id(object_id, &emoticon, duration_frames);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_face_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' facing '{}'", unit_name, target_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_face(super::HostScriptFaceRequest::NamedFaceNamed {
                unit: unit_name,
                target: target_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(target_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }
        let faced = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
            obj_guard.leave_group();
            let Some(ai) = obj_guard.get_ai_update_interface_mut() else {
                return false;
            };
            ai.clear_waypoint_queue();
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut params =
                AiCommandParams::new(AiCommandType::FaceObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            let _ = ai.execute_command(&params);
            true
        });
        if faced.is_none() {
            return Ok(ScriptActionResult::Success);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_face_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' facing waypoint '{}'", unit_name, waypoint);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_face(super::HostScriptFaceRequest::NamedFaceWaypoint {
                unit: unit_name,
                waypoint,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|way| *way.get_location())
            })
        };
        let Some(waypoint_pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
            obj_guard.leave_group();
            let Some(ai) = obj_guard.get_ai_update_interface_mut() else {
                return;
            };
            ai.clear_waypoint_queue();
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut params = AiCommandParams::new(
                AiCommandType::FacePosition,
                CommandSourceType::FromScript,
            );
            params.pos = waypoint_pos;
            let _ = ai.execute_command(&params);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_evac_left_or_right(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let disposition = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(0);
        log::debug!(
            "Setting evac disposition for '{}' to {}",
            unit_name,
            disposition
        );
        crate::object::contain::record_named_evac_disposition(
            &unit_name,
            disposition.max(0) as u32,
        );

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if let Some(contain) = obj_guard.get_contain_mut() {
                    contain.set_evac_disposition(disposition.max(0) as u32);
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_unmanned_status(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' set unmanned", unit_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_unmanned(super::HostScriptUnmannedRequest::Named {
                unit: unit_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.mark_object_unmanned(object_id);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_named_set_boobytrapped(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let boobytrap_template = self.get_string_param(action, 0)?;
        let unit_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' set boobytrapped using template '{}'",
            unit_name,
            boobytrap_template
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_boobytrap(super::HostScriptBoobytrapRequest::Named {
                thing: boobytrap_template,
                unit: unit_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = self.attach_boobytrap_to_object(&boobytrap_template, object_id);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn mark_object_unmanned(&self, object_id: ObjectID) {
        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
            obj_guard.set_disabled_unmanned();
            let _ = TheGameLogic::deselect_object(&*obj_guard, crate::common::PLAYERMASK_ALL, true);
            obj_guard.set_team_to_neutral();
        });
    }

    pub(crate) fn attach_boobytrap_to_object(
        &self,
        boobytrap_template_name: &str,
        target_object_id: ObjectID,
    ) -> bool {
        let Some(target_team) = OBJECT_REGISTRY
            .with_object(target_object_id, |obj| obj.get_team())
        else {
            return false;
        };
        let target_team = target_team;

        let Some(template) = TheObjectFactory::find_template(boobytrap_template_name) else {
            return false;
        };
        let Ok(boobytrap_obj) = TheObjectFactory::new_object(template, target_team) else {
            return false;
        };

        let module = OBJECT_REGISTRY
            .with_object(boobytrap_obj, |obj| obj.find_update_module("StickyBombUpdate"))
            .flatten();
        let Some(module) = module else {
            return false;
        };
        let mut initialized = false;
        let placed = OBJECT_REGISTRY.with_object(target_object_id, |obj| {
            let geom = obj.get_geometry_info();
            let mut local = crate::common::Coord3D::new(0.0, 0.0, 0.0);
            if let Some((x, y)) = obj.get_template().random_offset_on_perimeter() {
                local.x = x;
                local.y = y;
            } else if crate::common::types::geometry_type_to_u32(geom.geometry_type) == 2 {
                let major = (geom.bounds.max.x - geom.bounds.min.x).abs() * 0.5;
                let minor = (geom.bounds.max.y - geom.bounds.min.y).abs() * 0.5;
                if crate::helpers::get_game_logic_random_value_real(0.0, 1.0) < 0.5 {
                    local.x = crate::helpers::get_game_logic_random_value_real(-major, major);
                    local.y = if crate::helpers::get_game_logic_random_value_real(0.0, 1.0) < 0.5 {
                        -minor
                    } else {
                        minor
                    };
                } else {
                    local.y = crate::helpers::get_game_logic_random_value_real(-minor, minor);
                    local.x = if crate::helpers::get_game_logic_random_value_real(0.0, 1.0) < 0.5 {
                        -major
                    } else {
                        major
                    };
                }
            }
            let angle = obj.get_orientation();
            let (sin_a, cos_a) = angle.sin_cos();
            let origin = *obj.get_position();
            crate::common::Coord3D::new(
                origin.x + local.x * cos_a - local.y * sin_a,
                origin.y + local.x * sin_a + local.y * cos_a,
                0.0,
            )
        });
        module.with_module(|module| {
            if let Some(sticky_bomb) = module.get_sticky_bomb_control_interface() {
                if let Some(pos) = placed {
                    sticky_bomb.init_sticky_bomb_at(
                        target_object_id,
                        INVALID_ID,
                        pos.x,
                        pos.y,
                        pos.z,
                    );
                } else {
                    sticky_bomb.init_sticky_bomb(target_object_id, INVALID_ID);
                }
                initialized = true;
            }
        });

        initialized
    }

    pub(crate) fn find_recent_projectile_fired_by(
        &self,
        source_object_id: ObjectID,
        minimum_new_object_id: ObjectID,
    ) -> Option<ObjectID> {
        let manager = get_object_manager();
        let ids = manager.read().ok()?.all_object_ids();

        let mut latest = None;
        for object_id in ids {
            if object_id <= minimum_new_object_id {
                continue;
            }
            {
                enum _ObjFlow<T> { Cont, Ret(T), Fall }
                let _flow = OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                    if obj_guard.get_producer_id() != source_object_id {
                        return _ObjFlow::Cont;
                    }
                    if !obj_guard.is_kind_of(crate::common::KindOf::Projectile) {
                        return _ObjFlow::Cont;
                    }
                    if latest.is_none_or(|current| object_id > current) {
                        latest = Some(object_id);
                    }
                    _ObjFlow::Fall
                });
                match _flow {
                    None | Some(_ObjFlow::Cont) => continue,
                    Some(_ObjFlow::Ret(v)) => return v,
                    Some(_ObjFlow::Fall) => {}
                }
            }
        }

        latest
    }

    pub(crate) fn do_unit_execute_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let script_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' executing sequential script '{}'",
            unit_name,
            script_name
        );

        let tracker = get_named_object_tracker();
        let object_id = match tracker.get_object_id(&unit_name) {
            Ok(Some(id)) => id,
            _ => match crate::scripting::host_script_named_unit_id(&unit_name) {
                Some(id) => id,
                None => return Ok(ScriptActionResult::Success),
            },
        };

        let _ = with_script_engine_mut(|engine| {
            let Some(script) = engine.find_script_clone_by_name(&script_name) else {
                return;
            };

            let mut seq_script = crate::scripting::engine::SequentialScript::new();
            seq_script.object_id = object_id;
            seq_script.script_to_execute_sequentially = Some(Box::new(script));
            seq_script.times_to_loop = 0;
            engine.append_sequential_script(seq_script);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unit_execute_sequential_script_looping(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let script_name = self.get_string_param(action, 1)?;
        let loop_val = self.get_int_param(action, 2)? - 1;
        log::debug!(
            "Unit '{}' executing sequential script '{}' looping ({})",
            unit_name,
            script_name,
            loop_val
        );

        let tracker = get_named_object_tracker();
        let object_id = match tracker.get_object_id(&unit_name) {
            Ok(Some(id)) => id,
            _ => match crate::scripting::host_script_named_unit_id(&unit_name) {
                Some(id) => id,
                None => return Ok(ScriptActionResult::Success),
            },
        };

        let _ = with_script_engine_mut(|engine| {
            let Some(script) = engine.find_script_clone_by_name(&script_name) else {
                return;
            };

            let mut seq_script = crate::scripting::engine::SequentialScript::new();
            seq_script.object_id = object_id;
            seq_script.script_to_execute_sequentially = Some(Box::new(script));
            seq_script.times_to_loop = loop_val;
            engine.append_sequential_script(seq_script);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unit_stop_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' stopping sequential script", unit_name);

        let tracker = get_named_object_tracker();
        let object_id = match tracker.get_object_id(&unit_name) {
            Ok(Some(id)) => id,
            _ => match crate::scripting::host_script_named_unit_id(&unit_name) {
                Some(id) => id,
                None => return Ok(ScriptActionResult::Success),
            },
        };

        let _ = with_script_engine_mut(|engine| {
            engine.remove_all_sequential_scripts_for_object(object_id);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unit_guard_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Unit '{}' guarding for {} frames", unit_name, frames);

        // Live host objects are not in leftover OBJECT_REGISTRY. Queue
        // C++ doUnitGuardForFramecount: NORMAL loco + aiGuardPosition(self).
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::NamedGuard {
                unit: unit_name.clone(),
            });
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return if frames > 0 {
                Ok(ScriptActionResult::Pending(frames as f32))
            } else {
                Ok(ScriptActionResult::Success)
            };
        };
        let guarded = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
            let pos = *obj.get_position();
            let Some(ai) = obj.get_ai_update_interface_mut() else {
                return None;
            };
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut guard_params =
                AiCommandParams::new(AiCommandType::GuardPosition, CommandSourceType::FromScript);
            guard_params.pos = pos;
            let _ = ai.execute_command(&guard_params);
            Some(())
        });
        if guarded.flatten().is_none() {
            return if frames > 0 {
                Ok(ScriptActionResult::Pending(frames as f32))
            } else {
                Ok(ScriptActionResult::Success)
            };
        }

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn do_unit_idle_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Unit '{}' idling for {} frames", unit_name, frames);

        // Live host objects are not in leftover OBJECT_REGISTRY. Queue
        // C++ doUnitIdleForFramecount: aiIdle(CMD_FROM_SCRIPT) + sequential timer.
        if super::dual_world_registry_unavailable() {
            super::request_host_script_idle(super::HostScriptIdleRequest::NamedStop {
                unit: unit_name.clone(),
            });
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return if frames > 0 {
                Ok(ScriptActionResult::Pending(frames as f32))
            } else {
                Ok(ScriptActionResult::Success)
            };
        };
        let idled = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
            let Some(ai) = obj.get_ai_update_interface_mut() else {
                return None;
            };
            let idle_params =
                AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = ai.execute_command(&idle_params);
            Some(())
        });
        if idled.flatten().is_none() {
            return if frames > 0 {
                Ok(ScriptActionResult::Pending(frames as f32))
            } else {
                Ok(ScriptActionResult::Success)
            };
        }

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    pub(crate) fn do_unit_destroy_all_contained(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Destroying all units contained in '{}'", unit_name);
        super::request_host_script_kill_delete_damage(
            super::HostScriptKillDeleteDamageRequest::DestroyAllContained {
                unit: unit_name.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
            if let Some(contain) = obj.get_contain_mut() {
                if contain.get_contained_count() > 0 {
                    let _ = contain.kill_all_contained();
                }
            }
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unit_move_towards_nearest_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let trigger_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' moving towards nearest '{}' in trigger '{}'",
            unit_name,
            object_type,
            trigger_name
        );

        // Leftover partition / leftover crate objects are empty on the player path.
        if super::dual_world_registry_unavailable() {
            super::request_host_script_move_attack(
                super::HostScriptMoveAttackRequest::NamedMoveTowardsNearest {
                    unit: unit_name.clone(),
                    object_type: object_type.clone(),
                    trigger: trigger_name.clone(),
                },
            );
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some((source_pos, source_off_map)) = OBJECT_REGISTRY.with_object(object_id, |obj| {
            (*obj.get_position(), obj.is_off_map())
        }) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(target_id) = self.find_closest_object_of_type_in_trigger(
            object_id,
            &source_pos,
            source_off_map,
            &object_type,
            &trigger_name,
        ) else {
            return Ok(ScriptActionResult::Success);
        };
        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
            if let Some(ai) = obj.get_ai_update_interface_mut() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params = AiCommandParams::new(
                    AiCommandType::MoveToObject,
                    CommandSourceType::FromScript,
                );
                params.obj = Some(target_id);
                let _ = ai.execute_command(&params);
            }
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unit_affect_object_panel_flags(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let flag_name = self.get_string_param(action, 1)?;
        let enable = self.get_int_param(action, 2)? != 0;
        log::debug!(
            "Affecting object panel flag '{}' -> {} for '{}'",
            flag_name,
            enable,
            unit_name
        );
        // Live host path: leftover OBJECT_REGISTRY is empty. Queue by script name.
        super::request_host_object_panel_flag(&unit_name, &flag_name, enable);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        {
            enum _ObjFlow<T> { Cont, Ret(T), Fall }
            let _flow = OBJECT_REGISTRY.with_object_mut(object_id, |mut obj| {
                _ObjFlow::Ret(Ok(ScriptActionResult::Success))
            });
            match _flow {
                None => { return Ok(ScriptActionResult::Success); }
                Some(_ObjFlow::Cont) => return Ok(ScriptActionResult::Success),
                Some(_ObjFlow::Ret(v)) => return v,
                Some(_ObjFlow::Fall) => {}
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unit_spawn_named_location_orientation(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let team_name = self.get_string_param(action, 2)?;
        let position = self.get_coord_param(action, 3)?;
        let angle = self.get_real_param(action, 4)?;
        log::debug!(
            "Spawning named '{}' type '{}' on team '{}' at ({}, {}, {}) angle {}",
            unit_name,
            object_type,
            team_name,
            position.x,
            position.y,
            position.z,
            angle
        );

        let unit_name_opt = {
            let trimmed = unit_name.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        };

        if let Some(name) = unit_name_opt {
            let tracker = get_named_object_tracker();
            if let Ok(Some(old_object_id)) = tracker.get_object_id(name) {
                let alive = OBJECT_REGISTRY
                    .with_object(old_object_id, |o| !o.is_effectively_dead())
                    .unwrap_or(false);
                if alive {
                        log::warn!(
                            "WARNING - Object with name '{}' already exists. Failed Create.",
                            name
                        );
                        return Ok(ScriptActionResult::Success);
                }
            }
        }

        if super::dual_world_registry_unavailable() {
            if let Some(name) = unit_name_opt {
                if crate::scripting::host_script_named_unit_alive(name) == Some(true) {
                    log::warn!(
                        "WARNING - Object with name '{}' already exists. Failed Create.",
                        name
                    );
                    return Ok(ScriptActionResult::Success);
                }
            }
            super::request_host_script_create(super::HostScriptCreateRequest::Object {
                name: unit_name_opt.map(str::to_string),
                thing: object_type,
                team: team_name,
                x: position.x,
                y: position.y,
                z: position.z,
                angle,
            });
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = match self.get_or_create_team_by_name(&team_name) {
            Ok(team) => team,
            Err(_) => return Ok(ScriptActionResult::Success),
        };

        let object_id = {
            let manager_arc = get_object_manager();
            let Ok(mut manager) = manager_arc.write() else {
                return Ok(ScriptActionResult::Success);
            };
            let spawn_pos = crate::common::Coord3D::new(position.x, position.y, position.z);
            match manager.create_object(
                &object_type,
                spawn_pos,
                Some(team_arc),
                crate::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(_) => return Ok(ScriptActionResult::Success),
            }
        };

        let _ = crate::team::with_team_mut(team_arc, |team| {
            team.add_member(object_id);
        });

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                let _ = obj.set_orientation(angle);
                if let Some(name) = unit_name_opt {
                    obj.set_name(AsciiString::from(name));
                }
            });

        if let Some(name) = unit_name_opt {
            let tracker = get_named_object_tracker();
            if let Ok(Some(old_object_id)) = tracker.get_object_id(name) {
                let _ = tracker.unregister_object(old_object_id);
            }
            let _ = tracker.register_named_object(name.to_string(), object_id);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_create_unnamed_on_team_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_type = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Creating unnamed '{}' on team '{}' at waypoint '{}'",
            object_type,
            team_name,
            waypoint
        );
        let _ = self.create_unit_on_team_at_waypoint(None, &object_type, &team_name, &waypoint)?;
        Ok(ScriptActionResult::Success)
    }
}
