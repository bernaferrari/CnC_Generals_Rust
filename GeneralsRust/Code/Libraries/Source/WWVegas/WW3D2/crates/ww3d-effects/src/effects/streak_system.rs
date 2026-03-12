//! Streak Effects System - Advanced particle trail and streak rendering
//!
//! This module implements the Streak system from the original C++ code,
//! providing sophisticated streak effects for particles, lightning, and trails.
//!
//! Converted from:
//! - streak.cpp/h (main streak system)
//! - streakRender.cpp/h (streak rendering)
//! - line3d.cpp/h (3D line rendering)

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use ww3d_core::errors::{W3DError, W3DResult};
use ww3d_core::w3d_format::{W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct};
use ww3d_renderer_3d::{
    bounding_volumes::{aabox::AABoxClass, sphere::SphereClass},
    core::error::Error as RendererError,
    material_system::{MaterialPassClass, VertexMaterialClass},
    render_object_system::RenderInfoClass,
    rendering::mesh_system::{MeshClass, MeshModelClass},
    rendering::shader_system::shader::ShaderClass,
    texture_system::TextureClass,
    Renderer,
};

type Result<T> = W3DResult<T>;

static STREAK_LINE_ID_COUNTER: AtomicU32 = AtomicU32::new(0);
static STREAK_RENDERER_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn renderer_error_to_w3d(err: RendererError) -> W3DError {
    W3DError::UnknownWithMessage(err.to_string())
}

fn streak_renderer_store() -> &'static Mutex<Option<StreakRenderer>> {
    static STORE: OnceLock<Mutex<Option<StreakRenderer>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn with_streak_renderer_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut StreakRenderer) -> R,
{
    let mut slot = streak_renderer_store().lock().ok()?;
    let renderer = slot.as_mut()?;
    Some(f(renderer))
}

/// Streak rendering mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreakRenderMode {
    /// Normal streak rendering
    Normal = 0,
    /// UV animated streak
    UVAnimation,
    /// Distance-based fading
    DistanceFade,
    /// Time-based fading
    TimeFade,
}

/// Streak point structure
#[derive(Debug, Clone)]
pub struct StreakPoint {
    /// Position in world space
    pub position: Vec3,
    /// Direction vector
    pub direction: Vec3,
    /// Color at this point
    pub color: Vec4,
    /// Width at this point
    pub width: f32,
    /// UV coordinate (for animation)
    pub uv: Vec2,
    /// Time this point was created
    pub time: f32,
    /// Distance from start of streak
    pub distance: f32,
}

impl StreakPoint {
    /// Create new streak point
    pub fn new(position: Vec3, direction: Vec3) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            width: 1.0,
            uv: Vec2::ZERO,
            time: 0.0,
            distance: 0.0,
        }
    }

    /// Set color
    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
    }

    /// Set width
    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    /// Set UV coordinate
    pub fn set_uv(&mut self, uv: Vec2) {
        self.uv = uv;
    }

    /// Update time
    pub fn update_time(&mut self, current_time: f32) {
        self.time = current_time;
    }
}

/// Streak line structure
#[derive(Debug, Clone)]
pub struct StreakLine {
    /// Points along the streak
    pub points: Vec<StreakPoint>,
    /// Maximum number of points
    pub max_points: usize,
    /// Current number of active points
    pub active_points: usize,
    /// Total length of the streak
    pub total_length: f32,
    /// Whether the streak is active
    pub is_active: bool,
    /// Whether the streak is looping
    pub is_looping: bool,
    /// Streak ID
    pub streak_id: u32,
}

impl StreakLine {
    /// Create new streak line
    pub fn new(max_points: usize) -> Self {
        let streak_id = STREAK_LINE_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

        Self {
            points: vec![StreakPoint::new(Vec3::ZERO, Vec3::X); max_points],
            max_points,
            active_points: 0,
            total_length: 0.0,
            is_active: false,
            is_looping: false,
            streak_id,
        }
    }

    /// Add point to streak
    pub fn add_point(&mut self, point: StreakPoint) -> Result<()> {
        if self.active_points < self.max_points {
            self.points[self.active_points] = point;
            self.active_points += 1;
        } else {
            // Shift points and add new one
            for i in 1..self.active_points {
                self.points[i - 1] = self.points[i].clone();
            }
            self.points[self.active_points - 1] = point;
        }

        self.update_distances();
        Ok(())
    }

