//! W3D laser draw module (port of GameClient/Drawable/Draw/W3DLaserDraw.cpp).
//!
//! ## Pipeline Status: DEAD CODE (not instantiated at runtime)
//!
//! This struct is never created or called anywhere in the draw pipeline. The
//! active implementation is `gamelogic::object::draw::W3DLaserDraw`, which is
//! instantiated by `module_overrides.rs` and dispatched by
//! `GameLogic Drawable::draw()`.
//!
//! However, the GameLogic version only **computes beam geometry** into
//! `Vec<LaserLine>` — it never submits `SegmentedLine` objects to
//! `W3DDisplay::global_scene()`. This file contains the **reference
//! rendering implementation** that shows how the geometry should be
//! submitted to the W3D scene once the pipeline gap is closed.
//!
//! ### Why this can't be simply wired in
//!
//! The dependency chain is: `GameLogic → Common ← GameClient ← GameEngineDevice`.
//! GameLogic cannot depend on GameEngineDevice (circular dependency), so it
//! cannot call `W3DDisplay::global_scene()`. Wiring requires either:
//! - Moving scene-submission infrastructure into `Common`
//! - Adding a callback trait in `Common` that Device implements
//! - A bridge layer in GameClient/Device that syncs GameLogic state to Device
//!
//! This rendering gap affects ALL draw modules (see `W3DModelDraw::do_draw_module()`
//! which has `let _ = transform_mtx;` with a TODO comment).

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::{SegmentedLine, TextureMapMode};
use crate::W3DDevice::GameClient::Module::wthree_d_laser_draw::W3DLaserDrawModuleData;
use cgmath::{InnerSpace, Point3, Vector2, Vector3};
use image::io::Reader as ImageReader;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct W3DLaserDraw {
    data: W3DLaserDrawModuleData,
    line_ids: Vec<RenderObjectId>,
    texture_aspect_ratio: f32,
    self_dirty: bool,
    start_pos: Point3<f32>,
    end_pos: Point3<f32>,
    width_scale: f32,
}

impl W3DLaserDraw {
    pub fn new(data: W3DLaserDrawModuleData) -> Self {
        let mut draw = Self {
            data,
            line_ids: Vec::new(),
            texture_aspect_ratio: 1.0,
            self_dirty: true,
            start_pos: Point3::new(0.0, 0.0, 0.0),
            end_pos: Point3::new(0.0, 0.0, 0.0),
            width_scale: 1.0,
        };
        draw.allocate_lines();
        draw
    }

    pub fn set_positions(&mut self, start: Point3<f32>, end: Point3<f32>) {
        self.start_pos = start;
        self.end_pos = end;
        self.self_dirty = true;
    }

    pub fn set_width_scale(&mut self, width_scale: f32) {
        self.width_scale = width_scale;
        self.self_dirty = true;
    }

