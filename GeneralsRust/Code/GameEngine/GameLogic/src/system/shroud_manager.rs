//! Per-instance shroud grids, reveal queues and cached object visibility.
//!
//! The grid counters and pending reveal queue follow C++ PartitionManager.cpp.
//! Object.cpp::look supplies shroud-clearing ranges and allied/spied player masks;
//! PartitionManager.cpp::isClearLineOfSightTerrain derives collision-top eye positions.
//!
//! Object visibility caching is a Rust integration layer with its own update and
//! recalculation intervals, measured in logic frames. Those intervals and its opaque
//! structure sampler are not evidence of original C++ visibility-policy parity.
//! The driving world owns the manager through EngineStores; the engine-lifetime
//! fallback accessor remains an ownership-migration dependency.

use crate::common::{Coord3D, KindOf, ObjectID, ObjectShroudStatus};
use crate::object_manager::get_object_manager;
use crate::player::PLAYER_INDEX_INVALID;
use crate::weapon::WeaponStore;
use game_engine::common::system::radar::{CellShroudStatus, get_radar_system};
use log::{debug, trace};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

/// Maximum number of players in game
const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

/// Default frame interval between visibility updates (reduce per-frame cost)
const DEFAULT_UPDATE_INTERVAL: u32 = 2;

/// Default frame interval for full vision recalculation (every 10 frames as required)
const VISION_RECALC_INTERVAL: u32 = 10;

/// Grid-based shroud cell size in world units (C++ PartitionCellSize = 40).
const SHROUD_GRID_CELL_SIZE: f32 = 40.0;

/// Persistent C++ `PartitionCell::ShroudLevel` payload.
///
/// The public snapshot deliberately carries both counters rather than the
/// derived Hidden/Explored/Visible status.  The C++ save path transfers the
/// raw `currentShroud` and `activeShroudLevel` values, and restoring only the
/// derived status loses overlapping lookers and active shroud generators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShroudCellSnapshot {
    pub current_shroud: [i32; MAX_PLAYER_COUNT],
    pub active_shroud_level: [i32; MAX_PLAYER_COUNT],
}

impl Default for ShroudCellSnapshot {
    fn default() -> Self {
        Self {
            current_shroud: [1; MAX_PLAYER_COUNT],
            active_shroud_level: [0; MAX_PLAYER_COUNT],
        }
    }
}

/// Persistent `PartitionManager::SightingInfo` queue record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShroudPendingUndoRevealSnapshot {
    pub where_pos: [f32; 3],
    pub how_far: f32,
    pub for_whom: PlayerMask,
    pub expiration_frame: u32,
}

impl Default for ShroudPendingUndoRevealSnapshot {
    fn default() -> Self {
        Self {
            where_pos: [0.0; 3],
            how_far: 0.0,
            for_whom: 0,
            expiration_frame: 0,
        }
    }
}

/// Exact persistent shroud/FOW state transferred by a world save.
///
/// `grid` is `None` when the map has not initialized a partition grid yet.
/// The cells retain the raw per-player counters, while the pending queue
/// retains reveal expiry frames.  Object visibility sets and update caches
/// are intentionally omitted: they are derived runtime state and are rebuilt
/// after installation, matching C++ `PartitionManager::loadPostProcess`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShroudSnapshot {
    pub grid: Option<ShroudGridSnapshot>,
    pub pending_undo_shroud_reveals: Vec<ShroudPendingUndoRevealSnapshot>,
    pub pending_full_reveal_players: Vec<u32>,
    pub pending_permanent_reveal_players: Vec<u32>,
}

impl Default for ShroudSnapshot {
    fn default() -> Self {
        Self {
            grid: None,
            pending_undo_shroud_reveals: Vec::new(),
            pending_full_reveal_players: Vec::new(),
            pending_permanent_reveal_players: Vec::new(),
        }
    }
}

/// Exact persistent grid dimensions/cell payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShroudGridSnapshot {
    pub width: u32,
    pub height: u32,
    pub cell_size: f32,
    pub cells: Vec<ShroudCellSnapshot>,
}

impl Default for ShroudGridSnapshot {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            cell_size: SHROUD_GRID_CELL_SIZE,
            cells: Vec::new(),
        }
    }
}

/// Shroud visibility state for grid cells
/// Matches C++ CellShroudStatus enum from PartitionManager.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShroudState {
    /// Never seen - completely black (CELLSHROUD_SHROUDED)
    Hidden = 0,
    /// Explored but not currently visible - darkened/fogged (CELLSHROUD_FOGGED)
    Explored = 1,
    /// Currently visible - bright/clear (CELLSHROUD_CLEAR)
    Visible = 2,
}

impl Default for ShroudState {
    fn default() -> Self {
        ShroudState::Hidden
    }
}

/// Per-player shroud level information for a cell
/// Matches C++ PartitionCell::ShroudLevel from PartitionManager.cpp
#[derive(Debug, Clone, Copy)]
struct CellShroudLevel {
    /// Current shroud state counter
    /// - Negative values (-N): N units are looking at this cell (CLEAR)
    /// - 0: No active lookers, but explored (FOGGED)
    /// - 1: Never explored (SHROUDED)
    /// Matches C++ m_currentShroud
    current_shroud: i32,

    /// Active shroud level (passive shroud generators)
    /// Used for special abilities that create fog (e.g., stealth generators)
    /// Matches C++ m_activeShroudLevel
    active_shroud_level: i32,
}

impl Default for CellShroudLevel {
    fn default() -> Self {
        Self {
            current_shroud: 1, // Start as SHROUDED
            active_shroud_level: 0,
        }
    }
}

impl CellShroudLevel {
    /// Get the shroud status for this cell
    /// Matches C++ PartitionCell::getShroudStatusForPlayer()
    fn get_shroud_status(&self) -> ShroudState {
        if self.current_shroud == 1 {
            ShroudState::Hidden // CELLSHROUD_SHROUDED
        } else if self.current_shroud == 0 {
            ShroudState::Explored // CELLSHROUD_FOGGED (explored but not visible)
        } else {
            ShroudState::Visible // CELLSHROUD_CLEAR (actively being looked at)
        }
    }

    /// Add a looker to this cell
    /// Matches C++ PartitionCell::addLooker() from lines 1272-1300
    fn add_looker(&mut self) -> (ShroudState, ShroudState) {
        let old_status = self.get_shroud_status();

        // The decreasing algorithm: A 1 will go straight to -1, otherwise just decrement
        self.current_shroud = std::cmp::min(self.current_shroud - 1, -1);

        let new_status = self.get_shroud_status();
        (old_status, new_status)
    }

    /// Remove a looker from this cell
    /// Matches C++ PartitionCell::removeLooker() from lines 1303-1336
    fn remove_looker(&mut self) -> (ShroudState, ShroudState) {
        let old_status = self.get_shroud_status();

        // The increasing algorithm: -1 goes to min(1, activeLevel), otherwise increment
        if self.current_shroud == -1 {
            self.current_shroud = std::cmp::min(self.active_shroud_level, 1);
        } else {
            // In debug mode, C++ asserts current_shroud < 0
            // We'll just clamp to prevent errors in release mode
            self.current_shroud = std::cmp::min(self.current_shroud + 1, 1);
        }

        let new_status = self.get_shroud_status();
        (old_status, new_status)
    }

    /// Add active shrouder to this cell (passive fog generation)
    /// Matches C++ PartitionCell::addShrouder() from lines 1339-1363
    fn add_shrouder(&mut self) -> (ShroudState, ShroudState) {
        let old_status = self.get_shroud_status();

        // Increasing active shroud: increment activeLevel, set CS to 1 if at zero
        self.active_shroud_level += 1;
        if self.current_shroud == 0 {
            self.current_shroud = 1;
        }

        let new_status = self.get_shroud_status();
        (old_status, new_status)
    }

    /// Remove active shrouder from this cell
    /// Matches C++ PartitionCell::removeShrouder() from lines 1366-1372
    fn remove_shrouder(&mut self) {
        // Decreasing active shroud: just decrement activeLevel
        // This never results in a client change
        self.active_shroud_level = std::cmp::max(self.active_shroud_level - 1, 0);
    }
}

/// Partition cell with counter-based shroud tracking
/// Matches C++ PartitionCell from PartitionManager.cpp
#[derive(Debug, Clone)]
struct PartitionCell {
    /// Shroud levels for each player (indexed by player ID)
    shroud_levels: [CellShroudLevel; MAX_PLAYER_COUNT],

    /// Threat value per player (for AI targeting)
    /// Matches C++ m_threatValue[MAX_PLAYER_COUNT]
    threat_values: [u32; MAX_PLAYER_COUNT],

    /// Cash value per player (for AI resource tracking)
    /// Matches C++ m_cashValue[MAX_PLAYER_COUNT]
    cash_values: [u32; MAX_PLAYER_COUNT],
}

impl Default for PartitionCell {
    fn default() -> Self {
        Self {
            shroud_levels: [CellShroudLevel::default(); MAX_PLAYER_COUNT],
            threat_values: [0; MAX_PLAYER_COUNT],
            cash_values: [0; MAX_PLAYER_COUNT],
        }
    }
}

impl PartitionCell {
    /// Get shroud status for a specific player
    fn get_shroud_status(&self, player_id: usize) -> ShroudState {
        if player_id >= MAX_PLAYER_COUNT {
            return ShroudState::Hidden;
        }
        self.shroud_levels[player_id].get_shroud_status()
    }

    /// Add looker for a player
    fn add_looker(&mut self, player_id: usize) -> bool {
        if player_id >= MAX_PLAYER_COUNT {
            return false;
        }
        let (old_status, new_status) = self.shroud_levels[player_id].add_looker();
        old_status != new_status
    }

    /// Remove looker for a player
    fn remove_looker(&mut self, player_id: usize) -> bool {
        if player_id >= MAX_PLAYER_COUNT {
            return false;
        }
        let (old_status, new_status) = self.shroud_levels[player_id].remove_looker();
        old_status != new_status
    }

