/// AI Enter State
#[derive(Debug)]
pub struct AIEnterState {
    entry_to_clear: ObjectID,
}

impl AIEnterState {
    pub fn new() -> Self {
        Self {
            entry_to_clear: INVALID_ID,
        }
    }
}

impl AIState for AIEnterState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        self.entry_to_clear = INVALID_ID;
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = OBJECT_REGISTRY.get_object(goal_id) else {
            return StateReturnType::Failed;
        };

        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        let Ok(goal_guard) = goal_arc.read() else {
            return StateReturnType::Failed;
        };
        let Some(contain) = goal_guard.get_contain() else {
            return StateReturnType::Failed;
        };
        let Ok(mut contain_guard) = contain.lock() else {
            return StateReturnType::Failed;
        };
        if !contain_guard.is_valid_container_for(&*owner_guard, true) {
            return StateReturnType::Failed;
        }
        let _ = contain_guard
            .on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToEnter);
        drop(contain_guard);

        let goal_pos = *goal_guard.get_position();
        context.goal_position = Some(goal_pos);

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_allow_invalid_position(true);
                let legacy_goal = get_legacy_object(goal_id);
                let _ = ai_guard.ignore_obstacle(legacy_goal.as_ref().and_then(|a| a.read().ok().map(|g| g.get_id())));
                let _ = ai_guard.set_movement_target(&goal_pos);
            }
        }

        self.entry_to_clear = goal_id;
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = OBJECT_REGISTRY.get_object(goal_id) else {
            return StateReturnType::Failed;
        };

        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        let Ok(goal_guard) = goal_arc.read() else {
            return StateReturnType::Failed;
        };

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let current_goal = context
                    .goal_position
                    .unwrap_or_else(|| *goal_guard.get_position());
                if current_goal != *goal_guard.get_position() {
                    let new_goal = *goal_guard.get_position();
                    context.goal_position = Some(new_goal);
                    let _ = ai_guard.set_movement_target(&new_goal);
                }
            }
        }

        let Some(contain) = goal_guard.get_contain() else {
            return StateReturnType::Failed;
        };
        let Ok(mut contain_guard) = contain.lock() else {
            return StateReturnType::Failed;
        };
        if !contain_guard.is_valid_container_for(&*owner_guard, true) {
            return StateReturnType::Failed;
        }

        let owner_pos = owner_guard.get_position();
        let goal_pos = goal_guard.get_position();
        let dx = owner_pos.x - goal_pos.x;
        let dy = owner_pos.y - goal_pos.y;
        let radius = goal_guard.get_geometry_info().get_major_radius();
        if dx * dx + dy * dy <= radius * radius {
            let _ = contain_guard.add_to_contain(&*owner_guard);
            return StateReturnType::Success;
        }

        StateReturnType::Continue
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
                let _ = ai_guard.set_allow_invalid_position(false);
                let _ = ai_guard.ignore_obstacle(None);
            }
        }
        if self.entry_to_clear != INVALID_ID {
            if let Some(goal_arc) = OBJECT_REGISTRY.get_object(self.entry_to_clear) {
                if let Ok(goal_guard) = goal_arc.read() {
                    if let Some(contain) = goal_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            if let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) {
                                if let Ok(owner_guard) = owner_arc.read() {
                                    let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                        &*owner_guard,
                                        ContainWant::WantsNeither,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        self.entry_to_clear = INVALID_ID;
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Enter
    }
}

/// AI Exit State
#[derive(Debug)]
pub struct AIExitState;

impl AIExitState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIExitState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        let Some(container_id) = owner_guard.get_contained_by() else {
            return StateReturnType::Success;
        };
        let Some(container_arc) = OBJECT_REGISTRY.get_object(container_id) else {
            return StateReturnType::Failed;
        };
        let Ok(container_guard) = container_arc.read() else {
            return StateReturnType::Failed;
        };
        let Some(contain) = container_guard.get_contain() else {
            return StateReturnType::Failed;
        };
        let Ok(mut contain_guard) = contain.lock() else {
            return StateReturnType::Failed;
        };
        let _ =
            contain_guard.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToExit);
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        match OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_contained_by().is_none())
        {
            None | Some(true) => StateReturnType::Success,
            Some(false) => StateReturnType::Continue,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            let Some(container_id) = owner_guard.get_contained_by() else {
                return;
            };
            let Some(contain) = OBJECT_REGISTRY
                .with_object(container_id, |container_guard| container_guard.get_contain())
                .flatten()
            else {
                return;
            };
            if let Ok(mut contain_guard) = contain.lock() {
                let _ = contain_guard.on_object_wants_to_enter_or_exit(
                    owner_guard,
                    ContainWant::WantsNeither,
                );
            };
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Exit
    }
}