    pub fn do_draw_module(&mut self) {
        if !self.self_dirty {
            return;
        }

        let beams = self.data.num_beams.max(1);
        let segments = self.data.segments.max(1);
        let use_arc = self.data.arc_height > 0.0 && segments > 1;

        let (inner_r, inner_g, inner_b, inner_a) = self.data.inner_color.to_real();
        let (outer_r, outer_g, outer_b, _outer_a) = self.data.outer_color.to_real();

        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();

        for segment in 0..segments {
            let (seg_start, seg_end) = if use_arc {
                let line_start = self.start_pos;
                let line_end = self.end_pos;
                let line_vector = line_end - line_start;
                let line_length = line_vector.magnitude();
                let half_length = line_length * 0.5;
                if half_length <= 0.0001 {
                    (line_start, line_end)
                } else {
                    let line_middle = line_start + (line_end - line_start) * 0.5;
                    let mut start_ratio = segment as f32 / segments as f32;
                    let mut end_ratio = (segment as f32 + 1.0) / segments as f32;
                    if segment > 0 {
                        start_ratio -= self.data.segment_overlap_ratio;
                    }
                    if segment < segments - 1 {
                        end_ratio += self.data.segment_overlap_ratio;
                    }

                    let segment_start = line_start + line_vector * start_ratio;
                    let segment_end = line_start + line_vector * end_ratio;

                    let dist_start = (line_middle - segment_start).magnitude();
                    let dist_end = (line_middle - segment_end).magnitude();

                    let scaled_start = dist_start / half_length * std::f32::consts::PI * 0.5;
                    let scaled_end = dist_end / half_length * std::f32::consts::PI * 0.5;

                    let height_start = scaled_start.cos() * self.data.arc_height;
                    let height_end = scaled_end.cos() * self.data.arc_height;

                    let mut curved_start = segment_start;
                    let mut curved_end = segment_end;
                    curved_start.z += height_start;
                    curved_end.z += height_end;

                    (curved_start, curved_end)
                }
            } else {
                (self.start_pos, self.end_pos)
            };

            for beam in (0..beams).rev() {
                let index = (segment * beams + beam) as usize;
                let line_id = self.line_ids[index];
                let Some(line_arc) = scene_guard.get_segmented_line(line_id) else {
                    continue;
                };
                let mut line = line_arc.write();

                let width = if beams == 1 {
                    self.data.inner_beam_width * self.width_scale
                } else {
                    let scale = beam as f32 / (beams - 1) as f32;
                    (self.data.inner_beam_width
                        + scale * (self.data.outer_beam_width - self.data.inner_beam_width))
                        * self.width_scale
                };

                if self.data.tile && width > 0.0 {
                    let length = (seg_end - seg_start).magnitude();
                    let tile_factor =
                        length / width * self.texture_aspect_ratio * self.data.tiling_scalar;
                    line.set_texture_tile_factor(tile_factor);
                    line.set_texture_mapping_mode(TextureMapMode::Tiled);
                }

                let (red, green, blue) = if beams == 1 {
                    (inner_r * inner_a, inner_g * inner_a, inner_b * inner_a)
                } else {
                    let scale = beam as f32 / (beams - 1) as f32;
                    (
                        inner_r + scale * (outer_r - inner_r) * inner_a,
                        inner_g + scale * (outer_g - inner_g) * inner_a,
                        inner_b + scale * (outer_b - inner_b) * inner_a,
                    )
                };

                line.set_width(width);
                line.set_color(Vector3::new(red, green, blue));
                line.set_points(&[seg_start, seg_end]);
                line.set_visible(true);
            }
        }

        self.self_dirty = false;
    }

    pub fn on_delete(&mut self) {
        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();
        for id in self.line_ids.drain(..) {
            scene_guard.remove_render_object(id);
        }
    }

    fn allocate_lines(&mut self) {
        let beams = self.data.num_beams.max(1);
        let segments = self.data.segments.max(1);
        let total = (beams * segments) as usize;

        if !self.data.texture_name.is_empty() {
            self.texture_aspect_ratio =
                resolve_texture_aspect_ratio(&self.data.texture_name).unwrap_or(1.0);
        }

        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();

        self.line_ids.clear();
        self.line_ids.reserve(total);

        for _ in 0..total {
            let mut line = SegmentedLine::new();
            line.set_visible(false);
            if !self.data.texture_name.is_empty() {
                line.set_texture_name(Some(self.data.texture_name.clone()));
                line.set_texture_mapping_mode(TextureMapMode::Tiled);
            }
            line.set_uv_offset_rate(Vector2::new(0.0, self.data.scroll_rate));

            let id = scene_guard.add_segmented_line(line);
            self.line_ids.push(id);
        }
    }
}

impl Drop for W3DLaserDraw {
    fn drop(&mut self) {
        self.on_delete();
    }
}

fn resolve_texture_aspect_ratio(texture_name: &str) -> Option<f32> {
    let candidate_paths = texture_search_paths(texture_name);
    for path in candidate_paths {
        if let Ok(img) = ImageReader::open(&path).and_then(|r| r.decode()) {
            let width = img.width() as f32;
            let height = img.height() as f32;
            if height > 0.0 {
                return Some(width / height);
            }
        }
    }
    None
}

fn texture_search_paths(texture_name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let name_path = Path::new(texture_name);
    if name_path.exists() {
        paths.push(name_path.to_path_buf());
        return paths;
    }

    let extensions = ["png", "jpg", "jpeg", "tga", "bmp"];
    let base_dirs = [
        PathBuf::from("Art/Textures"),
        PathBuf::from("Data/Art/Textures"),
        PathBuf::from("Textures"),
        PathBuf::from("Data/Textures"),
    ];

    for dir in base_dirs {
        for ext in extensions {
            let candidate = dir.join(format!("{texture_name}.{ext}"));
            if candidate.exists() {
                paths.push(candidate);
            }
        }
    }

    paths
}
