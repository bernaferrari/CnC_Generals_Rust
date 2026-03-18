//! PathfindZoneManager implementation matching C++ PathfindZoneManager class
#![allow(missing_docs)]
//!
//! This class manages the zones in the map. A zone is an area in the map that
//! is one contiguous type of terrain (clear, cliff, water, building). If
//! a unit is in a zone, and wants to move to another location, the destination
//! zone has to be the same, or it can't get there.
//! There are equivalency tables for meta-zones. For example, an amphibious craft can
//! travel through water and clear cells.

use super::{
    PathfindCell, PathfindCellType, PathfindLayer, PathfindLayerEnum, ZoneBlock, ZONE_BLOCK_SIZE,
};
use crate::common::{ICoord2D, IRegion2D};
use crate::locomotor::SURFACE_AIR;
use crate::path::{
    LocomotorSurfaceTypeMask, ZoneStorageType, SURFACE_CLIFF, SURFACE_GROUND, SURFACE_RUBBLE,
    SURFACE_WATER,
};
/// Zone manager constants
pub const INITIAL_ZONES: usize = 256;
pub const UNINITIALIZED_ZONE: ZoneStorageType = 0;
const MAX_ZONES: usize = 24000;
const ZONE_UPDATE_FREQUENCY: u32 = 300;

/// PathfindZoneManager structure matching C++ PathfindZoneManager class
#[derive(Debug)]
pub struct PathfindZoneManager {
    // Zone block management
    block_of_zone_blocks: Vec<ZoneBlock>, // Zone blocks - Info for hierarchical pathfinding at a "blocky" level
    zone_blocks: Vec<Vec<*mut ZoneBlock>>, // Zone blocks as a matrix - contains matrix indexing into the map
    zone_block_extent: ICoord2D, // Zone block extents. Not the same scale as the pathfind extents

    // Zone tracking
    max_zone: ZoneStorageType,          // Max zone used
    next_frame_to_calculate_zones: u32, // When should I recalculate, next?

    // Zone equivalency tables
    zones_allocated: u16,
    ground_cliff_zones: Vec<ZoneStorageType>,
    ground_water_zones: Vec<ZoneStorageType>,
    ground_rubble_zones: Vec<ZoneStorageType>,
    terrain_zones: Vec<ZoneStorageType>,
    crusher_zones: Vec<ZoneStorageType>,
    hierarchical_zones: Vec<ZoneStorageType>,
}

impl PathfindZoneManager {
    /// Create a new PathfindZoneManager
    pub fn new() -> Self {
        Self {
            block_of_zone_blocks: Vec::new(),
            zone_blocks: Vec::new(),
            zone_block_extent: ICoord2D::new(0, 0),
            max_zone: 0,
            next_frame_to_calculate_zones: 0,
            zones_allocated: 0,
            ground_cliff_zones: Vec::new(),
            ground_water_zones: Vec::new(),
            ground_rubble_zones: Vec::new(),
            terrain_zones: Vec::new(),
            crusher_zones: Vec::new(),
            hierarchical_zones: Vec::new(),
        }
    }

    /// Reset zone manager
    pub fn reset(&mut self) {
        self.max_zone = 0;
        self.next_frame_to_calculate_zones = 0;
        self.free_zones();
        self.free_blocks();
    }

    /// Check if zones need to be recalculated
    pub fn need_to_calculate_zones(&self, current_frame: u32) -> bool {
        self.next_frame_to_calculate_zones <= current_frame
    }

    /// Mark zones as dirty and needing recalculation
    pub fn mark_zones_dirty(&mut self, current_frame: u32, insert: bool) {
        if current_frame < 2 {
            self.next_frame_to_calculate_zones = 2;
            return;
        }

        let _ = insert;
        let desired = current_frame.saturating_add(ZONE_UPDATE_FREQUENCY);
        // Match C++ MIN behavior (note: next_frame_to_calculate_zones may be 0).
        self.next_frame_to_calculate_zones = self.next_frame_to_calculate_zones.min(desired);
    }

