//! ZoneBlock implementation matching C++ ZoneBlock class
//!
//! This class is a helper class for zone manager. It maintains information regarding the
//! LocomotorSurfaceTypeMask equivalencies within a ZONE_BLOCK_SIZE x ZONE_BLOCK_SIZE area of
//! cells. This is used in hierarchical pathfinding to find the best coarse path at the
//! block level.

use super::{PathfindCell, PathfindCellType, PathfindLayer};
use crate::common::{ICoord2D, IRegion2D};
use crate::locomotor::SURFACE_AIR;
use crate::path::{
    LocomotorSurfaceTypeMask, ZoneStorageType, SURFACE_CLIFF, SURFACE_GROUND, SURFACE_RUBBLE,
    SURFACE_WATER,
};

/// Zone block size matching C++ implementation
pub const ZONE_BLOCK_SIZE: i32 = 10;

/// ZoneBlock structure matching C++ ZoneBlock class
#[derive(Debug)]
pub struct ZoneBlock {
    // Block position
    cell_origin: ICoord2D,

    // Zone information
    first_zone: ZoneStorageType, // First zone in this block
    num_zones: u16,              // Number of zones in this block

    // Equivalency tables for different locomotor types
    zones_allocated: u16,
    ground_cliff_zones: Vec<ZoneStorageType>, // Ground units that can climb cliffs
    ground_water_zones: Vec<ZoneStorageType>, // Amphibious units
    ground_rubble_zones: Vec<ZoneStorageType>, // Units that can move through rubble
    crusher_zones: Vec<ZoneStorageType>,      // Crusher units

    // Block state
    interacts_with_bridge: bool, // True if this block contains bridge connections
    marked_passable: bool,       // For hierarchical pathfinding
}

impl ZoneBlock {
    /// Create a new ZoneBlock
    pub fn new() -> Self {
        Self {
            cell_origin: ICoord2D::new(0, 0),
            first_zone: 0,
            num_zones: 0,
            zones_allocated: 0,
            ground_cliff_zones: Vec::new(),
            ground_water_zones: Vec::new(),
            ground_rubble_zones: Vec::new(),
            crusher_zones: Vec::new(),
            interacts_with_bridge: false,
            marked_passable: true,
        }
    }