    /// Add shrouder for a player
    fn add_shrouder(&mut self, player_id: usize) -> bool {
        if player_id >= MAX_PLAYER_COUNT {
            return false;
        }
        let (old_status, new_status) = self.shroud_levels[player_id].add_shrouder();
        old_status != new_status
    }

    /// Remove shrouder for a player
    fn remove_shrouder(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }
        self.shroud_levels[player_id].remove_shrouder();
    }

    /// Reveal this cell for a player (clears shroud while respecting active shrouders).
    fn reveal_for_player(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }
        let level = &mut self.shroud_levels[player_id];
        if level.current_shroud > 0 {
            if level.active_shroud_level > 0 {
                level.current_shroud = 1;
            } else {
                level.current_shroud = 0;
            }
        }
    }

    /// Get threat value for player
    fn get_threat_value(&self, player_id: usize) -> u32 {
        if player_id >= MAX_PLAYER_COUNT {
            return 0;
        }
        self.threat_values[player_id]
    }

    /// Add threat value for player
    fn add_threat_value(&mut self, player_id: usize, value: u32) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }
        self.threat_values[player_id] = self.threat_values[player_id].saturating_add(value);
    }

    /// Remove threat value for player
    fn remove_threat_value(&mut self, player_id: usize, value: u32) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }
        self.threat_values[player_id] = self.threat_values[player_id].saturating_sub(value);
    }

    /// Get cash value for player
    fn get_cash_value(&self, player_id: usize) -> u32 {
        if player_id >= MAX_PLAYER_COUNT {
            return 0;
        }
        self.cash_values[player_id]
    }

    /// Add cash value for player
    fn add_cash_value(&mut self, player_id: usize, value: u32) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }
        self.cash_values[player_id] = self.cash_values[player_id].saturating_add(value);
    }

    /// Remove cash value for player
    fn remove_cash_value(&mut self, player_id: usize, value: u32) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }
        self.cash_values[player_id] = self.cash_values[player_id].saturating_sub(value);
    }
}

/// Grid-based shroud tracking for spatial queries
/// Matches C++ PartitionManager grid structure
#[derive(Debug, Clone)]
struct ShroudGrid {
    /// Admitted map lower corner on the C++ XY ground plane; runtime metadata,
    /// reconstructed from the map before restoring the serialized cell array.
    world_origin_xy: [f32; 2],
    /// Grid dimensions (cells)
    width: usize,
    height: usize,
    /// Cell size in world units
    cell_size: f32,
    /// Grid of partition cells
    cells: Vec<PartitionCell>,
}

impl ShroudGrid {
    fn new(world_origin_xy: [f32; 2], map_width: f32, map_height: f32, cell_size: f32) -> Self {
        let width = ((map_width / cell_size).ceil() as usize).max(1);
        let height = ((map_height / cell_size).ceil() as usize).max(1);
        let total_cells = width * height;

        let cells = vec![PartitionCell::default(); total_cells];

        Self {
            world_origin_xy,
            width,
            height,
            cell_size,
            cells,
        }
    }

