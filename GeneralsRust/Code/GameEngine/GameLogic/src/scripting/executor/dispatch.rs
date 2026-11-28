//! Script action dispatch, token helpers, and shared executor helpers
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;
use generals_sim::scripting::command_button_actions::ScriptAbilityAction;
use generals_sim::scripting::creation_actions::CreationAction;
use generals_sim::scripting::lifecycle_actions::LifecycleAction;
use generals_sim::scripting::navigation_actions::NavigationAction;
use generals_sim::scripting::object_actions::ObjectAction;
use generals_sim::scripting::object_appearance_actions::ObjectAppearanceAction;
use generals_sim::scripting::object_control_actions::ObjectControlAction;
use generals_sim::scripting::object_status_actions::ObjectStatusAction;
use generals_sim::scripting::order_actions::OrderAction;
use generals_sim::scripting::player_actions::PlayerAction;
use generals_sim::scripting::readiness_actions::ReadinessAction;
use generals_sim::scripting::sequence_actions::SequenceAction;
use crate::scripting::engine::ScriptOwnerQuery;

/// Core's detached world still supplies name lookup and AI. Sequence state
/// belongs to the interpreter that invoked this adapter.
struct StandaloneSequenceOwner<'a, 'engine>(&'a ScriptActionDispatcher<'engine>);

impl ScriptExecutionDriver for StandaloneSequenceOwner<'_, '_> {
    fn named_object_id(&self, name: &str, _object: Option<ObjectID>) -> ScriptOwnerQuery<ObjectID> {
        let dispatcher = self.0;
        dispatcher
            .context
            .borrowed_engine()
            .standalone_named_object_id(name)
            .or_else(|| dispatcher.host_query().host_script_named_unit_id(name))
            .map_or(ScriptOwnerQuery::Missing, ScriptOwnerQuery::Present)
    }

    fn team_for_sequence(
        &mut self,
        name: &str,
        _calling: Option<crate::team::TeamID>,
        _condition: Option<crate::team::TeamID>,
        starting: bool,
    ) -> Option<GameLogicResult<Option<crate::team::TeamID>>> {
        let dispatcher = self.0;
        let Ok(team) = dispatcher.get_team_by_name(name) else {
            return Some(Ok(None));
        };
        let id = match team.read() {
            Ok(team) => team.get_id(),
            Err(error) => return Some(Err(GameLogicError::Threading(error.to_string()))),
        };
        if starting
            && let Ok(group) = dispatcher.create_ai_group_from_team(name)
            && let Ok(mut group) = group.write()
        {
            let params = AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }
        Some(Ok(Some(id)))
    }

    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
}