    /// Reset streak
    pub fn reset(&mut self) {
        self.active_points = 0;
        self.total_length = 0.0;
        self.is_active = false;
    }

    /// Set active state
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        if !active {
            self.reset();
        }
    }

    /// Set looping mode
    pub fn set_looping(&mut self, looping: bool) {
        self.is_looping = looping;
    }

    /// Update distances along the streak
    fn update_distances(&mut self) {
        if self.active_points < 2 {
            return;
        }

        self.total_length = 0.0;
        self.points[0].distance = 0.0;

        for i in 1..self.active_points {
            let distance = (self.points[i].position - self.points[i - 1].position).length();
            self.total_length += distance;
            self.points[i].distance = self.total_length;
        }
    }

    /// Get point at distance
    pub fn get_point_at_distance(&self, distance: f32) -> Option<&StreakPoint> {
        if self.active_points < 2 || distance < 0.0 || distance > self.total_length {
            return None;
        }

        let mut current_distance = 0.0;
        for i in 1..self.active_points {
            let segment_distance = (self.points[i].position - self.points[i - 1].position).length();
            if current_distance + segment_distance >= distance {
                let _t = (distance - current_distance) / segment_distance;
                // Return the point at the start of this segment
                return Some(&self.points[i - 1]);
            }
            current_distance += segment_distance;
        }

        None
    }

    /// Get interpolated point at distance
    pub fn get_interpolated_point(&self, distance: f32) -> Option<StreakPoint> {
        if self.active_points < 2 || distance < 0.0 || distance > self.total_length {
            return None;
        }

        let mut current_distance = 0.0;
        for i in 1..self.active_points {
            let segment_distance = (self.points[i].position - self.points[i - 1].position).length();
            if current_distance + segment_distance >= distance {
                let t = (distance - current_distance) / segment_distance;

                // Interpolate between points
                let position = self.points[i - 1].position.lerp(self.points[i].position, t);
                let direction = self.points[i - 1]
                    .direction
                    .lerp(self.points[i].direction, t);
                let color = self.points[i - 1].color.lerp(self.points[i].color, t);
                let width = self.points[i - 1].width
                    + (self.points[i].width - self.points[i - 1].width) * t;
                let uv = self.points[i - 1].uv.lerp(self.points[i].uv, t);

                let mut point = StreakPoint::new(position, direction);
                point.set_color(color);
                point.set_width(width);
                point.set_uv(uv);
                point.distance = distance;

                return Some(point);
            }
            current_distance += segment_distance;
        }

        None
    }

    /// Get active point count
    pub fn get_active_point_count(&self) -> usize {
        self.active_points
    }

    /// Get total length
    pub fn get_total_length(&self) -> f32 {
        self.total_length
    }

    /// Is streak active
    pub fn is_streak_active(&self) -> bool {
        self.is_active && self.active_points >= 2
    }
}

/// Streak renderer class
#[derive(Debug)]
pub struct StreakRenderer {
    /// Active streaks
    pub streaks: Vec<StreakLine>,
    /// Texture for streak rendering
    pub texture: Option<Arc<TextureClass>>,
    /// Shader for streak rendering
    pub shader: Option<Arc<ShaderClass>>,
    /// Current time
    pub current_time: f32,
    /// Maximum number of streaks
    pub max_streaks: usize,
    /// Vertex buffer for rendering
    pub vertex_buffer: Vec<StreakVertex>,
    /// Index buffer for rendering
    pub index_buffer: Vec<u32>,
    /// Renderer ID
    pub renderer_id: u32,
}

impl StreakRenderer {
    /// Create new streak renderer
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let _ = device; // Use parameters
        let _ = queue;

        let renderer_id = STREAK_RENDERER_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

