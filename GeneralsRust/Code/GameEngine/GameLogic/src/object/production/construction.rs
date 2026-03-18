//! Construction system for building placement and dozer construction
//!
//! Faithful port of C++ DozerAIUpdate construction logic from DozerAIUpdate.cpp

use crate::ai::THE_AI;
use crate::common::*;
use crate::helpers::{TheTerrainLogic, TheThingFactory};
use crate::system::shroud_manager::{get_shroud_manager, ShroudState};
use game_engine::common::global_data;
use game_engine::common::system::GeometryType;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// Construction state for a building
/// Matches C++ Object construction status from Object.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionState {
    /// Not under construction (complete or not started)
    None,
    /// Awaiting dozer to start construction
    /// Matches C++ MODELCONDITION_AWAITING_CONSTRUCTION
    AwaitingConstruction,
    /// Being actively constructed by a dozer
    /// Matches C++ MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED
    ActivelyBeingConstructed,
    /// Partially constructed but dozer left
    /// Matches C++ MODELCONDITION_PARTIALLY_CONSTRUCTED
    PartiallyConstructed,
    /// Construction complete, finalizing
    Complete,
}

/// Foundation placement validator
/// Matches C++ BuildAssistant::isLocationLegalToBuild
pub struct FoundationValidator {
    /// Whether to check terrain restrictions
    check_terrain: bool,
    /// Whether to check clear path
    check_clear_path: bool,
    /// Whether to check object overlap
    check_object_overlap: bool,
    /// Whether overlap checks only block enemies
    overlap_enemy_only: bool,
    /// Whether to ignore stealthed objects during overlap checks
    ignore_stealthed: bool,
    /// Whether to fail without feedback when blocked by stealth
    fail_stealthed_without_feedback: bool,
    /// Whether to check shroud revealed
    check_shroud_revealed: bool,
}

impl FoundationValidator {
    /// Create a new validator with all checks enabled
    /// Matches C++ BuildAssistant flags from DozerAIUpdate.cpp lines 1652-1656
    pub fn new_strict() -> Self {
        Self {
            check_terrain: true,
            check_clear_path: true,
            check_object_overlap: true,
            overlap_enemy_only: false,
            ignore_stealthed: false,
            fail_stealthed_without_feedback: false,
            check_shroud_revealed: true,
        }
    }

    /// Create a validator for AI (relaxed rules)
    pub fn new_ai() -> Self {
        Self {
            check_terrain: true,
            check_clear_path: false, // AI can cheat
            check_object_overlap: true,
            overlap_enemy_only: false,
            ignore_stealthed: false,
            fail_stealthed_without_feedback: false,
            check_shroud_revealed: false, // AI doesn't need vision
        }
    }

