/// AI Follow Waypoint Path State
#[derive(Debug)]
pub struct AIFollowWaypointPathState {
    move_as_group: bool,
    is_follow_waypoint_path_state: bool,
    attack_follow: bool,
    group_offset: Coord2D,
    angle: Real,
    frames_sleeping: i32,
    current_waypoint: Option<u32>,
    prior_waypoint: Option<u32>,
    append_goal_position: bool,
    exact_path: bool,
}

impl AIFollowWaypointPathState {
    fn get_waypoint_link_count(&self, waypoint_id: u32) -> usize {
        let Ok(terrain) = get_terrain_logic().read() else {
            return 0;
        };
        terrain
            .get_waypoint_by_id(waypoint_id)
            .map(|w| w.get_num_links())
            .unwrap_or(0)
    }

    fn get_waypoint_link(&self, waypoint_id: u32, index: usize) -> Option<u32> {
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain
            .get_waypoint_by_id(waypoint_id)
            .and_then(|w| w.get_link(index))
    }

    fn get_waypoint_location(&self, waypoint_id: u32) -> Option<Coord3D> {
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain
            .get_waypoint_by_id(waypoint_id)
            .map(|w| *w.get_location())
    }

    fn calc_extra_path_distance(&self) -> Real {
        let mut extra = PATHFIND_CELL_SIZE_F / 10.0;
        let mut current = self.current_waypoint;
        let mut limit = 5;

        while let Some(current_id) = current {
            if limit == 0 {
                break;
            }
            limit -= 1;
            let link_count = self.get_waypoint_link_count(current_id);
            if link_count == 0 {
                return extra;
            }
            let Some(next_id) = self.get_waypoint_link(current_id, 0) else {
                return extra;
            };
            let Some(cur_loc) = self.get_waypoint_location(current_id) else {
                return extra;
            };
            let Some(next_loc) = self.get_waypoint_location(next_id) else {
                return extra;
            };
            let dx = next_loc.x - cur_loc.x;
            let dy = next_loc.y - cur_loc.y;
            extra += (dx * dx + dy * dy).sqrt();
            current = Some(next_id);
        }

        extra
    }

    fn get_next_waypoint(&mut self) -> Option<u32> {
        let current_id = self.current_waypoint?;
        let link_count = self.get_waypoint_link_count(current_id);
        if link_count == 0 {
            return None;
        }

        if link_count == 1 {
            let next_id = self.get_waypoint_link(current_id, 0)?;
            if self.prior_waypoint == Some(next_id) {
                return None;
            }
            self.prior_waypoint = Some(current_id);
            self.current_waypoint = Some(next_id);
            return Some(next_id);
        }

        let idx = game_logic_random_value(0, (link_count - 1) as u32) as usize;
        let next_id = self.get_waypoint_link(current_id, idx)?;
        self.prior_waypoint = Some(current_id);
        self.current_waypoint = Some(next_id);
        Some(next_id)
    }

    pub fn new(as_group: bool) -> Self {
        Self::new_with_exact(as_group, false)
    }

    pub fn new_with_exact(as_group: bool, exact: bool) -> Self {
        Self::new_with_exact_and_attack(as_group, exact, false)
    }

    pub fn new_with_exact_and_attack(as_group: bool, exact: bool, attack_follow: bool) -> Self {
        Self {
            move_as_group: as_group,
            is_follow_waypoint_path_state: true,
            attack_follow,
            group_offset: Coord2D::new(0.0, 0.0),
            angle: 0.0,
            frames_sleeping: 0,
            current_waypoint: None,
            prior_waypoint: None,
            append_goal_position: exact,
            exact_path: exact,
        }
    }

    fn compute_goal(&mut self, context: &mut AIStateMachineContext, _use_group_offsets: bool) {
        if self.current_waypoint.is_none() {
            self.current_waypoint = context.goal_waypoint;
        }
        let Some(waypoint_id) = self.current_waypoint else {
            return;
        };
        let Some(mut dest) = self.get_waypoint_location(waypoint_id) else {
            return;
        };

        let mut goal = dest;
        goal.x += self.group_offset.x;
        goal.y += self.group_offset.y;

        if let Ok(terrain) = get_terrain_logic().read() {
            goal.z = terrain.get_ground_height(goal.x, goal.y, None);
        }

        if let Some(terrain) = TheTerrainLogic::get() {
            let extent = terrain.get_maximum_pathfind_extent();
            let dest_in = dest.x >= extent.lo.x
                && dest.x <= extent.hi.x
                && dest.y >= extent.lo.y
                && dest.y <= extent.hi.y;
            let mut goal_in = goal.x >= extent.lo.x
                && goal.x <= extent.hi.x
                && goal.y >= extent.lo.y
                && goal.y <= extent.hi.y;

            if dest_in && !goal_in {
                if goal.x < extent.lo.x + PATHFIND_CELL_SIZE_F {
                    goal.x = extent.lo.x + PATHFIND_CELL_SIZE_F;
                }
                if goal.y < extent.lo.y + PATHFIND_CELL_SIZE_F {
                    goal.y = extent.lo.y + PATHFIND_CELL_SIZE_F;
                }
                if goal.x > extent.hi.x - PATHFIND_CELL_SIZE_F {
                    goal.x = extent.hi.x - PATHFIND_CELL_SIZE_F;
                }
                if goal.y > extent.hi.y - PATHFIND_CELL_SIZE_F {
                    goal.y = extent.hi.y - PATHFIND_CELL_SIZE_F;
                }
                goal_in = goal.x >= extent.lo.x
                    && goal.x <= extent.hi.x
                    && goal.y >= extent.lo.y
                    && goal.y <= extent.hi.y;
            }

            if !goal_in {
                self.append_goal_position = true;
                if let Some(ai) = OBJECT_REGISTRY
                    .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
                    .flatten()
                {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_allow_invalid_position(true);
                    }
                }
            } else {
                self.append_goal_position = false;
            }
        }

