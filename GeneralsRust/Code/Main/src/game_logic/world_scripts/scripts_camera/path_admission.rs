//! Receiving terrain admission for W3DView::moveCameraAlongWaypointPath.
//! The 25-slot limit counts accepted waypoints, not raw graph visits.
use super::GameLogic;
use crate::presentation_frame::waypoint::Point;
use glam::Vec3;

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
    let start = Vec3::new(start.x, start.z, start.y);
    let Some(path) = terrain
        .waypoints()
        .camera_path(name, start)
        .map_err(|err| err.to_string())?
    else {
        return Ok(None);
    };
    let points = path
        .points
        .into_iter()
        .map(|pos| Point {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        })
        .collect();
    let final_ground = terrain.get_ground_height(path.endpoint.x, path.endpoint.y, None);
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
