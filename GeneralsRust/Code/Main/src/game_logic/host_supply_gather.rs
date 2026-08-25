//! Host supply-truck / warehouse / center helpers.
//!
//! C++: `SupplyTruckAIUpdate`, `SupplyWarehouseDockUpdate`, `SupplyCenterDockUpdate`.

use crate::game_logic::{DockKind, ObjectId};
use glam::Vec3;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

fn live_dock_queues() -> &'static Mutex<HashMap<ObjectId, HostDockApproachQueue>> {
    static QUEUES: OnceLock<Mutex<HashMap<ObjectId, HostDockApproachQueue>>> = OnceLock::new();
    QUEUES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// C++ `DockUpdate::reserveApproachPosition` + `update` promote first arriver.
/// Returns `PathTo` until the docker reaches its `DockWaiting` / boneless slot.
pub fn tick_live_dock_approach(
    dock_id: ObjectId,
    docker_id: ObjectId,
    number_approach_positions: i32,
    docker_alive: bool,
    current_active: Option<ObjectId>,
    current_active_alive: bool,
    docker_pos: Vec3,
    dock_pos: Vec3,
    dock_major_radius: f32,
    waiting_bones: &[Vec3],
    current_frame: u32,
    is_alive: impl FnMut(ObjectId) -> bool,
) -> DockApproachTick {
    tick_live_dock_approach_ex(
        dock_id,
        docker_id,
        number_approach_positions,
        docker_alive,
        current_active,
        current_active_alive,
        docker_pos,
        dock_pos,
        dock_major_radius,
        waiting_bones,
        current_frame,
        false,
        is_alive,
    )
}

/// Same as [`tick_live_dock_approach`], with C++ `m_dockCrippled`.
/// C++ `DockUpdate::update` never assigns `m_activeDocker` while crippled.
pub fn tick_live_dock_approach_ex(
    dock_id: ObjectId,
    docker_id: ObjectId,
    number_approach_positions: i32,
    docker_alive: bool,
    current_active: Option<ObjectId>,
    current_active_alive: bool,
    docker_pos: Vec3,
    dock_pos: Vec3,
    dock_major_radius: f32,
    waiting_bones: &[Vec3],
    current_frame: u32,
    crippled: bool,
    is_alive: impl FnMut(ObjectId) -> bool,
) -> DockApproachTick {
    let Ok(mut map) = live_dock_queues().lock() else {
        return DockApproachTick::Blocked;
    };
    let queue = map
        .entry(dock_id)
        .or_insert_with(|| HostDockApproachQueue::new(number_approach_positions));
    queue.evict_dead(is_alive);
    if !waiting_bones.is_empty() {
        queue.set_waiting_bones(waiting_bones.to_vec());
    }
    if !docker_alive {
        queue.cancel_docker(docker_id);
        return DockApproachTick::Blocked;
    }
    if current_active == Some(docker_id) && current_active_alive && !crippled {
        queue.on_enter_reached(docker_id);
        queue.clear_wait_started(docker_id);
        return DockApproachTick::ClearToAct;
    }
    if !queue.is_clear_to_approach(docker_id) {
        return DockApproachTick::Blocked;
    }
    let Some(slot) = queue.reserve_approach_position(docker_id) else {
        return DockApproachTick::Blocked;
    };
    let goal =
        queue.approach_world_position(slot as usize, docker_pos, dock_pos, dock_major_radius);
    if docker_pos.distance(goal) > DOCK_APPROACH_ARRIVAL_SLOP {
        queue.clear_wait_started(docker_id);
        return DockApproachTick::PathTo(goal);
    }
    queue.on_approach_reached(docker_id);
    if queue.promote_active(current_active, current_active_alive, crippled) == Some(docker_id) {
        queue.on_enter_reached(docker_id);
        queue.clear_wait_started(docker_id);
        DockApproachTick::ClearToAct
    } else if let Some(index) = queue.index_of(docker_id) {
        // C++ AIDockAdvancePositionState: scoot into a freed closer slot.
        if queue.is_clear_to_advance(docker_id, index) {
            if let Some(new_index) = queue.advance_approach_position(docker_id, index) {
                queue.clear_wait_started(docker_id);
                let goal = queue.approach_world_position(
                    new_index as usize,
                    docker_pos,
                    dock_pos,
                    dock_major_radius,
                );
                return DockApproachTick::PathTo(goal);
            }
        }
        if queue.wait_for_clearance_timed_out(docker_id, current_frame) {
            queue.cancel_docker(docker_id);
            DockApproachTick::TimedOut
        } else {
            DockApproachTick::Blocked
        }
    } else if queue.wait_for_clearance_timed_out(docker_id, current_frame) {
        queue.cancel_docker(docker_id);
        DockApproachTick::TimedOut
    } else {
        DockApproachTick::Blocked
    }
}

pub fn cancel_live_dock_approach(dock_id: ObjectId, docker_id: ObjectId) {
    if let Ok(mut map) = live_dock_queues().lock() {
        if let Some(queue) = map.get_mut(&dock_id) {
            queue.cancel_docker(docker_id);
        }
    }
}

/// C++ `DockUpdate::cancelDock` for every live queue this docker reserved.
pub fn cancel_all_live_dock_reservations_for(docker_id: ObjectId) {
    if let Ok(mut map) = live_dock_queues().lock() {
        for queue in map.values_mut() {
            queue.cancel_docker(docker_id);
        }
    }
}

/// Drop live approach queues so tests do not leak ObjectId state.
pub fn reset_live_dock_queues() {
    if let Ok(mut map) = live_dock_queues().lock() {
        map.clear();
    }
}

/// C++ `DockUpdate::xfer` approach-slot vectors for snapshot persist.
pub fn snapshot_live_dock_queues() -> Vec<(ObjectId, HostDockApproachQueue)> {
    let Ok(map) = live_dock_queues().lock() else {
        return Vec::new();
    };
    let mut entries: Vec<(ObjectId, HostDockApproachQueue)> =
        map.iter().map(|(id, queue)| (*id, queue.clone())).collect();
    entries.sort_by_key(|(id, _)| id.0);
    entries
}

/// Replace process-global queues so a load cannot leak the previous session.
pub fn restore_live_dock_queues(entries: Vec<(ObjectId, HostDockApproachQueue)>) {
    reset_live_dock_queues();
    if let Ok(mut map) = live_dock_queues().lock() {
        for (dock_id, queue) in entries {
            map.insert(dock_id, queue);
        }
    }
}

/// C++ AI_DOCK session states that own an approach reservation.
pub fn is_live_dock_ai_state(state: &crate::game_logic::AIState) -> bool {
    matches!(
        state,
        crate::game_logic::AIState::Gathering
            | crate::game_logic::AIState::ReturningResources
            | crate::game_logic::AIState::SeekingRepair
            | crate::game_logic::AIState::SeekingHealing
            | crate::game_logic::AIState::Docking
            | crate::game_logic::AIState::Docked
    )
}

