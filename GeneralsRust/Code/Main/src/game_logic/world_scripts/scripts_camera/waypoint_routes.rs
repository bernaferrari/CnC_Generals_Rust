//! Borrow the receiving graph once, carrying identities with its actual walk.
use super::*;

use generals_sim::terrain::WaypointRoute;

impl GameLogic {
    pub(in crate::game_logic) fn host_script_waypoint_route_from(
        &self,
        path_label: &str,
        from: Vec3,
    ) -> Option<WaypointRoute> {
        let owner = self.world_services.terrain();
        let terrain = owner.read().expect("receiving waypoint graph");
        let pos = Vec3::new(from.x, from.z, from.y);
        terrain
            .waypoints()
            .route_from(path_label, &pos, |location| {
                self.project_waypoint_to_host(location, &terrain)
            })
    }
    /// Receiving ground and bridge sampling remains a world operation.
    pub(in crate::game_logic) fn project_waypoint_to_host(
        &self,
        loc: Vec3,
        terrain: &gamelogic::terrain::TerrainLogic,
    ) -> Vec3 {
        let mut position = Vec3::new(loc.x, loc.z, loc.y);
        if let Some(height) = self.terrain_height_at_with_terrain_logic(
            Vec3::new(position.x, 0.0, position.z),
            Some(terrain),
        ) {
            position.y = height;
        }
        position
    }

    pub(in crate::game_logic) fn begin_host_waypoint_ai_observations(&mut self, ids: &[ObjectId]) {
        for id in ids {
            if let Some(object) = self
                .objects
                .get_mut(id)
                .filter(|object| object.has_ai_update_interface())
            {
                object.begin_waypoint_completion_observation();
            }
        }
    }
}