    /// Create a validator based on BuildAssistant option flags.
    pub fn from_build_options(
        options: game_engine::common::system::build_assistant::LocalLegalToBuildOptions,
    ) -> Self {
        let check_terrain = options
            .contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS);
        let check_clear_path = options.contains(
            game_engine::common::system::build_assistant::LocalLegalToBuildOptions::CLEAR_PATH,
        );
        let overlap_enemy_only = options
            .contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::NO_ENEMY_OBJECT_OVERLAP)
            && !options.contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::NO_OBJECT_OVERLAP);
        let check_object_overlap = options
            .contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::NO_OBJECT_OVERLAP)
            || options.contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::NO_ENEMY_OBJECT_OVERLAP);
        let ignore_stealthed = options
            .contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::IGNORE_STEALTHED);
        let fail_stealthed_without_feedback = options
            .contains(game_engine::common::system::build_assistant::LocalLegalToBuildOptions::FAIL_STEALTHED_WITHOUT_FEEDBACK);
        let check_shroud_revealed = options.contains(
            game_engine::common::system::build_assistant::LocalLegalToBuildOptions::SHROUD_REVEALED,
        );

        Self {
            check_terrain,
            check_clear_path,
            check_object_overlap,
            overlap_enemy_only,
            ignore_stealthed,
            fail_stealthed_without_feedback,
            check_shroud_revealed,
        }
    }

    /// Validate a foundation placement
    /// Matches C++ BuildAssistant::isLocationLegalToBuild lines 1651-1658
    pub fn validate_placement(
        &self,
        position: &Coord3D,
        template_name: &str,
        angle: f32,
        player_id: ObjectID,
    ) -> Result<(), String> {
        // Approximate BuildAssistant::isLocationLegalToBuild with available systems.

        // Check constraints

        // 1. Terrain Check
        if self.check_terrain {
            if let Some(terrain) = TheTerrainLogic::get() {
                let extent = terrain.get_maximum_pathfind_extent();
                if position.x < extent.lo.x
                    || position.x > extent.hi.x
                    || position.y < extent.lo.y
                    || position.y > extent.hi.y
                {
                    return Err("Location outside playable area".to_string());
                }

                if terrain.get_highest_layer_for_destination(position) != PathfindLayerEnum::Ground
                {
                    return Err("Location on bridge".to_string());
                }

                if terrain.is_underwater(position.x, position.y, None, None) {
                    return Err("Location underwater".to_string());
                }

                if terrain.is_cliff_cell(position.x, position.y) {
                    return Err("Location on cliff".to_string());
                }

                if let Some(template) = TheThingFactory::find_template(template_name) {
                    let geom = template.get_template_geometry_info();
                    let width = (geom.bounds.max.x - geom.bounds.min.x).abs();
                    let depth = (geom.bounds.max.y - geom.bounds.min.y).abs();
                    let (half_width, half_depth, is_circular) = match template
                        .get_template_geometry_type()
                        .unwrap_or(GeometryType::Cylinder)
                    {
                        GeometryType::Box => (width * 0.5, depth * 0.5, false),
                        GeometryType::Sphere | GeometryType::Cylinder => {
                            let radius = width.max(depth) * 0.5;
                            (radius, radius, true)
                        }
                    };

                    let sample =
                        |sample_x: f32, sample_y: f32, hi_z: &mut f32, lo_z: &mut f32| {
                            if terrain.get_highest_layer_for_destination(&Coord3D::new(
                                sample_x, sample_y, 0.0,
                            )) != PathfindLayerEnum::Ground
                            {
                                return Err("Location on bridge".to_string());
                            }
                            if terrain.is_underwater(sample_x, sample_y, None, None) {
                                return Err("Location underwater".to_string());
                            }
                            if terrain.is_cliff_cell(sample_x, sample_y) {
                                return Err("Location on cliff".to_string());
                            }
                            let z = terrain.get_ground_height(sample_x, sample_y, None);
                            *hi_z = hi_z.max(z);
                            *lo_z = lo_z.min(z);
                            Ok(())
                        };

                    let allowed_variation = global_data::read_safe()
                        .map(|data| data.allowed_height_variation_for_building)
                        .unwrap_or(3.0);

                    let sample_resolution = MAP_XY_FACTOR;
                    for &resolution in &[sample_resolution * 3.0, sample_resolution] {
                        let mut hi_z = extent.lo.z;
                        let mut lo_z = extent.hi.z;
                        let mut y = -half_depth;
                        while y < half_depth + resolution {
                            if y > half_depth {
                                y = half_depth;
                            }
                            let mut x = -half_width;
                            while x < half_width + resolution {
                                if x > half_width {
                                    x = half_width;
                                }

                                if is_circular && (x * x + y * y).sqrt() > half_width {
                                    x += resolution;
                                    continue;
                                }

                                let cos_angle = angle.cos();
                                let sin_angle = angle.sin();
                                let sample_x = position.x + x * cos_angle - y * sin_angle;
                                let sample_y = position.y + x * sin_angle + y * cos_angle;
                                sample(sample_x, sample_y, &mut hi_z, &mut lo_z)?;

                                x += resolution;
                            }
                            y += resolution;
                        }

                        if hi_z - lo_z > allowed_variation {
                            return Err("Location not flat enough".to_string());
                        }
                    }
                }
            }
        }

        // 1b. Shroud Check
        if self.check_shroud_revealed {
            if player_id != INVALID_OBJECT_ID {
                let player_index = player_id as u32;
                if player_index < MAX_PLAYER_COUNT as u32 {
                    let shroud = get_shroud_manager();
                    let shroud_guard = shroud
                        .lock()
                        .map_err(|_| "Failed to lock shroud manager".to_string())?;
                    if shroud_guard.has_shroud_grid() {
                        let state = shroud_guard.get_shroud_state(player_index, position);
                        if state != ShroudState::Visible {
                            return Err("Location not visible".to_string());
                        }
                    }
                }
            }
        }

        // 2. Clear Path Check
        if self.check_clear_path {
            if let Ok(ai_guard) = THE_AI.read() {
                if let Some(pathfinding) = ai_guard.pathfinding_system() {
                    if let Ok(pf) = pathfinding.read() {
                        let layer = TheTerrainLogic::get()
                            .map(|terrain| terrain.get_highest_layer_for_destination(position))
                            .unwrap_or(PathfindLayerEnum::Ground);
                        if !pf.is_cell_clear_at(position, layer) {
                            return Err("Build location not passable".to_string());
                        }
                    }
                }
            }
        }

        // 3. Object Overlap Check
        if self.check_object_overlap {
            // Check for objects near the build site
            let mut radius = 20.0;
            if let Some(template) = TheThingFactory::find_template(template_name) {
                let geom = template.get_template_geometry_info();
                let template_radius = geom.get_bounding_circle_radius();
                if template_radius > 0.0 {
                    radius = template_radius;
                }
            }
            let radius = radius.max(1.0);

            if let Ok(partition) =
                crate::object::collide::partition_manager::PARTITION_MANAGER.read()
            {
                let collide_pos =
                    crate::object::collide::Coord3D::new(position.x, position.y, position.z);
                let nearby = partition.find_objects_in_radius(&collide_pos, radius, &[]);
                if !nearby.is_empty() {
                    let builder_player = crate::system::game_logic::get_game_logic()
                        .lock()
                        .ok()
                        .and_then(|logic| logic.get_player(player_id as u32));

                    for object_id in nearby {
                        let Some(handle) = crate::object::OBJECT_REGISTRY.get_object(object_id)
                        else {
                            continue;
                        };
                        let Ok(obj_guard) = handle.read() else {
                            continue;
                        };

                        if obj_guard.is_stealthed() {
                            if self.ignore_stealthed {
                                continue;
                            }
                            if self.fail_stealthed_without_feedback {
                                return Err("Location blocked by stealth".to_string());
                            }
                        }

                        if obj_guard.is_kind_of(KindOf::Immobile) {
                            return Err("Location blocked by immobile object".to_string());
                        }

                        let Some(obj_player_id) = obj_guard.get_controlling_player_id() else {
                            continue;
                        };

                        if obj_player_id as ObjectID == player_id {
                            continue;
                        }

                        let Some(builder_player_arc) = builder_player.as_ref() else {
                            return Err("Location blocked by enemy object".to_string());
                        };

                        let Ok(builder_guard) = builder_player_arc.read() else {
                            return Err("Location blocked by enemy object".to_string());
                        };

                        let Some(other_player_arc) =
                            crate::player::player_list().read().ok().and_then(|list| {
                                obj_player_id
                                    .try_into()
                                    .ok()
                                    .and_then(|index| list.get_player(index).cloned())
                            })
                        else {
                            return Err("Location blocked by enemy object".to_string());
                        };
                        let Ok(other_guard) = other_player_arc.read() else {
                            return Err("Location blocked by enemy object".to_string());
                        };

                        if builder_guard.is_enemy_with_player(&other_guard) {
                            return Err("Location blocked by enemy object".to_string());
                        } else if !self.overlap_enemy_only {
                            return Err("Location blocked by object".to_string());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Construction progress tracker
/// Tracks construction percent and health restoration
#[derive(Debug, Clone)]
pub struct ConstructionProgress {
    /// Current construction percentage (0.0 to 100.0)
    /// Matches C++ Object::m_constructionPercent
    pub percent_complete: f32,
    /// Current health during construction
    /// Matches C++ BodyModule health during construction
    pub current_health: f32,
    /// Maximum health when complete
    pub max_health: f32,
    /// Frames spent under construction
    /// Matches C++ ProductionEntry::m_framesUnderConstruction line 139
    pub frames_under_construction: u32,
}

impl ConstructionProgress {
    /// Create new construction progress starting at 0%
    /// Matches C++ Object::setConstructionPercent(0.0) from DozerAIUpdate.cpp line 1704
    pub fn new(max_health: f32) -> Self {
        Self {
            percent_complete: 0.0,
            current_health: 1.0, // Start at 1 HP like C++ line 1708
            max_health,
            frames_under_construction: 0,
        }
    }

    /// Update construction by one frame
    /// Matches C++ DozerAIUpdate.cpp lines 515-527
    pub fn update_frame(&mut self, total_build_frames: u32) -> bool {
        self.frames_under_construction += 1;

        // Calculate percent progress this frame
        // C++ line 516: percentProgressThisFrame = 100.0f / framesToBuild;
        let percent_progress_this_frame = 100.0 / (total_build_frames as f32);
        self.percent_complete += percent_progress_this_frame;

        // Calculate health increase this frame
        // C++ lines 520-526: increase health proportionally
        let health_this_frame = self.max_health / (total_build_frames as f32);
        self.current_health = (self.current_health + health_this_frame).min(self.max_health);

        // Check if complete
        // C++ line 536: if( goalObject->getConstructionPercent() >= 100.0f )
        self.percent_complete >= 100.0
    }

    /// Check if construction is complete
    pub fn is_complete(&self) -> bool {
        self.percent_complete >= 100.0
    }

    /// Get completion ratio (0.0 to 1.0)
    pub fn completion_ratio(&self) -> f32 {
        (self.percent_complete / 100.0).min(1.0)
    }
}

/// Dozer construction task
/// Manages a dozer building a structure
#[derive(Debug, Clone)]
pub struct DozerConstructionTask {
    /// Object being constructed
    pub building_id: ObjectID,
    /// Dozer doing the construction
    pub dozer_id: ObjectID,
    /// Construction progress
    pub progress: ConstructionProgress,
    /// Total frames needed to build
    /// Calculated from ThingTemplate::calcTimeToBuild
    pub total_build_frames: u32,
    /// Whether this is a rebuild (free, no cost)
    /// Matches C++ DozerAIUpdate::m_isRebuild line 144
    pub is_rebuild: bool,
    /// Dock point where dozer should stand
    /// Matches C++ DozerAIUpdate dock points lines 1433-1439
    pub dock_point: Option<Coord3D>,
}

impl DozerConstructionTask {
    /// Create a new construction task
    /// Matches C++ DozerAIUpdate::construct lines 1608-1721
    pub fn new(
        building_id: ObjectID,
        dozer_id: ObjectID,
        max_health: f32,
        total_build_frames: u32,
        is_rebuild: bool,
    ) -> Self {
        Self {
            building_id,
            dozer_id,
            progress: ConstructionProgress::new(max_health),
            total_build_frames,
            is_rebuild,
            dock_point: None,
        }
    }

    /// Update construction by one frame
    /// Returns true if construction is now complete
    /// Matches C++ DozerActionDoActionState::update DOZER_TASK_BUILD case lines 476-641
    pub fn update_construction_frame(&mut self) -> bool {
        self.progress.update_frame(self.total_build_frames)
    }

    /// Set the dock point for the dozer
    /// Matches C++ DozerAIUpdate::findGoodBuildOrRepairPosition lines 1855-1896
    pub fn set_dock_point(&mut self, point: Coord3D) {
        self.dock_point = Some(point);
    }

    /// Get the dock point
    pub fn get_dock_point(&self) -> Option<&Coord3D> {
        self.dock_point.as_ref()
    }

    /// Check if construction is complete
    pub fn is_complete(&self) -> bool {
        self.progress.is_complete()
    }

    /// Get construction percent (0.0 to 100.0)
    pub fn get_percent_complete(&self) -> f32 {
        self.progress.percent_complete
    }
}

/// Construction interruption reasons
/// Tracks why construction was halted
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionInterruption {
    /// Dozer was killed or destroyed
    DozerDestroyed,
    /// Dozer received new orders
    DozerReassigned,
    /// Building was sold
    BuildingSold,
    /// Building was destroyed
    BuildingDestroyed,
    /// Construction cancelled by player
    Cancelled,
    /// Lost prerequisite during construction
    PrerequisiteLost,
    /// Insufficient power
    InsufficientPower,
}

/// Construction manager for tracking all ongoing construction
/// Manages multiple dozers building multiple structures
#[derive(Debug)]
pub struct ConstructionManager {
    /// Active construction tasks
    active_tasks: Vec<DozerConstructionTask>,
    /// Buildings currently under construction (set of IDs)
    buildings_under_construction: HashSet<ObjectID>,
    /// Dozers currently constructing (set of IDs)
    dozers_busy: HashSet<ObjectID>,
}

impl ConstructionManager {
    /// Create a new construction manager
    pub fn new() -> Self {
        Self {
            active_tasks: Vec::new(),
            buildings_under_construction: HashSet::new(),
            dozers_busy: HashSet::new(),
        }
    }

    /// Start a new construction task
    /// Matches C++ DozerAIUpdate::construct lines 1608-1721
    pub fn start_construction(
        &mut self,
        building_id: ObjectID,
        dozer_id: ObjectID,
        max_health: f32,
        total_build_frames: u32,
        is_rebuild: bool,
    ) -> Result<(), String> {
        // Check if building already being constructed
        if self.buildings_under_construction.contains(&building_id) {
            return Err("Building already under construction".to_string());
        }

        // Check if dozer already busy
        if self.dozers_busy.contains(&dozer_id) {
            return Err("Dozer already constructing".to_string());
        }

        // Create task
        let task = DozerConstructionTask::new(
            building_id,
            dozer_id,
            max_health,
            total_build_frames,
            is_rebuild,
        );

        // Track
        self.buildings_under_construction.insert(building_id);
        self.dozers_busy.insert(dozer_id);
        self.active_tasks.push(task);

        Ok(())
    }

    /// Update all construction tasks by one frame
    /// Returns list of completed building IDs
    pub fn update_frame(&mut self) -> Vec<ObjectID> {
        let mut completed_buildings = Vec::new();
        let mut completed_dozers = Vec::new();

        // Update each task
        for task in &mut self.active_tasks {
            if task.update_construction_frame() {
                completed_buildings.push(task.building_id);
                completed_dozers.push(task.dozer_id);
            }
        }

        // Remove completed tasks
        self.active_tasks
            .retain(|task| !completed_buildings.contains(&task.building_id));

        // Update tracking sets
        for building_id in &completed_buildings {
            self.buildings_under_construction.remove(building_id);
        }

        for dozer_id in &completed_dozers {
            self.dozers_busy.remove(dozer_id);
        }

        completed_buildings
    }

    /// Update construction for a single dozer by one frame.
    /// Returns completed building IDs for that dozer.
    pub fn update_for_dozer(&mut self, dozer_id: ObjectID) -> Vec<ObjectID> {
        let mut completed_buildings = Vec::new();
        let mut completed_dozers = Vec::new();

        for task in &mut self.active_tasks {
            if task.dozer_id == dozer_id {
                if task.update_construction_frame() {
                    completed_buildings.push(task.building_id);
                    completed_dozers.push(task.dozer_id);
                }
            }
        }

        self.active_tasks
            .retain(|task| !completed_buildings.contains(&task.building_id));

        for building_id in &completed_buildings {
            self.buildings_under_construction.remove(building_id);
        }
        for dozer_id in &completed_dozers {
            self.dozers_busy.remove(dozer_id);
        }

        completed_buildings
    }

    /// Cancel construction on a building
    /// Matches C++ cancelTask and related cleanup
    pub fn cancel_construction(
        &mut self,
        building_id: ObjectID,
        _reason: ConstructionInterruption,
    ) {
        // Find and remove task
        if let Some(pos) = self
            .active_tasks
            .iter()
            .position(|t| t.building_id == building_id)
        {
            let task = self.active_tasks.remove(pos);
            self.buildings_under_construction.remove(&building_id);
            self.dozers_busy.remove(&task.dozer_id);
        }
    }

    /// Cancel all construction by a specific dozer
    /// Used when dozer is destroyed or reassigned
    pub fn cancel_dozer_construction(&mut self, dozer_id: ObjectID) -> Vec<ObjectID> {
        let mut cancelled_buildings = Vec::new();

        // Find all tasks by this dozer
        let mut i = 0;
        while i < self.active_tasks.len() {
            if self.active_tasks[i].dozer_id == dozer_id {
                let task = self.active_tasks.remove(i);
                self.buildings_under_construction.remove(&task.building_id);
                cancelled_buildings.push(task.building_id);
            } else {
                i += 1;
            }
        }

        self.dozers_busy.remove(&dozer_id);
        cancelled_buildings
    }

    /// Get construction progress for a building
    pub fn get_progress(&self, building_id: ObjectID) -> Option<f32> {
        self.active_tasks
            .iter()
            .find(|t| t.building_id == building_id)
            .map(|t| t.get_percent_complete())
    }

    /// Get current construction health for a building.
    pub fn get_current_health(&self, building_id: ObjectID) -> Option<f32> {
        self.active_tasks
            .iter()
            .find(|t| t.building_id == building_id)
            .map(|t| t.progress.current_health)
    }

    /// Check if a building is under construction
    pub fn is_under_construction(&self, building_id: ObjectID) -> bool {
        self.buildings_under_construction.contains(&building_id)
    }

    /// Check if a dozer is busy constructing
    pub fn is_dozer_busy(&self, dozer_id: ObjectID) -> bool {
        self.dozers_busy.contains(&dozer_id)
    }

    /// Get all active construction tasks
    pub fn active_tasks(&self) -> &[DozerConstructionTask] {
        &self.active_tasks
    }
}

impl Default for ConstructionManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global construction manager instance (matches shared construction tracking in C++).
lazy_static::lazy_static! {
    pub static ref THE_CONSTRUCTION_MANAGER: Arc<RwLock<ConstructionManager>> =
        Arc::new(RwLock::new(ConstructionManager::new()));
}

/// Helper function to access the global construction manager.
pub fn get_construction_manager() -> Arc<RwLock<ConstructionManager>> {
    THE_CONSTRUCTION_MANAGER.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construction_progress() {
        let mut progress = ConstructionProgress::new(1000.0);

        assert_eq!(progress.percent_complete, 0.0);
        assert_eq!(progress.current_health, 1.0);
        assert!(!progress.is_complete());

        // Simulate 100 frame build
        for _ in 0..50 {
            progress.update_frame(100);
        }

        // Should be at 50%
        assert!((progress.percent_complete - 50.0).abs() < 1.0);
        assert!((progress.current_health - 500.0).abs() < 10.0);
        assert!(!progress.is_complete());

        // Complete construction
        for _ in 50..100 {
            progress.update_frame(100);
        }

        assert!(progress.is_complete());
        assert!(progress.percent_complete >= 100.0);
        assert!((progress.current_health - 1000.0).abs() < 1.0);
    }

    #[test]
    fn test_construction_task() {
        let mut task = DozerConstructionTask::new(1, 2, 500.0, 100, false);

        assert_eq!(task.building_id, 1);
        assert_eq!(task.dozer_id, 2);
        assert!(!task.is_rebuild);
        assert!(!task.is_complete());

        // Build halfway
        for _ in 0..50 {
            assert!(!task.update_construction_frame());
        }

        assert!((task.get_percent_complete() - 50.0).abs() < 1.0);

        // Complete
        for _ in 50..100 {
            task.update_construction_frame();
        }

        assert!(task.is_complete());
    }

    #[test]
    fn test_construction_manager() {
        let mut manager = ConstructionManager::new();

        // Start construction
        assert!(manager
            .start_construction(1, 10, 1000.0, 100, false)
            .is_ok());

        assert!(manager.is_under_construction(1));
        assert!(manager.is_dozer_busy(10));
        assert_eq!(manager.get_progress(1), Some(0.0));

        // Can't double-build
        assert!(manager
            .start_construction(1, 11, 1000.0, 100, false)
            .is_err());
        assert!(manager
            .start_construction(2, 10, 1000.0, 100, false)
            .is_err());

        // Update frames
        for _ in 0..99 {
            let completed = manager.update_frame();
            assert!(completed.is_empty());
        }

        // Complete on frame 100
        let completed = manager.update_frame();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], 1);

        assert!(!manager.is_under_construction(1));
        assert!(!manager.is_dozer_busy(10));
    }

    #[test]
    fn test_construction_cancellation() {
        let mut manager = ConstructionManager::new();

        manager
            .start_construction(1, 10, 1000.0, 100, false)
            .unwrap();
        manager
            .start_construction(2, 11, 1000.0, 100, false)
            .unwrap();

        assert_eq!(manager.active_tasks().len(), 2);

        // Cancel one building
        manager.cancel_construction(1, ConstructionInterruption::Cancelled);

        assert_eq!(manager.active_tasks().len(), 1);
        assert!(!manager.is_under_construction(1));
        assert!(!manager.is_dozer_busy(10));
        assert!(manager.is_under_construction(2));

        // Cancel by dozer
        let cancelled = manager.cancel_dozer_construction(11);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0], 2);
        assert_eq!(manager.active_tasks().len(), 0);
    }

    #[test]
    fn test_rebuild_flag() {
        let task = DozerConstructionTask::new(1, 2, 500.0, 100, true);
        assert!(task.is_rebuild);

        let task2 = DozerConstructionTask::new(1, 2, 500.0, 100, false);
        assert!(!task2.is_rebuild);
    }
}