/// Alias used by `Object::set_ai_state` / death cancel.
pub fn cancel_live_dock_for_docker(docker_id: ObjectId) {
    cancel_all_live_dock_reservations_for(docker_id);
}

/// C++ `DockUpdate::isClearToApproach` against the live approach-queue.
/// A dock that has never been reserved is clear (every slot still free).
pub fn live_dock_is_clear_to_approach(dock_id: ObjectId, docker_id: ObjectId) -> bool {
    let Ok(map) = live_dock_queues().lock() else {
        return false;
    };
    map.get(&dock_id)
        .map(|queue| queue.is_clear_to_approach(docker_id))
        .unwrap_or(true)
}

/// C++ `DYNAMIC_APPROACH_VECTOR_FLAG` (`DockUpdate.h:24`).
pub const DYNAMIC_APPROACH_VECTOR_FLAG: i32 = -1;
/// C++ `DEFAULT_APPROACH_VECTOR_SIZE` (`DockUpdate.h:19`).
pub const DEFAULT_APPROACH_VECTOR_SIZE: usize = 10;
/// Host arrival slop at a reserved approach point (`PATHFIND_CELL_SIZE_F`).
pub const DOCK_APPROACH_ARRIVAL_SLOP: f32 = PATHFIND_CELL_SIZE_F;
/// C++ `AIDockWaitForClearanceState` timeout: `30 * LOGICFRAMES_PER_SECOND`.
pub const WAIT_FOR_CLEARANCE_FRAMES: u32 = 900;

/// C++ `DockUpdate` approach-slot vectors (`m_approachPositionOwners` / `Reached`).
#[derive(Debug, Clone)]
pub struct HostDockApproachQueue {
    /// C++ `m_numberApproachPositions` (`-1` = dynamic / boneless infinite).
    pub number_approach_positions: i32,
    /// C++ `m_numberApproachPositionBones` (`0` = boneless bias).
    pub number_approach_position_bones: i32,
    /// Local `DockWaiting` bone positions, index 0 = first queue slot.
    pub waiting_bones: Vec<Vec3>,
    pub owners: Vec<Option<ObjectId>>,
    pub reached: Vec<bool>,
    /// C++ `AIDockWaitForClearanceState::m_enterFrame` per reserved docker.
    pub wait_started: HashMap<ObjectId, u32>,
}

impl HostDockApproachQueue {
    pub fn new(number_approach_positions: i32) -> Self {
        let len = if number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            DEFAULT_APPROACH_VECTOR_SIZE
        } else {
            number_approach_positions.max(0) as usize
        };
        Self {
            number_approach_positions,
            number_approach_position_bones: 0,
            waiting_bones: Vec::new(),
            owners: vec![None; len],
            reached: vec![false; len],
            wait_started: HashMap::new(),
        }
    }

    /// Install pristine `DockWaiting` bones. Empty keeps the boneless bias path.
    pub fn set_waiting_bones(&mut self, bones: Vec<Vec3>) {
        self.waiting_bones = bones;
        self.number_approach_position_bones = self.waiting_bones.len() as i32;
    }

    pub fn evict_dead(&mut self, mut is_alive: impl FnMut(ObjectId) -> bool) {
        for (owner, reached) in self.owners.iter_mut().zip(self.reached.iter_mut()) {
            if owner.is_some_and(|id| !is_alive(id)) {
                if let Some(id) = *owner {
                    self.wait_started.remove(&id);
                }
                *owner = None;
                *reached = false;
            }
        }
    }

    /// C++ `DockUpdate::isClearToApproach`.
    pub fn is_clear_to_approach(&self, docker: ObjectId) -> bool {
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            return true;
        }
        self.owners
            .iter()
            .any(|owner| owner.is_none() || *owner == Some(docker))
    }

    /// C++ `DockUpdate::reserveApproachPosition` — returns the reserved index.
    pub fn reserve_approach_position(&mut self, docker: ObjectId) -> Option<i32> {
        for (index, owner) in self.owners.iter().enumerate() {
            if *owner == Some(docker) {
                return Some(index as i32);
            }
            if owner.is_none() {
                self.owners[index] = Some(docker);
                self.reached[index] = false;
                return Some(index as i32);
            }
        }
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            self.owners.push(Some(docker));
            self.reached.push(false);
            self.waiting_bones.push(Vec3::ZERO);
            return Some((self.owners.len() - 1) as i32);
        }
        None
    }

    /// C++ `DockUpdate::advanceApproachPosition`.
    pub fn advance_approach_position(&mut self, docker: ObjectId, index: i32) -> Option<i32> {
        if index <= 0 {
            return None;
        }
        let his = index as usize;
        if self.owners.get(his) != Some(&Some(docker)) {
            return None;
        }
        if self.owners.get(his - 1) != Some(&None) {
            return None;
        }
        self.owners[his - 1] = Some(docker);
        self.reached[his - 1] = false;
        self.owners[his] = None;
        self.reached[his] = false;
        Some((his - 1) as i32)
    }

    /// C++ `DockUpdate::isClearToAdvance`.
    pub fn is_clear_to_advance(&self, docker: ObjectId, index: i32) -> bool {
        if index <= 0 {
            return false;
        }
        let i = index as usize;
        let correct = self.owners.get(i) == Some(&Some(docker));
        let reached = self.reached.get(i).copied().unwrap_or(false);
        let next_free = self.owners.get(i - 1) == Some(&None);
        correct && reached && next_free
    }

    /// C++ `DockUpdate::onApproachReached`.
    pub fn on_approach_reached(&mut self, docker: ObjectId) {
        if let Some(index) = self.index_of(docker) {
            if let Some(reached) = self.reached.get_mut(index as usize) {
                *reached = true;
            }
        }
    }

    /// C++ `onEnterReached` — free the approach slot so the line can advance.
    pub fn on_enter_reached(&mut self, docker: ObjectId) {
        if let Some(index) = self.index_of(docker) {
            let i = index as usize;
            if let Some(owner) = self.owners.get_mut(i) {
                *owner = None;
            }
            if let Some(reached) = self.reached.get_mut(i) {
                *reached = false;
            }
        }
    }

    /// C++ `DockUpdate::cancelDock` slot half.
    pub fn cancel_docker(&mut self, docker: ObjectId) {
        for (owner, reached) in self.owners.iter_mut().zip(self.reached.iter_mut()) {
            if *owner == Some(docker) {
                *owner = None;
                *reached = false;
            }
        }
        self.wait_started.remove(&docker);
    }

    pub fn clear_wait_started(&mut self, docker: ObjectId) {
        self.wait_started.remove(&docker);
    }

    /// C++ `AIDockWaitForClearanceState::update` 30s deadline.
    pub fn wait_for_clearance_timed_out(&mut self, docker: ObjectId, current_frame: u32) -> bool {
        let start = *self.wait_started.entry(docker).or_insert(current_frame);
        current_frame.saturating_sub(start) >= WAIT_FOR_CLEARANCE_FRAMES
    }

    /// C++ `DockUpdate::computeApproachPosition`.
    pub fn approach_world_position(
        &self,
        slot: usize,
        docker_pos: Vec3,
        dock_pos: Vec3,
        dock_major_radius: f32,
    ) -> Vec3 {
        if self.number_approach_position_bones > 0 {
            if let Some(&bone) = self.waiting_bones.get(slot) {
                return bone;
            }
        }
        let mut offset = docker_pos - dock_pos;
        offset.y = 0.0;
        if offset.length_squared() > 0.0001 {
            offset = offset.normalize() * (dock_major_radius.max(0.0) * 0.5);
        }
        dock_pos + offset
    }

    pub fn index_of(&self, docker: ObjectId) -> Option<i32> {
        self.owners
            .iter()
            .position(|owner| *owner == Some(docker))
            .map(|i| i as i32)
    }

    /// C++ `DockUpdate::update` — first arrived docker becomes `m_activeDocker`.
    pub fn promote_active(
        &self,
        current_active: Option<ObjectId>,
        current_active_alive: bool,
        crippled: bool,
    ) -> Option<ObjectId> {
        if current_active.is_some() && current_active_alive {
            return current_active;
        }
        if crippled {
            return None;
        }
        for (reached, owner) in self.reached.iter().zip(self.owners.iter()) {
            if *reached {
                return *owner;
            }
        }
        None
    }
}

