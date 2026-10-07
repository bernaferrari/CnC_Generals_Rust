// SupplyTruckAIUpdate.cpp:374–827. One owner and live borrowed AI queries.

/// C++ SupplyTruckAIUpdate.h state IDs (not declaration order in the machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SupplyTruckState {
    Idle = 0,
    Busy = 1,
    Wanting = 2,
    Regrouping = 3,
    Docking = 4,
}

const ST_IDLE: u32 = 0;
const ST_BUSY: u32 = 1;
const ST_WANTING: u32 = 2;
const ST_REGROUPING: u32 = 3;
const ST_DOCKING: u32 = 4;
const REGROUP_SUCCESS_DISTANCE_SQUARED: Real = 225.0;

fn resolve_supply_object(id: ObjectID) -> Result<Arc<RwLock<Object>>, String> {
    TheGameLogic::find_object_by_id(id).ok_or_else(|| format!("SupplyTruck object {id} not found"))
}

/// Callback entry adapter only. Driving UnitAI uses update_with_ai instead.
fn supply_owner_ai(owner_id: ObjectID) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
    let owner = resolve_supply_object(owner_id).ok()?;
    let ai = owner.read().ok()?.get_ai_update_interface();
    ai
}

/// Owned state-machine fields. Busy/Idle have empty snapshots; the other three
/// C++ states transfer their payload version without additional fields.
/// No callback stores an AI handle or independently lockable module state.
#[derive(Debug)]
struct SupplyTruckStateMachine {
    owner_id: ObjectID,
    state: Option<SupplyTruckState>,
    sleep_till: u32,
    default_state_id: u32,
    goal_object_id: ObjectID,
    goal_position: LogicCoord3D,
    locked: bool,
    default_state_inited: bool,
}