    /// Convert world position to grid coordinates
    fn world_to_grid(&self, pos: &Coord3D) -> Option<(usize, usize)> {
        let (x, y) = self.world_to_signed_cell(pos);

        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            Some((x as usize, y as usize))
        } else {
            None
        }
    }

    /// C++ PartitionManager.h:1506 subtracts the map extent before flooring.
    /// Keep signed off-map centers so circle spans can clip at the boundary.
    fn world_to_signed_cell(&self, pos: &Coord3D) -> (i32, i32) {
        (
            ((pos.x - self.world_origin_xy[0]) / self.cell_size).floor() as i32,
            ((pos.y - self.world_origin_xy[1]) / self.cell_size).floor() as i32,
        )
    }

    /// Convert world distance to cell distance
    fn world_to_cell_dist(&self, world_dist: f32) -> i32 {
        ((world_dist / self.cell_size).ceil() as i32).max(1)
    }

    /// Get cell at grid coordinates
    fn get_cell(&self, x: usize, y: usize) -> Option<&PartitionCell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get(y * self.width + x)
    }

    /// Get mutable cell at grid coordinates
    fn get_cell_mut(&mut self, x: usize, y: usize) -> Option<&mut PartitionCell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y * self.width + x;
        self.cells.get_mut(index)
    }

    /// Get state for a grid cell and player
    fn get_cell_state(&self, player_id: usize, x: usize, y: usize) -> ShroudState {
        self.get_cell(x, y)
            .map(|cell| cell.get_shroud_status(player_id))
            .unwrap_or(ShroudState::Hidden)
    }

    /// Check if a position is visible to a player
    fn is_position_visible(&self, player_id: usize, pos: &Coord3D) -> bool {
        if let Some((x, y)) = self.world_to_grid(pos) {
            self.get_cell_state(player_id, x, y) == ShroudState::Visible
        } else {
            false
        }
    }

    /// Check if a position has been explored by a player
    fn is_position_explored(&self, player_id: usize, pos: &Coord3D) -> bool {
        if let Some((x, y)) = self.world_to_grid(pos) {
            let state = self.get_cell_state(player_id, x, y);
            state == ShroudState::Explored || state == ShroudState::Visible
        } else {
            false
        }
    }

    /// Reveal a circular area for a player using DiscreteCircle algorithm
    /// Matches C++ PartitionManager::doShroudReveal() from lines 3969-3990
    fn do_shroud_reveal(&mut self, center: &Coord3D, radius: f32, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        // C++ draws around a signed center even outside the map; each
        // horizontal span clips against the grid bounds below.
        let (center_x, center_y) = self.world_to_signed_cell(center);

        let cell_radius = self.world_to_cell_dist(radius);

        // Use DiscreteCircle algorithm to add lookers to all cells in the circle
        let circle = DiscreteCircle::new(center_x, center_y, cell_radius);
        for line in circle.edges() {
            self.add_looker_horizontal_line(line.x_start, line.x_end, line.y_pos, player_id);
            // Draw bottom half if not at center
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.add_looker_horizontal_line(line.x_start, line.x_end, y_bottom, player_id);
            }
        }
    }

    /// Add shroud cover (active shroud) to a circular area for a player.
    /// Matches C++ PartitionManager::doShroudCover() from lines 4041-4061
    fn do_shroud_cover(&mut self, center: &Coord3D, radius: f32, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        // C++ draws around a signed center even outside the map; each
        // horizontal span clips against the grid bounds below.
        let (center_x, center_y) = self.world_to_signed_cell(center);

        let cell_radius = self.world_to_cell_dist(radius);

        let circle = DiscreteCircle::new(center_x, center_y, cell_radius);
        for line in circle.edges() {
            self.add_shrouder_horizontal_line(line.x_start, line.x_end, line.y_pos, player_id);
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.add_shrouder_horizontal_line(line.x_start, line.x_end, y_bottom, player_id);
            }
        }
    }

    /// Reveal the entire map for a player (clears shroud, keeps fog).
    /// Matches C++ PartitionManager::revealMapForPlayer.
    fn reveal_map_for_player(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        for cell in &mut self.cells {
            cell.add_looker(player_id);
            cell.remove_looker(player_id);
        }
    }

    /// Reveal the entire map for a player permanently (disables shroud generation).
    /// Matches C++ PartitionManager::revealMapForPlayerPermanently.
    fn reveal_map_for_player_permanently(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        for cell in &mut self.cells {
            cell.add_looker(player_id);
        }
    }

    /// Undo a permanent map reveal for a player.
    /// Matches C++ PartitionManager::undoRevealMapForPlayerPermanently.
    fn undo_reveal_map_for_player_permanently(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        for cell in &mut self.cells {
            cell.remove_looker(player_id);
        }
    }

    /// Shroud the entire map for a player (set all cells to fully hidden).
    /// Matches C++ PartitionManager::shroudMapForPlayer.
    fn shroud_map_for_player(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        for cell in &mut self.cells {
            cell.add_shrouder(player_id);
            cell.remove_shrouder(player_id);
        }
    }

    /// Undo reveal of a circular area for a player
    /// Matches C++ PartitionManager::undoShroudReveal() from lines 4036-4055
    fn undo_shroud_reveal(&mut self, center: &Coord3D, radius: f32, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        // C++ draws around a signed center even outside the map; each
        // horizontal span clips against the grid bounds below.
        let (center_x, center_y) = self.world_to_signed_cell(center);

        let cell_radius = self.world_to_cell_dist(radius);

        // Use DiscreteCircle algorithm to remove lookers from all cells in the circle
        let circle = DiscreteCircle::new(center_x, center_y, cell_radius);
        for line in circle.edges() {
            self.remove_looker_horizontal_line(line.x_start, line.x_end, line.y_pos, player_id);
            // Draw bottom half if not at center
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.remove_looker_horizontal_line(line.x_start, line.x_end, y_bottom, player_id);
            }
        }
    }

    /// Remove shroud cover (active shroud) from a circular area for a player.
    /// Matches C++ PartitionManager::undoShroudCover() from lines 4065-4085
    fn undo_shroud_cover(&mut self, center: &Coord3D, radius: f32, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        // C++ draws around a signed center even outside the map; each
        // horizontal span clips against the grid bounds below.
        let (center_x, center_y) = self.world_to_signed_cell(center);

        let cell_radius = self.world_to_cell_dist(radius);

        let circle = DiscreteCircle::new(center_x, center_y, cell_radius);
        for line in circle.edges() {
            self.remove_shrouder_horizontal_line(line.x_start, line.x_end, line.y_pos, player_id);
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.remove_shrouder_horizontal_line(line.x_start, line.x_end, y_bottom, player_id);
            }
        }
    }

    /// Apply threat influence with radial falloff for a player.
    /// Matches C++ PartitionManager::doThreatAffect().
    fn do_threat_affect(
        &mut self,
        center: &Coord3D,
        radius: f32,
        threat_value: u32,
        player_id: usize,
    ) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        let (center_x, center_y) = match self.world_to_grid(center) {
            Some(coords) => coords,
            None => return,
        };

        let cell_radius = self.world_to_cell_dist(radius).max(1);
        let influence_radius = (cell_radius + 1) as f32;
        let center_x_f = center_x as f32;
        let center_y_f = center_y as f32;

        let circle = DiscreteCircle::new(center_x as i32, center_y as i32, cell_radius);
        for line in circle.edges() {
            self.add_threat_horizontal_line(
                line.x_start,
                line.x_end,
                line.y_pos,
                player_id,
                threat_value,
                center_x_f,
                center_y_f,
                influence_radius,
            );
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.add_threat_horizontal_line(
                    line.x_start,
                    line.x_end,
                    y_bottom,
                    player_id,
                    threat_value,
                    center_x_f,
                    center_y_f,
                    influence_radius,
                );
            }
        }
    }

    /// Remove threat influence with radial falloff for a player.
    /// Matches C++ PartitionManager::undoThreatAffect().
    fn undo_threat_affect(
        &mut self,
        center: &Coord3D,
        radius: f32,
        threat_value: u32,
        player_id: usize,
    ) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        let (center_x, center_y) = match self.world_to_grid(center) {
            Some(coords) => coords,
            None => return,
        };

        let cell_radius = self.world_to_cell_dist(radius).max(1);
        let influence_radius = (cell_radius + 1) as f32;
        let center_x_f = center_x as f32;
        let center_y_f = center_y as f32;

        let circle = DiscreteCircle::new(center_x as i32, center_y as i32, cell_radius);
        for line in circle.edges() {
            self.remove_threat_horizontal_line(
                line.x_start,
                line.x_end,
                line.y_pos,
                player_id,
                threat_value,
                center_x_f,
                center_y_f,
                influence_radius,
            );
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.remove_threat_horizontal_line(
                    line.x_start,
                    line.x_end,
                    y_bottom,
                    player_id,
                    threat_value,
                    center_x_f,
                    center_y_f,
                    influence_radius,
                );
            }
        }
    }

    /// Apply cash/value influence with radial falloff for a player.
    /// Matches C++ PartitionManager::doValueAffect().
    fn do_value_affect(&mut self, center: &Coord3D, radius: f32, value: u32, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        let (center_x, center_y) = match self.world_to_grid(center) {
            Some(coords) => coords,
            None => return,
        };

        let cell_radius = self.world_to_cell_dist(radius).max(1);
        let influence_radius = (cell_radius + 1) as f32;
        let center_x_f = center_x as f32;
        let center_y_f = center_y as f32;

        let circle = DiscreteCircle::new(center_x as i32, center_y as i32, cell_radius);
        for line in circle.edges() {
            self.add_value_horizontal_line(
                line.x_start,
                line.x_end,
                line.y_pos,
                player_id,
                value,
                center_x_f,
                center_y_f,
                influence_radius,
            );
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.add_value_horizontal_line(
                    line.x_start,
                    line.x_end,
                    y_bottom,
                    player_id,
                    value,
                    center_x_f,
                    center_y_f,
                    influence_radius,
                );
            }
        }
    }

    /// Remove cash/value influence with radial falloff for a player.
    /// Matches C++ PartitionManager::undoValueAffect().
    fn undo_value_affect(&mut self, center: &Coord3D, radius: f32, value: u32, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        let (center_x, center_y) = match self.world_to_grid(center) {
            Some(coords) => coords,
            None => return,
        };

        let cell_radius = self.world_to_cell_dist(radius).max(1);
        let influence_radius = (cell_radius + 1) as f32;
        let center_x_f = center_x as f32;
        let center_y_f = center_y as f32;

        let circle = DiscreteCircle::new(center_x as i32, center_y as i32, cell_radius);
        for line in circle.edges() {
            self.remove_value_horizontal_line(
                line.x_start,
                line.x_end,
                line.y_pos,
                player_id,
                value,
                center_x_f,
                center_y_f,
                influence_radius,
            );
            if line.y_pos != circle.y_center() {
                let y_bottom = circle.y_center_doubled() - line.y_pos;
                self.remove_value_horizontal_line(
                    line.x_start,
                    line.x_end,
                    y_bottom,
                    player_id,
                    value,
                    center_x_f,
                    center_y_f,
                    influence_radius,
                );
            }
        }
    }

    fn scaled_affect_amount(
        x: i32,
        y: i32,
        center_x: f32,
        center_y: f32,
        radius: f32,
        base_value: u32,
    ) -> u32 {
        if radius <= 0.0 || base_value == 0 {
            return 0;
        }

        let dx = x as f32 - center_x;
        let dy = y as f32 - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        let mul = (1.0 - distance / radius).clamp(0.0, 1.0);
        let scaled = (base_value as f32) * mul;
        if scaled <= 0.0 { 0 } else { scaled as u32 }
    }

    /// Add looker to a horizontal line of cells
    fn add_looker_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 || x_start >= self.width as i32 || x_end < 0 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                cell.add_looker(player_id);
            }
        }
    }

    /// Add shrouder to a horizontal line of cells
    fn add_shrouder_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 || x_start >= self.width as i32 || x_end < 0 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                cell.add_shrouder(player_id);
            }
        }
    }

    /// Remove looker from a horizontal line of cells
    fn remove_looker_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 || x_start >= self.width as i32 || x_end < 0 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                cell.remove_looker(player_id);
            }
        }
    }

    /// Remove shrouder from a horizontal line of cells
    fn remove_shrouder_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 || x_start >= self.width as i32 || x_end < 0 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                cell.remove_shrouder(player_id);
            }
        }
    }

    fn add_threat_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
        threat_value: u32,
        center_x: f32,
        center_y: f32,
        radius: f32,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                let amount = Self::scaled_affect_amount(
                    x as i32,
                    y_pos,
                    center_x,
                    center_y,
                    radius,
                    threat_value,
                );
                if amount > 0 {
                    cell.add_threat_value(player_id, amount);
                }
            }
        }
    }

    fn remove_threat_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
        threat_value: u32,
        center_x: f32,
        center_y: f32,
        radius: f32,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                let amount = Self::scaled_affect_amount(
                    x as i32,
                    y_pos,
                    center_x,
                    center_y,
                    radius,
                    threat_value,
                );
                if amount > 0 {
                    cell.remove_threat_value(player_id, amount);
                }
            }
        }
    }

    fn add_value_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
        value: u32,
        center_x: f32,
        center_y: f32,
        radius: f32,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                let amount =
                    Self::scaled_affect_amount(x as i32, y_pos, center_x, center_y, radius, value);
                if amount > 0 {
                    cell.add_cash_value(player_id, amount);
                }
            }
        }
    }

    fn remove_value_horizontal_line(
        &mut self,
        x_start: i32,
        x_end: i32,
        y_pos: i32,
        player_id: usize,
        value: u32,
        center_x: f32,
        center_y: f32,
        radius: f32,
    ) {
        if y_pos < 0 || y_pos >= self.height as i32 {
            return;
        }

        let x_start = x_start.max(0) as usize;
        let x_end = (x_end.min(self.width as i32 - 1)) as usize;

        for x in x_start..=x_end {
            if let Some(cell) = self.get_cell_mut(x, y_pos as usize) {
                let amount =
                    Self::scaled_affect_amount(x as i32, y_pos, center_x, center_y, radius, value);
                if amount > 0 {
                    cell.remove_cash_value(player_id, amount);
                }
            }
        }
    }
}

/// Horizontal line for discrete circle algorithm
/// Matches C++ HorzLine struct from DiscreteCircle.h
#[derive(Debug, Clone, Copy)]
struct HorzLine {
    y_pos: i32,
    x_start: i32,
    x_end: i32,
}

/// DiscreteCircle - Generates pixel-perfect circles using Bresenham's algorithm
/// Matches C++ DiscreteCircle from DiscreteCircle.cpp (lines 49-114)
struct DiscreteCircle {
    edges: Vec<HorzLine>,
    y_center: i32,
    y_center_doubled: i32,
}

impl DiscreteCircle {
    /// Create a new discrete circle centered at (x_center, y_center) with given radius
    /// Matches C++ DiscreteCircle::DiscreteCircle() from lines 49-57
    fn new(x_center: i32, y_center: i32, radius: i32) -> Self {
        let y_center_doubled = y_center << 1;
        let mut edges = Vec::with_capacity((radius << 1) as usize);

        Self::generate_edge_pairs(x_center, y_center, radius, &mut edges);
        Self::remove_duplicates(&mut edges);

        Self {
            edges,
            y_center,
            y_center_doubled,
        }
    }

    /// Get the edges (horizontal lines) of the circle
    fn edges(&self) -> &[HorzLine] {
        &self.edges
    }

    /// Get the y-coordinate of the center
    fn y_center(&self) -> i32 {
        self.y_center
    }

    /// Get the doubled y-coordinate of the center (for bottom half rendering)
    fn y_center_doubled(&self) -> i32 {
        self.y_center_doubled
    }

    /// Generate edge pairs using Bresenham's midpoint circle algorithm
    /// Matches C++ DiscreteCircle::generateEdgePairs() from lines 71-95
    fn generate_edge_pairs(x_center: i32, y_center: i32, radius: i32, edges: &mut Vec<HorzLine>) {
        // Uses Bresenham to generate points
        let mut x = 0;
        let mut y = radius;
        let mut d = (1 - radius) << 1;

        while y >= 0 {
            let hl = HorzLine {
                x_start: x_center - x,
                x_end: x_center + x,
                y_pos: y_center + y,
            };
            edges.push(hl);

            if d + y > 0 {
                y -= 1;
                d -= (y << 1) - 1;
            }

            if x > d {
                x += 1;
                d += (x << 1) + 1;
            }
        }
    }

