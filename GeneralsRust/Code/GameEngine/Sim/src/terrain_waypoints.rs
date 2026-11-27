//! Authored waypoint roster, links and traversal for the receiving simulation.

use glam::Vec3;
use std::collections::HashSet;

pub type WaypointID = u32;

/// C++ `AIUpdate.cpp`: `enum {WAYPOINT_PATH_LIMIT=1024}`. Caps `link[0]` walks
/// so ShellMapMD 1-link rings (Car_Path) cannot grow an unbounded Vec.
pub const WAYPOINT_PATH_LIMIT: usize = 1024;
/// C++ `W3DView.h`: `enum {MAX_WAYPOINTS=25}` for camera path collection.
pub const CAMERA_WAYPOINT_PATH_LIMIT: usize = 25;

/// C++ WaveGuideUpdate::startMoving WaveGuide1 bind result.
#[derive(Debug, Clone, Copy)]
pub enum WaveGuide1Bind {
    Follow { first: Vec3, last: Vec3, angle: f32 },
    MissingWaypoint,
    InvalidPath,
}

/// Waypoint helper class for waypoint info in terrain logic
#[derive(Debug)]
pub struct Waypoint<Name> {
    /// Unique integer identifier
    id: WaypointID,
    /// Name
    name: Name,
    /// Location
    location: Vec3,
    /// Next waypoint in linked list
    next: Option<Box<Waypoint<Name>>>,
    /// Directed graph of waypoints (up to 8 links)
    links: Vec<WaypointID>,
    /// Path labels for waypoint classification
    path_label1: Name,
    path_label2: Name,
    path_label3: Name,
    /// Whether path is bidirectional
    bi_directional: bool,
}

impl<Name> Waypoint<Name> {
    const MAX_LINKS: usize = 8;

    pub fn new(
        id: WaypointID,
        name: Name,
        location: &Vec3,
        label1: Name,
        label2: Name,
        label3: Name,
        bi_directional: bool,
    ) -> Self {
        Self {
            id,
            name,
            location: *location,
            next: None,
            links: Vec::with_capacity(Self::MAX_LINKS),
            path_label1: label1,
            path_label2: label2,
            path_label3: label3,
            bi_directional,
        }
    }

    /// Get the next waypoint in the linked list
    pub fn get_next(&self) -> Option<&Self> {
        self.next.as_ref().map(|w| w.as_ref())
    }

    /// Get number of links from this waypoint
    pub fn get_num_links(&self) -> usize {
        self.links.len()
    }

    /// Get the nth directed link
    pub fn get_link(&self, index: usize) -> Option<WaypointID> {
        if index < self.links.len() {
            Some(self.links[index])
        } else {
            None
        }
    }

    /// Get the waypoint's name
    pub fn get_name(&self) -> &Name {
        &self.name
    }

    /// Get the waypoint ID
    pub fn get_id(&self) -> WaypointID {
        self.id
    }

    /// Get the waypoint location
    pub fn get_location(&self) -> &Vec3 {
        &self.location
    }

    /// Get path labels
    pub fn get_path_label1(&self) -> &Name {
        &self.path_label1
    }

    pub fn get_path_label2(&self) -> &Name {
        &self.path_label2
    }

    pub fn get_path_label3(&self) -> &Name {
        &self.path_label3
    }

    /// Get bidirectional flag
    pub fn get_bi_directional(&self) -> bool {
        self.bi_directional
    }

    /// Add a link to another waypoint
    pub fn add_link(&mut self, waypoint: WaypointID) {
        if self.links.len() < Self::MAX_LINKS {
            self.links.push(waypoint);
        }
    }

    pub fn has_link(&self, waypoint: WaypointID) -> bool {
        self.links.iter().any(|id| *id == waypoint)
    }

    /// Set location Z coordinate
    pub fn set_location_z(&mut self, z: f32) {
        self.location.z = z;
    }

    pub fn matches_path_label(&self, label: &str) -> bool
    where
        Name: AsRef<str>,
    {
        self.path_label1.as_ref().eq_ignore_ascii_case(label)
            || self.path_label2.as_ref().eq_ignore_ascii_case(label)
            || self.path_label3.as_ref().eq_ignore_ascii_case(label)
    }
}

/// The first and terminal identities used to publish route completion.
#[derive(Debug)]
pub struct WaypointIdentity {
    pub id: WaypointID,
    pub labels: [String; 3],
}

#[derive(Debug)]
pub struct WaypointRoute {
    pub points: Vec<Vec3>,
    first: WaypointIdentity,
    last: Option<WaypointIdentity>,
}

impl WaypointRoute {
    pub fn completion(&self, exact: bool) -> Option<&WaypointIdentity> {
        if exact {
            Some(&self.first)
        } else {
            self.last.as_ref()
        }
    }
}

/// Original XYZ points delivered to the receiving camera motion kernel.
pub struct CameraWaypointPath {
    pub points: Vec<Vec3>,
    pub endpoint: Vec3,
}

