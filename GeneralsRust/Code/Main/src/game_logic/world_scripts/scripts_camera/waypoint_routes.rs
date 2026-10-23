//! Borrow the receiving graph once, carrying identities with its actual walk.
use super::*;

pub(in crate::game_logic) struct HostWaypointIdentity {
    pub(in crate::game_logic) id: u32,
    pub(in crate::game_logic) labels: [String; 3],
}
pub(in crate::game_logic) struct HostWaypointRoute {
    pub(in crate::game_logic) points: Vec<Vec3>,
    first: HostWaypointIdentity,
    last: Option<HostWaypointIdentity>,
}
impl HostWaypointRoute {
    pub(in crate::game_logic) fn completion(&self, exact: bool) -> Option<&HostWaypointIdentity> {
        if exact {
            Some(&self.first)
        } else {
            self.last.as_ref()
        }
    }
}
fn identity(waypoint: &gamelogic::terrain::Waypoint) -> HostWaypointIdentity {
    HostWaypointIdentity {
        id: waypoint.get_id(),
        labels: [
            waypoint.get_path_label1().to_string(),
            waypoint.get_path_label2().to_string(),
            waypoint.get_path_label3().to_string(),
        ],
    }
}
impl GameLogic {
    pub(in crate::game_logic) fn host_script_waypoint_route_from(
        &self,
        path_label: &str,
        from: Vec3,
    ) -> Option<HostWaypointRoute> {
        let owner = self.world_services.terrain();
        let terrain = owner.read().expect("receiving waypoint graph");
        let pos = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let first = terrain.get_closest_waypoint_on_path(&pos, path_label)?;
        let chain = terrain.walk_link0_chain(first, gamelogic::terrain::WAYPOINT_PATH_LIMIT);
        let first = identity(chain.first()?);
        // Reaching the existing hop/visited limit does not manufacture a
        // terminal waypoint. The residual may still stop its finite path, but
        // only a real null link can publish ordinary Follow completion.
        let last = chain
            .last()
            .filter(|waypoint| waypoint.get_link(0).is_none())
            .map(|waypoint| identity(waypoint));
        let points = chain
            .into_iter()
            .map(|waypoint| {
                let loc = *waypoint.get_location();
                let mut position = Vec3::new(loc.x, loc.z, loc.y);
                if let Some(height) = self.terrain_height_at_with_terrain_logic(
                    Vec3::new(position.x, 0., position.z),
                    Some(&terrain),
                ) {
                    position.y = height;
                }
                position
            })
            .collect();
        Some(HostWaypointRoute {
            points,
            first,
            last,
        })
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