    /// Remove duplicate horizontal lines (same y position)
    /// Matches C++ DiscreteCircle::removeDuplicates() from lines 98-114
    fn remove_duplicates(edges: &mut Vec<HorzLine>) {
        let mut i = 0;
        while i < edges.len() {
            if i + 1 < edges.len() && edges[i].y_pos == edges[i + 1].y_pos {
                edges.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

/// Player mask type for team vision sharing
/// Matches C++ PlayerMaskType (typically u32 bitmask)
pub type PlayerMask = u32;

/// Helper function to check if a player is in a player mask
fn is_player_in_mask(player_id: u32, mask: PlayerMask) -> bool {
    (mask & (1 << player_id)) != 0
}

/// Sighting information for temporary shroud reveals
/// Matches C++ SightingInfo from PartitionManager.cpp
#[derive(Debug, Clone)]
struct SightingInfo {
    /// World position of the reveal center
    where_pos: Coord3D,
    /// Reveal radius in world units
    how_far: f32,
    /// Player mask (which players can see)
    for_whom: PlayerMask,
    /// Frame when this reveal expires
    expiration_frame: u32,
}

/// ShroudManager - Tracks per-player object visibility
///
/// This singleton manages fog-of-war information for all players,
/// maintaining a cache of which objects are visible to each player.
#[derive(Clone)]
pub struct ShroudManager {
    /// Visible objects for each player (indexed by player ID 0-7)
    player_visible_objects: Vec<HashSet<ObjectID>>,

    /// Explored objects for each player (persistent - once seen, always remembered)
    player_explored_objects: Vec<HashSet<ObjectID>>,
    /// C++ `PartitionData::m_shroudedness` for host objects (COI mix).
    host_object_shroud: Vec<HashMap<ObjectID, ObjectShroudStatus>>,
    /// C++ `PartitionData::m_everSeenByPlayer` for host objects.
    host_object_ever_seen: Vec<HashSet<ObjectID>>,

    /// Grid-based shroud for spatial queries
    shroud_grid: Option<ShroudGrid>,

    /// Last frame when visibility was updated
    last_update_frame: u32,

    /// Whether at least one update has run
    has_updated_once: bool,

    /// Main host vision has completed on this manager; never shared across games.
    host_vision_ready: bool,

    /// Last frame when full vision recalculation occurred
    last_vision_recalc_frame: u32,

    /// Frame interval between updates (default 2 for 15 Hz updates at 30 Hz logic)
    update_interval: u32,

    /// Frame interval for full vision recalculation (default 10 frames)
    vision_recalc_interval: u32,

    /// Queue of pending shroud reveals that need to be undone
    /// Matches C++ m_pendingUndoShroudReveals from PartitionManager
    pending_undo_shroud_reveals: VecDeque<SightingInfo>,

    /// Players queued for one-shot full reveal before grid initialization.
    pending_full_reveal_players: HashSet<u32>,

    /// Players queued for permanent full reveal before grid initialization.
    pending_permanent_reveal_players: HashSet<u32>,
}

impl ShroudManager {
    pub fn mark_host_vision_ready(&mut self) {
        self.host_vision_ready = true;
    }

    pub fn host_vision_ready(&self) -> bool {
        self.host_vision_ready
    }

    /// Returns true if the shroud grid has been initialized.
    pub fn has_shroud_grid(&self) -> bool {
        self.shroud_grid.is_some()
    }

    /// Grid dimensions when initialized: `(width_cells, height_cells, cell_size_world)`.
    ///
    /// Used by presentation FOW grid snapshots for terrain / minimap overlay sizing.
    pub fn grid_dimensions(&self) -> Option<(usize, usize, f32)> {
        self.shroud_grid
            .as_ref()
            .map(|g| (g.width, g.height, g.cell_size))
    }

    /// Ground-plane origin owned by the admitted map, not serialized counters.
    pub fn grid_world_origin(&self) -> Option<[f32; 2]> {
        self.shroud_grid.as_ref().map(|grid| grid.world_origin_xy)
    }

    /// Count the driving map's footprint cells for C++ object-shroud mixing.
    /// Off-map cells have no COIs (PartitionManager.cpp:1619-1622). An
    /// uninitialized map likewise contributes no cells; never consult another
    /// world's partition or clone mutable grid state to answer this query.
    pub fn count_footprint_cells(
        &self,
        player_id: u32,
        cells: &[(i32, i32)],
    ) -> crate::object::collide::partition_shroud::PartitionCellShroudCounts {
        use crate::object::collide::partition_shroud::PartitionCellShroudCounts;
        let Some(grid) = self.shroud_grid.as_ref() else {
            return PartitionCellShroudCounts::default();
        };
        if player_id as usize >= MAX_PLAYER_COUNT {
            return PartitionCellShroudCounts::default();
        }
        let on_map = cells.iter().copied().filter(|&(x, y)| {
            x >= 0 && y >= 0 && (x as usize) < grid.width && (y as usize) < grid.height
        });
        PartitionCellShroudCounts::sample(on_map, |x, y| {
            match grid.get_cell_state(player_id as usize, x as usize, y as usize) {
                ShroudState::Hidden => CellShroudStatus::Shrouded,
                ShroudState::Explored => CellShroudStatus::Fogged,
                ShroudState::Visible => CellShroudStatus::Clear,
            }
        })
    }

    /// Compact per-cell shroud state for one player (row-major `y * width + x`).
    ///
    /// Values are [`ShroudState`] discriminants: `0=Hidden`, `1=Explored`, `2=Visible`.
    /// Returns `None` when the grid is not initialized or `player_id` is invalid.
    ///
    /// Fail-closed vs full SAGE dirty-rect streaming — full grid copy for presentation.
    pub fn snapshot_grid_for_player(&self, player_id: u32) -> Option<Vec<u8>> {
        if (player_id as usize) >= MAX_PLAYER_COUNT {
            return None;
        }
        let grid = self.shroud_grid.as_ref()?;
        let mut cells = Vec::with_capacity(grid.width * grid.height);
        for y in 0..grid.height {
            for x in 0..grid.width {
                cells.push(grid.get_cell_state(player_id as usize, x, y) as u8);
            }
        }
        Some(cells)
    }

    /// Capture the exact persistent shroud payload used by save/load.
    ///
    /// This is intentionally separate from `snapshot_grid_for_player`, which
    /// exposes only a derived presentation status and cannot round-trip the
    /// C++ counter state for overlapping lookers or active shroud generators.
    pub fn snapshot_state(&self) -> ShroudSnapshot {
        let grid = self.shroud_grid.as_ref().map(|grid| ShroudGridSnapshot {
            width: grid.width as u32,
            height: grid.height as u32,
            cell_size: grid.cell_size,
            cells: grid
                .cells
                .iter()
                .map(|cell| ShroudCellSnapshot {
                    current_shroud: std::array::from_fn(|player_id| {
                        cell.shroud_levels[player_id].current_shroud
                    }),
                    active_shroud_level: std::array::from_fn(|player_id| {
                        cell.shroud_levels[player_id].active_shroud_level
                    }),
                })
                .collect(),
        });

        let mut pending_full_reveal_players: Vec<_> =
            self.pending_full_reveal_players.iter().copied().collect();
        pending_full_reveal_players.sort_unstable();
        let mut pending_permanent_reveal_players: Vec<_> = self
            .pending_permanent_reveal_players
            .iter()
            .copied()
            .collect();
        pending_permanent_reveal_players.sort_unstable();

        ShroudSnapshot {
            grid,
            pending_undo_shroud_reveals: self
                .pending_undo_shroud_reveals
                .iter()
                .map(|sighting| ShroudPendingUndoRevealSnapshot {
                    where_pos: sighting.where_pos.to_array(),
                    how_far: sighting.how_far,
                    for_whom: sighting.for_whom,
                    expiration_frame: sighting.expiration_frame,
                })
                .collect(),
            pending_full_reveal_players,
            pending_permanent_reveal_players,
        }
    }

    /// Install an exact persistent shroud payload after a map has been staged.
    ///
    /// The derived object visibility sets and update cadence are cleared, then
    /// the radar is refreshed from the restored cell counters.  A malformed
    /// grid is rejected before replacing any current state so a staged load
    /// can roll back without a partially installed singleton.
    pub fn replace_state(&mut self, snapshot: &ShroudSnapshot, frame: u32) -> Result<(), String> {
        let restored_grid = if let Some(grid) = &snapshot.grid {
            let width = usize::try_from(grid.width)
                .map_err(|_| "shroud snapshot width is not representable".to_string())?;
            let height = usize::try_from(grid.height)
                .map_err(|_| "shroud snapshot height is not representable".to_string())?;
            if width == 0 || height == 0 {
                return Err("shroud snapshot grid dimensions must be non-zero".to_string());
            }
            if !grid.cell_size.is_finite() || grid.cell_size <= 0.0 {
                return Err("shroud snapshot cell size is invalid".to_string());
            }
            let expected_cells = width
                .checked_mul(height)
                .ok_or_else(|| "shroud snapshot grid dimensions overflow".to_string())?;
            if grid.cells.len() != expected_cells {
                return Err(format!(
                    "shroud snapshot has {} cells, expected {expected_cells}",
                    grid.cells.len()
                ));
            }

            let cells = grid
                .cells
                .iter()
                .map(|snapshot_cell| PartitionCell {
                    shroud_levels: std::array::from_fn(|player_id| CellShroudLevel {
                        current_shroud: snapshot_cell.current_shroud[player_id],
                        active_shroud_level: snapshot_cell.active_shroud_level[player_id],
                    }),
                    // C++ PartitionCell::xfer does not persist threat/cash
                    // values. Those are rebuilt by the active simulation.
                    threat_values: [0; MAX_PLAYER_COUNT],
                    cash_values: [0; MAX_PLAYER_COUNT],
                })
                .collect();
            Some(ShroudGrid {
                // C++ xfers into a grid initialized from the staged map. The
                // original wire format carries counters, not another extent.
                world_origin_xy: self.grid_world_origin().unwrap_or([0.0, 0.0]),
                width,
                height,
                cell_size: grid.cell_size,
                cells,
            })
        } else {
            None
        };

        let mut pending_undo_shroud_reveals =
            VecDeque::with_capacity(snapshot.pending_undo_shroud_reveals.len());
        for sighting in &snapshot.pending_undo_shroud_reveals {
            if sighting
                .where_pos
                .iter()
                .any(|component| !component.is_finite())
                || !sighting.how_far.is_finite()
                || sighting.how_far < 0.0
            {
                return Err("shroud snapshot contains an invalid pending reveal".to_string());
            }
            pending_undo_shroud_reveals.push_back(SightingInfo {
                where_pos: Coord3D::from_array(sighting.where_pos),
                how_far: sighting.how_far,
                for_whom: sighting.for_whom,
                expiration_frame: sighting.expiration_frame,
            });
        }

        let pending_full_reveal_players =
            Self::validated_pending_player_set(&snapshot.pending_full_reveal_players)?;
        let pending_permanent_reveal_players =
            Self::validated_pending_player_set(&snapshot.pending_permanent_reveal_players)?;

        self.shroud_grid = restored_grid;
        self.pending_undo_shroud_reveals = pending_undo_shroud_reveals;
        self.pending_full_reveal_players = pending_full_reveal_players;
        self.pending_permanent_reveal_players = pending_permanent_reveal_players;
        for visible in &mut self.player_visible_objects {
            visible.clear();
        }
        for explored in &mut self.player_explored_objects {
            explored.clear();
        }
        self.force_update();
        self.refresh_shroud_for_local_player_at_frame(frame);
        Ok(())
    }

    fn validated_pending_player_set(players: &[u32]) -> Result<HashSet<u32>, String> {
        let mut result = HashSet::with_capacity(players.len());
        for &player_id in players {
            if (player_id as usize) >= MAX_PLAYER_COUNT {
                return Err(format!(
                    "shroud snapshot contains invalid pending player {player_id}"
                ));
            }
            if !result.insert(player_id) {
                return Err(format!(
                    "shroud snapshot contains duplicate pending player {player_id}"
                ));
            }
        }
        Ok(result)
    }

    /// Create a new ShroudManager
    pub fn new() -> Self {
        // Pre-allocate visible object sets for all players
        let player_visible_objects = vec![HashSet::new(); MAX_PLAYER_COUNT];
        let player_explored_objects = vec![HashSet::new(); MAX_PLAYER_COUNT];
        let host_object_shroud = vec![HashMap::new(); MAX_PLAYER_COUNT];
        let host_object_ever_seen = vec![HashSet::new(); MAX_PLAYER_COUNT];

        ShroudManager {
            player_visible_objects,
            player_explored_objects,
            host_object_shroud,
            host_object_ever_seen,
            shroud_grid: None,
            last_update_frame: 0,
            has_updated_once: false,
            host_vision_ready: false,
            last_vision_recalc_frame: 0,
            update_interval: DEFAULT_UPDATE_INTERVAL,
            vision_recalc_interval: VISION_RECALC_INTERVAL,
            pending_undo_shroud_reveals: VecDeque::new(),
            pending_full_reveal_players: HashSet::new(),
            pending_permanent_reveal_players: HashSet::new(),
        }
    }

    /// Initialize shroud grid with map dimensions
    ///
    /// Should be called after map is loaded with actual map dimensions
    pub fn init_shroud_grid(&mut self, map_width: f32, map_height: f32) {
        self.init_shroud_grid_at_origin([0.0, 0.0], map_width, map_height);
    }

    /// Initialize from this map's admitted C++ XY extent (Rust XZ).
    pub fn init_shroud_grid_at_origin(
        &mut self,
        world_origin_xy: [f32; 2],
        map_width: f32,
        map_height: f32,
    ) {
        self.shroud_grid = Some(ShroudGrid::new(
            world_origin_xy,
            map_width,
            map_height,
            SHROUD_GRID_CELL_SIZE,
        ));

        if let Some(grid) = self.shroud_grid.as_mut() {
            for player_id in self.pending_full_reveal_players.drain() {
                if (player_id as usize) < MAX_PLAYER_COUNT {
                    grid.reveal_map_for_player(player_id as usize);
                }
            }
            for player_id in self.pending_permanent_reveal_players.drain() {
                if (player_id as usize) < MAX_PLAYER_COUNT {
                    grid.reveal_map_for_player_permanently(player_id as usize);
                }
            }
        }

        debug!(
            "Initialized shroud grid: {}x{} cells for map {}x{} units",
            (map_width / SHROUD_GRID_CELL_SIZE).ceil(),
            (map_height / SHROUD_GRID_CELL_SIZE).ceil(),
            map_width,
            map_height
        );
    }

    /// Update visibility information for all players
    ///
    /// This method is called every game frame from GameLogic's post-update phase.
    /// It updates which objects are visible to each player based on:
    /// - Vision range of player-controlled units
    /// - Line-of-sight checks
    /// - Object positions and existence
    ///
    /// # Performance Note
    ///
    /// For optimization, visibility is cached and updated less frequently than
    /// every frame. The default interval is 2 frames, giving 15 Hz updates at 30 FPS logic.
    /// Full vision recalculation occurs every 10 frames as required.
    ///
    /// # Arguments
    ///
    /// * `frame` - Current logic frame number
    pub fn update(&mut self, frame: u32) -> Result<(), String> {
        // Check if we need a full vision recalculation (every 10 frames)
        let needs_vision_recalc =
            frame.saturating_sub(self.last_vision_recalc_frame) >= self.vision_recalc_interval;

        // Only update at configured interval (or force update on vision recalc)
        if self.has_updated_once
            && !needs_vision_recalc
            && frame.saturating_sub(self.last_update_frame) < self.update_interval
        {
            return Ok(());
        }

        trace!(
            "ShroudManager::update(frame={}): Full visibility recalculation (vision_recalc={})",
            frame, needs_vision_recalc
        );

        self.last_update_frame = frame;
        self.has_updated_once = true;

        // NOTE: No need to "downgrade" cells with counter-based system.
        // The counter system automatically transitions cells from CLEAR -> FOGGED
        // when lookers are removed in the next vision recalculation.

        // Clear current visibility state (explored objects persist)
        for visible_set in &mut self.player_visible_objects {
            visible_set.clear();
        }

        // Get ObjectManager for object queries
        let manager_arc = get_object_manager();
        let object_manager = match manager_arc.read() {
            Ok(mgr) => mgr,
            Err(_) => {
                return Err("Failed to acquire ObjectManager read lock".to_string());
            }
        };

        // Process pending temporary shroud reveals (expire old ones)
        self.process_pending_undo_shroud_reveals(frame);

        // For each player, determine which objects are visible.
        // C++ parity: terrain shroud lookers are owned by the per-object
        // look/unlook cycle (`Object::handleShroud`, Object.cpp:4779-4784,
        // driven by PartitionData cell changes); this pass only refreshes
        // object visibility + explored sets.
        for player_id in 0..MAX_PLAYER_COUNT {
            if let Err(e) = self.update_visibility_for_player(player_id as u32, &object_manager) {
                debug!(
                    "Failed to update visibility for player {}: {}",
                    player_id, e
                );
                // Continue with other players on error
            }
        }

        if needs_vision_recalc {
            self.last_vision_recalc_frame = frame;
        }

        Ok(())
    }

    /// Update visibility for a specific player
    ///
    /// For a given player, this method:
    /// 1. Gets all units owned by the player
    /// 2. For each unit, determines what it can see
    /// 3. Aggregates visible objects into a per-player cache
    ///
    /// # Faithful to C++
    ///
    /// Mirrors C++ Vision::update_shroud_for_player() behavior:
    /// - Identifies player-controlled units via team ownership
    /// - Checks each unit's shroud-clearing range and line-of-sight
    /// - Aggregates visible objects per player
    /// - Updates shroud state for rendering
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `object_manager` - Reference to the object manager (must already be locked)
    fn update_visibility_for_player(
        &mut self,
        player_id: u32,
        object_manager: &crate::object_manager::ObjectManager,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return Err(format!("Invalid player ID: {}", player_id));
        }

        trace!(
            "ShroudManager::update_visibility_for_player(player_id={})",
            player_id
        );

        // Get all objects owned by this player (their units and structures)
        let mut viewer_ids = object_manager.get_objects_owned_by_player(player_id as u32);

        // Add any "spied" viewers: units belonging to other players whose vision is shared to us.
        // This mirrors C++ SpyVision behavior (enemy units act as lookers for the spying player).
        let all_object_ids = object_manager.all_object_ids();
        for obj_id in &all_object_ids {
            if let Some(viewer_arc) = object_manager.get_object(*obj_id) {
                let base_arc = viewer_arc.read().ok().map(|g| g.base());
                if let Some(base_arc) = base_arc {
                    if let Ok(base_guard) = base_arc.read() {
                        if base_guard.is_vision_spied_by_player(player_id) {
                            viewer_ids.push(*obj_id);
                        }
                    }
                }
            }
        }

        viewer_ids.sort_unstable();
        viewer_ids.dedup();

        if viewer_ids.is_empty() {
            // Player has no units, can't see anything
            trace!("Player {} has no units, clearing visibility", player_id);
            self.player_visible_objects[player_id as usize].clear();
            return Ok(());
        }

        // For each unit owned by the player, check what it can see
        for viewer_id in viewer_ids {
            let Some(viewer_slot) = object_manager.get_object(viewer_id) else {
                continue;
            };
            // ObjectSlot uses a nonrecursive mutex. LOS scans nearby slots,
            // including this viewer and the target; end both borrows first.
            let (viewer_pos, viewer_shroud_range, viewer_eye_pos) = {
                let Ok(viewer) = viewer_slot.read() else {
                    trace!("Failed to read viewer unit {}", viewer_id);
                    continue;
                };
                let position = *viewer.get_position();
                let range = viewer
                    .base()
                    .read()
                    .map(|base| base.get_shroud_clearing_range())
                    .unwrap_or(0.0);
                let mut eye = position;
                eye.z += viewer.get_geometry_info().get_max_height_above_position();
                (position, range, eye)
            };

            for target_id in &all_object_ids {
                if *target_id == viewer_id {
                    self.player_visible_objects[player_id as usize].insert(*target_id);
                    continue;
                }
                let Some(target_slot) = object_manager.get_object(*target_id) else {
                    continue;
                };
                let (target_pos, target_eye_pos) = {
                    let Ok(target) = target_slot.read() else {
                        continue;
                    };
                    let position = *target.get_position();
                    let mut eye = position;
                    eye.z += target.get_geometry_info().get_max_height_above_position();
                    (position, eye)
                };

                let dx = viewer_pos.x - target_pos.x;
                let dy = viewer_pos.y - target_pos.y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance <= viewer_shroud_range
                    && self.check_line_of_sight(&viewer_eye_pos, &target_eye_pos, object_manager)
                {
                    self.player_visible_objects[player_id as usize].insert(*target_id);
                }
            }
        }

        trace!(
            "Player {} can see {} objects",
            player_id,
            self.player_visible_objects[player_id as usize].len()
        );

        Ok(())
    }
    /// Check if a player can see a specific object
    ///
    /// This is the primary query method used by rendering and AI systems
    /// to determine whether an object should be visible to a player.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `object_id` - Which object
    ///
    /// # Returns
    ///
    /// `true` if the object is visible to this player, `false` otherwise
    pub fn can_see_object(&self, player_id: u32, object_id: ObjectID) -> bool {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return false; // Invalid player
        }

        self.player_visible_objects[player_id as usize].contains(&object_id)
    }

    /// Check if a player can actually see an object considering stealth
    ///
    /// This method extends the basic FOW visibility check by also considering
    /// whether the object is stealthed and whether the player has detection
    /// capability to see through that stealth.
    ///
    /// # Integration with Stealth System
    ///
    /// Visibility logic:
    /// 1. If object is not in FOW (can_see_object returns false): NOT visible
    /// 2. If object is in FOW but stealthed (is_invisible_to_player): Check detection
    /// 3. If detection can detect stealth: visible, otherwise NOT visible
    /// 4. If object not stealthed or stealth is revealed: visible
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `object_id` - Which object to check
    ///
    /// # Returns
    ///
    /// `true` if the object is visible to this player (considering stealth/detection)
    pub fn can_see_object_with_stealth(
        &self,
        player_id: u32,
        object_id: ObjectID,
    ) -> Result<bool, String> {
        use crate::system::detection_manager::{DetectionModifier, get_detection_manager};
        use crate::system::stealth_manager::get_stealth_manager;

        if player_id >= MAX_PLAYER_COUNT as u32 {
            return Ok(false);
        }

        // First check: Is the object in line-of-sight (FOW check)?
        if !self.can_see_object(player_id, object_id) {
            return Ok(false); // Not visible due to fog-of-war
        }

        // Second check: Is the object stealthed?
        let stealth_mgr = match get_stealth_manager().lock() {
            Ok(mgr) => mgr,
            Err(_) => {
                // On lock error, assume visible (fail-open for visibility)
                return Ok(true);
            }
        };

        // Check if object is invisible to this player
        let is_invisible = match stealth_mgr.is_invisible_to_player(object_id, player_id as usize) {
            Ok(result) => result,
            Err(_) => {
                // Object not registered in stealth system, assume not stealthed
                false
            }
        };

        if !is_invisible {
            // Object is not stealthed or stealth has been revealed
            return Ok(true);
        }

        // Object is stealthed - check if we have detection capability
        drop(stealth_mgr); // Release the lock before acquiring detection lock

        let stealth_strength = match get_stealth_manager().lock() {
            Ok(mgr) => match mgr.get_stealth_strength(object_id) {
                Ok(strength) => strength.value(),
                Err(_) => 0.0,
            },
            Err(_) => 0.0,
        };

        // Get the player's detection capability
        let detection_mgr = match get_detection_manager().lock() {
            Ok(mgr) => mgr,
            Err(_) => {
                // On lock error, assume stealthed (fail-safe for stealth)
                return Ok(false);
            }
        };

        // Find a detector unit owned by the player that can detect this object
        // For simplicity, we'll aggregate detection from all player units
        let player_units = match crate::object_manager::get_object_manager().read() {
            Ok(obj_mgr) => obj_mgr.get_objects_owned_by_player(player_id),
            Err(_) => Vec::new(),
        };

        for detector_id in player_units {
            let modifier = DetectionModifier::default(); // Can be enhanced with distance/movement modifiers

            if let Ok(can_detect) =
                detection_mgr.can_detect_stealth(detector_id, stealth_strength, modifier)
            {
                if can_detect {
                    // At least one of player's units can detect this stealthed object
                    return Ok(true);
                }
            }
        }

        // Stealthed object not detected by any player units
        Ok(false)
    }

    /// Get all visible objects for a player
    ///
    /// Returns a snapshot of currently visible objects. This is used for:
    /// - Rendering visible units
    /// - AI target selection
    /// - UI information display
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    ///
    /// # Returns
    ///
    /// Vector of object IDs visible to this player
    pub fn get_visible_objects(&self, player_id: u32) -> Vec<ObjectID> {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return Vec::new();
        }

        self.player_visible_objects[player_id as usize]
            .iter()
            .copied()
            .collect()
    }

    /// O(1) "is any object currently visible" probe.
    ///
    /// Same semantics as `!get_visible_objects(player_id).is_empty()` without
    /// materializing the snapshot Vec — hot presentation paths call this per
    /// object per frame and must not allocate.
    pub fn has_any_visible_object(&self, player_id: u32) -> bool {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return false;
        }
        !self.player_visible_objects[player_id as usize].is_empty()
    }

