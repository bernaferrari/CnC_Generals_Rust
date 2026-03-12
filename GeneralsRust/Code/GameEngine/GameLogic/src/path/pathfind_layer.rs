//! PathfindLayer implementation matching C++ PathfindLayer class
//!
//! This class represents a bridge or wall layer in the map.
//! This is effectively a sub-rectangle of the big pathfind map.
#![allow(missing_docs, deprecated, unused_variables)]

use super::{PathNode, PathfindCell, PathfindCellInfo, PathfindCellType, PATHFIND_CELL_SIZE_F};
use crate::common::{Coord3D, ICoord2D, IRegion2D, ObjectID};
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::{PathfindLayerEnum, ZoneStorageType};
use std::collections::HashMap;

/// PathfindLayer structure matching C++ PathfindLayer class
#[derive(Debug)]
pub struct PathfindLayer {
    // Cell storage
    block_of_map_cells: Vec<PathfindCell>, // Pathfinding map - contains iconic representation
    layer_cells: Vec<Vec<*mut PathfindCell>>, // Pathfinding map indexes - matrix indexing

    // Dimensions
    width: i32,    // Number of cells in x
    height: i32,   // Number of cells in y
    x_origin: i32, // Index of first cell in x
    y_origin: i32, // Index of first cell in y

    // Bridge connection points
    start_cell: ICoord2D, // pathfind cell indexes for center cell on the from side
    end_cell: ICoord2D,   // pathfind cell indexes for center cell on the to side

    // Layer properties
    layer: PathfindLayerEnum, // Layer type
    zone: i32,                // Whole bridge is in same zone
    bridge_id: ObjectID,      // Corresponding bridge object ID
    destroyed: bool,          // Whether the bridge/wall is destroyed
}

impl PathfindLayer {
    /// Create a new PathfindLayer
    pub fn new() -> Self {
        Self {
            block_of_map_cells: Vec::new(),
            layer_cells: Vec::new(),
            width: 0,
            height: 0,
            x_origin: 0,
            y_origin: 0,
            start_cell: ICoord2D::new(0, 0),
            end_cell: ICoord2D::new(0, 0),
            layer: PathfindLayerEnum::Invalid,
            zone: 0,
            bridge_id: crate::common::INVALID_ID,
            destroyed: false,
        }
    }

    /// Reset the layer
    pub fn reset(&mut self) {
        self.block_of_map_cells.clear();
        self.layer_cells.clear();
        self.width = 0;
        self.height = 0;
        self.x_origin = 0;
        self.y_origin = 0;
        self.start_cell = ICoord2D::new(0, 0);
        self.end_cell = ICoord2D::new(0, 0);
        self.layer = PathfindLayerEnum::Invalid;
        self.zone = 0;
        self.bridge_id = crate::common::INVALID_ID;
        self.destroyed = false;
    }

    /// Initialize layer for a bridge
    pub fn init(&mut self, bridge_id: ObjectID, layer: PathfindLayerEnum) -> bool {
        self.bridge_id = bridge_id;
        self.layer = layer;
        self.destroyed = false;
        true
    }

    /// Allocate cells for the layer
    pub fn allocate_cells(&mut self, extent: &IRegion2D) {
        self.x_origin = extent.lo.x;
        self.y_origin = extent.lo.y;
        self.width = extent.hi.x - extent.lo.x + 1;
        self.height = extent.hi.y - extent.lo.y + 1;

        // Allocate cell storage
        let total_cells = (self.width * self.height) as usize;
        self.block_of_map_cells.clear();
        self.block_of_map_cells
            .resize_with(total_cells, PathfindCell::default);

        // Allocate index matrix
        self.layer_cells.clear();
        self.layer_cells.resize(self.width as usize, Vec::new());

        for x in 0..self.width {
            self.layer_cells[x as usize].resize(self.height as usize, std::ptr::null_mut());

            for y in 0..self.height {
                let index = (y * self.width + x) as usize;
                self.layer_cells[x as usize][y as usize] =
                    &mut self.block_of_map_cells[index] as *mut PathfindCell;
            }
        }

        // Set layer on all cells
        for cell in &mut self.block_of_map_cells {
            cell.set_layer(self.layer);
        }
    }

