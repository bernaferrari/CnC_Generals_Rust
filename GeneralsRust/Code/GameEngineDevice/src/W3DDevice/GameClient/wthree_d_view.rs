//! W3DView Module - Complete 3D View and Camera Management System
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp
//! 
//! This module provides comprehensive 3D view management, camera controls, rendering pipeline,
//! viewport management, frustum culling, and object picking for the W3D graphics engine.

use cgmath::{
    Deg, Matrix4, Point3, Vector3, Vector4, 
    EuclideanSpace, InnerSpace, SquareMatrix,
};
use wgpu::{
    Device, Queue, Surface, SurfaceConfiguration, RenderPassDescriptor,
    CommandEncoder, Buffer, BufferDescriptor, BufferUsages, BindGroup,
    RenderPipeline, Sampler, Texture, TextureUsages, TextureView,
    BindGroupLayout, BindGroupLayoutEntry, BindGroupLayoutDescriptor,
    BindGroupDescriptor, BindGroupEntry, ShaderModuleDescriptor, ShaderSource,
    PipelineLayoutDescriptor, RenderPipelineDescriptor, VertexState, FragmentState,
    VertexBufferLayout, VertexAttribute, VertexFormat, VertexStepMode,
    PrimitiveState, PrimitiveTopology, ColorTargetState, BlendState, BlendComponent,
    ColorWrites, MultisampleState,
    Color, Operations, LoadOp, StoreOp,
};
use bytemuck::{Pod, Zeroable};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use parking_lot::RwLock as ParkingRwLock;
use smallvec::SmallVec;
use slotmap::{SlotMap, DefaultKey};
use anyhow::{Result, Context};
use thiserror::Error;
use game_network::NetworkInstant;
use crate::W3DDevice::GameClient::wthree_d_scene::W3DScene;
use crate::W3DDevice::GameClient::wthree_d_segmented_line::{SegmentedLine, TextureMapMode, compute_line_perp};
use image::io::Reader as ImageReader;
use image::GenericImageView;
use std::path::Path;

/// Maximum number of waypoints for camera movement
pub const MAX_WAYPOINTS: usize = 25;

/// Camera movement constraints
pub const MIN_ZOOM_DISTANCE: f32 = 10.0;
pub const MAX_ZOOM_DISTANCE: f32 = 1000.0;
pub const MIN_PITCH_ANGLE: f32 = -85.0;
pub const MAX_PITCH_ANGLE: f32 = 85.0;

/// Frustum culling constants
pub const FRUSTUM_PLANES_COUNT: usize = 6;

/// Render queue constants
pub const MAX_RENDER_OBJECTS: usize = 10000;

/// Viewport and screen coordinates
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct IRegion2D {
    pub min: ICoord2D,
    pub max: ICoord2D,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Point3<f32>> for Coord3D {
    fn from(point: Point3<f32>) -> Self {
        Self { x: point.x, y: point.y, z: point.z }
    }
}

impl From<Coord3D> for Point3<f32> {
    fn from(coord: Coord3D) -> Self {
        Point3::new(coord.x, coord.y, coord.z)
    }
}

/// Camera waypoint system for cinematic movements
#[derive(Debug, Clone)]
pub struct CameraWaypoint {
    pub position: Coord3D,
    pub look_at: Coord3D,
    pub camera_angle: f32,
    pub time_multiplier: i32,
    pub ground_height: f32,
}

/// Camera movement along waypoint paths
#[derive(Debug, Clone)]
pub struct MoveAlongWaypointPathInfo {
    pub waypoints: SmallVec<[CameraWaypoint; MAX_WAYPOINTS]>,
    pub way_seg_length: SmallVec<[f32; MAX_WAYPOINTS]>,
    pub total_time_milliseconds: u32,
    pub elapsed_time_milliseconds: u32,
    pub total_distance: f32,
    pub cur_seg_distance: f32,
    pub cur_segment: usize,
    pub shutter: i32,
    pub cur_shutter: i32,
    pub rolling_average_frames: i32,
    pub ease_factor: f32,
}

/// Camera rotation information
#[derive(Debug, Clone)]
pub struct RotateCameraInfo {
    pub num_frames: i32,
    pub cur_frame: i32,
    pub start_time_multiplier: i32,
    pub end_time_multiplier: i32,
    pub num_hold_frames: i32,
    pub ease_factor: f32,
    pub track_object: bool,
    pub target_object_id: Option<u32>,
    pub target_position: Coord3D,
    pub start_angle: f32,
    pub end_angle: f32,
}

/// Camera pitch information
#[derive(Debug, Clone)]
pub struct PitchCameraInfo {
    pub num_frames: i32,
    pub cur_frame: i32,
    pub start_pitch: f32,
    pub end_pitch: f32,
    pub start_time_multiplier: i32,
    pub end_time_multiplier: i32,
    pub ease_factor: f32,
}

/// Camera zoom information
#[derive(Debug, Clone)]
pub struct ZoomCameraInfo {
    pub num_frames: i32,
    pub cur_frame: i32,
    pub start_zoom: f32,
    pub end_zoom: f32,
    pub start_time_multiplier: i32,
    pub end_time_multiplier: i32,
    pub ease_factor: f32,
}

#[derive(Debug, Clone, Copy)]
struct ParabolicEase {
    ease_in: f32,
    ease_out: f32,
}

impl ParabolicEase {
    fn new(ease_in: f32, ease_out: f32) -> Self {
        let mut ease = Self {
            ease_in: 0.0,
            ease_out: 0.0,
        };
        ease.set_ease_times(ease_in, ease_out);
        ease
    }

    fn set_ease_times(&mut self, ease_in_time: f32, ease_out_time: f32) {
        let mut ease_in = ease_in_time.clamp(0.0, 1.0);
        let mut ease_out = 1.0 - ease_out_time;
        if !(0.0..=1.0).contains(&ease_out) {
            ease_out = ease_out.clamp(0.0, 1.0);
        }
        if ease_in > ease_out {
            ease_in = ease_out;
        }
        self.ease_in = ease_in;
        self.ease_out = ease_out;
    }

    fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let v0 = 1.0 + self.ease_out - self.ease_in;
        if t < self.ease_in {
            t * t / (v0 * self.ease_in)
        } else if t <= self.ease_out {
            (self.ease_in + 2.0 * (t - self.ease_in)) / v0
        } else {
            (self.ease_in
                + 2.0 * (self.ease_out - self.ease_in)
                + (2.0 * (t - self.ease_out) + self.ease_out * self.ease_out - t * t)
                    / (1.0 - self.ease_out))
                / v0
        }
    }
}

