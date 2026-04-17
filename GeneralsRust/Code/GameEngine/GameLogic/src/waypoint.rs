//! Waypoint system - lightweight navigation data

use crate::common::*;
use crate::terrain::Waypoint as TerrainWaypoint;

/// Waypoint identifier
pub type WaypointId = u32;

/// Waypoint structure for navigation
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub id: WaypointId,
    pub position: Coord3D,
    pub name: String,
    pub links: Vec<WaypointId>,
}

impl Waypoint {
    pub fn new(id: WaypointId, position: Coord3D, name: String) -> Self {
        Self {
            id,
            position,
            name,
            links: Vec::new(),
        }
    }

    pub fn with_links(
        id: WaypointId,
        position: Coord3D,
        name: String,
        links: Vec<WaypointId>,
    ) -> Self {
        Self {
            id,
            position,
            name,
            links,
        }
    }

    pub fn from_terrain(waypoint: &TerrainWaypoint) -> Self {
        let links: Vec<_> = (0..waypoint.get_num_links())
            .filter_map(|i| waypoint.get_link(i))
            .collect();
        Self {
            id: waypoint.get_id(),
            position: *waypoint.get_location(),
            name: waypoint.get_name().as_str().to_string(),
            links,
        }
    }

    pub fn add_link(&mut self, link: WaypointId) {
        if !self.links.contains(&link) {
            self.links.push(link);
        }
    }

    pub fn get_num_links(&self) -> usize {
        self.links.len()
    }

    pub fn get_link(&self, index: usize) -> Option<WaypointId> {
        self.links.get(index).copied()
    }
}
