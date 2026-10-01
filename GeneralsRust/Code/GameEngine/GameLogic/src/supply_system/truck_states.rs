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

fn supply_object_present(id: ObjectID) -> Result<(), String> {
    // Wave 298: empty dual-world → not found.
    if dual_world_registry_unavailable() {
        return Err("Supply object unavailable on host-only path".into());
    }
    if crate::object::registry::OBJECT_REGISTRY
        .with_object(id, |_| ())
        .is_none()
    {
        return Err(format!("SupplyTruck object {id} not found"));
    }
    Ok(())
}

fn owner_id_from_state(state: &dyn StateImplementation) -> Option<ObjectID> {
    state
        .get_machine_owner_id()
        .ok()
        .filter(|id| *id != INVALID_ID)
}

/// Run `f` against the owner's AI update module while the owner read lock is
/// held (the AI module is owned by the Object; there is no separate handle).
fn with_owner_ai<R>(
    state: &dyn StateImplementation,
    f: impl FnOnce(&dyn crate::modules::AIUpdateInterface) -> R,
) -> Option<R> {
    let owner_id = owner_id_from_state(state)?;
    if dual_world_registry_unavailable() {
        return None;
    }
    crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |guard| {
        let ai = guard.get_ai_update_interface()?;
        Some(f(ai))
    })?
}

fn with_supply_truck_interface<R>(
    state: &State,
    f: impl FnOnce(&mut dyn SupplyTruckAIInterface) -> R,
) -> Result<R, String> {
    let owner_id = state
        .get_machine_owner_id()
        .ok_or_else(|| "SupplyTruck state missing owner".to_string())?;
    supply_object_present(owner_id)?;
    crate::object::registry::OBJECT_REGISTRY
        .with_object_mut(owner_id, |guard| {
            let ai = guard.get_ai_update_interface_mut()?;
            let truck = ai.get_supply_truck_ai_interface_mut()?;
            Some(f(truck))
        })
        .flatten()
        .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())
}