/// Retail `NumberApproachPositions` for the live dock kinds the host owns.
/// Unknown / unauthored docks stay `0` (fail-closed: reserve refuses).
pub fn number_approach_positions_for_dock(
    template_name: &str,
    dock_kind: DockKind,
    is_repair_dock: bool,
    delete_when_empty: bool,
) -> i32 {
    let name = template_name.to_ascii_lowercase();
    if is_repair_dock {
        return super::host_repair::REPAIR_DOCK_NUMBER_APPROACH_POSITIONS as i32;
    }
    if name.contains("supplypile") {
        return 5;
    }
    if name.contains("supplydock") {
        return DYNAMIC_APPROACH_VECTOR_FLAG;
    }
    match dock_kind {
        DockKind::SupplyWarehouse => {
            if delete_when_empty {
                5
            } else {
                9
            }
        }
        DockKind::SupplyCenter => {
            if name.contains("china") || name.contains("gla") {
                DYNAMIC_APPROACH_VECTOR_FLAG
            } else {
                9
            }
        }
        DockKind::RailedTransport => 9,
        DockKind::None => 0,
    }
}

/// Result of one live `AIDock` tick against a host dock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockApproachTick {
    /// Queue full, or waiting for the docker ahead. Do not act at the center.
    Blocked,
    /// Path to this reserved approach / enter point.
    PathTo(Vec3),
    /// C++ `isClearToEnter` — this docker is `m_activeDocker` and may act.
    ClearToAct,
    /// C++ `AIDockWaitForClearanceState` 30s failure — reservation cancelled.
    TimedOut,
}

/// C++ `PATHFIND_CELL_SIZE_F` (`AIPathfind.h:416`).
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// C++ twitch range: `0.4 * PATHFIND_CELL_SIZE_F`.
pub const WAREHOUSE_TWITCH_RANGE: f32 = 0.4 * PATHFIND_CELL_SIZE_F;
/// C++ `REGROUP_SUCCESS_DISTANCE_SQUARED` (`SupplyTruckAIUpdate.cpp:42`).
pub const REGROUP_SUCCESS_DISTANCE_SQUARED: f32 = 225.0;
/// C++ `FindPositionOptions.maxRadius` for regroup (`SupplyTruckAIUpdate.cpp:588`).
pub const REGROUP_FIND_POSITION_RADIUS: f32 = 100.0;
/// C++ `GUI:AddCash` key (`SupplyCenterDockUpdate.cpp:129`).
pub const SUPPLY_CENTER_ADD_CASH_KEY: &str = "GUI:AddCash";
/// C++ `GameMakeColor(0,0,0,230)` OR'd onto player color.
pub const SUPPLY_CENTER_FLOATING_TEXT_ALPHA: u8 = 230;

/// C++ `SupplyTruckAIUpdate::getWarehouseScanDistance` — AI players get 2×.
pub fn warehouse_scan_distance(authored: f32, is_computer: bool) -> f32 {
    if authored <= 0.0 {
        return 0.0;
    }
    if is_computer {
        authored * 2.0
    } else {
        authored
    }
}

/// C++ `SupplyWarehouseDockUpdate::action` close-enough
/// (`SupplyWarehouseDockUpdate.cpp:74-86`):
/// `curDistSqr = FROM_BOUNDINGSPHERE_2D` vs `sqr(docker_r*2)`.
/// `distCalcProc_BoundaryAndBoundary_2D` is the surface gap
/// `max(0, centerDist - r_docker - r_warehouse)`.
pub fn warehouse_too_far_2d(
    docker_xz: (f32, f32),
    warehouse_xz: (f32, f32),
    docker_bounding_circle_radius: f32,
    warehouse_bounding_circle_radius: f32,
) -> bool {
    let close = docker_bounding_circle_radius * 2.0;
    let dx = docker_xz.0 - warehouse_xz.0;
    let dz = docker_xz.1 - warehouse_xz.1;
    let center = (dx * dx + dz * dz).sqrt();
    let gap = (center
        - docker_bounding_circle_radius.max(0.0)
        - warehouse_bounding_circle_radius.max(0.0))
    .max(0.0);
    gap * gap > close * close
}

/// Host geometry circle: authored `GeometryInfo` when present, else mesh radius.
pub fn host_bounding_circle_radius(
    geometry_info_authored: bool,
    geometry_info_circle: f32,
    fallback_radius: f32,
) -> f32 {
    if geometry_info_authored {
        geometry_info_circle.max(0.0)
    } else {
        fallback_radius.max(0.0)
    }
}

/// Deterministic C++ `GameLogicRandomValue(-range, range)` twitch pair.
pub fn warehouse_twitch_delta(seed: u32, draw: u32) -> (f32, f32) {
    let x = crate::game_logic::host_rng_residual::pure_logic_random_real(
        seed,
        draw,
        -WAREHOUSE_TWITCH_RANGE,
        WAREHOUSE_TWITCH_RANGE,
    );
    let z = crate::game_logic::host_rng_residual::pure_logic_random_real(
        seed,
        draw.wrapping_add(1),
        -WAREHOUSE_TWITCH_RANGE,
        WAREHOUSE_TWITCH_RANGE,
    );
    (x, z)
}