    /// Set the visibility update interval
    ///
    /// Controls how frequently visibility is recalculated. Lower values mean
    /// more frequent updates but higher CPU cost. Default is 2 frames.
    ///
    /// # Arguments
    ///
    /// * `interval` - Frames between visibility updates (minimum 1)
    pub fn set_update_interval(&mut self, interval: u32) {
        self.update_interval = interval.max(1);
    }

    /// Get current visibility update interval
    pub fn get_update_interval(&self) -> u32 {
        self.update_interval
    }

    /// Get frame when visibility was last updated
    pub fn get_last_update_frame(&self) -> u32 {
        self.last_update_frame
    }

    /// Force immediate visibility update on next frame
    ///
    /// Used when major events occur that require fresh visibility data
    /// (e.g., units destroyed, new units created, vision upgrades applied)
    pub fn force_update(&mut self) {
        // Reset so next update() call will immediately recalculate
        self.last_update_frame = 0;
        self.last_vision_recalc_frame = 0;
        self.has_updated_once = false;
        self.host_vision_ready = false;
    }

    /// Clear all visibility information
    ///
    /// Resets shroud to completely obscured state. Useful for:
    /// - Scenario resets
    /// - Multiplayer match initialization
    /// - Debugging
    pub fn clear_all(&mut self) {
        for visible_set in &mut self.player_visible_objects {
            visible_set.clear();
        }
        for explored_set in &mut self.player_explored_objects {
            explored_set.clear();
        }
        for status in &mut self.host_object_shroud {
            status.clear();
        }
        for seen in &mut self.host_object_ever_seen {
            seen.clear();
        }
        self.pending_undo_shroud_reveals.clear();
        self.last_update_frame = 0;
        self.last_vision_recalc_frame = 0;
        self.has_updated_once = false;
        self.host_vision_ready = false;
        // Drop terrain grid so permanent reveal lookers cannot leak across tests
        // / scenario resets. Callers re-init via init_shroud_grid.
        self.shroud_grid = None;
    }