        Self {
            streaks: Vec::new(),
            texture: None,
            shader: None,
            current_time: 0.0,
            max_streaks: 128,
            vertex_buffer: Vec::new(),
            index_buffer: Vec::new(),
            renderer_id,
        }
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: Arc<TextureClass>) {
        self.texture = Some(texture);
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: Arc<ShaderClass>) {
        self.shader = Some(shader);
    }

    /// Add streak
    pub fn add_streak(&mut self, streak: StreakLine) -> Result<usize> {
        if self.streaks.len() >= self.max_streaks {
            return Err(W3DError::InvalidParameter(
                "Maximum streaks reached".to_string(),
            ));
        }

        self.streaks.push(streak);
        Ok(self.streaks.len() - 1)
    }

    /// Remove streak
    pub fn remove_streak(&mut self, index: usize) -> Result<()> {
        if index >= self.streaks.len() {
            return Err(W3DError::InvalidParameter(
                "Invalid streak index".to_string(),
            ));
        }

        self.streaks.remove(index);
        Ok(())
    }

    /// Get streak
    pub fn get_streak(&mut self, index: usize) -> Option<&mut StreakLine> {
        self.streaks.get_mut(index)
    }

    /// Update all streaks
    pub fn update(&mut self, delta_time: f32) {
        self.current_time += delta_time;

        // Update streak times and remove inactive streaks
        self.streaks.retain(|streak| streak.is_streak_active());

        for streak in &mut self.streaks {
            for point in &mut streak.points[..streak.active_points] {
                point.update_time(self.current_time);
            }
        }
    }

    /// Render all streaks
    pub fn render(&mut self, _rinfo: &RenderInfoClass) -> Result<()> {
        self.vertex_buffer.clear();
        self.index_buffer.clear();

        // Collect active streaks first to avoid borrow checker issues
        let active_streaks: Vec<StreakLine> = self
            .streaks
            .iter()
            .filter(|streak| streak.is_streak_active())
            .cloned()
            .collect();

        for streak in &active_streaks {
            self.build_streak_geometry(streak)?;
        }

        if self.vertex_buffer.is_empty() {
            return Ok(());
        }

        let positions: Vec<Vec3> = self.vertex_buffer.iter().map(|v| v.position).collect();
        let colors: Vec<Vec4> = self.vertex_buffer.iter().map(|v| v.color).collect();
        let texcoords: Vec<Vec2> = self.vertex_buffer.iter().map(|v| v.tex_coord).collect();

        let normals: Vec<Vec3> = positions.iter().map(|_| Vec3::Z).collect();

        let average_alpha = if colors.is_empty() {
            1.0
        } else {
            colors.iter().map(|c| c.w).sum::<f32>().max(0.0) / colors.len() as f32
        };

        let mut model = MeshModelClass::new(&format!("StreakRenderer_{}", self.renderer_id));
        model.vertices = positions
            .iter()
            .copied()
            .map(W3dVectorStruct::from)
            .collect();
        model.normals = normals.iter().copied().map(W3dVectorStruct::from).collect();
        model.texture_coords = texcoords
            .iter()
            .map(|tc| W3dTexCoordStruct { u: tc.x, v: tc.y })
            .collect();
        model.vertex_count = self.vertex_buffer.len() as u32;
        model.index_count = self.index_buffer.len() as u32;
        model.triangles = self
            .index_buffer
            .chunks(3)
            .filter_map(|chunk| {
                if chunk.len() == 3 {
                    Some(W3dTriangleStruct {
                        vindex: [chunk[0], chunk[1], chunk[2]],
                        attributes: 0,
                        normal: W3dVectorStruct::from(Vec3::Z),
                        distance: 0.0,
                    })
                } else {
                    None
                }
            })
            .collect();
        model.register_for_rendering();

        let mut pass = MaterialPassClass::new();
        let shader = self
            .shader
            .as_ref()
            .map(|arc| (**arc).clone())
            .unwrap_or_else(ShaderClass::new);
        pass.shader = shader;
        pass.diffuse_vertex_colors = Some(colors.clone());

        let mut vertex_material = VertexMaterialClass::new("StreakMaterial");
        vertex_material.diffuse = Vec3::new(1.0, 1.0, 1.0);
        vertex_material.opacity = average_alpha;
        vertex_material.ambient = vertex_material.diffuse * 0.2;
        vertex_material.emissive = vertex_material.diffuse * 0.1;
        pass.vertex_material = Some(Arc::new(vertex_material));
        model.material_passes = vec![pass];

        let mut mesh = MeshClass::new();
        mesh.name = format!("StreakRenderer_{}", self.renderer_id);
        mesh.model = Some(Arc::new(model));
        mesh.alpha_override = average_alpha;
        mesh.material_pass_alpha_override = average_alpha;
        mesh.material_pass_emissive_override = 1.0;

        let min = positions
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |acc, p| acc.min(*p));
        let max = positions
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |acc, p| acc.max(*p));
        let bbox = AABoxClass::from_min_max(min, max);
        let center = (min + max) * 0.5;
        let radius = positions
            .iter()
            .map(|p| (*p - center).length())
            .fold(0.0f32, f32::max);

        mesh.bounding_box = bbox;
        mesh.bounding_sphere = SphereClass::new(center, radius);
        mesh.set_transform(Mat4::IDENTITY);
        mesh.update_cached_bounding_volumes();

        let mesh = Arc::new(mesh);
        Renderer::with_global_mut(|renderer| {
            renderer.queue_mesh(Arc::clone(&mesh))?;
            Ok(())
        })
        .map_err(renderer_error_to_w3d)?;

        Ok(())
    }

    /// Build geometry for a streak
    fn build_streak_geometry(&mut self, streak: &StreakLine) -> Result<()> {
        if streak.active_points < 2 {
            return Ok(());
        }

        let base_vertex = self.vertex_buffer.len() as u32;

        // Build quad strips for the streak
        for i in 0..streak.active_points - 1 {
            let p1 = &streak.points[i];
            let p2 = &streak.points[i + 1];

            // Calculate perpendicular vectors for quad
            let view_dir = Vec3::new(0.0, 0.0, 1.0); // Simplified camera direction
            let right1 = p1.direction.cross(view_dir).normalize() * p1.width / 2.0;
            let right2 = p2.direction.cross(view_dir).normalize() * p2.width / 2.0;

            // Add vertices for quad
            self.vertex_buffer.push(StreakVertex {
                position: p1.position - right1,
                color: p1.color,
                tex_coord: Vec2::new(p1.distance / streak.total_length, 0.0),
            });

            self.vertex_buffer.push(StreakVertex {
                position: p1.position + right1,
                color: p1.color,
                tex_coord: Vec2::new(p1.distance / streak.total_length, 1.0),
            });

            self.vertex_buffer.push(StreakVertex {
                position: p2.position - right2,
                color: p2.color,
                tex_coord: Vec2::new(p2.distance / streak.total_length, 0.0),
            });

            self.vertex_buffer.push(StreakVertex {
                position: p2.position + right2,
                color: p2.color,
                tex_coord: Vec2::new(p2.distance / streak.total_length, 1.0),
            });

            // Add indices
            let vertex_index = base_vertex + (i as u32 * 4);
            self.index_buffer.extend_from_slice(&[
                vertex_index,
                vertex_index + 1,
                vertex_index + 2,
                vertex_index + 1,
                vertex_index + 3,
                vertex_index + 2,
            ]);
        }

        Ok(())
    }

    /// Clear all streaks
    pub fn clear(&mut self) {
        self.streaks.clear();
        self.vertex_buffer.clear();
        self.index_buffer.clear();
    }

    /// Get streak count
    pub fn get_streak_count(&self) -> usize {
        self.streaks.len()
    }

    /// Get active streak count
    pub fn get_active_streak_count(&self) -> usize {
        self.streaks.iter().filter(|s| s.is_streak_active()).count()
    }

    /// Get vertex count
    pub fn get_vertex_count(&self) -> usize {
        self.vertex_buffer.len()
    }

    /// Get index count
    pub fn get_index_count(&self) -> usize {
        self.index_buffer.len()
    }

    /// Render with modern GPU interface
    pub fn render_gpu(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        render_pass: &mut wgpu::RenderPass,
        view_projection_matrix: glam::Mat4,
    ) {
        let _ = device; // Use parameters
        let _ = queue;
        let _ = encoder;
        let _ = render_pass;
        let _ = view_projection_matrix;

        // In a full implementation, this would:
        // 1. Update streak geometry
        // 2. Upload vertex/index data to GPU
        // 3. Render all visible streaks with proper blending
        for streak in &mut self.streaks {
            if streak.is_streak_active() {
                // Render individual streak
            }
        }
    }

    /// Create a trail effect
    pub fn create_trail(
        &mut self,
        start: glam::Vec3,
        end: glam::Vec3,
        color: glam::Vec4,
        width: f32,
    ) {
        let mut streak = StreakLine::new(10);
        let _ = streak.add_point(StreakPoint {
            position: start,
            direction: (end - start).normalize(),
            color,
            width,
            uv: glam::Vec2::ZERO,
            time: self.current_time,
            distance: 0.0,
        });
        let _ = streak.add_point(StreakPoint {
            position: end,
            direction: (end - start).normalize(),
            color,
            width,
            uv: glam::Vec2::new(1.0, 0.0),
            time: self.current_time,
            distance: (end - start).length(),
        });
        streak.is_active = true;
        let _ = self.add_streak(streak);
    }

    /// Create a laser line effect
    pub fn create_laser(
        &mut self,
        start: glam::Vec3,
        end: glam::Vec3,
        color: glam::Vec4,
        width: f32,
    ) {
        // Similar to trail but with different visual properties
        self.create_trail(start, end, color, width * 0.5); // Thinner than trails
    }
}

