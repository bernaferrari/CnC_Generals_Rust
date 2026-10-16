//! Read-only radar geometry from the receiving, completed camera.
use crate::presentation_frame::{SessionView, camera::viewport::TacticalViewport};
use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RadarCamera {
    viewport: TacticalViewport,
    view: Mat4,
    projection: Mat4,
    terrain: f32,
}
impl RadarCamera {
    pub(crate) fn capture(view: &SessionView, terrain_average: f32) -> Self {
        Self {
            viewport: view.camera.viewport,
            view: view.camera.view_matrix,
            projection: view.camera.projection_matrix,
            terrain: if terrain_average.is_finite() {
                terrain_average
            } else {
                view.target.y
            },
        }
    }
    pub(crate) fn with_matrices(mut self, view: Mat4, projection: Mat4) -> Self {
        self.view = view;
        self.projection = projection;
        self
    }
    /// View.cpp:209-230 and Vector3::Find_X/Y_At_Z: absolute integer corners,
    /// clockwise UL, UR, LR, LL, projected to the receiving radar's plane.
    pub(crate) fn corners(self) -> Option<[Vec3; 4]> {
        let (width, height) = self.viewport.size();
        let (x, y) = self.viewport.origin();
        if width <= 0 || height <= 0 || !self.terrain.is_finite() {
            return None;
        }
        let right = x.checked_add(width)?;
        let bottom = y.checked_add(height)?;
        let mut corners = [Vec3::ZERO; 4];
        for (output, (x, y)) in
            corners
                .iter_mut()
                .zip([(x, y), (right, y), (right, bottom), (x, bottom)])
        {
            let (start, end) =
                self.viewport
                    .pick_ray(self.view, self.projection, x as f32, y as f32)?;
            let vertical = end.y - start.y;
            if vertical == 0. {
                return None;
            }
            // Preserve the original divide-before-multiply order.
            *output = Vec3::new(
                start.x + (self.terrain - start.y) * ((end.x - start.x) / vertical),
                self.terrain,
                start.z + (self.terrain - start.y) * ((end.z - start.z) / vertical),
            );
            if !output.is_finite() {
                return None;
            }
        }
        Some(corners)
    }
}

#[cfg(test)]
#[path = "radar_view_tests.rs"]
mod tests;