impl ScriptActionDispatcher<'_> {
    pub(crate) fn resolve_player_name_token_with_driver(
        &self,
        raw: &str,
        driver: &mut dyn crate::scripting::engine::ScriptExecutionDriver,
    ) -> Option<String> {
        let current_player = self
            .context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten();
        match driver.script_player_name_token(raw, current_player.as_deref()) {
            crate::scripting::engine::ScriptOwnerQuery::Present(Some(name)) => Some(name),
            crate::scripting::engine::ScriptOwnerQuery::Present(None)
            | crate::scripting::engine::ScriptOwnerQuery::Missing => None,
            crate::scripting::engine::ScriptOwnerQuery::Unavailable => {
                Some(self.resolve_player_name_token(raw))
            }
        }
    }

    pub(crate) fn resolve_player_name_token(&self, raw: &str) -> String {
        match raw {
            THE_PLAYER => {
                if !self.context.is_generals_challenge_campaign() {
                    raw.to_string()
                } else {
                    self.context.borrowed_engine().players()
                        .read()
                        .ok()
                        .and_then(|list| list.get_local_player().cloned())
                        .and_then(|p| {
                            p.read().ok().and_then(|p| {
                                NameKeyGenerator::key_to_name(p.get_player_name_key())
                            })
                        })
                        .unwrap_or_else(|| raw.to_string())
                }
            }
            THIS_PLAYER => self
                .context
                .with_engine_ref(|engine| engine.get_current_player_name())
                .flatten()
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => self.context.borrowed_engine().players()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            _ => raw.to_string(),
        }
    }

    pub(crate) fn resolve_team_name_token(&self, raw: &str) -> String {
        match raw {
            THIS_TEAM => match &self.this_team_name {
                crate::scripting::engine::ScriptOwnerQuery::Present(name) => name.clone(),
                crate::scripting::engine::ScriptOwnerQuery::Missing => String::new(),
                crate::scripting::engine::ScriptOwnerQuery::Unavailable => self
                    .context
                    .with_engine_ref(|engine| {
                        engine
                            .get_calling_team_name()
                            .or_else(|| engine.get_condition_team_name())
                    })
                    .flatten()
                    .unwrap_or_default(),
            },
            TEAM_THE_PLAYER => {
                // C++ ScriptEngine::getTeamNamed (ScriptEngine.cpp:5935-5939).
                if !self.context.is_generals_challenge_campaign() {
                    return raw.to_string();
                }
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    /// C++ AttitudeType from ScriptAction int param (AI.h: Sleep=-2 .. Aggressive=2).
    pub(crate) fn attitude_from_script_int(&self, attitude: i32) -> AIAttitudeType {
        match attitude {
            -2 => AIAttitudeType::Sleep,
            -1 => AIAttitudeType::Passive,
            1 => AIAttitudeType::Defensive,
            2 => AIAttitudeType::Aggressive,
            _ => AIAttitudeType::Normal,
        }
    }

    pub(crate) fn group_attitude_from_script_int(&self, attitude: i32) -> AttitudeType {
        match attitude {
            -2 => AttitudeType::Sleep,
            -1 => AttitudeType::Passive,
            1 => AttitudeType::Alert,
            2 => AttitudeType::Aggressive,
            _ => AttitudeType::Normal,
        }
    }

    pub(crate) fn flash_object_by_id(
        &self,
        object_id: ObjectID,
        time_in_seconds: i32,
        color: Option<Color>,
    ) {
        // C++ ScriptActions.cpp:2655-2666 doNamedFlash: frames / DRAWABLE_FRAMES_PER_FLASH.
        if time_in_seconds <= 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };

        let (drawable_arc, flash_color) = if let Ok(object_guard) = object_arc.read() {
            (
                object_guard.get_drawable(),
                color.unwrap_or_else(|| object_guard.get_indicator_color()),
            )
        } else {
            (None, Color::white())
        };

        let Some(drawable_arc) = drawable_arc else {
            return;
        };

        if let Ok(mut drawable_guard) = drawable_arc.write() {
            super::apply_script_drawable_flash(&mut drawable_guard, time_in_seconds, flash_color);
        };
    }

    pub(crate) fn emoticon_object_by_id(
        &self,
        object_id: ObjectID,
        emoticon: &str,
        duration_frames: i32,
    ) {
        if emoticon.is_empty() || duration_frames <= 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let drawable_arc = if let Ok(object_guard) = object_arc.read() {
            object_guard.get_drawable()
        } else {
            None
        };
        let Some(drawable_arc) = drawable_arc else {
            return;
        };

        if let Ok(mut drawable_guard) = drawable_arc.write() {
            drawable_guard.script_set_emoticon(emoticon, duration_frames);
        };
    }

    pub(crate) fn resolve_special_power_template_name(&self, power_name: &str) -> Option<String> {
        let store_arc = get_special_power_store()?;
        let store = store_arc.read().ok()?;
        let template = store.find_special_power_template(power_name)?;
        Some(template.get_name().to_string())
    }

    pub(crate) fn with_named_special_power_module_mut<F>(
        &self,
        unit_name: &str,
        power_name: &str,
        func: F,
    ) where
        F: FnOnce(&mut dyn crate::modules::SpecialPowerModuleInterface),
    {
        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(unit_name) else {
            return;
        };
        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let Some(template_name) = self.resolve_special_power_template_name(power_name) else {
            return;
        };

        if let Ok(object_guard) = object_arc.read() {
            let _ = object_guard.with_special_power_module_mut_by_name(&template_name, func);
        };
    }

    /// Execute a script action
    ///
    /// C++ Reference: ScriptActions::executeAction(ScriptAction *pAction)
    pub fn execute_action(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let context = self.context;
        context.with_active_scope(|| {
            self.execute_action_with_driver(
                action,
                &mut crate::scripting::engine::CanonicalScriptExecutionDriver,
            )
        })
    }

    pub(crate) fn execute_action_with_driver(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn crate::scripting::engine::ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        if let Some(request) = PlayerAction::decode(action)? {
            let current = self.context.borrowed_engine().get_current_player_name();
            return request
                .apply_to(driver, current.as_deref())
                .unwrap_or_else(|| self.execute_standalone_player_action(request, driver));
        }
        if let Some(request) = ObjectAction::decode(action)? {
            return request
                .apply_to(driver, self.context.borrowed_engine().interpreter())
                .unwrap_or_else(|| Ok(self.execute_standalone_object_action(request)));
        }
        if let Some(request) = OrderAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = ReadinessAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = NavigationAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = CreationAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = LifecycleAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = ObjectStatusAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = ObjectAppearanceAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = ObjectControlAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = ScriptAbilityAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(request) = SequenceAction::decode(action)?
            && let Some(result) =
                request.apply_to(driver, self.context.borrowed_engine().interpreter())
        {
            return result;
        }
        if let Some(result) = self.context.borrowed_engine().interpreter()
            .try_execute_world_effect_action(action, driver)?
        {
            return Ok(result);
        }
        if let Some(result) = self.context.borrowed_engine().interpreter()
            .try_execute_presentation_action(action, driver)?
        {
            return Ok(result);
        }
        self.execute_legacy_action_with_driver(action, driver)
    }

    pub(crate) fn execute_legacy_action_with_driver(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn crate::scripting::engine::ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let action_type = action.get_action_type();
        // Immutable operation metadata comes from the driving session. A
        // missing exact reference never consults a foreign factory by ID.
        let selected = self
            .context
            .with_engine_ref(|engine| {
                engine
                    .get_calling_team_id()
                    .or_else(|| engine.get_condition_team_id())
            })
            .flatten();
        self.this_team_name = selected
            .map(|id| driver.team_name(id))
            .unwrap_or(crate::scripting::engine::ScriptOwnerQuery::Missing);

        if let Some(request) = SequenceAction::decode(action)? {
            return request
                .apply_to(
                    &mut StandaloneSequenceOwner(self),
                    self.context.borrowed_engine().interpreter(),
                )
                .unwrap_or(Ok(ScriptActionResult::Success));
        }

        if let Some(result) = self.try_execute_media_action(action, driver)? {
            return Ok(result);
        }
        if let Some(result) = self.try_execute_camera_action(action, driver)? {
            return Ok(result);
        }
        if let Some(result) = self.try_execute_map_action(action, driver)? {
            return Ok(result);
        }

        if let Some(result) = self.try_execute_legacy_mission_action(action, driver)? {
            return Ok(result);
        }

        // Dispatch to the appropriate handler based on action type
        let result = match action_type {
            // Team actions
            ScriptActionType::MoveTeamTo => self.do_move_team_to(action),
            ScriptActionType::TeamAttackTeam => self.do_team_attack_team(action),
            ScriptActionType::TeamHunt => self.do_team_hunt(action),
            ScriptActionType::TeamGuard => self.do_team_guard(action),
            ScriptActionType::TeamDelete => self.do_team_delete(action),
            ScriptActionType::TeamKill => self.do_team_kill(action),
            ScriptActionType::DamageMembersOfTeam => self.do_damage_team_members(action),
            ScriptActionType::TeamSetState => self.do_set_team_state(action),
            ScriptActionType::TeamFollowWaypoints => self.do_team_follow_waypoints(action),
            ScriptActionType::TeamFollowWaypointsExact => {
                self.do_team_follow_waypoints_exact(action)
            }

            // Unit creation/deletion actions
            ScriptActionType::CreateObject => self.do_create_object(action),
            ScriptActionType::CreateNamedOnTeamAtWaypoint => {
                self.do_create_named_on_team_at_waypoint(action)
            }
            ScriptActionType::NamedDelete => self.do_named_delete(action),
            ScriptActionType::NamedKill => self.do_named_kill(action),
            ScriptActionType::NamedDamage => self.do_named_damage(action),

            // Named unit actions
            ScriptActionType::MoveNamedUnitTo => self.do_named_move_to_waypoint(action),
            ScriptActionType::NamedAttackNamed => self.do_named_attack_named(action),
            ScriptActionType::NamedFaceNamed => self.do_named_face_named(action),
            ScriptActionType::NamedHunt => self.do_named_hunt(action),
            ScriptActionType::NamedGuard => self.do_named_guard(action),
            ScriptActionType::NamedStop => self.do_named_stop(action),

            // Player actions
            ScriptActionType::PlayerSetMoney
            | ScriptActionType::PlayerGiveMoney
            | ScriptActionType::PlayerGrantScience
            | ScriptActionType::PlayerPurchaseScience
            | ScriptActionType::PlayerScienceAvailability
            | ScriptActionType::PlayerDisableBaseConstruction
            | ScriptActionType::PlayerEnableBaseConstruction
            | ScriptActionType::PlayerDisableUnitConstruction
            | ScriptActionType::PlayerEnableUnitConstruction
            | ScriptActionType::PlayerDisableFactories
            | ScriptActionType::PlayerEnableFactories
            | ScriptActionType::PlayerAddSkillpoints
            | ScriptActionType::PlayerAddRanklevel
            | ScriptActionType::PlayerSetRanklevel
            | ScriptActionType::PlayerSetRanklevellimit
            | ScriptActionType::PlayerAffectReceivingExperience
            | ScriptActionType::PlayerKill
            | ScriptActionType::PlayerHunt
            | ScriptActionType::PlayerExcludeFromScoreScreen
            | ScriptActionType::PlayerRelatesPlayer
            | ScriptActionType::SkirmishBuildBuilding
            | ScriptActionType::PlayerSellEverything
            | ScriptActionType::PlayerTransferOwnershipPlayer => Err(
                ScriptError::ExecutionFailed(format!("{action_type:?} requires player dispatch")),
            ),


            ScriptActionType::SetFlag
            | ScriptActionType::SetCounter
            | ScriptActionType::IncrementCounter
            | ScriptActionType::DecrementCounter
            | ScriptActionType::SetTimer
            | ScriptActionType::SetMillisecondTimer
            | ScriptActionType::SetRandomTimer
            | ScriptActionType::SetRandomMsecTimer
            | ScriptActionType::StopTimer
            | ScriptActionType::RestartTimer
            | ScriptActionType::AddToMsecTimer
            | ScriptActionType::SubFromMsecTimer => self
                .context
                .borrowed_engine()
                .interpreter()
                .try_execute_variable_action(
                    action,
                    |low, high| {
                        crate::scripting::engine::script_timer_random_int(driver, low, high)
                    },
                    |low, high| {
                        crate::scripting::engine::script_timer_random_real(driver, low, high)
                    },
                )
                .and_then(|handled| {
                    if handled {
                        Ok(ScriptActionResult::Success)
                    } else {
                        Err(ScriptError::ExecutionFailed(format!(
                            "Expected variable action, got {:?}",
                            action.action_type
                        )))
                    }
                }),

            // ============================================================================
            // SCRIPT CONTROL ACTIONS
            // ============================================================================
            ScriptActionType::NoOp => Ok(ScriptActionResult::Success),
            ScriptActionType::EnableScript
            | ScriptActionType::DisableScript
            | ScriptActionType::CameraFadeAdd
            | ScriptActionType::CameraFadeSubtract
            | ScriptActionType::CameraFadeSaturate
            | ScriptActionType::CameraFadeMultiply
            | ScriptActionType::SetTreeSway
            | ScriptActionType::ChooseVictimAlwaysUsesNormal
            | ScriptActionType::SetAttackPriorityThing
            | ScriptActionType::SetAttackPriorityKindOf
            | ScriptActionType::SetDefaultAttackPriority => self
                .context
                .borrowed_engine()
                .execute_local_control_action(action, driver),
            ScriptActionType::CallSubroutine => self.do_call_subroutine(action),
            ScriptActionType::DebugMessageBox => self.do_debug_message_box(action),
            ScriptActionType::DebugString => self.do_debug_string(action),
            ScriptActionType::DebugCrashBox => self.do_debug_crash_box(action),

            // ============================================================================
            // ADDITIONAL TEAM ACTIONS
            // ============================================================================
            ScriptActionType::BuildTeam => self.do_build_team(action),
            ScriptActionType::RecruitTeam => self.do_recruit_team(action),
            ScriptActionType::CreateReinforcementTeam => self.do_create_reinforcement_team(action),
            ScriptActionType::TeamWander => self.do_team_wander(action, driver),
            ScriptActionType::TeamWanderInPlace => self.do_team_wander_in_place(action, driver),
            ScriptActionType::TeamPanic => self.do_team_panic(action, driver),
            ScriptActionType::TeamStop => self.do_team_stop(action),
            ScriptActionType::TeamStopAndDisband => self.do_team_stop_and_disband(action),
            ScriptActionType::TeamAvailableForRecruitment => {
                self.do_team_available_for_recruitment(action)
            }
            ScriptActionType::TeamCollectNearbyForTeam => self.do_team_collect_nearby(action),
            ScriptActionType::TeamMergeIntoTeam => self.do_team_merge(action),
            ScriptActionType::TeamFlash => self.do_team_flash(action),
            ScriptActionType::TeamFlashWhite => self.do_team_flash_white(action),
            ScriptActionType::TeamTransferToPlayer => self.do_team_transfer_to_player(action),
            ScriptActionType::TeamSetOverrideRelationToTeam => {
                self.do_team_set_override_relation_to_team(action)
            }
            ScriptActionType::TeamRemoveOverrideRelationToTeam => {
                self.do_team_remove_override_relation_to_team(action)
            }
            ScriptActionType::TeamRemoveAllOverrideRelations => {
                self.do_team_remove_all_override_relations(action)
            }
            ScriptActionType::TeamSetOverrideRelationToPlayer => {
                self.do_team_set_override_relation_to_player(action)
            }
            ScriptActionType::TeamRemoveOverrideRelationToPlayer => {
                self.do_team_remove_override_relation_to_player(action)
            }
            ScriptActionType::TeamLoadTransports => self.do_team_load_transports(action),
            ScriptActionType::TeamEnterNamed => self.do_team_enter_named(action),
            ScriptActionType::TeamExitAll => self.do_team_exit_all(action),
            ScriptActionType::TeamGarrisonSpecificBuilding => {
                self.do_team_garrison_specific_building(action)
            }
            ScriptActionType::TeamGarrisonNearestBuilding => {
                self.do_team_garrison_nearest_building(action)
            }
            ScriptActionType::TeamExitAllBuildings => self.do_team_exit_all_buildings(action),
            ScriptActionType::TeamGuardPosition => self.do_team_guard_position(action),
            ScriptActionType::TeamGuardObject => self.do_team_guard_object(action),
            ScriptActionType::TeamGuardArea => self.do_team_guard_area(action),
            ScriptActionType::TeamGuardSupplyCenter => self.do_team_guard_supply_center(action),
            ScriptActionType::TeamGuardInTunnelNetwork => {
                self.do_team_guard_in_tunnel_network(action)
            }
            // C++ parity: executeAction dispatches TEAM_GUARD_FOR_FRAMECOUNT to
            // doTeamIdleForFramecount, despite a separate doTeamGuardForFramecount helper.
            ScriptActionType::TeamGuardForFramecount | ScriptActionType::TeamIdleForFramecount => {
                self.do_team_idle_for_framecount(action)
            }
            ScriptActionType::TeamSpinForFramecount => self.do_team_spin_for_framecount(action),
            ScriptActionType::TeamIncreasePriority => self.do_team_increase_priority(action),
            ScriptActionType::TeamDecreasePriority => self.do_team_decrease_priority(action),
            ScriptActionType::TeamAttackArea => self.do_team_attack_area(action),
            ScriptActionType::TeamAttackNamed => self.do_team_attack_named(action),
            ScriptActionType::TeamApplyAttackPrioritySet => {
                self.do_team_apply_attack_priority_set(action)
            }
            ScriptActionType::TeamSetAttitude => self.do_team_set_attitude(action),
            ScriptActionType::TeamExecuteSequentialScript
            | ScriptActionType::TeamExecuteSequentialScriptLooping
            | ScriptActionType::TeamStopSequentialScript => Err(ScriptError::ExecutionFailed(
                "Sequential action must be decoded by its interpreter".into(),
            )),
            ScriptActionType::TeamSetEmoticon => self.do_team_set_emoticon(action),
            ScriptActionType::TeamSetStealthEnabled => self.do_team_set_stealth_enabled(action),
            ScriptActionType::TeamSetRepulsor => self.do_team_set_repulsor(action),
            ScriptActionType::TeamDeleteLiving => self.do_team_delete_living(action),
            ScriptActionType::TeamWaitForNotContainedAll => {
                self.do_team_wait_for_not_contained_all(action)
            }
            ScriptActionType::TeamWaitForNotContainedPartial => {
                self.do_team_wait_for_not_contained_partial(action)
            }
            ScriptActionType::TeamMoveTowardsNearestObjectType => {
                self.do_team_move_towards_nearest_object_type(action)
            }
            ScriptActionType::TeamHuntWithCommandButton => {
                self.do_team_hunt_with_command_button(action)
            }
            ScriptActionType::TeamUseCommandbuttonAbilityOnNamed => {
                self.do_team_use_command_button_on_named(action)
            }
            ScriptActionType::TeamUseCommandbuttonAbilityAtWaypoint => {
                self.do_team_use_command_button_at_waypoint(action)
            }
            ScriptActionType::TeamUseCommandbuttonAbility => {
                self.do_team_use_command_button(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNamed => {
                self.do_team_all_use_command_button_on_named(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyUnit => {
                self.do_team_all_use_command_button_on_nearest_enemy_unit(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestGarrisonedBuilding => {
                self.do_team_all_use_command_button_on_nearest_garrisoned_building(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestKindof => {
                self.do_team_all_use_command_button_on_nearest_kindof(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyBuilding => {
                self.do_team_all_use_command_button_on_nearest_enemy_building(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyBuildingClass => {
                self.do_team_all_use_command_button_on_nearest_enemy_building_class(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestObjecttype => {
                self.do_team_all_use_command_button_on_nearest_object_type(action)
            }
            ScriptActionType::TeamPartialUseCommandbutton => {
                self.do_team_partial_use_command_button(action)
            }
            ScriptActionType::TeamCaptureNearestUnownedFactionUnit => self.do_team_capture_nearest_unowned_faction_unit(action),
            ScriptActionType::TeamSetUnmannedStatus => self.do_team_set_unmanned_status(action),
            ScriptActionType::TeamSetBoobytrapped => self.do_team_set_boobytrapped(action),
            ScriptActionType::TeamFaceNamed => self.do_team_face_named(action),
            ScriptActionType::TeamFaceWaypoint => self.do_team_face_waypoint(action),

            // ============================================================================
            // ADDITIONAL NAMED UNIT ACTIONS
            // ============================================================================
            ScriptActionType::NamedEnterNamed => self.do_named_enter_named(action),
            ScriptActionType::NamedExitAll => self.do_named_exit_all(action),
            ScriptActionType::NamedFollowWaypoints => self.do_named_follow_waypoints(action),
            ScriptActionType::NamedFollowWaypointsExact => {
                self.do_named_follow_waypoints_exact(action)
            }
            ScriptActionType::NamedAttackArea => self.do_named_attack_area(action),
            ScriptActionType::NamedAttackTeam => self.do_named_attack_team(action),
            ScriptActionType::NamedApplyAttackPrioritySet => self.do_named_apply_attack_priority_set(action),
            ScriptActionType::NamedSetAttitude => self.do_named_set_attitude(action),
            ScriptActionType::NamedFlash => self.do_named_flash(action),
            ScriptActionType::NamedFlashWhite => self.do_named_flash_white(action),
            ScriptActionType::NamedGarrisonSpecificBuilding => {
                self.do_named_garrison_specific_building(action)
            }
            ScriptActionType::NamedGarrisonNearestBuilding => {
                self.do_named_garrison_nearest_building(action)
            }
            ScriptActionType::NamedExitBuilding => self.do_named_exit_building(action),
            ScriptActionType::NamedSetStoppingDistance => {
                self.do_named_set_stopping_distance(action)
            }
            ScriptActionType::NamedTransferOwnershipPlayer => {
                self.do_named_transfer_ownership_player(action)
            }
            ScriptActionType::NamedHideSpecialPowerDisplay => {
                self.do_named_hide_special_power_display(action, driver)
            }
            ScriptActionType::NamedShowSpecialPowerDisplay => {
                self.do_named_show_special_power_display(action, driver)
            }
            ScriptActionType::NamedStopSpecialPowerCountdown => {
                self.do_named_stop_special_power_countdown(action, driver)
            }
            ScriptActionType::NamedStartSpecialPowerCountdown => {
                self.do_named_start_special_power_countdown(action, driver)
            }
            ScriptActionType::NamedSetSpecialPowerCountdown => {
                self.do_named_set_special_power_countdown(action, driver)
            }
            ScriptActionType::NamedAddSpecialPowerCountdown => {
                self.do_named_add_special_power_countdown(action, driver)
            }
            ScriptActionType::NamedFireSpecialPowerAtWaypoint => {
                self.do_named_fire_special_power_at_waypoint(action)
            }
            ScriptActionType::NamedFireSpecialPowerAtNamed => {
                self.do_named_fire_special_power_at_named(action)
            }
            ScriptActionType::NamedFireWeaponFollowingWaypointPath => {
                self.do_named_fire_weapon_following_waypoint_path(action, driver)
            }
            ScriptActionType::NamedUseCommandbuttonAbilityOnNamed => {
                self.do_named_use_command_button_on_named(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbilityAtWaypoint => {
                self.do_named_use_command_button_at_waypoint(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbility => {
                self.do_named_use_command_button(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbilityUsingWaypointPath => {
                self.do_named_use_command_button_using_waypoint_path(action)
            }
            ScriptActionType::NamedReceiveUpgrade => self.do_named_receive_upgrade(action),
            ScriptActionType::NamedSetHeld => self.do_named_set_held(action),
            ScriptActionType::NamedSetToppleDirection => Err(ScriptError::ExecutionFailed(
                "Topple directions are applied by the executing interpreter".into(),
            )),
            ScriptActionType::NamedSetRepulsor => self.do_named_set_repulsor(action),
            ScriptActionType::NamedCustomColor => self.do_named_custom_color(action),
            ScriptActionType::NamedSetStealthEnabled => self.do_named_set_stealth_enabled(action),
            ScriptActionType::NamedSetEmoticon => self.do_named_set_emoticon(action),
            ScriptActionType::NamedFaceWaypoint => self.do_named_face_waypoint(action),
            ScriptActionType::NamedSetEvacLeftOrRight => self.do_named_set_evac_left_or_right(action),
            ScriptActionType::NamedSetUnmannedStatus => self.do_named_set_unmanned_status(action),
            ScriptActionType::NamedSetBoobytrapped => self.do_named_set_boobytrapped(action),
            ScriptActionType::UnitExecuteSequentialScript
            | ScriptActionType::UnitExecuteSequentialScriptLooping
            | ScriptActionType::UnitStopSequentialScript => Err(ScriptError::ExecutionFailed(
                "Sequential action must be decoded by its interpreter".into(),
            )),
            ScriptActionType::UnitGuardForFramecount => self.do_unit_guard_for_framecount(action),
            ScriptActionType::UnitIdleForFramecount => self.do_unit_idle_for_framecount(action),
            ScriptActionType::UnitDestroyAllContained => self.do_unit_destroy_all_contained(action),
            ScriptActionType::UnitMoveTowardsNearestObjectType => {
                self.do_unit_move_towards_nearest_object_type(action)
            }
            ScriptActionType::UnitSpawnNamedLocationOrientation => {
                self.do_unit_spawn_named_location_orientation(action)
            }
            ScriptActionType::CreateUnnamedOnTeamAtWaypoint => {
                self.do_create_unnamed_on_team_at_waypoint(action)
            }

            // ============================================================================
            // ADDITIONAL PLAYER ACTIONS
            // ============================================================================
            ScriptActionType::PlayerSetOverrideRelationToTeam => {
                self.do_player_set_override_relation_to_team(action)
            }
            ScriptActionType::PlayerRemoveOverrideRelationToTeam => {
                self.do_player_remove_override_relation_to_team(action)
            }
            ScriptActionType::PlayerGarrisonAllBuildings => {
                self.do_player_garrison_all_buildings(action)
            }
            ScriptActionType::PlayerExitAllBuildings => self.do_player_exit_all_buildings(action),
            ScriptActionType::PlayerCreateTeamFromCapturedUnits => {
                self.do_player_create_team_from_captured_units(action)
            }
            ScriptActionType::PlayerRepairNamedStructure => {
                self.do_player_repair_named_structure(action, driver)
            }
            ScriptActionType::PlayerSelectSkillset => {
                self.do_player_select_skillset(action, driver)
            }

            // ============================================================================
            // DISPLAY/UI ACTIONS
            // ============================================================================
            ScriptActionType::ObjectForceSelect => self.do_object_force_select(action),

            // ============================================================================
            // ENVIRONMENT/WORLD ACTIONS
            // ============================================================================
            ScriptActionType::WaterChangeHeight => self.do_water_change_height(action, driver),
            ScriptActionType::WaterChangeHeightOverTime => {
                self.do_water_change_height_over_time(action, driver)
            }
            ScriptActionType::SetCaveIndex => {
                let name = self.get_string_param(action, 0)?;
                let index = self.get_int_param(action, 1)?;
                if driver.set_named_cave_index(&name, index).is_some() {
                    Ok(ScriptActionResult::Success)
                } else {
                    self.do_set_cave_index(action)
                }
            }
            ScriptActionType::SetInfantryLightingOverride => {
                self.do_set_infantry_lighting_override(action)
            }
            ScriptActionType::ResetInfantryLightingOverride => {
                self.do_reset_infantry_lighting_override()
            }

            // ============================================================================
            // CONSTRUCTION/TECHTREE ACTIONS
            // ============================================================================
            ScriptActionType::SetBaseConstructionSpeed => {
                self.do_set_base_construction_speed(action, driver)
            }
            ScriptActionType::TechtreeModifyBuildabilityObject => {
                self.do_techtree_modify_buildability_object(action)
            }
            ScriptActionType::WarehouseSetValue => self.do_warehouse_set_value(action),
            ScriptActionType::CommandbarRemoveButtonObjecttype => {
                self.do_command_bar_remove_button_object_type(action, driver)
            }
            ScriptActionType::CommandbarAddButtonObjecttypeSlot => {
                self.do_command_bar_add_button_object_type_slot(action, driver)
            }

            // ============================================================================
            // ATTACK PRIORITY ACTIONS
            // ============================================================================
            ScriptActionType::SetStoppingDistance => self.do_set_stopping_distance(action),

            // ============================================================================
            // OBJECT LIST ACTIONS
            // ============================================================================
            ScriptActionType::ObjectlistAddobjecttype => {
                self.do_object_list_add_object_type(action)
            }
            ScriptActionType::ObjectlistRemoveobjecttype => {
                self.do_object_list_remove_object_type(action)
            }
            ScriptActionType::ObjectAllowBonuses => self.do_object_allow_bonuses(action),
            ScriptActionType::DeleteAllUnmanned => self.do_delete_all_unmanned(action),
            ScriptActionType::ScriptingOverrideHulkLifetime => {
                self.do_scripting_override_hulk_lifetime(action, driver)
            }

            // ============================================================================
            // AI/SKIRMISH ACTIONS
            // ============================================================================
            ScriptActionType::SkirmishFollowApproachPath => {
                self.do_skirmish_follow_approach_path(action)
            }
            ScriptActionType::SkirmishMoveToApproachPath => {
                self.do_skirmish_move_to_approach_path(action)
            }
            ScriptActionType::SkirmishBuildBaseDefenseFront => {
                self.do_skirmish_build_base_defense_front(action, driver)
            }
            ScriptActionType::SkirmishBuildBaseDefenseFlank => {
                self.do_skirmish_build_base_defense_flank(action, driver)
            }
            ScriptActionType::SkirmishBuildStructureFront => {
                self.do_skirmish_build_structure_front(action, driver)
            }
            ScriptActionType::SkirmishBuildStructureFlank => {
                self.do_skirmish_build_structure_flank(action, driver)
            }
            ScriptActionType::SkirmishFireSpecialPowerAtMostCost => {
                self.do_skirmish_fire_special_power_at_most_cost(action, driver)
            }
            ScriptActionType::SkirmishAttackNearestGroupWithValue => {
                self.do_skirmish_attack_nearest_group_with_value(action)
            }
            ScriptActionType::SkirmishPerformCommandbuttonOnMostValuableObject => {
                self.do_skirmish_perform_command_button_on_most_valuable_object(action)
            }
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll => {
                self.do_skirmish_wait_for_command_button_available_all(action)
            }
            ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial => {
                self.do_skirmish_wait_for_command_button_available_partial(action)
            }
            ScriptActionType::AiPlayerBuildSupplyCenter => {
                self.do_ai_player_build_supply_center(action, driver)
            }
            ScriptActionType::AiPlayerBuildUpgrade => {
                self.do_ai_player_build_upgrade(action, driver)
            }
            ScriptActionType::AiPlayerBuildTypeNearestTeam => {
                self.do_ai_player_build_type_nearest_team(action, driver)
            }
            ScriptActionType::IdleAllUnits => self.do_player_units_idle(true),
            ScriptActionType::ResumeSupplyTrucking => self.do_player_units_idle(false),

            // ============================================================================
            // EVA/MISC ACTIONS
            // ============================================================================
            ScriptActionType::EvaSetEnabledDisabled => self.do_eva_set_enabled_disabled(action),
            ScriptActionType::OptionsSetOcclusionMode => {
                self.do_options_set_occlusion_mode(action, driver)
            }
            ScriptActionType::OptionsSetDrawiconUiMode => {
                self.do_options_set_draw_icon_ui_mode(action)
            }
            ScriptActionType::OptionsSetParticleCapMode => {
                self.do_options_set_particle_cap_mode(action)
            }
            ScriptActionType::ExitSpecificBuilding => self.do_exit_specific_building(action),
            ScriptActionType::EnableScoring => self.do_enable_scoring(),
            ScriptActionType::DisableScoring => self.do_disable_scoring(),
            ScriptActionType::SetTrainHeld
            | ScriptActionType::UnitAffectObjectPanelFlags
            | ScriptActionType::TeamAffectObjectPanelFlags => Err(
                ScriptError::ExecutionFailed(format!("{action_type:?} requires object dispatch")),
            ),

            ScriptActionType::NumItems => Ok(ScriptActionResult::Success),
            action_type => Err(ScriptError::ExecutionFailed(format!(
                "{action_type:?} has no script dispatcher"
            ))),
        };

        match result {
            Err(ScriptError::TeamNotFound(name)) => {
                log::warn!(
                    "Script action {:?} skipped because team '{}' was not found",
                    action_type,
                    name
                );
                Ok(ScriptActionResult::Success)
            }
            Err(ScriptError::ObjectNotFound(name)) => {
                log::warn!(
                    "Script action {:?} skipped because object '{}' was not found",
                    action_type,
                    name
                );
                Ok(ScriptActionResult::Success)
            }
            Err(ScriptError::PlayerNotFound(name)) => {
                log::warn!(
                    "Script action {:?} skipped because player '{}' was not found",
                    action_type,
                    name
                );
                Ok(ScriptActionResult::Success)
            }
            other => other,
        }
    }

    // ============================================================================
    // AI/SKIRMISH ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn with_current_player_ai<F>(&mut self, f: F)
    where
        F: FnOnce(&mut crate::ai::integration::IntegratedAiPlayer),
    {
        let current_player = self
            .context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten();
        let Some(player_name) = current_player else {
            log::warn!("Skirmish action: current player not available");
            return;
        };

        let __player_list_arc = player_list();
        let Ok(list) = __player_list_arc.read() else {
            return;
        };
        let Some(player_arc) = list.find_player_by_name(&player_name) else {
            log::warn!("Skirmish action: player '{}' not found", player_name);
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };

        let player_id = player_guard.get_player_index() as u32;
        let _difficulty = player_guard.get_player_difficulty();

        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                f(ai_player);
            })
        });
    }

    pub(crate) fn with_named_player_ai<F>(&mut self, player_name: &str, f: F)
    where
        F: FnOnce(&mut crate::ai::integration::IntegratedAiPlayer),
    {
        let __player_list_arc = player_list();
        let Ok(list) = __player_list_arc.read() else {
            return;
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            log::warn!("Skirmish action: player '{}' not found", player_name);
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };

        let player_id = player_guard.get_player_index() as u32;
        let _difficulty = player_guard.get_player_difficulty();

        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                f(ai_player);
            })
        });
    }

    pub(crate) fn get_skirmish_enemy_player(&self) -> Option<Arc<RwLock<crate::player::Player>>> {
        let current_player_name = self
            .context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten()?;

        let __player_list_arc = player_list();
        let list = __player_list_arc.read().ok()?;
        let current_player = list.find_player_by_name(&current_player_name)?;
        let current_guard = current_player.read().ok()?;

        if let Some(enemy_index) = current_guard.get_current_enemy_player_index() {
            if let Some(enemy_arc) = list.get_player(enemy_index).cloned() {
                let is_non_neutral = enemy_arc
                    .read()
                    .ok()
                    .map(|enemy_guard| enemy_guard.get_player_type() != PlayerType::Neutral)
                    .unwrap_or(false);
                if is_non_neutral {
                    return Some(enemy_arc);
                }
            }
        }

        for player_arc in list.iter() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() == PlayerType::Human {
                // C++ ScriptEngine.cpp:5789-5791: Challenge dummy ThePlayer is not the enemy.
                if self.context.is_generals_challenge_campaign() {
                    let is_dummy =
                        NameKeyGenerator::key_to_name(player_guard.get_player_name_key())
                            .as_deref()
                            == Some(THE_PLAYER);
                    if is_dummy {
                        continue;
                    }
                }
                return Some(player_arc.clone());
            }
        }

        None
    }

    pub(crate) fn compute_team_center_and_first(
        &self,
        team_arc: &Arc<RwLock<crate::team::Team>>,
    ) -> Option<(Coord3D, Arc<RwLock<crate::object::Object>>)> {
        let team_guard = team_arc.read().ok()?;
        let members = team_guard.get_members();
        let mut sum = Coord3D::new(0.0, 0.0, 0.0);
        let mut count = 0.0;
        let mut first_unit: Option<Arc<RwLock<crate::object::Object>>> = None;

        for &member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let pos = obj_guard.get_position();
            sum.x += pos.x;
            sum.y += pos.y;
            sum.z += pos.z;
            count += 1.0;
            if first_unit.is_none() {
                first_unit = Some(obj_arc.clone());
            }
        }

        let Some(first_unit) = first_unit else {
            return None;
        };
        if count == 0.0 {
            return None;
        }

        let center = Coord3D::new(sum.x / count, sum.y / count, sum.z / count);
        Some((center, first_unit))
    }

    pub(crate) fn resolve_follow_waypoint_id(
        &self,
        waypoint_name_or_path: &str,
        reference_pos: Coord3D,
    ) -> Option<WaypointID> {
        get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_closest_waypoint_on_path(&reference_pos, waypoint_name_or_path)
                .map(|waypoint| waypoint.get_id())
        })
    }

    /// C++ ScriptActions: aiPlayer->checkBridges(firstUnit, way).
    pub(crate) fn check_bridges_for_waypoint(
        &self,
        player_id: u32,
        unit: &Arc<RwLock<crate::object::Object>>,
        start_waypoint_id: crate::common::WaypointID,
    ) {
        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                ai_player.check_bridges(unit, start_waypoint_id);
            })
        });
    }

    // ============================================================================
    // HELPER METHODS FOR PARAMETER EXTRACTION
    // ============================================================================

    pub(crate) fn get_string_param(
        &self,
        action: &ScriptAction,
        index: usize,
    ) -> Result<String, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_string().to_string())
    }

    pub(crate) fn get_int_param(
        &self,
        action: &ScriptAction,
        index: usize,
    ) -> Result<i32, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_int())
    }

    pub(crate) fn get_real_param(
        &self,
        action: &ScriptAction,
        index: usize,
    ) -> Result<f32, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_real())
    }

    pub(crate) fn get_coord_param(
        &self,
        action: &ScriptAction,
        index: usize,
    ) -> Result<Coord3D, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| {
                let c = p.get_coord();
                Coord3D::new(c.x, c.y, c.z)
            })
    }

    pub(crate) fn get_bool_param_optional(
        &self,
        action: &ScriptAction,
        index: usize,
    ) -> Option<bool> {
        action.get_parameter(index).map(|p| p.get_int() != 0)
    }

    pub(crate) fn resolve_object_types_for_action(&self, type_or_list_name: &str) -> ObjectTypes {
        let mut types = ObjectTypes::new();
        if type_or_list_name.is_empty() {
            return types;
        }

        if let Some(Some(found)) = self
            .context
            .with_engine_ref(|engine| engine.get_object_types(type_or_list_name))
        {
            return found;
        }

        types.add_object_type(AsciiString::from(type_or_list_name));
        types
    }

    pub(crate) fn find_closest_object_of_type_in_trigger(
        &self,
        source_object_id: ObjectID,
        source_pos: &Coord3D,
        source_off_map: bool,
        type_or_list_name: &str,
        trigger_name: &str,
    ) -> Option<ObjectID> {
        let trigger = self.get_trigger_area(trigger_name).ok()?;
        let wanted_types = self.resolve_object_types_for_action(type_or_list_name);
        let max_search_radius = 1_000_000.0;

        let partition = ThePartitionManager::get()?;
        partition.get_closest_object_2d(source_pos, max_search_radius, |candidate| {
            if candidate.get_id() == source_object_id {
                return false;
            }
            if candidate.is_effectively_dead() {
                return false;
            }
            if candidate.is_off_map() != source_off_map {
                return false;
            }

            let pos = candidate.get_position();
            let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
            if !trigger.point_in_trigger_int(&point) {
                return false;
            }

            let template_ref: &dyn crate::common::ThingTemplate = candidate.get_template().as_ref();
            wanted_types.contains_template(Some(template_ref))
        })
    }

    // ============================================================================
    // TEAM AND OBJECT LOOKUP HELPERS
    // C++ Reference: TheScriptEngine->getTeamNamed(), TheAI->createGroup()
    // ============================================================================

    /// Get team by name from TeamFactory
    /// C++ Reference: TheScriptEngine->getTeamNamed(teamName)
    pub(crate) fn get_team_by_name(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<crate::team::Team>>, ScriptError> {
        let resolved = self.resolve_team_name_token(team_name);
        self.context
            .with_engine_ref(|engine| engine.standalone_context_team(&resolved))
            .flatten()
            .ok_or_else(|| ScriptError::TeamNotFound(resolved))
    }

    /// Get a team by name, creating it if missing (matches ScriptActions::createUnitOnTeamAt).
    pub(crate) fn get_or_create_team_by_name(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<crate::team::Team>>, ScriptError> {
        let team_name = self.resolve_team_name_token(team_name);
        let factory = get_team_factory();
        let Ok(mut factory_guard) = factory.lock() else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to lock team factory".to_string(),
            ));
        };

        if let Some(team) = factory_guard.find_team(&team_name) {
            return Ok(team);
        }

        factory_guard.create_team(&team_name).ok_or_else(|| {
            ScriptError::ExecutionFailed(format!("Failed to create team '{}'", team_name))
        })
    }

    /// Create a unit on a team at a waypoint (C++: ScriptActions::createUnitOnTeamAt).
    ///
    /// Returns `Ok(Some(object_id))` when a unit is created, or `Ok(None)` when the action
    /// intentionally does nothing (e.g. unit already exists and is alive).
    pub(crate) fn create_unit_on_team_at_waypoint(
        &mut self,
        unit_name: Option<&str>,
        object_type: &str,
        team_name: &str,
        waypoint_name: &str,
    ) -> Result<Option<crate::common::ObjectID>, ScriptError> {
        let unit_name = unit_name.and_then(|name| {
            let trimmed = name.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        });

        let tracker = get_named_object_tracker();
        if let Some(unit_name) = unit_name {
            if let Ok(Some(old_object_id)) = tracker.get_object_id(unit_name) {
                if let Some(old_obj) = TheGameLogic::find_object_by_id(old_object_id) {
                    if old_obj
                        .read()
                        .ok()
                        .is_some_and(|o| !o.is_effectively_dead())
                    {
                        log::warn!(
                            "WARNING - Object with name '{}' already exists. Failed Create.",
                            unit_name
                        );
                        return Ok(None);
                    }
                }
            }
        }

        if super::dual_world_registry_unavailable() {
            return Ok(None);
        }

        let team_arc = match self.get_or_create_team_by_name(team_name) {
            Ok(team) => team,
            Err(err) => {
                log::warn!("CREATE_UNIT: team '{}' unavailable: {}", team_name, err);
                return Ok(None);
            }
        };

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint_name);
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };

        let position = if let Some(pos) = waypoint_pos {
            pos
        } else {
            log::warn!("CREATE_UNIT: waypoint '{}' not found", waypoint_name);
            crate::common::Coord3D::new(0.0, 0.0, 0.0)
        };

        let object_id = {
            let manager_arc = get_object_manager();
            let Ok(mut manager) = manager_arc.write() else {
                log::warn!("CREATE_UNIT: failed to lock ObjectManager");
                return Ok(None);
            };

            match manager.create_object(
                object_type,
                position,
                Some(team_arc.clone()),
                crate::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(err) => {
                    log::warn!(
                        "CREATE_UNIT: failed to create '{}' for team '{}': {}",
                        object_type,
                        team_name,
                        err
                    );
                    return Ok(None);
                }
            }
        };

        if let Ok(mut team) = team_arc.write() {
            team.add_member(object_id);
        }

        if let Some(unit_name) = unit_name {
            if let Ok(Some(old_object_id)) = tracker.get_object_id(unit_name) {
                let _ = tracker.unregister_object(old_object_id);
            }

            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut obj) = obj_arc.write() {
                    obj.set_name(AsciiString::from(unit_name));
                }
            }

            if let Err(err) = tracker.register_named_object(unit_name.to_string(), object_id) {
                log::warn!(
                    "CREATE_UNIT: failed to register named object '{}' -> {}: {}",
                    unit_name,
                    object_id,
                    err
                );
            }
        }

        Ok(Some(object_id))
    }

    /// Create an AI group and populate it with team members
    /// C++ Reference: TheAI->createGroup(); theTeam->getTeamAsAIGroup(theGroup);
    pub(crate) fn create_ai_group_from_team(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<AiGroup>>, ScriptError> {
        // Get team
        let team_arc = self.get_team_by_name(team_name)?;
        let members = if let Ok(team) = team_arc.read() {
            team.get_members().to_vec()
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to read team".to_string(),
            ));
        };

        // C++ script actions use short-lived groups; avoid contending on global AI write lock.
        let group_id = crate::system::engine_stores::active_services()
            .session_flags()
            .script_temp_group_id
            .fetch_add(1, Ordering::Relaxed);
        let group = Arc::new(RwLock::new(AiGroup::new(group_id)));

        if let Ok(mut group_guard) = group.write() {
            for member_id in members {
                if TheGameLogic::find_object_by_id(member_id).is_some() {
                    group_guard.add(member_id);
                }
            }
        }

        Ok(group)
    }

    /// Issue a command to all members of a team through their AI interfaces
    /// C++ Reference: Matches pattern in doTeamGuard where we iterate team members
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    pub(crate) fn issue_command_to_team_members(
        &self,
        team_name: &str,
        _command: AiCommandType,
        params: &AiCommandParams,
    ) -> Result<(), ScriptError> {
        let team_name = self.resolve_team_name_token(team_name);
        log::debug!(
            "issue_command_to_team_members called for team '{}'",
            team_name
        );

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
                                            let _ = ai_write.execute_command(params);
                                        }
                                    }
                                }
                            }
                        }
                    };
                }
            } else {
                return Err(ScriptError::TeamNotFound(team_name.to_string()));
            }
        }

        Ok(())
    }

    /// Get object by ID from ObjectManager
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    pub(crate) fn get_object_by_id(
        &self,
        object_id: u32,
    ) -> Result<Arc<RwLock<crate::object::Object>>, ScriptError> {
        let obj_mgr = get_object_manager();
        let obj_mgr_guard = obj_mgr.read().map_err(|_| {
            ScriptError::ExecutionFailed("Failed to lock object manager".to_string())
        })?;
        obj_mgr_guard
            .with_object(object_id, |instance| instance.base())
            .ok_or_else(|| ScriptError::ObjectNotFound(format!("Object {} not found", object_id)))
    }

    /// Get waypoint position from terrain logic
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    pub(crate) fn get_waypoint_position(
        &self,
        waypoint_name: &str,
    ) -> Result<Coord3D, ScriptError> {
        let waypoint_name_ascii = AsciiString::from(waypoint_name);
        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(waypoint) = terrain.get_waypoint_by_name(&waypoint_name_ascii) {
                let loc = waypoint.get_location();
                Ok(Coord3D::new(loc.x, loc.y, loc.z))
            } else {
                Err(ScriptError::ObjectNotFound(format!(
                    "Waypoint '{}' not found",
                    waypoint_name
                )))
            }
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ))
        }
    }

    /// C++ ScriptEngine::getQualifiedTriggerAreaByName (ScriptEngine.cpp:5888).
    pub(crate) fn get_trigger_area(
        &self,
        area_name: &str,
    ) -> Result<crate::polygon_trigger::PolygonTrigger, ScriptError> {
        let owned = self
            .context
            .with_state(|context| context.owned_trigger_area(area_name));
        if let Some(trigger) = owned {
            return trigger.ok_or_else(|| {
                ScriptError::ObjectNotFound(format!(
                    "Trigger area '{}' not found in driving owner",
                    area_name
                ))
            });
        }
        if let Some(trigger) = self
            .context
            .with_engine_ref(|engine| engine.get_qualified_trigger_area_by_name(area_name))
            .flatten()
        {
            return Ok(trigger);
        }

        let Some(resolved) = crate::scripting::engine::qualify_trigger_area_name(area_name, None)
        else {
            return Err(ScriptError::ObjectNotFound(format!(
                "Trigger area '{}' not found",
                area_name
            )));
        };

        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(&resolved) {
                Ok(trigger.clone())
            } else {
                Err(ScriptError::ObjectNotFound(format!(
                    "Trigger area '{}' not found",
                    area_name
                )))
            }
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ))
        }
    }

    pub(crate) fn eval_skirmish_command_button_ready_by_name(
        &self,
        team_name: &str,
        command_button_name: &str,
        all_ready: bool,
    ) -> Result<bool, ScriptError> {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            let control_bar = self.owning_control_bar_bridge();
            if let Some(ready) = self.host_query().host_eval_skirmish_command_button_ready(
                team_name,
                command_button_name,
                all_ready,
                control_bar.as_deref(),
            ) {
                return Ok(ready);
            }
        }
        let team_arc = self.get_team_by_name(team_name)?;
        let control_bar = self.owning_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(command_button_name)
        else {
            return Ok(false);
        };

        let members = team_arc
            .read()
            .map(|team| team.get_members().to_vec())
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read team".to_string()))?;

        for obj_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let Some(is_ready) = super::eval_skirmish::leftover_command_button_ready_for_object(
                &obj_guard,
                command_button,
            ) else {
                continue;
            };

            if is_ready {
                if !all_ready {
                    return Ok(true);
                }
            } else if all_ready {
                return Ok(false);
            }
        }

        Ok(all_ready)
    }
}
