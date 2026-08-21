// Terrain height, LOS, extent, and waypoint math helpers
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// Terrain logic bridge for gameplay queries (matching C++ TheTerrainLogic).
/// Nested queries must `try_read`: C++ `TheTerrainLogic` is a raw pointer
/// (`TerrainLogic.cpp:1230` loadMap / `addBridgeToLogic`). A blocking
/// `RwLock::read` deadlocks when `load_map_data` already holds the write lock
/// and `classify_bridge_cells` asks for layer height.
pub struct TheTerrainLogic;

impl TheTerrainLogic {
    pub fn get() -> Option<&'static Self> {
        static TERRAIN: OnceLock<TheTerrainLogic> = OnceLock::new();
        Some(TERRAIN.get_or_init(|| TheTerrainLogic))
    }

    pub fn is_underwater(
        &self,
        _x: Real,
        _y: Real,
        water_z: Option<&mut f32>,
        terrain_z: Option<&mut f32>,
    ) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            return guard.is_underwater(_x as f32, _y as f32, water_z, terrain_z);
        }
        if let Some(wz) = water_z {
            *wz = 0.0;
        }
        if let Some(tz) = terrain_z {
            *tz = 0.0;
        }
        false
    }

    pub fn is_cliff_cell(&self, _x: Real, _y: Real) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            return guard.is_cliff_cell(_x as f32, _y as f32);
        }
        false
    }

    /// Get terrain height at coordinates (bridges to TerrainLogic when available).
    pub fn get_height_at(&self, x: Real, y: Real) -> Real {
        self.get_ground_height(x, y, None)
    }

    /// Get layer height at coordinates (bridges to TerrainLogic when available).
    pub fn get_layer_height(&self, x: Real, y: Real, layer: PathfindLayerEnum) -> Real {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            return 0.0;
        };
        let terrain_layer = match layer {
            PathfindLayerEnum::Air => crate::path::PathfindLayerEnum::Top,
            PathfindLayerEnum::Tunnel | PathfindLayerEnum::Water | PathfindLayerEnum::Last => {
                crate::path::PathfindLayerEnum::Ground
            }
            other => crate::path::PathfindLayerEnum::from_u32(other as u32),
        };
        guard.get_layer_height(x, y, terrain_layer, None, true)
    }

    /// Get highest layer for destination (bridges to TerrainLogic when available).
    pub fn get_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            return PathfindLayerEnum::Ground;
        };
        match guard.get_highest_layer_for_destination(pos) {
            crate::path::PathfindLayerEnum::Last => PathfindLayerEnum::Last,
            other => PathfindLayerEnum::from_u32(other as u32),
        }
    }

    /// Get destination layer (bridges to TerrainLogic::get_layer_for_destination).
    pub fn get_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            return PathfindLayerEnum::Ground;
        };
        match guard.get_layer_for_destination(pos) {
            crate::path::PathfindLayerEnum::Last => PathfindLayerEnum::Last,
            other => PathfindLayerEnum::from_u32(other as u32),
        }
    }

    /// Bridge interaction helper for locomotor/path layers.
    pub fn object_interacts_with_bridge_layer(
        &self,
        obj: &Object,
        layer: PathfindLayerEnum,
        consider_bridge_health: bool,
    ) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            return false;
        };
        let terrain_layer = match layer {
            PathfindLayerEnum::Air => crate::path::PathfindLayerEnum::Top,
            PathfindLayerEnum::Tunnel | PathfindLayerEnum::Water | PathfindLayerEnum::Last => {
                crate::path::PathfindLayerEnum::Ground
            }
            other => crate::path::PathfindLayerEnum::from_u32(other as u32),
        };
        guard.object_interacts_with_bridge_layer(obj, terrain_layer, consider_bridge_health)
    }

    /// Get ground height with optional normal output (mirrors TerrainLogic::getGroundHeight).
    pub fn get_ground_height(&self, x: Real, y: Real, mut normal: Option<&mut Coord3D>) -> Real {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            if let Some(n) = normal.as_deref_mut() {
                *n = Coord3D::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        };
        guard.get_ground_height(x, y, normal.as_deref_mut())
    }

    pub fn is_clear_line_of_sight(&self, from: &Coord3D, to: &Coord3D) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            return false;
        };
        guard.is_clear_line_of_sight(from, to)
    }

    /// Get map extent including border. Uses a large fallback region when no map data is wired.
    pub fn get_extent_including_border(&self) -> crate::common::Region3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            let extent = guard.get_extent_including_border();
            if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
                return extent;
            }
        }
        let lo = crate::common::Coord3D::new(0.0, 0.0, 0.0);
        let hi = crate::common::Coord3D::new(50000.0, 50000.0, 0.0);
        crate::common::Region3D::new(lo, hi)
    }

    /// C++ TerrainLogic::getExtent() — full map including border region.
    pub fn get_extent(&self) -> crate::common::Region3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            let extent = guard.get_extent();
            if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
                return extent;
            }
        }
        self.get_extent_including_border()
    }

    /// Get maximum pathfind extent (playable area excluding border).
    pub fn get_maximum_pathfind_extent(&self) -> crate::common::Region3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            let extent = guard.get_maximum_pathfind_extent();
            if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
                return extent;
            }
        }
        self.get_extent_including_border()
    }

    /// Find closest edge point to a location (fallback uses extent bounds).
    pub fn find_closest_edge_point(&self, location: &Coord3D) -> Coord3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            return guard.find_closest_edge_point(location);
        }

        let extent = self.get_maximum_pathfind_extent();
        let distances = [
            (location.y - extent.lo.y).abs(), // top
            (location.x - extent.hi.x).abs(), // right
            (location.y - extent.hi.y).abs(), // bottom
            (location.x - extent.lo.x).abs(), // left
        ];
        let mut best_index = 0usize;
        let mut best_distance = distances[0];
        for (idx, distance) in distances.iter().copied().enumerate().skip(1) {
            if distance < best_distance {
                best_distance = distance;
                best_index = idx;
            }
        }

        let mut ret = *location;
        match best_index {
            0 => ret.y = extent.lo.y,
            1 => ret.x = extent.hi.x,
            2 => ret.y = extent.hi.y,
            _ => ret.x = extent.lo.x,
        }
        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Find farthest edge point from a location (fallback uses extent bounds).
    pub fn find_farthest_edge_point(&self, location: &Coord3D) -> Coord3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.try_read() {
            return guard.find_farthest_edge_point(location);
        }

        let extent = self.get_maximum_pathfind_extent();
        let mid_x = (extent.hi.x - extent.lo.x) * 0.5;
        let mid_y = (extent.hi.y - extent.lo.y) * 0.5;

        let mut ret = *location;
        if location.x < mid_x {
            ret.x = extent.hi.x;
        } else {
            ret.x = extent.lo.x;
        }

        if location.y < mid_y {
            ret.y = extent.hi.y;
        } else {
            ret.y = extent.lo.y;
        }

        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Find the closest waypoint that matches a path label.
    pub fn get_closest_waypoint_on_path(&self, pos: &Coord3D, label: &str) -> Option<Coord3D> {
        let terrain = crate::terrain::get_terrain_logic();
        let guard = terrain.try_read().ok()?;
        guard
            .get_closest_waypoint_on_path(pos, label)
            .map(|way| *way.get_location())
    }

    /// Build a linked waypoint chain starting from waypoint name.
    pub fn get_waypoint_chain_by_name(&self, start_name: &str, max_points: usize) -> Vec<Coord3D> {
        let mut out = Vec::new();
        if start_name.trim().is_empty() {
            return out;
        }

        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.try_read() else {
            return out;
        };

        let name = AsciiString::from(start_name);
        let Some(start) = guard.get_waypoint_by_name(&name) else {
            return out;
        };

        let mut visited = HashSet::new();
        let mut current_id = Some(start.get_id());
        let limit = max_points.max(1);

        while let Some(id) = current_id {
            if !visited.insert(id) || out.len() >= limit {
                break;
            }

            let Some(waypoint) = guard.get_waypoint_by_id(id) else {
                break;
            };
            out.push(*waypoint.get_location());

            current_id = (0..waypoint.get_num_links())
                .filter_map(|idx| waypoint.get_link(idx))
                .find(|candidate| !visited.contains(candidate));
        }

        out
    }

    /// C++ `TheTerrainLogic->getWaypointByID` location.
    pub fn get_waypoint_location(&self, id: UnsignedInt) -> Option<Coord3D> {
        let terrain = crate::terrain::get_terrain_logic();
        let guard = terrain.try_read().ok()?;
        guard.get_waypoint_by_id(id).map(|way| *way.get_location())
    }

    /// Advance to a random outgoing waypoint link (C++ PUC scripted path).
    pub fn random_outgoing_waypoint_link(
        &self,
        id: UnsignedInt,
    ) -> Option<(UnsignedInt, Coord3D)> {
        let terrain = crate::terrain::get_terrain_logic();
        let guard = terrain.try_read().ok()?;
        let way = guard.get_waypoint_by_id(id)?;
        let link_count = way.get_num_links();
        if link_count == 0 {
            return None;
        }
        let which = game_logic_random_value(0, (link_count as u32) - 1) as usize;
        let next_id = way.get_link(which)?;
        let next = guard.get_waypoint_by_id(next_id)?;
        Some((next.get_id(), *next.get_location()))
    }
}
