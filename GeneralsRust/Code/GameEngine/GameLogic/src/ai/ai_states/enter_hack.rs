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
        let entered = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner_guard| {
            let goal_pos = OBJECT_REGISTRY.with_object_mut(goal_id, |goal_guard| {
                let Some(contain) = goal_guard.get_contain_mut() else {
                    return None;
                };
                if !contain.is_valid_container_for(owner_guard, true) {
                    return None;
                }
                let _ = contain
                    .on_object_wants_to_enter_or_exit(owner_guard, ContainWant::WantsToEnter);
                Some(*goal_guard.get_position())
            })?;
            context.goal_position = Some(goal_pos);
            if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                let _ = ai_guard.set_allow_invalid_position(true);
                let _ = ai_guard.ignore_obstacle(Some(goal_id));
                let _ = ai_guard.set_movement_target(&goal_pos);
            }
            Some(())
        });
        if entered.is_none() || entered == Some(None) {
            return StateReturnType::Failed;
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
        let result = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner_guard| {
            OBJECT_REGISTRY.with_object_mut(goal_id, |goal_guard| {
                if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                    let current_goal = context
                        .goal_position
                        .unwrap_or_else(|| *goal_guard.get_position());
                    if current_goal != *goal_guard.get_position() {
                        let new_goal = *goal_guard.get_position();
                        context.goal_position = Some(new_goal);
                        let _ = ai_guard.set_movement_target(&new_goal);
                    }
                }
                let owner_pos = *owner_guard.get_position();
                let goal_pos = *goal_guard.get_position();
                let dx = owner_pos.x - goal_pos.x;
                let dy = owner_pos.y - goal_pos.y;
                let radius = goal_guard.get_geometry_info().get_major_radius();
                let Some(contain) = goal_guard.get_contain_mut() else {
                    return StateReturnType::Failed;
                };
                if !contain.is_valid_container_for(owner_guard, true) {
                    return StateReturnType::Failed;
                }
                if dx * dx + dy * dy <= radius * radius {
                    let _ = contain.add_to_contain(owner_guard);
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            })
        });
        match result {
            None | Some(None) => StateReturnType::Failed,
            Some(Some(r)) => r,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            if let Some(ai_guard) = owner.get_ai_update_interface_mut() {
                let _ = ai_guard.set_allow_invalid_position(false);
                let _ = ai_guard.ignore_obstacle(None);
            }
        });
        if self.entry_to_clear != INVALID_ID {
            let entry_id = self.entry_to_clear;
            let _ = OBJECT_REGISTRY.with_object_mut(entry_id, |goal_guard| {
                if let Some(contain) = goal_guard.get_contain_mut() {
                    let _ = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
                        let _ = contain.on_object_wants_to_enter_or_exit(
                            owner_guard,
                            ContainWant::WantsNeither,
                        );
                    });
                }
            });
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

        let result = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            let Some(container_id) = owner_guard.get_contained_by() else {
                return StateReturnType::Success;
            };
            OBJECT_REGISTRY
                .with_object_mut(container_id, |container_guard| {
                    let Some(contain) = container_guard.get_contain_mut() else {
                        return StateReturnType::Failed;
                    };
                    let _ = contain
                        .on_object_wants_to_enter_or_exit(owner_guard, ContainWant::WantsToExit);
                    StateReturnType::Continue
                })
                .unwrap_or(StateReturnType::Failed)
        });
        result.unwrap_or(StateReturnType::Failed)
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
            let _ = OBJECT_REGISTRY.with_object_mut(container_id, |container_guard| {
                if let Some(contain) = container_guard.get_contain_mut() {
                    let _ = contain.on_object_wants_to_enter_or_exit(
                        owner_guard,
                        ContainWant::WantsNeither,
                    );
                }
            });
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
        let moved = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner_guard| {
            let pos = OBJECT_REGISTRY.with_object(goal_id, |goal_guard| *goal_guard.get_position())?;
            if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                let _ = ai_guard.set_movement_target(&pos);
            }
            Some(())
        });
        if moved.is_none() || moved == Some(None) {
            return StateReturnType::Failed;
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
        let near = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                let owner_pos = owner_guard.get_position();
                let goal_pos = goal_guard.get_position();
                let dx = owner_pos.x - goal_pos.x;
                let dy = owner_pos.y - goal_pos.y;
                dx * dx + dy * dy <= CRATE_PICKUP_RANGE_SQR
            })
        });
        match near {
            None => StateReturnType::Failed,
            Some(None) | Some(Some(true)) => StateReturnType::Success,
            Some(Some(false)) => StateReturnType::Continue,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            if let Some(ai_guard) = owner.get_ai_update_interface_mut() {
                ai_guard.destroy_path();
            }
        });
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

        if get_legacy_object(context.owner_id).is_none() {
            return StateReturnType::Failed;
        }
        let mut attack_machine = AIAttackThenIdleStateMachine::new(
            context.owner_id,
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
                    if get_legacy_object(victim).is_some() {
                        attack_machine.set_goal_object(Some(victim));
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

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.ai_pending_hack = true;
            owner.ai_pending_hack_source = crate::common::CommandSourceType::FromAi;
        });
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(status) = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            if !owner_guard.ai_fire_hack_known {
                return None;
            }
            Some(owner_guard.ai_fire_hacking)
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

