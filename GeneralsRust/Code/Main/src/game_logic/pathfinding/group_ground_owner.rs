//! Receiving-session admission for C++ `Pathfinder::findGroundPath`.
use super::*;

impl PathfindingSystem {
    pub(crate) fn find_group_ground_path(
        &mut self,
        from: Vec3,
        to: Vec3,
        objects: HostObjectRead<'_>,
        ground_source: &PathfindingGround<'_>,
    ) -> Option<Vec<Vec3>> {
        // A group query has no individual ignored obstacle or seeker. Admit
        // actual receiving occupancy rather than inherit another request's mask.
        self.grid.update_dynamic_obstacles(objects, ground_source);
        self.dynamic_obstacle_frame = self.logic_frame;
        self.sync_crate_astar();
        let jumps = self.hierarchical_bridge_jumps();
        let finder = &mut self.crate_astar.as_mut()?.finder;
        finder.apply_hierarchical_zone_prune(
            GridCoord::new(
                self.grid.world_to_grid(from).x,
                self.grid.world_to_grid(from).y,
            ),
            GridCoord::new(self.grid.world_to_grid(to).x, self.grid.world_to_grid(to).y),
            SURFACE_GROUND,
            false,
            &jumps,
        );
        let mut cleanup_cells = 0;
        let path = self.grid.find_group_ground_path_owned(
            from,
            to,
            objects,
            |cell| finder.clip_is_zone_passable(cell.x, cell.y),
            &mut cleanup_cells,
            ground_source,
        );
        self.note_cells_allocated(cleanup_cells);
        path
    }
}
