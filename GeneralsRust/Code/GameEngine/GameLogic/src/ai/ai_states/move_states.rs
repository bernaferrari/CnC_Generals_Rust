/// AI Idle State
#[derive(Debug)]
pub struct AIIdleState {
    initial_sleep_offset: u16,
    should_look_for_targets: bool,
    inited: bool,
}

impl AIIdleState {
    pub fn new(should_look_for_targets: bool) -> Self {
        Self {
            initial_sleep_offset: 0,
            should_look_for_targets,
            inited: false,
        }
    }

    fn do_init_idle_state(&mut self, context: &AIStateMachineContext) {
        if !self.inited {
            return;
        }
        self.inited = false;
        leftover_restake_idle_pathfinder(context.owner_id);
    }

}

impl AIState for AIIdleState {
    fn on_enter(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        // C++ AIIdleState::onEnter: mark inited so first update restakes.
        self.initial_sleep_offset = game_logic_random_value(0, LOGICFRAMES_PER_SECOND * 2) as u16;
        self.inited = true;
        StateReturnType::Continue
    }


    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // C++ AIIdleState::update calls doInitIdleState on first tick.
        self.do_init_idle_state(context);
        if self.should_look_for_targets {
            // Look for enemies to attack
            // This would interface with targeting system
        }
        StateReturnType::Continue
    }


    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Cleanup when leaving idle state
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Idle
    }

    fn is_idle(&self) -> bool {
        true
    }
}

/// C++ AIIdleState::doInitIdleState first updateGoal (AIStates.cpp:1320-1358).
fn leftover_restake_idle_pathfinder(owner_id: ObjectID) {
    if dual_world_registry_unavailable() {
        return;
    }
    let Some(owner_arc) = OBJECT_REGISTRY.get_object(owner_id) else {
        return;
    };
    let Ok(owner) = owner_arc.read() else {
        return;
    };
    let Some(ai) = owner.get_ai_update_interface() else {
        return;
    };
    let Ok(mut ai_guard) = ai.lock() else {
        return;
    };
    let ultra_accurate = ai_guard
        .get_cur_locomotor()
        .and_then(|loco| loco.lock().ok().map(|l| l.is_ultra_accurate()))
        .unwrap_or(false);
    let pos = *owner.get_position();
    let plan = crate::ai::states::idle_pathfinder_restake_plan(
        ai_guard.is_idle(),
        ai_guard.is_doing_ground_movement(),
        pos,
        ultra_accurate,
    );
    if plan.first_restake {
        let layer = match owner.get_layer() {
            crate::common::PathfindLayerEnum::Invalid
            | crate::common::PathfindLayerEnum::Last => {
                crate::ai::pathfind::PathfindLayerEnum::Invalid
            }
            crate::common::PathfindLayerEnum::Wall => crate::ai::pathfind::PathfindLayerEnum::Wall,
            crate::common::PathfindLayerEnum::Ground
            | crate::common::PathfindLayerEnum::Tunnel
            | crate::common::PathfindLayerEnum::Water
            | crate::common::PathfindLayerEnum::Air => {
                crate::ai::pathfind::PathfindLayerEnum::Ground
            }
            _ => crate::ai::pathfind::PathfindLayerEnum::Top,
        };
        let _ = crate::ai::pathfind::update_goal_for_object(owner_id, &pos, layer);
        if plan.snap {
            if let Some(snapped) = crate::ai::pathfind::goal_position(&pos) {
                if TheGameLogic::get_frame() <= 1 {
                    ai_guard.set_locomotor_goal_none();
                    ai_guard.set_current_victim(None);
                    drop(ai_guard);
                    drop(owner);
                    if let Ok(mut obj_w) = owner_arc.write() {
                        let _ = obj_w.set_position(&snapped);
                    }
                    let _ = crate::ai::pathfind::update_goal_for_object(owner_id, &snapped, layer);
                    return;
                }
                let _ = crate::ai::pathfind::update_goal_for_object(owner_id, &snapped, layer);
            }
        }
    }
    ai_guard.set_locomotor_goal_none();
    ai_guard.set_current_victim(None);
}


