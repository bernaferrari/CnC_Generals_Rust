//! Team command-button, capture, panel-flag, unmanned, boobytrap, and face actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptActionDispatcher {
    /// C++ Reference: ScriptActions::doTeamHuntWithCommandButton() (ScriptActions.cpp ~2003-2147)
    ///
    /// Validates that `ability` is a hunt-capable GUI command, then for each living team
    /// member with AI + that button in its command set, calls CommandButtonHuntUpdate::setCommandButton.
    pub(crate) fn do_team_hunt_with_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let command_button_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' hunting with command button '{}'",
            team_name,
            command_button_name
        );
        // Live host path: leftover OBJECT_REGISTRY is empty. Queue so GameLogic
        // can arm CommandButtonHuntUpdate instead of collapsing to plain Hunt.
        if super::dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(
                super::HostScriptHuntGuardRequest::TeamHuntWithCommandButton {
                    team: team_name,
                    button: command_button_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        // C++: Team *theTeam = TheScriptEngine->getTeamNamed(teamName); if (!theTeam) return;
        let Ok(team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();

        // C++: TheControlBar->findCommandButton(ability); if (!commandButton) return;
        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(command_button) = control_bar.find_command_button_by_name(&command_button_name)
        else {
            return Ok(ScriptActionResult::Success);
        };

        if !Self::command_button_is_hunt_capable(command_button, &command_button_name) {
            return Ok(ScriptActionResult::Success);
        }

        // C++: iterate TeamMemberList; skip units without AI; require the button in the unit's
        // command set; then CommandButtonHuntUpdate::setCommandButton(ability).
        for member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                continue;
            }
            if obj_guard.get_ai_update_interface().is_none() {
                continue;
            }

            let has_matching_command = control_bar
                .find_command_set_by_name(obj_guard.get_command_set_string())
                .map(|set| {
                    set.buttons.iter().flatten().any(|button| {
                        button.get_id() == command_button.get_id()
                            || button
                                .get_name()
                                .eq_ignore_ascii_case(command_button.get_name())
                    })
                })
                .unwrap_or(false);
            if !has_matching_command {
                log::warn!(
                    "Error - Team hunt with command button - unit type '{}' is not valid for ability {}",
                    obj_guard.get_template_name(),
                    command_button_name
                );
                continue;
            }

            let Some(module) = obj_guard.find_update_module("CommandButtonHuntUpdate") else {
                log::warn!(
                    "Error - Team hunt with command button - unit type '{}' requires CommandButtonHuntUpdate in .ini definition to hunt with {}",
                    obj_guard.get_template_name(),
                    command_button_name
                );
                continue;
            };

            let set_ok = module.with_module(|module| {
                module
                    .get_command_button_hunt_control_interface()
                    .map(|hunt| hunt.set_command_button(command_button_name.to_string()))
                    .is_some()
            });
            if !set_ok {
                log::warn!(
                    "Error - Team hunt with command button - unit type '{}' requires CommandButtonHuntUpdate in .ini definition to hunt with {}",
                    obj_guard.get_template_name(),
                    command_button_name
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ ScriptActions.cpp doTeamHuntWithCommandButton switch (~2017-2080):
    /// allow SPECIAL_POWER (object-target only), SWITCH_WEAPON, FIRE_WEAPON,
    /// HIJACK_VEHICLE, CONVERT_TO_CARBOMB, SABOTAGE_BUILDING; reject all others.
    pub(crate) fn command_button_is_hunt_capable(
        command_button: &crate::command_button::CommandButton,
        ability: &str,
    ) -> bool {
        match command_button.get_command_type() {
            CommandType::DoSpecialPower => {
                let Some(_sp_template) = command_button.get_special_power_template() else {
                    return false;
                };
                let options = SpecialPowerCommandOption::from_bits_truncate(
                    command_button.get_options_bits(),
                );
                // C++ COMMAND_OPTION_NEED_OBJECT_TARGET = ENEMY | NEUTRAL | ALLY
                let needs_object_target = options.intersects(
                    SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                );
                if !needs_object_target {
                    log::warn!(
                        "ERROR-Team hunt with command button - cannot hunt with ability {}",
                        ability
                    );
                    return false;
                }
                true
            }
            // FIRE_WEAPON -> DoAttackObject; HIJACK/SABOTAGE -> Enter
            CommandType::SwitchWeapons
            | CommandType::DoAttackObject
            | CommandType::Enter
            | CommandType::ConvertToCarbomb => true,
            _ => {
                log::warn!(
                    "ERROR-Team hunt with command button - cannot hunt with ability {}",
                    ability
                );
                false
            }
        }
    }

    pub(crate) fn do_team_use_command_button_on_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' using command '{}' on '{}'",
            team_name,
            command_button,
            target_name
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNamed {
                team: team_name.clone(),
                button: command_button.clone(),
                target: target_name.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let tracker = get_named_object_tracker();
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let can_use = {
            let Ok(src_guard) = source_obj.read() else {
                return Ok(ScriptActionResult::Success);
            };
            let Ok(target_guard) = target_obj.read() else {
                return Ok(ScriptActionResult::Success);
            };
            command_button.is_valid_to_use_on(
                &src_guard,
                Some(&target_guard),
                None,
                CommandSourceType::FromScript,
            )
        };

        if can_use {
            self.issue_group_command_button_at_object(
                &group_arc,
                command_button.get_id(),
                &target_obj,
            );
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_use_command_button_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' using command '{}' at waypoint '{}'",
            team_name,
            command_button,
            waypoint
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamAtWaypoint {
                team: team_name.clone(),
                button: command_button.clone(),
                waypoint: waypoint.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, _source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        self.issue_group_command_button_at_position(&group_arc, command_button.get_id(), &pos);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_use_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!("Team '{}' using command '{}'", team_name, command_button);
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::Team {
                team: team_name.clone(),
                button: command_button.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, _source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };
        self.issue_group_command_button(&group_arc, command_button.get_id());

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_all_use_command_button_on_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on '{}'",
            team_name,
            command_button,
            target_name
        );

        self.do_team_use_command_button_on_named(action)
    }

    pub(crate) fn do_team_all_use_command_button_on_nearest_enemy_unit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest enemy unit",
            team_name,
            command_button
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNearestEnemy {
                team: team_name.clone(),
                button: command_button.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| source.relationship_to(candidate) == Relationship::Enemies,
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_all_use_command_button_on_nearest_garrisoned_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest garrisoned building",
            team_name,
            command_button
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNearestGarrisonedBuilding {
                team: team_name.clone(),
                button: command_button.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |_source, candidate| {
                if !candidate.is_kind_of(crate::common::KindOf::Structure) {
                    return false;
                }
                candidate
                    .get_contain()
                    .and_then(|contain| contain.lock().ok().map(|c| c.is_garrisonable()))
                    .unwrap_or(false)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_all_use_command_button_on_nearest_kindof(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let kindof = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest kindof '{}'",
            team_name,
            command_button,
            kindof
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNearestKindof {
                team: team_name.clone(),
                button: command_button.clone(),
                kindof: kindof.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some(kind) = parse_kind_of(&kindof) else {
            return Ok(ScriptActionResult::Success);
        };

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                source.relationship_to(candidate) == Relationship::Enemies
                    && candidate.is_kind_of(kind)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_all_use_command_button_on_nearest_enemy_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest enemy building",
            team_name,
            command_button
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuilding {
                team: team_name.clone(),
                button: command_button.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                source.relationship_to(candidate) == Relationship::Enemies
                    && candidate.is_kind_of(crate::common::KindOf::Structure)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_all_use_command_button_on_nearest_enemy_building_class(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let building_class = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest enemy building class '{}'",
            team_name,
            command_button,
            building_class
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuildingClass {
                team: team_name.clone(),
                button: command_button.clone(),
                kindof: building_class.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let Some(kind) = parse_kind_of(&building_class) else {
            return Ok(ScriptActionResult::Success);
        };

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                source.relationship_to(candidate) == Relationship::Enemies
                    && candidate.is_kind_of(crate::common::KindOf::Structure)
                    && candidate.is_kind_of(kind)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_all_use_command_button_on_nearest_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let object_type = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest object type '{}'",
            team_name,
            command_button,
            object_type
        );
        super::request_host_script_use_command_button(
            super::HostScriptUseCommandButtonRequest::TeamOnNearestObjectType {
                team: team_name.clone(),
                button: command_button.clone(),
                object_type: object_type.clone(),
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let wanted_types = self.resolve_object_types_for_action(&object_type);
        if wanted_types.list_size() == 0 {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                let rel = source.relationship_to(candidate);
                if !matches!(rel, Relationship::Enemies | Relationship::Neutral) {
                    return false;
                }
                let template_ref: &dyn crate::common::ThingTemplate =
                    candidate.get_template().as_ref();
                wanted_types.contains_template(Some(template_ref))
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_partial_use_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let percentage = self.get_real_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let command_button_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' partial use command '{}' at {}%",
            team_name,
            command_button_name,
            percentage
        );
        super::request_host_team_partial_command_button(
            super::HostScriptTeamPartialCommandButtonRequest {
                team: team_name.clone(),
                button: command_button_name.clone(),
                percentage,
            },
        );
        if super::dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = if let Ok(team_guard) = team_arc.read() {
            team_guard.get_members().to_vec()
        } else {
            Vec::new()
        };
        if members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(&command_button_name)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let command_button = command_button.clone();

        let mut valid_members = Vec::new();
        for member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if command_button.is_valid_to_use_on(
                &obj_guard,
                None,
                None,
                CommandSourceType::FromScript,
            ) {
                valid_members.push(member_id);
            }
        }

        if valid_members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let mut num_to_use = ((percentage / 100.0) * valid_members.len() as f32) as i32;
        if num_to_use <= 0 {
            return Ok(ScriptActionResult::Success);
        }
        if num_to_use > valid_members.len() as i32 {
            num_to_use = valid_members.len() as i32;
        }

        let mut count = 0;
        for member_id in valid_members {
            if count >= num_to_use {
                break;
            }
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let _ =
                obj_guard.do_command_button(command_button.get_id(), CommandSourceType::FromScript);
            count += 1;
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_capture_nearest_unowned_faction_unit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!(
            "Team '{}' capturing nearest unowned faction unit",
            team_name
        );
        // Live host path: leftover partition / leftover crate objects are empty.
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_capture_nearest_unowned(&team_name);
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = self.get_team_by_name(&team_name)?;
        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let Some(group_center) = group_arc.read().ok().and_then(|group| group.get_center()) else {
            return Ok(ScriptActionResult::Success);
        };

        let controlling_player_arc = team_arc
            .read()
            .ok()
            .and_then(|team| team.get_controlling_player_id())
            .and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as i32).cloned())
            });

        let target_id = ThePartitionManager::get().and_then(|partition| {
            partition.get_closest_object_2d(&group_center, 1_000_000.0, |candidate| {
                if candidate.is_effectively_dead() || candidate.is_off_map() {
                    return false;
                }
                if !candidate.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned) {
                    return false;
                }

                let relationship = if let Some(player_arc) = &controlling_player_arc {
                    if let Ok(player_guard) = player_arc.read() {
                        if let Some(target_team_arc) = candidate.get_team() {
                            if let Ok(target_team_guard) = target_team_arc.read() {
                                player_guard.get_relationship_with_team(&target_team_guard)
                            } else {
                                Relationship::Neutral
                            }
                        } else {
                            Relationship::Neutral
                        }
                    } else {
                        Relationship::Neutral
                    }
                } else {
                    Relationship::Neutral
                };

                matches!(relationship, Relationship::Enemies | Relationship::Neutral)
            })
        });

        if let Some(target_id) = target_id {
            if let Ok(mut group) = group_arc.write() {
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(target_id);
                let _ = group.ai_do_command(&params);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn resolve_team_command_button_context(
        &self,
        team_name: &str,
        ability: &str,
    ) -> Result<
        Option<(
            Arc<RwLock<AiGroup>>,
            crate::command_button::CommandButton,
            Arc<RwLock<crate::object::Object>>,
        )>,
        ScriptError,
    > {
        let resolved_team = self.resolve_team_name_token(team_name);
        let group_arc = self.create_ai_group_from_team(&resolved_team)?;

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(ability) else {
            return Ok(None);
        };
        let command_button = command_button.clone();

        let source_obj = if let Some(template) = command_button.get_special_power_template() {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_special_power_source_object(template.get_id()))
        } else {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_command_button_source_object(command_button.get_id()))
        };
        let Some(source_obj) = source_obj else {
            return Ok(None);
        };

        Ok(Some((group_arc, command_button, source_obj)))
    }

    pub(crate) fn group_member_ids(&self, group_arc: &Arc<RwLock<AiGroup>>) -> Vec<ObjectID> {
        if let Ok(group) = group_arc.read() {
            group.get_all_ids().clone()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn issue_group_command_button(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        button_id: u32,
    ) {
        for member_id in self.group_member_ids(group_arc) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let _ = obj_guard.do_command_button(button_id, CommandSourceType::FromScript);
        }
    }

    pub(crate) fn issue_group_command_button_at_position(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        button_id: u32,
        pos: &Coord3D,
    ) {
        for member_id in self.group_member_ids(group_arc) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let _ = obj_guard.do_command_button_at_position(
                button_id,
                pos,
                CommandSourceType::FromScript,
            );
        }
    }

    pub(crate) fn issue_group_command_button_at_object(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        button_id: u32,
        target: &Arc<RwLock<crate::object::Object>>,
    ) {
        let Ok(target_guard) = target.read() else {
            return;
        };
        for member_id in self.group_member_ids(group_arc) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let _ = obj_guard.do_command_button_at_object(
                button_id,
                &target_guard,
                CommandSourceType::FromScript,
            );
        }
    }

    pub(crate) fn find_nearest_command_button_target<F>(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        source_obj: &Arc<RwLock<crate::object::Object>>,
        command_button: &crate::command_button::CommandButton,
        mut extra_filter: F,
    ) -> Option<ObjectID>
    where
        F: FnMut(&crate::object::Object, &crate::object::Object) -> bool,
    {
        let group_center = group_arc.read().ok().and_then(|group| group.get_center())?;
        let source_guard = source_obj.read().ok()?;
        let source_id = source_guard.get_id();
        let source_off_map = source_guard.is_off_map();

        let partition = ThePartitionManager::get()?;
        partition.get_closest_object_2d(&group_center, 1_000_000.0, |candidate| {
            if candidate.get_id() == source_id {
                return false;
            }
            if candidate.is_effectively_dead() || candidate.is_destroyed() {
                return false;
            }
            if candidate.is_off_map() != source_off_map {
                return false;
            }
            if !extra_filter(&source_guard, candidate) {
                return false;
            }
            command_button.is_valid_to_use_on(
                &source_guard,
                Some(candidate),
                None,
                CommandSourceType::FromScript,
            )
        })
    }

    pub(crate) fn do_team_affect_object_panel_flags(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let flag_name = self.get_string_param(action, 1)?;
        let enable = self.get_int_param(action, 2)? != 0;
        log::debug!(
            "Team '{}' affecting object panel flag '{}' -> {}",
            team_name,
            flag_name,
            enable
        );
        // Live host path: leftover team factory is empty. Queue by team name.
        super::request_host_team_panel_flag(&team_name, &flag_name, enable);

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = if let Ok(team_guard) = team_arc.read() {
                    team_guard.get_members().to_vec()
                } else {
                    Vec::new()
                };
                for object_id in members {
                    if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                        if let Ok(mut obj) = obj_arc.write() {
                            self.apply_object_panel_flag_for_single_object(
                                &mut obj, &flag_name, enable,
                            );
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_unmanned_status(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let team_name = self.resolve_team_name_token(&team_name);
        log::debug!("Team '{}' set unmanned", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_unmanned(super::HostScriptUnmannedRequest::Team {
                team: team_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    self.mark_object_unmanned(object_id);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_set_boobytrapped(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let boobytrap_template = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' set boobytrapped using template '{}'",
            team_name,
            boobytrap_template
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_boobytrap(super::HostScriptBoobytrapRequest::Team {
                thing: boobytrap_template,
                team: team_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let _ = self.attach_boobytrap_to_object(&boobytrap_template, object_id);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFaceNamed()
    /// Makes team members face towards a named object
    pub(crate) fn do_team_face_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' facing '{}'", team_name, target_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_face(super::HostScriptFaceRequest::TeamFaceNamed {
                team: team_name,
                target: target_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            log::warn!("Target '{}' not found for team face", target_name);
            return Ok(ScriptActionResult::Success);
        };
        if TheGameLogic::find_object_by_id(target_id).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let team_name = self.resolve_team_name_token(&team_name);
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
                        let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                            continue;
                        };
                        obj_guard.leave_group();
                        if let Ok(mut ai_guard) = ai_arc.lock() {
                            let _ = ai_guard
                                .choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            let mut params = AiCommandParams::new(
                                AiCommandType::FaceObject,
                                CommandSourceType::FromScript,
                            );
                            params.obj = Some(target_id);
                            let _ = ai_guard.execute_command(&params);
                        };
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFaceWaypoint()
    /// Makes team members face towards a waypoint
    pub(crate) fn do_team_face_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' facing waypoint '{}'", team_name, waypoint_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_face(super::HostScriptFaceRequest::TeamFaceWaypoint {
                team: team_name,
                waypoint: waypoint_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let waypoint_pos = self.get_waypoint_position(&waypoint_name)?;
        let waypoint_pos =
            crate::common::Coord3D::new(waypoint_pos.x, waypoint_pos.y, waypoint_pos.z);

        let team_name = self.resolve_team_name_token(&team_name);
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
                        let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                            continue;
                        };
                        obj_guard.leave_group();
                        if let Ok(mut ai_guard) = ai_arc.lock() {
                            let _ = ai_guard
                                .choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            let mut params = AiCommandParams::new(
                                AiCommandType::FacePosition,
                                CommandSourceType::FromScript,
                            );
                            params.pos = waypoint_pos;
                            let _ = ai_guard.execute_command(&params);
                        };
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }
}
