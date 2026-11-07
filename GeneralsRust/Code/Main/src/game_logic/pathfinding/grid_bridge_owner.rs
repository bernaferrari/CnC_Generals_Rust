//! Main owns bridge slot identity; geometry can be admitted before grid bounds.
use super::*;

impl PathfindingGrid {
    pub(crate) fn clear_bridge_admission(&mut self) {
        self.bridge_layers.clear();
        self.flight_bridge_order.clear();
        self.layer_occ.clear();
        self.ground_connect.fill(0);
    }


    /// Bounds changes rebuild cell storage, not original bridge identities.
    pub(crate) fn inherit_bridge_admission(&mut self, previous: &mut Self) {
        self.bridge_layers = std::mem::take(&mut previous.bridge_layers);
        self.flight_bridge_order = std::mem::take(&mut previous.flight_bridge_order);
        self.wall_pieces = std::mem::take(&mut previous.wall_pieces);
        self.wall_height = previous.wall_height;
        self.allocate_and_classify_wall_layer();
        for bridge in &mut self.bridge_layers {
            bridge.cells.clear();
            bridge.ground_connect_cells.clear();
        }
    }

    /// Carry event-owned goal cells into a replacement grid. Actor goal cells
    /// continue to use the same grid coordinates until their next updateGoal.
    pub(crate) fn inherit_goal_ownership(&mut self, previous: &Self) {
        for y in 0..previous.height.min(self.height) {
            for x in 0..previous.width.min(self.width) {
                let pos = GridPos::new(x, y);
                let (Some(old), Some(new)) = (previous.bit_index(pos), self.bit_index(pos)) else {
                    continue;
                };
                self.occ_goal_unit[new] = previous.occ_goal_unit[old];
                self.occ_goal_mask[new] = previous.occ_goal_mask[old];
                self.occ_goal_aircraft[new] = previous.occ_goal_aircraft[old];
            }
        }
        for (&layer, old) in &previous.layer_occ {
            let current = self.layer_occ.entry(layer).or_default();
            for (&(x, y), &id) in &old.occ_goal_unit {
                if x >= 0 && y >= 0 && x < self.width && y < self.height {
                    current.occ_goal_unit.insert((x, y), id);
                    if let Some(&mask) = old.occ_goal_mask.get(&(x, y)) {
                        current.occ_goal_mask.insert((x, y), mask);
                    }
                }
            }
        }
    }

    pub(crate) fn reclassify_ground(&mut self) {
        let bridges = std::mem::take(&mut self.bridge_layers);
        let order = std::mem::take(&mut self.flight_bridge_order);
        self.clear_static_blocks();
        self.bridge_layers = bridges;
        self.flight_bridge_order = order;
        for bridge in &mut self.bridge_layers {
            bridge.cells.clear();
        }
        self.classify_wall_layer();
    }

    pub(crate) fn bind_reserved_bridge_object(&mut self, layer: u8, object: u32) {
        if layer == PathfindLayerEnum::Ground as u8 {
            return;
        }
        if let Some(bridge) = self.bridge_layers.iter_mut().find(|b| b.id == layer) {
            bridge.object_id = object;
        }
    }

    /// Deleted bridges retain their slot but leave terrain-list visitation.
    pub(crate) fn deactivate_bridge_layer(&mut self, layer: u8) {
        if layer == PathfindLayerEnum::Ground as u8 {
            return;
        }
        self.flight_bridge_order.retain(|id| *id != layer);
        self.bind_reserved_bridge_object(layer, 0);
    }

    pub(crate) fn bridge_layer_for_object(&self, object: u32) -> Option<u8> {
        self.bridge_layers
            .iter()
            .find(|b| b.object_id == object && object != 0)
            .map(|b| b.id)
    }