#[derive(Debug, thiserror::Error)]
pub enum CameraWaypointError {
    #[error("non-finite receiving camera path start")]
    NonfiniteStart,
    #[error("camera waypoint cycle makes no admission progress")]
    Cycle,
    #[error("non-finite receiving camera waypoint")]
    NonfinitePoint,
    #[error("non-finite receiving camera waypoint distance")]
    NonfiniteDistance,
}

/// One roster per loaded terrain. The source pushes onto the front, so a
/// duplicate name or ID resolves to the newest admitted record.
#[derive(Debug)]
pub struct WaypointGraph<Name> {
    head: Option<Box<Waypoint<Name>>>,
}

impl<Name> Default for WaypointGraph<Name> {
    fn default() -> Self {
        Self { head: None }
    }
}

impl<Name: AsRef<str>> WaypointGraph<Name> {
    pub fn first(&self) -> Option<&Waypoint<Name>> {
        self.head.as_deref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Waypoint<Name>> {
        std::iter::successors(self.first(), |node| node.get_next())
    }

    pub fn clear(&mut self) {
        self.head = None;
    }

    pub fn prepend(&mut self, mut waypoint: Waypoint<Name>) {
        waypoint.next = self.head.take();
        self.head = Some(Box::new(waypoint));
    }

    pub fn by_name(&self, name: &str) -> Option<&Waypoint<Name>> {
        self.iter().find(|node| node.get_name().as_ref() == name)
    }

    pub fn by_id(&self, id: WaypointID) -> Option<&Waypoint<Name>> {
        self.iter().find(|node| node.get_id() == id)
    }

    pub fn by_id_mut(&mut self, id: WaypointID) -> Option<&mut Waypoint<Name>> {
        let mut current = self.head.as_deref_mut();
        while let Some(waypoint) = current {
            if waypoint.get_id() == id {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref_mut();
        }
        None
    }

    pub fn add_link(&mut self, id1: WaypointID, id2: WaypointID) {
        if id1 == id2 {
            return;
        }

        let should_link_back = {
            let Some(way1) = self.by_id_mut(id1) else {
                return;
            };
            if !way1.has_link(id2) {
                way1.add_link(id2);
            }
            way1.get_bi_directional()
        };

        if should_link_back {
            if let Some(way2) = self.by_id_mut(id2) {
                if !way2.has_link(id1) {
                    way2.add_link(id1);
                }
            }
        }
    }

    pub fn closest_on_path(&self, pos: &Vec3, label: &str) -> Option<&Waypoint<Name>> {
        if label.is_empty() {
            return None;
        }

        let mut best: Option<&Waypoint<Name>> = None;
        let mut best_dist_sqr = 0.0;

        for waypoint in self.iter() {
            if waypoint.matches_path_label(label) {
                let dx = waypoint.location.x - pos.x;
                let dy = waypoint.location.y - pos.y;
                let dist_sqr = dx * dx + dy * dy;
                if best.is_none() || dist_sqr < best_dist_sqr {
                    best_dist_sqr = dist_sqr;
                    best = Some(waypoint);
                }
            }
        }

        best
    }

    /// Resample at map finalization. Look up each ID on writeback to preserve
    /// the receiving roster's first-match rule for older malformed maps.
    pub fn resample_ground(&mut self, mut height_at: impl FnMut(f32, f32) -> f32) {
        let heights: Vec<_> = self
            .iter()
            .map(|waypoint| {
                let loc = waypoint.get_location();
                (waypoint.get_id(), height_at(loc.x, loc.y))
            })
            .collect();
        for (id, z) in heights {
            if let Some(waypoint) = self.by_id_mut(id) {
                waypoint.set_location_z(z);
            }
        }
    }

    /// AI paths stop when a link identity repeats or reaches the hop limit.
    pub fn walk_link0_chain<'a>(
        &'a self,
        start: &'a Waypoint<Name>,
        max_hops: usize,
    ) -> Vec<&'a Waypoint<Name>> {
        let mut out = Vec::new();
        let mut visited = HashSet::new();
        let mut current = Some(start);
        let limit = max_hops.max(1);
        while let Some(node) = current {
            if out.len() >= limit || !visited.insert(node.get_id()) {
                break;
            }
            out.push(node);
            current = node.get_link(0).and_then(|id| self.by_id(id));
        }
        out
    }

