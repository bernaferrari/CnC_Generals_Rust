/// AI Guard Tunnel Network State
#[derive(Debug)]
pub struct AIGuardTunnelNetworkState {
    guard_mode: GuardMode,
    guard_machine: Option<AITNGuardMachine>,
}

impl AIGuardTunnelNetworkState {
    pub fn new() -> Self {
        Self {
            guard_mode: GuardMode::Normal,
            guard_machine: None,
        }
    }
}

impl AIState for AIGuardTunnelNetworkState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        self.guard_mode = match context.int_value {
            0 => GuardMode::Normal,
            1 => GuardMode::GuardWithoutPursuit,
            2 => GuardMode::GuardFlyingUnitsOnly,
            _ => GuardMode::Normal,
        };

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AITNGuardMachine::new(Arc::downgrade(&owner_arc));
            guard_machine.set_guard_mode(self.guard_mode);
            if let Ok(owner_guard) = owner_arc.read() {
                guard_machine.set_target_position_to_guard(owner_guard.get_position());
            }
            if guard_machine.init_default_state().is_failure() {
                return StateReturnType::Failure;
            }
            let result = guard_machine.set_state(TNGuardStateType::Return);
            self.guard_machine = Some(guard_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(guard_machine) = self.guard_machine.as_mut() {
            return guard_machine.update();
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut guard_machine) = self.guard_machine.take() {
            let _ = guard_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::GuardTunnelNetwork
    }

    fn is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }

    fn is_guard_idle(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_guard_idle_state())
            .unwrap_or(true)
    }
}

/// AI Guard Retaliate State
#[derive(Debug)]
pub struct AIGuardRetaliateState {
    guard_machine: Option<AIGuardRetaliateMachine>,
}

impl AIGuardRetaliateState {
    pub fn new() -> Self {
        Self {
            guard_machine: None,
        }
    }
}

