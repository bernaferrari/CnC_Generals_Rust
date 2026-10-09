use super::*;

impl<'a> AIStateMachineDriver<'a> {
    pub(super) fn new(base: &'a mut StateMachine, data: &'a mut AIStateMachineData) -> Self {
        Self { base, data }
    }
}

impl AIStateMachineDriver<'_> {
    pub(crate) fn clear(&mut self) {
        self.clear_impl(None);
    }
    pub(crate) fn clear_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) {
        self.clear_impl(Some(ai));
    }
    fn clear_impl(
        &mut self,
        mut ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) {
        // C++ AIStateMachine::clear() calls StateMachine::clear(), not reset().
        if let Some(ai) = ai.as_deref_mut() {
            self.base.clear_with_ai(ai);
        } else {
            self.base.clear();
        }
        self.data.goal_path.clear();
        self.data.goal_waypoint = None;
        self.data.goal_squad = None;
        self.data.goal_polygon = None;
        self.base.set_goal_squad(None);
        self.base.set_goal_polygon(None);
        if let Some(ai) = ai {
            ai.set_queue_for_path_time(0);
        } else {
            self.notify_state_machine_changed();
        }
    }
    pub(crate) fn reset_to_default_state(&mut self) -> StateReturnType {
        let ret = self.base.reset_to_default_state();
        self.notify_state_machine_changed();
        ret
    }
    pub(crate) fn set_state(&mut self, new_state_id: u32) -> StateReturnType {
        let old_id = self.base.get_current_state_id();
        let ret = self.base.set_current_state(new_state_id);

        if old_id != Some(new_state_id) {
            self.notify_state_machine_changed();
        }

        ret
    }
    pub(crate) fn set_state_with_ai(
        &mut self,
        new_state_id: u32,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        let old_id = self.base.get_current_state_id();
        let ret = self
            .base
            .set_current_state_with_ai_and_owner(new_state_id, ai, &mut ());

        if old_id != Some(new_state_id) {
            // C++ AIStateMachine::setState calls onStateMachineChanged synchronously.
            // The caller already owns this AI, so do not try-lock the installed handle.
            ai.set_queue_for_path_time(0);
        }

        ret
    }
    pub(crate) fn set_goal_object(&mut self, obj_id: ObjectID) {
        self.base.set_goal_object_by_id(Some(obj_id));
    }
    pub(crate) fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.set_goal_position(pos);
    }
    pub(crate) fn set_goal_path(&mut self, path: &[Coord3D]) {
        self.data.goal_path = path.to_vec();
    }
    pub(crate) fn install_follow_exit_path(&mut self, path: &[Coord3D]) {
        let id = AIStateType::FollowExitProductionPath as u32;
        if let Some(state) = self.base.get_state_mut(id) {
            if let Some(follow) =
                crate::ai::states::follow_path::state_follow_path_kind(state.as_mut())
            {
                follow.set_path(path.to_vec(), None);
            }
        }
    }
    pub(crate) fn add_to_goal_path(&mut self, path_point: &Coord3D) {
        if self.data.goal_path.is_empty() {
            self.data.goal_path.push(*path_point);
            return;
        }

        if let Some(final_point) = self.data.goal_path.last() {
            if final_point.x == path_point.x
                && final_point.y == path_point.y
                && final_point.z == path_point.z
            {
                return;
            }
        }

        self.data.goal_path.push(*path_point);
    }
    pub(crate) fn set_goal_waypoint(&mut self, waypoint: Option<Arc<Waypoint>>) {
        self.data.goal_waypoint = waypoint;
        let waypoint_id = self.data.goal_waypoint.as_ref().map(|w| w.id);
        self.base.set_goal_waypoint(waypoint_id);
    }
    pub(crate) fn set_goal_team(&mut self, team: &Arc<RwLock<Team>>) {
        let squad = self
            .data
            .goal_squad
            .get_or_insert_with(|| Arc::new(Squad::new()));
        if let Ok(team_guard) = team.read() {
            Arc::make_mut(squad).squad_from_team(&team_guard, true);
        }
        self.base
            .set_goal_squad(self.data.goal_squad.as_ref().map(Arc::downgrade));
    }
    pub(crate) fn set_goal_squad(&mut self, squad: Option<Arc<Squad>>) {
        match squad {
            Some(source) => {
                let target = self
                    .data
                    .goal_squad
                    .get_or_insert_with(|| Arc::new(Squad::new()));
                if !Arc::ptr_eq(target, &source) {
                    *Arc::make_mut(target) = source.as_ref().clone();
                }
            }
            None => self.data.goal_squad = None,
        }
        self.base
            .set_goal_squad(self.data.goal_squad.as_ref().map(Arc::downgrade));
    }
    pub(crate) fn set_goal_polygon(&mut self, polygon: Option<Arc<PolygonTrigger>>) {
        self.data.goal_polygon = polygon.clone();
        self.base
            .set_goal_polygon(polygon.map(|value| Arc::downgrade(&value)));
    }
    pub(crate) fn set_goal_ai_group(&mut self, group: &AIGroup) {
        let squad = self
            .data
            .goal_squad
            .get_or_insert_with(|| Arc::new(Squad::new()));
        Arc::make_mut(squad).squad_from_ai_group(group, true);
        self.base
            .set_goal_squad(self.data.goal_squad.as_ref().map(Arc::downgrade));
    }
    pub(crate) fn set_temporary_state(
        &mut self,
        new_state_id: u32,
        frame_limit: u32,
    ) -> StateReturnType {
        self.set_temporary_state_impl(new_state_id, frame_limit, None)
    }
    pub(crate) fn enter_temporary_with_ai(
        &mut self,
        new_state_id: u32,
        frame_limit: u32,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        self.set_temporary_state_impl(new_state_id, frame_limit, Some(ai))
    }
    fn set_temporary_state_impl(
        &mut self,
        new_state_id: u32,
        frame_limit: u32,
        mut ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> StateReturnType {
        #[cfg(test)]
        let ordinary = self.base.get_current_state_id();
        // C++ AIStates.cpp:928-939: outgoing identity stays published through
        // Reset exit; incoming identity is published before synchronous enter.
        if let Some(current_id) = self.data.temporary_state_id {
            #[cfg(test)]
            self.data
                .observe("before_reset_exit", current_id, ordinary, ai.as_deref());
            if let Some(state) = self.base.get_state_mut(current_id) {
                if let Some(ai) = ai.as_deref_mut() {
                    state.on_exit_with_ai(StateExitType::Reset, ai);
                } else {
                    state.on_exit(StateExitType::Reset);
                }
            }
            #[cfg(test)]
            self.data
                .observe("after_reset_exit", current_id, ordinary, ai.as_deref());
            self.data.temporary_state_id = None;
            #[cfg(test)]
            self.data
                .observe("reset_cleared", current_id, ordinary, ai.as_deref());
        }
        if self.base.get_state_mut(new_state_id).is_none() {
            return StateReturnType::Failure;
        }
        let goal_id = self.base.get_goal_object_id();
        let goal_pos = self.base.get_goal_position();
        let goal_squad = self.base.get_goal_squad();
        let goal_polygon = self.base.get_goal_polygon();
        let goal_waypoint = self.base.get_goal_waypoint();
        self.data.temporary_state_id = Some(new_state_id);
        let state = self
            .base
            .get_state_mut(new_state_id)
            .expect("state checked above");
        state.bind_goal_object_id(goal_id);
        state.bind_goal_position(goal_pos);
        state.bind_goal_squad(goal_squad);
        state.bind_goal_polygon(goal_polygon);
        state.bind_goal_waypoint(goal_waypoint);
        #[cfg(test)]
        self.data
            .observe("before_enter", new_state_id, ordinary, ai.as_deref());
        let ret = if let Some(ai) = ai.as_deref_mut() {
            state.on_enter_with_ai(ai, goal_id, goal_pos)
        } else {
            state.on_enter()
        };
        #[cfg(test)]
        self.data
            .observe("after_enter", new_state_id, ordinary, ai.as_deref());
        if ret != StateReturnType::Continue {
            #[cfg(test)]
            self.data
                .observe("before_failed_exit", new_state_id, ordinary, ai.as_deref());
            if let Some(ai) = ai.as_deref_mut() {
                state.on_exit_with_ai(StateExitType::Normal, ai);
            } else {
                state.on_exit(StateExitType::Normal);
            }
            #[cfg(test)]
            self.data
                .observe("after_failed_exit", new_state_id, ordinary, ai.as_deref());
            self.data.temporary_state_id = None;
            #[cfg(test)]
            self.data
                .observe("failed_cleared", new_state_id, ordinary, ai.as_deref());
            return ret;
        }
        // C++ AIStates.cpp:944-946 clamps the count, then adds it to the
        // UnsignedInt frame; the deadline wraps rather than saturating.
        self.data.temporary_state_frame_end =
            TheGameLogic::get_frame().wrapping_add(frame_limit.min(60 * LOGICFRAMES_PER_SECOND));
        ret
    }
    pub(crate) fn ai_do_command_with_ai(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<(), crate::ai::AiError> {
        self.ai_do_command_impl(params, Some(ai))
    }
    fn ai_do_command_impl(
        &mut self,
        params: &AiCommandParams,
        mut ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<(), crate::ai::AiError> {
        let is_follow_path_cmd = matches!(
            params.cmd,
            AiCommandType::FollowPath
                | AiCommandType::FollowExitProductionPath
                | AiCommandType::FollowUserPath
                | AiCommandType::FollowPathAppend
        );
        if !is_follow_path_cmd {
            if let Some(obj_id) = params.obj {
                self.base.set_goal_object_by_id(Some(obj_id));
            } else {
                self.base.set_goal_object_by_id(None);
            }
        } else {
            self.base.set_goal_object_by_id(None);
        }

        if params.pos != Coord3D::new(0.0, 0.0, 0.0) {
            self.base.set_goal_position(params.pos);
        }

        if let Some(team_name) = params.team.as_ref() {
            if let Ok(mut factory) = TheTeamFactory().lock() {
                if let Some(team) = factory.find_team(team_name) {
                    self.set_goal_team(&team);
                }
            }
        }

        if let Some(trigger_id) = params.polygon {
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(trigger_id) {
                    let trigger_arc = Arc::new(trigger.clone());
                    self.set_goal_polygon(Some(trigger_arc));
                }
            }
        }

        if let Some(waypoint_id) = params.waypoint {
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(waypoint) = terrain_guard.get_waypoint_by_id(waypoint_id) {
                    let arc = Arc::new(Waypoint::from_terrain(waypoint));
                    self.set_goal_waypoint(Some(arc));
                } else {
                    self.set_goal_waypoint(None);
                }
            }
        }

        if matches!(
            params.cmd,
            AiCommandType::FollowPath
                | AiCommandType::FollowExitProductionPath
                | AiCommandType::FollowUserPath
        ) {
            self.set_goal_path(&params.coords);
            let target_state = if matches!(params.cmd, AiCommandType::FollowExitProductionPath) {
                AIStateType::FollowExitProductionPath
            } else {
                AIStateType::FollowPath
            };
            if let Some(state) = self.base.get_state_mut(target_state as u32) {
                if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                    path_state.set_path(params.coords.clone(), params.obj);
                }
            }
        } else if matches!(params.cmd, AiCommandType::FollowPathAppend) {
            let append_pos = params.pos;
            self.add_to_goal_path(&append_pos);
            if let Some(state_id) = self.base.get_current_state_id() {
                if let Some(state) = self.base.get_state_mut(state_id) {
                    if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                        path_state.append_path(append_pos);
                    }
                }
            } else if let Some(state) = self.base.get_state_mut(AIStateType::FollowPath as u32) {
                if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                    path_state.append_path(append_pos);
                }
            }
        }

        let state = match params.cmd {
            AiCommandType::Idle => AIStateType::Idle,
            AiCommandType::MoveToPosition
            | AiCommandType::MoveToObject
            | AiCommandType::MoveToPositionEvenIfSleeping => AIStateType::MoveTo,
            AiCommandType::FollowWaypointPath => AIStateType::FollowWaypointPathAsIndividuals,
            AiCommandType::FollowWaypointPathAsTeam => AIStateType::FollowWaypointPathAsTeam,
            AiCommandType::FollowWaypointPathExact => {
                AIStateType::FollowWaypointPathAsIndividualsExact
            }
            AiCommandType::FollowWaypointPathAsTeamExact => {
                AIStateType::FollowWaypointPathAsTeamExact
            }
            AiCommandType::FollowPath => AIStateType::FollowPath,
            AiCommandType::FollowExitProductionPath => AIStateType::FollowExitProductionPath,
            AiCommandType::FollowUserPath => AIStateType::FollowPath,
            AiCommandType::FollowPathAppend => AIStateType::FollowPath,
            AiCommandType::MoveToPositionAndEvacuate => AIStateType::MoveAndEvacuate,
            AiCommandType::MoveToPositionAndEvacuateAndExit => AIStateType::MoveAndEvacuateAndExit,
            AiCommandType::AttackObject => AIStateType::AttackObject,
            AiCommandType::ForceAttackObject => AIStateType::ForceAttackObject,
            AiCommandType::AttackPosition => AIStateType::AttackPosition,
            AiCommandType::AttackMoveToPosition => AIStateType::AttackMoveTo,
            AiCommandType::AttackFollowWaypointPath => {
                AIStateType::AttackFollowWaypointPathAsIndividuals
            }
            AiCommandType::AttackFollowWaypointPathAsTeam => {
                AIStateType::AttackFollowWaypointPathAsTeam
            }
            AiCommandType::AttackTeam => AIStateType::AttackSquad,
            AiCommandType::Hunt => AIStateType::Hunt,
            AiCommandType::AttackArea => AIStateType::AttackArea,
            AiCommandType::Repair => AIStateType::Busy,
            AiCommandType::ResumeConstruction => AIStateType::Busy,
            AiCommandType::GetHealed => AIStateType::Enter,
            AiCommandType::GetRepaired => AIStateType::Dock,
            AiCommandType::Enter => AIStateType::Enter,
            AiCommandType::Dock => AIStateType::Dock,
            AiCommandType::Exit => AIStateType::Exit,
            AiCommandType::ExitInstantly => AIStateType::ExitInstantly,
            AiCommandType::Evacuate => AIStateType::Exit,
            AiCommandType::EvacuateInstantly => AIStateType::ExitInstantly,
            AiCommandType::ExecuteRailedTransport => AIStateType::Busy,
            AiCommandType::GoProne => AIStateType::Busy,
            AiCommandType::GuardPosition => AIStateType::Guard,
            AiCommandType::GuardObject => AIStateType::Guard,
            AiCommandType::GuardArea => AIStateType::Guard,
            AiCommandType::GuardTunnelNetwork => AIStateType::GuardTunnelNetwork,
            AiCommandType::GuardRetaliate => AIStateType::GuardRetaliate,
            AiCommandType::HackInternet => AIStateType::HackInternet,
            AiCommandType::FaceObject => AIStateType::FaceObject,
            AiCommandType::FacePosition => AIStateType::FacePosition,
            AiCommandType::RappelInto => AIStateType::RappelInto,
            AiCommandType::CombatDrop => AIStateType::CombatDrop,
            AiCommandType::PickUpPrisoner => AIStateType::PickUpCrate,
            AiCommandType::Wander => AIStateType::Wander,
            AiCommandType::WanderInPlace => AIStateType::WanderInPlace,
            AiCommandType::Panic => AIStateType::Panic,
            AiCommandType::Busy => AIStateType::Busy,
            AiCommandType::MoveAwayFromUnit => AIStateType::MoveOutOfTheWay,
            AiCommandType::TightenToPosition => AIStateType::MoveAndTighten,
            AiCommandType::ReturnPrisoners => AIStateType::Busy,
            AiCommandType::DoSpecialPower => AIStateType::Busy,
            AiCommandType::DoSpecialPowerAtObject => AIStateType::Busy,
            AiCommandType::DoSpecialPowerAtLocation => AIStateType::Busy,
            AiCommandType::Sell => AIStateType::Busy,
            AiCommandType::ToggleOvercharge => AIStateType::Busy,
            AiCommandType::Surrender => AIStateType::Busy,
            AiCommandType::Cheer => AIStateType::Busy,
            _ => AIStateType::Idle,
        };

        if matches!(
            params.cmd,
            AiCommandType::GuardPosition
                | AiCommandType::GuardObject
                | AiCommandType::GuardArea
                | AiCommandType::GuardTunnelNetwork
        ) {
            self.base.set_guard_mode_raw(params.int_value);
        }

        if let Some(ai) = ai.as_deref_mut() {
            self.set_state_with_ai(state as u32, ai);
        } else {
            self.set_state(state as u32);
        }
        Ok(())
    }
    pub(crate) fn get_current_state_id(&self) -> Option<u32> {
        self.base.get_current_state_id()
    }
    pub(crate) fn get_current_state_name(&self) -> String {
        let mut name = self.base.get_current_state_name();

        if let Some(temp_state_id) = self.data.temporary_state_id {
            if let Some(temp_name) = self.base.get_state_name_by_id(temp_state_id) {
                name.push_str(" /T/");
                name.push_str(temp_name);
            }
        }

        name
    }
    pub(crate) fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 257: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.base.get_goal_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }
    pub(crate) fn get_goal_object_id(&self) -> crate::common::ObjectID {
        self.base.get_goal_object_id()
    }
    pub(crate) fn get_goal_path_size(&self) -> usize {
        self.data.goal_path.len()
    }
    pub(crate) fn get_goal_position(&self) -> Option<Coord3D> {
        Some(self.base.get_goal_position())
    }
    pub(crate) fn get_temporary_state(&self) -> Option<u32> {
        self.data.temporary_state_id
    }
    pub(crate) fn is_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }
    pub(crate) fn is_busy(&self) -> bool {
        self.base.is_in_busy_state()
    }
    pub(crate) fn is_idle(&self) -> bool {
        self.base.is_in_idle_state()
    }
    pub(crate) fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }
    pub(crate) fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }
    pub(crate) fn is_locked(&self) -> bool {
        self.base.is_locked()
    }
    pub(crate) fn lock(&mut self) {
        self.base.lock();
    }
    pub(crate) fn unlock(&mut self) {
        self.base.unlock();
    }
    pub(crate) fn get_goal_path_position(&self, i: usize) -> Option<&Coord3D> {
        self.data.goal_path.get(i)
    }
    pub(crate) fn get_goal_squad(&self) -> Option<&Arc<Squad>> {
        self.data.goal_squad.as_ref()
    }
    pub(crate) fn get_goal_waypoint(&self) -> Option<&Arc<Waypoint>> {
        self.data.goal_waypoint.as_ref()
    }
    fn notify_state_machine_changed(&self) {
        notify_state_machine_changed_for_base(self.base)
    }
    pub(crate) fn ai_do_command(
        &mut self,
        params: &AiCommandParams,
    ) -> Result<(), crate::ai::AiError> {
        self.ai_do_command_impl(params, None)
    }
}
