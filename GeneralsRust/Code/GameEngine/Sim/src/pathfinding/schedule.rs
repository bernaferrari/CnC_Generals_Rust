//! Ordered path requests and per-frame search scheduling for a receiving world.

use generals_game_domain::ObjectId;
use glam::Vec3;
use std::collections::VecDeque;

/// Original Pathfinder ring size (AIPathfind.h).
pub const PATHFIND_QUEUE_LEN: usize = 512;
/// C++ reserves one ring slot to distinguish an empty queue from a full one.
pub const MAX_PENDING_PATH_REQUESTS: usize = PATHFIND_QUEUE_LEN - 1;
/// Original per-frame search budget (AIPathfind.cpp).
pub const PATHFIND_CELLS_PER_FRAME: usize = 5000;
const ZONE_UPDATE_FREQUENCY: u64 = 300;

/// Detached path request view used at admission, dispatch, and save/load.
/// The live queue retains only its ID and transitional route continuation.
#[derive(Debug, Clone)]
pub struct PathRequest {
    pub unit_id: ObjectId,
    pub start: Vec3,
    pub destination: Vec3,
    pub waypoints: Vec<Vec3>,
    /// FollowPath orders admitted while this first search is still pending.
    /// They are handed to the actor after the search finishes, in order.
    pub follow_path_appends: Vec<Vec3>,
    pub aircraft: bool,
    pub surfaces: u32,
    /// C++ `obj->getCrusherLevel() > 0` (AIPathfind.cpp:8170).
    pub is_crusher: bool,
    /// C++ `AIUpdateInterface::ignoreObstacle` (DozerAIUpdate.cpp:210).
    pub ignore_obstacle: Option<ObjectId>,
    /// Actor's destination-adjustment setting at observation time.
    pub adjust_destinations: bool,
}

/// Queue-owned ordering and transitional route data. Search inputs belong to
/// the actor and are observed when its ID is dispatched.
#[derive(Debug, Clone)]
pub struct QueuedPath {
    pub unit_id: ObjectId,
    pub waypoints: Vec<Vec3>,
    pub follow_path_appends: Vec<Vec3>,
}

impl From<PathRequest> for QueuedPath {
    fn from(request: PathRequest) -> Self {
        Self {
            unit_id: request.unit_id,
            waypoints: request.waypoints,
            follow_path_appends: request.follow_path_appends,
        }
    }
}

/// The receiving world's path queue, search budget and zone rebuild schedule.
#[derive(Debug)]
pub struct PathSchedule {
    requests: VecDeque<QueuedPath>,
    logic_frame: u64,
    next_zone_rebuild: u64,
    cells_allocated: i32,
    cell_budget: i32,
}

impl Default for PathSchedule {
    fn default() -> Self {
        Self {
            requests: VecDeque::new(),
            logic_frame: 0,
            next_zone_rebuild: u64::MAX,
            cells_allocated: 0,
            cell_budget: PATHFIND_CELLS_PER_FRAME as i32,
        }
    }
}

impl PathSchedule {
    pub fn note_logic_frame(&mut self, frame: u64) {
        self.logic_frame = frame;
    }

    /// C++ PathfindZoneManager::markZonesDirty coalesces later changes at the
    /// earliest deadline. Fresh maps first rebuild on frame two.
    pub fn mark_zones_dirty(&mut self) {
        if self.logic_frame < 2 {
            self.next_zone_rebuild = 2;
            return;
        }
        self.next_zone_rebuild = self
            .next_zone_rebuild
            .min(self.logic_frame + ZONE_UPDATE_FREQUENCY);
    }

    /// A due rebuild uses the queue phase for this frame.
    pub fn calculate_zones_if_due(&mut self, rebuild: impl FnOnce()) -> bool {
        if self.next_zone_rebuild > self.logic_frame {
            return false;
        }
        rebuild();
        self.clear_zone_deadline();
        true
    }

    pub fn clear_zone_deadline(&mut self) {
        self.next_zone_rebuild = u64::MAX;
    }

    pub fn begin_frame(&mut self) {
        self.cells_allocated = 0;
    }

    pub fn budget_remaining(&self) -> bool {
        self.cells_allocated < self.cell_budget
    }

    pub fn charge(&mut self, cells: usize) {
        self.cells_allocated = self
            .cells_allocated
            .saturating_add(cells.min(i32::MAX as usize) as i32);
    }

    pub fn set_cell_budget(&mut self, cells: i32) {
        self.cell_budget = cells.max(1);
    }

    pub fn cells_allocated(&self) -> i32 {
        self.cells_allocated
    }

    /// A new order replaces the stored continuation in place. AI request
    /// inputs are observed on dispatch, as in Pathfinder::queueForPath.
    pub fn queue(&mut self, request: PathRequest) -> bool {
        if let Some(existing) = self
            .requests
            .iter_mut()
            .find(|entry| entry.unit_id == request.unit_id)
        {
            *existing = request.into();
            return true;
        }
        if self.requests.len() >= MAX_PENDING_PATH_REQUESTS {
            return false;
        }
        self.requests.push_back(request.into());
        true
    }

    /// A retry preserves any continuation already queued for this actor.
    pub fn queue_retry(&mut self, request: PathRequest) -> bool {
        self.requests
            .iter()
            .any(|entry| entry.unit_id == request.unit_id)
            || self.queue(request)
    }

    pub fn append_follow_path(&mut self, unit: ObjectId, goal: Vec3) -> bool {
        let Some(request) = self.requests.iter_mut().find(|entry| entry.unit_id == unit) else {
            return false;
        };
        request.follow_path_appends.push(goal);
        true
    }

    pub fn pop(&mut self) -> Option<QueuedPath> {
        self.requests.pop_front()
    }

    pub fn drain(&mut self) -> impl Iterator<Item = QueuedPath> + '_ {
        self.requests.drain(..)
    }

    pub fn clear_requests(&mut self) {
        self.requests.clear();
    }

    pub fn request_count(&self) -> usize {
        self.requests.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &QueuedPath> {
        self.requests.iter()
    }
}
