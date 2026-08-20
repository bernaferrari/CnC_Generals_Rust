// AIUpdateInterface trait
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// AI update interface (matching C++ AIUpdateInterface)
pub trait AIUpdateInterface: Send + Sync + std::fmt::Debug {
    /// Update AI logic
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Serialize live AIUpdate runtime state through the owning AIUpdate module.
    ///
    /// The default keeps test doubles and non-standard AI shims compatible; concrete
    /// gameplay AI implementations should override this when they own restorable state.
    fn xfer_ai_update_state(&mut self, _xfer: &mut dyn Xfer) -> Result<bool, String> {
        Ok(false)
    }
    /// Check if the object is moving
    fn is_moving(&self) -> bool;
    /// Check if the object is idle
    fn is_idle(&self) -> bool;
    /// Check if the object is idle without pending-command suppression.
    fn is_idle_unrestricted(&self) -> bool {
        self.is_idle()
    }
    /// Check if the object is currently attacking.
    fn is_attacking(&self) -> bool {
        false
    }
    /// Set movement target
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String>;
    /// Returns the locomotor preferred height if available.
    fn get_preferred_height(&self) -> Option<Real> {
        self.get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|guard| guard.preferred_height))
    }

    /// Get current locomotor (matches C++ AIUpdateInterface::getCurLocomotor).
    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        None
    }
    /// Get whether a locomotor path is active (matches C++ AIUpdateInterface::getPath).
    fn get_path(&self) -> Option<()> {
        self.get_path_destination().map(|_| ())
    }
    /// Get the destination of the active path (matches C++ AIUpdateInterface::getPath last node).
    fn get_path_destination(&self) -> Option<Coord3D> {
        None
    }
    /// Get remaining distance to goal along locomotor path (matches C++ getLocomotorDistanceToGoal).
    fn get_locomotor_distance_to_goal(&self) -> Real {
        0.0
    }

    /// Get current enter/garrison target (matches C++ AIUpdateInterface::getEnterTarget).
    fn get_enter_target(&self) -> Option<ObjectID> {
        None
    }
    /// Get last command source
    fn get_last_command_source(&self) -> crate::ai::CommandSourceType {
        crate::ai::CommandSourceType::FromAi // Default to AI source
    }
    /// Set last command source (matches C++ friend_setLastCommandSource).
    fn set_last_command_source(&mut self, _source: crate::ai::CommandSourceType) {}
    /// Get current AI command (matches C++ AIUpdateInterface::getAIStateType for docking checks).
    fn get_current_command(&self) -> Option<AiCommandType> {
        None
    }
    /// Get pending AI command (matches C++ AIUpdateInterface::friend_getPendingCommandType).
    fn get_pending_command_type(&self) -> Option<AiCommandType> {
        None
    }
    /// Purge pending AI command (matches C++ AIUpdateInterface::friend_purgePendingCommand).
    fn purge_pending_command(&mut self) {}

    /// Clear locomotor goal (matches C++ AIUpdateInterface::setLocomotorGoalNone).
    fn set_locomotor_goal_none(&mut self) {
        self.destroy_path();
    }

    /// Set locomotor goal orientation (matches C++ AIUpdateInterface::setLocomotorGoalOrientation).
    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        let _ = angle;
    }

    /// Set locomotor goal position explicitly (matches C++ AIUpdateInterface::setLocomotorGoalPositionExplicit).
    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        let _ = pos;
    }

    /// Notify AI that a move is ending (matches C++ friend_endingMove).
    fn friend_ending_move(&mut self) {
        let _ = self.is_blocked_and_stuck();
    }
    /// Notify AI that a move is starting (matches C++ friend_startingMove).
    fn friend_starting_move(&mut self) {
        self.set_blocked_and_stuck(false);
    }

    /// Set surrendered state (matches C++ AIUpdateInterface::setSurrendered).
    fn set_surrendered(&mut self, to_object_id: Option<ObjectID>, surrendered: bool) {
        let _ = (to_object_id, surrendered);
    }

    /// Check if this unit is surrendered (matches C++ AIUpdateInterface::isSurrendered).
    fn is_surrendered(&self) -> bool {
        false
    }

    /// Player index we surrendered to, if any (matches C++ AIUpdateInterface::getSurrenderedPlayerIndex).
    fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        None
    }

    /// Whether the AI is allowed to adjust destination on the fly.
    fn is_allowed_to_adjust_destination(&self) -> bool {
        true
    }

    /// Whether this aircraft should adjust destination (matches isAircraftThatAdjustsDestination).
    fn is_aircraft_that_adjusts_destination(&self) -> bool {
        false
    }

    /// Desired movement speed (matches C++ AIUpdateInterface::getDesiredSpeed).
    fn get_desired_speed(&self) -> Real {
        FAST_AS_POSSIBLE
    }

    /// Set desired movement speed (matches C++ AIUpdateInterface::setDesiredSpeed).
    fn set_desired_speed(&mut self, speed: Real) {
        let _ = speed;
    }

    /// Whether the unit is currently rappelling (AI_RAPPEL_INTO state support).
    fn is_in_rappel_state(&self) -> bool {
        false
    }

    /// Whether the unit is currently performing a combat drop (chinook AI support).
    fn is_doing_combat_drop(&self) -> bool {
        false
    }

    /// Whether this unit is moving out of the way of another unit (matches C++ isMovingAwayFrom).
    fn is_moving_away_from(&self, _obj_id: ObjectID) -> bool {
        false
    }

    /// Set a duration to ignore collisions (matches C++ setIgnoreCollisionTime).
    fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        let _ = duration_frames;
    }

    /// Frame until which collisions should be ignored.
    fn get_ignore_collisions_until(&self) -> UnsignedInt {
        0
    }

    /// Queue a pathfinding request after a delay (matches AIUpdateInterface::setQueueForPathTime).
    fn set_queue_for_path_time(&mut self, _frames: UnsignedInt) {}

    /// Re-evaluate horde/nationalism/fanaticism morale bonuses (matches C++ evaluateMoraleBonus).
    fn evaluate_morale_bonus(&mut self) {
        let _ = self.get_current_victim();
    }

    /// Whether AI can move away from another unit (matches JetAIUpdate::isAllowedToMoveAwayFromUnit).
    fn is_allowed_to_move_away_from_unit(&self) -> bool {
        true
    }

    /// Provide a sneaky targeting offset (matches JetAIUpdate::getSneakyTargetingOffset).
    fn get_sneaky_targeting_offset(&self, _offset: &mut Coord3D) -> bool {
        false
    }

    /// Whether aim success is temporarily prevented (matches JetAIUpdate::isTemporarilyPreventingAimSuccess).
    fn is_temporarily_preventing_aim_success(&self) -> bool {
        false
    }

    /// Whether out of special return-to-base ammo (matches JetAIUpdate::isOutOfSpecialReloadAmmo).
    fn is_out_of_special_reload_ammo(&self) -> bool {
        false
    }

    /// Add or remove a targeter (matches C++ AIUpdateInterface::addTargeter).
    fn add_targeter(&mut self, _id: ObjectID, _add: bool) {}

    /// Whether turrets are linked (matches C++ AIUpdateInterface::areTurretsLinked).
    fn are_turrets_linked(&self) -> Bool {
        false
    }

    /// C++ `AIUpdateInterface::friend_getTurretSync`.
    fn friend_get_turret_sync(&self) -> TurretType {
        TurretType::Invalid
    }

    /// C++ `AIUpdateInterface::friend_setTurretSync`.
    fn friend_set_turret_sync(&mut self, _turret: TurretType) {}

    /// C++ `AIUpdateInterface::clearGuardTargetType`.
    fn clear_guard_target_type(&mut self) {}

    /// Set turret target object (matches C++ AIUpdateInterface::setTurretTargetObject).
    fn set_turret_target_object(
        &mut self,
        _turret: TurretType,
        _target_id: Option<ObjectID>,
        _force_attacking: bool,
    ) {
    }

    /// Set turret target position (matches C++ AIUpdateInterface::setTurretTargetPosition).
    fn set_turret_target_position(&mut self, turret: TurretType, pos: &Coord3D) {
        self.set_turret_target_object(turret, None, false);
        let _ = pos;
    }

    /// Whether to treat as aircraft for distance-to-goal (matches JetAIUpdate::getTreatAsAircraftForLocoDistToGoal).
    fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        true
    }

    /// Whether a contained object is free to exit (matches C++ AIUpdateInterface::getAiFreeToExit).
    fn get_ai_free_to_exit(&self, _exiter: &Object) -> crate::object::production::AIFreeToExitType {
        crate::object::production::AIFreeToExitType::FreeToExit
    }

    /// Mark this unit as demoralized for a duration in frames.
    /// Matches C++ AIUpdateInterface::setDemoralized.
    fn set_demoralized(&mut self, duration_frames: UnsignedInt) {
        let _ = duration_frames;
    }

    /// Transfer active attackers from one object to another (matches C++ transferAttack).
    fn transfer_attack(&mut self, from_id: ObjectID, to_id: ObjectID) {
        let _ = (from_id, to_id);
    }

    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        _slot: crate::weapon::WeaponSlotType,
        _target: crate::common::ObjectID,
    ) -> bool {
        false
    }
    /// Check if dock is open
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
    /// Returns whether the unit is taxiing to its parking position (matches JetAIUpdate::isTaxiingToParking).
    fn is_taxiing_to_parking(&self) -> bool {
        false
    }
    /// Returns whether the unit is reloading (matches JetAIUpdate::isReloading).
    fn is_reloading(&self) -> bool {
        false
    }
    /// Returns whether the unit is clearing mines (matches AIUpdateInterface::isClearingMines).
    fn is_clearing_mines(&self) -> Bool {
        false
    }
    /// Returns whether the unit is taking off or landing (matches JetAIUpdate::isTakeoffOrLandingInProgress).
    fn is_takeoff_or_landing_in_progress(&self) -> bool {
        false
    }

    /// Current AI state ID, if available (matches AIStateMachine::getCurrentStateID usage).
    fn get_current_state_id(&self) -> Option<u32> {
        None
    }
    /// Returns parking offset for jets (matches JetAIUpdate::friend_getParkingOffset).
    fn get_parking_offset(&self) -> Real {
        0.0
    }
    /// Returns whether jets keep parking space while airborne (matches JetAIUpdate::friend_keepsParkingSpaceWhenAirborne).
    fn keeps_parking_space_when_airborne(&self) -> bool {
        true
    }
    /// Cancel dock operation
    fn cancel_dock(
        &mut self,
        _obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Supply truck AI interface access
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface> {
        None
    }
    /// Mutable supply truck AI interface access
    fn get_supply_truck_ai_interface_mut(&mut self) -> Option<&mut dyn SupplyTruckAIInterface> {
        None
    }
    /// POW truck AI interface access
    fn get_pow_truck_ai_update_interface(&mut self) -> Option<&mut dyn POWTruckAIUpdateInterface> {
        None
    }
    /// Hack internet AI interface access
    fn get_hack_internet_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn HackInternetAIUpdateInterface> {
        None
    }
    /// Assault transport AI interface access
    fn get_assault_transport_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn AssaultTransportAIUpdateInterface> {
        None
    }
    /// Worker AI update interface access
    fn get_worker_ai_update_interface_mut(&mut self) -> Option<&mut dyn WorkerAIUpdateInterface> {
        None
    }
    /// Dozer AI update interface access
    fn get_dozer_ai_update_interface_mut(&mut self) -> Option<&mut dyn DozerAIUpdateInterface> {
        None
    }
    /// Deliver payload AI interface access
    fn get_deliver_payload_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn DeliverPayloadAIUpdateInterface> {
        None
    }
    fn ignore_obstacle(
        &mut self,
        _obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Borrow-first ignored-obstacle setter (ObjectID only).
    fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = id;
        Ok(())
    }

    /// Get current ignored obstacle ID (matches AIUpdateInterface::getIgnoredObstacleID).
    fn get_ignored_obstacle_id(&self) -> ObjectID {
        crate::common::INVALID_ID
    }

    /// Apply bump speed limit logic when blocked (matches AIUpdateInterface::doLocomotor).
    fn apply_bump_speed_limit(&mut self, desired_speed: Real, _blocked: bool) -> Real {
        desired_speed
    }

    /// Set the current goal path index (used by waypoint rendering/debug UI).
    fn set_current_goal_path_index(
        &mut self,
        _index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get the current goal path index (default -1).
    fn get_current_goal_path_index(&self) -> i32 {
        -1
    }

    /// Allow pathing through units (AIStates.cpp uses this in AIDockState).
    fn set_can_path_through_units(
        &mut self,
        _value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this unit can path through other units (matches AIUpdateInterface::getCanPathThroughUnits).
    fn get_can_path_through_units(&self) -> bool {
        false
    }

    /// Check if the unit is blocked and stuck (MoveOutOfTheWay uses this).
    fn is_blocked_and_stuck(&self) -> bool {
        false
    }

    /// Mark this unit as blocked this frame (matches AIUpdateInterface::m_isBlocked).
    fn set_is_blocked(&mut self, blocked: bool) {
        if !blocked {
            self.set_blocked_and_stuck(false);
        }
    }

    /// Mark this unit as blocked and stuck (matches AIUpdateInterface::m_isBlockedAndStuck).
    fn set_blocked_and_stuck(&mut self, _blocked: bool) {}

    /// Frames blocked for movement (matches AIUpdateInterface::getNumFramesBlocked).
    fn get_num_frames_blocked(&self) -> u32 {
        0
    }

    /// Clear any active pathing state.
    fn destroy_path(&mut self) {
        self.set_goal_object(None);
    }

    /// Clear move-out-of-way state (noop by default).
    fn clear_move_out_of_way(&mut self) {}

    /// Goal object id (for AI coordination).
    fn get_goal_object_id(&self) -> ObjectID {
        crate::common::INVALID_ID
    }
    /// Resolve goal object for the duration of a call.
    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 340: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.get_goal_object_id();
        if id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }
    }

    /// Set goal object (friend_setGoalObject parity).
    fn set_goal_object(&mut self, obj_id: Option<ObjectID>) {
        let _ = obj_id;
    }

    /// Goal position from the AI state machine (matches C++ getGoalPosition).
    fn get_goal_position(&self) -> Option<Coord3D> {
        None
    }

    /// Set goal position on the AI state machine (matches C++ setGoalPosition).
    fn set_goal_position(&mut self, pos: Option<Coord3D>) {
        let _ = pos;
    }

    /// C++ `AIUpdateInterface::joinTeam` — catch up to a live teammate.
    fn join_team(&mut self) {}

    /// Snapshot of the unit's locomotor set for pathfinder queries
    /// (matches C++ `AIUpdateInterface::getLocomotorSet()`).
    fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet> {
        None
    }

    /// Check if any path exists to a destination (matches AIUpdateInterface::isPathAvailable).
    fn is_path_available(&self, _destination: &Coord3D) -> bool {
        false
    }

    /// Request a path to a destination (matches AIUpdateInterface::requestPath).
    fn request_path(&mut self, _destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        Ok(())
    }

    /// Request a path to attack a victim (matches AIUpdateInterface::requestAttackPath).
    fn request_attack_path(
        &mut self,
        _victim_id: ObjectID,
        _victim_pos: &Coord3D,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Request an approach path (matches AIUpdateInterface::requestApproachPath).
    fn request_approach_path(&mut self, _destination: &Coord3D) -> Result<(), String> {
        Ok(())
    }

    /// Whether we can compute a quick path (matches AIUpdateInterface::canComputeQuickPath).
    fn can_compute_quick_path(&self) -> bool {
        false
    }

    /// Compute a quick path to destination (matches AIUpdateInterface::computeQuickPath).
    fn compute_quick_path(&mut self, _destination: &Coord3D) -> bool {
        false
    }

    /// Check if a quick path exists to a destination (matches AIUpdateInterface::isQuickPathAvailable).
    fn is_quick_path_available(&self, _destination: &Coord3D) -> bool {
        false
    }

    /// Check if a locomotor position is valid (matches AIUpdateInterface::isValidLocomotorPosition).
    fn is_valid_locomotor_position(&self, _pos: &Coord3D) -> bool {
        false
    }

    /// Whether the unit needs to rotate to align with its path (matches AIUpdateInterface::needToRotate).
    fn need_to_rotate(&self) -> bool {
        false
    }

    /// Current locomotor set type (matches AIUpdateInterface::getCurLocomotorSetType).
    fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        LocomotorSetType::Invalid
    }

    /// Whether any locomotor supports the requested surface type.
    fn has_locomotor_for_surface(&self, _surface: crate::common::LocomotorSurfaceTypeMask) -> bool {
        false
    }

    /// Max locomotor speed for current damage condition (matches AIUpdateInterface::getCurLocomotorSpeed).
    fn get_cur_locomotor_speed(&self) -> Real {
        0.0
    }

    /// Current max blocked speed (matches AIUpdateInterface::m_curMaxBlockedSpeed).
    fn get_cur_max_blocked_speed(&self) -> Real {
        FAST_AS_POSSIBLE
    }

    /// Set current max blocked speed (matches AIUpdateInterface::m_curMaxBlockedSpeed).
    fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        let _ = speed;
    }
    /// Get current crate ID (matching C++ AIUpdateInterface::getCrateID)
    fn get_crate_id(&self) -> ObjectID {
        crate::common::INVALID_ID
    }
    /// Get current victim target (matching C++ AIUpdateInterface::getCurrentVictim).
    fn get_current_victim(&self) -> Option<ObjectID> {
        None
    }
    /// Set current victim target (matching C++ AIUpdateInterface::setCurrentVictim).
    fn set_current_victim(&mut self, _victim: Option<ObjectID>) {}
    /// Check for crate to pick up (matching C++ AIUpdateInterface::checkForCrateToPickup)
    fn check_for_crate_to_pickup_id(&self) -> ObjectID {
        crate::common::INVALID_ID
    }
    fn check_for_crate_to_pickup(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 340: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.check_for_crate_to_pickup_id();
        if id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }
    }
    /// Get next target based on mood/auto-acquire (matching C++ AIUpdateInterface::getNextMoodTarget)
    fn get_next_mood_target_id(
        &mut self,
        _use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> ObjectID {
        crate::common::INVALID_ID
    }
    fn get_next_mood_target(
        &mut self,
        use_existing_target: bool,
        ignore_attacked: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        // Wave 340: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.get_next_mood_target_id(use_existing_target, ignore_attacked);
        if id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }
    }
    /// Get next mood check time (matching C++ AIUpdateInterface::getNextMoodCheckTime)
    fn get_next_mood_check_time(&self) -> u32 {
        TheGameLogic::get_frame()
    }
    /// Reset next mood check time (matching C++ AIUpdateInterface::resetNextMoodCheckTime)
    fn reset_next_mood_check_time(&mut self) {}
    /// Set next mood check time (matching C++ AIUpdateInterface::setNextMoodCheckTime)
    fn set_next_mood_check_time(&mut self, _frame: u32) {}
    /// Get packed mood matrix parameters (matching C++ AIUpdateInterface::getMoodMatrixValue).
    fn get_mood_matrix_value(&self) -> u32 {
        0
    }
    /// Mood matrix action adjustment (matching C++ AIUpdateInterface::getMoodMatrixActionAdjustment)
    fn get_mood_matrix_action_adjustment(&mut self, _action: crate::ai::MoodMatrixAction) -> u32 {
        0
    }

    /// Notify AI that a shot has been fired (matches C++ AIAttackState::notifyFired).
    fn notify_fired(&mut self) {
        let _ = self.is_in_attack_state();
    }

    /// Notify AI that a new victim was chosen (matches C++ AIAttackState::notifyNewVictimChosen).
    fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        let _ = victim;
    }

    /// Whether a given weapon slot is allowed to fire for this attack (matches C++ isWeaponSlotOkToFire).
    fn is_weapon_slot_ok_to_fire(&self, _wslot: crate::weapon::WeaponSlotType) -> Bool {
        true
    }

    /// Original victim position for attack continuation (matches AIAttackState::getOriginalVictimPos).
    fn get_original_victim_pos(&self) -> Option<Coord3D> {
        None
    }

    /// Set original victim position for attack continuation.
    fn set_original_victim_pos(&mut self, _pos: Option<Coord3D>) {}

    /// Whether the current AI state machine is in an attack state.
    fn is_in_attack_state(&self) -> bool {
        false
    }

    /// Whether the current AI state machine is in guard idle.
    fn is_in_guard_idle_state(&self) -> bool {
        false
    }

    /// Set a temporary AI state (matches AIStateMachine::setTemporaryState).
    fn set_temporary_state(&mut self, _state: AIStateType, _frame_limit: UnsignedInt) {}
    /// Notify AI about a crate created by this unit (matching C++ AIUpdateInterface::notifyCrate)
    fn notify_crate(&mut self, crate_id: ObjectID) {
        let _ = crate_id;
    }

    /// Notify AI that its current victim died (matches C++ AIUpdateInterface::notifyVictimIsDead).
    fn notify_victim_is_dead(&mut self) {
        self.set_goal_object(None);
    }
    /// Record prior waypoint ID (matching C++ setPriorWaypointID)
    fn set_prior_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {}
    /// Record current waypoint ID (matching C++ setCurrentWaypointID)
    fn set_current_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {}
    /// Record completed waypoint (matching C++ setCompletedWaypoint)
    fn set_completed_waypoint_id(&mut self, _waypoint_id: Option<crate::waypoint::WaypointId>) {}
    /// Get most recently completed waypoint (matching C++ getCompletedWaypoint)
    fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        None
    }

    /// Check if clear to advance
    fn is_clear_to_advance(
        &self,
        _obj_id: ObjectID,
        _approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Reserve approach position
    fn reserve_approach_position(
        &mut self,
        _obj_id: ObjectID,
        _goal_pos: &mut Coord3D,
        _approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Execute a command packet (matching C++ AIUpdateInterface::ExecuteCommand)
    fn execute_command(
        &mut self,
        _command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Set locomotor upgrade flag (matching C++ AIUpdateInterface::setLocomotorUpgrade).
    fn set_locomotor_upgrade(
        &mut self,
        _enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Allow AI-issued commands to chase targets (matching C++ AIUpdateInterface::setAllowedToChase).
    fn set_allow_chase(&mut self, allowed: bool) {
        let _ = allowed;
    }

    /// Select locomotor set (matching C++ AIUpdateInterface::chooseLocomotorSet).
    fn choose_locomotor_set(
        &mut self,
        _set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Allow invalid positions (matching C++ Locomotor::setAllowInvalidPosition).
    fn set_allow_invalid_position(
        &mut self,
        _allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Toggle ultra-accurate pathing (matching C++ Locomotor::setUltraAccurate).
    fn set_ultra_accurate(
        &mut self,
        _ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Toggle precise Z positioning for pathing (matching C++ Locomotor::setPreciseZPos).
    fn set_precise_z_pos(
        &mut self,
        _precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// On approach reached
    fn on_approach_reached(
        &mut self,
        _obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Advance approach position
    fn advance_approach_position(
        &mut self,
        _obj_id: ObjectID,
        _goal_pos: &mut Coord3D,
        _approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Check if clear to enter
    fn is_clear_to_enter(
        &self,
        _obj_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Get enter position
    fn get_enter_position(
        &self,
        _obj_id: ObjectID,
        _goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// On enter reached
    fn on_enter_reached(
        &mut self,
        _obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get dock position
    fn get_dock_position(
        &self,
        _obj_id: ObjectID,
        _goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Check if allow passthrough type
    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }

    /// Get speed
    fn get_speed(&self) -> f32 {
        1.0
    }

    /// AI move to position
    fn ai_move_to_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI move to and evacuate
    fn ai_move_to_and_evacuate(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI idle
    fn ai_idle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI hunt
    fn ai_hunt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI force attack object
    fn ai_force_attack_object(
        &mut self,
        _target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI attack object
    fn ai_attack_object(
        &mut self,
        _target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI attack position
    fn ai_attack_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI guard position
    fn ai_guard_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI guard object
    fn ai_guard_object(
        &mut self,
        _target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Trigger prone behavior (matching C++ AIUpdateInterface::privateGoProne)
    fn ai_go_prone(&mut self, _damage_info: &DamageInfo, _cmd_source: CommandSourceType) {
        // Default implementation does nothing
    }

    /// AI busy (matching C++ AIUpdateInterface::aiBusy)
    fn ai_busy(
        &mut self,
        _cmd_source: crate::ai::CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Check if busy
    fn is_busy(&self) -> bool {
        false
    }

    /// Set attitude
    fn set_attitude(
        &mut self,
        _attitude: AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Set recruitable state (matches C++ AIUpdateInterface::setIsRecruitable).
    fn set_is_recruitable(&mut self, _recruitable: Bool) {}

    /// Get recruitable state (matches C++ AIUpdateInterface::isRecruitable).
    /// Default true: units without an override stay recruitable.
    fn is_recruitable(&self) -> bool {
        true
    }

    /// Get attitude
    fn get_attitude(&self) -> AIAttitudeType {
        AIAttitudeType::Normal
    }

    /// Check if AI is in dead state (used by slow death behaviors)
    fn is_ai_in_dead_state(&self) -> bool {
        false // Default: AI is not in dead state
    }

    /// Mark AI as dead (prevents other behaviors from handling death)
    fn mark_as_dead(&mut self) {
        // Default implementation does nothing - subclasses should track this state
    }

    /// Get which turret is used for current weapon
    /// Matches C++ AIUpdateInterface::GetWhichTurretForCurWeapon
    fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        TurretType::Invalid
    }

    /// Get which turret is used for a weapon slot
    /// Matches C++ AIUpdateInterface::GetWhichTurretForWeaponSlot
    fn get_which_turret_for_weapon_slot(&self, _slot: crate::weapon::WeaponSlotType) -> TurretType {
        TurretType::Invalid
    }

    /// Set turret enabled state
    /// Matches C++ AIUpdateInterface::SetTurretEnabled
    fn set_turret_enabled(&mut self, _turret: TurretType, _enabled: bool) {
        // Default implementation does nothing
    }

    /// Recenter turret to natural position
    /// Matches C++ AIUpdateInterface::RecenterTurret
    fn recenter_turret(&mut self, _turret: TurretType) {
        // Default implementation does nothing
    }

    /// Set extra distance used when following paths (matches AIUpdateInterface::setPathExtraDistance)
    fn set_path_extra_distance(
        &mut self,
        _distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Build a path from a waypoint chain (matches AIUpdateInterface::setPathFromWaypoint).
    fn set_path_from_waypoint(
        &mut self,
        _waypoint: &crate::waypoint::Waypoint,
        _group_offset: &Coord2D,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Check whether the waypoint queue is empty (used by waypoint path exact follow).
    fn is_waypoint_queue_empty(&self) -> bool {
        true
    }

    /// Check if AI is waiting for path (matches AIUpdateInterface::isWaitingForPath).
    /// C++ AIUpdateInterface::doPathfind — process queued path request.
    fn do_pathfind(&mut self) {}

    fn is_waiting_for_path(&self) -> bool {
        false
    }

    /// Append a goal position to the current path (used for off-map movement).
    fn append_goal_position_to_path(&mut self, _goal: &Coord3D) -> Result<(), String> {
        Ok(())
    }

    /// Replace current path with an explicit waypoint list (legacy-safe path support).
    fn set_path_from_coords(&mut self, _path: &[Coord3D]) -> Result<(), String> {
        Ok(())
    }

    /// Request a safe path away from a repulsor object (matches AIUpdateInterface::requestSafePath).
    fn request_safe_path(&mut self, _repulsor_id: ObjectID) -> Result<bool, String> {
        Ok(false)
    }

    /// Returns whether current movement is ground-based (matches AIUpdateInterface::isDoingGroundMovement).
    fn is_doing_ground_movement(&self) -> bool {
        true
    }

    /// Update pathfinding goal position for this unit.
    fn update_goal_position(
        &mut self,
        _goal: &Coord3D,
        _layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Adjust destination to a nearby passable location (returns false if no adjustment found).
    fn adjust_destination(&mut self, _goal: &mut Coord3D) -> bool {
        true
    }

    /// Set whether path following should adjust destinations (matches AIInternalMoveToState logic).
    fn set_adjusts_destination(&mut self, adjust: bool) {
        let _ = adjust;
    }

    /// Check if turret is in natural position
    /// Matches C++ AIUpdateInterface::IsTurretInNaturalPosition
    fn is_turret_in_natural_position(&self, _turret: TurretType) -> bool {
        true // Default: assume turret is centered
    }

    /// Check if turret is enabled (matches C++ AIUpdateInterface::isTurretEnabled).
    fn is_turret_enabled(&self, _turret: TurretType) -> bool {
        true
    }

    /// Get turret rotation and pitch (matches C++ AIUpdateInterface::getTurretRotAndPitch).
    fn get_turret_rot_and_pitch(&self, _turret: TurretType) -> Option<(Real, Real)> {
        None
    }

    /// Turret angle in radians (matches C++ AIUpdateInterface::getTurretAngle).
    fn get_turret_angle(&self, _turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(_turret)
            .map(|(angle, _)| angle)
            .unwrap_or(0.0)
    }

    /// Turret pitch in radians (matches C++ AIUpdateInterface::getTurretPitch).
    fn get_turret_pitch(&self, _turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(_turret)
            .map(|(_, pitch)| pitch)
            .unwrap_or(0.0)
    }

    /// C++ parity: AIUpdateInterface::queueWaypoint() — store waypoint without starting execution
    fn queue_waypoint(&mut self, _pos: &Coord3D) {}

    /// C++ parity: AIUpdateInterface::executeWaypointQueue() — start the first queued waypoint
    fn execute_waypoint_queue(&mut self) {}

    /// C++ AIUpdateInterface::clearWaypointQueue — drop queued waypoints first.
    fn clear_waypoint_queue(&mut self) {}
}