/// Frustum plane for culling
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct FrustumPlane {
    pub normal: Vector3<f32>,
    pub distance: f32,
}

/// Complete frustum for culling calculations
#[derive(Debug, Clone)]
pub struct ViewFrustum {
    pub planes: [FrustumPlane; FRUSTUM_PLANES_COUNT],
    pub corners: [Point3<f32>; 8],
}

/// Object picking result
#[derive(Debug, Clone)]
pub struct PickResult {
    pub object_id: Option<u32>,
    pub world_position: Point3<f32>,
    pub distance: f32,
    pub surface_normal: Vector3<f32>,
}

/// Pick type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickType {
    Normal,
    ForceAttack,
    SelectionOnly,
    TerrainOnly,
}

/// Render queue for efficient object sorting
#[derive(Debug)]
pub struct RenderQueue {
    pub opaque_objects: Vec<u32>,
    pub transparent_objects: Vec<(u32, f32)>, // (object_id, distance)
    pub shadow_casters: Vec<u32>,
    pub ui_elements: Vec<u32>,
}

impl Default for RenderQueue {
    fn default() -> Self {
        Self {
            opaque_objects: Vec::with_capacity(MAX_RENDER_OBJECTS / 2),
            transparent_objects: Vec::with_capacity(MAX_RENDER_OBJECTS / 4),
            shadow_casters: Vec::with_capacity(MAX_RENDER_OBJECTS / 3),
            ui_elements: Vec::with_capacity(100),
        }
    }
}

/// Camera state and configuration
#[derive(Debug, Clone)]
pub struct CameraState {
    pub position: Point3<f32>,
    pub look_at: Point3<f32>,
    pub up_vector: Vector3<f32>,
    pub field_of_view: Deg<f32>,
    pub aspect_ratio: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub zoom_factor: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            position: Point3::new(0.0, 100.0, 100.0),
            look_at: Point3::new(0.0, 0.0, 0.0),
            up_vector: Vector3::new(0.0, 1.0, 0.0),
            field_of_view: Deg(60.0),
            aspect_ratio: 16.0 / 9.0,
            near_plane: 1.0,
            far_plane: 1000.0,
            zoom_factor: 1.0,
            pitch: 0.0,
            yaw: 0.0,
            roll: 0.0,
        }
    }
}

/// Viewport configuration
#[derive(Debug, Clone)]
pub struct ViewportConfig {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub min_depth: f32,
    pub max_depth: f32,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

/// Performance metrics for the view system
#[derive(Debug, Default)]
pub struct ViewMetrics {
    pub frame_time: Duration,
    pub render_calls: u32,
    pub objects_rendered: u32,
    pub objects_culled: u32,
    pub triangles_rendered: u64,
    pub draw_calls: u32,
    pub texture_switches: u32,
    pub shader_switches: u32,
}

/// Main W3D View implementation with complete 3D rendering capabilities
#[derive(Debug)]
pub struct W3DView {
    // Core graphics state
    pub device: Option<Arc<Device>>,
    pub queue: Option<Arc<Queue>>,
    pub surface: Option<Arc<Surface>>,
    pub surface_config: Option<SurfaceConfiguration>,
    
    // Camera and view state
    pub camera: CameraState,
    pub viewport: ViewportConfig,
    pub view_matrix: Matrix4<f32>,
    pub projection_matrix: Matrix4<f32>,
    pub view_projection_matrix: Matrix4<f32>,
    pub inverse_view_matrix: Matrix4<f32>,
    
    // Frustum culling
    pub frustum: ViewFrustum,
    pub frustum_dirty: bool,
    
    // Animation and movement
    pub waypoint_info: Option<MoveAlongWaypointPathInfo>,
    pub rotation_info: Option<RotateCameraInfo>,
    pub pitch_info: Option<PitchCameraInfo>,
    pub zoom_info: Option<ZoomCameraInfo>,
    
    // Rendering system
    pub render_queue: RenderQueue,
    pub render_pipeline: Option<Arc<RenderPipeline>>,
    pub uniform_buffer: Option<Buffer>,
    pub bind_group: Option<BindGroup>,
    pub line_renderer: Option<LineRenderer>,
    
    // Resource management
    pub textures: HashMap<String, Arc<Texture>>,
    pub samplers: HashMap<String, Arc<Sampler>>,
    pub buffers: SlotMap<DefaultKey, Buffer>,
    
    // Performance tracking
    pub metrics: ViewMetrics,
    pub last_frame_time: NetworkInstant,
    
    // State flags
    pub initialized: bool,
    pub needs_redraw: bool,
    pub wireframe_mode: bool,
    pub debug_mode: bool,
    
