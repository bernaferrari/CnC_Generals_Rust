//! Receiving terrain admission for W3DView::moveCameraAlongWaypointPath.
//! The 25-slot limit counts accepted waypoints, not raw graph visits.
use super::GameLogic;
use crate::presentation_frame::waypoint::Point;
use glam::Vec3;
use std::collections::HashSet;

pub(super) struct CameraChain {
    pub(super) points: Vec<Point>,
    pub(super) final_ground: f32,
}

pub(super) fn admit_camera_chain(
    world: &GameLogic,
    name: &str,
    start: Vec3,
) -> Result<Option<CameraChain>, String> {
    if !start.is_finite() {
        return Err("non-finite receiving camera path start".into());
    }
    let owner = world.world_services.terrain();
    let terrain = owner
        .read()
        .map_err(|_| "receiving camera path terrain poisoned".to_owned())?;
    // CPP ScriptActions941 scans the actual roster and takes its first exact
    // authored-name match, independent of generic AI path discovery policy.
    let mut roster = terrain.get_first_waypoint();
    let mut current = None;
    while let Some(node) = roster {
        if node.get_name().as_str() == name {
            current = Some(node);
            break;
        }
        roster = node.get_next();
    }
    if current.is_none() {
        return Ok(None);
    }

    let mut points = Vec::new();
    let mut seen = HashSet::new();
    let mut accepted = 1usize; // CPP slot1 is the receiving View position.
    let mut endpoint = Point {
        x: start.x,
        y: start.z,
        z: start.y,
    };
    while let Some(node) = current {
        if accepted >= 25 {
            break;
        }
        // Revisit is legal after progress. A ring of discarded short nodes
        // cannot finish in C++; reject that malformed input without mutating
        // the receiving camera rather than hanging or imposing a raw-hop cap.
        if !seen.insert((std::ptr::from_ref(node), accepted)) {
            return Err("camera waypoint cycle makes no admission progress".into());
        }
        let location = node.get_location();
        if !location.x.is_finite() || !location.y.is_finite() || !location.z.is_finite() {
            return Err("non-finite receiving camera waypoint".into());
        }
        let candidate = Point {
            x: location.x,
            y: location.y,
            z: location.z,
        };
        let next = node
            .get_link(0)
            .and_then(|id| terrain.get_waypoint_by_id(id));
        points.push(candidate);
        let dx = candidate.x - endpoint.x;
        let dy = candidate.y - endpoint.y;
        // Match the numeric kernel/WWMath shim: round the squared sum as
        // float before the double sqrt and final float conversion.
        let distance = f64::from(dx * dx + dy * dy).sqrt() as f32;
        if !distance.is_finite() {
            return Err("non-finite receiving camera waypoint distance".into());
        }
        if distance < gamelogic::common::MAP_XY_FACTOR {
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
    let final_ground = terrain.get_ground_height(endpoint.x, endpoint.y, None);
    if !final_ground.is_finite() {
        return Err("non-finite receiving camera endpoint ground".into());
    }
    Ok(Some(CameraChain {
        points,
        final_ground,
    }))
}

#[cfg(test)]
#[path = "path_admission_tests.rs"]
mod tests;
