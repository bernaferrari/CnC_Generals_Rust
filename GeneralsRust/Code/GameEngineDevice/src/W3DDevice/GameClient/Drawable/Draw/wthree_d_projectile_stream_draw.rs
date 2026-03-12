//! W3D projectile stream draw module (port of W3DProjectileStreamDraw.cpp).

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::{SegmentedLine, TextureMapMode};
use crate::W3DDevice::GameClient::Module::wthree_d_projectile_stream_draw::W3DProjectileStreamDrawModuleData;
use cgmath::{Point3, Vector2};

const MAX_PROJECTILE_STREAM: usize = 20;

#[derive(Debug)]
pub struct W3DProjectileStreamDraw {
    data: W3DProjectileStreamDrawModuleData,
    lines: [Option<RenderObjectId>; MAX_PROJECTILE_STREAM],
    lines_valid: usize,
}

impl W3DProjectileStreamDraw {
    pub fn new(data: W3DProjectileStreamDrawModuleData) -> Self {
        Self {
            data,
            lines: [None; MAX_PROJECTILE_STREAM],
            lines_valid: 0,
        }
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();
        for line_id in self.lines.iter().flatten() {
            if let Some(line) = scene_guard.get_segmented_line(*line_id) {
                line.write().set_visible(!fully_obscured);
            }
        }
    }

    pub fn update_points(&mut self, all_points: &[Point3<f32>]) {
        let mut points_used = all_points.len();
        let zero = Point3::new(0.0, 0.0, 0.0);

        if self.data.max_segments > 0 {
            if points_used > self.data.max_segments as usize {
                points_used = self.data.max_segments as usize;
            }
        }

        let start_index = all_points.len().saturating_sub(points_used);
        let mut lines_made = 0usize;
        let mut staging: Vec<Point3<f32>> = Vec::with_capacity(MAX_PROJECTILE_STREAM);
        let mut current = start_index;

        while current < all_points.len() {
            while current < all_points.len() && all_points[current] != zero {
                staging.push(all_points[current]);
                current += 1;
            }

            if staging.len() > 1 {
                self.make_or_update_line(&staging, lines_made);
                lines_made += 1;
            }

            current += 1;
            staging.clear();
        }

        // Remove unused lines.
        for idx in lines_made..self.lines_valid {
            if let Some(line_id) = self.lines[idx].take() {
                let scene = W3DDisplay::global_scene();
                let mut scene_guard = scene.write();
                scene_guard.remove_render_object(line_id);
            }
        }
        self.lines_valid = lines_made;
    }

    fn make_or_update_line(&mut self, points: &[Point3<f32>], line_index: usize) {
        let mut new_line = false;
        if self.lines[line_index].is_none() {
            let mut line = SegmentedLine::new();
            line.set_texture_name(if self.data.texture_name.is_empty() {
                None
            } else {
                Some(self.data.texture_name.clone())
            });
            line.set_width(self.data.width);
            line.set_texture_mapping_mode(TextureMapMode::Tiled);
            line.set_texture_tile_factor(self.data.tile_factor);
            line.set_uv_offset_rate(Vector2::new(0.0, self.data.scroll_rate));
            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();
            let id = scene_guard.add_segmented_line(line);
            self.lines[line_index] = Some(id);
            self.lines_valid = self.lines_valid.max(line_index + 1);
            new_line = true;
        }

        if let Some(line_id) = self.lines[line_index] {
            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();
            if let Some(line) = scene_guard.get_segmented_line(line_id) {
                line.write().set_points(points);
                if new_line {
                    line.write().set_visible(true);
                }
            }
        }
    }
}

impl Drop for W3DProjectileStreamDraw {
    fn drop(&mut self) {
        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();
        for line_id in self.lines.iter().flatten() {
            scene_guard.remove_render_object(*line_id);
        }
    }
}
