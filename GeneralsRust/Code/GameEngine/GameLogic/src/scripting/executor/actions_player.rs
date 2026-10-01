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

        let object_ids = crate::player::with_player_named(&player_name, |player| player.get_all_objects())
            .unwrap_or_default();

        let frame = TheGameLogic::get_frame();
        for object_id in object_ids {
            let sell_obj = crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                // C++ Player::sellEverythingUnderTheSun -> sellBuildings():
                // faction structures, command centers, and FS power plants.
                if obj_guard.is_effectively_dead()
                    || !(obj_guard.is_faction_structure()
                        || obj_guard.is_kind_of(crate::common::KindOf::CommandCenter)
                        || obj_guard.is_kind_of(crate::common::KindOf::FSPower))
                {
                    return None;
                }
                Some(game_engine::common::system::build_assistant::Object {
                    id: obj_guard.get_id(),
                    position: game_engine::common::system::build_assistant::Coord3D {
                        x: obj_guard.get_position().x,
                        y: obj_guard.get_position().y,
                        z: obj_guard.get_position().z,
                    },
                    orientation: obj_guard.get_orientation(),
                    command_set: None,
                })
            });
            let Some(sell_obj) = sell_obj.flatten() else {
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_can_build_base(false);
        });
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_objects_enabled(&object_name, false);
        });
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_can_build_units(false);
        });
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_can_build_base(true);
        });
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_objects_enabled(&object_name, true);
        });
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_can_build_units(true);
        });
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

        let destination_team = crate::player::with_player_named(&to_player, |player| player.get_default_team())
            .flatten();
        let Some(destination_team) = destination_team else {
            return Ok(ScriptActionResult::Success);
        };

        let source_object_ids = crate::player::with_player_named(&from_player, |player| player.get_all_objects())
            .unwrap_or_default();

        let source_money = crate::player::with_player_named_mut(&from_player, |player| {
            let amount = player.get_money().get_money();
            player.get_money_mut().set_money(0);
            amount
        })
        .unwrap_or(0);
        if source_money != 0 {
            let _ = crate::player::with_player_named_mut(&to_player, |player| {
                player.get_money_mut().add_money(source_money);
            });
        }

        for object_id in source_object_ids {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                let old_owner = obj_guard.get_controlling_player();
                let _ = obj_guard.set_team(Some(destination_team));
                let new_owner = obj_guard.get_controlling_player();
                obj_guard.on_capture(old_owner, new_owner);
            });
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

        let target_player_index = crate::player::with_player_named(&player2, |player| player.get_player_index());
        if let Some(target_player_index) = target_player_index {
            let _ = crate::player::with_player_named_mut(&player1, |player| {
                player.set_player_relationship_by_index(target_player_index, relationship);
            });
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

        let team_id = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        if let Some(team_id) = team_id {
            let _ = crate::player::with_player_named_mut(&player_name, |player| {
                player.set_team_relationship_by_id(team_id, relationship);
            });
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

        let team_id = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        if let Some(team_id) = team_id {
            let _ = crate::player::with_player_named_mut(&player_name, |player| {
                let _ = crate::team::with_team(team_id, |team| {
                    let _ = player.remove_team_relationship(team);
                });
            });
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

        let object_ids = crate::player::with_player_named(&player_name, |player| player.get_all_objects())
            .unwrap_or_default();

        for object_id in object_ids {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if obj_guard.is_kind_of(crate::common::KindOf::Structure)
                    || !obj_guard.is_kind_of(crate::common::KindOf::Infantry)
                    || obj_guard.is_kind_of(crate::common::KindOf::NoGarrison)
                {
                    return;
                }
                if obj_guard.get_ai_update_interface().is_none() {
                    return;
                }
                obj_guard.leave_group();
                if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                    let _ = ai.execute_command(&params);
                }
            });
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

        let object_ids = crate::player::with_player_named(&player_name, |player| player.get_all_objects())
            .unwrap_or_default();

        for object_id in object_ids {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if obj_guard.is_kind_of(crate::common::KindOf::Structure) {
                    return;
                }
                if obj_guard.get_ai_update_interface().is_none() {
                    return;
                }
                obj_guard.leave_group();
                if let Some(ai) = obj_guard.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromScript);
                    let _ = ai.execute_command(&params);
                }
            });
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

        if crate::player::with_player_named_mut(&player_name, |player| {
            player.add_skill_points(points);
            log::info!("Player '{}' skill points added", player_name);
        })
        .is_none()
        {
            log::warn!("Player '{}' not found for add skill points", player_name);
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

        if crate::player::with_player_named_mut(&player_name, |player| {
            let current_level = player.get_rank_level();
            player.set_rank_level(current_level + levels);
            log::info!(
                "Player '{}' rank level now {}",
                player_name,
                current_level + levels
            );
        })
        .is_none()
        {
            log::warn!("Player '{}' not found for add rank level", player_name);
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

        if crate::player::with_player_named_mut(&player_name, |player| {
            player.set_rank_level(level);
            log::info!("Player '{}' rank level set to {}", player_name, level);
        })
        .is_none()
        {
            log::warn!("Player '{}' not found for set rank level", player_name);
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

        if crate::player::with_player_named_mut(&player_name, |player| {
            let _ = player.attempt_to_purchase_science(science_type);
        })
        .is_none()
        {
            log::warn!("Player '{}' not found for purchase science", player_name);
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

        if crate::player::with_player_named_mut(&player_name, |player| {
            player.repair_structure(structure_id);
        })
        .is_none()
        {
            log::warn!("Player '{}' not found for repair structure", player_name);
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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_skill_points_modifier(modifier);
        });

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

        let _ = crate::player::with_player_named_mut(&player_name, |player| {
            player.set_list_in_score_screen(false);
        });

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

        if crate::player::with_player_named_mut(&player_name, |player| {
            player.set_science_availability(science_type, availability_type);
        })
        .is_none()
        {
            log::warn!(
                "Player '{}' not found for science availability",
                player_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_select_skillset(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let skillset = self.get_int_param(action, 1)?;
        log::debug!("Player '{}' selecting skillset {}", player_name, skillset);
        crate::scripting::executor::request_host_script_player_misc(
            crate::scripting::executor::HostScriptPlayerMiscRequest::SelectSkillset {
                player: player_name.clone(),
                skillset,
            },
        );

        if crate::player::with_player_named_mut(&player_name, |player| {
            // Script uses 1-based skillset numbering; AI uses zero-based.
            player.friend_set_skillset(skillset - 1);
        })
        .is_none()
        {
            log::warn!("Player '{}' not found for select skillset", player_name);
        }

        Ok(ScriptActionResult::Success)
    }
}
