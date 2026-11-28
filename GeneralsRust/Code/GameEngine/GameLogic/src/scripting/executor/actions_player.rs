//! Remaining player construction, relations, science, and rank script actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;
use crate::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
use generals_sim::science::ScienceAvailabilityType;
use generals_sim::scripting::world::{ScriptPlayerConstructionRequest, ScriptPlayerRankRequest};

impl ScriptActionDispatcher<'_> {
    // ============================================================================
    // ADDITIONAL PLAYER ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(super) fn do_player_sell_everything(
        &mut self,
        player: &str,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player);
        log::debug!("Player '{}' selling everything", player_name);

        let __player_list_arc = player_list();
            let object_ids = __player_list_arc.read()
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

    pub(super) fn do_player_construction(
        &self,
        request: ScriptPlayerConstructionRequest,
    ) -> Result<ScriptActionResult, ScriptError> {
        use ScriptPlayerConstructionRequest as Build;
        let token = match &request {
            Build::Units { player, .. }
            | Build::Base { player, .. }
            | Build::Factories { player, .. } => player,
        };
        let player_name = self.resolve_player_name_token(token);
        let list = player_list();
        if let Ok(list) = list.read() {
            if let Some(player) = list.find_player_by_name(&player_name) {
                if let Ok(mut player) = player.write() {
                    match request {
                        Build::Units { enabled, .. } => player.set_can_build_units(enabled),
                        Build::Base { enabled, .. } => player.set_can_build_base(enabled),
                        Build::Factories { template, enabled, .. } => {
                            player.set_objects_enabled(&template, enabled);
                        }
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(super) fn do_player_transfer_ownership_player(
        &mut self,
        source: &str,
        destination: &str,
    ) -> Result<ScriptActionResult, ScriptError> {
        let from_player = self.resolve_player_name_token(source);
        let to_player = self.resolve_player_name_token(destination);
        if super::dual_world_registry_unavailable() {
            return Err(ScriptError::ExecutionFailed(
                "Player asset transfer requires its receiving world".into(),
            ));
        }

        log::debug!(
            "Transferring ownership from '{}' to '{}'",
            from_player,
            to_player
        );

        let __player_list_arc = player_list();
        let (source_player, dest_player) = if let Ok(players) = __player_list_arc.read() {
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

    pub(super) fn do_player_relates_player(
        &self,
        source: &str,
        destination: &str,
        relationship: Relationship,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player1 = self.resolve_player_name_token(source);
        let player2 = self.resolve_player_name_token(destination);
        let roster = self.context.borrowed_engine().players();
        let (source_player, target_player_index) = if let Ok(players) = roster.read() {
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

        let __player_list_arc = player_list();
        let player_arc = if let Ok(players) = __player_list_arc.read() {
            players.find_player_by_name(&player_name)
        } else {
            None
        };
        let published_team_factory = get_team_factory();
        let team_arc = if let Ok(mut factory) = published_team_factory.lock() {
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

        let __player_list_arc = player_list();
        let player_arc = if let Ok(players) = __player_list_arc.read() {
            players.find_player_by_name(&player_name)
        } else {
            None
        };
        let published_team_factory = get_team_factory();
        let team_arc = if let Ok(mut factory) = published_team_factory.lock() {
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

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_garrison_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' garrisoning all buildings", player_name);
        let __player_list_arc = player_list();
            let object_ids = __player_list_arc.read()
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
        let __player_list_arc = player_list();
            let object_ids = __player_list_arc.read()
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

    pub(super) fn do_player_rank(
        &mut self,
        request: ScriptPlayerRankRequest,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        use ScriptPlayerRankRequest as Rank;
        let roster = Arc::clone(self.context.borrowed_engine().players());
        let token = match &request {
            Rank::SetRankLevelLimit { limit } => {
                return self.do_player_set_ranklevellimit(*limit, &roster);
            }
            Rank::AddSkillPoints { player, .. }
            | Rank::AddRankLevel { player, .. }
            | Rank::SetRankLevel { player, .. }
            | Rank::AffectReceivingExperience { player, .. } => player,
        };
        let Some(player) = self.resolve_player_name_token_with_driver(token, driver) else {
            return Ok(ScriptActionResult::Success);
        };
        match request {
            Rank::AddSkillPoints { delta, .. } => self.do_player_add_skillpoints(player, delta, &roster),
            Rank::AddRankLevel { delta, .. } => self.do_player_add_ranklevel(player, delta, &roster),
            Rank::SetRankLevel { level, .. } => self.do_player_set_ranklevel(player, level, &roster),
            Rank::AffectReceivingExperience { modifier, .. } => {
                self.do_player_affect_receiving_experience(player, modifier, &roster)
            }
            Rank::SetRankLevelLimit { .. } => unreachable!("handled without a player above"),
        }
    }

    fn do_player_add_skillpoints(
        &mut self,
        player_name: String,
        points: i32,
        roster: &RwLock<crate::player::PlayerList>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Player '{}' adding {} skill points", player_name, points);

        let listed = roster.read().ok().and_then(|list| {
            list.find_player_by_name(&player_name)
                .map(|player| (player, list.rank_level_limit()))
        });
        if let Some((player, cap)) = listed {
            if let Ok(mut player) = player.write() {
                player.add_skill_points(points, cap);
                log::info!("Player '{}' skill points added", player_name);
            }
        } else {
            log::warn!("Player '{}' not found for add skill points", player_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_add_ranklevel(
        &mut self,
        player_name: String,
        levels: i32,
        roster: &RwLock<crate::player::PlayerList>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Player '{}' adding {} rank levels", player_name, levels);

        let listed = roster.read().ok().and_then(|list| {
            list.find_player_by_name(&player_name)
                .map(|player| (player, list.rank_level_limit()))
        });
        if let Some((player, cap)) = listed {
            if let Ok(mut player) = player.write() {
                let level = player.get_rank_level() + levels;
                player.set_rank_level(level, cap);
                log::info!("Player '{}' rank level now {}", player_name, level);
            }
        } else {
            log::warn!("Player '{}' not found for add rank level", player_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_set_ranklevel(
        &mut self,
        player_name: String,
        level: i32,
        roster: &RwLock<crate::player::PlayerList>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Player '{}' setting rank level to {}", player_name, level);

        let listed = roster.read().ok().and_then(|list| {
            list.find_player_by_name(&player_name)
                .map(|player| (player, list.rank_level_limit()))
        });
        if let Some((player, cap)) = listed {
            if let Ok(mut player) = player.write() {
                player.set_rank_level(level, cap);
                log::info!("Player '{}' rank level set to {}", player_name, level);
            }
        } else {
            log::warn!("Player '{}' not found for set rank level", player_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_set_ranklevellimit(
        &mut self,
        limit: i32,
        roster: &RwLock<crate::player::PlayerList>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Setting map rank level limit to {}", limit);
        // C++ doMapSetRankLevelLimit -> TheGameLogic->setRankLevelLimit: the
        // cap is visible to the very next rank/award operation of this world.
        roster
            .write()
            .map_err(|_| ScriptError::ExecutionFailed("player roster poisoned".into()))?
            .set_rank_level_limit(limit);
        Ok(ScriptActionResult::Success)
    }

    pub(super) fn do_player_science_action(
        &mut self,
        player: &str,
        science_name: &str,
        grant: bool,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player);

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        let __player_list_arc = player_list();
        if let Ok(list_guard) = __player_list_arc.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    if grant {
                        player_guard.grant_science(science_type);
                    } else {
                        let _ = player_guard.attempt_to_purchase_science(science_type);
                    }
                };
            } else {
                log::warn!("Player '{player_name}' not found for science '{science_name}'");
            }
        }
        crate::scripting::executor::request_host_science_action(&player_name, science_name, grant);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_repair_named_structure(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let structure_name = self.get_string_param(action, 1)?;
        if let Some(result) = driver.ai_player(
            crate::scripting::engine::ScriptAiPlayerRequest::RepairNamed {
                player: &player_name,
                structure: &structure_name,
            },
        ) {
            if let Err(error) = result {
                log::warn!("Owned scripted AI player effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        log::debug!(
            "Player '{}' repairing structure '{}'",
            player_name,
            structure_name
        );

        let tracker = get_named_object_tracker();
        let Some(structure_id) = tracker.get_object_id(&structure_name).ok().flatten() else {
            log::warn!("Named structure '{}' not found for repair", structure_name);
            return Ok(ScriptActionResult::Success);
        };

        let __player_list_arc = player_list();
        if let Ok(list_guard) = __player_list_arc.read() {
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

    fn do_player_affect_receiving_experience(
        &mut self,
        player_name: String,
        modifier: f32,
        roster: &RwLock<crate::player::PlayerList>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!(
            "Affecting experience receiving for '{}' modifier {}",
            player_name,
            modifier
        );

        if let Ok(list_guard) = roster.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_skill_points_modifier(modifier);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(super) fn do_player_exclude_from_score_screen(
        &self,
        player: &str,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player);
        let roster = self.context.borrowed_engine().players();
        if let Ok(list_guard) = roster.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_list_in_score_screen(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(super) fn do_player_science_availability(
        &mut self,
        player_token: &str,
        science_name: &str,
        availability_type: ScienceAvailabilityType,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player_token);
        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        let __player_list_arc = player_list();
        if let Ok(list_guard) = __player_list_arc.read() {
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
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let Some(player_name) =
            self.resolve_player_name_token_with_driver(&self.get_string_param(action, 0)?, driver)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let mut skillset = self.get_int_param(action, 1)?;
        if let Some(result) = driver.ai_player(
            crate::scripting::engine::ScriptAiPlayerRequest::SelectSkillset {
                player: &player_name,
                script_skillset: skillset,
            },
        ) {
            if let Err(error) = result {
                log::warn!("Owned scripted AI player effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        log::debug!("Player '{}' selecting skillset {}", player_name, skillset);

        let __player_list_arc = player_list();
        if let Ok(list_guard) = __player_list_arc.read() {
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
