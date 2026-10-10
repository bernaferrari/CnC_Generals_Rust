//! Main owns bridge slot identity; geometry can be admitted before grid bounds.
use super::*;

impl PathfindingGrid {
    pub(crate) fn clear_bridge_admission(&mut self) {
        self.bridge_layers.clear();
        self.flight_bridge_order.clear();
        self.layer_occ.clear();
        self.ground_connect.fill(0);
        self.admitted_raw_terrain = None;
    }

    pub(crate) fn admit_raw_terrain(
        &mut self,
        terrain: Option<&super::super::terrain::TerrainData>,
    ) {
        self.admitted_raw_terrain = terrain.map(|t| std::sync::Arc::new(t.clone()));
    }

    pub(super) fn admitted_ground_height(&self, pos: Vec3) -> f32 {
        self.admitted_raw_terrain
            .as_ref()
            .map_or(0.0, |t| t.logic_height_at_world(pos))
    }

    /// Bounds changes rebuild cell storage, not original bridge identities.
    pub(crate) fn inherit_bridge_admission(&mut self, previous: &mut Self) {
        self.bridge_layers = std::mem::take(&mut previous.bridge_layers);
        self.flight_bridge_order = std::mem::take(&mut previous.flight_bridge_order);
        self.admitted_raw_terrain = previous.admitted_raw_terrain.take();
        for bridge in &mut self.bridge_layers {
            bridge.cells.clear();
            bridge.ground_connect_cells.clear();
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
    }

    pub(crate) fn bind_reserved_bridge_object(&mut self, layer: u8, object: u32) {
        if let Some(bridge) = self.bridge_layers.iter_mut().find(|b| b.id == layer) {
            bridge.object_id = object;
        }
    }

    /// Deleted bridges retain their slot but leave terrain-list visitation.
    pub(crate) fn deactivate_bridge_layer(&mut self, layer: u8) {
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
        for expected in 2..=14 {
            assert_eq!(grid.reserve_bridge_layer(a, b, c, d), expected);
        }
        let original = grid.admitted_bridge_states();
        assert_eq!(
            grid.reserve_bridge_layer(a, b, c, d),
            PathfindLayerEnum::Ground as u8
        );
        assert_eq!(grid.admitted_bridge_states(), original);
        assert_eq!(grid.flight_bridge_order, (2..=14).rev().collect::<Vec<_>>());
        let mut replacement = PathfindingGrid::new(400.0, 400.0, 10.0);
        replacement.inherit_bridge_admission(&mut grid);
        replacement.reclassify_ground();
        assert_eq!(replacement.admitted_bridge_states(), original);
        assert!(grid.admitted_bridge_states().is_empty());
    }
}