    /// Full new-game reset. C++ tears per-world shroud down with the world:
    /// `TheGameLogic::clearGameData` resets ThePartitionManager (PartitionData
    /// `m_shroudedness` / `m_everSeenByPlayer` die with the partition data)
    /// and ThePlayerList, destroying each Player's Shroud and its
    /// SightingInfo records. No object-shroud entry may outlive its GameLogic
    /// world — stale entries from a previous world would fog unrelated
    /// objects in the next world (Object IDs are recycled from 1).
    /// `clear_all` covers the per-player object sets and the terrain grid.
    /// The pre-grid reveal queues are session-level queued player intents
    /// (reveal map for player N at next grid init); a same-session world
    /// reset must retain them so queued full reveals still apply.
    pub fn reset_for_new_game(&mut self) {
        self.clear_all();
    }
    /// Update explored territory from current visibility
    ///
    /// Adds all currently visible objects to the explored set
    fn update_explored_territory(&mut self, player_id: usize) {
        if player_id >= MAX_PLAYER_COUNT {
            return;
        }

        // Mark all visible objects as explored
        for &obj_id in &self.player_visible_objects[player_id] {
            self.player_explored_objects[player_id].insert(obj_id);
        }
    }

    /// Check if an object has been explored by a player (even if not currently visible)
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `object_id` - Which object
    ///
    /// # Returns
    ///
    /// `true` if the object has ever been seen by this player
    pub fn has_explored_object(&self, player_id: u32, object_id: ObjectID) -> bool {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return false;
        }