/// AI Move To State
#[derive(Debug)]
pub struct AIMoveToState {
    goal_position: Coord3D,
    path_goal_position: Coord3D,
    path_timestamp: u32,
    blocked_repath_timestamp: u32,
    adjust_destinations: bool,
    waiting_for_path: bool,
    try_one_more_repath: bool,
}

impl AIMoveToState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            adjust_destinations: true,
            waiting_for_path: false,
            try_one_more_repath: false,
        }
    }

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

    fn force_repath(&mut self) {
        self.path_goal_position = Coord3D::new(-100.0, -100.0, -100.0);
        self.path_timestamp = 0;
    }
}

impl AIState for AIMoveToState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(goal_pos) = context.goal_position {
            self.goal_position = goal_pos;
            self.compute_path(context);
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Check if we've reached the destination
        if context.goal_position.is_some() {
            // Real completion check depends on locomotor/path integration.
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
        AIStateType::MoveTo
    }
}

/// AI Move Out Of The Way State
/// Matches C++ AIMoveOutOfTheWayState from AIStates.cpp lines 2125-2168.
#[derive(Debug)]
pub struct AIMoveOutOfTheWayState {
    goal_position: Coord3D,
}

impl AIMoveOutOfTheWayState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl AIState for AIMoveOutOfTheWayState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        else {
            return StateReturnType::Failed;
        };
        let Ok(ai_guard) = ai.lock() else {
            return StateReturnType::Failed;
        };
        let Some(goal_pos) = ai_guard.get_path_destination() else {
            return StateReturnType::Failed;
        };

        self.goal_position = goal_pos;
        context.goal_position = Some(goal_pos);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(status) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return None;
            }
            owner.get_ai_update_interface()
        }) else {
            return StateReturnType::Failed;
        };
        let Some(ai) = status else {
            return StateReturnType::Success;
        };
        if let Ok(mut ai_guard) = ai.lock() {
            if ai_guard.is_blocked_and_stuck() {
                let _ = ai_guard.set_can_path_through_units(true);
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

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
                let _ = ai_guard.set_can_path_through_units(false);
                ai_guard.clear_move_out_of_way();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveOutOfTheWay
    }
}

/// AI Move And Evacuate State
#[derive(Debug)]
pub struct AIMoveAndEvacuateState {
    origin: Coord3D,
    goal_position: Coord3D,
    evacuate_and_exit: bool,
}

impl AIMoveAndEvacuateState {
    pub fn new(evacuate_and_exit: bool) -> Self {
        Self {
            origin: Coord3D::new(0.0, 0.0, 0.0),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            evacuate_and_exit,
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

        if let Some(team) = owner.get_team() {
            if let Ok(mut team_guard) = team.write() {
                team_guard.set_active();
            }
        }
    }
}

impl AIState for AIMoveAndEvacuateState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(origin) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return StateReturnType::Failed;
        };
        self.origin = origin;

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

        let Some((ret, destroy)) = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return (StateReturnType::Failed, false);
            }
            if goal_reached(context) {
                Self::evacuate_contents(owner);
                if self.evacuate_and_exit {
                    return (StateReturnType::Success, true);
                }
                return (StateReturnType::Success, false);
            }
            (StateReturnType::Continue, false)
        }) else {
            return StateReturnType::Failed;
        };
        if destroy {
            let _ = TheGameLogic::destroy_object_by_id(context.owner_id);
        }
        ret
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        context.goal_position = Some(self.origin);
    }

    fn get_state_type(&self) -> AIStateType {
        if self.evacuate_and_exit {
            AIStateType::MoveAndEvacuateAndExit
        } else {
            AIStateType::MoveAndEvacuate
        }
    }
}

/// AI Move And Delete State
#[derive(Debug)]
pub struct AIMoveAndDeleteState {
    goal_position: Coord3D,
}

impl AIMoveAndDeleteState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl AIState for AIMoveAndDeleteState {
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

        let Some(should_destroy) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return None;
            }
            Some(goal_reached(context))
        }) else {
            return StateReturnType::Failed;
        };
        let Some(should_destroy) = should_destroy else {
            return StateReturnType::Failed;
        };
        if should_destroy {
            let _ = TheGameLogic::destroy_object_by_id(context.owner_id);
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
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveAndDelete
    }
}

