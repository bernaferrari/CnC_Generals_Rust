// AIUpdateInterfaceExt and specialized AI helpers
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.
//
// The extension trait now targets the borrowed `dyn AIUpdateInterface` trait
// object (module containers are owned values). It only carries the
// `AiCommandParams` convenience builders; the plain getters/mutators that the
// old `Arc<Mutex<..>>` impl merely delegated to under `try_lock` are gone —
// call `AIUpdateInterface` directly for those.

/// Extension trait for `dyn AIUpdateInterface` providing the C++ command
/// convenience wrappers that build an `AiCommandParams` and execute it.
pub trait AIUpdateInterfaceExt {
    fn ai_move_to_position(&mut self, pos: &Coord3D, add_waypoint: bool, cmd_source: CommandSourceType);
    fn ai_move_to_position_even_if_sleeping(&mut self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_object(&mut self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_tighten_to_position(&mut self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_and_evacuate(&mut self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_and_evacuate_and_exit(&mut self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_idle(&mut self, cmd_source: CommandSourceType);
    fn ai_hunt(&mut self, cmd_source: CommandSourceType);
    fn ai_enter(&mut self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_force_attack_object(
        &mut self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_object(
        &mut self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_object_id(
        &mut self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_position(
        &mut self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_move_to_position(
        &mut self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_team(
        &mut self,
        team: crate::team::TeamID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_follow_waypoint_path(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_follow_waypoint_path_as_team(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_exact(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_as_team(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_exact_as_team(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_exit_production_path(
        &mut self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    );
    /// C++ AIUpdateInterface::aiDock(Object*, CommandSourceType).
    fn ai_dock(&mut self, dock_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_follow_path(
        &mut self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_path_append(&mut self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_away_from_unit(&mut self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_guard_retaliate(
        &mut self,
        victim_id: ObjectID,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    );
    fn ai_guard_object(
        &mut self,
        obj_to_guard_id: ObjectID,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    );
}

impl AIUpdateInterfaceExt for dyn AIUpdateInterface {
    fn ai_move_to_position(
        &mut self,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        let mut params = crate::ai::AiCommandParams::new(
            if add_waypoint {
                crate::ai::AiCommandType::FollowPathAppend
            } else {
                crate::ai::AiCommandType::MoveToPosition
            },
            cmd_source,
        );
        params.pos = *pos;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_move_to_position_even_if_sleeping(&mut self, pos: &Coord3D, cmd_source: CommandSourceType) {
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::MoveToPositionEvenIfSleeping,
            cmd_source,
        );
        params.pos = *pos;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_move_to_object(&mut self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::MoveToObject, cmd_source);
        params.obj = Some(obj_id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_tighten_to_position(&mut self, pos: &Coord3D, cmd_source: CommandSourceType) {
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::TightenToPosition,
            cmd_source,
        );
        params.pos = *pos;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_move_to_and_evacuate(&mut self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiMoveToAndEvacuate()
        // Move to position and then evacuate (exit garrison/transport)
        // Issue move-to-and-evacuate command
        // The AI state machine handles the evacuation after move completes
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::MoveToPositionAndEvacuate,
            cmd_source,
        );
        params.pos = *pos;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_move_to_and_evacuate_and_exit(&mut self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiMoveToAndEvacuateAndExit()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::MoveToPositionAndEvacuateAndExit,
            cmd_source,
        );
        params.pos = *pos;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_idle(&mut self, cmd_source: CommandSourceType) {
        let params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Idle, cmd_source);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_hunt(&mut self, cmd_source: CommandSourceType) {
        let params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Hunt, cmd_source);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_enter(&mut self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Enter, cmd_source);
        params.obj = Some(obj_id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_force_attack_object(
        &mut self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiForceAttackObject()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::ForceAttackObject,
            cmd_source,
        );
        params.obj = Some(victim_id);
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_attack_object(
        &mut self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackObject()
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, cmd_source);
        params.obj = Some(victim_id);
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_attack_object_id(
        &mut self,
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
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, cmd_source);
        params.obj = Some(victim_id);
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_attack_position(
        &mut self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::AttackPosition,
            cmd_source,
        );
        params.pos = *pos;
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_attack_move_to_position(
        &mut self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackMoveToPosition()
        // From AIStates.cpp: AI_ATTACK_MOVE_TO state
        // This is a special movement mode where the unit moves to a destination
        // but engages enemies encountered along the way (unlike regular move which ignores enemies)
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::AttackMoveToPosition,
            cmd_source,
        );
        params.pos = *pos;
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_attack_team(
        &mut self,
        team: crate::team::TeamID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackTeam()
        if let Some(name) = crate::team::factory_access::with_team(team, |t| t.get_name().as_str().to_string()) {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackTeam,
                cmd_source,
            );
            params.team = Some(name);
            params.int_value = max_shots_to_fire;
            let _ = AIUpdateInterface::execute_command(self, &params);
        }
    }

    fn ai_attack_follow_waypoint_path(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackFollowWaypointPath()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::AttackFollowWaypointPath,
            cmd_source,
        );
        params.waypoint = Some(waypoint.id);
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_attack_follow_waypoint_path_as_team(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackFollowWaypointPathAsTeam()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam,
            cmd_source,
        );
        params.waypoint = Some(waypoint.id);
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_waypoint_path(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPath()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::FollowWaypointPath,
            cmd_source,
        );
        params.waypoint = Some(waypoint.id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_waypoint_path_exact(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathExact()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::FollowWaypointPathExact,
            cmd_source,
        );
        params.waypoint = Some(waypoint.id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_waypoint_path_as_team(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathAsTeam()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::FollowWaypointPathAsTeam,
            cmd_source,
        );
        params.waypoint = Some(waypoint.id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_waypoint_path_exact_as_team(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathExactAsTeam()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::FollowWaypointPathAsTeamExact,
            cmd_source,
        );
        params.waypoint = Some(waypoint.id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_exit_production_path(
        &mut self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    ) {
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::FollowExitProductionPath,
            cmd_source,
        );
        params.coords = path.to_vec();
        params.obj = ignore_object_id;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_dock(&mut self, dock_id: ObjectID, cmd_source: CommandSourceType) {
        // C++ aiDock — AIPlayer onUnitProduced uses CMD_FROM_PLAYER for supply trucks.
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Dock, cmd_source);
        params.obj = Some(dock_id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_path(
        &mut self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    ) {
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::FollowPath, cmd_source);
        params.coords = path.to_vec();
        params.obj = ignore_object_id;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_follow_path_append(&mut self, pos: &Coord3D, cmd_source: CommandSourceType) {
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::FollowPathAppend,
            cmd_source,
        );
        params.pos = *pos;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_move_away_from_unit(&mut self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        // Wave 340: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if !self.is_allowed_to_move_away_from_unit() {
            return;
        }
        let busy = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |other_guard| {
                other_guard.test_status(crate::common::ObjectStatusTypes::IsUsingAbility)
                    || other_guard
                        .get_ai()
                        .map(|ai| ai.is_busy())
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if busy {
            return;
        }
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::MoveAwayFromUnit,
            cmd_source,
        );
        params.obj = Some(obj_id);
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_guard_retaliate(
        &mut self,
        victim_id: ObjectID,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardRetaliate()
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::GuardRetaliate,
            cmd_source,
        );
        params.obj = Some(victim_id);
        params.pos = *pos;
        params.int_value = max_shots_to_fire;
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardPosition()
        // Uses the AI state machine so guard mode and command source are preserved.
        let mut params = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::GuardPosition,
            cmd_source,
        );
        params.pos = *pos;
        params.int_value = guard_mode.as_i32();
        let _ = AIUpdateInterface::execute_command(self, &params);
    }

    fn ai_guard_object(
        &mut self,
        obj_to_guard_id: ObjectID,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardObject()
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::GuardObject, cmd_source);
        params.obj = Some(obj_to_guard_id);
        params.int_value = guard_mode.as_i32();
        let _ = AIUpdateInterface::execute_command(self, &params);
    }
}
