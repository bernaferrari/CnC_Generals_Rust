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
    pub(crate) fn project_world(self, position: glam::Vec3) -> Option<glam::Vec2> {
        let clip = self.projection * self.view * position.extend(1.0);
        if !clip.is_finite() || clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        let (x, y) = self.viewport.origin();
        let (width, height) = self.viewport.size();
        if width <= 0 || height <= 0 {
            return None;
        }
        Some(glam::Vec2::new(
            x as f32 + (ndc.x + 1.0) * 0.5 * width as f32,
            y as f32 + (1.0 - ndc.y) * 0.5 * height as f32,
        ))
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
