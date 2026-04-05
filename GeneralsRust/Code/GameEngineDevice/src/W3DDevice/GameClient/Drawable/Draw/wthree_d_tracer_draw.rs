//! W3D tracer draw module (port of W3DTracerDraw.cpp).
//!
//! ## Pipeline Status: DEAD CODE (not instantiated at runtime)
//!
//! This struct is never created or called anywhere in the draw pipeline. The
//! active implementation is `gamelogic::object::draw::W3DTracerDraw`, which is
//! instantiated by `module_overrides.rs` and dispatched by
//! `GameLogic Drawable::draw()`.
//!
//! However, the GameLogic version only updates `current_pos`/`line_end` in
//! memory — it never creates `SegmentedLine` objects in
//! `W3DDisplay::global_scene()`. This file contains the **reference
//! rendering implementation** that shows how tracer lines should be
//! submitted to the W3D scene once the pipeline gap is closed.
//!
//! ### Why this can't be simply wired in
//!
//! See `wthree_d_laser_draw.rs` for the dependency-chain explanation.
//! The same architectural constraint applies to all line-based draw modules.

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::SegmentedLine;
use cgmath::{EuclideanSpace, Matrix4, Point3, SquareMatrix, Transform, Vector3};

#[derive(Debug, Clone, Copy)]
pub struct RGBColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl RGBColor {
    pub fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }
}

#[derive(Debug)]
pub struct W3DTracerDraw {
    line_id: Option<RenderObjectId>,
    length: f32,
    width: f32,
    color: RGBColor,
    speed_in_dist_per_frame: f32,
    opacity: f32,
    transform: Matrix4<f32>,
    expiration_frame: Option<u32>,
}

impl W3DTracerDraw {
    pub fn new() -> Self {
        Self {
            line_id: None,
            length: 20.0,
            width: 0.5,
            color: RGBColor::new(0.9, 0.8, 0.7),
            speed_in_dist_per_frame: 1.0,
            opacity: 1.0,
            transform: Matrix4::identity(),
            expiration_frame: None,
        }
    }

    pub fn set_tracer_parms(
        &mut self,
        speed: f32,
        length: f32,
        width: f32,
        color: RGBColor,
        initial_opacity: f32,
    ) {
        self.speed_in_dist_per_frame = speed;
        self.length = length;
        self.width = width;
        self.color = color;
        self.opacity = initial_opacity;

        if let Some(line_id) = self.line_id {
            let scene = W3DDisplay::global_scene();
            if let Some(line) = scene.read().get_segmented_line(line_id) {
                let mut line = line.write();
                line.set_width(self.width);
                line.set_color(Vector3::new(
                    self.color.red,
                    self.color.green,
                    self.color.blue,
                ));
                line.set_opacity(self.opacity);
                let (start, end) = self.compute_endpoints();
                line.set_points(&[start, end]);
            }
        }
    }

    pub fn set_transform(&mut self, transform: Matrix4<f32>) {
        self.transform = transform;
        if let Some(line_id) = self.line_id {
            let scene = W3DDisplay::global_scene();
            if let Some(line) = scene.read().get_segmented_line(line_id) {
                let mut line = line.write();
                let (start, end) = self.compute_endpoints();
                line.set_points(&[start, end]);
            }
        }
    }

    pub fn set_expiration_frame(&mut self, frame: Option<u32>) {
        self.expiration_frame = frame;
    }

    pub fn do_draw_module(&mut self, current_frame: u32) {
        if self.line_id.is_none() {
            let mut line = SegmentedLine::new();
            line.set_width(self.width);
            line.set_color(Vector3::new(
                self.color.red,
                self.color.green,
                self.color.blue,
            ));
            line.set_opacity(self.opacity);
            let (start, end) = self.compute_endpoints();
            line.set_points(&[start, end]);

            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();
            let id = scene_guard.add_segmented_line(line);
            self.line_id = Some(id);
        }

        if let Some(expiration) = self.expiration_frame {
            if expiration > current_frame {
                let decay = self.opacity / (expiration - current_frame) as f32;
                self.opacity = (self.opacity - decay).max(0.0);
                if let Some(line_id) = self.line_id {
                    let scene = W3DDisplay::global_scene();
                    if let Some(line) = scene.read().get_segmented_line(line_id) {
                        line.write().set_opacity(self.opacity);
                    }
                }
            }
        }

        if self.speed_in_dist_per_frame != 0.0 {
            let translation =
                Matrix4::from_translation(Vector3::new(self.speed_in_dist_per_frame, 0.0, 0.0));
            self.transform = self.transform * translation;
            if let Some(line_id) = self.line_id {
                let scene = W3DDisplay::global_scene();
                if let Some(line) = scene.read().get_segmented_line(line_id) {
                    let mut line = line.write();
                    let (start, end) = self.compute_endpoints();
                    line.set_points(&[start, end]);
                }
            }
        }
    }

    pub fn on_delete(&mut self) {
        if let Some(line_id) = self.line_id.take() {
            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();
            scene_guard.remove_render_object(line_id);
        }
    }

    fn compute_endpoints(&self) -> (Point3<f32>, Point3<f32>) {
        let start_local = Vector3::new(0.0, 0.0, 0.0);
        let end_local = Vector3::new(self.length, 0.0, 0.0);
        let start_world = self
            .transform
            .transform_point(Point3::from_vec(start_local));
        let end_world = self.transform.transform_point(Point3::from_vec(end_local));
        (start_world, end_world)
    }
}

impl Default for W3DTracerDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for W3DTracerDraw {
    fn drop(&mut self) {
        self.on_delete();
    }
}