    /// A bounded AI route has a terminal identity only if it reached a null link.
    pub fn route_from(
        &self,
        label: &str,
        position: &Vec3,
        mut project: impl FnMut(Vec3) -> Vec3,
    ) -> Option<WaypointRoute> {
        let start = self.closest_on_path(position, label)?;
        let chain = self.walk_link0_chain(start, WAYPOINT_PATH_LIMIT);
        let identity = |node: &Waypoint<Name>| WaypointIdentity {
            id: node.id,
            labels: [
                node.path_label1.as_ref().to_owned(),
                node.path_label2.as_ref().to_owned(),
                node.path_label3.as_ref().to_owned(),
            ],
        };
        let first = identity(chain.first()?);
        let last = chain
            .last()
            .filter(|node| node.get_link(0).is_none())
            .map(|node| identity(node));
        let points = chain
            .into_iter()
            .map(|node| project(node.location))
            .collect();
        Some(WaypointRoute {
            points,
            first,
            last,
        })
    }

    /// Camera slots count accepted segments. A circular path can revisit a
    /// node after advancing a slot, while an all-short cycle is invalid.
    pub fn camera_path(
        &self,
        name: &str,
        start: Vec3,
    ) -> Result<Option<CameraWaypointPath>, CameraWaypointError> {
        if !start.is_finite() {
            return Err(CameraWaypointError::NonfiniteStart);
        }
        let Some(first) = self.by_name(name) else {
            return Ok(None);
        };
        let mut current = Some(first);
        let mut points = Vec::new();
        let mut seen = HashSet::new();
        let mut accepted = 1usize; // CPP slot1 is the receiving View position.
        let mut endpoint = start;
        while let Some(node) = current {
            if accepted >= CAMERA_WAYPOINT_PATH_LIMIT {
                break;
            }
            // Revisit is legal after progress. A ring of discarded short nodes
            // cannot finish in C++; reject that malformed input without mutating
            // the receiving camera rather than hanging or imposing a raw-hop cap.
            if !seen.insert((std::ptr::from_ref(node), accepted)) {
                return Err(CameraWaypointError::Cycle);
            }
            let location = node.get_location();
            if !location.is_finite() {
                return Err(CameraWaypointError::NonfinitePoint);
            }
            let candidate = *location;
            let next = node.get_link(0).and_then(|id| self.by_id(id));
            points.push(candidate);
            let dx = candidate.x - endpoint.x;
            let dy = candidate.y - endpoint.y;
            // Match the numeric kernel/WWMath shim: round the squared sum as
            // float before the double sqrt and final float conversion.
            let distance = f64::from(dx * dx + dy * dy).sqrt() as f32;
            if !distance.is_finite() {
                return Err(CameraWaypointError::NonfiniteDistance);
            }
            if distance < crate::terrain::MAP_XY_FACTOR {
                // CPP terminal short segment pushes the final point back into
                // the previous slot; it still determines final terrain height.
                if next.is_none() {
                    endpoint = candidate;
                }
            } else {
                accepted += 1;
                endpoint = candidate;
            }
            current = next;
        }
        Ok(Some(CameraWaypointPath { points, endpoint }))
    }

    /// The special wave route must have a single first link and a finite end.
    pub fn bind_wave_guide1(&self) -> WaveGuide1Bind {
        let Some(waypoint) = self.by_name("WaveGuide1") else {
            return WaveGuide1Bind::MissingWaypoint;
        };
        let mut last = *waypoint.get_location();
        let mut verify = Some(waypoint);
        let mut visited = HashSet::new();
        while let Some(node) = verify {
            if node.get_num_links() > 1 || !visited.insert(std::ptr::from_ref(node)) {
                return WaveGuide1Bind::InvalidPath;
            }
            last = *node.get_location();
            verify = node.get_link(0).and_then(|id| self.by_id(id));
        }
        let Some(next_id) = waypoint.get_link(0) else {
            return WaveGuide1Bind::InvalidPath;
        };
        let Some(next) = self.by_id(next_id) else {
            return WaveGuide1Bind::InvalidPath;
        };
        let angle = (next.get_location().y - waypoint.get_location().y)
            .atan2(next.get_location().x - waypoint.get_location().x);
        WaveGuide1Bind::Follow {
            first: *waypoint.get_location(),
            last,
            angle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveguide_rejects_cycle_and_keeps_other_receiving_roster() {
        let fixture = |cycle| {
            let mut graph = WaypointGraph::default();
            for (id, name, x) in [(1, "WaveGuide1", 0.0), (2, "End", 10.0)] {
                graph.prepend(Waypoint::new(
                    id,
                    name,
                    &Vec3::new(x, 4.0, 7.0),
                    "",
                    "",
                    "",
                    false,
                ));
            }
            graph.add_link(1, 2);
            if cycle {
                graph.add_link(2, 1);
            }
            graph
        };
        let cyclic = fixture(true);
        let finite = fixture(false);
        assert!(matches!(
            cyclic.bind_wave_guide1(),
            WaveGuide1Bind::InvalidPath
        ));
        assert!(
            matches!(finite.bind_wave_guide1(), WaveGuide1Bind::Follow { first, last, angle }
            if first == Vec3::new(0.0, 4.0, 7.0) && last == Vec3::new(10.0, 4.0, 7.0) && angle == 0.0)
        );
    }
}
