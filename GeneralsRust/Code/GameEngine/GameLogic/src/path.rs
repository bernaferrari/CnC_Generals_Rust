//! Pathfinding system - complete implementation matching C++ AIPathfind
//!
//! This module implements the full pathfinding system from the original C++ codebase,
//! including A* pathfinding, hierarchical zones, path optimization, and collision detection.

pub mod collision_map;
pub mod environment;
pub mod navigation_map;
pub mod path;
pub mod path_node;
pub mod path_optimization;
pub mod pathfind_cell;
pub mod pathfind_cell_info;
pub mod pathfind_layer;
pub mod pathfinder;
pub mod zone_block;
pub mod zone_manager;

pub use collision_map::*;
pub use environment::*;
pub use navigation_map::*;
pub use path::*;
pub use path_node::*;
pub use path_optimization::*;
pub use pathfind_cell::*;
pub use pathfind_cell_info::*;
pub use pathfind_layer::*;
pub use pathfinder::*;
pub use zone_block::*;
pub use zone_manager::*;

use crate::common::*;
use crate::locomotor::SURFACE_AIR;

pub use crate::waypoint::Waypoint;

// Constants matching C++ implementation
pub const PATHFIND_CELL_SIZE: i32 = 10;
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
pub const PATHFIND_CLOSE_ENOUGH: f32 = 1.0;
pub const PATH_MAX_PRIORITY: i32 = 0x7FFFFFFF;
pub const PATHFIND_QUEUE_LEN: usize = 512;
pub const MAX_WALL_PIECES: usize = 128;
pub const LAYER_Z_CLOSE_ENOUGH_F: f32 = 10.0;

/// Path identifier
pub type PathId = u32;

/// Zone storage type matching C++ implementation
pub type ZoneStorageType = u16;

/// Path layer enumeration matching C++ PathfindLayerEnum
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    Top = 2,
    // Additional layers for bridges and walls
    Bridge1 = 3,
    Bridge2 = 4,
    Bridge3 = 5,
    Bridge4 = 6,
    Wall = 7,
    Last = 8,
}

impl Default for PathfindLayerEnum {
    fn default() -> Self {
        PathfindLayerEnum::Invalid
    }
}

/// Locomotor surface type mask for terrain compatibility
pub type LocomotorSurfaceTypeMask = u32;

/// Surface type constants
pub const SURFACE_GROUND: LocomotorSurfaceTypeMask = 0x01;
pub const SURFACE_WATER: LocomotorSurfaceTypeMask = 0x02;
pub const SURFACE_CLIFF: LocomotorSurfaceTypeMask = 0x04;
pub const SURFACE_RUBBLE: LocomotorSurfaceTypeMask = 0x08;

/// Pathfinding services interface trait matching C++ PathfindServicesInterface
pub trait PathfindServicesInterface {
    /// Find a short, valid path between given locations
    fn find_path(
        &mut self,
        obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> Option<PathHandle>;

    /// Find a short, valid path to a location NEAR the destination
    /// This succeeds when the destination is unreachable (like inside a building)
    fn find_closest_path(
        &mut self,
        obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        to: &mut Coord3D,
        blocked: bool,
        path_cost_multiplier: f32,
        move_allies: bool,
    ) -> Option<PathHandle>;

    /// Find a short, valid path to a location that obj can attack victim from
    fn find_attack_path(
        &mut self,
        obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        victim: ObjectID,
        victim_pos: &Coord3D,
        weapon: Option<&WeaponHandle>,
    ) -> Option<PathHandle>;

    /// Patch to the existing path from the current position
    fn patch_path(
        &mut self,
        obj: ObjectID,
        locomotor_set: &LocomotorSet,
        original_path: PathHandle,
        blocked: bool,
    ) -> Option<PathHandle>;

    /// Find a short, valid path to a location that is away from the repulsors
    fn find_safe_path(
        &mut self,
        obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        repulsor_pos1: &Coord3D,
        repulsor_pos2: &Coord3D,
        repulsor_radius: f32,
    ) -> Option<PathHandle>;
}

/// Locomotor set representing movement capabilities
#[derive(Debug, Clone)]
pub struct LocomotorSet {
    valid_surfaces: LocomotorSurfaceTypeMask,
    is_crusher: bool,
    radius: f32,
}

impl LocomotorSet {
    pub fn new(valid_surfaces: LocomotorSurfaceTypeMask, is_crusher: bool, radius: f32) -> Self {
        Self {
            valid_surfaces,
            is_crusher,
            radius,
        }
    }

