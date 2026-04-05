//! W3D rope draw module (port of W3DRopeDraw.cpp).
//!
//! ## Pipeline Status: DEAD CODE (not instantiated at runtime)
//!
//! This struct is never created or called anywhere in the draw pipeline. The
//! active implementation is `gamelogic::object::draw::W3DRopeDraw`, which is
//! instantiated by `module_overrides.rs` and dispatched by
//! `GameLogic Drawable::draw()`.
//!
//! However, the GameLogic version only computes segment positions in
//! memory — it never creates `SegmentedLine` objects in
//! `W3DDisplay::global_scene()`. This file contains the **reference
//! rendering implementation** that shows how rope segments should be
//! submitted to the W3D scene once the pipeline gap is closed.
//!
//! ### Why this can't be simply wired in
//!
//! See `wthree_d_laser_draw.rs` for the dependency-chain explanation.
//! The same architectural constraint applies to all line-based draw modules.

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::SegmentedLine;
use cgmath::{Point3, Vector3};

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
struct SegInfo {
    line: RenderObjectId,
    soft_line: RenderObjectId,
    wobble_axis_x: f32,
    wobble_axis_y: f32,
}

#[derive(Debug)]
pub struct W3DRopeDraw {
    segments: Vec<SegInfo>,
    cur_len: f32,
    max_len: f32,
    width: f32,
    color: RGBColor,
    cur_speed: f32,
    max_speed: f32,
    accel: f32,
    wobble_len: f32,
    wobble_amp: f32,
    wobble_rate: f32,
    cur_wobble_phase: f32,
    cur_z_offset: f32,
    start_pos: Point3<f32>,
}

impl W3DRopeDraw {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            cur_len: 0.0,
            max_len: 1.0,
            width: 0.5,
            color: RGBColor::new(0.0, 0.0, 0.0),
            cur_speed: 0.0,
            max_speed: 0.0,
            accel: 0.0,
            wobble_len: 1.0,
            wobble_amp: 0.0,
            wobble_rate: 0.0,
            cur_wobble_phase: 0.0,
            cur_z_offset: 0.0,
            start_pos: Point3::new(0.0, 0.0, 0.0),
        }
    }

    pub fn init_rope_parms(
        &mut self,
        length: f32,
        width: f32,
        color: RGBColor,
        wobble_len: f32,
        wobble_amp: f32,
        wobble_rate: f32,
    ) {
        self.max_len = length.max(1.0);
        self.cur_len = 0.0;
        self.width = width;
        self.color = color;
        self.wobble_len = wobble_len.min(self.max_len);
        self.wobble_amp = wobble_amp;
        self.wobble_rate = wobble_rate;
        self.cur_z_offset = 0.0;

        self.toss_segments();
        self.build_segments();
    }

    pub fn set_rope_cur_len(&mut self, length: f32) {
        self.cur_len = length;
    }

    pub fn set_rope_speed(&mut self, cur_speed: f32, max_speed: f32, accel: f32) {
        self.cur_speed = cur_speed;
        self.max_speed = max_speed;
        self.accel = accel;
    }

    pub fn set_start_position(&mut self, start: Point3<f32>) {
        self.start_pos = start;
    }

    pub fn do_draw_module(&mut self) {
        if self.segments.is_empty() {
            self.build_segments();
        }

        if !self.segments.is_empty() {
            let deflection = self.cur_wobble_phase.sin() * self.wobble_amp;
            let mut start = Point3::new(
                self.start_pos.x,
                self.start_pos.y,
                self.start_pos.z + self.cur_z_offset,
            );
            let each_len = if self.segments.is_empty() {
                0.0
            } else {
                self.cur_len / self.segments.len() as f32
            };

            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();

            for seg in &self.segments {
                let end = Point3::new(
                    self.start_pos.x + deflection * seg.wobble_axis_x,
                    self.start_pos.y + deflection * seg.wobble_axis_y,
                    start.z - each_len,
                );

                if let Some(line) = scene_guard.get_segmented_line(seg.line) {
                    line.write().set_points(&[start, end]);
                }
                if let Some(line) = scene_guard.get_segmented_line(seg.soft_line) {
                    line.write().set_points(&[start, end]);
                }
                start = end;
            }
        }

        self.cur_wobble_phase += self.wobble_rate;
        if self.cur_wobble_phase > 2.0 * std::f32::consts::PI {
            self.cur_wobble_phase -= 2.0 * std::f32::consts::PI;
        }

        self.cur_z_offset += self.cur_speed;
        self.cur_speed += self.accel;
        if self.cur_speed > self.max_speed {
            self.cur_speed = self.max_speed;
        } else if self.cur_speed < -self.max_speed {
            self.cur_speed = -self.max_speed;
        }
    }

    fn build_segments(&mut self) {
        if !self.segments.is_empty() {
            return;
        }

        let num_segs = (self.max_len / self.wobble_len).ceil().max(1.0) as usize;
        let each_len = self.max_len / num_segs as f32;
        let mut pos = Point3::new(self.start_pos.x, self.start_pos.y, self.start_pos.z);

        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();

        for _ in 0..num_segs {
            let axis = fastrand::f32() * 2.0 * std::f32::consts::PI;
            let wobble_axis_x = axis.cos();
            let wobble_axis_y = axis.sin();

            let mut line = SegmentedLine::new();
            line.set_width(self.width * 0.5);
            line.set_color(Vector3::new(
                self.color.red,
                self.color.green,
                self.color.blue,
            ));
            line.set_points(&[
                Point3::new(pos.x, pos.y, pos.z),
                Point3::new(pos.x, pos.y, pos.z + each_len),
            ]);

            let mut soft_line = SegmentedLine::new();
            soft_line.set_width(self.width);
            soft_line.set_color(Vector3::new(
                self.color.red,
                self.color.green,
                self.color.blue,
            ));
            soft_line.set_opacity(0.5);
            soft_line.set_points(&[
                Point3::new(pos.x, pos.y, pos.z),
                Point3::new(pos.x, pos.y, pos.z + each_len),
            ]);

            let line_id = scene_guard.add_segmented_line(line);
            let soft_id = scene_guard.add_segmented_line(soft_line);

            self.segments.push(SegInfo {
                line: line_id,
                soft_line: soft_id,
                wobble_axis_x,
                wobble_axis_y,
            });

            pos.z += each_len;
        }
    }

    fn toss_segments(&mut self) {
        let scene = W3DDisplay::global_scene();
        let mut scene_guard = scene.write();
        for seg in self.segments.drain(..) {
            scene_guard.remove_render_object(seg.line);
            scene_guard.remove_render_object(seg.soft_line);
        }
    }
}

impl Default for W3DRopeDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for W3DRopeDraw {
    fn drop(&mut self) {
        self.toss_segments();
    }
}