impl AIState for AIGuardRetaliateState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AIGuardRetaliateMachine::new(Arc::downgrade(&owner_arc));
            if let Some(pos) = context.goal_position {
                guard_machine.set_target_position_to_guard(&pos);
            } else if let Ok(owner_guard) = owner_arc.read() {
                guard_machine.set_target_position_to_guard(owner_guard.get_position());
            }
            if let Some(target_id) = context.goal_object {
                guard_machine.set_nemesis_id(target_id);
            }
            if guard_machine.init_default_state().is_failure() {
                return StateReturnType::Failure;
            }
            let result = guard_machine.set_state(GuardRetaliateStateType::Return);
            self.guard_machine = Some(guard_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(guard_machine) = self.guard_machine.as_mut() {
            return guard_machine.update();
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut guard_machine) = self.guard_machine.take() {
            let _ = guard_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::GuardRetaliate
    }

    fn is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// AI Hunt State
#[derive(Debug)]
pub struct AIHuntState {
    next_enemy_scan_time: u32,
    hunt_radius: Real,
    current_target: Option<ObjectID>,
    hunt_machine: Option<AIAttackThenIdleStateMachine>,
}

impl AIHuntState {
    pub fn new() -> Self {
        Self {
            next_enemy_scan_time: 0,
            hunt_radius: 9999.9,
            current_target: None,
            hunt_machine: None,
        }
    }

    fn scan_for_enemies(&mut self, context: &mut AIStateMachineContext) -> Option<ObjectID> {
        let ai_store = the_ai();let ai = ai_store.read().ok()?;
        let attack_priority = resolve_attack_priority_info_for_object(context.owner_id);
        ai.find_closest_enemy(
            context.owner_id,
            self.hunt_radius,
            search_qualifiers::CAN_ATTACK,
            attack_priority.as_ref(),
            None,
        )
        .ok()
        .flatten()
    }
}

impl AIState for AIHuntState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        self.current_target = None;
        self.hunt_machine = None;

        let now = TheGameLogic::get_frame();
        let sleep_time = game_logic_random_value(0, LOGICFRAMES_PER_SECOND);
        self.next_enemy_scan_time = now.wrapping_add(sleep_time);

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut hunt_machine = AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner_arc),
                "AIAttackThenIdleStateMachine",
            );
            let result = hunt_machine.init_default_state();
            self.hunt_machine = Some(hunt_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        if self.hunt_machine.is_none() {
            return StateReturnType::Failed;
        }

        let current_frame = TheGameLogic::get_frame();
        if current_frame >= self.next_enemy_scan_time {
            let Some(owner_arc) = get_legacy_object(context.owner_id) else {
                return StateReturnType::Failed;
            };
            let Ok(owner) = owner_arc.read() else {
                return StateReturnType::Failed;
            };

            if owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile) {
                return StateReturnType::Failed;
            }

            if let Some(ai) = owner.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                        if let Some(hunt_machine) = self.hunt_machine.as_mut() {
                            hunt_machine
                                .set_goal_object(crate_obj.read().ok().map(|g| g.get_id()));
                            let _ = hunt_machine.set_state(LegacyAIStateType::PickUpCrate);
                        }
                        return StateReturnType::Continue;
                    }
                }
            }

            self.next_enemy_scan_time = current_frame + LOGICFRAMES_PER_SECOND;
            let units_should_hunt = owner
                .get_controlling_player()
                .and_then(|player_arc| {
                    player_arc
                        .read()
                        .ok()
                        .map(|player| player.get_units_should_hunt())
                })
                .unwrap_or(true);
            drop(owner);

            let victim = self.scan_for_enemies(context);
            self.current_target = victim;
            let victim_arc = victim.and_then(get_legacy_object);
            let Some(hunt_machine) = self.hunt_machine.as_mut() else {
                return StateReturnType::Failed;
            };
            hunt_machine.set_goal_object(
                victim_arc
                    .as_ref()
                    .and_then(|a| a.read().ok().map(|g| g.get_id())),
            );

            if hunt_machine.get_current_state_id() == Some(LegacyAIStateType::Idle as u32)
                && victim_arc.is_some()
            {
                let _ = hunt_machine.set_state(LegacyAIStateType::AttackObject);
            }

            if !units_should_hunt
                && hunt_machine.get_current_state_id() == Some(LegacyAIStateType::Idle as u32)
                && victim_arc.is_none()
            {
                return StateReturnType::Complete;
            }
        }

        self.hunt_machine
            .as_mut()
            .map(|hunt_machine| hunt_machine.update())
            .unwrap_or(StateReturnType::Failed)
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        self.current_target = None;
        if let Some(mut hunt_machine) = self.hunt_machine.take() {
            let _ = hunt_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Hunt
    }

    fn is_attack(&self) -> bool {
        self.current_target.is_some()
    }
}

/// AI Move And Tighten State
///
/// This state is used to tighten up group formations when units are too spread out.
/// Matches C++ AIMoveAndTightenState from AIStates.cpp lines 2181-2250
#[derive(Debug)]
pub struct AIMoveAndTightenState {
    goal_position: Coord3D,
    path_goal_position: Coord3D,
    path_timestamp: u32,
    ok_to_repath_times: i32,
    check_for_path: bool,
    waiting_for_path: bool,
    formation_config: FormationConfig,
    spread_threshold: Real,
}

impl AIMoveAndTightenState {
    fn compute_path(&mut self, context: &mut AIStateMachineContext) -> bool {
        // Wave 254: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(goal_pos) = context.goal_position else {
            return false;
        };
        let Some(start_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return false;
        };

        let path_result = with_ai_integration(|manager| {
            manager.request_pathfinding(&start_pos, &goal_pos, SURFACE_GROUND, false)
        });

        if let Some(Ok(Some(path))) = path_result {
            context.goal_path = path;
            self.waiting_for_path = false;
            return true;
        }

        context.goal_path = vec![goal_pos];
        self.waiting_for_path = false;
        true
    }

    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            ok_to_repath_times: 1,
            check_for_path: true,
            waiting_for_path: false,
            formation_config: FormationConfig::default(),
            spread_threshold: 50.0, // Matches C++ default threshold
        }
    }

    /// Check if group needs tightening
    ///
    /// # Arguments
    ///
    /// * `group_positions` - Positions of all units in the group
    ///
    /// # Returns
    ///
    /// Returns true if the group spread exceeds the threshold
    pub fn needs_tightening(&self, group_positions: &[Coord3D]) -> bool {
        is_group_too_spread(group_positions, self.spread_threshold)
    }

    /// Calculate spread distance for diagnostic purposes
    pub fn get_group_spread(&self, group_positions: &[Coord3D]) -> Real {
        calculate_group_spread(group_positions)
    }
}

