use crate::object::update::ai_update::dozer_ai_update::{DozerAIUpdate, DozerBuildSubTask, DozerDockPoint};

/// Worker AI update module data (matches C++ WorkerAIUpdateModuleData fields).
#[derive(Debug, Clone)]
pub struct WorkerAIUpdateData {
    /// Maximum number of boxes this worker can carry
    pub max_boxes: i32,
    /// Warehouse scan distance
    pub warehouse_scan_distance: Real,
    /// Delay time at warehouse (in frames)
    pub warehouse_delay: u32,
    /// Delay time at center (in frames)
    pub center_delay: u32,
    /// Supplies depleted voice event name
    pub supplies_depleted_voice: String,
    /// Repair health percent per second
    pub repair_health_percent_per_second: Real,
    /// Bored time (seconds)
    pub bored_time: Real,
    /// Bored range
    pub bored_range: Real,
    /// Supply boost when upgraded (worker shoes)
    pub upgraded_supply_boost: u32,
}

impl Default for WorkerAIUpdateData {
    fn default() -> Self {
        Self {
            max_boxes: 0,
            warehouse_scan_distance: 100.0,
            warehouse_delay: 0,
            center_delay: 0,
            supplies_depleted_voice: String::new(),
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
            upgraded_supply_boost: 0,
        }
    }
}
#[derive(Debug)]
enum WorkerData {
    Standalone(WorkerAIUpdateData),
    Shared(game_engine::common::thing::module::ModuleDataHandle),
}

struct WorkerDataView<'a> {
    max_boxes: i32,
    warehouse_scan_distance: Real,
    warehouse_delay: u32,
    center_delay: u32,
    supplies_depleted_voice: &'a str,
    repair_health_percent_per_second: Real,
    bored_time: Real,
    bored_range: Real,
}


/// GLA worker AI (similar to supply truck but for GLA faction)
/// GLA workers gather supplies on foot from supply piles
pub struct WorkerAIUpdate {
    /// Worker configuration (similar to SupplyTruckAIUpdate)
    data: WorkerData,
    /// Current AI state
    state: SupplyTruckState,
    /// Current number of boxes carried
    number_boxes: i32,
    /// Preferred dock ID (set by player command)
    preferred_dock: Option<ObjectID>,
    /// Whether to force wanting state
    force_wanting_state: bool,
    /// Whether to force busy state
    force_busy_state: bool,
    /// Supply truck state machine (workers reuse supply truck state logic)
    state_machine: Option<SupplyTruckStateMachine>,
    /// Active dozer-style task (repair/resume/build)
    dozer_task: Option<WorkerDozerTask>,
    /// Current dozer action state
    dozer_action_state: WorkerDozerActionState,
    /// Dozer task entries (build/repair/fortify)
    dozer_tasks: [WorkerTaskEntry; WORKER_DOZER_TASK_COUNT],
    /// Dock points per task (start/action/end)
    dozer_dock_points: [[WorkerDockPoint; WORKER_DOCK_POINT_COUNT]; WORKER_DOZER_TASK_COUNT],
    /// Current task slot
    current_task: Option<WorkerDozerTaskSlot>,
    /// Object ID of this worker
    object_id: ObjectID,
    /// Player index
    player_index: PlayerIndex,
    /// Audio system reference
    audio_system: Option<Arc<dyn AudioSystem>>,
    /// Upgrade system reference
    upgrade_system: Option<Arc<dyn UpgradeSystem>>,
    /// C++ getObject(): the exact owning Object, bound at construction by the
    /// factory. Standalone adapters leave it unbound.
    owner: Option<std::sync::Weak<RwLock<Object>>>,
}

/// Dozer-style task types for workers (matches C++ WorkerAIUpdate task handling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerTaskType {
    Repair,
    Build,
    Fortify,
}

/// Current dozer action phase (matches DOZER_ACTION_* flow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerActionState {
    PickActionPos,
    MoveToActionPos,
    DoAction,
}

#[derive(Debug, Clone)]
struct WorkerDozerTask {
    task_type: WorkerDozerTaskType,
    target_id: ObjectID,
    dock_point: Option<Coord3D>,
    failed_attempts: u32,
    build_total_frames: u32,
    build_max_health: f32,
    is_rebuild: bool,
    started_construction: bool,
    build_sub_task: DozerBuildSubTask,
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub(crate) struct WorkerBuildOrderObservation {
    pub request_target: ObjectID,
    pub action_target: ObjectID,
    pub order_frame: u32,
    pub docks: [Option<Coord3D>; 3],
    pub moving_to_dock: bool,
    pub build_sub_task: DozerBuildSubTask,
}

#[derive(Debug, Clone, Copy)]
struct WorkerDockPoint {
    valid: bool,
    location: Coord3D,
}

impl Default for WorkerDockPoint {
    fn default() -> Self {
        Self {
            valid: false,
            location: Coord3D::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerTaskSlot {
    Build = 0,
    Repair = 1,
    Fortify = 2,
}

impl WorkerDozerTaskSlot {
    fn as_index(self) -> usize {
        self as usize
    }
}

const WORKER_DOZER_TASK_COUNT: usize = 3;
const WORKER_DOCK_POINT_COUNT: usize = 3;

#[derive(Debug, Clone)]
struct WorkerTaskEntry {
    target_id: ObjectID,
    order_frame: u32,
}

impl Default for WorkerTaskEntry {
    fn default() -> Self {
        Self {
            target_id: INVALID_ID,
            order_frame: 0,
        }
    }
}

impl std::fmt::Debug for WorkerAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerAIUpdate")
            .field("data", &self.data)
            .field("state", &self.state)
            .field("number_boxes", &self.number_boxes)
            .field("preferred_dock", &self.preferred_dock)
            .field("force_wanting_state", &self.force_wanting_state)
            .field("force_busy_state", &self.force_busy_state)
            .field(
                "state_machine",
                &self
                    .state_machine
                    .as_ref()
                    .map(|_| "SupplyTruckStateMachine"),
            )
            .field("dozer_task", &self.dozer_task)
            .field("dozer_action_state", &self.dozer_action_state)
            .field("current_task", &self.current_task)
            .field("object_id", &self.object_id)
            .field("player_index", &self.player_index)
            .field("audio_system", &self.audio_system.is_some())
            .field("upgrade_system", &self.upgrade_system.is_some())
            .finish()
    }
}

impl WorkerAIUpdate {
    pub fn new(data: WorkerAIUpdateData, object_id: ObjectID, player_index: PlayerIndex) -> Self {
        Self {
            data: WorkerData::Standalone(data),
            state: SupplyTruckState::Idle,
            number_boxes: 0,
            preferred_dock: None,
            force_wanting_state: false,
            force_busy_state: false,
            state_machine: None,
            dozer_task: None,
            dozer_action_state: WorkerDozerActionState::PickActionPos,
            dozer_tasks: [
                WorkerTaskEntry::default(),
                WorkerTaskEntry::default(),
                WorkerTaskEntry::default(),
            ],
            dozer_dock_points: [
                [
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                ],
                [
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                ],
                [
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                ],
            ],
            current_task: None,
            object_id,
            player_index,
            audio_system: None,
            upgrade_system: None,
            owner: None,
        }
    }