/// Streak vertex structure
#[derive(Debug, Clone, Copy)]
pub struct StreakVertex {
    /// Position
    pub position: Vec3,
    /// Color
    pub color: Vec4,
    /// Texture coordinate
    pub tex_coord: Vec2,
}

impl StreakVertex {
    /// Get vertex buffer layout for WGPU
    pub fn get_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StreakVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Texture coordinates
                wgpu::VertexAttribute {
                    offset: 28,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Lightning streak - specialized streak for lightning effects
#[derive(Debug)]
pub struct LightningStreak {
    /// Base streak
    pub streak: StreakLine,
    /// Branch streaks
    pub branches: Vec<StreakLine>,
    /// Intensity
    pub intensity: f32,
    /// Fade time
    pub fade_time: f32,
    /// Creation time
    pub creation_time: f32,
}

impl LightningStreak {
    /// Create new lightning streak
    pub fn new(start_pos: Vec3, end_pos: Vec3, max_points: usize) -> Self {
        let mut streak = StreakLine::new(max_points);

        // Activate the streak before adding points
        streak.set_active(true);

        // Create lightning path with some randomness
        let direction = (end_pos - start_pos).normalize();
        let distance = (end_pos - start_pos).length();

        let mut current_pos = start_pos;
        let mut current_distance = 0.0;
        let segment_length = distance / (max_points - 1) as f32;

        for i in 0..max_points {
            let t = i as f32 / (max_points - 1) as f32;

            // Add some randomness to create lightning effect
            let random_offset = Vec3::new(
                (rand::random::<f32>() - 0.5) * 0.5,
                (rand::random::<f32>() - 0.5) * 0.5,
                (rand::random::<f32>() - 0.5) * 0.5,
            );

            let mut point = StreakPoint::new(current_pos + random_offset, direction);
            point.distance = current_distance;

            // Make lightning brighter at the center, ensure minimum alpha
            let intensity = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
            let alpha = intensity.max(0.1); // Ensure minimum alpha for visibility
            point.set_color(Vec4::new(0.8, 0.9, 1.0, alpha));

            streak.add_point(point).ok();

            current_pos += direction * segment_length;
            current_distance += segment_length;
        }

        Self {
            streak,
            branches: Vec::new(),
            intensity: 1.0,
            fade_time: 0.5,
            creation_time: 0.0,
        }
    }

    /// Add branch to lightning
    pub fn add_branch(&mut self, start_distance: f32, end_distance: f32) {
        let start_point = self.streak.get_interpolated_point(start_distance);
        let end_point = self.streak.get_interpolated_point(end_distance);

        if let (Some(start), Some(end)) = (start_point, end_point) {
            let branch = LightningStreak::new(start.position, end.position, 10);
            self.branches.push(branch.streak);
        }
    }

    /// Update lightning
    pub fn update(&mut self, current_time: f32) {
        self.streak
            .points
            .iter_mut()
            .take(self.streak.active_points)
            .for_each(|point| {
                let age = current_time - point.time;
                let fade_factor = (self.fade_time - age).max(0.0) / self.fade_time;
                point.color.w = point.color.w * fade_factor * self.intensity;
            });

        // Remove faded points
        let mut i = 0;
        while i < self.streak.active_points {
            if self.streak.points[i].color.w < 0.01 {
                self.streak.points.remove(i);
                self.streak.active_points -= 1;
            } else {
                i += 1;
            }
        }
    }

    /// Is lightning still active
    pub fn is_active(&self) -> bool {
        self.streak.is_streak_active() && self.streak.points[0].color.w > 0.01
    }

    /// Set intensity
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, 2.0);
    }