impl AIState for AIMoveAndTightenState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Matches C++ AIMoveAndTightenState::onEnter() from AIStates.cpp line 2211
        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.waiting_for_path = false;

        if let Some(goal_pos) = context.goal_position {
            self.goal_position = goal_pos;
            self.compute_path(context);
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Matches C++ AIMoveAndTightenState::update() from AIStates.cpp line 2225

        if self.check_for_path {
            if !self.waiting_for_path && !context.goal_path.is_empty() {
                self.check_for_path = false;
            }
        }

        // Check if we've reached the destination
        if context.goal_position.is_some() {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveAndTighten
    }
}

/// AI Move Away From Repulsors State
/// Matches C++ AIMoveAwayFromRepulsorsState from AIStates.cpp lines 2263-2312
#[derive(Debug)]
pub struct AIMoveAwayFromRepulsorsState {
    goal_position: Coord3D,
    ok_to_repath_times: i32,
    check_for_path: bool,
    waiting_for_path: bool,
}

impl AIMoveAwayFromRepulsorsState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            ok_to_repath_times: 1,
            check_for_path: true,
            waiting_for_path: false,
        }
    }

    fn compute_path(&mut self, context: &mut AIStateMachineContext) -> bool {
        // Wave 254: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.ok_to_repath_times <= 0 {
            return false;
        }
        self.ok_to_repath_times -= 1;

        let Some(goal_pos) = context.goal_position else {
            return false;
        };
        let Some(start_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return false;
        };

        let path_result = with_ai_integration(|manager| {
            manager.request_pathfinding(&start_pos, &goal_pos, SURFACE_GROUND, false)
        });

        if let Some(Ok(Some(path))) = path_result {
            context.goal_path = path;
            self.waiting_for_path = false;
            return true;
        }

        context.goal_path = vec![goal_pos];
        self.waiting_for_path = false;
        true
    }
}

impl AIState for AIMoveAwayFromRepulsorsState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Ok(owner) = owner_arc.read() else {
            return StateReturnType::Failed;
        };

        let ai_store = the_ai();let enemy_id = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.find_closest_repulsor(context.owner_id, owner.get_vision_range())
                    .ok()
            })
            .flatten();
        let Some(enemy_id) = enemy_id else {
            return StateReturnType::Failed;
        };
        let Some(enemy_arc) = OBJECT_REGISTRY.get_object(enemy_id) else {
            return StateReturnType::Failed;
        };
        let Ok(enemy) = enemy_arc.read() else {
            return StateReturnType::Failed;
        };

        if let Some(ai) = owner.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Panic);
            }
        }

        let owner_pos = *owner.get_position();
        let enemy_pos = *enemy.get_position();
        let mut dx = owner_pos.x - enemy_pos.x;
        let mut dy = owner_pos.y - enemy_pos.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            dx = 1.0;
            dy = 0.0;
        } else {
            dx /= len;
            dy /= len;
        }

        let flee_dist = owner.get_vision_range();
        self.goal_position = Coord3D::new(
            owner_pos.x + dx * flee_dist,
            owner_pos.y + dy * flee_dist,
            owner_pos.z,
        );
        context.goal_position = Some(self.goal_position);

        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.waiting_for_path = false;
        self.compute_path(context);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if self.check_for_path {
            if !self.waiting_for_path && !context.goal_path.is_empty() {
                self.check_for_path = false;
                if let Some(last) = context.goal_path.last().copied() {
                    self.goal_position = last;
                    context.goal_position = Some(last);
                }
            }
        }

        if context.goal_position.is_some() {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::PANICKING);
        });
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
                ai_guard.choose_locomotor_set(LocomotorSetType::Normal);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveAwayFromRepulsors
    }
}