    /// Keep the admitted definition shared with the owning WorkerAI module.
    pub fn new_with_module_data(
        data: game_engine::common::thing::module::ModuleDataHandle,
        object_id: ObjectID,
        player_index: PlayerIndex,
    ) -> Self {
        let mut worker = Self::new(WorkerAIUpdateData::default(), object_id, player_index);
        worker.data = WorkerData::Shared(data);
        worker
    }

    fn with_data<R>(&self, f: impl FnOnce(WorkerDataView<'_>) -> R) -> R {
        match &self.data {
            WorkerData::Standalone(data) => f(WorkerDataView {
                max_boxes: data.max_boxes,
                warehouse_scan_distance: data.warehouse_scan_distance,
                warehouse_delay: data.warehouse_delay,
                center_delay: data.center_delay,
                supplies_depleted_voice: &data.supplies_depleted_voice,
                repair_health_percent_per_second: data.repair_health_percent_per_second,
                bored_time: data.bored_time,
                bored_range: data.bored_range,
            }),
            WorkerData::Shared(handle) => {
                let data = handle.read();
                let data = data.as_any().downcast_ref::<crate::object::update::ai_update::worker_ai_update::WorkerAIUpdateModuleData>()
                    .expect("worker shared data must be WorkerAIUpdateModuleData");
                f(WorkerDataView {
                    max_boxes: data.max_boxes_data,
                    warehouse_scan_distance: data.warehouse_scan_distance,
                    warehouse_delay: data.warehouse_delay,
                    center_delay: data.center_delay,
                    supplies_depleted_voice: data.supplies_depleted_voice.as_str(),
                    repair_health_percent_per_second: data.repair_health_percent_per_second,
                    bored_time: data.bored_time,
                    bored_range: data.bored_range,
                })
            }
        }
    }

    /// Bind the exact owning Object (C++ `getObject()`), so Drawable
    /// notifications never resolve a same-ID foreign Object by registry id.
    pub fn bind_owner(&mut self, owner: &Arc<RwLock<Object>>) {
        self.owner = Some(Arc::downgrade(owner));
    }

    fn owner_id(&self) -> ObjectID {
        self.object_id
    }

    fn update_drawable_supply_status(&self) {
        // C++ WorkerAIUpdate.cpp 1005-1015: getObject()->getDrawable().
        if let Some(owner) = &self.owner {
            let Some(owner) = owner.upgrade() else {
                return;
            };
            let drawable = owner.read().ok().and_then(|guard| guard.get_drawable());
            if let Some(drawable) = drawable {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.update_supply_status(self.with_data(|data| data.max_boxes), self.number_boxes);
                }
            }
            return;
        }
        // Wave 298: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.object_id == INVALID_ID {
            return;
        }
        let Some(drawable) = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.object_id, |owner_guard| owner_guard.get_drawable())
            .flatten()
        else {
            return;
        };
        if let Ok(mut draw_guard) = drawable.write() {
            draw_guard.update_supply_status(self.with_data(|data| data.max_boxes), self.number_boxes);
        };
    }

    fn sync_state_from_machine(&mut self) {
        let Some(machine) = &self.state_machine else {
            return;
        };
        match machine.current_state_id() {
            Some(ST_IDLE) => self.state = SupplyTruckState::Idle,
            Some(ST_BUSY) => self.state = SupplyTruckState::Busy,
            Some(ST_WANTING) => self.state = SupplyTruckState::Wanting,
            Some(ST_REGROUPING) => self.state = SupplyTruckState::Regrouping,
            Some(ST_DOCKING) => self.state = SupplyTruckState::Docking,
            _ => {}
        }
    }

    /// True when a dozer build/repair/fortify task is active (C++ AS_DOZER).
    fn is_acting_as_dozer(&self) -> bool {
        self.dozer_task.is_some() || self.current_task.is_some()
    }