/// C++ `gainOneBox` depleted-voice: play when no next warehouse, or next is
/// farther than `getWarehouseScanDistance()/4` (AI scan already 2×).
pub fn should_play_supplies_depleted_voice(
    next_warehouse_distance: Option<f32>,
    scan_distance: f32,
    voice: &str,
) -> bool {
    if voice.is_empty() {
        return false;
    }
    match next_warehouse_distance {
        None => true,
        Some(dist) => dist > scan_distance / 4.0,
    }
}

/// C++ `TheGameText->fetch("GUI:AddCash")` — retail English CSF is `$%d`.
pub fn format_gui_add_cash(value: u32) -> String {
    format!("${value}")
}

/// C++ hide popup only when the *center* is STEALTHED, not locally controlled,
/// and not DETECTED.
pub fn hide_stealth_supply_cash(
    center_stealthed: bool,
    center_locally_controlled: bool,
    center_detected: bool,
) -> bool {
    center_stealthed && !center_locally_controlled && !center_detected
}

/// C++ `setDockCrippled(true)` victim action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockCrippleVictimAction {
    None,
    KillGround,
    IdleAndForceWanting,
}

pub fn dock_cripple_victim_action(
    crippled: bool,
    docker_inside: bool,
    docker_airborne: bool,
) -> DockCrippleVictimAction {
    if !crippled {
        return DockCrippleVictimAction::None;
    }
    if docker_inside {
        if docker_airborne {
            DockCrippleVictimAction::None
        } else {
            DockCrippleVictimAction::KillGround
        }
    } else {
        DockCrippleVictimAction::IdleAndForceWanting
    }
}

/// C++ `UnitCrateCollide` `findPositionAround` maxRadius 20.
pub const UNIT_CRATE_FIND_POSITION_RADIUS: f32 = 20.0;

/// Ring search around `origin` (host Y-up) so spawned units do not stack.
pub fn find_position_around_xz(
    origin: Vec3,
    occupied: &[(Vec3, f32)],
    index: u32,
    seed: u32,
) -> Vec3 {
    let start_ang = (seed.wrapping_add(index.wrapping_mul(17)) as f32) * 0.37;
    let mut best = origin;
    for ring in 0..6u32 {
        let radius = (ring as f32) * (UNIT_CRATE_FIND_POSITION_RADIUS / 5.0);
        let steps = if ring == 0 { 1 } else { 8 };
        for step in 0..steps {
            let ang = start_ang + (step as f32) * (std::f32::consts::TAU / steps as f32);
            let candidate = Vec3::new(
                origin.x + ang.cos() * radius,
                origin.y,
                origin.z + ang.sin() * radius,
            );
            let blocked = occupied.iter().any(|(pos, r)| {
                let dx = pos.x - candidate.x;
                let dz = pos.z - candidate.z;
                dx * dx + dz * dz < (*r + 2.0) * (*r + 2.0)
            });
            if !blocked {
                return candidate;
            }
            best = candidate;
        }
    }
    best
}

/// C++ `Player::hasScience` residual — case-insensitive name match.
pub fn player_has_science<'a, I>(unlocked: I, required: &str) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    if required.is_empty() {
        return true;
    }
    let req = required.to_ascii_lowercase().replace('-', "_");
    unlocked.into_iter().any(|s| {
        let u = s.to_ascii_lowercase().replace('-', "_");
        u == req || u.ends_with(&req) || req.ends_with(&u)
    })
}

/// C++ KindOf mask test: every set bit in `mask` must be present on the killer.
pub fn killer_matches_kindof_mask(killer_kindof_names: &[&str], mask: u64) -> bool {
    if mask == 0 {
        return true;
    }
    let names = game_engine::common::system::kind_of::KIND_OF_BIT_NAMES;
    for (index, name) in names.iter().enumerate() {
        if index >= 64 {
            break;
        }
        if (mask & (1u64 << index)) == 0 {
            continue;
        }
        let want = name.to_ascii_uppercase();
        if !killer_kindof_names
            .iter()
            .any(|have| have.eq_ignore_ascii_case(&want))
        {
            return false;
        }
    }
    true
}

/// Host object id used as a twitch RNG seed.
pub fn twitch_seed(object: ObjectId, frame: u32) -> u32 {
    object.0.wrapping_mul(2654435761).wrapping_add(frame)
}

/// C++ `SabotageSupplyCenterCrateCollide`: `min(desired, victimMoney)`; 0 if broke.
pub fn steal_cash_clamped(available: u32, desired: u32) -> u32 {
    desired.min(available)
}

/// C++ `Drawable::updateDrawableSupplyStatus(startingBoxes, boxesStored)`.
/// Host cash is `boxes * ValuePerSupplyBox`; convert back for the crate visual.
pub fn drawable_supply_status_from_cash(
    starting_boxes: Option<u32>,
    stored_cash: u32,
) -> (u32, u32) {
    let value = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    let value = if value > 0 { value as u32 } else { 75 };
    let max_boxes = starting_boxes.unwrap_or(0);
    let current_boxes = stored_cash / value;
    (max_boxes, current_boxes)
}

/// C++ `SupplyTruckAIUpdate::gainOneBox` / `loseOneBox`
/// (`SupplyTruckAIUpdate.cpp:122-126`, `:164-168`):
/// `drawable->updateDrawableSupplyStatus(maxBoxes, m_numberBoxes)`.
/// Host collectors store cash; convert with retail `ValuePerSupplyBox`.
pub fn collector_drawable_supply_status(max_boxes: u32, stored_cash: u32) -> (u32, u32) {
    drawable_supply_status_from_cash(Some(max_boxes), stored_cash)
}

/// C++ `W3DModelDraw::updateDrawModuleSupplyStatus` (`W3DModelDraw.cpp:3907-3917`):
/// CARRYING when `currentSupply > 0`. Presentation stamps the mesh bit.
pub fn collector_carrying_from_boxes(current_boxes: u32) -> bool {
    current_boxes > 0
}

/// C++ `SupplyTruckWantsToPickUpOrDeliverBoxesState::update`
/// (`SupplyTruckAIUpdate.cpp:507-529`): `numBoxes > 0` docks a center
/// first. Empty cargo seeks a warehouse. Neither search falls through —
/// leftover `truck_states.rs` returns Failure → regroup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WantingDockTarget {
    Center,
    Warehouse,
}

/// Leftover `SupplyTruckWantsToPickUpOrDeliverBoxesState::update`
/// (`truck_states.rs:231-257`) — wire this instead of retargeting a
/// warehouse while the collector is still carrying.
pub fn wanting_dock_target(number_boxes: i32) -> WantingDockTarget {
    if number_boxes > 0 {
        WantingDockTarget::Center
    } else {
        WantingDockTarget::Warehouse
    }
}

