// ============================================================================
// SUPPLY TRUCK AI
// ============================================================================

/// Supply truck AI state
/// Matches C++ SupplyTruckAIUpdate states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyTruckState {
    /// Not doing anything, should autopilot?
    Idle,
    /// Direct player involvement, off autopilot
    Busy,
    /// Search for warehouse or center and dock with it
    Wanting,
    /// Wanting failed, hang out at base until something changes
    Regrouping,
    /// Docking substates running
    Docking,
}

const ST_IDLE: u32 = 0;
const ST_BUSY: u32 = 1;
const ST_WANTING: u32 = 2;
const ST_REGROUPING: u32 = 3;
const ST_DOCKING: u32 = 4;

const REGROUP_SUCCESS_DISTANCE_SQUARED: Real = 225.0;

fn resolve_supply_object(id: ObjectID) -> Result<Arc<RwLock<Object>>, String> {
    // Wave 298: empty dual-world → not found.
    if dual_world_registry_unavailable() {
        return Err("Supply object unavailable on host-only path".into());
    }

    crate::helpers::TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        .ok_or_else(|| format!("SupplyTruck object {id} not found"))
}

fn owner_id_from_state(state: &dyn StateImplementation) -> Option<ObjectID> {
    state
        .get_machine_owner_id()
        .ok()
        .filter(|id| *id != INVALID_ID)
}

fn owner_from_state(state: &dyn StateImplementation) -> Option<Arc<RwLock<Object>>> {
    let owner_id = owner_id_from_state(state)?;
    resolve_supply_object(owner_id).ok()
}

fn owner_ai_from_state(
    state: &dyn StateImplementation,
) -> Option<Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>>> {
    let owner_id = owner_id_from_state(state)?;
    let owner = resolve_supply_object(owner_id).ok()?;
    owner
        .read()
        .ok()
        .and_then(|guard| guard.get_ai_update_interface())
}
fn owner_ai_and_truck(
    state: &State,
) -> Result<(ObjectID, Arc<Mutex<dyn AIUpdateInterface>>), String> {
    let owner_id = state
        .get_machine_owner_id()
        .ok_or_else(|| "SupplyTruck state missing owner".to_string())?;
    let owner = resolve_supply_object(owner_id)?;
    let ai = owner
        .read()
        .map_err(|_| "SupplyTruck owner lock poisoned".to_string())?
        .get_ai_update_interface()
        .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())?;
    Ok((owner_id, ai))
}

fn with_supply_truck_interface<R>(
    state: &State,
    f: impl FnOnce(&mut dyn SupplyTruckAIInterface) -> R,
) -> Result<R, String> {
    let (_owner_id, ai) = owner_ai_and_truck(state)?;
    let mut ai_guard = ai
        .lock()
        .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
    let truck = ai_guard
        .get_supply_truck_ai_interface_mut()
        .ok_or_else(|| "SupplyTruck AI interface missing".to_string())?;
    Ok(f(truck))
}

#[derive(Debug)]
struct SupplyTruckBusyState {
    base: State,
}

impl SupplyTruckBusyState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "SupplyTruckBusyState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Err(err) = with_supply_truck_interface(&self.base, |truck| {
            truck.set_force_busy_state(false);
        }) {
            log::debug!("SupplyTruckBusyState::on_enter: {}", err);
        }
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for SupplyTruckBusyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct SupplyTruckIdleState {
    base: State,
}

impl SupplyTruckIdleState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "SupplyTruckIdleState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for SupplyTruckIdleState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct SupplyTruckWantsToPickUpOrDeliverBoxesState {
    base: State,
}

