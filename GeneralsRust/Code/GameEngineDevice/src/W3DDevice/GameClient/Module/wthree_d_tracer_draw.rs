//! W3D tracer draw module (port of W3DTracerDraw.h / W3DTracerDraw.cpp).
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTracerDraw.h
//! - GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTracerDraw.cpp
//!
//! Tracer drawing for projectiles with configurable speed, length, width, color, and opacity.

use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::SegmentedLine;
use cgmath::{EuclideanSpace, Matrix4, Point3, SquareMatrix, Transform, Vector3};

/// RGB color for tracer rendering (matching C++ RGBColor).
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

/// W3D tracer draw implementation (matching C++ W3DTracerDraw).
///
/// Manages a 3D line render object that moves along the X axis each frame.
/// Supports opacity decay based on expiration frame.
#[derive(Debug)]
pub struct W3DTracerDraw {
    /// The tracer render object in the W3D scene (Line3DClass equivalent)
    line_id: Option<RenderObjectId>,
    /// Length of tracer
    length: f32,
    /// Width of tracer
    width: f32,
    /// Color of tracer
    color: RGBColor,
    /// Speed of tracer (in dist/frame)
    speed_in_dist_per_frame: f32,
    /// Opacity of the tracer
    opacity: f32,
    /// Transform matrix (matches drawable transform)
    transform: Matrix4<f32>,
    /// Frame at which tracer expires (0 = no expiration)
    expiration_frame: Option<u32>,
}

impl W3DTracerDraw {
    /// Create new tracer draw with C++ default values.
    ///
    /// Default values from W3DTracerDraw constructor:
    /// - opacity: 1.0
    /// - length: 20.0
    /// - width: 0.5
    /// - color: (0.9, 0.8, 0.7)
    /// - speed: 1.0
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

    /// Set tracer parameters (matching C++ setTracerParms).
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

    /// Set transform matrix (matching C++ reactToTransformChange / Set_Transform).
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

    /// Set the expiration frame for opacity decay.
    pub fn set_expiration_frame(&mut self, frame: Option<u32>) {
        self.expiration_frame = frame;
    }

    /// Draw module update (matching C++ doDrawModule).
    ///
    /// Creates the tracer line on first call, then each frame:
    /// - Decays opacity if expiration is set
    /// - Moves tracer along X axis by speed_in_dist_per_frame
    pub fn do_draw_module(&mut self, current_frame: u32) {
        // Create tracer if not yet created
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

        // Handle expiration-based opacity decay
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

        // Move tracer along X axis
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

    /// Clean up render resources (matching C++ destructor).
    pub fn on_delete(&mut self) {
        if let Some(line_id) = self.line_id.take() {
            let scene = W3DDisplay::global_scene();
            let mut scene_guard = scene.write();
            scene_guard.remove_render_object(line_id);
        }
    }

    /// Compute start and end points in world space from transform.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_w3d_tracer_draw_creation() {
        let tracer = W3DTracerDraw::new();
        assert_eq!(tracer.length, 20.0);
        assert_eq!(tracer.width, 0.5);
        assert_eq!(tracer.opacity, 1.0);
        assert_eq!(tracer.speed_in_dist_per_frame, 1.0);
        assert!((tracer.color.red - 0.9).abs() < 0.001);
        assert!((tracer.color.green - 0.8).abs() < 0.001);
        assert!((tracer.color.blue - 0.7).abs() < 0.001);
        assert!(tracer.line_id.is_none());
    }

    #[test]
    fn test_w3d_tracer_set_parms() {
        let mut tracer = W3DTracerDraw::new();
        tracer.set_tracer_parms(2.0, 30.0, 1.0, RGBColor::new(1.0, 0.0, 0.0), 0.8);
        assert_eq!(tracer.speed_in_dist_per_frame, 2.0);
        assert_eq!(tracer.length, 30.0);
        assert_eq!(tracer.width, 1.0);
        assert_eq!(tracer.opacity, 0.8);
        assert_eq!(tracer.color.red, 1.0);
    }

    #[test]
    fn test_w3d_tracer_default() {
        let tracer = W3DTracerDraw::default();
        assert_eq!(tracer.length, 20.0);
    }
}