        let is_projectile = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.is_kind_of(KindOf::Projectile))
            .unwrap_or(false);
        if !self.has_next_waypoint() && is_projectile {
            if let Some(ai) = OBJECT_REGISTRY
                .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
                .flatten()
            {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                        if let Ok(mut locomotor_guard) = locomotor.lock() {
                            locomotor_guard.set_precise_z_pos(true);
                        }
                    }
                }
            }
        }
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_path_extra_distance(self.calc_extra_path_distance());
            }
        }

        context.goal_position = Some(goal);
    }

    fn has_next_waypoint(&self) -> bool {
        let Some(current_id) = self.current_waypoint else {
            return false;
        };
        let link_count = self.get_waypoint_link_count(current_id);
        if link_count == 0 {
            return false;
        }
        if self.prior_waypoint.is_none() {
            return true;
        }
        if link_count > 1 {
            return true;
        }
        let Some(next_id) = self.get_waypoint_link(current_id, 0) else {
            return false;
        };
        self.prior_waypoint != Some(next_id)
    }
}

impl AIState for AIFollowWaypointPathState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.append_goal_position = false;
        self.prior_waypoint = None;
        self.current_waypoint = context.goal_waypoint;

        if self.current_waypoint.is_none() && !self.move_as_group {
            return StateReturnType::Failed;
        }

        self.frames_sleeping = 0;
        self.group_offset = Coord2D::new(0.0, 0.0);
        self.compute_goal(context, self.move_as_group);
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(goal_pos) = context.goal_position else {
            return StateReturnType::Failed;
        };
        let Some(current_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return StateReturnType::Failed;
        };
        let delta = goal_pos - current_pos;
        let dist_sqr = delta.x * delta.x + delta.y * delta.y;
        let close_enough = PATHFIND_CLOSE_ENOUGH * PATHFIND_CLOSE_ENOUGH;

        if dist_sqr <= close_enough {
            if self.has_next_waypoint() {
                let _ = self.get_next_waypoint();
                self.compute_goal(context, self.move_as_group);
                return StateReturnType::Continue;
            }
            return StateReturnType::Complete;
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
        if self.attack_follow {
            if self.move_as_group {
                return AIStateType::AttackFollowWaypointPathAsTeam;
            }
            return AIStateType::AttackFollowWaypointPathAsIndividuals;
        }
        if self.move_as_group {
            if self.exact_path {
                AIStateType::FollowWaypointPathAsTeamExact
            } else {
                AIStateType::FollowWaypointPathAsTeam
            }
        } else {
            if self.exact_path {
                AIStateType::FollowWaypointPathAsIndividualsExact
            } else {
                AIStateType::FollowWaypointPathAsIndividuals
            }
        }
    }
}

/// AI Wander State
#[derive(Debug)]
pub struct AIWanderState {
    follow: AIFollowWaypointPathState,
    wait_frames: i32,
    timer: i32,
}