    /// C++ WorkerAIUpdate::update: `getObject()->setWeaponSetFlag(WEAPONSET_MINE_CLEARING_DETAIL)`.
    /// Gathering workers can be diverted to clear mines while ferrying.
    fn set_harvest_mine_clearing_weapon_set(&self) {
        if dual_world_registry_unavailable() {
            return;
        }
        if self.object_id == INVALID_ID {
            return;
        }
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        else {
            return;
        };
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.set_weapon_set_flag(crate::weapon::WeaponSetType::MineClearingDetail);
        }
    }

    /// Update the worker: dozer machine or supply-truck harvest machine.
    /// Matches C++ WorkerAIUpdate::update.
    pub fn update(&mut self) -> StateReturnType {
        let Some(ai) = supply_owner_ai(self.object_id) else {
            return StateReturnType::Failure;
        };
        let Ok(mut ai) = ai.lock() else {
            return StateReturnType::Failure;
        };
        self.update_with_ai(&mut *ai)
    }

    /// Execute against UnitAI's existing mutable borrow, never its cached mutex.
    pub fn update_with_ai(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        if self.object_id == INVALID_ID {
            return StateReturnType::Failure;
        }
        if self.state_machine.is_none() {
            self.state_machine = Some(SupplyTruckStateMachine::new(self.object_id));
        }
        // WorkerAIUpdate::update selects the dozer machine before harvest.
        if self.is_acting_as_dozer() {
            if let Err(error) = self.update_dozer_task(ai) {
                log::error!("Worker construction completion failed: {error}");
                return StateReturnType::Error;
            }
            return StateReturnType::Continue;
        }
        let mut machine = self
            .state_machine
            .take()
            .expect("worker supply machine initialized");
        let status = machine.update(self, ai, true);
        self.state_machine = Some(machine);
        self.sync_state_from_machine();
        self.set_harvest_mine_clearing_weapon_set();
        status
    }

    /// Handle idle command (matches C++ WorkerAIUpdate::privateIdle).
    pub fn private_idle(&mut self, _cmd_source: CommandSourceType) {
        // Worker does not force busy on player stop (see C++ comment).
    }

    /// Handle dock command (matches C++ WorkerAIUpdate::privateDock).
    pub fn private_dock(&mut self, dock_id: Option<ObjectID>, cmd_source: CommandSourceType) {
        if cmd_source == CommandSourceType::FromPlayer {
            if let Some(dock_id) = dock_id {
                self.preferred_dock = Some(dock_id);
            }
        }
    }

    fn is_task_pending(&self, task: WorkerDozerTaskSlot) -> bool {
        self.dozer_tasks[task.as_index()].target_id != INVALID_ID
    }

    fn get_task_target(&self, task: WorkerDozerTaskSlot) -> Option<ObjectID> {
        let entry = &self.dozer_tasks[task.as_index()];
        if entry.target_id == INVALID_ID {
            None
        } else {
            Some(entry.target_id)
        }
    }

    fn set_current_task(&mut self, task: Option<WorkerDozerTaskSlot>) {
        self.current_task = task;
    }

    fn get_most_recent_task(&self) -> Option<WorkerDozerTaskSlot> {
        let mut most_recent: Option<WorkerDozerTaskSlot> = None;
        let mut most_recent_frame: u32 = 0;
        for slot in [
            WorkerDozerTaskSlot::Build,
            WorkerDozerTaskSlot::Repair,
            WorkerDozerTaskSlot::Fortify,
        ] {
            if self.is_task_pending(slot) {
                let entry = &self.dozer_tasks[slot.as_index()];
                if most_recent.is_none() || entry.order_frame > most_recent_frame {
                    most_recent = Some(slot);
                    most_recent_frame = entry.order_frame;
                }
            }
        }
        most_recent
    }

    fn clear_task(&mut self, task: WorkerDozerTaskSlot, owner: Option<&Arc<RwLock<Object>>>) {
        // WorkerAIUpdate.cpp:783–822 ends the builder animation with its task.
        if matches!(task, WorkerDozerTaskSlot::Build | WorkerDozerTaskSlot::Repair)
            && let Some(owner) = owner
            && let Ok(mut actor) = owner.write()
        {
            actor.clear_model_condition_state(ModelConditionFlags::ACTIVELY_CONSTRUCTING);
        }
        let idx = task.as_index();
        self.dozer_tasks[idx] = WorkerTaskEntry::default();
        for point in &mut self.dozer_dock_points[idx] {
            point.valid = false;
        }
        if self.current_task == Some(task) {
            self.current_task = None;
        }
    }

    fn set_dock_points_for_task(&mut self, task: WorkerDozerTaskSlot, position: Coord3D) {
        let idx = task.as_index();
        self.dozer_dock_points[idx][0] = WorkerDockPoint {
            valid: true,
            location: position,
        };
        self.dozer_dock_points[idx][1] = WorkerDockPoint {
            valid: true,
            location: position,
        };
        self.dozer_dock_points[idx][2] = WorkerDockPoint {
            valid: true,
            location: position,
        };
    }

    fn bridge_scaffold_blocks_heal(tower_id: ObjectID) -> bool {
        let Some(tower_obj) = TheGameLogic::find_object_by_id(tower_id) else {
            return false;
        };
        let bridge_id = {
            let Ok(tower_guard) = tower_obj.read() else {
                return false;
            };
            let mut id = INVALID_ID;
            for mut behavior in tower_guard.get_behavior_modules() {
                let Ok(mut behavior) = behavior.access() else {
                    continue;
                };
                if let Some(tower) = behavior.get_bridge_tower_behavior_interface() {
                    id = tower.get_bridge_id();
                    break;
                }
            }
            id
        };
        if bridge_id == INVALID_ID {
            return false;
        }
        let Some(bridge_obj) = TheGameLogic::find_object_by_id(bridge_id) else {
            return false;
        };
        let Ok(bridge_guard) = bridge_obj.read() else {
            return false;
        };
        let behaviors = bridge_guard.get_behavior_modules();
        drop(bridge_guard);
        for mut behavior in behaviors {
            let Ok(mut behavior) = behavior.access() else {
                continue;
            };
            if let Some(bridge) = behavior.get_bridge_behavior_interface() {
                bridge.create_scaffolding();
                return bridge.is_scaffold_in_motion();
            }
        }
        false
    }

    fn remove_bridge_scaffolding(bridge_tower_id: ObjectID) {
        let Some(tower_obj) = TheGameLogic::find_object_by_id(bridge_tower_id) else {
            return;
        };
        let Ok(tower_guard) = tower_obj.read() else {
            return;
        };
        let mut bridge_id: Option<ObjectID> = None;
        for mut behavior in tower_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.access() else {
                continue;
            };
            let Some(tower) = behavior.get_bridge_tower_behavior_interface() else {
                continue;
            };
            bridge_id = Some(tower.get_bridge_id());
            if bridge_id.is_some() {
                break;
            }
        }
        let Some(bridge_id) = bridge_id else {
            return;
        };
        let Some(bridge_obj) = TheGameLogic::find_object_by_id(bridge_id) else {
            return;
        };
        let Ok(bridge_guard) = bridge_obj.read() else {
            return;
        };
        let behaviors = bridge_guard.get_behavior_modules();
        drop(bridge_guard);
        for mut behavior in behaviors {
            let Ok(mut behavior) = behavior.access() else {
                continue;
            };
            let Some(bridge) = behavior.get_bridge_behavior_interface() else {
                continue;
            };
            if let Err(err) = bridge.try_remove_scaffolding() {
                log::debug!(
                    "WorkerAIUpdate::remove_bridge_scaffolding failed for bridge {}: {}",
                    bridge_id,
                    err
                );
            }
            break;
        }
    }

    fn construction_objects(
        &self,
        target_id: ObjectID,
    ) -> Option<(Arc<RwLock<Object>>, Arc<RwLock<Object>>)> {
        if self.object_id == INVALID_ID || dual_world_registry_unavailable() {
            return None;
        }
        let owner = TheGameLogic::find_object_by_id(self.object_id)?;
        // The legacy construction query runs in the published Core world. Its
        // installed Worker must be the owner registered in that same world.
        if self.owner.as_ref().is_some_and(|bound| {
            !std::ptr::eq(bound.as_ptr(), Arc::as_ptr(&owner))
        }) {
            return None;
        }
        Some((owner, TheGameLogic::find_object_by_id(target_id)?))
    }

    fn new_task(
        &mut self,
        task: WorkerDozerTaskSlot,
        owner: Arc<RwLock<Object>>,
        target: Arc<RwLock<Object>>,
        owner_airborne: bool,
    ) -> Option<ObjectID> {
        let mut dock = Coord3D::ZERO;
        let has_dock = matches!(task, WorkerDozerTaskSlot::Build | WorkerDozerTaskSlot::Repair);
        let (direct_target, target_id) = {
            let owner = owner.read().ok()?;
            let target = target.read().ok()?;
            let direct = target.get_id();
            let selected = if has_dock {
                // WorkerAIUpdate.cpp:619–620 uses the shared Dozer order dock.
                DozerAIUpdate::find_good_build_or_repair_position_and_target_with_locomotor(
                    &owner, &target, &mut dock, Some(owner_airborne),
                )?
            } else {
                direct
            };
            (direct, selected)
        };

        // WorkerAIUpdate.cpp:631–645 claims the selected building before
        // writing all three dock points and the task's original order frame.
        if task == WorkerDozerTaskSlot::Build {
            let target = if direct_target == target_id {
                target
            } else {
                TheGameLogic::find_object_by_id(target_id)?
            };
            target.write().ok()?.set_builder_id(self.object_id);
        }
        self.preferred_dock = None;
        if has_dock {
            self.set_dock_points_for_task(task, dock);
        }
        self.dozer_tasks[task.as_index()] = WorkerTaskEntry {
            target_id,
            order_frame: TheGameLogic::get_frame(),
        };
        self.set_current_task(Some(task));
        Some(target_id)
    }

    fn spawn_dozer_task_from_current(&mut self) {
        let Some(current) = self.current_task else {
            return;
        };
        let Some(target_id) = self.get_task_target(current) else {
            return;
        };
        match current {
            WorkerDozerTaskSlot::Build => {
                self.dozer_task = Some(WorkerDozerTask {
                    task_type: WorkerDozerTaskType::Build,
                    target_id,
                    dock_point: None,
                    failed_attempts: 0,
                    build_total_frames: 0,
                    build_max_health: 0.0,
                    is_rebuild: false,
                    started_construction: false,
                    build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
                });
            }
            WorkerDozerTaskSlot::Repair => {
                self.dozer_task = Some(WorkerDozerTask {
                    task_type: WorkerDozerTaskType::Repair,
                    target_id,
                    dock_point: None,
                    failed_attempts: 0,
                    build_total_frames: 0,
                    build_max_health: 0.0,
                    is_rebuild: false,
                    started_construction: false,
                    build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
                });
            }
            WorkerDozerTaskSlot::Fortify => {
                self.dozer_task = Some(WorkerDozerTask {
                    task_type: WorkerDozerTaskType::Fortify,
                    target_id,
                    dock_point: None,
                    failed_attempts: 0,
                    build_total_frames: 0,
                    build_max_health: 0.0,
                    is_rebuild: false,
                    started_construction: false,
                    build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
                });
            }
        }
    }

    /// Issue a repair task to this worker (matches C++ WorkerAIUpdate::privateRepair).
    pub fn set_repair_target(
        &mut self,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
        owner_airborne: bool,
    ) {
        let Some((owner, target)) = self.construction_objects(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_repair_object(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }
        // C++ WorkerAIUpdate::privateRepair — sole healer, not builder_id.
        let sole = target_guard.get_sole_healing_benefactor();
        if sole != INVALID_ID && sole != self.object_id {
            return;
        }

        drop(target_guard);
        drop(owner_guard);
        let Some(target_id) = self.new_task(WorkerDozerTaskSlot::Repair, owner, target, owner_airborne) else {
            return;
        };
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Repair,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
            build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    /// Issue a resume-construction task (matches C++ WorkerAIUpdate::privateResumeConstruction).
    pub fn set_resume_construction_target(
        &mut self,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
        owner_airborne: bool,
    ) {
        let Some((owner, target)) = self.construction_objects(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_resume_construction_with_builder(
            &owner_guard, &target_guard, cmd_source, self.active_build_target(),
        ) {
            return;
        }
        // C++ resume is DOZER_TASK_BUILD: percent += 100/frames, heal max/frames.
        let frames = target_guard.get_template().calc_time_to_build(None).max(1) as u32;
        let max_health = target_guard
            .get_body_module()
            .map(|b| b.get_max_health())
            .unwrap_or(0.0);

        drop(target_guard);
        drop(owner_guard);
        let Some(target_id) = self.new_task(WorkerDozerTaskSlot::Build, owner, target, owner_airborne) else {
            return;
        };
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Build,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: frames,
            build_max_health: max_health,
            is_rebuild: false,
            started_construction: false,
            build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    /// Issue a build task for a newly created construction site.
    pub fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
        owner_airborne: bool,
    ) {
        let Some((owner, target)) = self.construction_objects(building_id) else {
            return;
        };
        let Some(target_id) = self.new_task(WorkerDozerTaskSlot::Build, owner, target, owner_airborne) else {
            return;
        };
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Build,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: total_build_frames.max(1),
            build_max_health: max_health,
            is_rebuild,
            started_construction: false,
            build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    fn update_dozer_task(&mut self, ai: &mut dyn AIUpdateInterface) -> Result<(), String> {
        let mut failure = None;
        (|| {
        // Wave 298: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        const MIN_ACTION_TOLERANCE: Real = 70.0;
        let repair_rate = self.get_repair_health_per_second();
        let clear_current = |this: &mut WorkerAIUpdate, owner| {
            if let Some(current) = this.current_task {
                this.clear_task(current, owner);
            }
        };
        let Some(target_id) = self.dozer_task.as_ref().map(|task| task.target_id) else {
            return;
        };
        let Some((owner, target)) = self.construction_objects(target_id) else {
            self.dozer_task = None;
            let actor = self.owner.as_ref().and_then(std::sync::Weak::upgrade);
            clear_current(self, actor.as_ref());
            return;
        };
        let installed = self.owner.is_some();
        let task = self.dozer_task.as_mut().expect("active worker construction task");
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        let owner_pos = *owner_guard.get_position();
        let building = task.task_type == WorkerDozerTaskType::Build;
        let changed_controller = installed && building
            && self.dozer_action_state == WorkerDozerActionState::DoAction
            && owner_guard.get_controlling_player_id() != target_guard.get_controlling_player_id();
        drop(owner_guard);
        let target_pos = *target_guard.get_position();
        let target_radius = target_guard
            .get_geometry_info()
            .get_bounding_sphere_radius();
        let target_builder_id = target_guard.get_builder_id();
        let target_sole = target_guard.get_sole_healing_benefactor();
        let target_is_bridge_tower = target_guard.is_kind_of(KindOf::BridgeTower);
        drop(target_guard);

        if self.dozer_action_state == WorkerDozerActionState::PickActionPos {
            // DozerAIUpdate.cpp:151–216 uses the Start dock when supplied.
            // The fallback searches a fixed radius, towards the worker.
            let start = self.current_task.and_then(|current| {
                let start = self.dozer_dock_points[current.as_index()][0];
                start.valid.then_some(start.location)
            });
            let dock = start.unwrap_or_else(|| {
                let direction = owner_pos - target_pos;
                let mut options = FindPositionOptions::default();
                options.min_radius = target_radius;
                options.max_radius = target_radius;
                options.start_angle = Some(direction.y.atan2(direction.x));
                let mut position = target_pos;
                if let Some(partition) = ThePartitionManager::get() {
                    partition.find_position_around_with_options(&target_pos, &options, &mut position);
                }
                position
            });
            task.dock_point = Some(dock);
            let mut command = AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
            command.pos = dock;
            if let Err(error) = ai.ignore_obstacle_id(target_id).and_then(|()| ai.execute_command(&command)) {
                failure = Some(error.to_string());
                return;
            }
            self.dozer_action_state = WorkerDozerActionState::MoveToActionPos;
            return;
        }

        // DozerAIUpdate.cpp:474–477 rechecks the controlling player in the
        // shared action without re-running the command admission predicate.
        if changed_controller {
            self.dozer_task = None;
            clear_current(self, Some(&owner));
            return;
        }
        if self.dozer_action_state == WorkerDozerActionState::DoAction
            && building && task.build_sub_task != DozerBuildSubTask::DoBuildAtDock
        {
            let dock = self.current_task.and_then(|current| {
                let dock = self.dozer_dock_points[current.as_index()][DozerDockPoint::Action as usize];
                dock.valid.then_some(dock.location)
            });
            // WorkerAIUpdate.cpp:247–280 runs the same nested Dozer action.
            match task.build_sub_task.advance_to_dock(ai, dock) {
                Ok(true) => {}
                result => {
                    failure = result.err();
                    return;
                }
            }
        }

        let mut build_complete_rebuild: Option<bool> = None;

        match self.dozer_action_state {
            WorkerDozerActionState::MoveToActionPos => {
                let other_builder = building && target_builder_id != self.object_id;
                let other_repairer = task.task_type == WorkerDozerTaskType::Repair
                    && target_sole != INVALID_ID && target_sole != self.object_id;
                if other_builder || other_repairer {
                    self.dozer_task = None;
                    clear_current(self, Some(&owner));
                    return;
                }
                let Ok(owner_guard) = owner.read() else { return };
                let dock = task.dock_point.unwrap_or(target_pos);
                let distance = ThePartitionManager::get_distance_squared_to_pos(
                    &owner_guard, &dock, crate::common::FROM_BOUNDING_SPHERE_2D,
                );
                let tolerance = MIN_ACTION_TOLERANCE.max(
                    owner_guard.get_geometry_info().get_bounding_sphere_radius() + 15.0,
                );
                drop(owner_guard);
                if distance <= tolerance * tolerance {
                    if building && let Ok(mut target) = target.write() {
                        let _ = target.clear_and_set_model_condition_flags(
                            ModelConditionFlags::AWAITING_CONSTRUCTION,
                            ModelConditionFlags::PARTIALLY_CONSTRUCTED
                                | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
                        );
                    }
                    if building {
                        task.build_sub_task = DozerBuildSubTask::SelectBuildDockLocation;
                    }
                    self.dozer_action_state = WorkerDozerActionState::DoAction;
                } else if ai.is_idle() {
                    task.dock_point = None;
                    self.dozer_action_state = WorkerDozerActionState::PickActionPos;
                }
            }
            WorkerDozerActionState::DoAction => match task.task_type {
                WorkerDozerTaskType::Repair => {
                    if target_sole != INVALID_ID && target_sole != self.object_id {
                        self.dozer_task = None;
                        clear_current(self, Some(&owner));
                        return;
                    }
                    let can_repair = {
                        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
                            self.dozer_task = None;
                            return;
                        };
                        ActionManager::can_repair_object(&owner_guard, &target_guard, CommandSourceType::FromAi)
                    };
                    if !can_repair {
                        self.dozer_task = None;
                        clear_current(self, Some(&owner));
                        return;
                    }
                    let already_full = target
                        .read()
                        .ok()
                        .and_then(|guard| guard.get_body_module())
                        .map(|body| body.get_health() == body.get_max_health())
                        .unwrap_or(false);
                    if already_full {
                        // C++ TheGameText / TheInGameUI of the dozer's engine.
                        if let Ok(owner_guard) = owner.read() {
                            let message = crate::helpers::TheGameText::fetch_for(&owner_guard, "DOZER:RepairComplete");
                            crate::helpers::TheInGameUI::display_message_for(&owner_guard, &message);
                        }
                        if target_is_bridge_tower {
                            Self::remove_bridge_scaffolding(task.target_id);
                        }
                        self.dozer_task = None;
                        self.clear_task(WorkerDozerTaskSlot::Repair, Some(&owner));
                        return;
                    }
                    let can_heal = !target_is_bridge_tower
                        || !Self::bridge_scaffold_blocks_heal(task.target_id);
                    if can_heal {
                        let health = {
                            let max_health = if let Ok(tg) = target.read() {
                                tg.get_body_module()
                                    .map(|b| b.get_max_health())
                                    .unwrap_or(0.0)
                            } else {
                                0.0
                            };
                            max_health * repair_rate * SECONDS_PER_LOGICFRAME_REAL
                        };
                        let healed = if let Ok(mut tw) = target.write() {
                            match tw.attempt_healing_from_sole_benefactor_id(
                                health,
                                self.object_id,
                                2,
                            ) {
                                Ok(ok) => ok,
                                Err(_) => false,
                            }
                        } else {
                            false
                        };
                        if !healed {
                            self.dozer_task = None;
                            clear_current(self, Some(&owner));
                            return;
                        }
                    }
                }
                WorkerDozerTaskType::Build => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self, Some(&owner));
                        return;
                    }
                    // DozerAIUpdate.cpp:507–514 sets this before every contribution.
                    if let Ok(mut actor) = owner.write() {
                        actor.set_model_condition_state(ModelConditionFlags::ACTIVELY_CONSTRUCTING);
                    }
                    if installed {
                        let frames = DozerAIUpdate::current_build_frames(&owner, &target);
                        let Ok(mut scaffold) = target.write() else { return };
                        // DozerAIUpdate.cpp:511–536 uses the live scaffold and
                        // body. Replacing the builder never replaces either.
                        let progress = scaffold.get_construction_percent();
                        if progress < 0.0
                            || scaffold.get_builder_id() != self.object_id
                            || scaffold.is_destroyed()
                            || scaffold.is_effectively_dead()
                        {
                            drop(scaffold);
                            self.dozer_task = None;
                            clear_current(self, Some(&owner));
                            return;
                        }
                        let progress = progress + 100.0 / frames as f32;
                        scaffold.set_construction_percent(progress);
                        if let Err(error) = scaffold.add_construction_health_with_owner(frames) {
                            log::debug!("Worker construction health update failed: {error}");
                        }
                        scaffold.set_producer_id(self.object_id);
                        scaffold.set_builder_id(self.object_id);
                        if progress >= 100.0 {
                            build_complete_rebuild = Some(task.is_rebuild);
                        }
                    } else {
                        let frames = task.build_total_frames.max(1);
                        let world = crate::system::engine_stores::active_services();
                        let construction_manager = &world.logic_managers().construction;
                        // A Resume order enters the same C++ BUILD action. Start
                        // tracking from the scaffold's existing percent and HP.
                        let starting_progress = if !task.started_construction {
                            let Ok(building) = target.read() else {
                                self.dozer_task = None;
                                return;
                            };
                            let max_health = if task.build_max_health > 0.0 {
                                task.build_max_health
                            } else {
                                building.get_body_module().map(|body| body.get_max_health()).unwrap_or(0.0)
                            };
                            Some((max_health, building.get_construction_percent(), building.get_health()))
                        } else {
                            None
                        };
                        let (completed, progress, current_health) = {
                            let mut manager = match construction_manager.write() {
                                Ok(manager) => manager,
                                Err(_) => {
                                    self.dozer_task = None;
                                    return;
                                }
                            };
                            if let Some((max_health, percent, health)) = starting_progress {
                                match manager.start_construction(
                                    task.target_id,
                                    self.object_id,
                                    max_health,
                                    frames,
                                    task.is_rebuild,
                                ) {
                                    Ok(()) => manager.seed_from_structure(task.target_id, percent, health),
                                    Err(error) => log::debug!("WorkerAIUpdate::update_dozer_task start_construction failed: {error}"),
                                }
                                task.started_construction = true;
                            }
                            let completed = manager.update_for_dozer(self.object_id);
                            let progress = manager.get_progress(task.target_id);
                            let current_health = manager.get_current_health(task.target_id);
                            (completed, progress, current_health)
                        };
                        if let Some(progress) = progress
                            && let Ok(mut target_write) = target.write()
                        {
                            target_write.set_construction_percent(progress);
                            if let Some(health) = current_health {
                                if let Err(err) = target_write.set_health(health) {
                                    log::debug!(
                                        "WorkerAIUpdate::update_dozer_task set_health failed: {}",
                                        err
                                    );
                                }
                            }
                            target_write.set_producer_id(self.object_id);
                            target_write.set_builder_id(self.object_id);
                        }
                        if completed == Some(task.target_id) {
                            build_complete_rebuild = Some(task.is_rebuild);
                        }
                    }
                }
                WorkerDozerTaskType::Fortify => {
                    // C++ path leaves fortify as a no-op; complete immediately so the worker AI does not stall.
                    self.dozer_task = None;
                    self.clear_task(WorkerDozerTaskSlot::Fortify, Some(&owner));
                }
            },
            WorkerDozerActionState::PickActionPos => {}
        }
        if let Some(is_rebuild) = build_complete_rebuild {
            if let Err(error) = self.handle_build_completion(&owner, &target, is_rebuild) {
                failure = Some(error);
                return;
            }
            let dock = self.dozer_dock_points[WorkerDozerTaskSlot::Build.as_index()][DozerDockPoint::End as usize];
            DozerAIUpdate::move_to_end_dock(&owner, ai, dock.valid.then_some(dock.location));
            self.dozer_task = None;
            self.clear_task(WorkerDozerTaskSlot::Build, Some(&owner));
        }
        })();
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn handle_build_completion(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
        is_rebuild: bool,
    ) -> Result<(), String> {
        let mut target_display_name: Option<String> = None;
        let mut target_pos: Option<LogicCoord3D> = None;
        let mut controlling_player: Option<Arc<RwLock<crate::player::Player>>> = None;

        if let Ok(mut target_guard) = target.write() {
            target_guard.clear_status(
                crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::UnderConstruction,
                ) | crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::Reconstructing,
                ),
            );

            if let Err(err) = target_guard.clear_model_condition_flags(
                ModelConditionFlags::AWAITING_CONSTRUCTION
                    | ModelConditionFlags::PARTIALLY_CONSTRUCTED
                    | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
            ) {
                log::debug!(
                    "WorkerAIUpdate::handle_build_completion clear_model_condition_flags failed: {}",
                    err
                );
            }
            target_guard.set_construction_percent(crate::object::CONSTRUCTION_COMPLETE);

            if let Some(body) = target_guard.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    if let Err(err) = body_guard.evaluate_visual_condition() {
                        log::debug!(
                            "WorkerAIUpdate::handle_build_completion evaluate_visual_condition failed: {}",
                            err
                        );
                    }
                }
            }

            let template = target_guard.get_template();
            target_display_name = Some(template.get_name().as_str().to_string());
            target_pos = Some(*target_guard.get_position());
            controlling_player = target_guard.get_controlling_player();
        }

        if let Some(player) = controlling_player {
            let builder_id = owner.read().ok().map(|guard| guard.get_id());
            crate::player::Player::on_structure_construction_complete(
                &player,
                builder_id,
                target,
                is_rebuild,
            )?;
            // WorkerAIUpdate.cpp uses DozerPrimaryStateMachine: player and AI
            // notification precedes create hooks, and requires a controller.
            if let Ok(mut target_guard) = target.write() {
                target_guard.on_build_complete();
            }
        }

        if let Ok(mut target_guard) = target.write() {
            target_guard.handle_partition_cell_maintenance();
            target_guard.update_upgrade_modules_from_player();
        }

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_locally_controlled() {
                if let Some(display_name) = target_display_name.as_ref() {
                    let format = crate::helpers::TheGameText::fetch_for(&owner_guard, "DOZER:ConstructionComplete");
                    let message = if format.contains("%s") {
                        format.replace("%s", display_name)
                    } else {
                        format!("{} {}", format, display_name)
                    };
                    crate::helpers::TheInGameUI::display_message_for(&owner_guard, &message);
                }

                if let Some(voice) = owner_guard
                    .get_template()
                    .get_per_unit_sound("VoiceTaskComplete")
                {
                    if let Some(audio) = TheAudio::of_object(&owner_guard) {
                        let mut event = voice;
                        event.set_object_id(owner_guard.get_id());
                        audio.add_audio_event(&event);
                    }
                }

                if let (Some(radar), Some(pos)) = (crate::helpers::TheRadar::get(), target_pos) {
                    radar.create_event(
                        &pos,
                        game_engine::common::system::radar::RadarEventType::Construction,
                        4.0,
                    );
                }
            }
        }
        Ok(())
    }

    /// Same interface as SupplyTruckAIUpdate for consistency
    pub fn lose_one_box(&mut self) -> bool {
        if self.number_boxes == 0 {
            return false;
        }
        self.number_boxes -= 1;
        self.update_drawable_supply_status();
        true
    }

    /// C++ WorkerAIUpdate::aiDoCommand: drop carried boxes when clearing mines.
    pub fn drop_all_boxes_if_carrying(&mut self) {
        if self.number_boxes <= 0 {
            return;
        }
        self.number_boxes = 0;
        self.update_drawable_supply_status();
    }

    pub fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        // Wave 298: empty dual-world → fail-closed.
        if self.owner.is_none() && dual_world_registry_unavailable() {
            return false;
        }

        if self.number_boxes >= self.with_data(|data| data.max_boxes) {
            return false;
        }
        self.number_boxes += 1;

        // Play depleted voice if took last box
        if remaining_stock == 0 && self.with_data(|data| !data.supplies_depleted_voice.is_empty()) {
            let mut play_depleted = true;
            if let Some(best_warehouse) = resource::find_best_supply_warehouse(self.object_id) {
                if let (Some(owner), Some(warehouse)) = (
                    TheGameLogic::find_object_by_id(self.object_id).or_else(|| {
                        crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id)
                    }),
                    TheGameLogic::find_object_by_id(best_warehouse),
                ) {
                    if let (Ok(owner_guard), Ok(warehouse_guard)) = (owner.read(), warehouse.read())
                    {
                        let delta = *owner_guard.get_position() - *warehouse_guard.get_position();
                        let distance =
                            (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt();
                        let is_ai_player = owner_guard
                            .get_controlling_player_id()
                            .and_then(|player_id| {
                                crate::player::with_player(player_id as i32, |guard| {
                                    guard.is_skirmish_ai()
                                })
                            })
                            .unwrap_or(false);
                        if distance <= self.get_warehouse_scan_distance(is_ai_player) / 4.0 {
                            play_depleted = false;
                        }
                    }
                }
            }

            if play_depleted {
                if let Some(audio) = &self.audio_system {
                    self.with_data(|data| audio.play_voice_event(data.supplies_depleted_voice, self.object_id));
                }
            }
        }

        self.update_drawable_supply_status();
        true
    }

    /// C++ WorkerAIUpdate.cpp:1376-1386 gates on Upgrade_GLAWorkerShoes.
    /// Without a live upgrade owner, never invent the authored boost amount.
    pub fn get_upgraded_supply_boost(&self) -> u32 {
        if let Some(upgrade) = &self.upgrade_system {
            upgrade.get_supply_boost(self.player_index)
        } else {
            0
        }
    }

    /// Repair health percent per second (matches C++ WorkerAIUpdate::getRepairHealthPerSecond).
    pub fn get_repair_health_per_second(&self) -> Real {
        self.with_data(|data| data.repair_health_percent_per_second)
    }

    /// Worker bored time (matches C++ WorkerAIUpdate::getBoredTime).
    pub fn get_bored_time(&self) -> Real {
        self.with_data(|data| data.bored_time)
    }

    /// Worker bored range (matches C++ WorkerAIUpdate::getBoredRange).
    pub fn get_bored_range(&self) -> Real {
        self.with_data(|data| data.bored_range)
    }

    pub fn get_number_boxes(&self) -> i32 {
        self.number_boxes
    }

    pub fn set_preferred_dock(&mut self, dock_id: ObjectID) {
        self.preferred_dock = Some(dock_id);
    }

    pub fn get_preferred_dock(&self) -> Option<ObjectID> {
        self.preferred_dock
    }

    pub fn set_force_wanting_state(&mut self, force: bool) {
        self.force_wanting_state = force;
    }

    pub fn is_forced_into_wanting_state(&self) -> bool {
        self.force_wanting_state
    }

    pub fn set_force_busy_state(&mut self, force: bool) {
        self.force_busy_state = force;
    }

    pub fn is_forced_into_busy_state(&self) -> bool {
        self.force_busy_state
    }

    pub fn is_available_for_supplying(&self) -> bool {
        true
    }

    pub fn is_currently_ferrying_supplies(&self) -> bool {
        if let Some(machine) = &self.state_machine {
            match machine.current_state_id() {
                Some(ST_IDLE) | Some(ST_BUSY) | Some(ST_REGROUPING) => false,
                Some(ST_WANTING) | Some(ST_DOCKING) => true,
                _ => false,
            }
        } else {
            matches!(
                self.state,
                SupplyTruckState::Wanting | SupplyTruckState::Docking
            )
        }
    }

    /// Get action delay for a dock (matches C++ WorkerAIUpdate::getActionDelayForDock).
    /// C++ WorkerAIUpdate::getActionDelayForDock — warehouse, else center, else 0.
    pub fn get_action_delay_for_dock_modules(
        &self,
        has_warehouse: bool,
        has_center: bool,
    ) -> u32 {
        self.with_data(|data| {
            if has_warehouse {
                data.warehouse_delay
            } else if has_center {
                data.center_delay
            } else {
                0
            }
        })
    }

    pub fn get_action_delay_for_dock(&self, is_warehouse: bool) -> u32 {
        self.get_action_delay_for_dock_modules(is_warehouse, !is_warehouse)
    }

    /// Get warehouse scan distance (AI players get 2x distance).
    pub fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Real {
        self.with_data(|data| {
            if is_ai_player {
                data.warehouse_scan_distance * 2.0
            } else {
                data.warehouse_scan_distance
            }
        })
    }

    pub fn get_state(&self) -> SupplyTruckState {
        self.state
    }

    pub fn set_state(&mut self, state: SupplyTruckState) {
        self.state = state;
    }
}

