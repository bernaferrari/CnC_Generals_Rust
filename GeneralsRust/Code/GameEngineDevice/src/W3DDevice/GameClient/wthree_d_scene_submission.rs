use cgmath::Point3;
use game_engine::common::system::scene_submission::{SceneLineDesc, SceneLineId, SceneSubmission};
use parking_lot::RwLock;

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::{SegmentedLine, TextureMapMode};

pub struct DeviceSceneSubmission;

impl DeviceSceneSubmission {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeviceSceneSubmission {
    fn default() -> Self {
        Self::new()
    }
}

fn coord3d_to_point3(c: &game_engine::common::system::geometry::Coord3D) -> Point3<f32> {
    Point3::new(c.x, c.y, c.z)
}

impl SceneSubmission for DeviceSceneSubmission {
    fn submit_line(&self, _drawable_id: u32, desc: &SceneLineDesc) -> Option<SceneLineId> {
        let mut line = SegmentedLine::new();
        line.set_points(&[coord3d_to_point3(&desc.start), coord3d_to_point3(&desc.end)]);
        line.set_width(desc.width);
        line.set_color(cgmath::Vector3::new(
            desc.color_r,
            desc.color_g,
            desc.color_b,
        ));
        line.set_opacity(desc.opacity);
        line.set_visible(desc.visible);
        if let Some(ref tex) = desc.texture_name {
            line.set_texture_name(Some(tex.clone()));
        }
        if desc.tile_factor > 0.0 {
            line.set_texture_tile_factor(desc.tile_factor);
            line.set_texture_mapping_mode(TextureMapMode::Tiled);
        }

        let scene = W3DDisplay::global_scene();
        let mut guard = scene.write();
        Some(guard.add_segmented_line(line))
    }

    fn update_line(&self, id: SceneLineId, desc: &SceneLineDesc) {
        let scene = W3DDisplay::global_scene();
        let guard = scene.read();
        if let Some(line_arc) = guard.get_segmented_line(id) {
            let mut line = line_arc.write();
            line.set_points(&[coord3d_to_point3(&desc.start), coord3d_to_point3(&desc.end)]);
            line.set_width(desc.width);
            line.set_color(cgmath::Vector3::new(
                desc.color_r,
                desc.color_g,
                desc.color_b,
            ));
            line.set_opacity(desc.opacity);
            line.set_visible(desc.visible);
        }
    }

    fn remove_line(&self, id: SceneLineId) {
        let scene = W3DDisplay::global_scene();
        let mut guard = scene.write();
        guard.remove_segmented_line(id);
    }
}

impl DeviceSceneSubmission {
    pub fn new() -> Self {
        Self {
            scene: Arc::new(RwLock::new(())),
        }
    }
}

impl Default for DeviceSceneSubmission {
    fn default() -> Self {
        Self::new()
    }
}

fn coord3d_to_point3(c: &game_engine::common::system::geometry::Coord3D) -> Point3<f32> {
    Point3::new(c.x, c.y, c.z)
}

impl SceneSubmission for DeviceSceneSubmission {
    fn submit_line(&self, _drawable_id: u32, desc: &SceneLineDesc) -> Option<SceneLineId> {
        let mut line = SegmentedLine::new();
        line.set_points(&[coord3d_to_point3(&desc.start), coord3d_to_point3(&desc.end)]);
        line.set_width(desc.width);
        line.set_color(cgmath::Vector3::new(
            desc.color_r,
            desc.color_g,
            desc.color_b,
        ));
        line.set_opacity(desc.opacity);
        line.set_visible(desc.visible);
        if let Some(ref tex) = desc.texture_name {
            line.set_texture_name(Some(tex.clone()));
        }
        if desc.tile_factor > 0.0 {
            line.set_texture_tile_factor(desc.tile_factor);
            line.set_texture_mapping_mode(TextureMapMode::Tiled);
        }

        let scene = W3DDisplay::global_scene();
        let mut guard = scene.write();
        Some(guard.add_segmented_line(line))
    }

    fn update_line(&self, id: SceneLineId, desc: &SceneLineDesc) {
        let scene = W3DDisplay::global_scene();
        let guard = scene.read();
        if let Some(line_arc) = guard.get_segmented_line(id) {
            let mut line = line_arc.write();
            line.set_points(&[coord3d_to_point3(&desc.start), coord3d_to_point3(&desc.end)]);
            line.set_width(desc.width);
            line.set_color(cgmath::Vector3::new(
                desc.color_r,
                desc.color_g,
                desc.color_b,
            ));
            line.set_opacity(desc.opacity);
            line.set_visible(desc.visible);
        }
    }

    fn remove_line(&self, id: SceneLineId) {
        let scene = W3DDisplay::global_scene();
        let mut guard = scene.write();
        guard.remove_segmented_line(id);
    }
}