impl SupplyTruckWantsToPickUpOrDeliverBoxesState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(
                Some(Arc::downgrade(machine)),
                "SupplyTruckWantsToPickUpOrDeliverBoxesState",
            ),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Err(err) = with_supply_truck_interface(&self.base, |truck| {
            truck.set_force_wanting_state(false);
        }) {
            log::debug!(
                "SupplyTruckWantsToPickUpOrDeliverBoxesState::on_enter: {}",
                err
            );
        }
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        let (owner_id, ai) = owner_ai_and_truck(&self.base)?;

        let mut ai_guard = ai
            .lock()
            .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
        let truck = ai_guard
            .get_supply_truck_ai_interface_mut()
            .ok_or_else(|| "SupplyTruck AI interface missing".to_string())?;

        if !truck.is_available_for_supplying() {
            return Ok(StateReturnType::Failure);
        }

        let num_boxes = truck.get_number_boxes();
        if num_boxes > 0 {
            if let Some(best_center) = resource::find_best_supply_center(owner_id) {
                let mut params =
                    AiCommandParams::new(AiCommandType::Dock, CommandSourceType::FromAi);
                params.obj = Some(best_center);
                if let Err(err) = ai_guard.execute_command(&params) {
                    log::debug!(
                        "SupplyTruckWantsToPickUpOrDeliverBoxesState::update dock(center) failed: {}",
                        err
                    );
                }
                return Ok(StateReturnType::Success);
            }
        } else if let Some(best_warehouse) = resource::find_best_supply_warehouse(owner_id) {
            let mut params = AiCommandParams::new(AiCommandType::Dock, CommandSourceType::FromAi);
            params.obj = Some(best_warehouse);
            if let Err(err) = ai_guard.execute_command(&params) {
                log::debug!(
                    "SupplyTruckWantsToPickUpOrDeliverBoxesState::update dock(warehouse) failed: {}",
                    err
                );
            }
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for SupplyTruckWantsToPickUpOrDeliverBoxesState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct RegroupingState {
    base: State,
}

impl RegroupingState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "RegroupingState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        let (owner_id, ai) = owner_ai_and_truck(&self.base)?;
        let owner_arc = resolve_supply_object(owner_id)?;

        {
            let mut ai_guard = ai
                .lock()
                .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
            if let Err(err) = ai_guard.ignore_obstacle(None) {
                log::debug!("RegroupingState::on_enter ignore_obstacle failed: {}", err);
            }
        }

        let owner_guard = owner_arc
            .read()
            .map_err(|_| "SupplyTruck owner lock poisoned".to_string())?;
        let owner_player_id = owner_guard
            .get_controlling_player_id()
            .ok_or_else(|| "SupplyTruck owner missing player".to_string())?;
        let owner_player = {
            let list_guard = player_list()
                .read()
                .map_err(|_| "Player list lock poisoned".to_string())?;
            list_guard
                .get_player(owner_player_id as i32)
                .cloned()
                .ok_or_else(|| "SupplyTruck owner player missing".to_string())?
        };
        let owner_player_guard = owner_player
            .read()
            .map_err(|_| "Player lock poisoned".to_string())?;

        let destination_object = find_regroup_target(&owner_guard, &owner_player_guard);
        let Some(destination_object) = destination_object else {
            return Ok(StateReturnType::Failure);
        };

        let destination_guard = destination_object
            .read()
            .map_err(|_| "Regroup target lock poisoned".to_string())?;
        let dist_sq = ThePartitionManager::get_distance_squared(
            &owner_guard,
            &destination_guard,
            crate::common::FROM_BOUNDING_SPHERE_2D,
        );
        if dist_sq < REGROUP_SUCCESS_DISTANCE_SQUARED {
            return Ok(StateReturnType::Continue);
        }

        let mut destination = LogicCoord3D::ZERO;
        let mut options = FindPositionOptions::default();
        options.min_radius = 0.0;
        options.max_radius = 100.0;

        let can_find_destination = ThePartitionManager::get()
            .map(|partition| {
                partition.find_position_around_with_options(
                    destination_guard.get_position(),
                    &options,
                    &mut destination,
                )
            })
            .unwrap_or(false);
        if !can_find_destination {
            return Ok(StateReturnType::Failure);
        }

        let mut ai_guard = ai
            .lock()
            .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
        let mut params =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        params.pos = destination;
        if let Err(err) = ai_guard.execute_command(&params) {
            log::debug!("RegroupingState::on_enter move command failed: {}", err);
        }

        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        let (_owner_id, ai) = owner_ai_and_truck(&self.base)?;
        let ai_guard = ai
            .lock()
            .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;

        if ai_guard.is_idle() {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for RegroupingState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct DockingState {
    base: State,
}

impl DockingState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "DockingState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Err(err) = with_supply_truck_interface(&self.base, |truck| {
            truck.set_force_wanting_state(false);
        }) {
            log::debug!("DockingState::on_enter: {}", err);
        }
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for DockingState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct SupplyTruckStateMachine {
    machine: Arc<Mutex<StateMachine>>,
}