    /// Update zones for structure modification
    pub fn update_zones_for_modify(
        &mut self,
        map: &[Vec<*mut PathfindCell>],
        layers: &[PathfindLayer],
        structure_bounds: &IRegion2D,
        global_bounds: &IRegion2D,
    ) {
        let _ = layers;

        let mut bounds = *structure_bounds;
        bounds.hi.x = bounds.hi.x.saturating_add(1);
        bounds.hi.y = bounds.hi.y.saturating_add(1);
        if bounds.hi.x > global_bounds.hi.x {
            bounds.hi.x = global_bounds.hi.x;
        }
        if bounds.hi.y > global_bounds.hi.y {
            bounds.hi.y = global_bounds.hi.y;
        }

        for block_x in 0..self.zone_block_extent.x {
            for block_y in 0..self.zone_block_extent.y {
                let mut block_bounds = Self::make_block_bounds(global_bounds, block_x, block_y);
                if block_bounds.hi.x > bounds.hi.x {
                    block_bounds.hi.x = bounds.hi.x;
                }
                if block_bounds.hi.y > bounds.hi.y {
                    block_bounds.hi.y = bounds.hi.y;
                }
                if block_bounds.lo.x < bounds.lo.x {
                    block_bounds.lo.x = bounds.lo.x;
                }
                if block_bounds.lo.y < bounds.lo.y {
                    block_bounds.lo.y = bounds.lo.y;
                }
                if block_bounds.lo.x > block_bounds.hi.x || block_bounds.lo.y > block_bounds.hi.y {
                    continue;
                }

                if let Some(block) = self.get_zone_block_mut(block_x, block_y) {
                    block.set_interacts_with_bridge(false);
                }

                for y in block_bounds.lo.y..=block_bounds.hi.y {
                    for x in block_bounds.lo.x..=block_bounds.hi.x {
                        let Some(cell_ptr) = self.get_cell_from_map(map, x, y, global_bounds)
                        else {
                            continue;
                        };
                        let cell_ref = unsafe { &mut *cell_ptr };
                        if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                            continue;
                        }

                        if x > block_bounds.lo.x {
                            if let Some(left_ptr) =
                                self.get_cell_from_map(map, x - 1, y, global_bounds)
                            {
                                let left_ref = unsafe { &*left_ptr };
                                if types_match(cell_ref, left_ref) {
                                    cell_ref.set_zone(left_ref.get_zone());
                                    if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                                        continue;
                                    }
                                }
                            }
                        }

                        if y > block_bounds.lo.y {
                            if let Some(top_ptr) =
                                self.get_cell_from_map(map, x, y - 1, global_bounds)
                            {
                                let top_ref = unsafe { &*top_ptr };
                                if types_match(cell_ref, top_ref) {
                                    cell_ref.set_zone(top_ref.get_zone());
                                    if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                                        continue;
                                    }
                                }
                            }

                            if x < block_bounds.hi.x {
                                let diag_ptr =
                                    self.get_cell_from_map(map, x + 1, y - 1, global_bounds);
                                let right_ptr =
                                    self.get_cell_from_map(map, x + 1, y, global_bounds);
                                if let (Some(diag_ptr), Some(right_ptr)) = (diag_ptr, right_ptr) {
                                    let diag_ref = unsafe { &*diag_ptr };
                                    let right_ref = unsafe { &*right_ptr };
                                    if types_match(cell_ref, diag_ref)
                                        && types_match(cell_ref, right_ref)
                                    {
                                        cell_ref.set_zone(diag_ref.get_zone());
                                        if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                for y in (block_bounds.lo.y..=block_bounds.hi.y).rev() {
                    for x in (block_bounds.lo.x..=block_bounds.hi.x).rev() {
                        let Some(cell_ptr) = self.get_cell_from_map(map, x, y, global_bounds)
                        else {
                            continue;
                        };
                        let cell_ref = unsafe { &mut *cell_ptr };
                        if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                            continue;
                        }

                        if x < block_bounds.hi.x {
                            if let Some(right_ptr) =
                                self.get_cell_from_map(map, x + 1, y, global_bounds)
                            {
                                let right_ref = unsafe { &*right_ptr };
                                if types_match(cell_ref, right_ref) {
                                    cell_ref.set_zone(right_ref.get_zone());
                                    if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                                        continue;
                                    }
                                }
                            }
                        }

                        if y < block_bounds.hi.y {
                            if let Some(bottom_ptr) =
                                self.get_cell_from_map(map, x, y + 1, global_bounds)
                            {
                                let bottom_ref = unsafe { &*bottom_ptr };
                                if types_match(cell_ref, bottom_ref) {
                                    cell_ref.set_zone(bottom_ref.get_zone());
                                    if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                                        continue;
                                    }
                                }
                            }

                            if x < block_bounds.hi.x {
                                let diag_ptr =
                                    self.get_cell_from_map(map, x + 1, y + 1, global_bounds);
                                let right_ptr =
                                    self.get_cell_from_map(map, x + 1, y, global_bounds);
                                if let (Some(diag_ptr), Some(right_ptr)) = (diag_ptr, right_ptr) {
                                    let diag_ref = unsafe { &*diag_ptr };
                                    let right_ref = unsafe { &*right_ptr };
                                    if types_match(cell_ref, diag_ref)
                                        && types_match(cell_ref, right_ref)
                                    {
                                        cell_ref.set_zone(diag_ref.get_zone());
                                        if cell_ref.get_zone() != UNINITIALIZED_ZONE {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Calculate zones for the entire map
    pub fn calculate_zones(
        &mut self,
        map: &[Vec<*mut PathfindCell>],
        layers: &mut [PathfindLayer],
        bounds: &IRegion2D,
    ) {
        self.allocate_blocks(bounds);
        self.max_zone = 1;

        let mut zone_equivalency: Vec<ZoneStorageType> =
            (0..MAX_ZONES).map(|zone| zone as ZoneStorageType).collect();

        for layer in layers.iter_mut() {
            layer.set_zone(0);
        }

        let blocks_x = (bounds.hi.x - bounds.lo.x + 1 + ZONE_BLOCK_SIZE - 1) / ZONE_BLOCK_SIZE;
        let blocks_y = (bounds.hi.y - bounds.lo.y + 1 + ZONE_BLOCK_SIZE - 1) / ZONE_BLOCK_SIZE;

        for block_x in 0..blocks_x {
            for block_y in 0..blocks_y {
                if let Some(block) = self.get_zone_block_mut(block_x, block_y) {
                    block.set_interacts_with_bridge(false);
                }

                let block_bounds = Self::make_block_bounds(bounds, block_x, block_y);
                for y in block_bounds.lo.y..=block_bounds.hi.y {
                    for x in block_bounds.lo.x..=block_bounds.hi.x {
                        let Some(cell_ptr) = self.get_cell_from_map(map, x, y, bounds) else {
                            continue;
                        };
                        let cell_ref = unsafe { &mut *cell_ptr };
                        cell_ref.set_zone(0);

                        if x > block_bounds.lo.x {
                            if let Some(left_ptr) = self.get_cell_from_map(map, x - 1, y, bounds) {
                                let left_ref = unsafe { &*left_ptr };
                                if cell_ref.get_type() == left_ref.get_type() {
                                    apply_zone(
                                        cell_ptr,
                                        left_ptr,
                                        &mut zone_equivalency,
                                        self.max_zone,
                                    );
                                }
                            }
                        }

                        if y > block_bounds.lo.y {
                            if let Some(top_ptr) = self.get_cell_from_map(map, x, y - 1, bounds) {
                                let top_ref = unsafe { &*top_ptr };
                                if cell_ref.get_type() == top_ref.get_type() {
                                    apply_zone(
                                        cell_ptr,
                                        top_ptr,
                                        &mut zone_equivalency,
                                        self.max_zone,
                                    );
                                }
                            }
                        }

                        if cell_ref.get_zone() == 0 {
                            cell_ref.set_zone(self.max_zone);
                            self.max_zone = self.max_zone.saturating_add(1);
                        }

                        if (cell_ref.get_connect_layer() as u8) > (PathfindLayerEnum::Ground as u8)
                        {
                            if let Some(block) = self.get_zone_block_mut(block_x, block_y) {
                                block.set_interacts_with_bridge(true);
                            }
                        }
                    }
                }
            }
        }

        let total_zones = self.max_zone as usize;
        let mut collapsed_zones = vec![0; MAX_ZONES];
        collapsed_zones[0] = 0;
        self.max_zone = 1;

        for i in 1..total_zones {
            let zone = zone_equivalency
                .get(i)
                .copied()
                .unwrap_or(i as ZoneStorageType) as usize;
            if zone == i {
                collapsed_zones[i] = self.max_zone;
                self.max_zone = self.max_zone.saturating_add(1);
            } else {
                collapsed_zones[i] = collapsed_zones.get(zone).copied().unwrap_or(0);
            }
        }

        for y in bounds.lo.y..=bounds.hi.y {
            for x in bounds.lo.x..=bounds.hi.x {
                if let Some(cell_ptr) = self.get_cell_from_map(map, x, y, bounds) {
                    let cell_ref = unsafe { &mut *cell_ptr };
                    let zone = cell_ref.get_zone() as usize;
                    let collapsed = collapsed_zones.get(zone).copied().unwrap_or(0);
                    cell_ref.set_zone(collapsed);
                }
            }
        }

        for layer in layers.iter_mut() {
            let mut zone = collapsed_zones
                .get(layer.get_zone() as usize)
                .copied()
                .unwrap_or(0);
            if zone == 0 {
                zone = self.max_zone;
                self.max_zone = self.max_zone.saturating_add(1);
            }
            layer.set_zone(zone as i32);
            layer.apply_zone();

            if !layer.is_unused() && !layer.is_destroyed() {
                let start = layer.get_start_cell_index();
                self.set_bridge(start.x, start.y, true);
                let end = layer.get_end_cell_index();
                self.set_bridge(end.x, end.y, true);
            }
        }

        self.allocate_zones();

        self.calculate_zone_blocks(map, layers, bounds);
        self.build_equivalency_tables(map, layers, bounds);
        self.next_frame_to_calculate_zones = u32::MAX;
    }

    /// Calculate zone blocks for hierarchical pathfinding
    fn calculate_zone_blocks(
        &mut self,
        map: &[Vec<*mut PathfindCell>],
        layers: &[PathfindLayer],
        bounds: &IRegion2D,
    ) {
        let blocks_x = ((bounds.hi.x - bounds.lo.x) / ZONE_BLOCK_SIZE) + 1;
        let blocks_y = ((bounds.hi.y - bounds.lo.y) / ZONE_BLOCK_SIZE) + 1;

        let const_map = Self::convert_map_to_const(map);

        for block_y in 0..blocks_y {
            for block_x in 0..blocks_x {
                if let Some(block) = self.get_zone_block_mut(block_x, block_y) {
                    let origin = ICoord2D::new(
                        bounds.lo.x + block_x * ZONE_BLOCK_SIZE,
                        bounds.lo.y + block_y * ZONE_BLOCK_SIZE,
                    );
                    block.set_cell_origin(origin);

                    let block_bounds = Self::make_block_bounds(bounds, block_x, block_y);
                    block.block_calculate_zones(&const_map, layers, &block_bounds);
                }
            }
        }
    }

    fn make_block_bounds(bounds: &IRegion2D, block_x: i32, block_y: i32) -> IRegion2D {
        let lo = ICoord2D::new(
            bounds.lo.x + block_x * ZONE_BLOCK_SIZE,
            bounds.lo.y + block_y * ZONE_BLOCK_SIZE,
        );
        let hi = ICoord2D::new(
            (lo.x + ZONE_BLOCK_SIZE - 1).min(bounds.hi.x),
            (lo.y + ZONE_BLOCK_SIZE - 1).min(bounds.hi.y),
        );
        IRegion2D { lo, hi }
    }

    /// Build equivalency tables for different locomotor types
    fn build_equivalency_tables(
        &mut self,
        map: &[Vec<*mut PathfindCell>],
        layers: &[PathfindLayer],
        bounds: &IRegion2D,
    ) {
        self.allocate_zones();

        let zones_len = self.zones_allocated as usize;
        for i in 0..zones_len {
            let zone = i as ZoneStorageType;
            self.ground_cliff_zones[i] = zone;
            self.ground_water_zones[i] = zone;
            self.ground_rubble_zones[i] = zone;
            self.terrain_zones[i] = zone;
            self.crusher_zones[i] = zone;
            self.hierarchical_zones[i] = zone;
        }

        let max_zone = self.max_zone;

        for y in bounds.lo.y..=bounds.hi.y {
            for x in bounds.lo.x..=bounds.hi.x {
                let Some(cell_ptr) = self.get_cell_from_map(map, x, y, bounds) else {
                    continue;
                };
                let cell_ref = unsafe { &*cell_ptr };

                if (cell_ref.get_connect_layer() as u8) > (PathfindLayerEnum::Ground as u8)
                    && cell_ref.get_type() == PathfindCellType::Clear
                {
                    let layer_idx = cell_ref.get_connect_layer() as usize;
                    if let Some(layer) = layers.get(layer_idx) {
                        let layer_zone = layer.get_zone();
                        if layer_zone > 0 {
                            resolve_zones(
                                cell_ref.get_zone(),
                                layer_zone as ZoneStorageType,
                                &mut self.hierarchical_zones,
                                max_zone as usize,
                            );
                        }
                    }
                }

                if x > bounds.lo.x {
                    let Some(left_ptr) = self.get_cell_from_map(map, x - 1, y, bounds) else {
                        continue;
                    };
                    let left_ref = unsafe { &*left_ptr };
                    if cell_ref.get_zone() != left_ref.get_zone() {
                        if cell_ref.get_type() == left_ref.get_type() {
                            apply_zone(cell_ptr, left_ptr, &mut self.hierarchical_zones, max_zone);
                        } else {
                            let mut not_terrain_or_crusher = true;
                            if terrain_match(cell_ref, left_ref) {
                                apply_zone(cell_ptr, left_ptr, &mut self.terrain_zones, max_zone);
                                not_terrain_or_crusher = false;
                            }
                            if crusher_ground(cell_ref, left_ref) {
                                apply_zone(cell_ptr, left_ptr, &mut self.crusher_zones, max_zone);
                                not_terrain_or_crusher = false;
                            }
                            if not_terrain_or_crusher {
                                if water_ground(cell_ref, left_ref) {
                                    apply_zone(
                                        cell_ptr,
                                        left_ptr,
                                        &mut self.ground_water_zones,
                                        max_zone,
                                    );
                                } else if ground_rubble(cell_ref, left_ref) {
                                    apply_zone(
                                        cell_ptr,
                                        left_ptr,
                                        &mut self.ground_rubble_zones,
                                        max_zone,
                                    );
                                } else if ground_cliff(cell_ref, left_ref) {
                                    apply_zone(
                                        cell_ptr,
                                        left_ptr,
                                        &mut self.ground_cliff_zones,
                                        max_zone,
                                    );
                                }
                            }
                        }
                    }
                }

                if y > bounds.lo.y {
                    let Some(top_ptr) = self.get_cell_from_map(map, x, y - 1, bounds) else {
                        continue;
                    };
                    let top_ref = unsafe { &*top_ptr };
                    if cell_ref.get_zone() != top_ref.get_zone() {
                        if cell_ref.get_type() == top_ref.get_type() {
                            apply_zone(cell_ptr, top_ptr, &mut self.hierarchical_zones, max_zone);
                        } else {
                            let mut not_terrain_or_crusher = true;
                            if terrain_match(cell_ref, top_ref) {
                                apply_zone(cell_ptr, top_ptr, &mut self.terrain_zones, max_zone);
                                not_terrain_or_crusher = false;
                            }
                            if crusher_ground(cell_ref, top_ref) {
                                apply_zone(cell_ptr, top_ptr, &mut self.crusher_zones, max_zone);
                                not_terrain_or_crusher = false;
                            }
                            if not_terrain_or_crusher {
                                if water_ground(cell_ref, top_ref) {
                                    apply_zone(
                                        cell_ptr,
                                        top_ptr,
                                        &mut self.ground_water_zones,
                                        max_zone,
                                    );
                                } else if ground_rubble(cell_ref, top_ref) {
                                    apply_zone(
                                        cell_ptr,
                                        top_ptr,
                                        &mut self.ground_rubble_zones,
                                        max_zone,
                                    );
                                } else if ground_cliff(cell_ref, top_ref) {
                                    apply_zone(
                                        cell_ptr,
                                        top_ptr,
                                        &mut self.ground_cliff_zones,
                                        max_zone,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        for i in 1..max_zone {
            let zone = self.hierarchical_zones[i as usize];
            if (zone as usize) < self.hierarchical_zones.len() {
                self.hierarchical_zones[i as usize] = self.hierarchical_zones[zone as usize];
            }
        }

        let max_zone_size = max_zone as usize;
        flatten_zones(
            &mut self.ground_cliff_zones,
            &self.hierarchical_zones,
            max_zone_size,
        );
        flatten_zones(
            &mut self.ground_water_zones,
            &self.hierarchical_zones,
            max_zone_size,
        );
        flatten_zones(
            &mut self.ground_rubble_zones,
            &self.hierarchical_zones,
            max_zone_size,
        );
        flatten_zones(
            &mut self.terrain_zones,
            &self.hierarchical_zones,
            max_zone_size,
        );
        flatten_zones(
            &mut self.crusher_zones,
            &self.hierarchical_zones,
            max_zone_size,
        );
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
        if zone > self.max_zone {
            debug_assert!(
                zone <= self.max_zone,
                "Invalid zone {} > max {}",
                zone,
                self.max_zone
            );
            return UNINITIALIZED_ZONE;
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

        let mut zone = zone;

        if crusher {
            let index = zone as usize;
            if index < self.crusher_zones.len() {
                zone = self.crusher_zones[index];
            }
        }

        let zone_index = zone as usize;

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

        if zone_index < self.hierarchical_zones.len() {
            self.hierarchical_zones[zone_index]
        } else {
            zone
        }
    }

    /// Get effective terrain zone (basic terrain compatibility)
    pub fn get_effective_terrain_zone(&self, zone: ZoneStorageType) -> ZoneStorageType {
        if zone == 0 || zone > self.max_zone {
            return zone;
        }
        let zone_index = zone as usize;
        if zone_index < self.terrain_zones.len() && zone_index < self.hierarchical_zones.len() {
            let terrain_zone = self.terrain_zones[zone_index] as usize;
            if terrain_zone < self.hierarchical_zones.len() {
                return self.hierarchical_zones[terrain_zone];
            }
        }
        zone
    }

    /// Get next available zone number
    pub fn get_next_zone(&mut self) -> ZoneStorageType {
        self.max_zone += 1;
        self.max_zone
    }

    /// Get zone block extent
    pub fn get_extent(&self) -> ICoord2D {
        self.zone_block_extent
    }

    /// Get zone for hierarchical pathfinding at block level
    pub fn get_block_zone(
        &self,
        acceptable_surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        cell_x: i32,
        cell_y: i32,
        map: &[Vec<*const PathfindCell>],
    ) -> ZoneStorageType {
        let block_x = cell_x / ZONE_BLOCK_SIZE;
        let block_y = cell_y / ZONE_BLOCK_SIZE;

        let Some(block) = self.get_zone_block(block_x, block_y) else {
            return 0;
        };

        if cell_x < 0 || cell_y < 0 {
            return 0;
        }
        let cell_x = cell_x as usize;
        let cell_y = cell_y as usize;
        let Some(cell_ptr) = map.get(cell_x).and_then(|col| col.get(cell_y)).copied() else {
            return 0;
        };
        if cell_ptr.is_null() {
            return 0;
        }

        let zone = unsafe { (*cell_ptr).get_zone() };
        let effective = block.get_effective_zone(acceptable_surfaces, crusher, zone);
        if effective >= self.max_zone {
            debug_assert!(
                effective < self.max_zone,
                "Invalid block zone {} >= max {}",
                effective,
                self.max_zone
            );
            UNINITIALIZED_ZONE
        } else {
            effective
        }
    }

    /// Allocate zone blocks
    pub fn allocate_blocks(&mut self, global_bounds: &IRegion2D) {
        let blocks_x = ((global_bounds.hi.x - global_bounds.lo.x) / ZONE_BLOCK_SIZE) + 1;
        let blocks_y = ((global_bounds.hi.y - global_bounds.lo.y) / ZONE_BLOCK_SIZE) + 1;

        self.zone_block_extent.x = blocks_x;
        self.zone_block_extent.y = blocks_y;

        // Allocate block storage
        let total_blocks = (blocks_x * blocks_y) as usize;
        self.block_of_zone_blocks.clear();
        self.block_of_zone_blocks
            .resize_with(total_blocks, ZoneBlock::default);

        // Allocate index matrix
        self.zone_blocks.clear();
        self.zone_blocks.resize(blocks_x as usize, Vec::new());

        for x in 0..blocks_x {
            self.zone_blocks[x as usize].resize(blocks_y as usize, std::ptr::null_mut());

            for y in 0..blocks_y {
                let index = (y * blocks_x + x) as usize;
                self.zone_blocks[x as usize][y as usize] =
                    &mut self.block_of_zone_blocks[index] as *mut ZoneBlock;
            }
        }
    }

    /// Clear passable flags on all blocks
    pub fn clear_passable_flags(&mut self) {
        for block in &mut self.block_of_zone_blocks {
            block.clear_marked_passable();
        }
    }

    /// Check if a cell position is passable
    pub fn is_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        let block_x = cell_x / ZONE_BLOCK_SIZE;
        let block_y = cell_y / ZONE_BLOCK_SIZE;

        if let Some(block) = self.get_zone_block(block_x, block_y) {
            block.is_passable()
        } else {
            false
        }
    }

    /// Check if a cell position is passable with bounds checking
    pub fn clip_is_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        let block_x = cell_x / ZONE_BLOCK_SIZE;
        let block_y = cell_y / ZONE_BLOCK_SIZE;

        if block_x >= 0
            && block_x < self.zone_block_extent.x
            && block_y >= 0
            && block_y < self.zone_block_extent.y
        {
            self.is_passable(cell_x, cell_y)
        } else {
            false
        }
    }

    /// Set passable state for a cell
    pub fn set_passable(&mut self, cell_x: i32, cell_y: i32, passable: bool) {
        let block_x = cell_x / ZONE_BLOCK_SIZE;
        let block_y = cell_y / ZONE_BLOCK_SIZE;

        if let Some(block) = self.get_zone_block_mut(block_x, block_y) {
            block.set_passable(passable);
        }
    }

    /// Set all blocks as passable
    pub fn set_all_passable(&mut self) {
        for block in &mut self.block_of_zone_blocks {
            block.set_passable(true);
        }
    }

    /// Set bridge interaction for a cell
    pub fn set_bridge(&mut self, cell_x: i32, cell_y: i32, bridge: bool) {
        let block_x = cell_x / ZONE_BLOCK_SIZE;
        let block_y = cell_y / ZONE_BLOCK_SIZE;

        if let Some(block) = self.get_zone_block_mut(block_x, block_y) {
            block.set_interacts_with_bridge(bridge);
        }
    }

    /// Check if cell interacts with bridge
    pub fn interacts_with_bridge(&self, cell_x: i32, cell_y: i32) -> bool {
        let block_x = cell_x / ZONE_BLOCK_SIZE;
        let block_y = cell_y / ZONE_BLOCK_SIZE;

        if let Some(block) = self.get_zone_block(block_x, block_y) {
            block.get_interacts_with_bridge()
        } else {
            false
        }
    }

    /// Get zone block at given coordinates
    fn get_zone_block(&self, block_x: i32, block_y: i32) -> Option<&ZoneBlock> {
        if block_x >= 0
            && block_x < self.zone_block_extent.x
            && block_y >= 0
            && block_y < self.zone_block_extent.y
        {
            let index = (block_y * self.zone_block_extent.x + block_x) as usize;
            self.block_of_zone_blocks.get(index)
        } else {
            None
        }
    }

    /// Get mutable zone block at given coordinates
    fn get_zone_block_mut(&mut self, block_x: i32, block_y: i32) -> Option<&mut ZoneBlock> {
        if block_x >= 0
            && block_x < self.zone_block_extent.x
            && block_y >= 0
            && block_y < self.zone_block_extent.y
        {
            let index = (block_y * self.zone_block_extent.x + block_x) as usize;
            self.block_of_zone_blocks.get_mut(index)
        } else {
            None
        }
    }

    /// Helper to convert mutable map to const map
    fn convert_map_to_const(map: &[Vec<*mut PathfindCell>]) -> Vec<Vec<*const PathfindCell>> {
        map.iter()
            .map(|row| {
                row.iter()
                    .map(|&cell_ptr| cell_ptr as *const PathfindCell)
                    .collect()
            })
            .collect()
    }

    /// Get cell from mutable map
    fn get_cell_from_map(
        &self,
        map: &[Vec<*mut PathfindCell>],
        x: i32,
        y: i32,
        bounds: &IRegion2D,
    ) -> Option<*mut PathfindCell> {
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

    /// Get cell from const map
    fn get_cell_from_const_map(
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

    /// Allocate zone equivalency arrays
    fn allocate_zones(&mut self) {
        if (self.zones_allocated as usize) > (self.max_zone as usize)
            && !self.ground_cliff_zones.is_empty()
        {
            return;
        }

        self.free_zones();

        if self.zones_allocated == 0 {
            self.zones_allocated = INITIAL_ZONES as u16;
        }

        while (self.zones_allocated as usize) <= (self.max_zone as usize) {
            self.zones_allocated = self.zones_allocated.saturating_mul(2).max(1);
        }

        let num_zones = self.zones_allocated as usize;
        self.ground_cliff_zones.resize(num_zones, 0);
        self.ground_water_zones.resize(num_zones, 0);
        self.ground_rubble_zones.resize(num_zones, 0);
        self.terrain_zones.resize(num_zones, 0);
        self.crusher_zones.resize(num_zones, 0);
        self.hierarchical_zones.resize(num_zones, 0);
    }

    /// Free zone equivalency arrays
    fn free_zones(&mut self) {
        self.zones_allocated = 0;
        self.ground_cliff_zones.clear();
        self.ground_water_zones.clear();
        self.ground_rubble_zones.clear();
        self.terrain_zones.clear();
        self.crusher_zones.clear();
        self.hierarchical_zones.clear();
    }

    /// Free zone blocks
    fn free_blocks(&mut self) {
        self.block_of_zone_blocks.clear();
        self.zone_blocks.clear();
        self.zone_block_extent = ICoord2D::new(0, 0);
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

fn terrain_match(target: &PathfindCell, source: &PathfindCell) -> bool {
    let mut target_type = target.get_type();
    let mut source_type = source.get_type();
    if matches!(target_type, PathfindCellType::Obstacle) {
        target_type = PathfindCellType::Clear;
    }
    if matches!(source_type, PathfindCellType::Obstacle) {
        source_type = PathfindCellType::Clear;
    }
    target_type == source_type
}

fn types_match(target: &PathfindCell, source: &PathfindCell) -> bool {
    target.get_type() == source.get_type()
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

fn resolve_zones(
    src_zone: ZoneStorageType,
    target_zone: ZoneStorageType,
    zone_equivalency: &mut [ZoneStorageType],
    size_of_zones: usize,
) {
    if src_zone == 0 || target_zone == 0 {
        return;
    }
    let size = size_of_zones.min(zone_equivalency.len());
    let src_index = src_zone as usize;
    let target_index = target_zone as usize;
    if src_index >= size || target_index >= size {
        return;
    }
    let src_zone = zone_equivalency.get(src_index).copied().unwrap_or(src_zone);
    let target_zone = zone_equivalency
        .get(target_index)
        .copied()
        .unwrap_or(target_zone);

    let final_zone = if target_zone < src_zone {
        zone_equivalency
            .get(target_zone as usize)
            .copied()
            .unwrap_or(target_zone)
    } else {
        zone_equivalency
            .get(src_zone as usize)
            .copied()
            .unwrap_or(src_zone)
    };

    for ze in zone_equivalency.iter_mut().take(size) {
        if *ze == target_zone || *ze == src_zone {
            *ze = final_zone;
        }
    }
}

fn flatten_zones(
    zone_array: &mut [ZoneStorageType],
    zone_hierarchical: &[ZoneStorageType],
    size_of_zones: usize,
) {
    let size = size_of_zones
        .min(zone_array.len())
        .min(zone_hierarchical.len());

    for i in 0..size {
        let zone1 = zone_array[i] as usize;
        if zone1 >= size {
            continue;
        }
        let zone2 = zone_hierarchical[zone1] as usize;
        if zone2 >= size {
            continue;
        }
        let zone1b = zone_array[zone2] as usize;
        if zone1b >= size {
            continue;
        }
        let zone2b = zone_hierarchical[zone1b] as usize;
        if zone2b < size {
            zone_array[i] = zone2b as ZoneStorageType;
        }
    }

    for i in 0..size {
        let zone1 = zone_array[i];
        let zone2 = zone_hierarchical[i];
        if zone1 != zone2 {
            resolve_zones(zone1, zone2, zone_array, size);
        }
    }
}

fn apply_zone(
    target: *mut PathfindCell,
    source: *const PathfindCell,
    zone_equivalency: &mut [ZoneStorageType],
    max_zone: ZoneStorageType,
) {
    let src_zone = unsafe { (*source).get_zone() };
    if src_zone == 0 || src_zone > max_zone {
        return;
    }
    let src_zone = zone_equivalency
        .get(src_zone as usize)
        .copied()
        .unwrap_or(src_zone);
    let target_zone = unsafe { (*target).get_zone() };
    if target_zone == 0 {
        unsafe {
            (*target).set_zone(src_zone);
        }
        return;
    }

    if target_zone > max_zone {
        return;
    }

    let target_zone = zone_equivalency
        .get(target_zone as usize)
        .copied()
        .unwrap_or(target_zone);
    if target_zone == src_zone {
        return;
    }
    resolve_zones(src_zone, target_zone, zone_equivalency, max_zone as usize);
}

impl Default for PathfindZoneManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::SURFACE_GROUND;

    #[test]
    fn test_zone_manager_creation() {
        let manager = PathfindZoneManager::new();
        assert_eq!(manager.max_zone, 0);
        assert_eq!(manager.get_extent().x, 0);
        assert_eq!(manager.get_extent().y, 0);
    }

    #[test]
    fn test_zone_manager_next_zone() {
        let mut manager = PathfindZoneManager::new();
        assert_eq!(manager.get_next_zone(), 1);
        assert_eq!(manager.get_next_zone(), 2);
        assert_eq!(manager.max_zone, 2);
    }

    #[test]
    fn test_zone_manager_effective_zone() {
        let mut manager = PathfindZoneManager::new();
        manager.max_zone = 5;
        manager.allocate_zones();

        let zones_len = manager.zones_allocated as usize;
        for i in 0..zones_len {
            let zone = i as ZoneStorageType;
            manager.ground_cliff_zones[i] = zone;
            manager.ground_water_zones[i] = zone;
            manager.ground_rubble_zones[i] = zone;
            manager.terrain_zones[i] = zone;
            manager.crusher_zones[i] = zone;
            manager.hierarchical_zones[i] = zone;
        }

        // Test identity mapping (initialized for the test)
        assert_eq!(manager.get_effective_zone(SURFACE_GROUND, false, 3), 3);
        assert_eq!(manager.get_effective_terrain_zone(2), 2);
    }

    #[test]
    fn test_zone_manager_block_allocation() {
        let mut manager = PathfindZoneManager::new();
        let bounds = IRegion2D {
            lo: ICoord2D::new(0, 0),
            hi: ICoord2D::new(50, 50),
        };

        manager.allocate_blocks(&bounds);
        let extent = manager.get_extent();
        assert!(extent.x > 0);
        assert!(extent.y > 0);
    }

    #[test]
    fn test_zone_manager_passable_flags() {
        let mut manager = PathfindZoneManager::new();
        let bounds = IRegion2D {
            lo: ICoord2D::new(0, 0),
            hi: ICoord2D::new(30, 30),
        };

        manager.allocate_blocks(&bounds);

        // C++ ZoneBlock initializes marked passable to TRUE.
        assert!(manager.is_passable(15, 15));
        manager.set_passable(15, 15, true);
        assert!(manager.is_passable(15, 15));

        manager.clear_passable_flags();
        assert!(!manager.is_passable(15, 15));

        manager.set_all_passable();
        assert!(manager.is_passable(15, 15));
    }

    #[test]
    fn test_zone_manager_bridge_interaction() {
        let mut manager = PathfindZoneManager::new();
        let bounds = IRegion2D {
            lo: ICoord2D::new(0, 0),
            hi: ICoord2D::new(30, 30),
        };

        manager.allocate_blocks(&bounds);

        assert!(!manager.interacts_with_bridge(15, 15));
        manager.set_bridge(15, 15, true);
        assert!(manager.interacts_with_bridge(15, 15));
    }

    #[test]
    fn test_zone_manager_need_calculation() {
        let mut manager = PathfindZoneManager::new();

        assert!(manager.need_to_calculate_zones(100)); // Always true initially

        manager.mark_zones_dirty(1, true);
        assert!(!manager.need_to_calculate_zones(1)); // Not yet time
        assert!(manager.need_to_calculate_zones(2)); // Frame 2 triggers

        // Simulate post-calc state where next_frame is max (C++ sets 0xffffffff)
        manager.next_frame_to_calculate_zones = u32::MAX;
        manager.mark_zones_dirty(100, false);
        assert!(!manager.need_to_calculate_zones(399));
        assert!(manager.need_to_calculate_zones(400));
    }
}