#[cfg(test)]
mod worker_dozer_parity_tests {
    use super::*;

    #[test]
    fn drop_all_boxes_if_carrying_clears_count() {
        // C++ WorkerAIUpdate.cpp:1043-1050.
        let mut worker = WorkerAIUpdate::new(WorkerAIUpdateData::default(), INVALID_ID, 0);
        worker.number_boxes = 3;
        worker.drop_all_boxes_if_carrying();
        assert_eq!(worker.get_number_boxes(), 0);
        worker.drop_all_boxes_if_carrying();
        assert_eq!(worker.get_number_boxes(), 0);
    }

    fn in_world(f: impl FnOnce()) {
        let _serial = crate::test_sync::lock();
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let stores = crate::system::engine_stores::new_for_world();
            crate::system::engine_stores::with_active_stores(&stores, f);
        });
    }

    fn owner_with_drawable(
        id: ObjectID,
    ) -> (
        Arc<RwLock<Object>>,
        Arc<RwLock<crate::object::drawable::Drawable>>,
    ) {
        let owner = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
        let drawable = Arc::new(RwLock::new(crate::object::drawable::Drawable::new(
            0xA0_0000 + id,
            id,
            "WorkerOwnerTest".to_owned(),
            crate::object::drawable::DrawableType::Static,
        )));
        owner.write().unwrap().set_drawable(Some(drawable.clone()));
        (owner, drawable)
    }

    fn carrying(drawable: &Arc<RwLock<crate::object::drawable::Drawable>>) -> bool {
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::CARRYING)
    }

    #[test]
    fn bound_worker_notifies_its_own_drawable_not_a_same_id_foreign_owner() {
        in_world(|| {
        // C++ WorkerAIUpdate.cpp:1005-1015: getObject()->getDrawable() with the
        // authored MaxBoxes of that actual owner.
        let (owner_a, drawable_a) = owner_with_drawable(7);
        let (owner_b, drawable_b) = owner_with_drawable(7);
        let mut data_a = WorkerAIUpdateData::default();
        data_a.max_boxes = 1;
        let mut data_b = WorkerAIUpdateData::default();
        data_b.max_boxes = 3;
        let mut worker_a = WorkerAIUpdate::new(data_a, 7, 0);
        let mut worker_b = WorkerAIUpdate::new(data_b, 7, 0);
        worker_a.bind_owner(&owner_a);
        worker_b.bind_owner(&owner_b);

        assert!(worker_a.gain_one_box(5));
        assert!(carrying(&drawable_a));
        assert!(!carrying(&drawable_b));
        // A's capacity (1) is exhausted; B's authored capacity is independent.
        assert!(!worker_a.gain_one_box(5));
        assert!(worker_b.gain_one_box(5));
        assert!(worker_b.gain_one_box(5));
        assert!(carrying(&drawable_b));
        assert!(worker_b.lose_one_box());
        assert!(carrying(&drawable_b));
        assert!(worker_b.lose_one_box());
        assert!(!carrying(&drawable_b));
        assert!(carrying(&drawable_a));
        // Mine-clearing drop clears only A's own drawable.
        worker_a.drop_all_boxes_if_carrying();
        assert!(!carrying(&drawable_a));
        assert_eq!(worker_a.get_number_boxes(), 0);
        });
    }

    #[test]
    fn bound_worker_with_expired_owner_keeps_cargo_but_notifies_nothing() {
        in_world(|| {
        let (owner, drawable) = owner_with_drawable(9);
        let mut data = WorkerAIUpdateData::default();
            data.max_boxes = 2;
            let mut worker = WorkerAIUpdate::new(data, 9, 0);
        worker.bind_owner(&owner);
        drop(owner);
        assert!(worker.gain_one_box(5));
        assert_eq!(worker.get_number_boxes(), 1);
        assert!(!carrying(&drawable));
        });
    }

    #[test]
    fn ungated_worker_shoes_boost_is_zero_without_upgrade_owner() {
        // C++ WorkerAIUpdate.cpp:1376-1386 gates on Upgrade_GLAWorkerShoes.
        let mut data = WorkerAIUpdateData::default();
        data.upgraded_supply_boost = 8;
        let worker = WorkerAIUpdate::new(data, INVALID_ID, 0);
        assert_eq!(worker.get_upgraded_supply_boost(), 0);
    }

    #[test]
    fn non_supply_dock_action_delay_is_zero() {
        // C++ WorkerAIUpdate.cpp:196-213 returns 0 without warehouse/center modules.
        let mut data = WorkerAIUpdateData::default();
        data.warehouse_delay = 30;
        data.center_delay = 20;
        let worker = WorkerAIUpdate::new(data, INVALID_ID, 0);
        assert_eq!(worker.get_action_delay_for_dock_modules(false, false), 0);
        assert_eq!(worker.get_action_delay_for_dock_modules(true, false), 30);
        assert_eq!(worker.get_action_delay_for_dock_modules(false, true), 20);
    }
}

#[cfg(test)]
#[path = "construction_tests.rs"]
mod construction_tests;