impl SupplyTruckStateMachine {
    fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            state: None,
            sleep_till: 0,
            default_state_id: ST_BUSY,
            goal_object_id: INVALID_ID,
            goal_position: LogicCoord3D::ZERO,
            locked: false,
            default_state_inited: false,
        }
    }

    fn current_state_id(&self) -> Option<u32> {
        self.state.map(|state| state as u32)
    }

    fn update(
        &mut self,
        truck: &mut dyn SupplyTruckAIInterface,
        ai: &mut dyn AIUpdateInterface,
        available: bool,
    ) -> StateReturnType {
        if !self.default_state_inited {
            self.default_state_inited = true;
            let Some(default) = Self::state_from_id(self.default_state_id) else {
                return StateReturnType::Failure;
            };
            let _ = self.enter(default, truck, ai, available, 0);
        }
        let Some(state) = self.state else {
            return StateReturnType::Failure;
        };
        let now = TheGameLogic::get_frame();
        if self.sleep_till != 0 && now < self.sleep_till {
            return self.check(
                state,
                StateReturnType::Sleep(self.sleep_till.wrapping_sub(now)),
                truck,
                ai,
                available,
                0,
            );
        }
        self.sleep_till = 0;
        let status = match state {
            SupplyTruckState::Wanting => self.seek_dock(truck, ai, available),
            SupplyTruckState::Regrouping if ai.is_idle() => StateReturnType::Success,
            _ => StateReturnType::Continue,
        };
        self.check(state, status, truck, ai, available, 0)
    }

    fn enter(
        &mut self,
        state: SupplyTruckState,
        truck: &mut dyn SupplyTruckAIInterface,
        ai: &mut dyn AIUpdateInterface,
        available: bool,
        depth: usize,
    ) -> StateReturnType {
        // All five outgoing C++ onExit hooks are empty.
        self.sleep_till = 0;
        self.state = Some(state);
        let status = match state {
            SupplyTruckState::Busy => {
                truck.set_force_busy_state(false);
                StateReturnType::Continue
            }
            SupplyTruckState::Wanting | SupplyTruckState::Docking => {
                truck.set_force_wanting_state(false);
                StateReturnType::Continue
            }
            SupplyTruckState::Regrouping => self.enter_regroup(ai),
            SupplyTruckState::Idle => StateReturnType::Continue,
        };
        self.check(state, status, truck, ai, available, depth)
    }

    fn check(
        &mut self,
        state: SupplyTruckState,
        status: StateReturnType,
        truck: &mut dyn SupplyTruckAIInterface,
        ai: &mut dyn AIUpdateInterface,
        available: bool,
        depth: usize,
    ) -> StateReturnType {
        // StateMachine.cpp:94–100 rejects the twentieth recursive check.
        if depth + 1 >= 20 {
            return StateReturnType::Failure;
        }
        let next = if status.is_success() {
            Some(if state == SupplyTruckState::Regrouping {
                SupplyTruckState::Wanting
            } else {
                SupplyTruckState::Busy
            })
        } else if status.is_failure() {
            Some(if state == SupplyTruckState::Wanting {
                SupplyTruckState::Regrouping
            } else {
                SupplyTruckState::Busy
            })
        } else {
            // Match the original ordered condition lists, querying the live AI.
            let docking = |ai: &dyn AIUpdateInterface| {
                ai.get_current_state_id() == Some(crate::ai::states::AIStateType::Dock as u32)
            };
            let not_docking_or_idle = |ai: &dyn AIUpdateInterface| !ai.is_idle() && !docking(ai);
            match state {
                SupplyTruckState::Busy if ai.is_idle() => Some(SupplyTruckState::Idle),
                SupplyTruckState::Busy if docking(ai) => Some(SupplyTruckState::Docking),
                SupplyTruckState::Idle | SupplyTruckState::Docking
                    if truck.is_forced_into_busy_state() =>
                {
                    Some(SupplyTruckState::Busy)
                }
                SupplyTruckState::Idle if truck.is_forced_into_wanting_state() => {
                    Some(SupplyTruckState::Wanting)
                }
                SupplyTruckState::Idle | SupplyTruckState::Wanting if docking(ai) => {
                    Some(SupplyTruckState::Docking)
                }
                SupplyTruckState::Docking if available && ai.is_idle() => {
                    Some(SupplyTruckState::Wanting)
                }
                SupplyTruckState::Idle | SupplyTruckState::Wanting | SupplyTruckState::Docking
                    if not_docking_or_idle(ai) =>
                {
                    Some(SupplyTruckState::Busy)
                }
                SupplyTruckState::Regrouping
                    if ai.get_last_command_source() == CommandSourceType::FromPlayer =>
                {
                    Some(SupplyTruckState::Busy)
                }
                _ => None,
            }
        };
        if let Some(next) = next {
            self.enter(next, truck, ai, available, depth + 1)
        } else {
            status
        }
    }

    fn seek_dock(
        &self,
        truck: &dyn SupplyTruckAIInterface,
        ai: &mut dyn AIUpdateInterface,
        available: bool,
    ) -> StateReturnType {
        if !available {
            return StateReturnType::Failure;
        }
        let dock = if truck.get_number_boxes() > 0 {
            resource::find_best_supply_center(self.owner_id)
        } else {
            resource::find_best_supply_warehouse(self.owner_id)
        };
        let Some(dock) = dock else {
            return StateReturnType::Failure;
        };
        let mut command = AiCommandParams::new(AiCommandType::Dock, CommandSourceType::FromAi);
        command.obj = Some(dock);
        if let Err(err) = ai.execute_command(&command) {
            log::debug!("SupplyTruck dock command failed: {err}");
        }
        StateReturnType::Success
    }

    fn enter_regroup(&self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        let result = (|| -> Result<StateReturnType, String> {
            let owner = resolve_supply_object(self.owner_id)?;
            let player_id = owner
                .read()
                .map_err(|_| "SupplyTruck owner poisoned")?
                .get_controlling_player_id()
                .ok_or("SupplyTruck owner missing player")?;
            // C++ checks the actual controlling player before ignoreObstacle.
            // End this player borrow before the synchronous AI callback.
            crate::player::with_player(player_id as i32, |_| ())
                .ok_or("SupplyTruck player missing")?;
            let _ = ai.ignore_obstacle(None);
            let destination_object = {
                let owner = owner.read().map_err(|_| "SupplyTruck owner poisoned")?;
                crate::player::with_player(player_id as i32, |player| {
                    find_regroup_target(&owner, player)
                })
                .ok_or("SupplyTruck player missing")?
            };
            let Some(target) = destination_object else {
                return Ok(StateReturnType::Failure);
            };
            let position = {
                let owner = owner.read().map_err(|_| "SupplyTruck owner poisoned")?;
                let target = target
                    .read()
                    .map_err(|_| "SupplyTruck regroup target poisoned")?;
                if ThePartitionManager::get_distance_squared(
                    &owner,
                    &target,
                    crate::common::FROM_BOUNDING_SPHERE_2D,
                ) < REGROUP_SUCCESS_DISTANCE_SQUARED
                {
                    return Ok(StateReturnType::Continue);
                }
                *target.get_position()
            };
            let mut destination = LogicCoord3D::ZERO;
            let mut options = FindPositionOptions::default();
            options.max_radius = 100.0;
            if !ThePartitionManager::get().is_some_and(|partition| {
                partition.find_position_around_with_options(&position, &options, &mut destination)
            }) {
                return Ok(StateReturnType::Failure);
            }
            let mut command =
                AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
            command.pos = destination;
            let _ = ai.execute_command(&command);
            Ok(StateReturnType::Continue)
        })();
        result.unwrap_or(StateReturnType::Failure)
    }

    fn state_from_id(id: u32) -> Option<SupplyTruckState> {
        match id {
            ST_IDLE => Some(SupplyTruckState::Idle),
            ST_BUSY => Some(SupplyTruckState::Busy),
            ST_WANTING => Some(SupplyTruckState::Wanting),
            ST_REGROUPING => Some(SupplyTruckState::Regrouping),
            ST_DOCKING => Some(SupplyTruckState::Docking),
            _ => None,
        }
    }

    fn xfer_state_payload(id: u32, xfer: &mut dyn Xfer) -> Result<(), String> {
        if matches!(id, ST_WANTING | ST_REGROUPING | ST_DOCKING) {
            let mut version = 1;
            xfer.xfer_version(&mut version, 1)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// C++ StateMachine.cpp799-868 and SupplyTruckAIUpdate.h44/62/79 wire.
    /// Restore the state reference and payload without executing callbacks.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.sleep_till)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.default_state_id)
            .map_err(|e| e.to_string())?;
        let mut current = self
            .current_state_id()
            .unwrap_or(crate::state_machine::INVALID_STATE_ID);
        xfer.xfer_unsigned_int(&mut current)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
            // Preserve the generic serializer's fallback to the default state
            // before either current-state or debug-all-state payload handling.
            let restored = Self::state_from_id(current)
                .or_else(|| Self::state_from_id(self.default_state_id))
                .ok_or_else(|| "SupplyTruck current/default state missing".to_string())?;
            self.state = Some(restored);
        }
        let mut snapshot_all = false;
        xfer.xfer_bool(&mut snapshot_all)
            .map_err(|e| e.to_string())?;
        if snapshot_all {
            let mut count = 5i32;
            xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
            if count != 5 {
                return Err(format!(
                    "SupplyTruck state count mismatch: expected 5, read {count}"
                ));
            }
            // C++ std::map visits numeric IDs, rather than definition order.
            for expected in [ST_IDLE, ST_BUSY, ST_WANTING, ST_REGROUPING, ST_DOCKING] {
                let mut saved = expected;
                xfer.xfer_unsigned_int(&mut saved)
                    .map_err(|e| e.to_string())?;
                if saved != expected {
                    return Err(format!(
                        "SupplyTruck state ID mismatch: expected {expected}, read {saved}"
                    ));
                }
                Self::xfer_state_payload(expected, xfer)?;
            }
        } else {
            if self.state.is_none() {
                self.state = Self::state_from_id(self.default_state_id);
            }
            let state = self
                .state
                .ok_or_else(|| "SupplyTruck current/default state missing".to_string())?;
            Self::xfer_state_payload(state as u32, xfer)?;
        }
        xfer.xfer_object_id(&mut self.goal_object_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.goal_position.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.goal_position.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.goal_position.z)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.locked)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.default_state_inited)
            .map_err(|e| e.to_string())?;
        Ok(())
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