#[derive(Debug)]
struct SupplyTruckBusyState {
    base: State,
}

    fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "SupplyTruckBusyState"),
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
    fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "SupplyTruckIdleState"),
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
    fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "SupplyTruckWantsToPickUpOrDeliverBoxesState"),
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
        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "SupplyTruck state missing owner".to_string())?;
        supply_object_present(owner_id)?;

        // Phase 1 (owner read): availability and box count.
        let phase = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner_id, |guard| {
                let ai = guard.get_ai_update_interface()?;
                let truck = ai.get_supply_truck_ai_interface()?;
                if !truck.is_available_for_supplying() {
                    return Some(Err(()));
                }
                Some(Ok(truck.get_number_boxes()))
            })
            .flatten()
            .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())?;
        let num_boxes = match phase {
            Err(()) => return Ok(StateReturnType::Failure),
            Ok(n) => n,
        };

        // Phase 2 (no owner lock): the resource service queries the world.
        let dock_target = if num_boxes > 0 {
            resource::find_best_supply_center(owner_id)
        } else {
            resource::find_best_supply_warehouse(owner_id)
        };
        let Some(dock_target) = dock_target else {
            return Ok(StateReturnType::Failure);
        };

        // Phase 3 (owner write): issue the dock command.
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(owner_id, |guard| {
                let ai = guard.get_ai_update_interface_mut()?;
                let mut params =
                    AiCommandParams::new(AiCommandType::Dock, CommandSourceType::FromAi);
                params.obj = Some(dock_target);
                if let Err(err) = ai.execute_command(&params) {
                    log::debug!(
                        "SupplyTruckWantsToPickUpOrDeliverBoxesState::update dock failed: {}",
                        err
                    );
                }
                Some(())
            })
            .flatten()
            .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())?;
        Ok(StateReturnType::Success)
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
    fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "RegroupingState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "SupplyTruck state missing owner".to_string())?;
        supply_object_present(owner_id)?;

        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(owner_id, |owner_guard| {
                let ai = owner_guard.get_ai_update_interface_mut()?;
                if let Err(err) = ai.ignore_obstacle(None) {
                    log::debug!("RegroupingState::on_enter ignore_obstacle failed: {}", err);
                }
                Some(())
            })
            .flatten()
            .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())?;

        let owner_player_id = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner_id, |owner_guard| owner_guard.get_controlling_player_id())
            .flatten()
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

        let destination_id = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner_id, |owner_guard| {
                find_regroup_target(owner_guard, &owner_player_guard)
            })
            .flatten();
        drop(owner_player_guard);
        let Some(destination_id) = destination_id else {
            return Ok(StateReturnType::Failure);
        };

        let near = crate::object::registry::OBJECT_REGISTRY.with_object(destination_id, |destination_guard| {
            crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                let dist_sq = ThePartitionManager::get_distance_squared(
                    owner_guard,
                    destination_guard,
                    crate::common::FROM_BOUNDING_SPHERE_2D,
                );
                if dist_sq < REGROUP_SUCCESS_DISTANCE_SQUARED {
                    return true;
                }
                false
            }).unwrap_or(false)
        }).unwrap_or(false);
        if near {
            return Ok(StateReturnType::Continue);
        }

        let mut destination = LogicCoord3D::ZERO;
        let mut options = FindPositionOptions::default();
        options.min_radius = 0.0;
        options.max_radius = 100.0;

        let can_find_destination = crate::object::registry::OBJECT_REGISTRY
            .with_object(destination_id, |destination_guard| {
                ThePartitionManager::get()
                    .map(|partition| {
                        partition.find_position_around_with_options(
                            destination_guard.get_position(),
                            &options,
                            &mut destination,
                        )
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if !can_find_destination {
            return Ok(StateReturnType::Failure);
        }

        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(owner_id, |owner_guard| {
                let ai = owner_guard.get_ai_update_interface_mut()?;
                let mut params =
                    AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
                params.pos = destination;
                if let Err(err) = ai.execute_command(&params) {
                    log::debug!("RegroupingState::on_enter move command failed: {}", err);
                }
                Some(())
            })
            .flatten()
            .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())?;

        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        let is_idle = with_owner_ai(&self.base, |ai| ai.is_idle()).unwrap_or(false);
        if is_idle {
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
    fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "DockingState"),
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
    machine: StateMachine,
}

impl SupplyTruckStateMachine {
    fn new(owner_id: ObjectID) -> Self {
        let mut machine =
            StateMachine::new_with_owner_id(owner_id, "SupplyTruckStateMachine");


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
            &mut machine,
            ST_BUSY,
            SupplyTruckBusyState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &busy_conditions,
        );

        register_classic_state(
            &mut machine,
            ST_IDLE,
            SupplyTruckIdleState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &idle_conditions,
        );

        register_classic_state(
            &mut machine,
            ST_WANTING,
            SupplyTruckWantsToPickUpOrDeliverBoxesState::new(&machine),
            Some(ST_BUSY),
            Some(ST_REGROUPING),
            &wanting_conditions,
        );

        register_classic_state(
            &mut machine,
            ST_REGROUPING,
            RegroupingState::new(&machine),
            Some(ST_WANTING),
            Some(ST_BUSY),
            &regrouping_conditions,
        );

        register_classic_state(
            &mut machine,
            ST_DOCKING,
            DockingState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &docking_conditions,
        );

        let _ = machine.init_default_state();
        Self { machine }
    }

    fn update(&mut self) -> StateReturnType {
        self.machine.update()
    }

    fn current_state_id(&self) -> Option<u32> {
        self.machine.get_current_state_id()
    }

    fn owner_docking(state: &dyn StateImplementation, _data: &StateTransitionUserData) -> bool {
        with_owner_ai(state, |ai| {
            ai.get_current_command() == Some(AiCommandType::Dock)
        })
        .unwrap_or(false)
    }

    fn owner_idle(state: &dyn StateImplementation, _data: &StateTransitionUserData) -> bool {
        with_owner_ai(state, |ai| ai.is_idle()).unwrap_or(false)
    }

    fn owner_available_for_supplying(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        with_owner_ai(state, |ai| {
            ai.is_idle()
                && ai.get_supply_truck_ai_interface()
                    .map(SupplyTruckAIInterface::is_available_for_supplying)
                    .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    fn owner_not_docking_or_idle(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        with_owner_ai(state, |ai| {
            !ai.is_idle()
                && ai.get_current_command()
                    .map(|cmd| cmd != AiCommandType::Dock)
                    .unwrap_or(true)
        })
        .unwrap_or(false)
    }

    fn is_forced_into_wanting_state(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        with_owner_ai(state, |ai| {
            ai.get_supply_truck_ai_interface()
                .map(SupplyTruckAIInterface::is_forced_into_wanting_state)
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    fn is_forced_into_busy_state(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        with_owner_ai(state, |ai| {
            ai.get_supply_truck_ai_interface()
                .map(SupplyTruckAIInterface::is_forced_into_busy_state)
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    fn owner_player_commanded(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        with_owner_ai(state, |ai| {
            ai.get_last_command_source() == CommandSourceType::FromPlayer
        })
        .unwrap_or(false)
    }
}

fn find_regroup_target(owner: &Object, player: &crate::player::Player) -> Option<ObjectID> {
    let candidates = [
        KindOf::CashGenerator,
        KindOf::CommandCenter,
        KindOf::Structure,
    ];

    for kindof in candidates {
        let mut best: Option<(ObjectID, Real)> = None;
        for object_id in player.get_all_objects() {
            let Some(dist_sq) =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                    if obj_guard.is_destroyed() || !obj_guard.is_kind_of(kindof) {
                        return None;
                    }
                    Some(ThePartitionManager::get_distance_squared(
                        owner,
                        obj_guard,
                        crate::common::FROM_BOUNDING_SPHERE_2D,
                    ))
                })
            else {
                continue;
            };
            let Some(dist_sq) = dist_sq else {
                continue;
            };
            if best
                .as_ref()
                .map_or(true, |(_, best_dist)| dist_sq < *best_dist)
            {
                best = Some((object_id, dist_sq));
            }
        }
        if let Some((id, _)) = best {
            return Some(id);
        }
    }
    None
}