impl SupplyTruckStateMachine {
    fn new(owner_id: ObjectID) -> Self {
        let machine = Arc::new(Mutex::new(StateMachine::new_with_owner_id(
            owner_id,
            "SupplyTruckStateMachine",
        )));
        let mut guard = machine
            .lock()
            .expect("SupplyTruckStateMachine lock poisoned");

        let busy_conditions = vec![
            StateConditionInfo::new(
                Self::owner_idle,
                ST_IDLE,
                StateTransitionUserData::new(),
                "owner_idle",
            ),
            StateConditionInfo::new(
                Self::owner_docking,
                ST_DOCKING,
                StateTransitionUserData::new(),
                "owner_docking",
            ),
        ];

        let idle_conditions = vec![
            StateConditionInfo::new(
                Self::is_forced_into_busy_state,
                ST_BUSY,
                StateTransitionUserData::new(),
                "forced_busy",
            ),
            StateConditionInfo::new(
                Self::is_forced_into_wanting_state,
                ST_WANTING,
                StateTransitionUserData::new(),
                "forced_wanting",
            ),
            StateConditionInfo::new(
                Self::owner_docking,
                ST_DOCKING,
                StateTransitionUserData::new(),
                "owner_docking",
            ),
            StateConditionInfo::new(
                Self::owner_not_docking_or_idle,
                ST_BUSY,
                StateTransitionUserData::new(),
                "owner_not_docking_or_idle",
            ),
        ];

        let wanting_conditions = vec![
            StateConditionInfo::new(
                Self::owner_docking,
                ST_DOCKING,
                StateTransitionUserData::new(),
                "owner_docking",
            ),
            StateConditionInfo::new(
                Self::owner_not_docking_or_idle,
                ST_BUSY,
                StateTransitionUserData::new(),
                "owner_not_docking_or_idle",
            ),
        ];

        let regrouping_conditions = vec![StateConditionInfo::new(
            Self::owner_player_commanded,
            ST_BUSY,
            StateTransitionUserData::new(),
            "owner_player_commanded",
        )];

        let docking_conditions = vec![
            StateConditionInfo::new(
                Self::is_forced_into_busy_state,
                ST_BUSY,
                StateTransitionUserData::new(),
                "forced_busy",
            ),
            StateConditionInfo::new(
                Self::owner_available_for_supplying,
                ST_WANTING,
                StateTransitionUserData::new(),
                "owner_available_for_supplying",
            ),
            StateConditionInfo::new(
                Self::owner_not_docking_or_idle,
                ST_BUSY,
                StateTransitionUserData::new(),
                "owner_not_docking_or_idle",
            ),
        ];

        register_classic_state(
            &mut guard,
            ST_BUSY,
            SupplyTruckBusyState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &busy_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_IDLE,
            SupplyTruckIdleState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &idle_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_WANTING,
            SupplyTruckWantsToPickUpOrDeliverBoxesState::new(&machine),
            Some(ST_BUSY),
            Some(ST_REGROUPING),
            &wanting_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_REGROUPING,
            RegroupingState::new(&machine),
            Some(ST_WANTING),
            Some(ST_BUSY),
            &regrouping_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_DOCKING,
            DockingState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &docking_conditions,
        );

        let _ = guard.init_default_state();
        drop(guard);
        Self { machine }
    }

    fn update(&mut self) -> StateReturnType {
        self.machine
            .lock()
            .map(|mut guard| guard.update())
            .unwrap_or(StateReturnType::Failure)
    }

    fn current_state_id(&self) -> Option<u32> {
        self.machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_current_state_id())
    }

    fn owner_docking(state: &dyn StateImplementation, _data: &StateTransitionUserData) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        ai.lock()
            .ok()
            .and_then(|guard| guard.get_current_command())
            .map(|cmd| cmd == AiCommandType::Dock)
            .unwrap_or(false)
    }

    fn owner_idle(state: &dyn StateImplementation, _data: &StateTransitionUserData) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        ai.lock().ok().map_or(false, |guard| guard.is_idle())
    }

    fn owner_available_for_supplying(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if !ai_guard.is_idle() {
            return false;
        }
        let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() else {
            return false;
        };
        truck.is_available_for_supplying()
    }

    fn owner_not_docking_or_idle(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        let ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if ai_guard.is_idle() {
            return false;
        }
        ai_guard
            .get_current_command()
            .map(|cmd| cmd != AiCommandType::Dock)
            .unwrap_or(true)
    }

    fn is_forced_into_wanting_state(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() else {
            return false;
        };
        truck.is_forced_into_wanting_state()
    }

    fn is_forced_into_busy_state(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() else {
            return false;
        };
        truck.is_forced_into_busy_state()
    }

    fn owner_player_commanded(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let ai = match owner_ai_from_state(state) {
            Some(ai) => ai,
            None => return false,
        };
        ai.lock()
            .ok()
            .map(|guard| guard.get_last_command_source() == CommandSourceType::FromPlayer)
            .unwrap_or(false)
    }
}

fn find_regroup_target(
    owner: &Object,
    player: &crate::player::Player,
) -> Option<Arc<RwLock<Object>>> {
    let candidates = [
        KindOf::CashGenerator,
        KindOf::CommandCenter,
        KindOf::Structure,
    ];

    for kindof in candidates {
        let mut best: Option<(Arc<RwLock<Object>>, Real)> = None;
        for object_id in player.get_all_objects() {
            let Some(obj) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if obj_guard.is_destroyed() || !obj_guard.is_kind_of(kindof) {
                continue;
            }
            let dist_sq = ThePartitionManager::get_distance_squared(
                owner,
                &obj_guard,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            );
            if best
                .as_ref()
                .map_or(true, |(_, best_dist)| dist_sq < *best_dist)
            {
                best = Some((obj.clone(), dist_sq));
            }
        }
        if let Some((obj, _)) = best {
            return Some(obj);
        }
    }
    None
}

