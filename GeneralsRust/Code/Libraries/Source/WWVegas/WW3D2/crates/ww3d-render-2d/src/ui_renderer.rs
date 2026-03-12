//! UI Renderer
//!
//! Provides immediate-mode helpers for rendering UI quads and outlines using the
//! shared Render2D batching layer with enhanced features like gradients,
//! rounded corners, and more primitive types.

use glam::Vec2;
use ww3d_renderer_3d::rendering::render2d::{Rect, Render2D, Render2DGpuContext, Vertex2D};

/// Convenience wrapper for composing UI primitives with enhanced features.
pub struct UiRenderer {
    render2d: Render2D,
    enable_anti_aliasing: bool,
}

impl UiRenderer {
    /// Construct an empty renderer.
    pub fn new() -> Self {
        Self {
            render2d: Render2D::new(),
            enable_anti_aliasing: false,
        }
    }

    /// Clear any queued geometry.
    pub fn reset(&mut self) {
        self.render2d.reset();
    }

    /// Enable or disable anti-aliasing for primitives
    pub fn set_anti_aliasing(&mut self, enabled: bool) {
        self.enable_anti_aliasing = enabled;
    }

    /// Draw a filled rectangle.
    pub fn draw_rect(&mut self, rect: Rect, color: u32) {
        self.render2d.add_quad_solid(rect, color);
    }

    /// Draw a rectangle with a vertical gradient
    pub fn draw_rect_vgradient(&mut self, rect: Rect, top_color: u32, bottom_color: u32) {
        self.render2d
            .add_quad_v_gradient(rect, top_color, bottom_color);
    }

    /// Draw a rectangle with a horizontal gradient
    pub fn draw_rect_hgradient(&mut self, rect: Rect, left_color: u32, right_color: u32) {
        self.render2d
            .add_quad_h_gradient(rect, left_color, right_color);
    }

    /// Draw a rectangle with a 4-corner gradient
    pub fn draw_rect_gradient(
        &mut self,
        rect: Rect,
        top_left: u32,
        top_right: u32,
        bottom_left: u32,
        bottom_right: u32,
    ) {
        // Use the underlying Render2D to create a quad with per-vertex colors
        // Since add_quad_colors doesn't exist, manually add vertices
        let base_index = self.render2d.vertices.len() as u16;

        let v0 = Vec2::new(rect.left, rect.top);
        let v1 = Vec2::new(rect.right, rect.top);
        let v2 = Vec2::new(rect.right, rect.bottom);
        let v3 = Vec2::new(rect.left, rect.bottom);

        // Add vertices with individual colors
        self.render2d
            .vertices
            .push(Vertex2D::new(v0, Vec2::new(0.0, 0.0), top_left));
        self.render2d
            .vertices
            .push(Vertex2D::new(v1, Vec2::new(1.0, 0.0), top_right));
        self.render2d
            .vertices
            .push(Vertex2D::new(v2, Vec2::new(1.0, 1.0), bottom_right));
        self.render2d
            .vertices
            .push(Vertex2D::new(v3, Vec2::new(0.0, 1.0), bottom_left));

        // Add indices for two triangles
        self.render2d.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);