    /// Allocate cells for wall layer with specific wall pieces
    pub fn allocate_cells_for_wall_layer(&mut self, extent: &IRegion2D, wall_pieces: &[ObjectID]) {
        self.allocate_cells(extent);

        // Classify wall cells
        self.classify_wall_cells(wall_pieces);
    }

    /// Classify cells based on terrain and obstacles
    pub fn classify_cells(&mut self) {
        let layer = self.layer;
        let x_origin = self.x_origin;
        let y_origin = self.y_origin;
        let destroyed = self.destroyed;

        // Basic terrain classification
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(cell) = self.get_cell_mut(x, y) {
                    Self::classify_layer_map_cell_static(
                        layer, x_origin, y_origin, x, y, cell, destroyed,
                    );
                }
            }
        }
    }

    /// Classify wall cells
    pub fn classify_wall_cells(&mut self, wall_pieces: &[ObjectID]) {
        let x_origin = self.x_origin;
        let y_origin = self.y_origin;

        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(cell) = self.get_cell_mut(x, y) {
                    Self::classify_wall_map_cell_static(
                        x_origin,
                        y_origin,
                        x,
                        y,
                        cell,
                        wall_pieces,
                    );
                }
            }
        }
    }

    /// Internal cell classification for layers
    fn classify_layer_map_cell(&mut self, x: i32, y: i32, cell: &mut PathfindCell) {
        // Convert to world coordinates
        let _world_x = (self.x_origin + x) as f32 * PATHFIND_CELL_SIZE_F;
        let _world_y = (self.y_origin + y) as f32 * PATHFIND_CELL_SIZE_F;

        // Default to clear for bridges
        cell.set_type(PathfindCellType::Clear);

        // Set layer-specific properties
        match self.layer {
            PathfindLayerEnum::Bridge1
            | PathfindLayerEnum::Bridge2
            | PathfindLayerEnum::Bridge3
            | PathfindLayerEnum::Bridge4 => {
                // Bridge cells - check if this is a valid bridge section
                if self.is_valid_bridge_section(x, y) {
                    cell.set_type(PathfindCellType::Clear);
                } else {
                    cell.set_type(PathfindCellType::BridgeImpassable);
                }
            }
            PathfindLayerEnum::Wall => {
                // Wall cells are typically impassable
                cell.set_type(PathfindCellType::Impassable);
            }
            _ => {
                cell.set_type(PathfindCellType::Clear);
            }
        }
    }

    /// Internal cell classification for walls
    fn classify_wall_map_cell(
        &mut self,
        x: i32,
        y: i32,
        cell: &mut PathfindCell,
        wall_pieces: &[ObjectID],
    ) {
        let world_pos = Coord3D::new(
            (self.x_origin + x) as f32 * PATHFIND_CELL_SIZE_F,
            (self.y_origin + y) as f32 * PATHFIND_CELL_SIZE_F,
            0.0,
        );

        // Check if this point is on a wall
        if self.is_point_on_wall(wall_pieces, &world_pos) {
            cell.set_type(PathfindCellType::Obstacle);
            // Set the first wall piece as the obstacle ID (simplified)
            if !wall_pieces.is_empty() {
                let pos = ICoord2D::new(self.x_origin + x, self.y_origin + y);
                cell.set_type_as_obstacle(wall_pieces[0], true, &pos);
            }
        } else {
            cell.set_type(PathfindCellType::Clear);
        }
    }

    /// Check if a position is on any wall piece
    fn is_point_on_wall(&self, wall_pieces: &[ObjectID], pt: &Coord3D) -> bool {
        let cell_pad = PATHFIND_CELL_SIZE_F * 0.5;
        for &wall_id in wall_pieces {
            let Some(wall_arc) = OBJECT_REGISTRY.get_object(wall_id) else {
                continue;
            };
            let Ok(wall_guard) = wall_arc.read() else {
                continue;
            };

            let wall_pos = wall_guard.get_position();
            let radius = wall_guard.get_geometry_info().get_major_radius();

            let dx = wall_pos.x - pt.x;
            let dy = wall_pos.y - pt.y;
            let dist_sq = dx * dx + dy * dy;
            let allowed = radius + cell_pad;
            if dist_sq <= allowed * allowed {
                return true;
            }
        }
        false
    }

    /// Check if this is a valid bridge section (not destroyed)
    fn is_valid_bridge_section(&self, _x: i32, _y: i32) -> bool {
        !self.destroyed
    }

    /// Set destroyed state
    pub fn set_destroyed(&mut self, destroyed: bool) -> bool {
        if self.destroyed != destroyed {
            self.destroyed = destroyed;

            // Update all cells when bridge state changes
            if destroyed {
                // Mark all bridge cells as impassable
                for cell in &mut self.block_of_map_cells {
                    if cell.get_type() == PathfindCellType::Clear {
                        cell.set_type(PathfindCellType::BridgeImpassable);
                    }
                }
            } else {
                // Restore bridge cells
                for cell in &mut self.block_of_map_cells {
                    if cell.get_type() == PathfindCellType::BridgeImpassable {
                        cell.set_type(PathfindCellType::Clear);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Check if layer is unused (contains no bridge)
    pub fn is_unused(&self) -> bool {
        self.bridge_id == crate::common::INVALID_ID
    }

    /// Check if layer is destroyed
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Get cell at local coordinates
    pub fn get_cell(&self, x: i32, y: i32) -> Option<&PathfindCell> {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            let index = (y * self.width + x) as usize;
            self.block_of_map_cells.get(index)
        } else {
            None
        }
    }

    /// Get mutable cell at local coordinates
    pub fn get_cell_mut(&mut self, x: i32, y: i32) -> Option<&mut PathfindCell> {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            let index = (y * self.width + x) as usize;
            self.block_of_map_cells.get_mut(index)
        } else {
            None
        }
    }

    /// Get cell at global coordinates
    pub fn get_cell_global(&self, global_x: i32, global_y: i32) -> Option<&PathfindCell> {
        let local_x = global_x - self.x_origin;
        let local_y = global_y - self.y_origin;
        self.get_cell(local_x, local_y)
    }

    /// Get zone
    pub fn get_zone(&self) -> i32 {
        self.zone
    }

    /// Set zone
    pub fn set_zone(&mut self, zone: i32) {
        self.zone = zone;
    }

    /// Apply zone to all cells
    pub fn apply_zone(&mut self) {
        for cell in &mut self.block_of_map_cells {
            cell.set_zone(self.zone as ZoneStorageType);
        }
    }

    /// Get start cell index
    pub fn get_start_cell_index(&self) -> ICoord2D {
        self.start_cell
    }

    /// Get end cell index
    pub fn get_end_cell_index(&self) -> ICoord2D {
        self.end_cell
    }

    /// Set start cell index
    pub fn set_start_cell_index(&mut self, start: ICoord2D) {
        self.start_cell = start;
    }

    /// Set end cell index
    pub fn set_end_cell_index(&mut self, end: ICoord2D) {
        self.end_cell = end;
    }

    /// Get bridge ID
    pub fn get_bridge_id(&self) -> ObjectID {
        self.bridge_id
    }

    /// Get layer type
    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    /// Check if this layer connects two zones
    pub fn connects_zones(&self, zone1: ZoneStorageType, zone2: ZoneStorageType) -> bool {
        // Simplified implementation - check if bridge spans different zones
        if self.destroyed || self.is_unused() {
            false
        } else {
            // A bridge connects zones if it has cells in both zones
            let mut has_zone1 = false;
            let mut has_zone2 = false;

            for cell in &self.block_of_map_cells {
                let cell_zone = cell.get_zone();
                if cell_zone == zone1 {
                    has_zone1 = true;
                }
                if cell_zone == zone2 {
                    has_zone2 = true;
                }
                if has_zone1 && has_zone2 {
                    return true;
                }
            }
            false
        }
    }

    /// Get layer dimensions
    pub fn get_dimensions(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    /// Get layer origin
    pub fn get_origin(&self) -> (i32, i32) {
        (self.x_origin, self.y_origin)
    }

    /// Check if point is within layer bounds
    pub fn contains_point(&self, global_x: i32, global_y: i32) -> bool {
        global_x >= self.x_origin
            && global_x < self.x_origin + self.width
            && global_y >= self.y_origin
            && global_y < self.y_origin + self.height
    }
}

impl Default for PathfindLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl PathfindLayer {
    /// Static version of classify_layer_map_cell to avoid borrowing conflicts
    fn classify_layer_map_cell_static(
        layer: PathfindLayerEnum,
        x_origin: i32,
        y_origin: i32,
        x: i32,
        y: i32,
        cell: &mut PathfindCell,
        destroyed: bool,
    ) {
        // Convert to world coordinates
        let _world_x = (x_origin + x) as f32 * PATHFIND_CELL_SIZE_F;
        let _world_y = (y_origin + y) as f32 * PATHFIND_CELL_SIZE_F;

        // Default to clear for bridges
        cell.set_type(PathfindCellType::Clear);

        // Set layer-specific properties
        match layer {
            PathfindLayerEnum::Bridge1
            | PathfindLayerEnum::Bridge2
            | PathfindLayerEnum::Bridge3
            | PathfindLayerEnum::Bridge4 => {
                // Bridge cells - check if this is a valid bridge section
                if !destroyed {
                    cell.set_type(PathfindCellType::Clear);
                } else {
                    cell.set_type(PathfindCellType::BridgeImpassable);
                }
            }
            PathfindLayerEnum::Wall => {
                // Wall cells are typically impassable
                cell.set_type(PathfindCellType::Impassable);
            }
            _ => {
                cell.set_type(PathfindCellType::Clear);
            }
        }
    }

    /// Static version of classify_wall_map_cell to avoid borrowing conflicts
    fn classify_wall_map_cell_static(
        x_origin: i32,
        y_origin: i32,
        x: i32,
        y: i32,
        cell: &mut PathfindCell,
        wall_pieces: &[ObjectID],
    ) {
        let _world_pos = Coord3D::new(
            (x_origin + x) as f32 * PATHFIND_CELL_SIZE_F,
            (y_origin + y) as f32 * PATHFIND_CELL_SIZE_F,
            0.0,
        );

        // Check if this point is on a wall (simplified implementation)
        if !wall_pieces.is_empty() {
            cell.set_type(PathfindCellType::Obstacle);
            // Set the first wall piece as the obstacle ID (simplified)
            let pos = ICoord2D::new(x_origin + x, y_origin + y);
            cell.set_type_as_obstacle(wall_pieces[0], true, &pos);
        } else {
            cell.set_type(PathfindCellType::Clear);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathfind_layer_creation() {
        let layer = PathfindLayer::new();
        assert!(layer.is_unused());
        assert!(!layer.is_destroyed());
        assert_eq!(layer.get_zone(), 0);
        assert_eq!(layer.get_layer(), PathfindLayerEnum::Invalid);
    }

    #[test]
    fn test_pathfind_layer_init() {
        let mut layer = PathfindLayer::new();
        let bridge_id = 123;

        assert!(layer.init(bridge_id, PathfindLayerEnum::Bridge1));
        assert_eq!(layer.get_bridge_id(), bridge_id);
        assert_eq!(layer.get_layer(), PathfindLayerEnum::Bridge1);
        assert!(!layer.is_unused());
    }

    #[test]
    fn test_pathfind_layer_allocate_cells() {
        let mut layer = PathfindLayer::new();
        let extent = IRegion2D {
            lo: ICoord2D::new(10, 10),
            hi: ICoord2D::new(15, 15),
        };

        layer.init(1, PathfindLayerEnum::Bridge1);
        layer.allocate_cells(&extent);

        let (width, height) = layer.get_dimensions();
        assert_eq!(width, 6); // 15 - 10 + 1
        assert_eq!(height, 6);

        let (x_origin, y_origin) = layer.get_origin();
        assert_eq!(x_origin, 10);
        assert_eq!(y_origin, 10);
    }

    #[test]
    fn test_pathfind_layer_destroyed() {
        let mut layer = PathfindLayer::new();
        layer.init(1, PathfindLayerEnum::Bridge1);

        assert!(!layer.is_destroyed());
        assert!(layer.set_destroyed(true));
        assert!(layer.is_destroyed());
        assert!(!layer.set_destroyed(true)); // No change, should return false
    }

    #[test]
    fn test_pathfind_layer_zone() {
        let mut layer = PathfindLayer::new();
        assert_eq!(layer.get_zone(), 0);

        layer.set_zone(42);
        assert_eq!(layer.get_zone(), 42);
    }

    #[test]
    fn test_pathfind_layer_contains_point() {
        let mut layer = PathfindLayer::new();
        let extent = IRegion2D {
            lo: ICoord2D::new(10, 10),
            hi: ICoord2D::new(15, 15),
        };

        layer.allocate_cells(&extent);

        assert!(layer.contains_point(12, 13));
        assert!(!layer.contains_point(5, 5));
        assert!(!layer.contains_point(20, 20));
    }
}