    // Thread safety
    pub state_lock: Arc<ParkingRwLock<()>>,
}

impl Default for W3DView {
    fn default() -> Self {
        Self::new()
    }
}

/// Error types for W3D View operations
#[derive(Error, Debug)]
pub enum W3DViewError {
    #[error("Graphics device not initialized")]
    DeviceNotInitialized,
    #[error("Surface configuration failed: {0}")]
    SurfaceConfigError(String),
    #[error("Shader compilation failed: {0}")]
    ShaderError(String),
    #[error("Buffer creation failed: {0}")]
    BufferError(String),
    #[error("Texture creation failed: {0}")]
    TextureError(String),
    #[error("Render pipeline creation failed: {0}")]
    PipelineError(String),
    #[error("Invalid camera parameters: {0}")]
    InvalidCameraParams(String),
    #[error("Picking operation failed: {0}")]
    PickingError(String),
    #[error("View matrix calculation failed")]
    MatrixError,
    #[error("Frustum culling calculation failed")]
    CullingError,
}

impl W3DView {
    /// Create a new W3D View instance
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            camera: CameraState::default(),
            viewport: ViewportConfig::default(),
            view_matrix: Matrix4::identity(),
            projection_matrix: Matrix4::identity(),
            view_projection_matrix: Matrix4::identity(),
            inverse_view_matrix: Matrix4::identity(),
            frustum: ViewFrustum {
                planes: [FrustumPlane {
                    normal: Vector3::new(0.0, 0.0, 0.0),
                    distance: 0.0,
                }; FRUSTUM_PLANES_COUNT],
                corners: [Point3::origin(); 8],
            },
            frustum_dirty: true,
            waypoint_info: None,
            rotation_info: None,
            pitch_info: None,
            zoom_info: None,
            render_queue: RenderQueue::default(),
            render_pipeline: None,
            uniform_buffer: None,
            bind_group: None,
            line_renderer: None,
            textures: HashMap::new(),
            samplers: HashMap::new(),
            buffers: SlotMap::new(),
            metrics: ViewMetrics::default(),
            last_frame_time: NetworkInstant::now(),
            initialized: false,
            needs_redraw: true,
            wireframe_mode: false,
            debug_mode: false,
            state_lock: Arc::new(ParkingRwLock::new(())),
        }
    }

    /// Initialize the view with a GPU device, queue, and surface configuration.
    pub fn initialize(
        &mut self,
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface: Arc<Surface>,
        mut surface_config: SurfaceConfiguration,
    ) -> Result<()> {
        surface_config.width = surface_config.width.max(1);
        surface_config.height = surface_config.height.max(1);
        surface.configure(&device, &surface_config);

        self.device = Some(device.clone());
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);

        self.line_renderer = Some(LineRenderer::new(
            &device,
            &self.queue.as_ref().unwrap(),
            self.surface_config.as_ref().unwrap().format,
        )?);
        self.initialized = true;
        self.needs_redraw = true;
        self.update_camera_matrices()?;
        Ok(())
    }

    /// Render the current scene (lines/effects) to the surface.
    pub fn render_scene(&mut self, scene: &mut W3DScene) -> Result<()> {
        let device = self.device.as_ref().ok_or(W3DViewError::DeviceNotInitialized)?;
        let queue = self.queue.as_ref().ok_or(W3DViewError::DeviceNotInitialized)?;
        let surface = self.surface.as_ref().ok_or(W3DViewError::DeviceNotInitialized)?;

        let now = NetworkInstant::now();
        self.metrics.frame_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        scene.update(self.metrics.frame_time.as_secs_f32());

        let output = surface
            .get_current_texture()
            .map_err(|err| W3DViewError::SurfaceConfigError(format!("{err:?}")))?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("w3d_view_render_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("w3d_view_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
            });

            if let Some(renderer) = self.line_renderer.as_mut() {
                let mut camera_dir = self.camera.look_at - self.camera.position;
                if camera_dir.magnitude2() > 0.0 {
                    camera_dir = camera_dir.normalize();
                } else {
                    camera_dir = Vector3::new(0.0, 0.0, -1.0);
                }
                renderer.render_lines(
                    device,
                    queue,
                    &mut render_pass,
                    &self.view_projection_matrix,
                    camera_dir,
                    scene,
                    &self.textures,
                )?;
            }
        }

        queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }

    /// Load a texture from disk into the view's texture cache.
    pub fn load_texture_from_file(&mut self, name: &str, path: &Path) -> Result<()> {
        let device = self.device.as_ref().ok_or(W3DViewError::DeviceNotInitialized)?;
        let queue = self.queue.as_ref().ok_or(W3DViewError::DeviceNotInitialized)?;

        let image = ImageReader::open(path)
            .with_context(|| format!("open texture {:?}", path))?
            .decode()
            .with_context(|| format!("decode texture {:?}", path))?;
        let rgba = image.to_rgba8();
        let (width, height) = image.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("texture_{name}")),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.textures.insert(name.to_string(), Arc::new(texture));
        Ok(())
    }

    /// Update camera matrices when camera state changes
    pub fn update_camera_matrices(&mut self) -> Result<()> {
        // Calculate view matrix
        self.view_matrix = Matrix4::look_at_rh(
            self.camera.position,
            self.camera.look_at,
            self.camera.up_vector,
        );
        
        // Calculate projection matrix
        self.projection_matrix = cgmath::perspective(
            self.camera.field_of_view,
            self.camera.aspect_ratio,
            self.camera.near_plane,
            self.camera.far_plane,
        );
        
        // Calculate combined view-projection matrix
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;
        
        // Calculate inverse view matrix for world-space calculations
        self.inverse_view_matrix = self.view_matrix.invert()
            .ok_or(W3DViewError::MatrixError)?;
        
        // Mark frustum as dirty for recalculation
        self.frustum_dirty = true;
        
        // Update uniform buffer if available
        if let (Some(queue), Some(uniform_buffer)) = (&self.queue, &self.uniform_buffer) {
            let matrix_data = bytemuck::cast_slice(&[self.view_projection_matrix]);
            queue.write_buffer(uniform_buffer, 0, matrix_data);
        }
        
        Ok(())
    }

    /// Calculate frustum planes for culling
    pub fn calculate_frustum(&mut self) -> Result<()> {
        if !self.frustum_dirty {
            return Ok();
        }
        
        let view_proj = self.view_projection_matrix;
        
        // Extract frustum planes from view-projection matrix
        // Left plane: row4 + row1
        self.frustum.planes[0] = FrustumPlane {
            normal: Vector3::new(
                view_proj.w.x + view_proj.x.x,
                view_proj.w.y + view_proj.x.y,
                view_proj.w.z + view_proj.x.z,
            ),
            distance: view_proj.w.w + view_proj.x.w,
        };
        
        // Right plane: row4 - row1
        self.frustum.planes[1] = FrustumPlane {
            normal: Vector3::new(
                view_proj.w.x - view_proj.x.x,
                view_proj.w.y - view_proj.x.y,
                view_proj.w.z - view_proj.x.z,
            ),
            distance: view_proj.w.w - view_proj.x.w,
        };
        
        // Bottom plane: row4 + row2
        self.frustum.planes[2] = FrustumPlane {
            normal: Vector3::new(
                view_proj.w.x + view_proj.y.x,
                view_proj.w.y + view_proj.y.y,
                view_proj.w.z + view_proj.y.z,
            ),
            distance: view_proj.w.w + view_proj.y.w,
        };
        
        // Top plane: row4 - row2
        self.frustum.planes[3] = FrustumPlane {
            normal: Vector3::new(
                view_proj.w.x - view_proj.y.x,
                view_proj.w.y - view_proj.y.y,
                view_proj.w.z - view_proj.y.z,
            ),
            distance: view_proj.w.w - view_proj.y.w,
        };
        
        // Near plane: row4 + row3
        self.frustum.planes[4] = FrustumPlane {
            normal: Vector3::new(
                view_proj.w.x + view_proj.z.x,
                view_proj.w.y + view_proj.z.y,
                view_proj.w.z + view_proj.z.z,
            ),
            distance: view_proj.w.w + view_proj.z.w,
        };
        
        // Far plane: row4 - row3
        self.frustum.planes[5] = FrustumPlane {
            normal: Vector3::new(
                view_proj.w.x - view_proj.z.x,
                view_proj.w.y - view_proj.z.y,
                view_proj.w.z - view_proj.z.z,
            ),
            distance: view_proj.w.w - view_proj.z.w,
        };
        
        // Normalize planes
        for plane in &mut self.frustum.planes {
            let length = plane.normal.magnitude();
            if length > 0.0 {
                plane.normal /= length;
                plane.distance /= length;
            }
        }
        
        self.frustum_dirty = false;
        Ok(())
    }

    /// Test if a point is inside the view frustum
    pub fn is_point_in_frustum(&self, point: Point3<f32>) -> bool {
        for plane in &self.frustum.planes {
            let distance = plane.normal.dot(point.to_vec()) + plane.distance;
            if distance < 0.0 {
                return false;
            }
        }
        true
    }

    /// Test if a sphere is inside or intersecting the view frustum
    pub fn is_sphere_in_frustum(&self, center: Point3<f32>, radius: f32) -> bool {
        for plane in &self.frustum.planes {
            let distance = plane.normal.dot(center.to_vec()) + plane.distance;
            if distance < -radius {
                return false;
            }
        }
        true
    }

    /// Pick objects at screen coordinates
    pub fn pick_object(&self, screen_coords: ICoord2D, _pick_type: PickType) -> Result<PickResult> {
        // Convert screen coordinates to normalized device coordinates
        let ndc_x = (2.0 * screen_coords.x as f32) / self.viewport.width as f32 - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_coords.y as f32) / self.viewport.height as f32;
        
        // Create ray from camera through screen point
        let ray_start = self.camera.position;
        let ray_end = {
            let clip_coords = Vector4::new(ndc_x, ndc_y, 1.0, 1.0);
            let view_coords = self.projection_matrix.invert().unwrap() * clip_coords;
            let world_coords = self.inverse_view_matrix * Vector4::new(
                view_coords.x, view_coords.y, -1.0, 0.0
            );
            Point3::new(world_coords.x, world_coords.y, world_coords.z)
        };
        
        let ray_direction = (ray_end - ray_start).normalize();
        
        // TODO: Implement actual object intersection testing
        // This would involve testing the ray against all objects in the scene
        
        Ok(PickResult {
            object_id: None,
            world_position: ray_start + ray_direction * 100.0,
            distance: 100.0,
            surface_normal: Vector3::new(0.0, 1.0, 0.0),
        })
    }

    /// Set camera position
    pub fn set_camera_position(&mut self, position: Point3<f32>) -> Result<()> {
        self.camera.position = position;
        self.update_camera_matrices()
    }

    /// Set camera look-at target
    pub fn set_camera_look_at(&mut self, look_at: Point3<f32>) -> Result<()> {
        self.camera.look_at = look_at;
        self.update_camera_matrices()
    }

    /// Set field of view
    pub fn set_field_of_view(&mut self, fov: Deg<f32>) -> Result<()> {
        self.camera.field_of_view = fov;
        self.update_camera_matrices()
    }

    /// Set viewport size
    pub fn set_viewport_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.viewport.width = width;
        self.viewport.height = height;
        self.camera.aspect_ratio = width as f32 / height as f32;
        
        // Reconfigure surface if available
        if let (Some(surface), Some(device), Some(ref mut config)) = (
            &self.surface,
            &self.device,
            &mut self.surface_config,
        ) {
            config.width = width;
            config.height = height;
            surface.configure(device, config);
        }
        
        self.update_camera_matrices()
    }

    /// Clear render queue
    pub fn clear_render_queue(&mut self) {
        self.render_queue.opaque_objects.clear();
        self.render_queue.transparent_objects.clear();
        self.render_queue.shadow_casters.clear();
        self.render_queue.ui_elements.clear();
    }

    /// Add object to render queue
    pub fn add_object_to_queue(&mut self, object_id: u32, is_transparent: bool, distance: f32) {
        if is_transparent {
            self.render_queue.transparent_objects.push((object_id, distance));
        } else {
            self.render_queue.opaque_objects.push(object_id);
        }
    }

    /// Sort render queue for optimal rendering
    pub fn sort_render_queue(&mut self) {
        // Sort transparent objects back-to-front by distance
        self.render_queue.transparent_objects.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Opaque objects are typically sorted by material/texture to minimize state changes
        // For now, we'll leave them in insertion order
    }

    /// Begin frame rendering
    pub fn begin_frame(&mut self) -> Result<()> {
        let now = NetworkInstant::now();
        self.metrics.frame_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        
        // Reset frame metrics
        self.metrics.render_calls = 0;
        self.metrics.objects_rendered = 0;
        self.metrics.objects_culled = 0;
        self.metrics.triangles_rendered = 0;
        self.metrics.draw_calls = 0;
        self.metrics.texture_switches = 0;
        self.metrics.shader_switches = 0;
        
        // Clear render queue
        self.clear_render_queue();
        
        // Update frustum if needed
        self.calculate_frustum()?;
        
        Ok(())
    }

    /// Update view (called once per frame)
    pub fn update(&mut self, delta_time: Duration) -> Result<()> {
        // Update camera animations
        self.update_camera_animations(delta_time)?;
        
        // Update camera matrices if needed
        if self.frustum_dirty {
            self.update_camera_matrices()?;
        }
        
        Ok(())
    }

    /// Update camera animations and movements
    fn update_camera_animations(&mut self, delta_time: Duration) -> Result<()> {
        let dt_ms = delta_time.as_millis() as u32;
        let mut camera_dirty = false;
        
        // Update waypoint movement
        if let Some(ref mut waypoint_info) = self.waypoint_info {
            waypoint_info.elapsed_time_milliseconds += dt_ms;
            
            if waypoint_info.elapsed_time_milliseconds >= waypoint_info.total_time_milliseconds {
                // Movement complete
                if let Some(last) = waypoint_info.waypoints.last() {
                    self.camera.position = Point3::new(last.position.x, last.position.y, last.position.z);
                    self.camera.look_at = Point3::new(last.look_at.x, last.look_at.y, last.look_at.z);
                    self.camera.yaw = last.camera_angle;
                    camera_dirty = true;
                }
                self.waypoint_info = None;
            } else {
                let waypoint_count = waypoint_info.waypoints.len();
                if waypoint_count >= 2 {
                    let total_time = waypoint_info.total_time_milliseconds as f32;
                    let elapsed = waypoint_info.elapsed_time_milliseconds as f32;
                    let prev_elapsed = elapsed - dt_ms as f32;
                    let ease = ParabolicEase::new(waypoint_info.ease_factor, waypoint_info.ease_factor);
                    let delta_time = ease.apply(elapsed / total_time)
                        - ease.apply((prev_elapsed / total_time).clamp(0.0, 1.0));
                    waypoint_info.cur_seg_distance += delta_time * waypoint_info.total_distance;

                    while waypoint_info.cur_segment < waypoint_info.way_seg_length.len()
                        && waypoint_info.cur_seg_distance >= waypoint_info.way_seg_length[waypoint_info.cur_segment]
                    {
                        waypoint_info.cur_seg_distance -= waypoint_info.way_seg_length[waypoint_info.cur_segment];
                        waypoint_info.cur_segment += 1;
                        if waypoint_info.cur_segment + 1 >= waypoint_count {
                            break;
                        }
                    }

                    if waypoint_info.cur_segment + 1 < waypoint_count {
                        if waypoint_info.cur_shutter > 0 {
                            waypoint_info.cur_shutter -= 1;
                        }
                        if waypoint_info.cur_shutter <= 0 {
                            waypoint_info.cur_shutter = waypoint_info.shutter;

                            let seg_len = waypoint_info.way_seg_length[waypoint_info.cur_segment].max(0.0001);
                            let mut factor = waypoint_info.cur_seg_distance / seg_len;
                            factor = factor.clamp(0.0, 1.0);
                            let mut factor1 = 1.0 - factor;
                            let factor2 = 1.0 - factor1;

                            let mut angle1 = waypoint_info.waypoints[waypoint_info.cur_segment].camera_angle;
                            let mut angle2 = waypoint_info.waypoints[waypoint_info.cur_segment + 1].camera_angle;
                            if angle2 - angle1 > std::f32::consts::PI {
                                angle1 += 2.0 * std::f32::consts::PI;
                            }
                            if angle2 - angle1 < -std::f32::consts::PI {
                                angle1 -= 2.0 * std::f32::consts::PI;
                            }
                            let angle = angle1 * factor1 + angle2 * factor2;
                            self.camera.yaw = norm_angle(angle);

                            let (pos, look) = if waypoint_count >= 3 {
                                let (start, mid, end, adj_factor) = if factor < 0.5 {
                                    let prev_index = waypoint_info.cur_segment.saturating_sub(1);
                                    let start = midpoint(
                                        waypoint_info.waypoints[prev_index].position,
                                        waypoint_info.waypoints[waypoint_info.cur_segment].position,
                                    );
                                    let mid = waypoint_info.waypoints[waypoint_info.cur_segment].position;
                                    let end = midpoint(
                                        waypoint_info.waypoints[waypoint_info.cur_segment].position,
                                        waypoint_info.waypoints[waypoint_info.cur_segment + 1].position,
                                    );
                                    (start, mid, end, factor + 0.5)
                                } else {
                                    let next_index = (waypoint_info.cur_segment + 2).min(waypoint_count - 1);
                                    let start = midpoint(
                                        waypoint_info.waypoints[waypoint_info.cur_segment].position,
                                        waypoint_info.waypoints[waypoint_info.cur_segment + 1].position,
                                    );
                                    let mid = waypoint_info.waypoints[waypoint_info.cur_segment + 1].position;
                                    let end = midpoint(
                                        waypoint_info.waypoints[waypoint_info.cur_segment + 1].position,
                                        waypoint_info.waypoints[next_index].position,
                                    );
                                    (start, mid, end, factor - 0.5)
                                };
                                let pos = quadratic_interpolate(start, mid, end, adj_factor);
                                let look = lerp_coord(
                                    waypoint_info.waypoints[waypoint_info.cur_segment].look_at,
                                    waypoint_info.waypoints[waypoint_info.cur_segment + 1].look_at,
                                    factor,
                                );
                                (pos, look)
                            } else {
                                let pos = lerp_coord(
                                    waypoint_info.waypoints[waypoint_info.cur_segment].position,
                                    waypoint_info.waypoints[waypoint_info.cur_segment + 1].position,
                                    factor,
                                );
                                let look = lerp_coord(
                                    waypoint_info.waypoints[waypoint_info.cur_segment].look_at,
                                    waypoint_info.waypoints[waypoint_info.cur_segment + 1].look_at,
                                    factor,
                                );
                                (pos, look)
                            };

                            self.camera.position = Point3::new(pos.x, pos.y, pos.z);
                            self.camera.look_at = Point3::new(look.x, look.y, look.z);
                            camera_dirty = true;
                        }
                    }
                }
            }
        }
        
        // Update rotation animation
        if let Some(ref mut rotation_info) = self.rotation_info {
            rotation_info.cur_frame += 1;
            
            if rotation_info.cur_frame >= rotation_info.num_frames {
                // Rotation complete
                self.camera.yaw = rotation_info.end_angle;
                self.apply_camera_orbit();
                camera_dirty = true;
                self.rotation_info = None;
            } else {
                let denom = rotation_info.num_frames.max(1) as f32;
                let t = (rotation_info.cur_frame as f32 / denom).clamp(0.0, 1.0);
                let ease = ParabolicEase::new(rotation_info.ease_factor, rotation_info.ease_factor);
                let factor = ease.apply(t);
                self.camera.yaw = lerp_angle(rotation_info.start_angle, rotation_info.end_angle, factor);
                self.apply_camera_orbit();
                camera_dirty = true;
            }
        }
        
        // Update pitch animation
        if let Some(ref mut pitch_info) = self.pitch_info {
            pitch_info.cur_frame += 1;
            
            if pitch_info.cur_frame >= pitch_info.num_frames {
                // Pitch complete
                self.camera.pitch = pitch_info.end_pitch;
                self.apply_camera_orbit();
                camera_dirty = true;
                self.pitch_info = None;
            } else {
                let denom = pitch_info.num_frames.max(1) as f32;
                let t = (pitch_info.cur_frame as f32 / denom).clamp(0.0, 1.0);
                let ease = ParabolicEase::new(pitch_info.ease_factor, pitch_info.ease_factor);
                let factor = ease.apply(t);
                self.camera.pitch = lerp(pitch_info.start_pitch, pitch_info.end_pitch, factor);
                self.apply_camera_orbit();
                camera_dirty = true;
            }
        }
        
        // Update zoom animation
        if let Some(ref mut zoom_info) = self.zoom_info {
            zoom_info.cur_frame += 1;
            
            if zoom_info.cur_frame >= zoom_info.num_frames {
                // Zoom complete
                self.camera.zoom_factor = zoom_info.end_zoom;
                self.apply_camera_orbit();
                camera_dirty = true;
                self.zoom_info = None;
            } else {
                let denom = zoom_info.num_frames.max(1) as f32;
                let t = (zoom_info.cur_frame as f32 / denom).clamp(0.0, 1.0);
                let ease = ParabolicEase::new(zoom_info.ease_factor, zoom_info.ease_factor);
                let factor = ease.apply(t);
                self.camera.zoom_factor = lerp(zoom_info.start_zoom, zoom_info.end_zoom, factor);
                self.apply_camera_orbit();
                camera_dirty = true;
            }
        }

        if camera_dirty {
            self.update_camera_matrices()?;
        }
        
        Ok(())
    }

    fn apply_camera_orbit(&mut self) {
        let look_at = self.camera.look_at;
        let offset = self.camera.position - look_at;
        let mut base_distance = offset.magnitude();
        if base_distance <= 0.0001 {
            base_distance = 1.0;
        }
        let zoom = if self.camera.zoom_factor.abs() < 0.0001 {
            1.0
        } else {
            self.camera.zoom_factor
        };
        let distance = (base_distance / zoom).max(0.1) * zoom;

        let (sy, cy) = self.camera.yaw.sin_cos();
        let (sp, cp) = self.camera.pitch.sin_cos();
        let dir = Vector3::new(cp * cy, cp * sy, sp);
        let new_pos = Point3::new(
            look_at.x + dir.x * distance,
            look_at.y + dir.y * distance,
            look_at.z + dir.z * distance,
        );
        self.camera.position = new_pos;
    }

    /// Check if view is initialized and ready for rendering
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Enable or disable wireframe rendering mode
    pub fn set_wireframe_mode(&mut self, enabled: bool) {
        self.wireframe_mode = enabled;
        // TODO: Recreate render pipeline with wireframe mode
    }

    /// Enable or disable debug rendering
    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> &ViewMetrics {
        &self.metrics
    }

    /// Get current camera state
    pub fn get_camera_state(&self) -> &CameraState {
        &self.camera
    }

    /// Get current viewport configuration
    pub fn get_viewport(&self) -> &ViewportConfig {
        &self.viewport
    }

    /// Reset view to default state
    pub fn reset(&mut self) {
        let _lock = self.state_lock.write();
        
        self.camera = CameraState::default();
        self.viewport = ViewportConfig::default();
        self.waypoint_info = None;
        self.rotation_info = None;
        self.pitch_info = None;
        self.zoom_info = None;
        self.clear_render_queue();
        self.frustum_dirty = true;
        self.needs_redraw = true;
        
        if self.initialized {
            let _ = self.update_camera_matrices();
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut delta = b - a;
    if delta > std::f32::consts::PI {
        delta -= 2.0 * std::f32::consts::PI;
    } else if delta < -std::f32::consts::PI {
        delta += 2.0 * std::f32::consts::PI;
    }
    norm_angle(a + delta * t)
}

fn norm_angle(mut angle: f32) -> f32 {
    while angle > std::f32::consts::PI {
        angle -= 2.0 * std::f32::consts::PI;
    }
    while angle < -std::f32::consts::PI {
        angle += 2.0 * std::f32::consts::PI;
    }
    angle
}

fn lerp_coord(a: Coord3D, b: Coord3D, t: f32) -> Coord3D {
    Coord3D::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t))
}

