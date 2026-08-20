// AIUpdateInterfaceExt and specialized AI helpers
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Extension trait for Arc<Mutex<dyn AIUpdateInterface>> to provide convenient methods
pub trait AIUpdateInterfaceExt {
    fn get_speed(&self) -> f32;
    fn ai_move_to_position(&self, pos: &Coord3D, add_waypoint: bool, cmd_source: CommandSourceType);
    fn ai_move_to_position_even_if_sleeping(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_object(&self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_tighten_to_position(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_and_evacuate(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_and_evacuate_and_exit(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_idle(&self, cmd_source: CommandSourceType);
    fn ai_hunt(&self, cmd_source: CommandSourceType);
    fn ai_busy(&self, cmd_source: CommandSourceType);
    fn ai_enter(&self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_object_id(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_move_to_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_team(
        &self,
        team: &Arc<RwLock<Team>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_exact(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_exact_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_exit_production_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    );
    /// C++ AIUpdateInterface::aiDock(Object*, CommandSourceType).
    fn ai_dock(&self, dock_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_follow_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_path_append(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_away_from_unit(&self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_guard_retaliate(
        &self,
        victim_id: ObjectID,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_guard_position(
        &self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    );
    fn ai_guard_object(
        &self,
        obj_to_guard_id: ObjectID,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    );
    fn is_idle(&self) -> bool;
    fn is_busy(&self) -> bool {
        !self.is_idle()
    }
    fn set_attitude(&self, attitude: AIAttitudeType);
    fn get_attitude(&self) -> AIAttitudeType;
    fn is_ai_in_dead_state(&self) -> bool;
    fn mark_as_dead(&self);
    fn get_last_command_source(&self) -> CommandSourceType;
    fn get_which_turret_for_cur_weapon(&self) -> TurretType;
    fn set_turret_enabled(&self, turret: TurretType, enabled: bool);
    fn recenter_turret(&self, turret: TurretType);
    fn is_turret_in_natural_position(&self, turret: TurretType) -> bool;
    fn get_path(&self) -> Option<()>;
    fn get_path_destination(&self) -> Option<Coord3D>;
    fn peek_cached_point_on_path(&self) -> Option<Coord3D>;

    fn get_locomotor_distance_to_goal(&self) -> Real;
    fn get_current_victim(&self) -> Option<ObjectID>;
    fn set_current_victim(&mut self, victim: Option<ObjectID>);
    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>>;
    fn get_preferred_height(&self) -> Option<Real>;
    fn ai_go_prone(&self, damage_info: &DamageInfo, cmd_source: CommandSourceType);
    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>>;
    fn get_goal_position(&self) -> Option<Coord3D>;
    fn join_team(&self);
    fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet>;
    fn choose_locomotor_set(&self, set: LocomotorSetType);
    fn set_allow_invalid_position(&self, allow: bool);
    fn set_ultra_accurate(&self, ultra: bool);
    fn execute_command(
        &self,
        params: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn queue_waypoint(&self, pos: &Coord3D);
    fn execute_waypoint_queue(&self);
}

impl AIUpdateInterfaceExt for Arc<Mutex<dyn AIUpdateInterface>> {
    fn get_speed(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_speed()
        } else {
            0.0
        }
    }

    fn ai_move_to_position(
        &self,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                if add_waypoint {
                    crate::ai::AiCommandType::FollowPathAppend
                } else {
                    crate::ai::AiCommandType::MoveToPosition
                },
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_position_even_if_sleeping(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPositionEvenIfSleeping,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_object(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::MoveToObject, cmd_source);
            params.obj = Some(obj_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_tighten_to_position(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::TightenToPosition,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_and_evacuate(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiMoveToAndEvacuate()
        // Move to position and then evacuate (exit garrison/transport)
        if let Ok(mut guard) = self.try_lock() {
            // Issue move-to-and-evacuate command
            // The AI state machine handles the evacuation after move completes
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPositionAndEvacuate,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_and_evacuate_and_exit(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiMoveToAndEvacuateAndExit()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPositionAndEvacuateAndExit,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_idle(&self, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Idle, cmd_source);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_hunt(&self, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Hunt, cmd_source);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_busy(&self, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiBusy()
        if let Ok(mut guard) = self.try_lock() {
            let params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Busy, cmd_source);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_enter(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Enter, cmd_source);
            params.obj = Some(obj_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiForceAttackObject()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::ForceAttackObject,
                cmd_source,
            );
            params.obj = Some(victim_id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackObject()
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, cmd_source);
            params.obj = Some(victim_id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_object_id(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 340: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        // C++ Reference: AIUpdateInterface::aiAttackObject() with object ID
        if victim_id == INVALID_ID || OBJECT_REGISTRY.with_object(victim_id, |_| ()).is_none() {
            return;
        }
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, cmd_source);
            params.obj = Some(victim_id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackPosition,
                cmd_source,
            );
            params.pos = *pos;
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_move_to_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackMoveToPosition()
        // From AIStates.cpp: AI_ATTACK_MOVE_TO state
        // This is a special movement mode where the unit moves to a destination
        // but engages enemies encountered along the way (unlike regular move which ignores enemies)
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackMoveToPosition,
                cmd_source,
            );
            params.pos = *pos;
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_team(
        &self,
        team: &Arc<RwLock<Team>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackTeam()
        if let Ok(team_guard) = team.read() {
            if let Ok(mut guard) = self.try_lock() {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::AttackTeam,
                    cmd_source,
                );
                params.team = Some(team_guard.get_name().as_str().to_string());
                params.int_value = max_shots_to_fire;
                let _ = guard.execute_command(&params);
            }
        }
    }

    fn ai_attack_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackFollowWaypointPath()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackFollowWaypointPath,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackFollowWaypointPathAsTeam()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPath()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPath,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path_exact(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathExact()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPathExact,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathAsTeam()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPathAsTeam,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path_exact_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathExactAsTeam()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPathAsTeamExact,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_exit_production_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowExitProductionPath,
                cmd_source,
            );
            params.coords = path.to_vec();
            params.obj = ignore_object_id;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_dock(&self, dock_id: ObjectID, cmd_source: CommandSourceType) {
        // C++ aiDock — AIPlayer onUnitProduced uses CMD_FROM_PLAYER for supply trucks.
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Dock, cmd_source);
            params.obj = Some(dock_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::FollowPath, cmd_source);
            params.coords = path.to_vec();
            params.obj = ignore_object_id;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_path_append(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowPathAppend,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_away_from_unit(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        // Wave 340: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Ok(mut guard) = self.try_lock() {
            if !guard.is_allowed_to_move_away_from_unit() {
                return;
            }
            if let Some(other) = crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
                if let Ok(other_guard) = other.read() {
                    if other_guard.test_status(crate::common::ObjectStatusTypes::IsUsingAbility)
                        || other_guard
                            .get_ai()
                            .and_then(|ai| ai.lock().ok().map(|ai_guard| ai_guard.is_busy()))
                            .unwrap_or(false)
                    {
                        return;
                    }
                }
            }
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveAwayFromUnit,
                cmd_source,
            );
            params.obj = Some(obj_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_guard_retaliate(
        &self,
        victim_id: ObjectID,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardRetaliate()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::GuardRetaliate,
                cmd_source,
            );
            params.obj = Some(victim_id);
            params.pos = *pos;
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_guard_position(
        &self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardPosition()
        // Uses the AI state machine so guard mode and command source are preserved.
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::GuardPosition,
                cmd_source,
            );
            params.pos = *pos;
            params.int_value = guard_mode.as_i32();
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_guard_object(
        &self,
        obj_to_guard_id: ObjectID,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardObject()
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::GuardObject, cmd_source);
            params.obj = Some(obj_to_guard_id);
            params.int_value = guard_mode.as_i32();
            let _ = guard.execute_command(&params);
        }
    }

    fn is_idle(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_idle()
        } else {
            false
        }
    }

    fn is_busy(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_busy()
        } else {
            false
        }
    }

    fn set_attitude(&self, attitude: AIAttitudeType) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_attitude(attitude);
        }
    }

    fn get_attitude(&self) -> AIAttitudeType {
        if let Ok(guard) = self.try_lock() {
            guard.get_attitude()
        } else {
            AIAttitudeType::Normal
        }
    }

    fn is_ai_in_dead_state(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_ai_in_dead_state()
        } else {
            false
        }
    }

    fn mark_as_dead(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.mark_as_dead();
        }
    }

    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        if let Ok(guard) = self.try_lock() {
            guard.get_goal_object()
        } else {
            None
        }
    }

    fn get_goal_position(&self) -> Option<Coord3D> {
        if let Ok(guard) = self.try_lock() {
            guard.get_goal_position()
        } else {
            None
        }
    }

    fn join_team(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.join_team();
        }
    }

    fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet> {
        if let Ok(guard) = self.try_lock() {
            guard.get_locomotor_set_clone()
        } else {
            None
        }
    }

    fn choose_locomotor_set(&self, set: LocomotorSetType) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.choose_locomotor_set(set);
        }
    }

    fn set_allow_invalid_position(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_allow_invalid_position(allow);
        }
    }

    fn set_ultra_accurate(&self, ultra: bool) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_ultra_accurate(ultra);
        }
    }
    fn get_last_command_source(&self) -> CommandSourceType {
        if let Ok(guard) = self.try_lock() {
            guard.get_last_command_source()
        } else {
            CommandSourceType::FromAi
        }
    }

    fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        if let Ok(guard) = self.try_lock() {
            guard.get_which_turret_for_cur_weapon()
        } else {
            TurretType::Invalid
        }
    }

    fn set_turret_enabled(&self, turret: TurretType, enabled: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_turret_enabled(turret, enabled);
        }
    }

    fn recenter_turret(&self, turret: TurretType) {
        if let Ok(mut guard) = self.try_lock() {
            guard.recenter_turret(turret);
        }
    }

    fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_turret_in_natural_position(turret)
        } else {
            true
        }
    }

    fn get_path(&self) -> Option<()> {
        if let Ok(guard) = self.try_lock() {
            if guard.get_path_destination().is_some() {
                return Some(());
            }
        }
        None
    }

    fn get_path_destination(&self) -> Option<Coord3D> {
        if let Ok(guard) = self.try_lock() {
            guard.get_path_destination()
        } else {
            None
        }
    }

    fn peek_cached_point_on_path(&self) -> Option<Coord3D> {
        if let Ok(guard) = self.try_lock() {
            guard.peek_cached_point_on_path()
        } else {
            None
        }
    }


    fn get_locomotor_distance_to_goal(&self) -> Real {
        if let Ok(guard) = self.try_lock() {
            guard.get_locomotor_distance_to_goal()
        } else {
            0.0
        }
    }

    fn get_current_victim(&self) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.get_current_victim()
        } else {
            None
        }
    }

    fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_current_victim(victim);
        }
    }

    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        if let Ok(guard) = self.try_lock() {
            guard.get_cur_locomotor()
        } else {
            None
        }
    }

    fn get_preferred_height(&self) -> Option<Real> {
        self.try_lock()
            .ok()
            .and_then(|guard| guard.get_preferred_height())
    }

    fn ai_go_prone(&self, damage_info: &DamageInfo, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            guard.ai_go_prone(damage_info, cmd_source);
        }
    }

    fn execute_command(
        &self,
        params: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.execute_command(params)
        } else {
            Err("Failed to lock AIUpdateInterface".into())
        }
    }

    fn queue_waypoint(&self, pos: &Coord3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.queue_waypoint(pos);
        }
    }

    fn execute_waypoint_queue(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.execute_waypoint_queue();
        }
    }
}

