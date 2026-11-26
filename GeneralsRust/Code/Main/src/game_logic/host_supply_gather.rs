//! Host supply-truck / warehouse / center helpers.
//!
//! C++: `SupplyTruckAIUpdate`, `SupplyWarehouseDockUpdate`, `SupplyCenterDockUpdate`.

use crate::game_logic::{DockKind, ObjectId};
use glam::Vec3;

use std::collections::{HashMap, HashSet};

/// Main-owned mirror of each dock module's C++ approach-position state.
/// `GameLogic` owns this store; object state transitions report cancellations
/// with epochs that the owner reconciles before observing queue state.
#[derive(Debug, Clone, Default)]
pub(crate) struct HostDockApproachQueues {
    queues: HashMap<ObjectId, HostDockApproachQueue>,
    observed_cancel_epochs: HashMap<ObjectId, u64>,
    pending_cancel_effects: HashSet<(ObjectId, ObjectId)>,
}

impl HostDockApproachQueues {
    /// C++ `DockUpdate::reserveApproachPosition` + `update` promote first arriver.
    /// Returns `PathTo` until the docker reaches its `DockWaiting` / boneless slot.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn tick(
        &mut self,
        dock_id: ObjectId,
        docker_id: ObjectId,
        docker_cancel_epoch: u64,
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
        defer_enter_reached: bool,
        is_alive: impl FnMut(ObjectId) -> bool,
        mut has_active_owner: impl FnMut(ObjectId) -> bool,
    ) -> DockApproachTick {
        let queue = self
            .queues
            .entry(dock_id)
            .or_insert_with(|| HostDockApproachQueue::new(number_approach_positions));
        let previous_owners: Vec<_> = queue.owned_dockers().collect();
        let result = tick_dock_approach(
            queue,
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
            crippled,
            defer_enter_reached,
            is_alive,
        );
        let current_owners: Vec<_> = self
            .queues
            .get(&dock_id)
            .into_iter()
            .flat_map(HostDockApproachQueue::owned_dockers)
            .collect();
        for previous in previous_owners {
            if !current_owners.contains(&previous) {
                self.forget_if_unowned(previous, has_active_owner(previous));
            }
        }
        if matches!(&result, DockApproachTick::ClearToAct) {
            // C++ onEnterReached releases its approach slot but keeps
            // DockUpdate::m_activeDocker until onExitReached. The canonical
            // active owner lives on the receiving dock Object; retain only
            // its cancellation epoch here, not a second active-owner map.
            self.observed_cancel_epochs
                .insert(docker_id, docker_cancel_epoch);
        } else if let Some(stale) = current_active.filter(|_| !current_active_alive) {
            self.forget_if_unowned(stale, has_active_owner(stale));
        }
        if current_owners.contains(&docker_id) {
            self.observed_cancel_epochs
                .insert(docker_id, docker_cancel_epoch);
        }
        result
    }

    pub(crate) fn on_enter_reached(&mut self, dock_id: ObjectId, docker_id: ObjectId) {
        if let Some(queue) = self.queues.get_mut(&dock_id) {
            queue.on_enter_reached(docker_id);
        }
    }
    pub(crate) fn on_approach_reached(&mut self, dock_id: ObjectId, docker_id: ObjectId) {
        if let Some(queue) = self.queues.get_mut(&dock_id) {
            queue.on_approach_reached(docker_id);
        }
    }
    pub(crate) fn owns_approach(&self, dock_id: ObjectId, docker_id: ObjectId) -> bool {
        self.queues.get(&dock_id).is_some_and(|queue| queue.index_of(docker_id).is_some())
    }




    pub(crate) fn cancel_dock(
        &mut self,
        dock_id: ObjectId,
        docker_id: ObjectId,
        active_elsewhere: bool,
    ) {
        if let Some(queue) = self.queues.get_mut(&dock_id) {
            queue.cancel_docker(docker_id);
        }
        self.pending_cancel_effects.remove(&(dock_id, docker_id));
        self.forget_if_unowned(docker_id, active_elsewhere);
    }

    /// C++ `DockUpdate::cancelDock` for every live queue this docker reserved.
    pub(crate) fn cancel_docker(&mut self, docker_id: ObjectId) {
        for queue in self.queues.values_mut() {
            queue.cancel_docker(docker_id);
        }
        self.pending_cancel_effects
            .retain(|(_, active_docker)| *active_docker != docker_id);
        self.observed_cancel_epochs.remove(&docker_id);
    }

    /// A dock module's queue has the same lifetime as its owning Object.
    pub(crate) fn remove_dock(
        &mut self,
        dock_id: ObjectId,
        removed_active_docker: Option<ObjectId>,
        active_elsewhere: &[ObjectId],
    ) {
        let mut owners: Vec<_> = self
            .queues
            .remove(&dock_id)
            .into_iter()
            .flat_map(|queue| queue.owned_dockers().collect::<Vec<_>>())
            .collect();
        if let Some(active) = removed_active_docker {
            if !owners.contains(&active) {
                owners.push(active);
            }
        }
        for owner in owners {
            self.forget_if_unowned(owner, active_elsewhere.contains(&owner));
        }
    }

    /// Reconcile only tracked docker IDs. The owner resolves canonical active
    /// dock Objects lazily, and only for epochs that actually changed.
    pub(crate) fn reconcile_cancel_epochs(
        &mut self,
        current_epochs: &[(ObjectId, Option<u64>)],
        mut active_docks_for_docker: impl FnMut(ObjectId) -> Vec<ObjectId>,
    ) {
        for &(docker_id, current) in current_epochs {
            let Some(previous) = self.observed_cancel_epochs.get(&docker_id).copied() else {
                continue;
            };
            if current != Some(previous) {
                let active_docks = active_docks_for_docker(docker_id);
                self.reconcile_cancelled_docker(docker_id, &active_docks);
            }
        }
    }

    pub(crate) fn reconcile_dock_cancel_epochs(
        &mut self,
        current_epochs: &[(ObjectId, Option<u64>)],
        mut active_docks_for_docker: impl FnMut(ObjectId) -> Vec<ObjectId>,
    ) {
        let changed: Vec<_> = current_epochs
            .iter()
            .filter_map(|&(docker_id, current)| {
                self.observed_cancel_epochs
                    .get(&docker_id)
                    .is_some_and(|previous| current != Some(*previous))
                    .then_some(docker_id)
            })
            .collect();
        if changed.is_empty() {
            return;
        }
        // C++ cancelDock releases this docker from every dock module. Once a
        // referenced dock notices the callback epoch, remove it world-wide.
        for docker_id in changed {
            let active_docks = active_docks_for_docker(docker_id);
            self.reconcile_cancelled_docker(docker_id, &active_docks);
        }
    }

    pub(crate) fn tracked_docker_ids_for_dock(
        &self,
        dock_id: ObjectId,
        active_docker: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        let mut ids: Vec<_> = self
            .queues
            .get(&dock_id)
            .into_iter()
            .flat_map(HostDockApproachQueue::owned_dockers)
            .collect();
        if let Some(active) = active_docker {
            if !ids.contains(&active) {
                ids.push(active);
            }
        }
        ids
    }

    pub(crate) fn tracked_docker_ids(&self) -> Vec<ObjectId> {
        self.observed_cancel_epochs.keys().copied().collect()
    }

    pub(crate) fn tracks_docker(&self, docker_id: ObjectId) -> bool {
        self.observed_cancel_epochs.contains_key(&docker_id)
    }

    pub(crate) fn has_dock(&self, dock_id: ObjectId) -> bool {
        self.queues.contains_key(&dock_id)
    }

    pub(crate) fn is_clear_to_approach(&self, dock_id: ObjectId, docker_id: ObjectId) -> bool {
        self.queues
            .get(&dock_id)
            .map(|queue| queue.is_clear_to_approach(docker_id))
            .unwrap_or(true)
    }

    pub(crate) fn clear(&mut self) {
        self.queues.clear();
        self.observed_cancel_epochs.clear();
        self.pending_cancel_effects.clear();
    }

    pub(crate) fn track_restored_active_docker(
        &mut self,
        docker_id: ObjectId,
        docker_cancel_epoch: u64,
    ) {
        self.observed_cancel_epochs
            .insert(docker_id, docker_cancel_epoch);
    }

    pub(crate) fn take_pending_cancel_effects(&mut self) -> Vec<(ObjectId, ObjectId)> {
        let mut pending: Vec<_> = self.pending_cancel_effects.drain().collect();
        pending.sort_by_key(|(dock_id, docker_id)| (dock_id.0, docker_id.0));
        pending
    }

    /// C++ `DockUpdate::xfer` approach-slot vectors for snapshot persist.
    pub(crate) fn snapshot(&self) -> Vec<(ObjectId, HostDockApproachQueue)> {
        let mut entries: Vec<_> = self
            .queues
            .iter()
            .map(|(id, queue)| (*id, queue.clone()))
            .collect();
        entries.sort_by_key(|(id, _)| id.0);
        entries
    }

    /// Restore this world's module queues and seed callback epochs from its
    /// already-admitted objects, without touching another world's store.
    pub(crate) fn restore(
        &mut self,
        entries: Vec<(ObjectId, HostDockApproachQueue)>,
        object_epochs: &HashMap<ObjectId, u64>,
    ) {
        self.clear();
        for (dock_id, queue) in entries {
            for docker_id in queue.owned_dockers() {
                if let Some(&epoch) = object_epochs.get(&docker_id) {
                    self.observed_cancel_epochs.insert(docker_id, epoch);
                }
            }
            self.queues.insert(dock_id, queue);
        }
    }

    fn forget_if_unowned(&mut self, docker_id: ObjectId, active_elsewhere: bool) {
        if !active_elsewhere
            && !self
                .queues
                .values()
                .any(|queue| queue.index_of(docker_id).is_some())
        {
            self.observed_cancel_epochs.remove(&docker_id);
        }
    }

    fn reconcile_cancelled_docker(&mut self, docker_id: ObjectId, active_docks: &[ObjectId]) {
        // Main supplies this operation-local projection from canonical dock
        // Object fields at the same cancellation-observation boundary.
        for &dock_id in active_docks {
            self.pending_cancel_effects.insert((dock_id, docker_id));
        }
        for queue in self.queues.values_mut() {
            queue.cancel_docker(docker_id);
        }
        self.observed_cancel_epochs.remove(&docker_id);
    }
}

