//! Pathfinding cells, layers, terrain metadata and coordinate helpers.
//!
//! The current AI Pathfinder is `crate::ai::Pathfinder`. Canonical UnitAI path
//! snapshots use the owned `crate::ai::pathfind::Path`.

pub mod collision_map;
pub mod navigation_map;
pub mod pathfind_cell;
pub mod pathfind_cell_info;
pub mod pathfind_layer;
pub mod pathfind_layer_classify;

pub mod zone_block;
pub mod zone_manager;

pub use collision_map::*;
pub use navigation_map::*;
pub use pathfind_cell::*;
pub use pathfind_cell_info::*;
pub use pathfind_layer::*;
pub use zone_block::*;
pub use zone_manager::*;

use crate::common::*;
pub use crate::waypoint::Waypoint;
pub use generals_sim::pathfinding::{
    LAYER_LAST, LAYER_WALL, LAYER_Z_CLOSE_ENOUGH_F, LocomotorSurfaceTypeMask, MAX_WALL_PIECES,
    PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F, PATHFIND_CLOSE_ENOUGH, PATHFIND_QUEUE_LEN,
    PathfindLayerEnum, SURFACE_AIR, SURFACE_CLIFF, SURFACE_GROUND, SURFACE_RUBBLE, SURFACE_WATER,
};

pub const PATH_MAX_PRIORITY: i32 = 0x7FFFFFFF;

/// Path identifier
pub type PathId = u32;

/// Zone storage type matching C++ implementation
pub type ZoneStorageType = u16;

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

    if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
        let common_layer = crate::common::PathfindLayerEnum::from_u32(layer as u32);
        pos.z = terrain.get_layer_height(pos.x, pos.y, common_layer);
    } else {
        match layer {
            PathfindLayerEnum::Ground => pos.z = 0.0,
            PathfindLayerEnum::Top => pos.z = 10.0,
            PathfindLayerEnum::Wall => pos.z = 10.0,
            _ => pos.z = 5.0,
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
