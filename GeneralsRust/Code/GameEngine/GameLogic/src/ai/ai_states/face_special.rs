/// AI Face Object State — C++ AIFaceState (m_obj=true).
#[derive(Debug)]
pub struct AIFaceObjectState {
    can_turn_in_place: bool,
}

impl AIFaceObjectState {
    pub fn new() -> Self {
        Self {
            can_turn_in_place: false,
        }
    }
}

impl AIState for AIFaceObjectState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }
        if context.goal_object.is_none() {
            return StateReturnType::Failed;
        }
        self.can_turn_in_place = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| leftover_can_turn_in_place(owner))
            .unwrap_or(false);
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        leftover_face_update(context, true, self.can_turn_in_place)
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {}

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FaceObject
    }
}

/// AI Face Position State — C++ AIFaceState (m_obj=false).
#[derive(Debug)]
pub struct AIFacePositionState {
    can_turn_in_place: bool,
}

impl AIFacePositionState {
    pub fn new() -> Self {
        Self {
            can_turn_in_place: false,
        }
    }
}

impl AIState for AIFacePositionState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }
        if context.goal_position.is_none() {
            return StateReturnType::Failed;
        }
        self.can_turn_in_place = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| leftover_can_turn_in_place(owner))
            .unwrap_or(false);
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        leftover_face_update(context, false, self.can_turn_in_place)
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {}

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FacePosition
    }
}

fn leftover_can_turn_in_place(owner: &GameObject) -> bool {
    let Some(ai) = owner.get_ai_update_interface() else {
        return false;
    };
    let Ok(ai_guard) = ai.lock() else {
        return false;
    };
    let Some(locomotor) = ai_guard.get_cur_locomotor() else {
        return false;
    };
    locomotor
        .lock()
        .map(|loco| loco.template.min_speed == 0.0)
        .unwrap_or(false)
}

fn leftover_face_update(
    context: &mut AIStateMachineContext,
    face_object: bool,
    can_turn_in_place: bool,
) -> StateReturnType {
    if dual_world_registry_unavailable() {
        return StateReturnType::Failed;
    }
    let target_pos = if face_object {
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(pos) = OBJECT_REGISTRY.with_object(goal_id, |g| *g.get_position()) else {
            return StateReturnType::Failed;
        };
        pos
    } else {
        let Some(pos) = context.goal_position else {
            return StateReturnType::Failed;
        };
        pos
    };
    let Some((owner_pos, owner_ori, ai)) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
        (
            *owner.get_position(),
            owner.get_orientation(),
            owner.get_ai_update_interface(),
        )
    }) else {
        return StateReturnType::Failed;
    };
    let Some(ai) = ai else {
        return StateReturnType::Failed;
    };
    let Ok(mut ai_guard) = ai.lock() else {
        return StateReturnType::Failed;
    };
    let angle_to = (target_pos.y - owner_pos.y).atan2(target_pos.x - owner_pos.x);
    let mut rel = angle_to - owner_ori;
    const PI: f32 = std::f32::consts::PI;
    const TAU: f32 = std::f32::consts::TAU;
    while rel > PI {
        rel -= TAU;
    }
    while rel < -PI {
        rel += TAU;
    }
    if rel.abs() < 0.035 {
        return StateReturnType::Success;
    }
    if can_turn_in_place {
        ai_guard.set_locomotor_goal_orientation(owner_ori + rel);
    } else {
        ai_guard.set_locomotor_goal_position_explicit(target_pos);
    }
    StateReturnType::Continue
}

/// AI Rappel Into State
#[derive(Debug)]
pub struct AIRappelIntoState;

impl AIRappelIntoState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIRappelIntoState {
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
                    AiCommandType::RappelInto,
                    CommandSourceType::FromAi,
                );
                params.obj = context.goal_object;
                if let Some(goal_pos) = context.goal_position {
                    params.pos = goal_pos;
                }
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        });
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.set_desired_speed(f32::MAX);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::RappelInto
    }
}

/// AI Combat Drop State
#[derive(Debug)]
pub struct AICombatDropState;

impl AICombatDropState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AICombatDropState {
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
                    AiCommandType::CombatDrop,
                    CommandSourceType::FromAi,
                );
                params.obj = context.goal_object;
                if let Some(goal_pos) = context.goal_position {
                    params.pos = goal_pos;
                }
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        // C++ ChinookCombatDropState::onExit clears DISABLED_HELD, sets flight status to FLYING,
        // idles any rappellers if the owner died, and expires rope drawables.
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_disabled(crate::common::DisabledType::Held);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::CombatDrop
    }
}

/// AI Busy State
#[derive(Debug)]
pub struct AIBusyState;

impl AIBusyState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIBusyState {
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
                let params =
                    AiCommandParams::new(AiCommandType::Busy, CommandSourceType::FromAi);
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

        let idle = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner_guard| {
                owner_guard.get_ai_update_interface().and_then(|ai| {
                    ai.lock().ok().map(|ai_guard| ai_guard.is_idle())
                })
            })
            .flatten()
            .unwrap_or(true);
        if idle {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        }
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ AIBusyState::onExit() is genuinely empty -- inline in AIStateMachine.h line 325.
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Busy
    }
}

/// AI Exit Instantly State
#[derive(Debug)]
pub struct AIExitInstantlyState;

impl AIExitInstantlyState {
    pub fn new() -> Self {
        Self
    }

    fn release_from_container(owner: &GameObject) {
        if let Some(container_id) = owner.get_container_id() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(container_id, |container| {
                if let Some(contain) = container.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        let _ = contain_guard.release_object(owner.get_id());
                    }
                }
            });
        }
    }

    fn evacuate_contents(owner: &mut GameObject) {
        if let Some(contain) = owner.get_contain() {
            if let Ok(mut contain_guard) = contain.lock() {
                let ids: Vec<ObjectID> = contain_guard.get_contained_objects().to_vec();
                for id in ids {
                    let _ = contain_guard.release_object(id);
                }
            }
        }
    }
}

impl AIState for AIExitInstantlyState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(ok) = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return false;
            }
            Self::release_from_container(owner);
            Self::evacuate_contents(owner);
            true
        }) else {
            return StateReturnType::Failed;
        };
        if ok {
            StateReturnType::Success
        } else {
            StateReturnType::Failed
        }
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
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
        AIStateType::ExitInstantly
    }
}

/// AI Get Repaired State
#[derive(Debug)]
pub struct AIGetRepairedState {
    goal_position: Coord3D,
}

impl AIGetRepairedState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl AIState for AIGetRepairedState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_pos) = resolve_goal_position(context) else {
            return StateReturnType::Failed;
        };
        self.goal_position = goal_pos;
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(dead) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            owner.is_effectively_dead()
        }) else {
            return StateReturnType::Failed;
        };
        if dead {
            return StateReturnType::Failed;
        }
        if goal_reached(context) {
            return StateReturnType::Success;
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        // C++ has no AIGetRepairedState class -- GetRepaired delegates to AIDockState/landing states.
        // Destroy any path that may have been computed for the repair depot approach.
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
        AIStateType::GetRepaired
    }
}