/// AI Pick Up Crate State
#[derive(Debug)]
pub struct AIPickUpCrateState;

impl AIPickUpCrateState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIPickUpCrateState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = get_legacy_object(goal_id) else {
            return StateReturnType::Failed;
        };

        if let Ok(owner_guard) = owner_arc.read() {
            if let Ok(goal_guard) = goal_arc.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_movement_target(goal_guard.get_position());
                    }
                }
            }
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Success;
        };
        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = get_legacy_object(goal_id) else {
            return StateReturnType::Success;
        };

        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Continue;
        };
        let Ok(goal_guard) = goal_arc.read() else {
            return StateReturnType::Success;
        };

        let owner_pos = owner_guard.get_position();
        let goal_pos = goal_guard.get_position();
        let dx = owner_pos.x - goal_pos.x;
        let dy = owner_pos.y - goal_pos.y;
        let dist_sqr = dx * dx + dy * dy;
        if dist_sqr <= CRATE_PICKUP_RANGE_SQR {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
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
        AIStateType::PickUpCrate
    }
}

/// AI Attack Squad State
#[derive(Debug)]
pub struct AIAttackSquadState {
    attack_machine: Option<AIAttackThenIdleStateMachine>,
}

impl AIAttackSquadState {
    pub fn new() -> Self {
        Self {
            attack_machine: None,
        }
    }
}

impl AIState for AIAttackSquadState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let mut attack_machine = AIAttackThenIdleStateMachine::new(
            Arc::downgrade(&owner_arc),
            "AIAttackSquadStateMachine",
        );
        let result = attack_machine.init_default_state();
        self.attack_machine = Some(attack_machine);
        result
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Failed;
        };
        // Fallback behavior: attack closest enemy when no squad context is available.
        if attack_machine.get_current_state_id() == Some(LegacyAIStateType::Idle as u32) {
            let attack_priority = resolve_attack_priority_info_for_object(context.owner_id);
            let ai_store = the_ai(); if let Ok(ai) = ai_store.read() {
                if let Ok(Some(victim)) = ai.find_closest_enemy(
                    context.owner_id,
                    9999.9,
                    search_qualifiers::CAN_ATTACK,
                    attack_priority.as_ref(),
                    None,
                ) {
                    if let Some(victim_arc) = get_legacy_object(victim) {
                        attack_machine.set_goal_object(victim_arc.read().ok().map(|g| g.get_id()));
                        let _ = attack_machine.set_state(LegacyAIStateType::AttackObject);
                    }
                }
            }
        }
        attack_machine.update()
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::AttackSquad
    }
}

/// AI Hack Internet State
#[derive(Debug)]
pub struct AIHackInternetState;

impl AIHackInternetState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIHackInternetState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let mut params = AiCommandParams::new(
                    AiCommandType::HackInternet,
                    CommandSourceType::FromAi,
                );
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(status) = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            let Some(ai) = owner_guard.get_ai_update_interface() else {
                return None; // Failed
            };
            let Ok(mut ai_guard) = ai.lock() else {
                return None; // Failed
            };
            let Some(hack) = ai_guard.get_hack_internet_ai_update_interface() else {
                return Some(false); // Success (not busy)
            };
            Some(hack.is_hacking_packing_or_unpacking())
        })
        .flatten() else {
            return StateReturnType::Failed;
        };
        if status {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        // C++ HackInternetState::onExit clears MODELCONDITION_FIRING_A on the owner.
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::FIRING_A);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::HackInternet
    }
}