/// Retail W3DSupplyDraw `SupplyBonePrefix` default (`SupplyBox01`…).
pub const DEFAULT_SUPPLY_BONE_PREFIX: &str = "SupplyBox";
/// Leftover Device / typical warehouse crate-bone count when HTree is unknown.
pub const DEFAULT_SUPPLY_DRAW_BONE_COUNT: u32 = 8;

/// C++ `W3DSupplyDraw::updateDrawModuleSupplyStatus`
/// (`W3DSupplyDraw.cpp:60-61`): `ceil(total * current / max)`.
pub fn supply_draw_bones_to_show(total_bones: u32, current_supply: u32, max_supply: u32) -> u32 {
    if total_bones == 0 || max_supply == 0 {
        return 0;
    }
    let shown =
        ((total_bones as f32) * (current_supply as f32) / (max_supply as f32)).ceil() as u32;
    shown.min(total_bones)
}

/// C++ `sprintf("%s%02d", prefix, index)` (`W3DSupplyDraw.cpp:75`).
pub fn supply_draw_bone_name(prefix: &str, index: u32) -> String {
    format!("{prefix}{index:02}")
}

/// 1-based crate index if `name` is `{prefix}NN` (case-insensitive prefix).
pub fn supply_draw_bone_index(prefix: &str, name: &str) -> Option<u32> {
    if prefix.is_empty() {
        return None;
    }
    let name = name.trim();
    if name.len() < prefix.len() + 2 {
        return None;
    }
    if !name.get(..prefix.len())?.eq_ignore_ascii_case(prefix) {
        return None;
    }
    name[prefix.len()..].parse::<u32>().ok().filter(|i| *i >= 1)
}

/// Count `{prefix}NN` names, then hide bones after `bonesToShow`.
/// If no names are listed, emit `DEFAULT_SUPPLY_DRAW_BONE_COUNT` slots so the
/// HLOD resolver can hide existing `SupplyBoxNN` children.
pub fn supply_draw_hide_directives(
    prefix: &str,
    named_bones: &[impl AsRef<str>],
    current_supply: u32,
    max_supply: u32,
) -> Vec<(String, bool)> {
    let prefix = if prefix.is_empty() {
        DEFAULT_SUPPLY_BONE_PREFIX
    } else {
        prefix
    };
    let mut total = 0u32;
    for name in named_bones {
        if let Some(idx) = supply_draw_bone_index(prefix, name.as_ref()) {
            total = total.max(idx);
        }
    }
    if total == 0 {
        total = DEFAULT_SUPPLY_DRAW_BONE_COUNT;
    }
    let show = supply_draw_bones_to_show(total, current_supply, max_supply);
    (1..=total)
        .map(|i| (supply_draw_bone_name(prefix, i), i > show))
        .collect()
}

/// C++ `setCashValue` → host stored cash (`boxes * ValuePerSupplyBox`).
pub fn warehouse_stored_supplies_from_cash(cash: i32) -> u32 {
    let boxes =
        crate::game_logic::host_structure_economy_residual::supply_warehouse_boxes_from_cash(cash);
    let value = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    let value = if value > 0 { value as u32 } else { 75 };
    (boxes.max(0) as u32).saturating_mul(value)
}

static PENDING_WAREHOUSE_SETS: LazyLock<Mutex<Vec<(String, i32)>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
static WAREHOUSE_CRIPPLING_STATES: LazyLock<Mutex<HashMap<ObjectId, WarehouseCripplingState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Queue `WAREHOUSE_SET_VALUE` for the live host drain.
pub fn queue_warehouse_set_value(name: &str, cash: i32) {
    if name.is_empty() {
        return;
    }
    if let Ok(mut q) = PENDING_WAREHOUSE_SETS.lock() {
        q.push((name.to_string(), cash));
    }
}

pub fn drain_warehouse_set_values() -> Vec<(String, i32)> {
    PENDING_WAREHOUSE_SETS
        .lock()
        .map(|mut q| q.drain(..).collect())
        .unwrap_or_default()
}

/// Drop live warehouse queues so tests do not leak ObjectId state.
pub fn reset_live_warehouse_host_state() {
    if let Ok(mut q) = PENDING_WAREHOUSE_SETS.lock() {
        q.clear();
    }
    if let Ok(mut s) = WAREHOUSE_CRIPPLING_STATES.lock() {
        s.clear();
    }
    reset_live_dock_queues();
}

/// Live `WAREHOUSE_CRIPPLING_STATES` entry.
///
/// C++ `SupplyWarehouseCripplingBehavior::xfer` writes
/// `m_healingSupressedUntilFrame` + `m_nextHealingFrame`. `last_health` is the
/// live `onDamage` stand-in: first observation below max re-arms suppression.
/// Persist it so a mid-suppression load does not treat remaining damage as a
/// fresh hit and restart the full SelfHealSupression window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarehouseCripplingState {
    pub last_health: f32,
    pub healing_suppressed_until_frame: u32,
    pub next_healing_frame: u32,
}

/// C++ `SupplyWarehouseCripplingBehavior::xfer` heal clocks for snapshot persist.
pub fn snapshot_live_warehouse_crippling_states() -> Vec<(ObjectId, WarehouseCripplingState)> {
    let Ok(map) = WAREHOUSE_CRIPPLING_STATES.lock() else {
        return Vec::new();
    };
    let mut entries: Vec<(ObjectId, WarehouseCripplingState)> =
        map.iter().map(|(id, state)| (*id, *state)).collect();
    entries.sort_by_key(|(id, _)| id.0);
    entries
}

/// Replace process-global heal clocks so a load cannot leak the previous session.
pub fn restore_live_warehouse_crippling_states(entries: Vec<(ObjectId, WarehouseCripplingState)>) {
    if let Ok(mut map) = WAREHOUSE_CRIPPLING_STATES.lock() {
        map.clear();
        for (id, state) in entries {
            map.insert(id, state);
        }
    }
}

/// C++ `SupplyWarehouseCripplingBehavior::update` pulse after suppression.
pub fn warehouse_crippling_heal_amount(
    current_frame: u32,
    current_health: f32,
    max_health: f32,
    last_health: f32,
    healing_suppressed_until_frame: &mut u32,
    next_healing_frame: &mut u32,
) -> f32 {
    use crate::game_logic::host_structure_economy_residual::{
        SUPPLY_WAREHOUSE_SELF_HEAL_AMOUNT, SUPPLY_WAREHOUSE_SELF_HEAL_DELAY_FRAMES,
        SUPPLY_WAREHOUSE_SELF_HEAL_SUPPRESSION_FRAMES,
    };
    if current_health + 0.01 < last_health {
        *healing_suppressed_until_frame =
            current_frame.saturating_add(SUPPLY_WAREHOUSE_SELF_HEAL_SUPPRESSION_FRAMES);
        *next_healing_frame = *healing_suppressed_until_frame;
    }
    if max_health <= 0.0 || current_health >= max_health - f32::EPSILON {
        return 0.0;
    }
    if current_frame < *healing_suppressed_until_frame || current_frame < *next_healing_frame {
        return 0.0;
    }
    *next_healing_frame = current_frame.saturating_add(SUPPLY_WAREHOUSE_SELF_HEAL_DELAY_FRAMES);
    SUPPLY_WAREHOUSE_SELF_HEAL_AMOUNT
}