        self.render2d.vertex_buffer = None; // Invalidate buffers
    }

    /// Draw a rounded rectangle (approximated with multiple quads)
    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: f32, color: u32, segments: u32) {
        let segments = segments.max(2);

        // Draw the main body
        let inner_rect = Rect::new(
            rect.left + radius,
            rect.top,
            rect.right - radius,
            rect.bottom,
        );
        self.draw_rect(inner_rect, color);

        let top_rect = Rect::new(
            rect.left,
            rect.top + radius,
            rect.right,
            rect.bottom - radius,
        );
        self.draw_rect(top_rect, color);

        // Draw corners as pie slices
        self.draw_rounded_corner(
            Vec2::new(rect.left + radius, rect.top + radius),
            radius,
            180.0,
            270.0,
            color,
            segments,
        );
        self.draw_rounded_corner(
            Vec2::new(rect.right - radius, rect.top + radius),
            radius,
            270.0,
            360.0,
            color,
            segments,
        );
        self.draw_rounded_corner(
            Vec2::new(rect.right - radius, rect.bottom - radius),
            radius,
            0.0,
            90.0,
            color,
            segments,
        );
        self.draw_rounded_corner(
            Vec2::new(rect.left + radius, rect.bottom - radius),
            radius,
            90.0,
            180.0,
            color,
            segments,
        );
    }

    /// Draw a rounded corner (helper for rounded rectangles)
    fn draw_rounded_corner(
        &mut self,
        center: Vec2,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        color: u32,
        segments: u32,
    ) {
        let angle_range = end_angle - start_angle;
        let angle_step = angle_range / segments as f32;

        for i in 0..segments {
            let angle1 = (start_angle + angle_step * i as f32).to_radians();
            let angle2 = (start_angle + angle_step * (i + 1) as f32).to_radians();

            let p1 = center + Vec2::new(angle1.cos() * radius, angle1.sin() * radius);
            let p2 = center + Vec2::new(angle2.cos() * radius, angle2.sin() * radius);

            // Draw triangle using add_triangle
            self.render2d.add_triangle(
                center,
                p1,
                p2,
                Vec2::new(0.5, 0.5),
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 1.0),
                color,
            );
        }
    }

    /// Draw a rectangle outline.
    pub fn draw_outline(&mut self, rect: Rect, line_width: f32, color: u32) {
        self.render2d.add_outline(rect, line_width, color);
    }

    /// Draw a rectangle with an inner fill and border.
    pub fn draw_panel(
        &mut self,
        rect: Rect,
        border_width: f32,
        border_color: u32,
        fill_color: u32,
    ) {
        self.render2d
            .add_rect(rect, border_width, border_color, fill_color);
    }

    /// Draw a panel with gradient fill and border
    pub fn draw_panel_gradient(
        &mut self,
        rect: Rect,
        border_width: f32,
        border_color: u32,
        top_color: u32,
        bottom_color: u32,
    ) {
        // Draw border
        self.draw_outline(rect, border_width, border_color);

        // Draw gradient fill inside
        let inner_rect = Rect::new(
            rect.left + border_width,
            rect.top + border_width,
            rect.right - border_width,
            rect.bottom - border_width,
        );
        self.draw_rect_vgradient(inner_rect, top_color, bottom_color);
    }

    /// Draw an individual line segment.
    pub fn draw_line(&mut self, start: Vec2, end: Vec2, width: f32, color: u32) {
        self.render2d.add_line(start, end, width, color);
    }

    /// Draw a line with gradient color
    pub fn draw_line_gradient(
        &mut self,
        start: Vec2,
        end: Vec2,
        width: f32,
        start_color: u32,
        end_color: u32,
    ) {
        // Implement line gradient manually using a quad with gradient colors
        let direction = (end - start).normalize();
        let perpendicular = Vec2::new(-direction.y, direction.x) * width * 0.5;

        let base_index = self.render2d.vertices.len() as u16;

        let v0 = start - perpendicular;
        let v1 = start + perpendicular;
        let v2 = end + perpendicular;
        let v3 = end - perpendicular;

        // Add vertices with gradient colors
        self.render2d
            .vertices
            .push(Vertex2D::new(v0, Vec2::new(0.0, 0.0), start_color));
        self.render2d
            .vertices
            .push(Vertex2D::new(v1, Vec2::new(1.0, 0.0), start_color));
        self.render2d
            .vertices
            .push(Vertex2D::new(v2, Vec2::new(1.0, 1.0), end_color));
        self.render2d
            .vertices
            .push(Vertex2D::new(v3, Vec2::new(0.0, 1.0), end_color));

        // Add indices
        self.render2d.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);

        self.render2d.vertex_buffer = None;
    }

    /// Draw a polyline (connected line segments)
    pub fn draw_polyline(&mut self, points: &[Vec2], width: f32, color: u32, closed: bool) {
        if points.len() < 2 {
            return;
        }

        for i in 0..points.len() - 1 {
            self.draw_line(points[i], points[i + 1], width, color);
        }

        if closed && points.len() > 2 {
            self.draw_line(points[points.len() - 1], points[0], width, color);
        }
    }

    /// Draw a circle (approximated with lines)
    pub fn draw_circle(&mut self, center: Vec2, radius: f32, color: u32, segments: u32) {
        let segments = segments.max(3);
        let angle_step = std::f32::consts::TAU / segments as f32;

        for i in 0..segments {
            let angle1 = angle_step * i as f32;
            let angle2 = angle_step * ((i + 1) % segments) as f32;

            let p1 = center + Vec2::new(angle1.cos() * radius, angle1.sin() * radius);
            let p2 = center + Vec2::new(angle2.cos() * radius, angle2.sin() * radius);

            self.render2d.add_triangle(
                center,
                p1,
                p2,
                Vec2::new(0.5, 0.5),
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 1.0),
                color,
            );
        }
    }

    /// Draw a circle outline
    pub fn draw_circle_outline(
        &mut self,
        center: Vec2,
        radius: f32,
        width: f32,
        color: u32,
        segments: u32,
    ) {
        let segments = segments.max(3);
        let angle_step = std::f32::consts::TAU / segments as f32;

        for i in 0..segments {
            let angle1 = angle_step * i as f32;
            let angle2 = angle_step * ((i + 1) % segments) as f32;

            let p1 = center + Vec2::new(angle1.cos() * radius, angle1.sin() * radius);
            let p2 = center + Vec2::new(angle2.cos() * radius, angle2.sin() * radius);

            self.draw_line(p1, p2, width, color);
        }
    }

    /// Draw an arc
    pub fn draw_arc(
        &mut self,
        center: Vec2,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        color: u32,
        segments: u32,
    ) {
        let segments = segments.max(2);
        let angle_range = end_angle - start_angle;
        let angle_step = angle_range / segments as f32;

        for i in 0..segments {
            let angle1 = (start_angle + angle_step * i as f32).to_radians();
            let angle2 = (start_angle + angle_step * (i + 1) as f32).to_radians();

            let p1 = center + Vec2::new(angle1.cos() * radius, angle1.sin() * radius);
            let p2 = center + Vec2::new(angle2.cos() * radius, angle2.sin() * radius);

            self.render2d.add_triangle(
                center,
                p1,
                p2,
                Vec2::new(0.5, 0.5),
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 1.0),
                color,
            );
        }
    }

    /// Draw a triangle
    pub fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: u32) {
        self.render2d.add_triangle(
            p1,
            p2,
            p3,
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 1.0),
            color,
        );
    }

    /// Draw a triangle with gradient
    pub fn draw_triangle_gradient(
        &mut self,
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
        c1: u32,
        c2: u32,
        c3: u32,
    ) {
        // Manually add triangle vertices with different colors
        let base_index = self.render2d.vertices.len() as u16;

        self.render2d
            .vertices
            .push(Vertex2D::new(p1, Vec2::new(0.0, 0.0), c1));
        self.render2d
            .vertices
            .push(Vertex2D::new(p2, Vec2::new(1.0, 0.0), c2));
        self.render2d
            .vertices
            .push(Vertex2D::new(p3, Vec2::new(0.5, 1.0), c3));

        self.render2d
            .indices
            .extend_from_slice(&[base_index, base_index + 1, base_index + 2]);

        self.render2d.vertex_buffer = None;
    }

    /// Submit the queued primitives to the GPU.
    pub fn render<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render2d.render(gpu, render_pass);
    }
}

impl Default for UiRenderer {
    fn default() -> Self {
        Self::new()
    }
}