    pub(crate) fn admitted_bridge_states(&self) -> Vec<(u8, u32, bool, bool)> {
        self.bridge_layers
            .iter()
            .map(|b| (b.id, b.object_id, b.destroyed, b.cells.is_empty()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservation_exhaustion_never_overwrites_and_equal_geometry_keeps_source_identity() {
        let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
        let a = Vec3::new(20.0, 21.0, 20.0);
        let b = Vec3::new(40.0, 21.0, 20.0);
        let c = Vec3::new(20.0, 21.0, 80.0);
        let d = Vec3::new(40.0, 21.0, 80.0);
        for expected in 2..=15 {
            assert_eq!(grid.reserve_bridge_layer(a, b, c, d), expected);
        }
        let original = grid.admitted_bridge_states();
        assert_eq!(
            grid.reserve_bridge_layer(a, b, c, d),
            PathfindLayerEnum::Ground as u8
        );
        assert_eq!(grid.admitted_bridge_states(), original);
        assert_eq!(grid.flight_bridge_order, (2..=15).rev().collect::<Vec<_>>());
        let mut replacement = PathfindingGrid::new(400.0, 400.0, 10.0);
        replacement.inherit_bridge_admission(&mut grid);
        replacement.reclassify_ground();
        assert_eq!(replacement.admitted_bridge_states(), original);
        assert!(grid.admitted_bridge_states().is_empty());
    }

    #[test]
    fn occupied_wall_reserves_last_layer_independently_per_grid() {
        let mut wall_world = PathfindingGrid::new(200.0, 200.0, 10.0);
        let mut bridge_world = PathfindingGrid::new(200.0, 200.0, 10.0);
        wall_world.add_wall_piece(1, Vec3::new(100.0, 0.0, 100.0), 0.0, 40.0, 40.0);
        wall_world.allocate_and_classify_wall_layer();
        let a = Vec3::new(20.0, 21.0, 20.0);
        let b = Vec3::new(40.0, 21.0, 20.0);
        let c = Vec3::new(20.0, 21.0, 80.0);
        let d = Vec3::new(40.0, 21.0, 80.0);
        for expected in 2..=14 {
            assert_eq!(wall_world.reserve_bridge_layer(a, b, c, d), expected);
            assert_eq!(bridge_world.reserve_bridge_layer(a, b, c, d), expected);
        }
        assert_eq!(wall_world.reserve_bridge_layer(a, b, c, d), 1);
        assert_eq!(bridge_world.reserve_bridge_layer(a, b, c, d), 15);
        assert_eq!(wall_world.layer_cell_type(15, GridPos::new(10, 10)), Some(PathfindCellType::Clear));
    }

    #[test]
    fn last_bridge_slot_uses_wall_map_allocation_and_keeps_bridge_identity() {
        let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
        let a = Vec3::new(20.0, 21.0, 20.0);
        let b = Vec3::new(40.0, 21.0, 20.0);
        let c = Vec3::new(20.0, 21.0, 80.0);
        let d = Vec3::new(40.0, 21.0, 80.0);
        for expected in 2..=15 {
            assert_eq!(grid.reserve_bridge_layer(a, b, c, d), expected);
        }
        grid.bind_reserved_bridge_object(15, 88);
        grid.stamp_reserved_bridge_layer(15, false, &PathfindingGround::Flat(0.0));
        assert_eq!(grid.layer_cell_type(15, GridPos::new(3, 5)), None);
        assert_eq!(grid.bridge_layer_for_object(88), Some(15));
        // TerrainLogic visits the coincident newest bridge first, but W3D
        // interprets layer 15 as WALL for height and Pathfinder has no cells.
        let point = Vec3::new(30.0, 30.0, 50.0);
        grid.set_wall_height(12.0);
        assert_eq!(grid.highest_layer_at_or_below(point, 0.0, false), 15);
        assert_eq!(grid.layer_height_clipped(point, 15, 0.0), 0.0);
        assert_eq!(grid.layer_height_unclipped(point, 15, 0.0), 12.0);
        assert_eq!(grid.reserve_bridge_layer(a, b, c, d), 1);
        grid.add_wall_piece(7, Vec3::new(100.0, 0.0, 100.0), 0.0, 40.0, 40.0);
        assert_eq!(grid.layer_cell_type(15, GridPos::new(10, 10)), None);
        grid.allocate_and_classify_wall_layer();
        assert_eq!(grid.layer_cell_type(15, GridPos::new(10, 10)), Some(PathfindCellType::Clear));
        assert_eq!(grid.bridge_layer_for_object(88), Some(15));
        grid.stamp_reserved_bridge_layer(15, true, &PathfindingGround::Flat(0.0));
        assert_eq!(grid.layer_cell_type(15, GridPos::new(10, 10)), Some(PathfindCellType::BridgeImpassable));
        grid.stamp_reserved_bridge_layer(15, false, &PathfindingGround::Flat(0.0));
        assert_eq!(grid.layer_cell_type(15, GridPos::new(10, 10)), None);
        grid.reclassify_ground();
        assert_eq!(grid.layer_cell_type(15, GridPos::new(10, 10)), Some(PathfindCellType::Clear));
        grid.remove_wall_piece(7);
        assert_eq!(grid.layer_cell_type(15, GridPos::new(10, 10)), None);
        assert_eq!(grid.bridge_layer_for_object(88), Some(15));
        assert_eq!(grid.reserve_bridge_layer(a, b, c, d), 1);
    }
}