        self.player_explored_objects[player_id as usize].contains(&object_id)
    }

    /// Check if a world position is currently visible to a player
    ///
    /// Uses grid-based shroud for fast spatial queries
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `position` - World position to check
    ///
    /// # Returns
    ///
    /// `true` if the position is currently visible
    pub fn is_position_visible(&self, player_id: u32, position: &Coord3D) -> bool {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return false;
        }

        if let Some(ref grid) = self.shroud_grid {
            grid.is_position_visible(player_id as usize, position)
        } else {
            false
        }
    }

    /// Check if a world position has been explored by a player
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `position` - World position to check
    ///
    /// # Returns
    ///
    /// `true` if the position has been explored (visible or previously seen)
    pub fn is_position_explored(&self, player_id: u32, position: &Coord3D) -> bool {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return false;
        }

        if let Some(ref grid) = self.shroud_grid {
            grid.is_position_explored(player_id as usize, position)
        } else {
            false
        }
    }

    /// Get shroud state for a world position
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    /// * `position` - World position to check
    ///
    /// # Returns
    ///
    /// ShroudState (Hidden, Explored, or Visible)
    pub fn get_shroud_state(&self, player_id: u32, position: &Coord3D) -> ShroudState {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return ShroudState::Hidden;
        }

        if let Some(ref grid) = self.shroud_grid {
            if let Some((x, y)) = grid.world_to_grid(position) {
                return grid.get_cell_state(player_id as usize, x, y);
            }
        }

        ShroudState::Hidden
    }

    /// Reveal the entire map for a player (addLooker then removeLooker; not permanent).
    pub fn reveal_map_for_player(&mut self, player_id: u32) -> Result<(), String> {
        if (player_id as usize) >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player index {player_id}"));
        }
        if let Some(grid) = self.shroud_grid.as_mut() {
            grid.reveal_map_for_player(player_id as usize);
        } else {
            self.pending_full_reveal_players.insert(player_id);
        }
        Ok(())
    }

    /// Reveal the entire map for a player permanently (disables shroud generation).
    pub fn reveal_map_for_player_permanently(&mut self, player_id: u32) -> Result<(), String> {
        if (player_id as usize) >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player index {player_id}"));
        }
        if let Some(grid) = self.shroud_grid.as_mut() {
            grid.reveal_map_for_player_permanently(player_id as usize);
        } else {
            self.pending_permanent_reveal_players.insert(player_id);
        }
        Ok(())
    }

    /// Undo a permanent map reveal for a player.
    pub fn undo_reveal_map_for_player_permanently(&mut self, player_id: u32) -> Result<(), String> {
        if (player_id as usize) >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player index {player_id}"));
        }
        self.pending_permanent_reveal_players.remove(&player_id);
        self.pending_full_reveal_players.remove(&player_id);
        self.process_entire_pending_undo_shroud_reveal_queue();
        if let Some(grid) = self.shroud_grid.as_mut() {
            grid.undo_reveal_map_for_player_permanently(player_id as usize);
        }
        Ok(())
    }

    /// Shroud the entire map for a player (reset fog/shroud).
    pub fn shroud_map_for_player(&mut self, player_id: u32) -> Result<(), String> {
        if (player_id as usize) >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player index {player_id}"));
        }
        self.pending_permanent_reveal_players.remove(&player_id);
        self.pending_full_reveal_players.remove(&player_id);
        self.process_entire_pending_undo_shroud_reveal_queue();
        if let Some(grid) = self.shroud_grid.as_mut() {
            grid.shroud_map_for_player(player_id as usize);
        }
        Ok(())
    }

    /// Refresh shroud for the local player (visual refresh hook).
    /// Matches C++ PartitionManager::refreshShroudForLocalPlayer intent.
    pub fn refresh_shroud_for_local_player(&mut self) {
        self.refresh_shroud_for_local_player_at_frame(crate::helpers::TheGameLogic::get_frame());
    }

    /// Refresh using the driving instance clock, including while its save/load owner is borrowed.
    pub fn refresh_shroud_for_local_player_at_frame(&mut self, frame: u32) {
        if let Ok(list) = crate::player::player_list().read() {
            let local_index = list.get_local_player_index();
            if local_index != PLAYER_INDEX_INVALID {
                let player_id = local_index as u32;
                if let Some(grid) = self.shroud_grid.as_ref() {
                    if let Ok(mut radar) = get_radar_system().write() {
                        radar.clear_shroud();
                        for y in 0..grid.height {
                            for x in 0..grid.width {
                                let status = match grid.get_cell_state(player_id as usize, x, y) {
                                    ShroudState::Visible => CellShroudStatus::Clear,
                                    ShroudState::Explored => CellShroudStatus::Fogged,
                                    ShroudState::Hidden => CellShroudStatus::Shrouded,
                                };
                                radar.set_shroud_level_from_partition_cell(
                                    x as i32,
                                    y as i32,
                                    status,
                                    SHROUD_GRID_CELL_SIZE,
                                    SHROUD_GRID_CELL_SIZE,
                                    grid.world_origin_xy,
                                );
                            }
                        }
                    }
                }
            }
        }

        self.last_update_frame = frame;
        self.last_vision_recalc_frame = frame;
        self.has_updated_once = true;
    }

    /// C++ `PartitionManager::refreshShroudForLocalPlayer` for a host player id.
    /// Leftover `player_list` is often empty on the Main host path.
    pub fn refresh_radar_shroud_for_player(&mut self, player_id: u32) {
        if (player_id as usize) >= MAX_PLAYER_COUNT {
            return;
        }
        if let Some(grid) = self.shroud_grid.as_ref() {
            if let Ok(mut radar) = get_radar_system().write() {
                radar.clear_shroud();
                for y in 0..grid.height {
                    for x in 0..grid.width {
                        let status = match grid.get_cell_state(player_id as usize, x, y) {
                            ShroudState::Visible => CellShroudStatus::Clear,
                            ShroudState::Explored => CellShroudStatus::Fogged,
                            ShroudState::Hidden => CellShroudStatus::Shrouded,
                        };
                        radar.set_shroud_level_from_partition_cell(
                            x as i32,
                            y as i32,
                            status,
                            SHROUD_GRID_CELL_SIZE,
                            SHROUD_GRID_CELL_SIZE,
                            grid.world_origin_xy,
                        );
                    }
                }
            }
        }
        let frame = crate::helpers::TheGameLogic::get_frame();
        self.last_update_frame = frame;
        self.last_vision_recalc_frame = frame;
        self.has_updated_once = true;
    }

    /// Get all explored objects for a player
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player (0-7)
    ///
    /// # Returns
    ///
    /// Vector of object IDs that have been explored by this player
    pub fn get_explored_objects(&self, player_id: u32) -> Vec<ObjectID> {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return Vec::new();
        }

        self.player_explored_objects[player_id as usize]
            .iter()
            .copied()
            .collect()
    }

    /// O(1) "has any object ever been explored" probe.
    ///
    /// Same semantics as `!get_explored_objects(player_id).is_empty()` without
    /// materializing the snapshot Vec — hot presentation paths call this per
    /// object per frame and must not allocate.
    pub fn has_any_explored_object(&self, player_id: u32) -> bool {
        if player_id >= MAX_PLAYER_COUNT as u32 {
            return false;
        }
        !self.player_explored_objects[player_id as usize].is_empty()
    }

    /// Set vision recalculation interval
    ///
    /// Controls how frequently full vision recalculation occurs.
    /// Default is 10 frames as required.
    ///
    /// # Arguments
    ///
    /// * `interval` - Frames between vision recalculations (minimum 1)
    /// Host residual: Main GameLogic objects are not in ObjectManager on the default
    /// authority path. Register object membership so FOW object filters and
    /// presentation snapshots see host units without Arc registry dual-world.
    pub fn mark_host_object_seen(&mut self, player_id: u32, object_id: ObjectID) {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return;
        }
        let idx = player_id as usize;
        self.player_visible_objects[idx].insert(object_id);
        self.player_explored_objects[idx].insert(object_id);
    }

    /// Fogged ghost: explored but not currently visible.
    pub fn mark_host_object_explored(&mut self, player_id: u32, object_id: ObjectID) {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return;
        }
        self.player_explored_objects[player_id as usize].insert(object_id);
    }

    pub fn set_host_object_shroud_status(
        &mut self,
        player_id: u32,
        object_id: ObjectID,
        status: ObjectShroudStatus,
    ) {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return;
        }
        self.host_object_shroud[player_id as usize].insert(object_id, status);
    }

    pub fn get_host_object_shroud_status(
        &self,
        player_id: u32,
        object_id: ObjectID,
    ) -> Option<ObjectShroudStatus> {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return None;
        }
        self.host_object_shroud[player_id as usize]
            .get(&object_id)
            .copied()
    }

    pub fn host_object_ever_seen(&self, player_id: u32, object_id: ObjectID) -> bool {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return false;
        }
        self.host_object_ever_seen[player_id as usize].contains(&object_id)
    }

    pub fn set_host_object_ever_seen(&mut self, player_id: u32, object_id: ObjectID, seen: bool) {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return;
        }
        let slot = &mut self.host_object_ever_seen[player_id as usize];
        if seen {
            slot.insert(object_id);
        } else {
            slot.remove(&object_id);
        }
    }

    /// Host residual: clear per-player object membership before a full host vision pass.
    /// Does not touch terrain looker counters / shroud grid cells.
    pub fn clear_host_object_visibility(&mut self, player_id: u32) {
        if player_id as usize >= MAX_PLAYER_COUNT {
            return;
        }
        let idx = player_id as usize;
        self.player_visible_objects[idx].clear();
        self.host_object_shroud[idx].clear();
        // Explored / ever-seen persist across frames (C++ explored territory).
    }

    pub fn set_vision_recalc_interval(&mut self, interval: u32) {
        self.vision_recalc_interval = interval.max(1);
    }

    /// Get current vision recalculation interval
    pub fn get_vision_recalc_interval(&self) -> u32 {
        self.vision_recalc_interval
    }

    /// Reveal a circular area for specified players
    /// Matches C++ PartitionManager::doShroudReveal() from lines 3969-3990
    ///
    /// # Arguments
    ///
    /// * `center` - World position of reveal center
    /// * `radius` - Radius in world units
    /// * `player_mask` - Bitmask of players who can see (bit 0 = player 0, bit 1 = player 1, etc.)
    pub fn do_shroud_reveal(&mut self, center: &Coord3D, radius: f32, player_mask: PlayerMask) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        // Apply reveal to all players in the mask
        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.do_shroud_reveal(center, radius, player_id);
            }
        }
    }

    /// Apply shroud cover (active shroud) to a circular area for specified players.
    /// Matches C++ PartitionManager::doShroudCover() from lines 4041-4061
    pub fn do_shroud_cover(&mut self, center: &Coord3D, radius: f32, player_mask: PlayerMask) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.do_shroud_cover(center, radius, player_id);
            }
        }
    }

    /// Undo reveal of a circular area for specified players
    /// Matches C++ PartitionManager::undoShroudReveal() from lines 4036-4055
    ///
    /// # Arguments
    ///
    /// * `center` - World position of reveal center
    /// * `radius` - Radius in world units
    /// * `player_mask` - Bitmask of players who can no longer see
    pub fn undo_shroud_reveal(&mut self, center: &Coord3D, radius: f32, player_mask: PlayerMask) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        // Remove reveal from all players in the mask
        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.undo_shroud_reveal(center, radius, player_id);
            }
        }
    }

    /// Remove shroud cover (active shroud) from a circular area for specified players.
    /// Matches C++ PartitionManager::undoShroudCover() from lines 4065-4085
    pub fn undo_shroud_cover(&mut self, center: &Coord3D, radius: f32, player_mask: PlayerMask) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.undo_shroud_cover(center, radius, player_id);
            }
        }
    }

    /// Apply threat influence with radial falloff for all players in the mask.
    /// Matches C++ PartitionManager::doThreatAffect().
    pub fn do_threat_affect(
        &mut self,
        center: &Coord3D,
        radius: f32,
        threat_value: u32,
        player_mask: PlayerMask,
    ) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.do_threat_affect(center, radius, threat_value, player_id);
            }
        }
    }

    /// Remove threat influence with radial falloff for all players in the mask.
    /// Matches C++ PartitionManager::undoThreatAffect().
    pub fn undo_threat_affect(
        &mut self,
        center: &Coord3D,
        radius: f32,
        threat_value: u32,
        player_mask: PlayerMask,
    ) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.undo_threat_affect(center, radius, threat_value, player_id);
            }
        }
    }

    /// Apply cash/value influence with radial falloff for all players in the mask.
    /// Matches C++ PartitionManager::doValueAffect().
    pub fn do_value_affect(
        &mut self,
        center: &Coord3D,
        radius: f32,
        value: u32,
        player_mask: PlayerMask,
    ) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.do_value_affect(center, radius, value, player_id);
            }
        }
    }

    /// Remove cash/value influence with radial falloff for all players in the mask.
    /// Matches C++ PartitionManager::undoValueAffect().
    pub fn undo_value_affect(
        &mut self,
        center: &Coord3D,
        radius: f32,
        value: u32,
        player_mask: PlayerMask,
    ) {
        let grid = match self.shroud_grid.as_mut() {
            Some(g) => g,
            None => return,
        };

        for player_id in 0..MAX_PLAYER_COUNT {
            if is_player_in_mask(player_id as u32, player_mask) {
                grid.undo_value_affect(center, radius, value, player_id);
            }
        }
    }

    /// Queue an undo for a shroud reveal that will automatically revert after a duration
    /// Matches C++ PartitionManager::queueUndoShroudReveal() from lines 4058-4070
    ///
    /// # Arguments
    ///
    /// * `center` - World position of reveal center
    /// * `radius` - Radius in world units
    /// * `player_mask` - Bitmask of players who can see
    /// * `duration_frames` - How many frames until the reveal expires
    /// * `current_frame` - Current game frame number
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Queue undo for a reveal that lasts 600 frames (20 seconds at 30 FPS)
    /// manager.queue_undo_shroud_reveal(&position, 500.0, 0xFF, 600, current_frame);
    /// ```
    pub fn queue_undo_shroud_reveal(
        &mut self,
        center: &Coord3D,
        radius: f32,
        player_mask: PlayerMask,
        duration_frames: u32,
        current_frame: u32,
    ) {
        let sighting = SightingInfo {
            where_pos: *center,
            how_far: radius,
            for_whom: player_mask,
            expiration_frame: current_frame + duration_frames,
        };

        self.pending_undo_shroud_reveals.push_back(sighting);
    }

    /// Process pending undo shroud reveals
    /// Matches C++ PartitionManager::processPendingUndoShroudRevealQueue() from lines 3993-4012
    ///
    /// This should be called every frame from the update loop
    ///
    /// # Arguments
    ///
    /// * `current_frame` - Current game frame number
    pub fn process_pending_undo_shroud_reveals(&mut self, current_frame: u32) {
        self.process_pending_undo_shroud_reveals_internal(true, current_frame);
    }

    /// Process the entire pending undo shroud reveal queue.
    /// Matches C++ PartitionManager::processEntirePendingUndoShroudRevealQueue().
    pub fn process_entire_pending_undo_shroud_reveal_queue(&mut self) {
        self.process_pending_undo_shroud_reveals_internal(false, u32::MAX);
    }

    fn process_pending_undo_shroud_reveals_internal(
        &mut self,
        consider_timestamp: bool,
        current_frame: u32,
    ) {
        let compare_time = if consider_timestamp {
            current_frame
        } else {
            u32::MAX
        };

        while let Some(front) = self.pending_undo_shroud_reveals.front() {
            if front.expiration_frame < compare_time {
                let sighting = self
                    .pending_undo_shroud_reveals
                    .pop_front()
                    .expect("front checked");
                self.undo_shroud_reveal(&sighting.where_pos, sighting.how_far, sighting.for_whom);
            } else {
                break;
            }
        }
    }

    /// Reset all pending undo shroud reveals
    /// Matches C++ PartitionManager::resetPendingUndoShroudRevealQueue() from lines 4025-4033
    pub fn reset_pending_undo_shroud_reveals(&mut self) {
        self.pending_undo_shroud_reveals.clear();
    }

    /// Remove any pending undo reveals that include the specified player.
    fn clear_pending_undo_shroud_reveals_for_player(&mut self, player_id: u32) {
        let bit = 1u32 << player_id;
        let mut filtered = VecDeque::with_capacity(self.pending_undo_shroud_reveals.len());
        while let Some(mut sighting) = self.pending_undo_shroud_reveals.pop_front() {
            if sighting.for_whom & bit != 0 {
                sighting.for_whom &= !bit;
            }
            if sighting.for_whom != 0 {
                filtered.push_back(sighting);
            }
        }
        self.pending_undo_shroud_reveals = filtered;
    }

    /// Check line-of-sight between two positions
    ///
    /// Terrain eyes follow C++ PartitionManager::isClearLineOfSightTerrain.
    /// The additional opaque-structure sampler retains current Rust behavior;
    /// its original policy correspondence remains unverified.
    ///
    /// # Arguments
    ///
    /// * `from` - Source position (viewer)
    /// * `to` - Target position (what we're trying to see)
    /// * `object_manager` - Reference to object manager for obstacle checks
    ///
    /// # Returns
    ///
    /// `true` if line-of-sight is clear, `false` if blocked
    fn check_line_of_sight(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        _object_manager: &crate::object_manager::ObjectManager,
    ) -> bool {
        if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
            if !terrain.is_clear_line_of_sight(from, to) {
                return false;
            }
        }

        let delta = *to - *from;
        let distance_xy = (delta.x * delta.x + delta.y * delta.y).sqrt();
        if distance_xy <= 0.001 {
            return true;
        }

        let step_len = 10.0_f32;
        let steps = (distance_xy / step_len).ceil().clamp(2.0, 512.0) as u32;

        for i in 1..steps {
            let t = i as f32 / steps as f32;
            let sample = Coord3D::new(
                from.x + delta.x * t,
                from.y + delta.y * t,
                from.z + delta.z * t,
            );

            let candidates = _object_manager.find_objects_in_radius(sample, step_len * 2.0);
            for object_id in candidates {
                let Some(instance) = _object_manager.get_object(object_id) else {
                    continue;
                };

                let Ok(instance_guard) = instance.read() else {
                    continue;
                };
                let __base_arc = instance_guard.base();
                let Ok(obj_guard) = __base_arc.read() else {
                    continue;
                };

                if obj_guard.is_destroyed() {
                    continue;
                }

                if obj_guard.is_kind_of(KindOf::CanSeeThrough) {
                    continue;
                }

                if !(obj_guard.is_structure()
                    || obj_guard.is_kind_of(KindOf::Bridge)
                    || obj_guard.is_kind_of(KindOf::Barrier))
                {
                    continue;
                }

                let geom = obj_guard.get_geometry_info();
                let radius = geom.get_bounding_circle_radius();
                if radius <= 0.0 {
                    continue;
                }

                let dx = sample.x - geom.position.x;
                let dy = sample.y - geom.position.y;
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }

                let min_z = geom.position.z + geom.bounds.min.z;
                let max_z = geom.position.z + geom.bounds.max.z;
                if sample.z >= min_z && sample.z <= max_z {
                    return false;
                }
            }
        }

        true
    }
}

impl Default for ShroudManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The active ShroudManager (C++ `ThePartitionManager` shroud state): the
/// installed world bundle's manager, or the engine-lifetime manager when no
/// GameLogic world is active. World bundles start with a fresh manager;
/// mutable map counters and reveal queues are never inherited from the engine.
pub fn get_shroud_manager() -> Arc<Mutex<ShroudManager>> {
    crate::system::engine_stores::shroud_manager()
}

#[cfg(test)]
#[path = "shroud_map_origin_tests.rs"]
mod map_origin_tests;

#[cfg(test)]
#[path = "shroud_manager_tests.rs"]
mod tests;