    /// Calculate zones for this block
    pub fn block_calculate_zones(
        &mut self,
        map: &[Vec<*const PathfindCell>],
        _layers: &[PathfindLayer],
        bounds: &IRegion2D,
    ) {
        self.cell_origin = bounds.lo;
        self.reset_zones();

        let mut min_zone = ZoneStorageType::MAX;
        let mut max_zone = 0;

        for y in bounds.lo.y..=bounds.hi.y {
            for x in bounds.lo.x..=bounds.hi.x {
                if let Some(cell) = self.get_cell_from_map(map, x, y, bounds) {
                    let zone = unsafe { (*cell).get_zone() };
                    min_zone = min_zone.min(zone);
                    max_zone = max_zone.max(zone);
                }
            }
        }

        if min_zone == ZoneStorageType::MAX {
            return;
        }

        self.first_zone = min_zone;
        self.num_zones = max_zone.saturating_sub(min_zone).saturating_add(1) as u16;

        self.allocate_zones();

        if self.num_zones <= 1 {
            return;
        }

        for i in 0..self.zones_allocated as usize {
            let zone = self.first_zone + i as ZoneStorageType;
            self.ground_cliff_zones[i] = zone;
            self.ground_water_zones[i] = zone;
            self.ground_rubble_zones[i] = zone;
            self.crusher_zones[i] = zone;
        }

        for y in bounds.lo.y..=bounds.hi.y {
            for x in bounds.lo.x..=bounds.hi.x {
                let Some(cell) = self.get_cell_from_map(map, x, y, bounds) else {
                    continue;
                };
                let cell_zone = unsafe { (*cell).get_zone() };
                if cell_zone == 0 {
                    continue;
                }

                if x > bounds.lo.x {
                    let Some(left_cell) = self.get_cell_from_map(map, x - 1, y, bounds) else {
                        continue;
                    };
                    if cell_zone != unsafe { (*left_cell).get_zone() } {
                        let cell_ref = unsafe { &*cell };
                        let left_ref = unsafe { &*left_cell };

                        if water_ground(cell_ref, left_ref) {
                            apply_block_zone(
                                cell,
                                left_cell,
                                &mut self.ground_water_zones,
                                self.first_zone,
                            );
                        }
                        if ground_rubble(cell_ref, left_ref) {
                            apply_block_zone(
                                cell,
                                left_cell,
                                &mut self.ground_rubble_zones,
                                self.first_zone,
                            );
                        }
                        if ground_cliff(cell_ref, left_ref) {
                            apply_block_zone(
                                cell,
                                left_cell,
                                &mut self.ground_cliff_zones,
                                self.first_zone,
                            );
                        }
                        if crusher_ground(cell_ref, left_ref) {
                            apply_block_zone(
                                cell,
                                left_cell,
                                &mut self.crusher_zones,
                                self.first_zone,
                            );
                        }
                    }
                }

                if y > bounds.lo.y {
                    let Some(top_cell) = self.get_cell_from_map(map, x, y - 1, bounds) else {
                        continue;
                    };
                    if cell_zone != unsafe { (*top_cell).get_zone() } {
                        let cell_ref = unsafe { &*cell };
                        let top_ref = unsafe { &*top_cell };

                        if water_ground(cell_ref, top_ref) {
                            apply_block_zone(
                                cell,
                                top_cell,
                                &mut self.ground_water_zones,
                                self.first_zone,
                            );
                        }
                        if ground_rubble(cell_ref, top_ref) {
                            apply_block_zone(
                                cell,
                                top_cell,
                                &mut self.ground_rubble_zones,
                                self.first_zone,
                            );
                        }
                        if ground_cliff(cell_ref, top_ref) {
                            apply_block_zone(
                                cell,
                                top_cell,
                                &mut self.ground_cliff_zones,
                                self.first_zone,
                            );
                        }
                        if crusher_ground(cell_ref, top_ref) {
                            apply_block_zone(
                                cell,
                                top_cell,
                                &mut self.crusher_zones,
                                self.first_zone,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Get effective zone for given locomotor capabilities
    pub fn get_effective_zone(
        &self,
        acceptable_surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        zone: ZoneStorageType,
    ) -> ZoneStorageType {
        if zone == 0 {
            return zone;
        }

        if (acceptable_surfaces & SURFACE_AIR) != 0 {
            return 1;
        }

        if (acceptable_surfaces & SURFACE_GROUND) != 0
            && (acceptable_surfaces & SURFACE_WATER) != 0
            && (acceptable_surfaces & SURFACE_CLIFF) != 0
        {
            return 1;
        }

        if self.num_zones < 2 {
            return self.first_zone;
        }

        if zone < self.first_zone || zone >= self.first_zone + self.num_zones {
            return self.first_zone;
        }

        let mut zone_index = (zone - self.first_zone) as usize;

        if crusher && zone_index < self.crusher_zones.len() {
            let mapped = self.crusher_zones[zone_index];
            if mapped >= self.first_zone {
                zone_index = (mapped - self.first_zone) as usize;
            }
        }

        if (acceptable_surfaces & SURFACE_GROUND) != 0
            && (acceptable_surfaces & SURFACE_CLIFF) != 0
            && zone_index < self.ground_cliff_zones.len()
        {
            return self.ground_cliff_zones[zone_index];
        }

        if (acceptable_surfaces & SURFACE_GROUND) != 0
            && (acceptable_surfaces & SURFACE_WATER) != 0
            && zone_index < self.ground_water_zones.len()
        {
            return self.ground_water_zones[zone_index];
        }

        if (acceptable_surfaces & SURFACE_GROUND) != 0
            && (acceptable_surfaces & SURFACE_RUBBLE) != 0
            && zone_index < self.ground_rubble_zones.len()
        {
            return self.ground_rubble_zones[zone_index];
        }

        if (acceptable_surfaces & SURFACE_CLIFF) != 0 && (acceptable_surfaces & SURFACE_WATER) != 0
        {
            debug_assert!(false, "Cliff+water-only locomotor sets not supported yet.");
        }

        (zone_index as ZoneStorageType) + self.first_zone
    }

    /// Clear marked passable flag
    pub fn clear_marked_passable(&mut self) {
        self.marked_passable = false;
    }

    /// Check if block is passable
    pub fn is_passable(&self) -> bool {
        self.marked_passable
    }

    /// Set passable state
    pub fn set_passable(&mut self, passable: bool) {
        self.marked_passable = passable;
    }

    /// Check if block interacts with bridge
    pub fn get_interacts_with_bridge(&self) -> bool {
        self.interacts_with_bridge
    }

    /// Set bridge interaction
    pub fn set_interacts_with_bridge(&mut self, interacts: bool) {
        self.interacts_with_bridge = interacts;
    }

    /// Set cell origin for this block
    pub fn set_cell_origin(&mut self, origin: ICoord2D) {
        self.cell_origin = origin;
    }

    /// Get cell origin
    pub fn get_cell_origin(&self) -> ICoord2D {
        self.cell_origin
    }

    /// Get number of zones in this block
    pub fn get_num_zones(&self) -> u16 {
        self.num_zones
    }

    /// Get first zone in this block
    pub fn get_first_zone(&self) -> ZoneStorageType {
        self.first_zone
    }

    /// Reset zone information
    fn reset_zones(&mut self) {
        self.first_zone = 0;
        self.num_zones = 0;
        self.free_zones();
    }

    /// Allocate zone equivalency arrays
    fn allocate_zones(&mut self) {
        if (self.zones_allocated as u16) > self.num_zones && !self.ground_cliff_zones.is_empty() {
            return;
        }

        self.free_zones();

        if self.num_zones == 1 {
            return;
        }

        if self.zones_allocated == 0 {
            self.zones_allocated = 4;
        }
        while self.zones_allocated <= self.num_zones {
            self.zones_allocated = self.zones_allocated.saturating_mul(2).max(1);
        }

        let num_zones = self.zones_allocated as usize;
        self.ground_cliff_zones.resize(num_zones, 0);
        self.ground_water_zones.resize(num_zones, 0);
        self.ground_rubble_zones.resize(num_zones, 0);
        self.crusher_zones.resize(num_zones, 0);
    }

    /// Free zone equivalency arrays
    fn free_zones(&mut self) {
        self.zones_allocated = 0;
        self.ground_cliff_zones.clear();
        self.ground_water_zones.clear();
        self.ground_rubble_zones.clear();
        self.crusher_zones.clear();
    }

    /// Get cell from map at given coordinates
    fn get_cell_from_map(
        &self,
        map: &[Vec<*const PathfindCell>],
        x: i32,
        y: i32,
        bounds: &IRegion2D,
    ) -> Option<*const PathfindCell> {
        if x >= bounds.lo.x && x <= bounds.hi.x && y >= bounds.lo.y && y <= bounds.hi.y {
            let map_x = (x - bounds.lo.x) as usize;
            let map_y = (y - bounds.lo.y) as usize;

            if map_x < map.len() && map_y < map[map_x].len() {
                let cell_ptr = map[map_x][map_y];
                if !cell_ptr.is_null() {
                    Some(cell_ptr)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

fn water_ground(target: &PathfindCell, source: &PathfindCell) -> bool {
    matches!(
        (target.get_type(), source.get_type()),
        (PathfindCellType::Clear, PathfindCellType::Water)
            | (PathfindCellType::Water, PathfindCellType::Clear)
    )
}

fn ground_rubble(target: &PathfindCell, source: &PathfindCell) -> bool {
    matches!(
        (target.get_type(), source.get_type()),
        (PathfindCellType::Clear, PathfindCellType::Rubble)
            | (PathfindCellType::Rubble, PathfindCellType::Clear)
    )
}

fn ground_cliff(target: &PathfindCell, source: &PathfindCell) -> bool {
    matches!(
        (target.get_type(), source.get_type()),
        (PathfindCellType::Clear, PathfindCellType::Cliff)
            | (PathfindCellType::Cliff, PathfindCellType::Clear)
    )
}

fn crusher_ground(target: &PathfindCell, source: &PathfindCell) -> bool {
    if matches!(target.get_type(), PathfindCellType::Obstacle)
        && target.is_obstacle_fence()
        && matches!(source.get_type(), PathfindCellType::Clear)
    {
        return true;
    }
    if matches!(source.get_type(), PathfindCellType::Obstacle)
        && source.is_obstacle_fence()
        && matches!(target.get_type(), PathfindCellType::Clear)
    {
        return true;
    }
    false
}

fn resolve_block_zones(
    src_zone: ZoneStorageType,
    target_zone: ZoneStorageType,
    zone_equivalency: &mut [ZoneStorageType],
) {
    if target_zone < src_zone {
        for zone in zone_equivalency.iter_mut() {
            if *zone == src_zone {
                *zone = target_zone;
            }
        }
    } else {
        for zone in zone_equivalency.iter_mut() {
            if *zone == target_zone {
                *zone = src_zone;
            }
        }
    }
}

fn apply_block_zone(
    target: *const PathfindCell,
    source: *const PathfindCell,
    zone_equivalency: &mut [ZoneStorageType],
    first_zone: ZoneStorageType,
) {
    let src_zone = unsafe { (*source).get_zone() };
    let target_zone = unsafe { (*target).get_zone() };
    if src_zone < first_zone || target_zone < first_zone {
        return;
    }

    let src_index = (src_zone - first_zone) as usize;
    let target_index = (target_zone - first_zone) as usize;
    if src_index >= zone_equivalency.len() || target_index >= zone_equivalency.len() {
        return;
    }

    let src_zone = zone_equivalency[src_index];
    let target_zone = zone_equivalency[target_index];
    if target_zone == src_zone {
        return;
    }

    resolve_block_zones(src_zone, target_zone, zone_equivalency);
}

impl Default for ZoneBlock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::SURFACE_GROUND;

    #[test]
    fn test_zone_block_creation() {
        let block = ZoneBlock::new();
        assert_eq!(block.get_num_zones(), 0);
        assert_eq!(block.get_first_zone(), 0);
        assert!(block.is_passable());
        assert!(!block.get_interacts_with_bridge());
    }

    #[test]
    fn test_zone_block_passable() {
        let mut block = ZoneBlock::new();
        assert!(block.is_passable());

        block.set_passable(true);
        assert!(block.is_passable());

        block.clear_marked_passable();
        assert!(!block.is_passable());
    }

    #[test]
    fn test_zone_block_bridge_interaction() {
        let mut block = ZoneBlock::new();
        assert!(!block.get_interacts_with_bridge());

        block.set_interacts_with_bridge(true);
        assert!(block.get_interacts_with_bridge());
    }

    #[test]
    fn test_zone_block_origin() {
        let mut block = ZoneBlock::new();
        let origin = ICoord2D::new(100, 200);

        block.set_cell_origin(origin);
        assert_eq!(block.get_cell_origin(), origin);
    }

    #[test]
    fn test_zone_block_effective_zone() {
        let mut block = ZoneBlock::new();

        // Single zone case (defaults to first zone)
        let zone = block.get_effective_zone(SURFACE_GROUND, false, 42);
        assert_eq!(zone, 0);

        // Setup multi-zone block
        block.first_zone = 10;
        block.num_zones = 3;
        block.allocate_zones();
        let num_zones = block.zones_allocated as usize;
        for i in 0..num_zones {
            let zone = block.first_zone + i as ZoneStorageType;
            block.ground_cliff_zones[i] = zone;
            block.ground_water_zones[i] = zone;
            block.ground_rubble_zones[i] = zone;
            block.crusher_zones[i] = zone;
        }

        // Should return mapped zone
        let effective = block.get_effective_zone(SURFACE_GROUND, false, 11);
        assert!(effective >= 10 && effective < 13);
    }
}