fn midpoint(a: Coord3D, b: Coord3D) -> Coord3D {
    Coord3D::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5)
}

fn quadratic_interpolate(start: Coord3D, mid: Coord3D, end: Coord3D, t: f32) -> Coord3D {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    let mut result = Coord3D::new(
        start.x + t * (end.x - start.x),
        start.y + t * (end.y - start.y),
        start.z + t * (end.z - start.z),
    );
    result.x += inv * t * (mid.x - end.x + mid.x - start.x);
    result.y += inv * t * (mid.y - end.y + mid.y - start.y);
    result.z += inv * t * (mid.z - end.z + mid.z - start.z);
    result
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct LineVertex {
    position: [f32; 3],
    color: [f32; 4],
    uv: [f32; 2],
}

impl LineVertex {
    fn layout<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
                VertexAttribute {
                    offset: 28,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct LineUniform {
    view_proj: [[f32; 4]; 4],
}

struct LineTextureBinding {
    _view: TextureView,
    bind_group: BindGroup,
}

struct LineRenderer {
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    uniform_bind_group: BindGroup,
    texture_bind_group_layout: BindGroupLayout,
    sampler: Sampler,
    white_texture: Texture,
    white_texture_view: TextureView,
    texture_bindings: HashMap<String, LineTextureBinding>,
    vertex_buffer: Buffer,
    vertex_capacity: usize,
}

impl std::fmt::Debug for LineRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineRenderer").finish()
    }
}

impl LineRenderer {
    fn new(device: &Device, queue: &Queue, target_format: wgpu::TextureFormat) -> Result<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("segmented_line_shader"),
            source: ShaderSource::Wgsl(include_str!("wthree_d_segmented_line.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("segmented_line_uniforms"),
            size: std::mem::size_of::<LineUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("segmented_line_uniform_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("segmented_line_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("segmented_line_texture_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("segmented_line_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("segmented_line_white_texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let white_texture_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("segmented_line_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("segmented_line_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[LineVertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: target_format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("segmented_line_vertex_buffer"),
            size: 1024,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
            sampler,
            white_texture,
            white_texture_view,
            texture_bindings: HashMap::new(),
            vertex_buffer,
            vertex_capacity: 1024,
        })
    }

    fn render_lines(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        view_proj: &Matrix4<f32>,
        camera_dir: Vector3<f32>,
        scene: &W3DScene,
        textures: &HashMap<String, Arc<Texture>>,
    ) -> Result<()> {
        let uniform = LineUniform {
            view_proj: (*view_proj).into(),
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for line in scene.iter_segmented_lines() {
            let guard = line.read();
            if !guard.is_visible() || guard.get_num_points() < 2 {
                continue;
            }

            let vertices = build_line_vertices(&guard, camera_dir);
            if vertices.is_empty() {
                continue;
            }

            let required_size = (vertices.len() * std::mem::size_of::<LineVertex>()) as u64;
            if required_size > self.vertex_capacity as u64 {
                self.vertex_capacity = required_size.next_power_of_two() as usize;
                self.vertex_buffer = device.create_buffer(&BufferDescriptor {
                    label: Some("segmented_line_vertex_buffer"),
                    size: self.vertex_capacity as u64,
                    usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            let bind_group = self.get_texture_binding(device, textures, guard.get_texture_name());
            render_pass.set_bind_group(1, bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..required_size));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        Ok(())
    }

    fn get_texture_binding(
        &mut self,
        device: &Device,
        textures: &HashMap<String, Arc<Texture>>,
        name: Option<&str>,
    ) -> &BindGroup {
        let key = name.unwrap_or("__white__");
        if let Some(binding) = self.texture_bindings.get(key) {
            return &binding.bind_group;
        }

        let (view, label) = if let Some(name) = name {
            if let Some(texture) = textures.get(name) {
                (texture.create_view(&wgpu::TextureViewDescriptor::default()), name.to_string())
            } else {
                (self.white_texture_view.clone(), "__white__".to_string())
            }
        } else {
            (self.white_texture_view.clone(), "__white__".to_string())
        };

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("segmented_line_texture_bind_group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.texture_bindings.insert(
            label.clone(),
            LineTextureBinding {
                _view: view,
                bind_group,
            },
        );

        &self.texture_bindings.get(&label).unwrap().bind_group
    }
}

fn build_line_vertices(line: &SegmentedLine, camera_dir: Vector3<f32>) -> Vec<LineVertex> {
    let points = line.get_points();
    if points.len() < 2 {
        return Vec::new();
    }

    let lengths = line.get_segment_lengths();
    let total_length: f32 = lengths.iter().sum();
    let tile_factor = match line.get_texture_mapping_mode() {
        TextureMapMode::Stretch => 1.0,
        TextureMapMode::Tiled => line.get_texture_tile_factor().max(0.0),
    };
    let uv_offset = line.get_uv_offset();

    let mut vertices = Vec::new();
    let mut length_accum = 0.0;
    let color = line.get_color();
    let opacity = line.get_opacity();
    let color_rgba = [color.x * opacity, color.y * opacity, color.z * opacity, opacity];
    let half_width = line.get_width() * 0.5;

    for (idx, segment_len) in lengths.iter().enumerate() {
        let start = points[idx];
        let end = points[idx + 1];
        let dir = (end - start).normalize();
        let perp = compute_line_perp(dir, camera_dir) * half_width;

        let u0 = 0.0;
        let u1 = 1.0;
        let v0 = if total_length > 0.0 {
            (length_accum / total_length) * tile_factor + uv_offset
        } else {
            uv_offset
        };
        let v1 = if total_length > 0.0 {
            ((length_accum + segment_len) / total_length) * tile_factor + uv_offset
        } else {
            uv_offset + tile_factor
        };

        let p0 = start - perp;
        let p1 = start + perp;
        let p2 = end - perp;
        let p3 = end + perp;

        vertices.push(LineVertex {
            position: [p0.x, p0.y, p0.z],
            color: color_rgba,
            uv: [u0, v0],
        });
        vertices.push(LineVertex {
            position: [p1.x, p1.y, p1.z],
            color: color_rgba,
            uv: [u1, v0],
        });
        vertices.push(LineVertex {
            position: [p2.x, p2.y, p2.z],
            color: color_rgba,
            uv: [u0, v1],
        });

        vertices.push(LineVertex {
            position: [p2.x, p2.y, p2.z],
            color: color_rgba,
            uv: [u0, v1],
        });
        vertices.push(LineVertex {
            position: [p1.x, p1.y, p1.z],
            color: color_rgba,
            uv: [u1, v0],
        });
        vertices.push(LineVertex {
            position: [p3.x, p3.y, p3.z],
            color: color_rgba,
            uv: [u1, v1],
        });

        length_accum += segment_len;
    }

    vertices
}

// Thread-safe implementation
unsafe impl Send for W3DView {}
unsafe impl Sync for W3DView {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_w3d_view_creation() {
        let view = W3DView::new();
        assert!(!view.is_initialized());
        assert_eq!(view.camera.position, Point3::new(0.0, 100.0, 100.0));
        assert_eq!(view.camera.look_at, Point3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_frustum_culling_calculations() {
        let view = W3DView::new();
        
        // Test point culling
        let test_point = Point3::new(0.0, 0.0, 0.0);
        // Note: frustum won't be valid until matrices are calculated
        // This is just testing the API
        let _result = view.is_point_in_frustum(test_point);
    }

    #[test]
    fn test_camera_matrix_calculations() {
        let mut view = W3DView::new();
        let result = view.update_camera_matrices();
        
        // Should succeed even without graphics device (for matrix math)
        assert!(result.is_ok());
        assert_ne!(view.view_matrix, Matrix4::identity());
        assert_ne!(view.projection_matrix, Matrix4::identity());
    }

    #[test]
    fn test_viewport_management() {
        let mut view = W3DView::new();
        
        let result = view.set_viewport_size(1920, 1080);
        assert!(result.is_ok());
        assert_eq!(view.viewport.width, 1920);
        assert_eq!(view.viewport.height, 1080);
        assert_eq!(view.camera.aspect_ratio, 1920.0 / 1080.0);
    }

    #[test]
    fn test_render_queue_management() {
        let mut view = W3DView::new();
        
        view.add_object_to_queue(1, false, 0.0);
        view.add_object_to_queue(2, true, 50.0);
        view.add_object_to_queue(3, true, 25.0);
        
        assert_eq!(view.render_queue.opaque_objects.len(), 1);
        assert_eq!(view.render_queue.transparent_objects.len(), 2);
        
        view.sort_render_queue();
        
        // Transparent objects should be sorted back-to-front
        assert_eq!(view.render_queue.transparent_objects[0].0, 2); // Furthest first
        assert_eq!(view.render_queue.transparent_objects[1].0, 3); // Closer second
    }

    #[test]
    fn test_camera_controls() {
        let mut view = W3DView::new();
        
        let new_position = Point3::new(100.0, 200.0, 300.0);
        let result = view.set_camera_position(new_position);
        assert!(result.is_ok());
        assert_eq!(view.camera.position, new_position);
        
        let new_look_at = Point3::new(50.0, 0.0, 50.0);
        let result = view.set_camera_look_at(new_look_at);
        assert!(result.is_ok());
        assert_eq!(view.camera.look_at, new_look_at);
        
        let new_fov = Deg(75.0);
        let result = view.set_field_of_view(new_fov);
        assert!(result.is_ok());
        assert_eq!(view.camera.field_of_view, new_fov);
    }
}
