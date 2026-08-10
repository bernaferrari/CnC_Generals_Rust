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

/// GLA worker AI (similar to supply truck but for GLA faction)
/// GLA workers gather supplies on foot from supply piles
pub struct WorkerAIUpdate {
    /// Worker configuration (similar to SupplyTruckAIUpdate)
    data: WorkerAIUpdateData,
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
}

/// Dozer-style task types for workers (matches C++ WorkerAIUpdate task handling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerTaskType {
    Repair,
    ResumeConstruction,
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
            location: Coord3D::zero(),
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
            data,
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
        }
    }

    fn owner_id(&self) -> ObjectID {
        self.object_id
    }

    fn update_drawable_supply_status(&self) {
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
            draw_guard.update_supply_status(self.data.max_boxes, self.number_boxes);
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

    /// Update the worker supply state machine.
    pub fn update(&mut self) -> StateReturnType {
        if self.state_machine.is_none() {
            if self.object_id != INVALID_ID {
                self.state_machine = Some(SupplyTruckStateMachine::new(self.object_id));
            } else {
                return StateReturnType::Failure;
            }
        }

        let status = if let Some(machine) = &mut self.state_machine {
            machine.update()
        } else {
            StateReturnType::Failure
        };
        self.sync_state_from_machine();
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

    fn clear_task(&mut self, task: WorkerDozerTaskSlot) {
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

    fn find_action_position_for_target(&self, owner: &Object, target: &Object) -> Coord3D {
        let radius = target.get_geometry_info().get_bounding_sphere_radius();
        let start_angle = (owner.get_position().y - target.get_position().y)
            .atan2(owner.get_position().x - target.get_position().x);
        let mut options = FindPositionOptions::default();
        options.min_radius = radius;
        options.max_radius = radius;
        options.start_angle = Some(start_angle);

        let mut result = *target.get_position();
        if let Some(partition) = ThePartitionManager::get() {
            if partition.find_position_around_with_options(
                target.get_position(),
                &options,
                &mut result,
            ) {
                return Coord3D::new(result.x, result.y, result.z);
            }
        }
        Coord3D::new(
            target.get_position().x,
            target.get_position().y,
            target.get_position().z,
        )
    }

    fn remove_bridge_scaffolding(bridge_tower_id: ObjectID) {
        let Some(tower_obj) = TheGameLogic::find_object_by_id(bridge_tower_id) else {
            return;
        };
        let Ok(tower_guard) = tower_obj.read() else {
            return;
        };
        let mut bridge_id: Option<ObjectID> = None;
        for behavior in tower_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
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
        for behavior in bridge_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
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

    fn new_task(&mut self, task: WorkerDozerTaskSlot, target_id: ObjectID) {
        // Wave 298: empty dual-world → no-op.
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
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };

        self.preferred_dock = None;

        if task == WorkerDozerTaskSlot::Build || task == WorkerDozerTaskSlot::Repair {
            let pos = self.find_action_position_for_target(&owner_guard, &target_guard);
            self.set_dock_points_for_task(task, pos);
        }
        if task == WorkerDozerTaskSlot::Build {
            if let Ok(mut target_write) = target.write() {
                target_write.set_builder(Some(&owner_guard));
            }
        }

        self.dozer_tasks[task.as_index()].target_id = target_id;
        self.dozer_tasks[task.as_index()].order_frame = TheGameLogic::get_frame();
        self.set_current_task(Some(task));
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
                });
            }
        }
    }

    /// Issue a repair task to this worker (matches C++ WorkerAIUpdate::privateRepair).
    pub fn set_repair_target(&mut self, target_id: ObjectID, cmd_source: CommandSourceType) {
        // Wave 298: empty dual-world → no-op.
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
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_repair_object(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }

        self.new_task(WorkerDozerTaskSlot::Repair, target_id);
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Repair,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    /// Issue a resume-construction task (matches C++ WorkerAIUpdate::privateResumeConstruction).
    pub fn set_resume_construction_target(
        &mut self,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        // Wave 298: empty dual-world → no-op.
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
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_resume_construction_of(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }

        self.new_task(WorkerDozerTaskSlot::Build, target_id);
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::ResumeConstruction,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
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
    ) {
        self.new_task(WorkerDozerTaskSlot::Build, building_id);
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Build,
            target_id: building_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: total_build_frames.max(1),
            build_max_health: max_health,
            is_rebuild,
            started_construction: false,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    fn update_dozer_task(&mut self) {
        // Wave 298: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        const MIN_ACTION_TOLERANCE: Real = 70.0;
        let repair_rate = self.get_repair_health_per_second();
        let clear_current = |this: &mut WorkerAIUpdate| {
            if let Some(current) = this.current_task {
                this.clear_task(current);
            }
        };
        if self.object_id == INVALID_ID {
            self.dozer_task = None;
            clear_current(self);
            return;
        }
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        else {
            self.dozer_task = None;
            clear_current(self);
            return;
        };
        let Some(task) = self.dozer_task.as_mut() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(task.target_id) else {
            self.dozer_task = None;
            clear_current(self);
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        let owner_pos = *owner_guard.get_position();
        let owner_pos_local = Coord3D::new(owner_pos.x, owner_pos.y, owner_pos.z);
        let owner_airborne = owner_guard.is_using_airborne_locomotor();
        let owner_ai_update = owner_guard.get_ai_update_interface();

        let target_pos = *target_guard.get_position();
        let target_pos_local = Coord3D::new(target_pos.x, target_pos.y, target_pos.z);
        let target_radius = target_guard
            .get_geometry_info()
            .get_bounding_sphere_radius();
        let target_builder_id = target_guard.get_builder_id();
        let target_is_bridge_tower = target_guard.is_kind_of(KindOf::BridgeTower);
        drop(target_guard);

        // Determine action position if needed.
        if task.dock_point.is_none()
            && self.dozer_action_state == WorkerDozerActionState::PickActionPos
        {
            if let Some(current) = self.current_task {
                let points = &self.dozer_dock_points[current.as_index()];
                if points[0].valid {
                    task.dock_point = Some(points[0].location);
                }
            }
        }

        if task.dock_point.is_none()
            && self.dozer_action_state == WorkerDozerActionState::PickActionPos
        {
            let start_angle = (owner_pos_local.y - target_pos_local.y)
                .atan2(owner_pos_local.x - target_pos_local.x);
            let mut options = FindPositionOptions::default();
            options.min_radius = target_radius;
            options.max_radius = 100.0;
            options.start_angle = Some(start_angle);
            options.source_to_path_to_dest_id = Some(self.object_id);
            if !owner_airborne {
                options.max_z_delta = 10.0;
            } else {
                options.ignore_object_id = Some(task.target_id);
            }

            let mut dock_pos = target_pos;
            if let Some(partition) = ThePartitionManager::get() {
                if partition.find_position_around_with_options(&target_pos, &options, &mut dock_pos)
                {
                    task.dock_point = Some(Coord3D::new(dock_pos.x, dock_pos.y, dock_pos.z));
                }
            }
            if task.dock_point.is_none() {
                task.dock_point = Some(Coord3D::new(dock_pos.x, dock_pos.y, dock_pos.z));
            }
            self.dozer_action_state = WorkerDozerActionState::MoveToActionPos;
        }

        let dock_pos = task.dock_point.unwrap_or(target_pos_local);
        let delta = Coord3D::new(
            owner_pos_local.x - dock_pos.x,
            owner_pos_local.y - dock_pos.y,
            owner_pos_local.z - dock_pos.z,
        );
        let dist_sq = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
        let mut build_complete_rebuild: Option<bool> = None;

        match self.dozer_action_state {
            WorkerDozerActionState::MoveToActionPos => {
                if dist_sq <= MIN_ACTION_TOLERANCE * MIN_ACTION_TOLERANCE {
                    self.dozer_action_state = WorkerDozerActionState::DoAction;
                } else if let Some(ai) = owner_ai_update.as_ref() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let dock_pos_logic = LogicCoord3D::new(dock_pos.x, dock_pos.y, dock_pos.z);
                        if let Err(err) = ai_guard.set_movement_target(&dock_pos_logic) {
                            log::debug!(
                                "WorkerAIUpdate::update_dozer_task set_movement_target failed: {}",
                                err
                            );
                        }
                    }
                }
            }
            WorkerDozerActionState::DoAction => match task.task_type {
                WorkerDozerTaskType::Repair => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let Ok(target_guard) = target.read() else {
                        self.dozer_task = None;
                        return;
                    };
                    if !ActionManager::can_repair_object(
                        &*owner_guard,
                        &*target_guard,
                        CommandSourceType::FromAi,
                    ) {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    if let Some(body) = target_guard.get_body_module() {
                        let max_health = body.get_max_health();
                        let current = body.get_health();
                        if max_health > 0.0 {
                            let delta = max_health * repair_rate * SECONDS_PER_LOGICFRAME_REAL;
                            let new_health = (current + delta).min(max_health);
                            body.set_health(new_health);
                            if new_health >= max_health {
                                if target_is_bridge_tower {
                                    Self::remove_bridge_scaffolding(task.target_id);
                                }
                                self.dozer_task = None;
                                self.clear_task(WorkerDozerTaskSlot::Repair);
                            }
                        }
                    } else {
                        self.dozer_task = None;
                        self.clear_task(WorkerDozerTaskSlot::Repair);
                    }
                }
                WorkerDozerTaskType::ResumeConstruction => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let Ok(target_guard) = target.read() else {
                        self.dozer_task = None;
                        return;
                    };
                    if !ActionManager::can_resume_construction_of(
                        &*owner_guard,
                        &*target_guard,
                        CommandSourceType::FromAi,
                    ) {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let current_percent = target_guard.get_construction_percent() as Real;
                    if current_percent >= 100.0 {
                        self.dozer_task = None;
                        self.clear_task(WorkerDozerTaskSlot::Build);
                        return;
                    }
                    drop(target_guard);
                    let new_percent = (current_percent
                        + repair_rate * 100.0 * SECONDS_PER_LOGICFRAME_REAL)
                        .min(100.0);
                    if let Ok(mut target_write) = target.write() {
                        target_write.set_construction_percent(new_percent);
                    }
                    if new_percent >= 100.0 {
                        build_complete_rebuild = Some(task.is_rebuild);
                    }
                }
                WorkerDozerTaskType::Build => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let construction_manager = get_construction_manager();
                    let mut manager = match construction_manager.write() {
                        Ok(manager) => manager,
                        Err(_) => {
                            self.dozer_task = None;
                            return;
                        }
                    };

                    if !task.started_construction {
                        let max_health = if task.build_max_health > 0.0 {
                            task.build_max_health
                        } else {
                            let Ok(target_guard) = target.read() else {
                                self.dozer_task = None;
                                return;
                            };
                            target_guard
                                .get_body_module()
                                .map(|body| body.get_max_health())
                                .unwrap_or(0.0)
                        };
                        if let Err(err) = manager.start_construction(
                            task.target_id,
                            self.object_id,
                            max_health,
                            task.build_total_frames.max(1),
                            task.is_rebuild,
                        ) {
                            log::debug!(
                                "WorkerAIUpdate::update_dozer_task start_construction failed: {}",
                                err
                            );
                        }
                        task.started_construction = true;
                    }

                    let completed = manager.update_for_dozer(self.object_id);
                    let progress = manager.get_progress(task.target_id).unwrap_or(0.0);
                    let current_health = manager.get_current_health(task.target_id);
                    if let Ok(mut target_write) = target.write() {
                        target_write.set_construction_percent(progress);
                        if let Some(health) = current_health {
                            if let Err(err) = target_write.set_health(health) {
                                log::debug!(
                                    "WorkerAIUpdate::update_dozer_task set_health failed: {}",
                                    err
                                );
                            }
                        }
                    }
                    if completed.contains(&task.target_id) {
                        build_complete_rebuild = Some(task.is_rebuild);
                    }
                }
                WorkerDozerTaskType::Fortify => {
                    // C++ path leaves fortify as a no-op; complete immediately so the worker AI does not stall.
                    self.dozer_task = None;
                    self.clear_task(WorkerDozerTaskSlot::Fortify);
                }
            },
            WorkerDozerActionState::PickActionPos => {}
        }

        if let Some(is_rebuild) = build_complete_rebuild {
            self.handle_build_completion(&owner, &target, is_rebuild);
            self.dozer_task = None;
            self.clear_task(WorkerDozerTaskSlot::Build);
        }
    }

    fn handle_build_completion(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
        is_rebuild: bool,
    ) {
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

            target_guard.handle_partition_cell_maintenance();
            target_guard.update_upgrade_modules_from_player();
            target_guard.on_build_complete();

            let template = target_guard.get_template();
            target_display_name = Some(template.get_name().as_str().to_string());
            target_pos = Some(*target_guard.get_position());
            controlling_player = target_guard.get_controlling_player();
        }

        if let Some(player) = controlling_player {
            if let Ok(mut player_guard) = player.write() {
                let builder_id = owner.read().ok().map(|g| g.get_id());
                let structure_id = target
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                player_guard.on_structure_construction_complete_id(
                    builder_id,
                    structure_id,
                    is_rebuild,
                );
            }
        }

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_locally_controlled() {
                if let Some(display_name) = target_display_name.as_ref() {
                    let format = crate::helpers::TheGameText::fetch("DOZER:ConstructionComplete");
                    let message = if format.contains("%s") {
                        format.replace("%s", display_name)
                    } else {
                        format!("{} {}", format, display_name)
                    };
                    crate::helpers::TheInGameUI::display_message(&message);
                }

                if let Some(voice) = owner_guard
                    .get_template()
                    .get_per_unit_sound("VoiceTaskComplete")
                {
                    if let Some(audio) = TheAudio::get() {
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

    pub fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        // Wave 298: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.number_boxes >= self.data.max_boxes {
            return false;
        }
        self.number_boxes += 1;

        // Play depleted voice if took last box
        if remaining_stock == 0 && !self.data.supplies_depleted_voice.is_empty() {
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
                                let Ok(list) = player_list().read() else {
                                    return None;
                                };
                                list.get_player(player_id as i32).cloned()
                            })
                            .and_then(|player| {
                                player.read().ok().map(|guard| guard.is_skirmish_ai())
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
                    audio.play_voice_event(&self.data.supplies_depleted_voice, self.object_id);
                }
            }
        }

        self.update_drawable_supply_status();
        true
    }

    pub fn get_upgraded_supply_boost(&self) -> u32 {
        if let Some(upgrade) = &self.upgrade_system {
            upgrade.get_supply_boost(self.player_index)
        } else {
            self.data.upgraded_supply_boost
        }
    }

    /// Repair health percent per second (matches C++ WorkerAIUpdate::getRepairHealthPerSecond).
    pub fn get_repair_health_per_second(&self) -> Real {
        self.data.repair_health_percent_per_second
    }

    /// Worker bored time (matches C++ WorkerAIUpdate::getBoredTime).
    pub fn get_bored_time(&self) -> Real {
        self.data.bored_time
    }

    /// Worker bored range (matches C++ WorkerAIUpdate::getBoredRange).
    pub fn get_bored_range(&self) -> Real {
        self.data.bored_range
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
    pub fn get_action_delay_for_dock(&self, is_warehouse: bool) -> u32 {
        if is_warehouse {
            self.data.warehouse_delay
        } else {
            self.data.center_delay
        }
    }

    /// Get warehouse scan distance (AI players get 2x distance).
    pub fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Real {
        if is_ai_player {
            self.data.warehouse_scan_distance * 2.0
        } else {
            self.data.warehouse_scan_distance
        }
    }

    pub fn get_state(&self) -> SupplyTruckState {
        self.state
    }

    pub fn set_state(&mut self, state: SupplyTruckState) {
        self.state = state;
    }
}

