//! Remaining player construction, relations, science, and rank script actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptActionDispatcher {
    // ============================================================================
    // ADDITIONAL PLAYER ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_player_sell_everything(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' selling everything", player_name);
        crate::scripting::executor::request_host_script_player_misc(
            crate::scripting::executor::HostScriptPlayerMiscRequest::SellEverything {
                player: player_name.clone(),
            },
        );

        let object_ids = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().map(|p| p.get_all_objects()))
            .unwrap_or_default();

        let frame = TheGameLogic::get_frame();
        for object_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let sell_obj = if let Ok(obj_guard) = obj_arc.read() {
                // C++ Player::sellEverythingUnderTheSun -> sellBuildings():
                // faction structures, command centers, and FS power plants.
                if obj_guard.is_effectively_dead()
                    || !(obj_guard.is_faction_structure()
                        || obj_guard.is_kind_of(crate::common::KindOf::CommandCenter)
                        || obj_guard.is_kind_of(crate::common::KindOf::FSPower))
                {
                    continue;
                }
                game_engine::common::system::build_assistant::Object {
                    id: obj_guard.get_id(),
                    position: game_engine::common::system::build_assistant::Coord3D {
                        x: obj_guard.get_position().x,
                        y: obj_guard.get_position().y,
                        z: obj_guard.get_position().z,
                    },
                    orientation: obj_guard.get_orientation(),
                    command_set: None,
                }
            } else {
                continue;
            };

            let Some(mut assistant) =
                game_engine::common::system::build_assistant::get_build_assistant()
            else {
                break;
            };
            assistant.sell_object(&sell_obj, frame);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_disable_base_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Disabling base construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_base(false);
                }
            }
        }
        crate::scripting::executor::request_host_can_build(
            crate::scripting::executor::HostScriptCanBuildRequest::Base {
                player: player_name,
                enable: false,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_disable_factories(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let object_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Disabling factories '{}' for '{}'",
            object_name,
            player_name
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_objects_enabled(&object_name, false);
                }
            }
        }
        crate::scripting::executor::request_host_can_build(
            crate::scripting::executor::HostScriptCanBuildRequest::Factories {
                player: player_name,
                template: object_name,
                enable: false,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_disable_unit_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Disabling unit construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_units(false);
                }
            }
        }
        crate::scripting::executor::request_host_can_build(
            crate::scripting::executor::HostScriptCanBuildRequest::Units {
                player: player_name,
                enable: false,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_enable_base_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Enabling base construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_base(true);
                }
            }
        }
        crate::scripting::executor::request_host_can_build(
            crate::scripting::executor::HostScriptCanBuildRequest::Base {
                player: player_name,
                enable: true,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_enable_factories(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let object_name = self.get_string_param(action, 1)?;
        log::debug!("Enabling factories '{}' for '{}'", object_name, player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_objects_enabled(&object_name, true);
                }
            }
        }
        crate::scripting::executor::request_host_can_build(
            crate::scripting::executor::HostScriptCanBuildRequest::Factories {
                player: player_name,
                template: object_name,
                enable: true,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_enable_unit_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Enabling unit construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_units(true);
                }
            }
        }
        crate::scripting::executor::request_host_can_build(
            crate::scripting::executor::HostScriptCanBuildRequest::Units {
                player: player_name,
                enable: true,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_transfer_ownership_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let from_player = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let to_player = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_transfer(super::HostScriptTransferRequest::Player {
                from: from_player,
                to: to_player,
            });
            return Ok(ScriptActionResult::Success);
        }

        log::debug!(
            "Transferring ownership from '{}' to '{}'",
            from_player,
            to_player
        );

        let (source_player, dest_player) = if let Ok(players) = player_list().read() {
            (
                players.find_player_by_name(&from_player),
                players.find_player_by_name(&to_player),
            )
        } else {
            (None, None)
        };
        let (Some(source_player), Some(dest_player)) = (source_player, dest_player) else {
            return Ok(ScriptActionResult::Success);
        };

        let destination_team = dest_player
            .read()
            .ok()
            .and_then(|player| player.get_default_team());
        let Some(destination_team) = destination_team else {
            return Ok(ScriptActionResult::Success);
        };

        let source_object_ids = source_player
            .read()
            .ok()
            .map(|player| player.get_all_objects())
            .unwrap_or_default();

        let source_money = if let Ok(mut src_guard) = source_player.write() {
            let amount = src_guard.get_money().get_money();
            src_guard.get_money_mut().set_money(0);
            amount
        } else {
            0
        };
        if source_money != 0 {
            if let Ok(mut dst_guard) = dest_player.write() {
                dst_guard.get_money_mut().add_money(source_money);
            }
        }

        for object_id in source_object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            if let Ok(mut obj_guard) = obj_arc.write() {
                let old_owner = obj_guard.get_controlling_player();
                let _ = obj_guard.set_team(Some(destination_team.clone()));
                let new_owner = obj_guard.get_controlling_player();
                obj_guard.on_capture(old_owner, new_owner);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_relates_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player1 = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let player2 = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Player '{}' relation to '{}' ({})",
            player1,
            player2,
            relation
        );

        let (source_player, target_player_index) = if let Ok(players) = player_list().read() {
            (
                players.find_player_by_name(&player1),
                players
                    .find_player_by_name(&player2)
                    .and_then(|player| player.read().ok().map(|player| player.get_player_index())),
            )
        } else {
            (None, None)
        };
        if let (Some(source_player), Some(target_player_index)) =
            (source_player, target_player_index)
        {
            if let Ok(mut source_guard) = source_player.write() {
                source_guard.set_player_relationship_by_index(target_player_index, relationship);
            }
        }

        request_host_player_relates(HostScriptPlayerRelatesRequest {
            source: player1,
            dest: player2,
            relationship,
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_set_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Player '{}' override relation to team '{}' ({})",
            player_name,
            team_name,
            relation
        );

        let player_arc = if let Ok(players) = player_list().read() {
            players.find_player_by_name(&player_name)
        } else {
            None
        };
        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        if let (Some(player_arc), Some(team_arc)) = (player_arc, team_arc) {
            if let Ok(team_guard) = team_arc.read() {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_team_relationship(&team_guard, relationship);
                }
            };
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::SetPlayerToTeam {
                source_player: player_name,
                dest_team: team_name,
                relationship,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_remove_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Player '{}' remove override relation to team '{}'",
            player_name,
            team_name
        );

        let player_arc = if let Ok(players) = player_list().read() {
            players.find_player_by_name(&player_name)
        } else {
            None
        };
        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        if let (Some(player_arc), Some(team_arc)) = (player_arc, team_arc) {
            if let Ok(team_guard) = team_arc.read() {
                if let Ok(mut player_guard) = player_arc.write() {
                    let _ = player_guard.remove_team_relationship(&team_guard);
                }
            };
        }

        crate::scripting::request_host_team_override_relation(
            crate::scripting::HostScriptTeamOverrideRelationRequest::RemovePlayerToTeam {
                source_player: player_name,
                dest_team: team_name,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_garrison_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' garrisoning all buildings", player_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::PlayerGarrisonAll {
                    player: player_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let object_ids = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().map(|p| p.get_all_objects()))
            .unwrap_or_default();

        for object_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            if let Ok(mut obj_guard) = obj_arc.write() {
                if obj_guard.is_kind_of(crate::common::KindOf::Structure)
                    || !obj_guard.is_kind_of(crate::common::KindOf::Infantry)
                    || obj_guard.is_kind_of(crate::common::KindOf::NoGarrison)
                {
                    continue;
                }
                let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                    continue;
                };
                obj_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                    let _ = ai_guard.execute_command(&params);
                };
            };
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_exit_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' exiting all buildings", player_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::PlayerExitAll {
                    player: player_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let object_ids = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().map(|p| p.get_all_objects()))
            .unwrap_or_default();

        for object_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            if let Ok(mut obj_guard) = obj_arc.write() {
                if obj_guard.is_kind_of(crate::common::KindOf::Structure) {
                    continue;
                }
                let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                    continue;
                };
                obj_guard.leave_group();
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

    pub(crate) fn do_player_create_team_from_captured_units(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Player '{}' creating team '{}' from captured units",
            player_name,
            team_name
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_add_skillpoints(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let points = self.get_int_param(action, 1)?;
        log::info!("Player '{}' adding {} skill points", player_name, points);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.add_skill_points(points);
                    log::info!("Player '{}' skill points added", player_name);
                }
            } else {
                log::warn!("Player '{}' not found for add skill points", player_name);
            }
        }

        crate::scripting::executor::request_host_rank(HostScriptRankRequest::AddSkillPoints {
            player: player_name,
            delta: points,
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_add_ranklevel(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let levels = self.get_int_param(action, 1)?;
        log::info!("Player '{}' adding {} rank levels", player_name, levels);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    let current_level = player_guard.get_rank_level();
                    player_guard.set_rank_level(current_level + levels);
                    log::info!(
                        "Player '{}' rank level now {}",
                        player_name,
                        current_level + levels
                    );
                }
            } else {
                log::warn!("Player '{}' not found for add rank level", player_name);
            }
        }

        crate::scripting::executor::request_host_rank(HostScriptRankRequest::AddRankLevel {
            player: player_name,
            delta: levels,
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_set_ranklevel(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let level = self.get_int_param(action, 1)?;
        log::info!("Player '{}' setting rank level to {}", player_name, level);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_rank_level(level);
                    log::info!("Player '{}' rank level set to {}", player_name, level);
                }
            } else {
                log::warn!("Player '{}' not found for set rank level", player_name);
            }
        }

        crate::scripting::executor::request_host_rank(HostScriptRankRequest::SetRankLevel {
            player: player_name,
            level,
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_set_ranklevellimit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let limit = self.get_int_param(action, 0)?;
        log::debug!("Setting map rank level limit to {}", limit);
        TheGameLogic::set_rank_level_limit(limit);
        crate::scripting::executor::request_host_rank(HostScriptRankRequest::SetRankLevelLimit {
            limit,
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_purchase_science(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let science_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Player '{}' purchasing science '{}'",
            player_name,
            science_name
        );

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    let _ = player_guard.attempt_to_purchase_science(science_type);
                };
            } else {
                log::warn!("Player '{}' not found for purchase science", player_name);
            }
        }
        crate::scripting::executor::request_host_science_action(&player_name, &science_name, false);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_repair_named_structure(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let structure_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Player '{}' repairing structure '{}'",
            player_name,
            structure_name
        );
        crate::scripting::executor::request_host_script_player_misc(
            crate::scripting::executor::HostScriptPlayerMiscRequest::RepairNamed {
                player: player_name.clone(),
                structure: structure_name.clone(),
            },
        );

        let tracker = get_named_object_tracker();
        let Some(structure_id) = tracker.get_object_id(&structure_name).ok().flatten() else {
            log::warn!("Named structure '{}' not found for repair", structure_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.repair_structure(structure_id);
                };
            } else {
                log::warn!("Player '{}' not found for repair structure", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_affect_receiving_experience(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let modifier = self.get_real_param(action, 1)?;
        log::debug!(
            "Affecting experience receiving for '{}' modifier {}",
            player_name,
            modifier
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_skill_points_modifier(modifier);
                }
            }
        }

        crate::scripting::executor::request_host_rank(
            HostScriptRankRequest::AffectReceivingExperience {
                player: player_name,
                modifier,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_exclude_from_score_screen(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Excluding '{}' from score screen", player_name);
        crate::scripting::executor::request_host_script_player_misc(
            crate::scripting::executor::HostScriptPlayerMiscRequest::ExcludeFromScore {
                player: player_name.clone(),
            },
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_list_in_score_screen(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_science_availability(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let science_name = self.get_string_param(action, 1)?;
        let availability_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Setting science '{}' availability '{}' for '{}'",
            science_name,
            availability_name,
            player_name
        );

        let Some(availability_type) =
            crate::player::Player::get_science_availability_type_from_string(&availability_name)
        else {
            log::warn!(
                "Invalid science availability '{}' for '{}'",
                availability_name,
                science_name
            );
            return Ok(ScriptActionResult::Success);
        };

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_science_availability(science_type, availability_type);
                };
            } else {
                log::warn!(
                    "Player '{}' not found for science availability",
                    player_name
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_select_skillset(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let mut skillset = self.get_int_param(action, 1)?;
        log::debug!("Player '{}' selecting skillset {}", player_name, skillset);
        crate::scripting::executor::request_host_script_player_misc(
            crate::scripting::executor::HostScriptPlayerMiscRequest::SelectSkillset {
                player: player_name.clone(),
                skillset,
            },
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    // Script uses 1-based skillset numbering; AI uses zero-based.
                    skillset -= 1;
                    player_guard.friend_set_skillset(skillset);
                };
            } else {
                log::warn!("Player '{}' not found for select skillset", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }
}