/// Retail GLA SupplyCenterDock `GrantTemporaryStealth = 20000` ms → 600 frames.
/// America / China omit the field → module default 0.
pub fn grant_temporary_stealth_frames_for_center(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("gla") && (n.contains("supply") || n.contains("stash")) {
        crate::game_logic::host_dock_contain_exit_heal_residual::SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_FRAMES
    } else {
        crate::game_logic::host_dock_contain_exit_heal_residual::SUPPLY_CENTER_DOCK_DEFAULT_GRANT_STEALTH_FRAMES
    }
}

/// C++ `SupplyCenterDockUpdate::action` stealth grant gate:
/// center STEALTHED and (`isTemporaryGrant` or docker lacks `CAN_STEALTH`).
pub fn should_grant_temporary_stealth(
    center_stealthed: bool,
    grant_frames: u32,
    docker_is_temporary_grant: bool,
    docker_can_stealth: bool,
) -> bool {
    grant_frames > 0 && center_stealthed && (docker_is_temporary_grant || !docker_can_stealth)
}

/// C++ `DockUpdate::isClearToEnter`: only `m_activeDocker` may act.
/// Empty / dead active is reclaimable (update() promotes the next arriver).
pub fn dock_claim_active(
    current_active: Option<ObjectId>,
    current_active_alive: bool,
    claimant: ObjectId,
) -> Option<ObjectId> {
    match current_active {
        Some(id) if id == claimant => Some(id),
        Some(_) if current_active_alive => current_active,
        _ => Some(claimant),
    }
}

pub fn dock_is_clear_to_act(active: Option<ObjectId>, docker: ObjectId) -> bool {
    active == Some(docker)
}

/// C++ `SupplyWarehouseDockUpdate::action` (`SupplyWarehouseDockUpdate.cpp:89-111`)
/// tentatively `--m_boxesStored`, then `SupplyTruckAIUpdate::gainOneBox`
/// (`SupplyTruckAIUpdate.cpp:134-135`). Already at MaxBoxes fails and the
/// warehouse `++m_boxesStored` takes the box back. Fail-closed: no transfer.
pub fn warehouse_action_transfer_one_box(
    warehouse_boxes: u32,
    collector_boxes: u32,
    max_boxes: u32,
) -> (u32, u32, bool) {
    if warehouse_boxes == 0 {
        return (0, collector_boxes, false);
    }
    if collector_boxes >= max_boxes {
        return (warehouse_boxes, collector_boxes, false);
    }
    (warehouse_boxes - 1, collector_boxes + 1, true)
}

/// Retail regular `AmericaVehicleChinook` `UpgradedSupplyBoost` (leftover INI parse = 4).
pub const REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST: u32 = 4;

/// C++ `ChinookAIUpdate` collectors: regular + Combat Chinook template names.
pub fn is_chinook_supply_collector(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    !n.is_empty() && n.contains("chinook")
}

/// C++ `ChinookAIUpdate::isAvailableForSupplying` (`ChinookAIUpdate.cpp:982-991`).
/// Non-Chinook collectors (trucks, workers) stay available.
pub fn chinook_available_for_supplying(
    is_chinook: bool,
    contain_count: usize,
    wanting_enter_or_exit: bool,
    is_overlord_style: bool,
) -> bool {
    if !is_chinook {
        return true;
    }
    !wanting_enter_or_exit && contain_count == 0 && !is_overlord_style
}

/// C++ `SupplyCenterDockUpdate::action` + `getUpgradedSupplyBoost`:
/// Chinooks return INI `UpgradedSupplyBoost` only with `Upgrade_AmericaSupplyLines`.
/// Trucks return 0. Worker shoes is a separate upgrade.
pub fn collector_supply_lines_boost(
    is_chinook: bool,
    is_combat_chinook: bool,
    authored_boost: u32,
    has_supply_lines: bool,
) -> u32 {
    if !is_chinook || !has_supply_lines {
        return 0;
    }
    if authored_boost > 0 {
        return authored_boost;
    }
    if is_combat_chinook {
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_UPGRADED_SUPPLY_BOOST
    } else {
        REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST
    }
}