    pub fn get_valid_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.valid_surfaces
    }

    pub fn is_crusher(&self) -> bool {
        self.is_crusher
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }

    pub fn can_move_on_surface(&self, surface: LocomotorSurfaceTypeMask) -> bool {
        (self.valid_surfaces & surface) != 0
    }
}

/// Weapon handle for attack pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeaponHandle(pub u32);

/// Path handle for managing paths
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathHandle(pub u32);

/// Inline helper functions matching C++ implementation

/// Convert world coordinates to pathfinding grid coordinates
pub fn world_to_grid(pos: &Coord3D) -> ICoord2D {
    ICoord2D::new(
        (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
        (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
    )
}

/// Convert grid coordinates to world coordinates
pub fn grid_to_world(cell: &ICoord2D, layer: PathfindLayerEnum) -> Coord3D {
    let mut pos = Coord3D::new(
        cell.x as f32 * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F / 2.0,
        cell.y as f32 * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F / 2.0,
        0.0,
    );

    // Adjust Z based on terrain layer height when available (C++ uses TerrainLogic::getLayerHeight)
    if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
        let common_layer = crate::common::PathfindLayerEnum::from_u32(layer as u32);
        pos.z = terrain.get_layer_height(pos.x, pos.y, common_layer);
    } else {
        match layer {
            PathfindLayerEnum::Ground => pos.z = 0.0,
            PathfindLayerEnum::Top => pos.z = 10.0,
            _ => pos.z = 5.0, // Default for bridges/walls
        }
    }

    pos
}

// Helper functions using types from sub-modules

/// Check if a cell type is impassable to ground units
pub fn is_impassable(cell_type: PathfindCellType) -> bool {
    matches!(
        cell_type,
        PathfindCellType::Impassable
            | PathfindCellType::Obstacle
            | PathfindCellType::BridgeImpassable
    )
}

/// Valid locomotor surfaces for cell type
pub fn valid_locomotor_surfaces_for_cell_type(
    cell_type: PathfindCellType,
) -> LocomotorSurfaceTypeMask {
    #[allow(unreachable_patterns)]
    match cell_type {
        PathfindCellType::Clear => SURFACE_GROUND | SURFACE_AIR,
        PathfindCellType::Water => SURFACE_WATER | SURFACE_AIR,
        PathfindCellType::Cliff => SURFACE_CLIFF | SURFACE_AIR,
        PathfindCellType::Rubble => SURFACE_RUBBLE | SURFACE_AIR,
        PathfindCellType::Impassable
        | PathfindCellType::BridgeImpassable
        | PathfindCellType::Obstacle => SURFACE_AIR,
        _ => 0,
    }
}

// Additional types referenced in other modules

/// Pathfinding map structure
#[derive(Debug, Clone)]
pub struct PathfindMap {
    width: u32,
    height: u32,
    cells: Vec<Vec<PathfindCellType>>,
}

impl PathfindMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells: vec![vec![PathfindCellType::Clear; height as usize]; width as usize],
        }
    }

    pub fn get_cell_type(&self, x: u32, y: u32) -> Option<PathfindCellType> {
        if x < self.width && y < self.height {
            Some(self.cells[x as usize][y as usize])
        } else {
            None
        }
    }

    pub fn set_cell_type(&mut self, x: u32, y: u32, cell_type: PathfindCellType) {
        if x < self.width && y < self.height {
            self.cells[x as usize][y as usize] = cell_type;
        }
    }
}
