//! Owned Dozer primary/action state data, with explicit original saved IDs.
//! CPP DozerAIUpdate.cpp51–58/936–940; no shared state graph or ambient owner.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrimaryState {
    Idle,
    Build,
    Repair,
    Fortify,
    GoingHome,
}
impl PrimaryState {
    pub(super) fn id(self) -> u32 {
        match self {
            Self::Idle => 0,
            Self::Build => 1,
            Self::Repair => 2,
            Self::Fortify => 3,
            Self::GoingHome => 4,
        }
    }
    pub(super) fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::Idle),
            1 => Ok(Self::Build),
            2 => Ok(Self::Repair),
            3 => Ok(Self::Fortify),
            4 => Ok(Self::GoingHome),
            _ => Err("invalid Dozer primary state".into()),
        }
    }
}
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ActionState {
    PickPosition { failed_attempts: Int },
    MoveToPosition,
    DoAction { enter_frame: u32 },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MachineControl {
    pub sleep_until: u32,
    pub default_id: u32,
    pub goal_object: ObjectID,
    pub goal_position: Coord3D,
    pub locked: bool,
    pub initialized: bool,
}
impl MachineControl {
    fn new(initialized: bool) -> Self {
        Self {
            sleep_until: 0,
            default_id: 0,
            goal_object: INVALID_ID,
            goal_position: Coord3D::ZERO,
            locked: false,
            initialized,
        }
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionMachine {
    pub task: DozerTask,
    pub state: ActionState,
    pub control: MachineControl,
}
impl ActionMachine {
    fn new(task: DozerTask) -> Self {
        Self {
            task,
            state: ActionState::PickPosition { failed_attempts: 0 },
            control: MachineControl::new(true),
        }
    }
    pub(super) fn reset(&mut self) {
        self.state = ActionState::PickPosition { failed_attempts: 0 };
        self.control.sleep_until = 0;
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrimaryMachine {
    pub state: PrimaryState,
    pub control: MachineControl,
    pub idle: DozerPrimaryIdle,
    #[serde(skip, default = "fresh_actions")]
    pub actions: [ActionMachine; DOZER_NUM_TASKS],
}
impl PrimaryMachine {
    pub fn new() -> Self {
        Self {
            state: PrimaryState::Idle,
            control: MachineControl::new(false),
            idle: DozerPrimaryIdle {
                idle_since: 0,
                player: 0,
                marked: false,
            },
            actions: fresh_actions(),
        }
    }
}
fn fresh_actions() -> [ActionMachine; DOZER_NUM_TASKS] {
    [
        ActionMachine::new(DozerTask::Build),
        ActionMachine::new(DozerTask::Repair),
        ActionMachine::new(DozerTask::Fortify),
    ]
}
impl Default for PrimaryMachine {
    fn default() -> Self {
        Self::new()
    }
}
/// Immediate self-AI effects. The caller releases only the bounded derived
/// borrow, executes the original commands, then resumes the state transition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum DozerStep {
    Continue,
    CancelTask {
        task: DozerTask,
    },
    MoveToPosition {
        task: DozerTask,
        target: ObjectID,
        position: Coord3D,
        ignore_obstacle_count: u8,
        action_dock: bool,
    },
    BuildComplete,
}

impl DozerAIUpdate {
    pub(crate) fn prepare_primary_step_with_world(
        &mut self,
        regular_idle: bool,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<DozerStep, String> {
        use super::action_kernel::{ActionFacts, ActionStep};
        if self.task_owner.primary.state == PrimaryState::Idle {
            if self.owned_build_is_admitted() && self.task_owner.current == DozerTask::Build {
                // set_build_task records the request and the original dock
                // before the primary Dozer state machine enters BUILD.
                self.task_owner.enter_primary_task(DozerTask::Build);
            } else {
                self.update_primary_with_world(regular_idle, world)?;
                return Ok(DozerStep::Continue);
            }
        }
        if self.task_owner.primary.state == PrimaryState::Build {
            return self.prepare_build_step_with_world(regular_idle, world);
        }
        if self.task_owner.primary.state != PrimaryState::Fortify {
            return Err("supplied Dozer Repair action services unavailable".into());
        }
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        let action = &self.task_owner.primary.actions[DozerTask::Fortify.as_index()];
        let state = action.state;
        let target = if matches!(state, ActionState::PickPosition { .. }) {
            self.get_task_target(DozerTask::Fortify)
        } else {
            action.control.goal_object
        };
        let mut facts = ActionFacts {
            frame: world.frame(),
            regular_idle,
            target,
            target_exists: false,
            claim_valid: true,
            unmanned: false,
            picked_position: Coord3D::ZERO,
            ignore_obstacle_count: 1,
            distance_squared: 0.0,
            actor_radius: 0.0,
        };
        match state {
            ActionState::PickPosition { .. } => {
                if let Some(goal) = world.object(target) {
                    world.validate_dozer_action_effects(&owner, &goal)?;
                    facts.target_exists = true;
                    let (position, count) =
                        match self.get_dock_point(DozerTask::Fortify, DozerDockPoint::Start) {
                            Some(point) => (point, 1),
                            None => (world.dozer_action_position(&owner, &goal)?, 2),
                        };
                    facts.picked_position = position;
                    facts.ignore_obstacle_count = count;
                }
            }
            ActionState::MoveToPosition => {
                if let Some(goal) = world.object(target) {
                    world.validate_dozer_action_effects(&owner, &goal)?;
                    facts.target_exists = true;
                    facts.actor_radius = owner
                        .read()
                        .map_err(|_| "Dozer owner poisoned")?
                        .get_geometry_info()
                        .get_bounding_sphere_radius();
                    let position = self.task_owner.primary.actions[DozerTask::Fortify.as_index()]
                        .control
                        .goal_position;
                    facts.distance_squared =
                        world.dozer_action_distance_squared(&owner, &position)?;
                }
            }
            ActionState::DoAction { .. } => {
                facts.unmanned = owner
                    .read()
                    .map_err(|_| "Dozer owner poisoned")?
                    .is_disabled_by_type(
                        game_engine::common::thing::update_module::DisabledType::Unmanned,
                    );
                if let Some(goal) = world.object(target) {
                    facts.target_exists = true;
                    if !facts.unmanned {
                        world.validate_dozer_action_effects(&owner, &goal)?;
                    }
                }
            }
        }
        match self.task_owner.prepare_action_phase(facts) {
            ActionStep::Move {
                target,
                position,
                ignore_obstacle_count,
                ..
            } => Ok(DozerStep::MoveToPosition {
                task: DozerTask::Fortify,
                target,
                position,
                ignore_obstacle_count,
                action_dock: false,
            }),
            ActionStep::CancelTask { task } => {
                self.cancel_task_bookkeeping_owned(task)?;
                Ok(DozerStep::CancelTask { task })
            }
            ActionStep::EnterAction {
                task,
                target,
                frame,
            } => {
                self.task_owner.finish_enter_action(task, target, frame);
                Ok(DozerStep::Continue)
            }
            ActionStep::ResetPrimary => {
                self.reset_primary_idle_owned(world)?;
                Ok(DozerStep::Continue)
            }
            ActionStep::RunAction { .. } => {
                // CPP725-732: Fortify intentionally has no action effect.
                world.set_weapon_set_flag(
                    &owner,
                    crate::weapon::WeaponSetType::MineClearingDetail,
                    false,
                )?;
                Ok(DozerStep::Continue)
            }
            ActionStep::Continue => Ok(DozerStep::Continue),
            ActionStep::CompleteRequest { .. } => {
                Err("Fortify action has an invalid Build/Repair claim check".into())
            }
        }
    }

    pub(crate) fn finish_primary_step_with_world(
        &mut self,
        step: &DozerStep,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        match step {
            DozerStep::MoveToPosition {
                task,
                target,
                position,
                action_dock,
                ..
            } => {
                if *action_dock {
                    if self.task_owner.current == DozerTask::Build
                        && self.task_owner.build_sub_task
                            == DozerBuildSubTask::SelectBuildDockLocation
                    {
                        self.task_owner.build_sub_task = DozerBuildSubTask::MovingToBuildDockLocation;
                    }
                } else {
                    self.task_owner.finish_action_move(*task, *target, *position);
                }
                Ok(())
            }
            DozerStep::BuildComplete => Ok(()),
            DozerStep::CancelTask { .. } => self.reset_primary_idle_owned(world),
            DozerStep::Continue => Ok(()),
        }
    }

    fn prepare_build_step_with_world(
        &mut self,
        regular_idle: bool,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<DozerStep, String> {
        use super::action_kernel::{ActionFacts, ActionStep};
        if self.task_owner.current != DozerTask::Build {
            return Err("owned Dozer Build primary has no current Build task".into());
        }
        if self.pending_owned_completion.is_some() {
            return Ok(DozerStep::BuildComplete);
        }
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        let action = &self.task_owner.primary.actions[DozerTask::Build.as_index()];
        let state = action.state;
        let target_id = if matches!(state, ActionState::PickPosition { .. }) {
            self.get_task_target(DozerTask::Build)
        } else {
            action.control.goal_object
        };
        let mut facts = ActionFacts {
            frame: world.frame(),
            regular_idle,
            target: target_id,
            target_exists: false,
            claim_valid: false,
            unmanned: false,
            picked_position: Coord3D::ZERO,
            ignore_obstacle_count: 1,
            distance_squared: 0.0,
            actor_radius: 0.0,
        };
        if let Some(target) = world.object(target_id) {
            world.validate_dozer_action_effects(&owner, &target)?;
            let target_guard = target.read().map_err(|_| "Build target poisoned")?;
            facts.target_exists = true;
            facts.claim_valid = target_guard.get_builder_id() == self.object_id;
            drop(target_guard);
            match state {
                ActionState::PickPosition { .. } => {
                    let (position, ignore_count) =
                        if let Some(position) = self.get_dock_point(DozerTask::Build, DozerDockPoint::Start) {
                            (position, 1)
                        } else {
                            (world.dozer_action_position(&owner, &target)?, 2)
                        };
                    facts.picked_position = position;
                    facts.ignore_obstacle_count = ignore_count;
                }
                ActionState::MoveToPosition => {
                    facts.actor_radius = owner
                        .read()
                        .map_err(|_| "Dozer owner poisoned")?
                        .get_geometry_info()
                        .get_bounding_sphere_radius();
                    facts.distance_squared = world.dozer_action_distance_squared(
                        &owner,
                        &action.control.goal_position,
                    )?;
                }
                ActionState::DoAction { .. } => {
                    facts.unmanned = owner
                        .read()
                        .map_err(|_| "Dozer owner poisoned")?
                        .is_disabled_by_type(
                            game_engine::common::thing::update_module::DisabledType::Unmanned,
                        );
                }
            }
        }
        match self.task_owner.prepare_action_phase(facts) {
            ActionStep::Move { target, position, ignore_obstacle_count, .. } => {
                Ok(DozerStep::MoveToPosition {
                    task: DozerTask::Build,
                    target,
                    position,
                    ignore_obstacle_count,
                    action_dock: false,
                })
            }
            ActionStep::EnterAction { task, target, frame } => {
                let target = world.object(target).ok_or("Build scaffold disappeared")?;
                let mut scaffold = target.write().map_err(|_| "Build scaffold borrowed")?;
                let _ = scaffold.clear_and_set_model_condition_flags(
                    crate::common::ModelConditionFlags::AWAITING_CONSTRUCTION,
                    crate::common::ModelConditionFlags::PARTIALLY_CONSTRUCTED
                        | crate::common::ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
                );
                drop(scaffold);
                self.task_owner.finish_enter_action(task, target_id, frame);
                Ok(DozerStep::Continue)
            }
            ActionStep::RunAction { task: DozerTask::Build, enter_frame } => {
                let owner_player = owner
                    .read()
                    .map_err(|_| "Build owner poisoned")?
                    .controlling_player_id_from_pin()?;
                let target_player = world
                    .object(target_id)
                    .ok_or("Build scaffold disappeared")?
                    .read()
                    .map_err(|_| "Build scaffold poisoned")?
                    .controlling_player_id_from_pin()?;
                // DozerAIUpdate.cpp:436 fails the installed Build action if
                // either admitted object changed controlling Player.
                if owner_player != target_player {
                    self.finish_building_sound_with_world(world);
                    self.reset_primary_idle_owned(world)?;
                    return Ok(DozerStep::Continue);
                }
                if world.frame() == enter_frame {
                    return Ok(DozerStep::Continue);
                }
                if self.task_owner.build_sub_task == DozerBuildSubTask::SelectBuildDockLocation {
                    if let Some(position) = self.get_dock_point(DozerTask::Build, DozerDockPoint::Action) {
                        return Ok(DozerStep::MoveToPosition {
                            task: DozerTask::Build,
                            target: target_id,
                            position,
                            ignore_obstacle_count: 1,
                            action_dock: true,
                        });
                    }
                    self.task_owner.build_sub_task = DozerBuildSubTask::MovingToBuildDockLocation;
                }
                if self.task_owner.build_sub_task == DozerBuildSubTask::MovingToBuildDockLocation {
                    if !regular_idle {
                        return Ok(DozerStep::Continue);
                    }
                    self.task_owner.build_sub_task = DozerBuildSubTask::DoBuildAtDock;
                    if self.building_sound_target != target_id {
                        let sound = world
                            .object(target_id)
                            .ok_or("Build scaffold disappeared")?
                            .read()
                            .map_err(|_| "Build scaffold poisoned")?
                            .get_template()
                            .get_per_unit_sound("UnderConstruction");
                        if let Some(sound) = sound {
                            self.start_building_sound_with_world(&sound, target_id, world);
                        }
                    }
                }
                if self.task_owner.build_sub_task != DozerBuildSubTask::DoBuildAtDock {
                    return Ok(DozerStep::Continue);
                }
                let target = world.object(target_id).ok_or("Build scaffold disappeared")?;
                let frames = Self::current_build_frames(&owner, &target);
                owner
                    .write()
                    .map_err(|_| "Dozer owner borrowed")?
                    .set_model_condition_state(crate::common::MODELCONDITION_ACTIVELY_CONSTRUCTING);
                let advance = {
                    let mut scaffold = target.write().map_err(|_| "Build scaffold borrowed")?;
                    Self::advance_live_scaffold(&mut scaffold, self.object_id, frames)
                };
                match advance {
                    ConstructionAdvance::Unavailable => {
                        return Err("owned Build lost its live scaffold claim".into());
                    }
                    ConstructionAdvance::InProgress => {
                        if let Some(task) = self.dozer_task.as_mut() {
                            task.started_construction = true;
                        }
                    }
                    ConstructionAdvance::Complete => {
                        let builder = world.object(self.object_id).ok_or("Dozer owner absent")?;
                        let end_dock = self.get_dock_point(DozerTask::Build, DozerDockPoint::End);
                        self.pending_owned_completion = Some(
                            crate::modules::OwnedDozerBuildCompletion {
                                builder_id: self.object_id,
                                scaffold_id: target_id,
                                builder: std::sync::Arc::downgrade(&builder),
                                scaffold: std::sync::Arc::downgrade(&target),
                                frame: world.frame(),
                                is_rebuild: self.task_owner.construct_is_rebuild,
                                end_dock,
                            },
                        );
                        return Ok(DozerStep::BuildComplete);
                    }
                }
                Ok(DozerStep::Continue)
            }
            ActionStep::ResetPrimary => {
                self.reset_primary_idle_owned(world)?;
                Ok(DozerStep::Continue)
            }
            ActionStep::CancelTask { task } => {
                self.cancel_task_bookkeeping_owned(task)?;
                Ok(DozerStep::CancelTask { task })
            }
            ActionStep::CompleteRequest { task, target } => {
                self.task_owner.finish_claim_failure(task, target);
                Err("owned Dozer Build claim changed before Action entry".into())
            }
            ActionStep::Continue => Ok(DozerStep::Continue),
            ActionStep::RunAction { .. } => Err("owned Build action task mismatch".into()),
        }
    }
}
impl DozerTaskOwner<DozerTaskEntry, DozerDockPointInfo> {
    pub(super) fn build_is_admitted(&self) -> bool {
        self.primary.control.initialized
            && !self.primary.control.locked
            && self.requests[DozerTask::Build.as_index()].target_id != INVALID_ID
            && self.requests[DozerTask::Repair.as_index()].target_id == INVALID_ID
            && self.requests[DozerTask::Fortify.as_index()].target_id == INVALID_ID
            && matches!(
                (self.primary.state, self.current),
                (PrimaryState::Idle, DozerTask::Invalid | DozerTask::Build)
                    | (PrimaryState::Build, DozerTask::Build)
            )
    }
}
impl DozerAIUpdate {
    pub(crate) fn owned_build_is_admitted(&self) -> bool {
        self.task_owner.build_is_admitted()
    }
    pub(crate) fn owned_primary_is_admitted(&self) -> bool {
        self.owned_fortify_is_admitted() || self.owned_build_is_admitted()
            || (self.task_owner.primary.state == PrimaryState::Idle
                && self.task_owner.current == DozerTask::Invalid
                && !self.task_owner.primary.control.locked
                && !self.is_any_task_pending()
                && self.dozer_task.is_none())
    }
    pub(crate) fn pending_owned_completion(
        &self,
    ) -> Option<crate::modules::OwnedDozerBuildCompletion> {
        self.pending_owned_completion.clone()
    }
    pub(crate) fn validate_owned_completion(
        &self,
        receipt: &crate::modules::OwnedDozerBuildCompletion,
    ) -> Result<(), String> {
        let pending = self.pending_owned_completion.as_ref().ok_or("Dozer completion receipt is absent")?;
        if !std::sync::Weak::ptr_eq(&pending.builder, &receipt.builder)
            || !std::sync::Weak::ptr_eq(&pending.scaffold, &receipt.scaffold)
            || pending.frame != receipt.frame
            || pending.builder_id != receipt.builder_id
            || pending.scaffold_id != receipt.scaffold_id
            || pending.is_rebuild != receipt.is_rebuild
            || pending.end_dock != receipt.end_dock
            || self.task_owner.current != DozerTask::Build
            || self.get_task_target(DozerTask::Build) != receipt.scaffold_id
        {
            return Err("Dozer completion receipt no longer matches its installed task".into());
        }
        Ok(())
    }
    pub(crate) fn finish_owned_completion_bookkeeping(
        &mut self,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        self.pending_owned_completion = None;
        self.task_owner.requests[DozerTask::Build.as_index()] = DozerTaskEntry::default();
        for dock in &mut self.task_owner.docks[DozerTask::Build.as_index()] {
            dock.invalidate();
        }
        self.task_owner.primary.actions[DozerTask::Build.as_index()].control.goal_object = INVALID_ID;
        self.task_owner.build_sub_task = DozerBuildSubTask::SelectBuildDockLocation;
        self.reset_primary_idle_owned(world)
    }
    pub(crate) fn owned_fortify_is_admitted(&self) -> bool {
        self.task_owner.primary.control.initialized
            && !self.task_owner.primary.control.locked
            && !self.is_task_pending(DozerTask::Build)
            && !self.is_task_pending(DozerTask::Repair)
            && matches!(
                self.task_owner.primary.state,
                PrimaryState::Idle | PrimaryState::Fortify
            )
            && (self.task_owner.primary.state == PrimaryState::Fortify
                || self.is_task_pending(DozerTask::Fortify))
            && self.dozer_task.is_none()
    }
    pub(crate) fn validate_owned_action_move(
        &self,
        task: DozerTask,
        target: ObjectID,
        position: Coord3D,
        ignore_obstacle_count: u8,
        action_dock: bool,
    ) -> Result<(), String> {
        if task == DozerTask::Fortify {
            if !self.owned_fortify_is_admitted()
                || self.task_owner.primary.state != PrimaryState::Fortify
                || self.task_owner.current != task
            {
                return Err("Dozer movement lacks its owned Fortify action".into());
            }
            let action = &self.task_owner.primary.actions[task.as_index()];
            let expected_count = if self.get_dock_point(task, DozerDockPoint::Start).is_some() { 1 } else { 2 };
            if action_dock
                || !matches!(action.state, ActionState::PickPosition { .. })
                || target != self.get_task_target(task)
                || target != action.control.goal_object
                || position != action.control.goal_position
                || ignore_obstacle_count != expected_count
            {
                return Err("Dozer movement differs from its pending action continuation".into());
            }
            return Ok(());
        }
        if task != DozerTask::Build || !self.owned_build_is_admitted() {
            return Err("Dozer movement lacks its owned Build action".into());
        }
        let action = &self.task_owner.primary.actions[task.as_index()];
        if action_dock {
            if !matches!(action.state, ActionState::DoAction { .. })
                || self.task_owner.build_sub_task != DozerBuildSubTask::SelectBuildDockLocation
                || target != action.control.goal_object
                || self.get_dock_point(task, DozerDockPoint::Action) != Some(position)
                || ignore_obstacle_count != 1
            {
                return Err("Dozer action-dock move differs from its Build continuation".into());
            }
        } else if !matches!(action.state, ActionState::PickPosition { .. })
            || target != self.get_task_target(task)
            || target != action.control.goal_object
            || position != action.control.goal_position
            || ignore_obstacle_count != if self.get_dock_point(task, DozerDockPoint::Start).is_some() { 1 } else { 2 }
        {
            return Err("Dozer movement differs from its pending Build continuation".into());
        }
        Ok(())
    }
}
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DozerOwnedObservation {
    pub primary: u32,
    pub action: u32,
    pub goal: ObjectID,
    pub position: Coord3D,
    pub current_task: DozerTask,
    pub idle_marked: bool,
    pub enter_frame: Option<u32>,
}
#[cfg(test)]
impl DozerAIUpdate {
    pub(crate) fn owned_state_observation_for_test(&self) -> DozerOwnedObservation {
        let observed_task = match self.task_owner.current {
            DozerTask::Build | DozerTask::Repair | DozerTask::Fortify => self.task_owner.current,
            DozerTask::Invalid => DozerTask::Fortify,
        };
        let action = &self.task_owner.primary.actions[observed_task.as_index()];
        let (state, enter_frame) = match action.state {
            ActionState::PickPosition { .. } => (0, None),
            ActionState::MoveToPosition => (1, None),
            ActionState::DoAction { enter_frame } => (2, Some(enter_frame)),
        };
        DozerOwnedObservation {
            primary: self.task_owner.primary.state.id(),
            action: state,
            goal: action.control.goal_object,
            position: action.control.goal_position,
            current_task: self.task_owner.current,
            idle_marked: self.task_owner.primary.idle.marked,
            enter_frame,
        }
    }
}
impl DozerAIUpdate {
    pub(crate) fn validate_owned_action_idle(&self, task: DozerTask) -> Result<(), String> {
        if task != DozerTask::Fortify
            || self.task_owner.primary.state != PrimaryState::Fortify
            || self.task_owner.current != task
            || self.is_task_pending(task)
        {
            return Err("Dozer Idle lacks its cancelled Fortify continuation".into());
        }
        let action = &self.task_owner.primary.actions[task.as_index()];
        if !matches!(action.state, ActionState::PickPosition { .. })
            || action.control.goal_object != INVALID_ID
        {
            return Err("Dozer Idle differs from its missing-target action continuation".into());
        }
        Ok(())
    }
}
