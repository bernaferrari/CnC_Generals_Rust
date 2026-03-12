//! Segmented line renderer – Rust port of WW3D's SegLineRendererClass.
//!
//! This implementation rebuilds the dynamic line ribbon geometry that the original DX8 renderer
//! produced, including subdivision, UV animation, and optional procedural noise.

use crate::material_system::{MaterialPassClass, VertexMaterialClass};
use crate::render_object_system::AABoxClass;
use crate::rendering::mesh_system::{MeshClass, MeshModelClass};
use crate::rendering::shader_system::shader::ShaderClass;
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::Arc;
use ww3d_collision::SphereClass;
use ww3d_core::w3d_format::{W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct};

/// Renderer that generates ribbon geometry for segmented line effects.
#[derive(Debug, Clone)]
pub struct SegLineRenderer {
    width: f32,
    color: Vec4,
    subdivision_level: u32,
    noise_amplitude: f32,
    merge_abort_factor: f32,
    texture_tile_factor: f32,
    uv_offset: Vec2,
    uv_scroll_rate: Vec2,
    end_caps_enabled: bool,
    shader: ShaderClass,
}

impl SegLineRenderer {
    pub fn new() -> Self {
        Self {
            width: 1.0,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            subdivision_level: 0,
            noise_amplitude: 0.0,
            merge_abort_factor: 1.5,
            texture_tile_factor: 1.0,
            uv_offset: Vec2::ZERO,
            uv_scroll_rate: Vec2::ZERO,
            end_caps_enabled: true,
            shader: ShaderClass::get_opaque_shader(),
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.max(0.0001);
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
    }

    pub fn color(&self) -> Vec4 {
        self.color
    }

    pub fn set_subdivision_level(&mut self, level: u32) {
        self.subdivision_level = level.min(6); // Clamp to sane limit
    }

    pub fn subdivision_level(&self) -> u32 {
        self.subdivision_level
    }

    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.noise_amplitude = amplitude.max(0.0);
    }

    pub fn set_merge_abort_factor(&mut self, factor: f32) {
        self.merge_abort_factor = factor.max(0.0);
    }

    pub fn set_texture_tile_factor(&mut self, factor: f32) {
        self.texture_tile_factor = factor.clamp(0.01, 128.0);
    }

    pub fn set_uv_offset(&mut self, offset: Vec2) {
        self.uv_offset = offset;
    }

    pub fn set_uv_scroll_rate(&mut self, rate: Vec2) {
        self.uv_scroll_rate = rate;
    }