#[allow(clippy::too_many_arguments)]
fn tick_dock_approach(
    queue: &mut HostDockApproachQueue,
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
    defer_enter_reached: bool,
    is_alive: impl FnMut(ObjectId) -> bool,
) -> DockApproachTick {
    queue.evict_dead(is_alive);
    if !waiting_bones.is_empty() {
        queue.set_waiting_bones(waiting_bones.to_vec());
    }
    if !docker_alive {
        queue.cancel_docker(docker_id);
        return DockApproachTick::Blocked;
    }
    if current_active == Some(docker_id) && current_active_alive && !crippled {
        if !defer_enter_reached { queue.on_enter_reached(docker_id); }
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
    let reached = if defer_enter_reached {
        queue.index_of(docker_id).and_then(|index| queue.reached.get(index as usize)).copied().unwrap_or(false)
    } else {
        docker_pos.distance(goal) <= DOCK_APPROACH_ARRIVAL_SLOP
    };
    if !reached {
        queue.clear_wait_started(docker_id);
        return DockApproachTick::PathTo(goal);
    }
    queue.on_approach_reached(docker_id);
    if queue.promote_active(current_active, current_active_alive, crippled) == Some(docker_id) {
        if !defer_enter_reached { queue.on_enter_reached(docker_id); }
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

    pub(crate) fn owned_dockers(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.owners.iter().flatten().copied()
    }

    /// C++ `DockUpdate::reserveApproachPosition` — returns the reserved index.
    pub fn reserve_approach_position(&mut self, docker: ObjectId) -> Option<i32> {
        if let Some(index) = self.index_of(docker) {
            return Some(index);
        }
        for (index, owner) in self.owners.iter().enumerate() {
            if owner.is_none() {
                self.owners[index] = Some(docker);
                self.reached[index] = false;
                return Some(index as i32);
            }
        }
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            self.owners.push(Some(docker));
            self.reached.push(false);
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
            return self.waiting_bones.get(slot).copied().unwrap_or(dock_pos);
        }
        let mut offset = docker_pos - dock_pos;
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
    // Rail docks use their parsed module data at both call sites.
    if dock_kind == DockKind::RailedTransport {
        return 0;
    }
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
        DockKind::RailedTransport => unreachable!("rail approach count is typed module data"),
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

/// C++ `W3DModelDraw::updateDrawModuleSupplyStatus` (`W3DModelDraw.cpp:3907-3917`):
/// CARRYING when `currentSupply > 0`. Presentation stamps the mesh bit.
pub fn collector_carrying_from_boxes(current_boxes: i32) -> bool {
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

/// SupplyWarehouseDockUpdate::setCashValue uses a float quotient and ceil.
fn warehouse_boxes_from_cash(cash: i32, value_per_box: i32) -> Option<i32> {
    let boxes = (cash as f32 / value_per_box as f32).ceil();
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&boxes) {
        return None;
    }
    Some(boxes as i32)
}

/// Healing observations and clocks owned by one live game instance.
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
    warehouse_boxes: i32,
    collector_boxes: i32,
    max_boxes: i32,
) -> (i32, i32, bool) {
    if warehouse_boxes == 0 {
        return (0, collector_boxes, false);
    }
    if collector_boxes >= max_boxes {
        return (warehouse_boxes, collector_boxes, false);
    }
    let Some(remaining) = warehouse_boxes.checked_sub(1) else {
        return (warehouse_boxes, collector_boxes, false);
    };
    (remaining, collector_boxes + 1, true)
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
    /// Snapshot in stable ID order for the existing lifecycle-tail encoding.
    pub(crate) fn snapshot_warehouse_crippling_states(
        &self,
    ) -> Vec<(ObjectId, WarehouseCripplingState)> {
        let mut entries: Vec<_> = self
            .warehouse_crippling_states
            .iter()
            .map(|(id, state)| (*id, *state))
            .collect();
        entries.sort_by_key(|(id, _)| id.0);
        entries
    }

    /// Replace clocks only on the world being restored.
    pub(crate) fn restore_warehouse_crippling_states(
        &mut self,
        entries: Vec<(ObjectId, WarehouseCripplingState)>,
    ) {
        self.warehouse_crippling_states.clear();
        self.warehouse_crippling_states.extend(entries);
    }

    /// C++ `SupplyWarehouseCripplingBehavior` + `WAREHOUSE_SET_VALUE` drain.
    pub fn update_supply_warehouse_crippling(&mut self) -> generals_sim::GameLogicResult<()> {
        self.update_supply_warehouse_crippling_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_supply_warehouse_crippling_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        self.drain_warehouse_script_set_values_with_effects(effects);
        let frame = self.frame as u32;
        let objects = &self.objects;
        self.warehouse_crippling_states.retain(|id, _| {
            objects.get(id).is_some_and(|object| {
                object.is_alive()
                    && object.thing().template.dock_kind
                        == crate::game_logic::DockKind::SupplyWarehouse
            })
        });
        let warehouses: Vec<(ObjectId, std::sync::Weak<()>)> = self
            .objects
            .registered()
            .filter(|(_, o)| {
                o.is_alive()
                    && o.thing().template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse
            })
            .map(|(id, object)| {
                (
                    *id,
                    std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                )
            })
            .collect();
        for (id, birth) in warehouses {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !birth.ptr_eq(&std::sync::Arc::downgrade(&obj.visual_draw_identity_token)) {
                continue;
            }
            let max_h = obj.health.maximum.max(obj.max_health).max(1.0);
            let cur = obj.health.current;
            // C++ onDamage resets SelfHealSupression. First observation below
            // max means damage already happened (module would be awake).
            let amount = {
                let state =
                    self.warehouse_crippling_states
                        .entry(id)
                        .or_insert(WarehouseCripplingState {
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
                amount
            };
            // The module-state borrow ends before entering the receiving
            // world's synchronous Body/healing callbacks.
            if amount > 0.0 {
                // C++ SupplyWarehouseCripplingBehavior::update attemptHealing.
                // The source argument is NULL in the C++ call.
                self.apply_owned_healing_with_effects(id, amount, None, effects)?;
                // C++ resumes the module update after attemptHealing and reads
                // the body's post-callback health before deciding its cadence.
                let still_original = self.objects.get(&id).is_some_and(|obj| {
                    birth.ptr_eq(&std::sync::Arc::downgrade(&obj.visual_draw_identity_token))
                });
                if still_original {
                    if let Some(state) = self.warehouse_crippling_states.get_mut(&id) {
                        let obj = self.objects.get(&id).expect("validated warehouse birth");
                        state.last_health = obj.health.current;
                    }
                }
            } else if let Some(state) = self.warehouse_crippling_states.get_mut(&id) {
                state.last_health = cur;
            }
        }
        Ok(())
    }

    pub fn drain_warehouse_script_set_values(&mut self) {
        self.drain_warehouse_script_set_values_with_effects(&mut Default::default());
    }

    pub(crate) fn drain_warehouse_script_set_values_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        for (name, cash) in self.drain_warehouse_set_value_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&name) {
                self.apply_warehouse_set_value_to_object_with_effects(id, cash, effects);
            }
        }
    }

    /// C++ `SupplyWarehouseDockUpdate::setCashValue`.
    pub fn apply_warehouse_set_value(&mut self, name: &str, cash: i32) -> bool {
        self.apply_warehouse_set_value_with_effects(name, cash, &mut Default::default())
    }

    pub(crate) fn apply_warehouse_set_value_with_effects(
        &mut self,
        name: &str,
        cash: i32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(id) = self.host_object_id_by_script_name(name) else {
            return false;
        };
        self.apply_warehouse_set_value_to_object_with_effects(id, cash, effects)
    }

    pub(crate) fn apply_warehouse_set_value_to_object_with_effects(
        &mut self,
        id: ObjectId,
        cash: i32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let value_per_box = self.supply_box_value();
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.thing().template.dock_kind != crate::game_logic::DockKind::SupplyWarehouse {
            return false;
        }
        let Some(boxes) = warehouse_boxes_from_cash(cash, value_per_box) else {
            return false;
        };
        obj.set_warehouse_boxes(boxes);
        let max = obj.get_template().dock_starting_boxes.unwrap_or(0);
        self.update_receiving_drawable_supply_status(id, max, boxes, effects);
        true
    }

    /// SupplyTruckAIUpdate.cpp:116–128 and WorkerAIUpdate.cpp:1029–1042.
    /// A deposit or transport diversion drains successful box losses in order.
    pub(crate) fn drain_receiving_supply_boxes(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let Some(carrier) = self.objects.get_mut(&id) else {
            return 0;
        };
        let Some(metadata) = carrier.get_template().supply_truck_metadata else {
            return 0;
        };
        let carried = carrier.carried_supply_boxes();
        if carried <= 0 || effects.view.is_none() {
            // The detached/headless path observes only the final cargo. A
            // negative restored count retains the existing Main normalization.
            if carried != 0 {
                carrier.set_carried_supply_boxes(0);
            }
            return carried.max(0) as u32;
        }
        for remaining in (0..carried).rev() {
            self.objects
                .get_mut(&id)
                .expect("box callback cannot replace its host carrier")
                .set_carried_supply_boxes(remaining);
            self.update_receiving_drawable_supply_status(id, metadata.max_boxes, remaining, effects);
        }
        carried as u32
    }

    /// SupplyWarehouseDockUpdate.cpp:104/158 and SupplyTruckAIUpdate.cpp:125.
    /// The signed gameplay count is already installed before the Drawable call.
    pub(crate) fn update_receiving_drawable_supply_status(
        &mut self,
        id: ObjectId,
        max: i32,
        current: i32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.update_receiving_drawable_supply_status_inner(id, max, current, false, effects);
    }

    /// SupplyWarehouseDockUpdate.cpp:197–208. This is a visual postprocess
    /// of already restored signed stock on the current warehouse Drawable.
    #[cfg(feature = "game_client")]
    pub(crate) fn post_process_restored_supply_warehouses(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        if !self.native_drawable_birth_context().in_load_game {
            return Err("supply postprocess requires a saved snapshot admission".into());
        }
        for id in self.restored_supply_warehouse_ids() {
            let object = &self.objects[&id];
            let maximum = object.get_template().dock_starting_boxes.unwrap_or(0);
            let current = object.warehouse_boxes();
            self.update_receiving_drawable_supply_status_inner(id, maximum, current, true, effects);
        }
        Ok(())
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn restored_supply_warehouse_ids(&self) -> Vec<ObjectId> {
        self.objects.registered().filter_map(|(id, object)| {
            (object.get_template().dock_kind == crate::game_logic::DockKind::SupplyWarehouse)
                .then_some(*id)
        }).collect()
    }

    fn update_receiving_drawable_supply_status_inner(
        &mut self,
        id: ObjectId,
        max: i32,
        current: i32,
        restoring: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        #[cfg(not(feature = "game_client"))]
        let _ = (id, max, current, restoring, effects);
        #[cfg(feature = "game_client")]
        {
            let Some(view) = effects.view.as_deref_mut() else {
                return;
            };
            let Ok(mut source) = self.current_native_slave_source(
                id, self.local_player_id().unwrap_or_default(),
            ) else {
                return;
            };
            let Some(template) = self.templates.get(&source.template_name)
                .and_then(|template| template.receiving_client_template.as_ref())
            else {
                return;
            };
            if restoring {
                // The saved logical Drawable state precedes the module
                // callback and can differ from the Object's projected bits.
                let Ok(Some(bits)) = view.drawable_model_condition_bits(&source) else {
                    return;
                };
                source.condition_bits = bits;
            }
            let carry = 1u128 << crate::game_logic::host_enum_table_residual::carrying_model_bit();
            source.condition_bits = (source.condition_bits & !carry)
                | if current > 0 { carry } else { 0 };
            let accepted = view.update_drawable_supply_status(
                &source, template.receiving_supply_draw_modules(),
                &self.world_services.common().system.runtime_global_data, max, current,
            );
            if accepted
                && let Some(object) = self.objects.get_mut(&id)
            {
                object.model_condition_bits =
                    (object.model_condition_bits & !carry) | (source.condition_bits & carry);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warehouse_action_does_not_debit_when_collector_already_at_max_boxes() {
        // Last remaining box + full collector: C++ take-back, warehouse stays.
        assert_eq!(warehouse_action_transfer_one_box(1, 4, 4), (1, 4, false));
        assert_eq!(warehouse_action_transfer_one_box(10, 4, 4), (10, 4, false));
        // Room for one more: debit and credit.
        assert_eq!(warehouse_action_transfer_one_box(1, 3, 4), (0, 4, true));
        assert_eq!(warehouse_action_transfer_one_box(0, 2, 4), (0, 2, false));
        assert_eq!(warehouse_action_transfer_one_box(-1, 0, 4), (-2, 1, true));
        assert_eq!(warehouse_action_transfer_one_box(-1, 4, 4), (-1, 4, false));
        assert_eq!(warehouse_action_transfer_one_box(1, 0, -1), (1, 0, false));
    }

    #[test]
    fn temporary_stealth_grant_preserves_permanent_stealth() {
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
    fn warehouse_set_value_ceils_boxes_from_cash() {
        assert_eq!(warehouse_boxes_from_cash(75, 75), Some(1));
        assert_eq!(warehouse_boxes_from_cash(76, 75), Some(2));
        assert_eq!(warehouse_boxes_from_cash(0, 75), Some(0));
        assert_eq!(warehouse_boxes_from_cash(-126, 125), Some(-1));
        assert_eq!(warehouse_boxes_from_cash(-1, 125), Some(0));
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
    fn warehouse_self_heal_uses_receiving_body_callback_without_an_emitter() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};

        fn world() -> (GameLogic, ObjectId) {
            let mut logic = GameLogic::new();
            let mut warehouse = ThingTemplate::new("OwnedWarehouse");
            warehouse
                .add_kind_of(KindOf::SupplySource)
                .add_kind_of(KindOf::Structure)
                .set_health(500.0);
            warehouse.dock_kind = crate::game_logic::DockKind::SupplyWarehouse;
            logic.templates.insert(warehouse.name.clone(), warehouse);
            let id = logic
                .create_object("OwnedWarehouse", Team::GLA, Vec3::ZERO)
                .unwrap();
            logic
                .host_object_mut(id)
                .unwrap()
                .internal_change_health(-300.0);
            (logic, id)
        }

        let (mut receiving, id) = world();
        let (peer, peer_id) = world();
        assert_eq!(id, peer_id);
        receiving.frame = 0;
        receiving.update_supply_warehouse_crippling().unwrap();
        receiving.frame = 90;
        receiving.update_supply_warehouse_crippling().unwrap();

        let healed = receiving.host_object(id).unwrap();
        assert!((healed.health.current - 205.0).abs() < 0.01);
        assert_eq!(healed.last_damage_info_type, Some(DamageType::Healing));
        assert_eq!(healed.last_damage_source, None);
        assert_eq!(healed.last_healing_timestamp, Some(90));
        let foreign = peer.host_object(peer_id).unwrap();
        assert_eq!(foreign.health.current, 200.0);
        assert_eq!(foreign.last_damage_info_type, None);
        assert_eq!(foreign.last_healing_timestamp, None);
    }

    #[test]
    fn warehouse_crippling_snapshot_keeps_mid_suppression_cadence() {
        let id = ObjectId(7);
        let mut source = crate::game_logic::GameLogic::new();
        source.restore_warehouse_crippling_states(vec![(
            id,
            WarehouseCripplingState {
                last_health: 200.0,
                healing_suppressed_until_frame: 100,
                next_healing_frame: 100,
            },
        )]);
        let snap = source.snapshot_warehouse_crippling_states();
        let mut other = crate::game_logic::GameLogic::new();
        other.restore_warehouse_crippling_states(vec![(
            ObjectId(99),
            WarehouseCripplingState {
                last_health: 1.0,
                healing_suppressed_until_frame: 1,
                next_healing_frame: 1,
            },
        )]);
        source.restore_warehouse_crippling_states(snap);
        let states = source.snapshot_warehouse_crippling_states();
        assert_eq!(
            other.snapshot_warehouse_crippling_states()[0].0,
            ObjectId(99)
        );
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
        let mut queues = HostDockApproachQueues::default();
        let dock = ObjectId(21);
        let a = ObjectId(22);
        let bone = Vec3::new(10.0, 0.0, 0.0);
        let tick = queues.tick(
            dock,
            a,
            0,
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
            false,
            |_| true,
            |_| false,
        );
        assert_eq!(tick, DockApproachTick::Blocked);
    }

    #[test]
    fn evict_dead_clears_ghost_reservation() {
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
        let mut queues = HostDockApproachQueues::default();
        let dock = ObjectId(1);
        let a = ObjectId(2);
        let bone = Vec3::new(40.0, 0.0, 0.0);
        let tick = queues.tick(
            dock,
            a,
            0,
            5,
            true,
            None,
            false,
            Vec3::new(200.0, 0.0, 0.0),
            Vec3::ZERO,
            20.0,
            &[bone],
            0,
            false,
            false,
            |_| true,
            |_| false,
        );
        assert_eq!(tick, DockApproachTick::PathTo(bone));
        let arrived = queues.tick(
            dock,
            a,
            0,
            5,
            true,
            None,
            false,
            bone,
            Vec3::ZERO,
            20.0,
            &[bone],
            1,
            false,
            false,
            |_| true,
            |_| false,
        );
        assert_eq!(arrived, DockApproachTick::ClearToAct);
    }

    #[test]
    fn evicting_queue_slot_preserves_epoch_for_docker_active_at_another_dock() {
        let mut queues = HostDockApproachQueues::default();
        let dock = ObjectId(30);
        let active_dock = ObjectId(31);
        let former_waiter = ObjectId(32);
        let new_waiter = ObjectId(33);
        let point = Vec3::ZERO;

        assert_eq!(
            queues.tick(
                dock,
                former_waiter,
                7,
                2,
                true,
                None,
                false,
                Vec3::new(100.0, 0.0, 0.0),
                point,
                20.0,
                &[point],
                0,
                false,
                false,
                |_| true,
                |_| false,
            ),
            DockApproachTick::PathTo(point)
        );
        assert!(queues.tracks_docker(former_waiter));

        // The former waiter is no longer live in this queue, but the receiving
        // owner's canonical dock fields still identify it as active elsewhere.
        let result = queues.tick(
            dock,
            new_waiter,
            2,
            2,
            true,
            None,
            false,
            point,
            point,
            20.0,
            &[point],
            1,
            false,
            false,
            |id| id == new_waiter,
            |id| id == former_waiter,
        );
        assert_eq!(result, DockApproachTick::ClearToAct);
        assert!(queues.tracks_docker(former_waiter));

        queues.reconcile_cancel_epochs(&[(former_waiter, None)], |_| vec![active_dock]);
        assert_eq!(
            queues.take_pending_cancel_effects(),
            vec![(active_dock, former_waiter)]
        );
    }

    #[test]
    fn active_identity_is_observed_from_dock_owner_after_approach_slot_frees() {
        let mut queues = HostDockApproachQueues::default();
        let dock = ObjectId(31);
        let docker = ObjectId(32);
        let point = Vec3::ZERO;
        assert_eq!(
            queues.tick(
                dock,
                docker,
                4,
                1,
                true,
                None,
                false,
                point,
                point,
                20.0,
                &[point],
                10,
                false,
                false,
                |_| true,
                |_| false,
            ),
            DockApproachTick::ClearToAct
        );
        assert_eq!(queues.queues[&dock].owners, vec![None]);
        assert!(queues.tracked_docker_ids().contains(&docker));

        // The receiving GameLogic supplies the canonical Object field here at
        // its existing cancellation observation point, matching C++ where the
        // approach slot is released but DockUpdate::m_activeDocker remains.
        queues.reconcile_cancel_epochs(&[(docker, None)], |_| vec![dock]);
        assert_eq!(queues.take_pending_cancel_effects(), vec![(dock, docker)]);
        assert!(!queues.tracks_docker(docker));
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
        let mut queues = HostDockApproachQueues::default();
        let dock = ObjectId(3);
        let a = ObjectId(4);
        let b = ObjectId(5);
        let bone_a = Vec3::new(10.0, 0.0, 0.0);
        let bone_b = Vec3::new(20.0, 0.0, 0.0);
        assert_eq!(
            queues.tick(
                dock,
                a,
                0,
                5,
                true,
                None,
                false,
                bone_a,
                Vec3::ZERO,
                10.0,
                &[bone_a, bone_b],
                0,
                false,
                false,
                |_| true,
                |_| false,
            ),
            DockApproachTick::ClearToAct
        );
        assert_eq!(
            queues.tick(
                dock,
                b,
                0,
                5,
                true,
                Some(a),
                true,
                bone_b,
                Vec3::ZERO,
                10.0,
                &[bone_a, bone_b],
                10,
                false,
                false,
                |_| true,
                |_| false,
            ),
            DockApproachTick::Blocked
        );
        assert_eq!(
            queues.tick(
                dock,
                b,
                0,
                5,
                true,
                Some(a),
                true,
                bone_b,
                Vec3::ZERO,
                10.0,
                &[bone_a, bone_b],
                10 + WAIT_FOR_CLEARANCE_FRAMES,
                false,
                false,
                |_| true,
                |_| false,
            ),
            DockApproachTick::TimedOut
        );
        assert!(queues.is_clear_to_approach(dock, b));
    }

    #[test]
    fn death_or_retask_cancels_all_reservations() {
        let mut queues = HostDockApproachQueues::default();
        let dock = ObjectId(6);
        let a = ObjectId(7);
        let _ = queues.tick(
            dock,
            a,
            0,
            5,
            true,
            Some(ObjectId(8)),
            true,
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::ZERO,
            20.0,
            &[],
            0,
            false,
            false,
            |_| true,
            |_| false,
        );
        queues.cancel_docker(a);
        assert!(queues.is_clear_to_approach(dock, ObjectId(9)));
    }
}