impl AIWanderState {
    pub fn new() -> Self {
        Self {
            follow: AIFollowWaypointPathState::new(false),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn update_group_offset(&mut self, ai: &dyn crate::modules::AIUpdateInterface) {
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                let factor = locomotor_guard.template.wander_width_factor;
                if factor > 0.0 {
                    let mut delta = (factor + 0.5).floor() as i32;
                    if delta < 1 {
                        delta = 1;
                    }
                    let x =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    let y =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    self.follow.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl AIState for AIWanderState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        self.follow.current_waypoint = context.goal_waypoint;
        self.follow.prior_waypoint = None;
        self.follow.group_offset = Coord2D::new(0.0, 0.0);

        if self.follow.current_waypoint.is_none() {
            return StateReturnType::Failed;
        }
        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        else {
            return StateReturnType::Failed;
        };

        if let Ok(ai_guard) = ai.lock() {
            self.update_group_offset(&*ai_guard);
        }

        self.timer = 0;
        self.wait_frames = 10 + ((context.owner_id & 0x7) as i32);
        self.follow.compute_goal(context, false);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some((can_be_repulsed, vision_range, ai)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                (
                    owner.is_kind_of(KindOf::CanBeRepulsed),
                    owner.get_vision_range(),
                    owner.get_ai_update_interface(),
                )
            })
        else {
            return StateReturnType::Failed;
        };

        if can_be_repulsed {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let ai_store = the_ai();let enemy_id = ai_store
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(context.owner_id, vision_range)
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failed;
                }
            }
        }

        if goal_reached(context) {
            if self.follow.get_next_waypoint().is_none() {
                return StateReturnType::Complete;
            }

            if let Some(ai) = ai {
                if let Ok(ai_guard) = ai.lock() {
                    self.update_group_offset(&*ai_guard);
                }
            }

            self.follow.compute_goal(context, false);
            return StateReturnType::Continue;
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
        AIStateType::Wander
    }
}

/// AI Wander In Place State
#[derive(Debug)]
pub struct AIWanderInPlaceState {
    origin: Coord3D,
    goal_position: Coord3D,
    wait_frames: i32,
    timer: i32,
}

impl AIWanderInPlaceState {
    pub fn new() -> Self {
        Self {
            origin: Coord3D::new(0.0, 0.0, 0.0),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn choose_new_goal(&mut self, ai: &dyn crate::modules::AIUpdateInterface) {
        let mut delta = 3;
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                delta = ((locomotor_guard.template.wander_about_point_radius
                    / PATHFIND_CELL_SIZE_F)
                    + 0.5)
                    .floor() as i32;
            }
        }

        let offset_x = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        let offset_y = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        self.goal_position = self.origin;
        self.goal_position.x += offset_x;
        self.goal_position.y += offset_y;
    }
}

impl AIState for AIWanderInPlaceState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| {
                self.origin = *owner.get_position();
                owner.get_ai_update_interface()
            })
            .flatten()
        else {
            return StateReturnType::Failed;
        };
        if let Ok(mut ai_guard) = ai.lock() {
            ai_guard.choose_locomotor_set(LocomotorSetType::Wander);
            self.choose_new_goal(&*ai_guard);
        }

        self.timer = 0;
        self.wait_frames = 10 + ((context.owner_id & 0x7) as i32);
        context.goal_position = Some(self.goal_position);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some((can_be_repulsed, vision_range, ai)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                (
                    owner.is_kind_of(KindOf::CanBeRepulsed),
                    owner.get_vision_range(),
                    owner.get_ai_update_interface(),
                )
            })
        else {
            return StateReturnType::Failed;
        };
        let Some(ai) = ai else {
            return StateReturnType::Failed;
        };

        if can_be_repulsed {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let ai_store = the_ai();let enemy_id = ai_store
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(context.owner_id, vision_range)
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failed;
                }
            }
        }

        if goal_reached(context) {
            if let Ok(ai_guard) = ai.lock() {
                self.choose_new_goal(&*ai_guard);
            }
            context.goal_position = Some(self.goal_position);
            return StateReturnType::Continue;
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
                ai_guard.choose_locomotor_set(LocomotorSetType::Normal);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::WanderInPlace
    }
}

/// AI Panic State
#[derive(Debug)]
pub struct AIPanicState {
    follow: AIFollowWaypointPathState,
    wait_frames: i32,
    timer: i32,
}

impl AIPanicState {
    pub fn new() -> Self {
        Self {
            follow: AIFollowWaypointPathState::new(false),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn update_group_offset(&mut self, ai: &dyn crate::modules::AIUpdateInterface) {
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                let factor = locomotor_guard.template.wander_width_factor;
                if factor > 0.0 {
                    let mut delta = (factor + 0.5).floor() as i32;
                    if delta < 1 {
                        delta = 1;
                    }
                    let x =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    let y =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    self.follow.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl AIState for AIPanicState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        self.follow.current_waypoint = context.goal_waypoint;
        self.follow.prior_waypoint = None;
        self.follow.group_offset = Coord2D::new(0.0, 0.0);

        if self.follow.current_waypoint.is_none() {
            return StateReturnType::Failed;
        }
        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        else {
            return StateReturnType::Failed;
        };

        if let Ok(ai_guard) = ai.lock() {
            self.update_group_offset(&*ai_guard);
        }

        self.follow.compute_goal(context, false);
        self.timer = 0;
        self.wait_frames = 10 + ((context.owner_id & 0x7) as i32);

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            if let Ok(mut owner) = owner_arc.write() {
                owner.set_model_condition_state(ModelConditionFlags::PANICKING);
            }
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some((can_be_repulsed, vision_range, ai)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                (
                    owner.is_kind_of(KindOf::CanBeRepulsed),
                    owner.get_vision_range(),
                    owner.get_ai_update_interface(),
                )
            })
        else {
            return StateReturnType::Failed;
        };

        if can_be_repulsed {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let ai_store = the_ai();let enemy_id = ai_store
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(context.owner_id, vision_range)
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failed;
                }
            }
        }

        if goal_reached(context) {
            if self.follow.get_next_waypoint().is_none() {
                return StateReturnType::Complete;
            }

            if let Some(ai) = ai {
                if let Ok(ai_guard) = ai.lock() {
                    self.update_group_offset(&*ai_guard);
                }
            }

            self.follow.compute_goal(context, false);
            return StateReturnType::Continue;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::PANICKING);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Panic
    }
}