    pub fn set_end_caps_enabled(&mut self, enabled: bool) {
        self.end_caps_enabled = enabled;
    }

    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.shader = shader;
    }

    pub fn shader(&self) -> &ShaderClass {
        &self.shader
    }

    pub fn sort_hint(&self) -> u32 {
        if self.opacity() < 1.0 {
            0x8000_0000
        } else {
            0
        }
    }

    pub fn opacity(&self) -> f32 {
        self.color.w.clamp(0.0, 1.0)
    }

    /// Generate a transient mesh for the line between `start` and `end` transformed by `transform`.
    pub fn generate_mesh(
        &self,
        start: Vec3,
        end: Vec3,
        transform: Mat4,
        camera_pos: Vec3,
        time_seconds: f32,
        name: &str,
    ) -> Option<Arc<MeshClass>> {
        let world_a = transform.transform_point3(start);
        let world_b = transform.transform_point3(end);
        let line_vec = world_b - world_a;
        if line_vec.length_squared() <= f32::EPSILON {
            return None;
        }

        let line_dir = line_vec.normalize();
        let center = (world_a + world_b) * 0.5;
        let mut view_dir = (camera_pos - center).normalize_or_zero();
        if view_dir.length_squared() < 1e-6 {
            view_dir = Vec3::Y;
        }

        let mut right = view_dir.cross(line_dir).normalize_or_zero();
        if right.length_squared() < 1e-6 {
            // View direction aligned with line; choose fallback axis.
            let fallback = if line_dir.cross(Vec3::Y).length_squared() > 1e-6 {
                Vec3::Y
            } else {
                Vec3::Z
            };
            right = fallback.cross(line_dir).normalize();
        }
        let normal = line_dir.cross(right).normalize_or_zero();

        let segments = 1usize << self.subdivision_level;
        let step = 1.0 / segments as f32;
        let animated_uv = self.uv_offset + self.uv_scroll_rate * time_seconds;

        let mut points = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f32 * step;
            let mut point = world_a + line_vec * t;
            if i != 0 && i != segments {
                let noise_s = pseudo_noise(i as u32, 0, time_seconds);
                let noise_t = pseudo_noise(i as u32, 1, time_seconds);
                let jitter_right = (noise_s - 0.5) * 2.0 * self.noise_amplitude;
                let jitter_normal = (noise_t - 0.5) * 2.0 * self.noise_amplitude;
                point += right * jitter_right + normal * jitter_normal;
            }
            points.push(point);
        }

        // Pre-allocate based on segment count
        // Each segment creates 4 vertices (2 at each end, top/bottom) and 6 indices (2 triangles)
        let mut vertices = Vec::with_capacity(segments * 4);
        let mut indices = Vec::with_capacity(segments * 6);
        let mut texcoords = Vec::with_capacity(segments * 4);
        let mut vertex_colors = Vec::with_capacity(segments * 4);
        let mut normals = Vec::with_capacity(segments * 4);

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut accumulated_length = 0.0f32;
        let tile_scale = self.texture_tile_factor.max(0.01);

        for seg in 0..segments {
            let p0 = points[seg];
            let p1 = points[seg + 1];
            let segment_vec = p1 - p0;
            let segment_length = segment_vec.length();
            if segment_length <= f32::EPSILON {
                continue;
            }
            let segment_dir = segment_vec / segment_length;
            let mut seg_right = view_dir.cross(segment_dir).normalize_or_zero();
            if seg_right.length_squared() < 1e-6 {
                seg_right = right;
            }
            let seg_normal = segment_dir.cross(seg_right).normalize_or_zero();
            let half_width = self.width * 0.5;
            let offset = seg_right * half_width;

            let base_index = vertices.len() as u32;
            let u0 = animated_uv.x + accumulated_length * tile_scale;
            accumulated_length += segment_length;
            let u1 = animated_uv.x + accumulated_length * tile_scale;
            let v0 = animated_uv.y;
            let v1 = animated_uv.y + 1.0;

            let quad = [
                (p0 - offset, Vec2::new(u0, v1)),
                (p0 + offset, Vec2::new(u0, v0)),
                (p1 + offset, Vec2::new(u1, v0)),
                (p1 - offset, Vec2::new(u1, v1)),
            ];

            for (pos, uv) in quad {
                min = min.min(pos);
                max = max.max(pos);
                vertices.push(W3dVectorStruct::from(pos));
                texcoords.push(W3dTexCoordStruct { u: uv.x, v: uv.y });
                vertex_colors.push(self.color);
                normals.push(W3dVectorStruct::from(seg_normal));
            }

            indices.extend_from_slice(&[
                base_index,
                base_index + 1,
                base_index + 2,
                base_index,
                base_index + 2,
                base_index + 3,
            ]);
        }

        if vertices.is_empty() {
            return None;
        }

        let mut model = MeshModelClass::new(name);
        model.vertices = vertices;
        model.vertex_count = model.vertices.len() as u32;
        model.index_count = indices.len() as u32;
        model.normals = normals;
        model.triangles = indices
            .chunks_exact(3)
            .map(|chunk| W3dTriangleStruct {
                vindex: [chunk[0], chunk[1], chunk[2]],
                attributes: 0,
                normal: W3dVectorStruct::from(normal),
                distance: 0.0,
            })
            .collect();
        model.texture_coords = texcoords;

        let mut pass = MaterialPassClass::new();
        let shader = if self.opacity() < 1.0 {
            ShaderClass::get_alpha_shader()
        } else {
            self.shader.clone()
        };
        pass.shader = shader;
        pass.diffuse_vertex_colors = Some(vertex_colors);

        let mut vertex_material = VertexMaterialClass::new("SegLineMaterial");
        vertex_material.diffuse = self.color.truncate();
        vertex_material.opacity = self.opacity();
        vertex_material.ambient = vertex_material.diffuse * 0.2;
        vertex_material.emissive = vertex_material.diffuse * 0.1;
        pass.vertex_material = Some(Arc::new(vertex_material));
        model.material_passes = vec![pass];
        model.register_for_rendering();

        let mut mesh = MeshClass::new();
        mesh.name = name.to_string();
        mesh.model = Some(Arc::new(model));
        mesh.alpha_override = self.opacity();
        mesh.material_pass_alpha_override = self.opacity();
        mesh.material_pass_emissive_override = 1.0;

        let bbox = AABoxClass::from_min_max(min, max);
        let sphere_center = (min + max) * 0.5;
        let sphere_radius = (max - min).length() * 0.5;
        mesh.bounding_box = bbox;
        mesh.bounding_sphere = SphereClass::new(sphere_center, sphere_radius);
        mesh.set_transform(Mat4::IDENTITY);
        mesh.sort_level = self.sort_hint();
        mesh.update_cached_bounding_volumes();

        Some(Arc::new(mesh))
    }
}

impl Default for SegLineRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn pseudo_noise(index: u32, axis: u32, time: f32) -> f32 {
    let seed = (index as f32 * 12.9898) + (axis as f32 * 78.233) + time * 37.719;
    (seed.sin() * 43758.5453).fract()
}
