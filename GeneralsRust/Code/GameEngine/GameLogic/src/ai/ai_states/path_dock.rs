/// AI Follow Path State
#[derive(Debug)]
pub struct AIFollowPathState {
    path_index: usize,
}

impl AIFollowPathState {
    pub fn new() -> Self {
        Self { path_index: 0 }
    }

    fn set_next_goal(&mut self, context: &mut AIStateMachineContext) -> bool {
        // Wave 254: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.path_index >= context.goal_path.len() {
            return false;
        }
        let next = context.goal_path[self.path_index];
        context.goal_position = Some(next);
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_movement_target(&next);
            }
        }
        true
    }
}

impl AIState for AIFollowPathState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if context.goal_path.is_empty() {
            return StateReturnType::Failed;
        }
        self.path_index = 0;
        if self.set_next_goal(context) {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if context.goal_path.is_empty() {
            return StateReturnType::Success;
        }
        if goal_reached(context) {
            self.path_index = self.path_index.saturating_add(1);
            if !self.set_next_goal(context) {
                return StateReturnType::Success;
            }
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
                ai_guard.set_can_path_through_units(false);
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FollowPath
    }
}

/// AI Follow Exit Production Path State
#[derive(Debug)]
pub struct AIFollowExitProductionPathState {
    base: AIFollowPathState,
}

impl AIFollowExitProductionPathState {
    pub fn new() -> Self {
        Self {
            base: AIFollowPathState::new(),
        }
    }
}

impl AIState for AIFollowExitProductionPathState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.base.on_enter(context)
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.base.update(context)
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, exit_type: StateExitType) {
        self.base.on_exit(context, exit_type);
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FollowExitProductionPath
    }
}

/// AI Wait State
#[derive(Debug)]
pub struct AIWaitState {
    wake_frame: u32,
}

impl AIWaitState {
    pub fn new() -> Self {
        Self { wake_frame: 0 }
    }
}

impl AIState for AIWaitState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let delay = if context.int_value > 0 {
            context.int_value as u32
        } else {
            LOGICFRAMES_PER_SECOND
        };
        self.wake_frame = TheGameLogic::get_frame().wrapping_add(delay);
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if TheGameLogic::get_frame() >= self.wake_frame {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        }
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ AIWaitState has no onExit() override -- inherits empty base class default.
        // No temporary state to clean up; wake_frame is recalculated on next on_enter().
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Wait
    }
}

/// AI Dead State
#[derive(Debug)]
pub struct AIDeadState;

impl AIDeadState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIDeadState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            ai.mark_as_dead();
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        match OBJECT_REGISTRY.with_object(context.owner_id, |owner| owner.is_effectively_dead()) {
            None | Some(true) => StateReturnType::Success,
            Some(false) => StateReturnType::Continue,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::DYING);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Dead
    }
}

/// AI Dock State
#[derive(Debug)]
pub struct AIDockState {
    dock_machine: Option<AIDockMachine>,
}

impl AIDockState {
    pub fn new() -> Self {
        Self { dock_machine: None }
    }
}

impl AIState for AIDockState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let has_dock = OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                goal_guard.with_dock_update_interface(|_| true).unwrap_or(false)
            })
            .unwrap_or(false);
        if !has_dock {
            return StateReturnType::Failed;
        }

        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = get_legacy_object(goal_id) else {
            return StateReturnType::Failed;
        };

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner_guard| owner_guard.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.ignore_obstacle(
                    goal_arc.read().ok().map(|g| g.get_id()),
                );
                let _ = ai_guard.set_can_path_through_units(true);
            }
        }

        let mut dock_machine = match AIDockMachine::new(owner_arc.clone()) {
            Ok(machine) => machine,
            Err(_) => return StateReturnType::Failed,
        };
        if let Ok(mut machine) = dock_machine.state_machine.lock() {
            machine.set_goal_object(Some(Arc::downgrade(&goal_arc)));
            let _ = machine.init_default_state();
        }
        self.dock_machine = Some(dock_machine);
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(machine) = self.dock_machine.as_mut() else {
            return StateReturnType::Failed;
        };
        let Ok(mut state_machine) = machine.state_machine.lock() else {
            return StateReturnType::Failed;
        };
        match state_machine.update() {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            result => result,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(mut machine) = self.dock_machine.take() {
            let _ = machine.halt();
        }
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_can_path_through_units(false);
                let _ = ai_guard.ignore_obstacle(None);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Dock
    }
}