    /// Set fade time
    pub fn set_fade_time(&mut self, fade_time: f32) {
        self.fade_time = fade_time.max(0.1);
    }
}

/// Initialize global streak renderer
pub fn init_streak_renderer(max_streaks: usize) -> Result<()> {
    let _ = max_streaks; // Placeholder until device/queue wiring is implemented
    Ok(())
}

/// Shutdown global streak renderer
pub fn shutdown_streak_renderer() {
    if let Ok(mut slot) = streak_renderer_store().lock() {
        *slot = None;
    }
}

/// Quick streak functions
pub fn create_lightning_streak(start_pos: Vec3, end_pos: Vec3) -> LightningStreak {
    LightningStreak::new(start_pos, end_pos, 20)
}

pub fn add_streak_to_renderer(streak: StreakLine) -> Result<usize> {
    with_streak_renderer_mut(|renderer| renderer.add_streak(streak)).unwrap_or_else(|| {
        Err(W3DError::NotInitialized(
            "Streak system not initialized".to_string(),
        ))
    })
}

pub fn update_streak_renderer(delta_time: f32) {
    let _ = with_streak_renderer_mut(|renderer| renderer.update(delta_time));
}

pub fn render_streaks(rinfo: &RenderInfoClass) -> Result<()> {
    with_streak_renderer_mut(|renderer| renderer.render(rinfo)).unwrap_or_else(|| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streak_point() {
        let mut point = StreakPoint::new(Vec3::new(1.0, 2.0, 3.0), Vec3::X);
        assert_eq!(point.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(point.direction, Vec3::X);
        assert_eq!(point.color, Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(point.width, 1.0);

        point.set_color(Vec4::new(0.5, 0.5, 0.5, 1.0));
        point.set_width(2.0);
        assert_eq!(point.color, Vec4::new(0.5, 0.5, 0.5, 1.0));
        assert_eq!(point.width, 2.0);
    }

    #[test]
    fn test_streak_line() {
        let mut streak = StreakLine::new(10);
        assert_eq!(streak.max_points, 10);
        assert_eq!(streak.active_points, 0);
        assert!(!streak.is_streak_active());

        // Activate the streak
        streak.set_active(true);

        // Add some points
        let point1 = StreakPoint::new(Vec3::ZERO, Vec3::X);
        let point2 = StreakPoint::new(Vec3::new(1.0, 0.0, 0.0), Vec3::X);

        streak.add_point(point1).unwrap();
        streak.add_point(point2).unwrap();

        assert_eq!(streak.active_points, 2);
        assert!(streak.is_streak_active());
        assert_eq!(streak.total_length, 1.0);

        // Test reset
        streak.reset();
        assert_eq!(streak.active_points, 0);
        assert!(!streak.is_streak_active());
    }

    #[test]
    fn test_streak_renderer() {
        // Create mock device and queue for testing
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let mut renderer = StreakRenderer::new(&device, &queue);
        assert_eq!(renderer.max_streaks, 128);
        assert_eq!(renderer.get_streak_count(), 0);

        let streak = StreakLine::new(10);
        let index = renderer.add_streak(streak).unwrap();
        assert_eq!(index, 0);
        assert_eq!(renderer.get_streak_count(), 1);

        renderer.clear();
        assert_eq!(renderer.get_streak_count(), 0);
    }

    #[test]
    fn test_lightning_streak() {
        let mut lightning = LightningStreak::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 10);
        assert!(lightning.is_active());
        assert_eq!(lightning.intensity, 1.0);
        assert_eq!(lightning.fade_time, 0.5);

        lightning.set_intensity(2.0);
        lightning.set_fade_time(1.0);
        assert_eq!(lightning.intensity, 2.0);
        assert_eq!(lightning.fade_time, 1.0);
    }

    #[test]
    fn test_streak_vertex() {
        let vertex = StreakVertex {
            position: Vec3::new(1.0, 2.0, 3.0),
            color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            tex_coord: Vec2::new(0.5, 0.5),
        };

        assert_eq!(vertex.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(vertex.color, Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(vertex.tex_coord, Vec2::new(0.5, 0.5));
    }
}