impl crate::game_logic::GameLogic {
    /// C++ `SupplyWarehouseCripplingBehavior` + `WAREHOUSE_SET_VALUE` drain.
    pub fn update_supply_warehouse_crippling(&mut self) {
        self.drain_warehouse_script_set_values();
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && o.thing.template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse
            })
            .map(|(id, _)| *id)
            .collect();
        let mut states = match WAREHOUSE_CRIPPLING_STATES.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let max_h = obj.health.maximum.max(obj.max_health).max(1.0);
            let cur = obj.health.current;
            // C++ onDamage resets SelfHealSupression. First observation below
            // max means damage already happened (module would be awake).
            let state = states.entry(id).or_insert(WarehouseCripplingState {
                last_health: if cur + 0.01 < max_h { max_h } else { cur },
                healing_suppressed_until_frame: u32::MAX,
                next_healing_frame: u32::MAX,
            });
            let amount = warehouse_crippling_heal_amount(
                frame,
                cur,
                max_h,
                state.last_health,
                &mut state.healing_suppressed_until_frame,
                &mut state.next_healing_frame,
            );
            if amount > 0.0 {
                // C++ SupplyWarehouseCripplingBehavior::update attemptHealing.
                obj.heal(amount);
            }
            state.last_health = obj.health.current;
        }
    }

    pub fn drain_warehouse_script_set_values(&mut self) {
        for (name, cash) in drain_warehouse_set_values() {
            let _ = self.apply_warehouse_set_value(&name, cash);
        }
    }

    /// C++ `SupplyWarehouseDockUpdate::setCashValue`.
    pub fn apply_warehouse_set_value(&mut self, name: &str, cash: i32) -> bool {
        let Some(id) = self.find_object_id_by_name(name) else {
            return false;
        };
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.thing.template.dock_kind != crate::game_logic::DockKind::SupplyWarehouse {
            return false;
        }
        obj.set_stored_supplies(warehouse_stored_supplies_from_cash(cash));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steal_from_broke_victim_is_zero() {
        assert_eq!(steal_cash_clamped(0, 1000), 0);
        assert_eq!(steal_cash_clamped(250, 1000), 250);
        assert_eq!(steal_cash_clamped(5000, 1000), 1000);
    }

    #[test]
    fn warehouse_action_does_not_debit_when_collector_already_at_max_boxes() {
        // Last remaining box + full collector: C++ take-back, warehouse stays.
        assert_eq!(warehouse_action_transfer_one_box(1, 4, 4), (1, 4, false));
        assert_eq!(warehouse_action_transfer_one_box(10, 4, 4), (10, 4, false));
        // Room for one more: debit and credit.
        assert_eq!(warehouse_action_transfer_one_box(1, 3, 4), (0, 4, true));
        assert_eq!(warehouse_action_transfer_one_box(0, 2, 4), (0, 2, false));
    }

    #[test]
    fn warehouse_drawable_boxes_follow_cash() {
        let (max, current) = drawable_supply_status_from_cash(Some(10), 10 * 75);
        assert_eq!(max, 10);
        assert_eq!(current, 10);
        let (_, after) = drawable_supply_status_from_cash(Some(10), 9 * 75);
        assert_eq!(after, 9);
        let (_, empty) = drawable_supply_status_from_cash(Some(10), 0);
        assert_eq!(empty, 0);
    }

    #[test]
    fn collector_drawable_boxes_follow_cash_and_carrying() {
        // C++ gainOneBox / loseOneBox: updateDrawableSupplyStatus(maxBoxes, m_numberBoxes).
        assert_eq!(collector_drawable_supply_status(4, 0), (4, 0));
        assert_eq!(collector_drawable_supply_status(4, 75), (4, 1));
        assert_eq!(collector_drawable_supply_status(4, 4 * 75), (4, 4));
        assert!(!collector_carrying_from_boxes(0));
        assert!(collector_carrying_from_boxes(1));
    }

    #[test]
    fn gla_stash_grants_temporary_stealth_america_does_not() {
        assert_eq!(
            grant_temporary_stealth_frames_for_center("GLASupplyStash"),
            600
        );
        assert_eq!(
            grant_temporary_stealth_frames_for_center("AmericaSupplyCenter"),
            0
        );
        assert!(should_grant_temporary_stealth(true, 600, false, false));
        assert!(!should_grant_temporary_stealth(true, 600, false, true));
        assert!(should_grant_temporary_stealth(true, 600, true, true));
        assert!(!should_grant_temporary_stealth(false, 600, false, false));
        assert!(!should_grant_temporary_stealth(true, 0, false, false));
    }

    #[test]
    fn single_active_docker_queue() {
        let a = ObjectId(1);
        let b = ObjectId(2);
        let first = dock_claim_active(None, false, a);
        assert_eq!(first, Some(a));
        assert!(dock_is_clear_to_act(first, a));
        assert!(!dock_is_clear_to_act(first, b));
        let blocked = dock_claim_active(Some(a), true, b);
        assert_eq!(blocked, Some(a));
        let after_dead = dock_claim_active(Some(a), false, b);
        assert_eq!(after_dead, Some(b));
    }

    #[test]
    fn loaded_chinook_is_not_available_for_supplying() {
        // C++ ChinookAIUpdate::isAvailableForSupplying (ChinookAIUpdate.cpp:982-991).
        assert!(is_chinook_supply_collector("AmericaVehicleChinook"));
        assert!(is_chinook_supply_collector("AirF_AmericaVehicleChinook"));
        assert!(!is_chinook_supply_collector("ChinaVehicleSupplyTruck"));
        assert!(chinook_available_for_supplying(true, 0, false, false));
        assert!(!chinook_available_for_supplying(true, 1, false, false));
        assert!(!chinook_available_for_supplying(true, 0, true, false));
        assert!(!chinook_available_for_supplying(true, 0, false, true));
        assert!(chinook_available_for_supplying(false, 3, true, true));
    }

    #[test]
    fn supply_lines_boost_is_chinook_ini_only() {
        // C++ ChinookAIUpdate::getUpgradedSupplyBoost (ChinookAIUpdate.cpp:1644-1652)
        // vs SupplyTruckAIUpdate.h:196 returning 0.
        assert_eq!(collector_supply_lines_boost(false, false, 0, true), 0);
        assert_eq!(collector_supply_lines_boost(true, false, 0, false), 0);
        assert_eq!(
            collector_supply_lines_boost(true, false, 0, true),
            REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST
        );
        assert_eq!(collector_supply_lines_boost(true, true, 0, true), 60);
        assert_eq!(collector_supply_lines_boost(true, true, 4, true), 4);
    }

    #[test]
    fn warehouse_close_enough_is_bounding_sphere_gap() {
        // Hull contact: center 35, r 10+20, gap 5 vs docker diameter 20.
        assert!(!warehouse_too_far_2d((0.0, 0.0), (35.0, 0.0), 10.0, 20.0));
        // Old center-to-center vs (r*2)^2 would reject this.
        assert!(35.0 * 35.0 > (10.0 * 2.0) * (10.0 * 2.0));
        assert!(warehouse_too_far_2d((0.0, 0.0), (60.0, 0.0), 10.0, 20.0));
        assert!(!warehouse_too_far_2d((0.0, 0.0), (5.0, 0.0), 10.0, 20.0));
    }

    #[test]
    fn gui_add_cash_is_retail_dollar_n() {
        assert_eq!(format_gui_add_cash(75), "$75");
        assert_eq!(format_gui_add_cash(0), "$0");
    }

    #[test]
    fn supply_draw_hides_crate_bones_by_ratio() {
        // 8 bones, half stock → show 4, hide 05..08.
        let dirs = supply_draw_hide_directives("SupplyBox", &[] as &[&str], 200, 400);
        assert_eq!(dirs.len(), 8);
        assert!(!dirs[3].1);
        assert!(dirs[4].1);
        assert_eq!(dirs[4].0, "SupplyBox05");
        assert_eq!(supply_draw_bones_to_show(8, 0, 10), 0);
        assert_eq!(supply_draw_bones_to_show(8, 10, 10), 8);
        assert_eq!(supply_draw_bones_to_show(8, 1, 10), 1);
    }

    #[test]
    fn warehouse_set_value_ceils_boxes_from_cash() {
        assert_eq!(warehouse_stored_supplies_from_cash(75), 75);
        assert_eq!(warehouse_stored_supplies_from_cash(76), 150);
        assert_eq!(warehouse_stored_supplies_from_cash(0), 0);
        assert_eq!(warehouse_stored_supplies_from_cash(1000), 14 * 75);
    }

    #[test]
    fn warehouse_crippling_heals_after_suppression() {
        let mut suppressed = 0;
        let mut next = 0;
        // Damage at frame 10 from 1000 → 200.
        let first =
            warehouse_crippling_heal_amount(10, 200.0, 1000.0, 1000.0, &mut suppressed, &mut next);
        assert_eq!(first, 0.0);
        assert_eq!(suppressed, 100);
        assert_eq!(next, 100);
        let mid =
            warehouse_crippling_heal_amount(99, 200.0, 1000.0, 200.0, &mut suppressed, &mut next);
        assert_eq!(mid, 0.0);
        let heal =
            warehouse_crippling_heal_amount(100, 200.0, 1000.0, 200.0, &mut suppressed, &mut next);
        assert!((heal - 5.0).abs() < 0.01);
        assert_eq!(next, 115);
        let full = warehouse_crippling_heal_amount(
            200,
            1000.0,
            1000.0,
            1000.0,
            &mut suppressed,
            &mut next,
        );
        assert_eq!(full, 0.0);
    }

    #[test]
    fn warehouse_crippling_snapshot_keeps_mid_suppression_cadence() {
        reset_live_warehouse_host_state();
        let id = ObjectId(7);
        restore_live_warehouse_crippling_states(vec![(
            id,
            WarehouseCripplingState {
                last_health: 200.0,
                healing_suppressed_until_frame: 100,
                next_healing_frame: 100,
            },
        )]);
        let snap = snapshot_live_warehouse_crippling_states();
        restore_live_warehouse_crippling_states(vec![(
            ObjectId(99),
            WarehouseCripplingState {
                last_health: 1.0,
                healing_suppressed_until_frame: 1,
                next_healing_frame: 1,
            },
        )]);
        restore_live_warehouse_crippling_states(snap);
        let states = snapshot_live_warehouse_crippling_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].0, id);
        let mut state = states[0].1;
        let mid = warehouse_crippling_heal_amount(
            50,
            200.0,
            1000.0,
            state.last_health,
            &mut state.healing_suppressed_until_frame,
            &mut state.next_healing_frame,
        );
        assert_eq!(mid, 0.0);
        assert_eq!(
            state.healing_suppressed_until_frame, 100,
            "mid-suppression load must not restart SelfHealSupression"
        );
        let heal = warehouse_crippling_heal_amount(
            100,
            200.0,
            1000.0,
            state.last_health,
            &mut state.healing_suppressed_until_frame,
            &mut state.next_healing_frame,
        );
        assert!((heal - 5.0).abs() < 0.01);
        reset_live_warehouse_host_state();
    }

    #[test]
    fn promote_active_never_assigns_when_crippled() {
        let mut q = HostDockApproachQueue::new(5);
        let docker = ObjectId(11);
        assert_eq!(q.reserve_approach_position(docker), Some(0));
        q.on_approach_reached(docker);
        assert_eq!(q.promote_active(None, false, false), Some(docker));
        assert_eq!(q.promote_active(None, false, true), None);
    }

    #[test]
    fn crippled_warehouse_never_clears_to_act() {
        reset_live_dock_queues();
        let dock = ObjectId(21);
        let a = ObjectId(22);
        let bone = Vec3::new(10.0, 0.0, 0.0);
        let tick = tick_live_dock_approach_ex(
            dock,
            a,
            5,
            true,
            None,
            false,
            bone,
            Vec3::ZERO,
            20.0,
            &[bone],
            0,
            true,
            |_| true,
        );
        assert_eq!(tick, DockApproachTick::Blocked);
        reset_live_dock_queues();
    }

    #[test]
    fn evict_dead_clears_ghost_reservation() {
        reset_live_dock_queues();
        let mut q = HostDockApproachQueue::new(5);
        let ghost = ObjectId(9);
        assert_eq!(q.reserve_approach_position(ghost), Some(0));
        q.on_approach_reached(ghost);
        q.evict_dead(|_| true);
        assert_eq!(
            q.owners[0],
            Some(ghost),
            "alive predicate must keep the slot"
        );
        q.evict_dead(|_| false);
        assert_eq!(q.owners[0], None);
        assert!(!q.reached[0]);
        assert!(q.promote_active(None, false, false).is_none());
    }

    #[test]
    fn waiting_bones_drive_path_to_not_instant_act() {
        reset_live_dock_queues();
        let dock = ObjectId(1);
        let a = ObjectId(2);
        let bone = Vec3::new(40.0, 0.0, 0.0);
        let tick = tick_live_dock_approach(
            dock,
            a,
            5,
            true,
            None,
            false,
            Vec3::new(200.0, 0.0, 0.0),
            Vec3::ZERO,
            20.0,
            &[bone],
            0,
            |_| true,
        );
        assert_eq!(tick, DockApproachTick::PathTo(bone));
        let arrived = tick_live_dock_approach(
            dock,
            a,
            5,
            true,
            None,
            false,
            bone,
            Vec3::ZERO,
            20.0,
            &[bone],
            1,
            |_| true,
        );
        assert_eq!(arrived, DockApproachTick::ClearToAct);
        reset_live_dock_queues();
    }

    #[test]
    fn boneless_bias_is_half_major_radius_toward_docker() {
        let q = HostDockApproachQueue::new(5);
        let goal = q.approach_world_position(0, Vec3::new(100.0, 0.0, 0.0), Vec3::ZERO, 40.0);
        assert!((goal.x - 20.0).abs() < 0.01);
        assert!(goal.z.abs() < 0.01);
    }

    #[test]
    fn wait_for_clearance_times_out_after_900_frames() {
        reset_live_dock_queues();
        let dock = ObjectId(3);
        let a = ObjectId(4);
        let b = ObjectId(5);
        let bone_a = Vec3::new(10.0, 0.0, 0.0);
        let bone_b = Vec3::new(20.0, 0.0, 0.0);
        assert_eq!(
            tick_live_dock_approach(
                dock,
                a,
                5,
                true,
                None,
                false,
                bone_a,
                Vec3::ZERO,
                10.0,
                &[bone_a, bone_b],
                0,
                |_| true,
            ),
            DockApproachTick::ClearToAct
        );
        assert_eq!(
            tick_live_dock_approach(
                dock,
                b,
                5,
                true,
                Some(a),
                true,
                bone_b,
                Vec3::ZERO,
                10.0,
                &[bone_a, bone_b],
                10,
                |_| true,
            ),
            DockApproachTick::Blocked
        );
        assert_eq!(
            tick_live_dock_approach(
                dock,
                b,
                5,
                true,
                Some(a),
                true,
                bone_b,
                Vec3::ZERO,
                10.0,
                &[bone_a, bone_b],
                10 + WAIT_FOR_CLEARANCE_FRAMES,
                |_| true,
            ),
            DockApproachTick::TimedOut
        );
        assert!(live_dock_is_clear_to_approach(dock, b));
        reset_live_dock_queues();
    }

    #[test]
    fn death_or_retask_cancels_all_reservations() {
        reset_live_dock_queues();
        let dock = ObjectId(6);
        let a = ObjectId(7);
        let _ = tick_live_dock_approach(
            dock,
            a,
            5,
            true,
            Some(ObjectId(8)),
            true,
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::ZERO,
            20.0,
            &[],
            0,
            |_| true,
        );
        cancel_all_live_dock_reservations_for(a);
        assert!(live_dock_is_clear_to_approach(dock, ObjectId(9)));
        reset_live_dock_queues();
    }
}
