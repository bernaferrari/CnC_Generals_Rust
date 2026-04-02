//! # W3D Performance Optimizer - Advanced GPU Performance Monitoring
//!
//! This module implements comprehensive performance monitoring and optimization features:
//! - Real-time GPU performance profiling with wgpu-profiler
//! - Frame time analysis and bottleneck detection
//! - Memory usage tracking and optimization
//! - Draw call batching and instancing optimization
//! - LOD system with automatic quality adjustment
//! - GPU culling system for efficient rendering
//! - Dynamic resolution scaling
//! - Performance-based quality settings adjustment

use super::renderer::{InstanceData, RenderBatch};
use super::w3d_device::RenderObject;
use super::{BoundingBox, Camera, Result, W3DError};
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[cfg(feature = "w3d")]
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, Buffer, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor,
    Device, MapMode, PipelineLayoutDescriptor, PollType, Queue,
};

#[cfg(feature = "w3d")]
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};

/// Number of frames to keep in performance history
const PERFORMANCE_HISTORY_SIZE: usize = 120;
/// GPU memory tracking update interval (frames)
const MEMORY_UPDATE_INTERVAL: u64 = 60;
/// Performance sampling interval for automatic quality adjustment
const QUALITY_ADJUST_INTERVAL: u64 = 300; // 5 seconds at 60fps

/// Performance optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct W3DOptimizationSettings {
    /// Enable automatic quality adjustment
    pub auto_quality_adjustment: bool,
    /// Target frame rate for quality adjustment
    pub target_frame_rate: f32,
    /// Frame rate tolerance before adjusting quality
    pub frame_rate_tolerance: f32,
    /// Enable dynamic resolution scaling
    pub dynamic_resolution: bool,
    /// Minimum resolution scale factor
    pub min_resolution_scale: f32,
    /// Maximum resolution scale factor
    pub max_resolution_scale: f32,
    /// Enable GPU culling
    pub gpu_culling: bool,
    /// Culling distance
    pub culling_distance: f32,
    /// Enable LOD system
    pub lod_system: bool,
    /// LOD bias adjustment
    pub lod_bias: f32,
    /// Enable draw call batching
    pub draw_call_batching: bool,
    /// Maximum batch size
    pub max_batch_size: u32,
    /// Enable instanced rendering
    pub instanced_rendering: bool,
    /// Memory pressure threshold (bytes)
    pub memory_pressure_threshold: u64,
    /// Enable performance profiling
    pub profiling_enabled: bool,
}

impl Default for W3DOptimizationSettings {
    fn default() -> Self {
        Self {
            auto_quality_adjustment: true,
            // C++ uses LOGIC_FRAMES_PER_SECOND (30), not 60 for GPU
            // (which is internal adaptive quality)
            target_frame_rate: 30.0,
            frame_rate_tolerance: 5.0,
            dynamic_resolution: true,
            min_resolution_scale: 0.5,
            max_resolution_scale: 1.0,
            gpu_culling: true,
            culling_distance: 1000.0,
            lod_system: true,
            lod_bias: 0.0,
            draw_call_batching: true,
            max_batch_size: 1000,
            instanced_rendering: true,
            memory_pressure_threshold: 2 * 1024 * 1024 * 1024, // 2GB
            profiling_enabled: true,
        }
    }
}

/// GPU performance metrics
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuPerformanceMetrics {
    /// Frame time on GPU (microseconds)
    pub gpu_frame_time_us: u64,
    /// Geometry pass time
    pub geometry_time_us: u64,
    /// Lighting pass time
    pub lighting_time_us: u64,
    /// Shadow mapping time
    pub shadow_time_us: u64,
    /// Post-processing time
    pub post_process_time_us: u64,
    /// GPU memory usage (bytes)
    pub gpu_memory_used: u64,
    /// GPU memory available (bytes)
    pub gpu_memory_available: u64,
    /// GPU utilization percentage (0-100)
    pub gpu_utilization: f32,
    /// GPU temperature (Celsius)
    pub gpu_temperature: f32,
}

/// Advanced performance optimizer and profiler
pub struct W3DPerformanceOptimizer {
    /// GPU device
    #[cfg(feature = "w3d")]
    device: Arc<Device>,
    /// GPU queue
    #[cfg(feature = "w3d")]
    queue: Arc<Queue>,

    /// Configuration
    settings: Arc<RwLock<W3DOptimizationSettings>>,

    /// GPU profiler
    #[cfg(feature = "w3d")]
    gpu_profiler: Arc<Mutex<GpuProfiler>>,

    /// Performance history
    frame_history: Arc<Mutex<VecDeque<FrameTimingStats>>>,

    /// Memory statistics
    memory_stats: Arc<RwLock<MemoryStatistics>>,

    /// Frame counters
    frame_count: Arc<Mutex<u64>>,
    /// Batch optimizer
    batch_optimizer: BatchOptimizer,
    /// GPU culling subsystem
    gpu_culler: GpuCuller,
    /// Instance generation subsystem
    instance_manager: InstanceManager,
    /// LOD selection subsystem
    lod_manager: LodManager,
    /// Live optimization stats
    stats: Arc<RwLock<PerformanceStats>>,
    /// Batch sorting enabled
    sort_batches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QualityAdjustment {
    Increase,
    Decrease,
    None,
}

/// Key for batching render calls
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct BatchKey {
    /// Material ID (None for default material)
    material_id: Option<String>,
    /// Mesh ID
    mesh_id: String,
    /// Shader variant
    shader_variant: u32,
    /// Transparency pass classification
    transparent: bool,
    /// Specialized renderer priority
    priority: u32,
}

/// GPU-based culling system
pub struct GpuCuller {
    /// Culling compute pipeline
    cull_pipeline: Option<ComputePipeline>,
    /// Object data buffer
    object_buffer: Option<Buffer>,
    /// Visibility results buffer
    visibility_buffer: Option<Buffer>,
    /// Camera frustum buffer
    frustum_buffer: Option<Buffer>,
    /// Occlusion query support
    occlusion_queries: bool,
    /// Maximum objects per cull
    max_objects: u32,
}

/// Instancing system for rendering multiple objects efficiently
pub struct InstanceManager {
    /// Instance data by mesh+material combination
    instance_data: HashMap<String, Vec<InstanceData>>,
    /// Instance buffers
    instance_buffers: HashMap<String, Buffer>,
    /// Maximum instances per draw call
    max_instances_per_draw: u32,
    /// Dynamic instancing enabled
    dynamic_instancing: bool,
}

/// Level of Detail management system
pub struct LodManager {
    /// LOD configurations by object type
    lod_configs: HashMap<String, LodConfig>,
    /// Distance-based LOD selection
    distance_lod: bool,
    /// Screen-space LOD selection
    screen_space_lod: bool,
    /// LOD bias
    global_lod_bias: f32,
}

/// LOD configuration for an object type
#[derive(Debug, Clone)]
pub struct LodConfig {
    /// LOD levels with distance thresholds
    pub levels: Vec<LodLevel>,
    /// Enable morphing between LOD levels
    pub morphing_enabled: bool,
    /// Hysteresis factor to prevent LOD popping
    pub hysteresis: f32,
}

/// Individual LOD level
#[derive(Debug, Clone)]
pub struct LodLevel {
    /// Distance threshold for this LOD
    pub distance: f32,
    /// Mesh ID for this LOD
    pub mesh_id: String,
    /// Quality multiplier (0.0 - 1.0)
    pub quality: f32,
}

/// Culling data for GPU compute shader
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CullData {
    /// World-space bounding box min
    pub bounds_min: [f32; 3],
    pub _padding1: f32,
    /// World-space bounding box max
    pub bounds_max: [f32; 3],
    pub _padding2: f32,
    /// Object transform matrix
    pub transform: [[f32; 4]; 4],
}

/// Frustum data for GPU culling
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct FrustumData {
    /// Six frustum planes (normal.xyz, distance)
    pub planes: [[f32; 4]; 6],
    /// Camera position
    pub camera_position: [f32; 3],
    pub _padding: f32,
}

/// Performance statistics
#[derive(Debug, Default, Clone)]
pub struct PerformanceStats {
    /// Original draw calls before batching
    pub original_draw_calls: u32,
    /// Final draw calls after batching
    pub final_draw_calls: u32,
    /// Batch reduction ratio
    pub batch_reduction_ratio: f32,
    /// Objects submitted for culling
    pub objects_submitted: u32,
    /// Objects culled by frustum
    pub objects_frustum_culled: u32,
    /// Objects culled by occlusion
    pub objects_occlusion_culled: u32,
    /// Objects rendered
    pub objects_rendered: u32,
    /// Cull ratio
    pub cull_ratio: f32,
    /// Instances rendered
    pub instances_rendered: u32,
    /// Instance ratio
    pub instance_ratio: f32,
    /// LOD transitions
    pub lod_transitions: u32,
    /// Average LOD level used
    pub average_lod_level: f32,
    /// GPU culling time (ms)
    pub gpu_cull_time_ms: f32,
    /// Batching time (ms)
    pub batch_time_ms: f32,
    /// Total optimization time (ms)
    pub total_optimization_time_ms: f32,
}

#[derive(Debug, Clone, Default)]
struct FrameTimingStats {
    frame_time_ms: f32,
    gpu_time_ms: f32,
    draw_calls: u32,
    triangle_count: u32,
}

#[derive(Debug, Clone)]
struct MemoryStatistics {
    used_bytes: u64,
    budget_bytes: u64,
    allocations: u32,
}

impl Default for MemoryStatistics {
    fn default() -> Self {
        Self {
            used_bytes: 0,
            budget_bytes: 2 * 1024 * 1024 * 1024,
            allocations: 0,
        }
    }
}

#[derive(Debug)]
struct BatchOptimizer {
    batched_calls: HashMap<BatchKey, Vec<RenderBatch>>,
    max_batch_size: u32,
    sort_batches: bool,
}

impl W3DPerformanceOptimizer {
    /// Create new performance optimizer
    pub async fn new(device: Arc<Device>, queue: Arc<Queue>) -> Result<Self> {
        tracing::info!("Creating W3D performance optimizer");

        let batch_optimizer = BatchOptimizer {
            batched_calls: HashMap::new(),
            max_batch_size: 1000,
            sort_batches: true,
        };

        let gpu_culler = GpuCuller::new(&device).await?;

        let instance_manager = InstanceManager {
            instance_data: HashMap::new(),
            instance_buffers: HashMap::new(),
            max_instances_per_draw: 1000,
            dynamic_instancing: true,
        };

        let lod_manager = LodManager {
            lod_configs: HashMap::new(),
            distance_lod: true,
            screen_space_lod: true,
            global_lod_bias: 0.0,
        };

        let settings = Arc::new(RwLock::new(W3DOptimizationSettings::default()));
        let frame_history = Arc::new(Mutex::new(VecDeque::with_capacity(
            PERFORMANCE_HISTORY_SIZE,
        )));
        let memory_stats = Arc::new(RwLock::new(MemoryStatistics::default()));
        let frame_count = Arc::new(Mutex::new(0_u64));
        let stats = Arc::new(RwLock::new(PerformanceStats::default()));
        let sort_batches = true;

        let gpu_profiler = GpuProfiler::new(device.as_ref(), GpuProfilerSettings::default())
            .map_err(|e| {
                W3DError::InitializationFailed(format!("Failed to initialize GPU profiler: {e:?}"))
            })?;
        let gpu_profiler = Arc::new(Mutex::new(gpu_profiler));

        Ok(Self {
            device,
            queue,
            settings,
            gpu_profiler,
            frame_history,
            memory_stats,
            frame_count,
            batch_optimizer,
            gpu_culler,
            instance_manager,
            lod_manager,
            stats,
            sort_batches,
        })
    }

    /// Optimize render batches for maximum performance
    pub async fn optimize_batches(
        &mut self,
        render_objects: &[RenderObject],
        camera: &Camera,
    ) -> Result<Vec<RenderBatch>> {
        let start_time = std::time::Instant::now();
        let original_count = render_objects.len();
        let settings = self.settings.read().clone();
        let (avg_lod_level, lod_transitions) =
            self.compute_lod_metrics(render_objects, camera, settings.lod_system);

        self.batch_optimizer.max_batch_size = settings.max_batch_size.max(1);
        self.batch_optimizer.sort_batches = self.sort_batches && settings.draw_call_batching;
        self.instance_manager.dynamic_instancing = settings.instanced_rendering;
        self.lod_manager.global_lod_bias = settings.lod_bias;

        // Step 1: Perform LOD selection
        let lod_objects = if settings.lod_system {
            self.lod_manager
                .select_lod_levels(render_objects, camera)
                .await?
        } else {
            render_objects.to_vec()
        };

        // Step 2: Frustum culling (CPU-side pre-cull)
        let frustum_visible = self.frustum_cull_cpu(&lod_objects, camera).await?;
        let distance_visible =
            self.distance_cull_cpu(&frustum_visible, camera, settings.culling_distance);

        // Step 3: GPU culling for remaining objects
        let gpu_cull_start = std::time::Instant::now();
        let gpu_visible = if settings.gpu_culling {
            self.gpu_culler
                .cull_objects(
                    &distance_visible,
                    camera,
                    self.device.as_ref(),
                    self.queue.as_ref(),
                )
                .await?
        } else {
            distance_visible.clone()
        };
        let gpu_cull_time_ms = gpu_cull_start.elapsed().as_secs_f32() * 1000.0;

        // Step 4: Instance detection and batching
        let batch_stage_start = std::time::Instant::now();
        let instanced_batches = self
            .instance_manager
            .create_instances(&gpu_visible, camera, self.device.as_ref())
            .await?;

        // Step 5: Sort and optimize batches
        let optimized_batches = if settings.draw_call_batching {
            self.batch_optimizer
                .optimize_batches(instanced_batches)
                .await?
        } else {
            let mut unbatched = instanced_batches;
            if self.sort_batches {
                sort_render_batches(&mut unbatched);
            }
            unbatched
        };
        let batch_time_ms = batch_stage_start.elapsed().as_secs_f32() * 1000.0;

        // Update statistics
        let optimization_time = start_time.elapsed().as_secs_f32() * 1000.0;
        let frustum_culled = lod_objects.len().saturating_sub(frustum_visible.len());
        let distance_culled = frustum_visible.len().saturating_sub(distance_visible.len());
        let gpu_culled = distance_visible.len().saturating_sub(gpu_visible.len());
        let instances_rendered: u32 = optimized_batches
            .iter()
            .map(|batch| batch.instances.len() as u32)
            .sum();

        {
            let mut stats = self.stats.write();
            stats.objects_submitted = original_count as u32;
            stats.original_draw_calls = original_count as u32;
            stats.final_draw_calls = optimized_batches.len() as u32;
            stats.batch_reduction_ratio = if original_count > 0 {
                1.0 - (optimized_batches.len() as f32 / original_count as f32)
            } else {
                0.0
            };
            stats.objects_frustum_culled = (frustum_culled + distance_culled) as u32;
            stats.objects_occlusion_culled = gpu_culled as u32;
            stats.objects_rendered = gpu_visible.len() as u32;
            stats.cull_ratio = if original_count > 0 {
                1.0 - (gpu_visible.len() as f32 / original_count as f32)
            } else {
                0.0
            };
            stats.instances_rendered = instances_rendered;
            stats.instance_ratio = if gpu_visible.is_empty() {
                0.0
            } else {
                instances_rendered as f32 / gpu_visible.len() as f32
            };
            stats.lod_transitions = lod_transitions;
            stats.average_lod_level = avg_lod_level;
            stats.gpu_cull_time_ms = gpu_cull_time_ms;
            stats.batch_time_ms = batch_time_ms;
            stats.total_optimization_time_ms = optimization_time;
        }

        let triangle_count = estimated_triangle_count(&optimized_batches);
        let estimated_batch_memory = estimated_batch_memory_bytes(&optimized_batches);
        self.record_frame_timing(
            optimization_time,
            gpu_cull_time_ms,
            optimized_batches.len() as u32,
            triangle_count,
            estimated_batch_memory,
        );

        let batch_reduction_ratio = if original_count > 0 {
            1.0 - (optimized_batches.len() as f32 / original_count as f32)
        } else {
            0.0
        };
        tracing::debug!(
            "Optimized {} objects -> {} batches in {:.2}ms ({}% reduction)",
            original_count,
            optimized_batches.len(),
            optimization_time,
            (batch_reduction_ratio * 100.0) as i32
        );

        Ok(optimized_batches)
    }

    fn compute_lod_metrics(
        &self,
        render_objects: &[RenderObject],
        camera: &Camera,
        lod_enabled: bool,
    ) -> (f32, u32) {
        if render_objects.is_empty() || !lod_enabled {
            return (0.0, 0);
        }

        let camera_pos = Vec3::from_slice(&camera.position);
        let mut lod_sum = 0.0f32;
        let mut transitions = 0u32;
        for obj in render_objects {
            let distance = (Vec3::from_slice(&obj.world_bounds.center()) - camera_pos).length();
            let lod_level = self.lod_manager.calculate_lod_level(distance, &obj.mesh_id);
            lod_sum += lod_level as f32;
            if lod_level > 0 {
                transitions = transitions.saturating_add(1);
            }
        }

        (lod_sum / render_objects.len() as f32, transitions)
    }

    fn record_frame_timing(
        &mut self,
        frame_time_ms: f32,
        gpu_time_ms: f32,
        draw_calls: u32,
        triangle_count: u32,
        estimated_batch_memory: u64,
    ) {
        let frame_number = {
            let mut frame = self.frame_count.lock();
            *frame = frame.saturating_add(1);
            *frame
        };

        {
            let mut history = self.frame_history.lock();
            if history.len() == PERFORMANCE_HISTORY_SIZE {
                history.pop_front();
            }
            history.push_back(FrameTimingStats {
                frame_time_ms,
                gpu_time_ms,
                draw_calls,
                triangle_count,
            });
        }

        if frame_number % MEMORY_UPDATE_INTERVAL == 0 {
            self.refresh_memory_stats(draw_calls, estimated_batch_memory);
        }
        if frame_number % QUALITY_ADJUST_INTERVAL == 0 {
            self.auto_adjust_quality();
        }
    }

    fn refresh_memory_stats(&mut self, draw_calls: u32, estimated_batch_memory: u64) {
        let mut stats = self.memory_stats.write();
        let gpu_culler_memory = self
            .gpu_culler
            .approximate_memory_bytes()
            .saturating_add(self.instance_manager.approximate_memory_bytes());

        stats.used_bytes = estimated_batch_memory.saturating_add(gpu_culler_memory);
        stats.allocations = draw_calls;
    }

    fn auto_adjust_quality(&mut self) {
        let history = self.frame_history.lock();
        let Some(avg_frame_time_ms) = average_frame_time_ms(&history) else {
            return;
        };
        drop(history);

        if !avg_frame_time_ms.is_finite() || avg_frame_time_ms <= 0.0 {
            return;
        }

        let avg_fps = 1000.0 / avg_frame_time_ms;

        let mut settings = self.settings.write();
        if !settings.auto_quality_adjustment || settings.target_frame_rate <= 0.0 {
            return;
        }
        if settings.max_resolution_scale < settings.min_resolution_scale {
            let previous_max = settings.max_resolution_scale;
            settings.max_resolution_scale = settings.min_resolution_scale;
            settings.min_resolution_scale = previous_max;
        }

        let memory_pressure = {
            let memory = self.memory_stats.read();
            memory.used_bytes > settings.memory_pressure_threshold
        };

        match quality_adjustment_direction(
            avg_fps,
            settings.target_frame_rate,
            settings.frame_rate_tolerance,
            memory_pressure,
        ) {
            QualityAdjustment::Decrease => {
                settings.lod_bias = (settings.lod_bias + 0.1).clamp(-1.0, 2.5);
                settings.culling_distance = (settings.culling_distance * 0.95).clamp(200.0, 5000.0);
                settings.max_batch_size = settings.max_batch_size.saturating_add(64).max(128);
                if settings.dynamic_resolution {
                    settings.max_resolution_scale = (settings.max_resolution_scale - 0.05)
                        .clamp(settings.min_resolution_scale, 1.0);
                }
            }
            QualityAdjustment::Increase => {
                settings.lod_bias = (settings.lod_bias - 0.05).clamp(-1.0, 2.5);
                settings.culling_distance = (settings.culling_distance * 1.05).clamp(200.0, 5000.0);
                settings.max_batch_size = settings.max_batch_size.saturating_sub(32).max(128);
                if settings.dynamic_resolution {
                    settings.max_resolution_scale = (settings.max_resolution_scale + 0.02)
                        .clamp(settings.min_resolution_scale, 1.0);
                }
            }
            QualityAdjustment::None => {}
        }
    }

    /// CPU-based frustum culling as pre-pass
    async fn frustum_cull_cpu(
        &self,
        objects: &[RenderObject],
        camera: &Camera,
    ) -> Result<Vec<RenderObject>> {
        // Extract frustum planes from camera matrices
        let view_proj = Mat4::from_cols_array_2d(&camera.projection_matrix)
            * Mat4::from_cols_array_2d(&camera.view_matrix);
        let frustum = extract_frustum_planes(view_proj);

        // Parallel frustum culling
        let visible_objects: Vec<RenderObject> = objects
            .par_iter()
            .filter(|obj| is_aabb_in_frustum(&obj.world_bounds, &frustum))
            .cloned()
            .collect();

        tracing::trace!(
            "CPU frustum culling: {} -> {} objects",
            objects.len(),
            visible_objects.len()
        );

        Ok(visible_objects)
    }

    fn distance_cull_cpu(
        &self,
        objects: &[RenderObject],
        camera: &Camera,
        culling_distance: f32,
    ) -> Vec<RenderObject> {
        if !culling_distance.is_finite() || culling_distance <= 0.0 {
            return objects.to_vec();
        }

        let max_distance_sq = culling_distance * culling_distance;
        let camera_pos = Vec3::from_slice(&camera.position);
        objects
            .iter()
            .filter(|obj| {
                let center = Vec3::from_slice(&obj.world_bounds.center());
                (center - camera_pos).length_squared() <= max_distance_sq
            })
            .cloned()
            .collect()
    }

    /// Get performance statistics
    pub async fn get_stats(&self) -> PerformanceStats {
        self.stats.read().clone()
    }

    /// Configure LOD for object type
    pub fn configure_lod(&mut self, object_type: &str, config: LodConfig) {
        self.lod_manager
            .lod_configs
            .insert(object_type.to_string(), config);
        tracing::debug!("Configured LOD for object type: {}", object_type);
    }

    /// Set global optimization parameters
    pub fn set_optimization_params(
        &mut self,
        max_batch_size: u32,
        max_instances: u32,
        lod_bias: f32,
    ) {
        self.batch_optimizer.max_batch_size = max_batch_size;
        self.instance_manager.max_instances_per_draw = max_instances;
        self.lod_manager.global_lod_bias = lod_bias;

        tracing::debug!(
            "Set optimization params: batch_size={}, instances={}, lod_bias={}",
            max_batch_size,
            max_instances,
            lod_bias
        );
    }
}

fn sort_render_batches(batches: &mut [RenderBatch]) {
    batches.sort_by(|a, b| {
        let transparency_order = a.transparent.cmp(&b.transparent);
        if transparency_order != std::cmp::Ordering::Equal {
            return transparency_order;
        }

        if a.transparent {
            b.camera_distance
                .partial_cmp(&a.camera_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.priority.cmp(&b.priority))
                .then_with(|| a.mesh_id.cmp(&b.mesh_id))
        } else {
            a.priority
                .cmp(&b.priority)
                .then_with(|| match (&a.material_id, &b.material_id) {
                    (Some(mat_a), Some(mat_b)) => {
                        let material_cmp = mat_a.cmp(mat_b);
                        if material_cmp == std::cmp::Ordering::Equal {
                            a.mesh_id.cmp(&b.mesh_id)
                        } else {
                            material_cmp
                        }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.mesh_id.cmp(&b.mesh_id),
                })
                .then_with(|| {
                    a.camera_distance
                        .partial_cmp(&b.camera_distance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        }
    });
}

impl GpuCuller {
    /// Create GPU culler with compute shader
    async fn new(device: &Device) -> Result<Self> {
        // Create culling compute pipeline
        let cull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("W3D GPU Culling Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/gpu_culling.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("W3D GPU Culling Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("W3D GPU Culling Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let cull_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("W3D GPU Culling Pipeline"),
            layout: Some(&pipeline_layout),
            module: &cull_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let max_objects = 100000; // Support up to 100k objects

        // Create buffers
        let object_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("W3D GPU Cull Objects Buffer"),
            size: (max_objects * std::mem::size_of::<CullData>() as u32) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let visibility_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("W3D GPU Cull Visibility Buffer"),
            size: (max_objects * 4) as u64, // 4 bytes per visibility flag
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let frustum_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("W3D GPU Cull Frustum Buffer"),
            size: std::mem::size_of::<FrustumData>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            cull_pipeline: Some(cull_pipeline),
            object_buffer: Some(object_buffer),
            visibility_buffer: Some(visibility_buffer),
            frustum_buffer: Some(frustum_buffer),
            occlusion_queries: false, // Would be detected from device capabilities
            max_objects,
        })
    }

    /// Perform GPU-based culling
    async fn cull_objects(
        &self,
        objects: &[RenderObject],
        camera: &Camera,
        device: &Device,
        queue: &Queue,
    ) -> Result<Vec<RenderObject>> {
        if objects.is_empty() {
            return Ok(Vec::new());
        }

        let Some(cull_pipeline) = self.cull_pipeline.as_ref() else {
            return Ok(Self::cpu_fallback_cull(objects, camera));
        };
        let Some(object_buffer) = self.object_buffer.as_ref() else {
            return Ok(Self::cpu_fallback_cull(objects, camera));
        };
        let Some(visibility_buffer) = self.visibility_buffer.as_ref() else {
            return Ok(Self::cpu_fallback_cull(objects, camera));
        };
        let Some(frustum_buffer) = self.frustum_buffer.as_ref() else {
            return Ok(Self::cpu_fallback_cull(objects, camera));
        };

        let object_count = objects.len().min(self.max_objects as usize);
        if object_count == 0 {
            return Ok(Vec::new());
        }

        let cull_data: Vec<CullData> = objects
            .iter()
            .take(object_count)
            .map(|obj| CullData {
                bounds_min: obj.world_bounds.min,
                _padding1: 0.0,
                bounds_max: obj.world_bounds.max,
                _padding2: 0.0,
                // world_bounds is already world-space; keep identity transform.
                transform: Mat4::IDENTITY.to_cols_array_2d(),
            })
            .collect();
        queue.write_buffer(object_buffer, 0, cast_slice(&cull_data));

        let view_proj = Mat4::from_cols_array_2d(&camera.projection_matrix)
            * Mat4::from_cols_array_2d(&camera.view_matrix);
        let frustum_planes = extract_frustum_planes(view_proj);
        let frustum_data = FrustumData {
            planes: frustum_planes.map(|plane| plane.to_array()),
            camera_position: camera.position,
            _padding: 0.0,
        };
        queue.write_buffer(frustum_buffer, 0, bytes_of(&frustum_data));

        let bind_group_layout = cull_pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("W3D GPU Culling Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: object_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: visibility_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: frustum_buffer.as_entire_binding(),
                },
            ],
        });

        let readback_size = (object_count * std::mem::size_of::<u32>()) as u64;
        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("W3D GPU Cull Readback Buffer"),
            size: readback_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("W3D GPU Culling Encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("W3D GPU Culling Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(cull_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = ((object_count as u32) + 63) / 64;
            pass.dispatch_workgroups(groups.max(1), 1, 1);
        }
        encoder.copy_buffer_to_buffer(visibility_buffer, 0, &readback_buffer, 0, readback_size);
        queue.submit(std::iter::once(encoder.finish()));

        let slice = readback_buffer.slice(..readback_size);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(PollType::wait_indefinitely());

        if !matches!(rx.recv(), Ok(Ok(()))) {
            return Ok(Self::cpu_fallback_cull(objects, camera));
        }

        let mapped = slice.get_mapped_range();
        let visibility: Vec<u32> = cast_slice(&mapped).to_vec();
        drop(mapped);
        readback_buffer.unmap();

        let visible: Vec<RenderObject> = objects
            .iter()
            .take(object_count)
            .zip(visibility.iter())
            .filter(|(_, visible)| **visible != 0)
            .map(|(obj, _)| obj.clone())
            .collect();

        tracing::trace!(
            "GPU cull pass kept {} / {} objects",
            visible.len(),
            object_count
        );

        if objects.len() > object_count {
            // Keep deterministic behavior when incoming count exceeds GPU buffer capacity.
            let overflow_visible = Self::cpu_fallback_cull(&objects[object_count..], camera);
            let mut merged = visible;
            merged.extend(overflow_visible);
            return Ok(merged);
        }

        Ok(visible)
    }

    fn cpu_fallback_cull(objects: &[RenderObject], camera: &Camera) -> Vec<RenderObject> {
        let view_proj = Mat4::from_cols_array_2d(&camera.projection_matrix)
            * Mat4::from_cols_array_2d(&camera.view_matrix);
        let frustum = extract_frustum_planes(view_proj);
        objects
            .iter()
            .filter(|obj| is_aabb_in_frustum(&obj.world_bounds, &frustum))
            .cloned()
            .collect()
    }

    fn approximate_memory_bytes(&self) -> u64 {
        let mut total = 0u64;
        if let Some(buffer) = &self.object_buffer {
            total = total.saturating_add(buffer.size());
        }
        if let Some(buffer) = &self.visibility_buffer {
            total = total.saturating_add(buffer.size());
        }
        if let Some(buffer) = &self.frustum_buffer {
            total = total.saturating_add(buffer.size());
        }
        total
    }
}

impl BatchOptimizer {
    /// Optimize batches by grouping similar render calls
    async fn optimize_batches(&mut self, batches: Vec<RenderBatch>) -> Result<Vec<RenderBatch>> {
        self.batched_calls.clear();

        // Group batches by material and mesh
        for batch in batches {
            let key = BatchKey {
                material_id: batch.material_id.clone(),
                mesh_id: batch.mesh_id.clone(),
                shader_variant: 0, // Simplified - would be computed from material properties
                transparent: batch.transparent,
                priority: batch.priority,
            };

            self.batched_calls
                .entry(key)
                .or_insert_with(Vec::new)
                .push(batch);
        }

        // Merge compatible batches
        let mut optimized = Vec::new();
        let max_instances = self.max_batch_size.max(1) as usize;
        let drained_groups: Vec<Vec<RenderBatch>> = self
            .batched_calls
            .drain()
            .map(|(_key, group)| group)
            .collect();

        for batch_group in drained_groups {
            if batch_group.len() == 1 {
                let batch = batch_group.into_iter().next().unwrap();
                optimized.extend(BatchOptimizer::split_batch_by_instance_limit(
                    batch,
                    max_instances,
                ));
            } else {
                // Merge multiple batches of the same type
                let merged = BatchOptimizer::merge_batches(batch_group)?;
                optimized.extend(BatchOptimizer::split_batch_by_instance_limit(
                    merged,
                    max_instances,
                ));
            }
        }

        // Sort batches for optimal rendering
        if self.sort_batches {
            sort_render_batches(&mut optimized);
        }

        Ok(optimized)
    }

    /// Merge compatible render batches
    fn merge_batches(batches: Vec<RenderBatch>) -> Result<RenderBatch> {
        if batches.is_empty() {
            return Err(W3DError::RenderingError(
                "Cannot merge empty batch list".to_string(),
            ));
        }

        let first = &batches[0];
        let mut merged = first.clone();

        // Combine all instances
        for batch in batches.iter().skip(1) {
            merged.instances.extend_from_slice(&batch.instances);
        }

        // Update distance to average
        merged.camera_distance =
            batches.iter().map(|b| b.camera_distance).sum::<f32>() / batches.len() as f32;

        Ok(merged)
    }

    fn split_batch_by_instance_limit(batch: RenderBatch, max_instances: usize) -> Vec<RenderBatch> {
        if batch.instances.len() <= max_instances {
            return vec![batch];
        }

        let mut chunks = Vec::new();
        for chunk in batch.instances.chunks(max_instances) {
            let mut split = batch.clone();
            split.instances = chunk.to_vec();
            chunks.push(split);
        }
        chunks
    }
}

impl InstanceManager {
    /// Create instances from render objects
    async fn create_instances(
        &mut self,
        objects: &[RenderObject],
        camera: &Camera,
        device: &Device,
    ) -> Result<Vec<RenderBatch>> {
        self.instance_data.clear();

        // Group objects by mesh+material combination
        let mut object_groups: HashMap<String, Vec<&RenderObject>> = HashMap::new();

        for obj in objects {
            let key = render_object_batch_key(obj);
            object_groups.entry(key).or_insert_with(Vec::new).push(obj);
        }

        let mut batches = Vec::new();

        let camera_pos = Vec3::from_slice(&camera.position);

        for (_key, group) in object_groups {
            if group.is_empty() {
                continue;
            }

            // Create instance data
            let instances: Vec<InstanceData> = group
                .iter()
                .map(|obj| {
                    let normal_matrix = compute_normal_matrix(obj.transform);
                    InstanceData {
                        model_matrix: obj.transform,
                        normal_matrix,
                        material_index: 0,
                        lod_level: 0,
                        animation_frame: 0.0,
                        custom_data: 0.0,
                        color: [1.0, 1.0, 1.0, 1.0],
                        material_params: obj.material_params,
                    }
                })
                .collect();

            if !self.dynamic_instancing {
                for (instance_index, obj) in group.iter().enumerate() {
                    let distance =
                        (Vec3::from_slice(&obj.world_bounds.center()) - camera_pos).length();
                    let batch = RenderBatch {
                        mesh_id: obj.mesh_id.clone(),
                        mesh: None,
                        material_id: obj.material_id.clone(),
                        material: None,
                        instances: vec![instances[instance_index]],
                        camera_distance: distance,
                        priority: obj.priority,
                        transparent: obj.transparent,
                    };
                    batches.push(batch);
                }
                continue;
            }

            let max_instances = self.max_instances_per_draw.max(1) as usize;
            if let Some(first_obj) = group.first() {
                for (group_chunk, instance_chunk) in group
                    .chunks(max_instances)
                    .zip(instances.chunks(max_instances))
                {
                    let avg_distance = if group_chunk.is_empty() {
                        0.0
                    } else {
                        let distance_sum: f32 = group_chunk
                            .iter()
                            .map(|obj| {
                                (Vec3::from_slice(&obj.world_bounds.center()) - camera_pos).length()
                            })
                            .sum();
                        distance_sum / group_chunk.len() as f32
                    };

                    let batch = RenderBatch {
                        mesh_id: first_obj.mesh_id.clone(),
                        mesh: None,
                        material_id: first_obj.material_id.clone(),
                        material: None,
                        instances: instance_chunk.to_vec(),
                        camera_distance: avg_distance,
                        priority: first_obj.priority,
                        transparent: group_chunk.iter().any(|obj| obj.transparent),
                    };

                    batches.push(batch);
                }
            }
        }

        tracing::debug!(
            "Created {} instanced batches from {} objects",
            batches.len(),
            objects.len()
        );

        Ok(batches)
    }

    fn approximate_memory_bytes(&self) -> u64 {
        let mut total = self
            .instance_data
            .values()
            .map(|instances| {
                (instances.len() as u64).saturating_mul(std::mem::size_of::<InstanceData>() as u64)
            })
            .sum::<u64>();

        for buffer in self.instance_buffers.values() {
            total = total.saturating_add(buffer.size());
        }

        total
    }
}

fn render_object_batch_key(obj: &RenderObject) -> String {
    format!(
        "{}_{}_{}_{}",
        obj.mesh_id,
        obj.material_id.as_deref().unwrap_or("default"),
        obj.priority,
        if obj.transparent {
            "transparent"
        } else {
            "opaque"
        }
    )
}

impl LodManager {
    /// Select appropriate LOD levels for objects
    async fn select_lod_levels(
        &self,
        objects: &[RenderObject],
        camera: &Camera,
    ) -> Result<Vec<RenderObject>> {
        let camera_pos = Vec3::from_slice(&camera.position);

        let lod_objects: Vec<RenderObject> = objects
            .par_iter()
            .map(|obj| {
                let distance = (Vec3::from_slice(&obj.world_bounds.center()) - camera_pos).length();
                let lod_level = self.calculate_lod_level(distance, &obj.mesh_id);

                // Return object with potentially different mesh for LOD
                let mut lod_obj = obj.clone();
                if let Some(lod_mesh) = self.get_lod_mesh(&obj.mesh_id, lod_level) {
                    lod_obj.mesh_id = lod_mesh;
                }

                lod_obj
            })
            .collect();

        Ok(lod_objects)
    }

    /// Calculate LOD level based on distance
    fn calculate_lod_level(&self, distance: f32, mesh_id: &str) -> u32 {
        let biased_distance = distance * (1.0 + self.global_lod_bias).clamp(0.1, 10.0);

        if let Some(config) = self.lod_configs.get(mesh_id) {
            for (i, level) in config.levels.iter().enumerate() {
                if biased_distance <= level.distance {
                    return i as u32;
                }
            }
            return (config.levels.len() - 1) as u32;
        }

        // Default LOD calculation
        if biased_distance < 50.0 {
            0
        } else if biased_distance < 150.0 {
            1
        } else if biased_distance < 400.0 {
            2
        } else {
            3
        }
    }

    /// Get LOD mesh for given level
    fn get_lod_mesh(&self, base_mesh_id: &str, lod_level: u32) -> Option<String> {
        if let Some(config) = self.lod_configs.get(base_mesh_id) {
            config
                .levels
                .get(lod_level as usize)
                .map(|level| level.mesh_id.clone())
        } else if lod_level == 0 {
            None
        } else {
            // Keep the original mesh unless explicit LOD mappings exist. Implicitly
            // rewriting names can create mesh misses in mixed/modded content.
            None
        }
    }
}

fn compute_normal_matrix(model_matrix: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let model = Mat4::from_cols_array_2d(&model_matrix);
    let det = model.determinant();
    if !det.is_finite() || det.abs() <= f32::EPSILON {
        model_matrix
    } else {
        model.inverse().transpose().to_cols_array_2d()
    }
}

fn average_frame_time_ms(history: &VecDeque<FrameTimingStats>) -> Option<f32> {
    if history.is_empty() {
        return None;
    }
    let total = history.iter().map(|entry| entry.frame_time_ms).sum::<f32>();
    Some(total / history.len() as f32)
}

fn quality_adjustment_direction(
    avg_fps: f32,
    target_fps: f32,
    tolerance: f32,
    memory_pressure: bool,
) -> QualityAdjustment {
    if memory_pressure {
        return QualityAdjustment::Decrease;
    }
    if !avg_fps.is_finite() || !target_fps.is_finite() || target_fps <= 0.0 {
        return QualityAdjustment::None;
    }

    let tolerance = tolerance.max(0.0);
    if avg_fps < target_fps - tolerance {
        QualityAdjustment::Decrease
    } else if avg_fps > target_fps + tolerance {
        QualityAdjustment::Increase
    } else {
        QualityAdjustment::None
    }
}

fn estimated_batch_memory_bytes(batches: &[RenderBatch]) -> u64 {
    let batch_bytes =
        (batches.len() as u64).saturating_mul(std::mem::size_of::<RenderBatch>() as u64);
    let instance_bytes = batches
        .iter()
        .map(|batch| {
            (batch.instances.len() as u64)
                .saturating_mul(std::mem::size_of::<InstanceData>() as u64)
        })
        .sum::<u64>();
    batch_bytes.saturating_add(instance_bytes)
}

fn estimated_triangle_count(batches: &[RenderBatch]) -> u32 {
    batches
        .iter()
        .map(|batch| batch.instances.len() as u32)
        .fold(0u32, |acc, count| {
            acc.saturating_add(count.saturating_mul(2))
        })
}

/// Extract frustum planes from view-projection matrix
fn extract_frustum_planes(view_proj: Mat4) -> [Vec4; 6] {
    let m = view_proj.to_cols_array();
    [
        // Left
        Vec4::new(m[3] + m[0], m[7] + m[4], m[11] + m[8], m[15] + m[12]).normalize(),
        // Right
        Vec4::new(m[3] - m[0], m[7] - m[4], m[11] - m[8], m[15] - m[12]).normalize(),
        // Bottom
        Vec4::new(m[3] + m[1], m[7] + m[5], m[11] + m[9], m[15] + m[13]).normalize(),
        // Top
        Vec4::new(m[3] - m[1], m[7] - m[5], m[11] - m[9], m[15] - m[13]).normalize(),
        // Near
        Vec4::new(m[3] + m[2], m[7] + m[6], m[11] + m[10], m[15] + m[14]).normalize(),
        // Far
        Vec4::new(m[3] - m[2], m[7] - m[6], m[11] - m[10], m[15] - m[14]).normalize(),
    ]
}

/// Test if axis-aligned bounding box is inside frustum
fn is_aabb_in_frustum(bounds: &BoundingBox, frustum: &[Vec4; 6]) -> bool {
    let min = Vec3::from_slice(&bounds.min);
    let max = Vec3::from_slice(&bounds.max);

    for plane in frustum {
        let normal = plane.truncate();
        let distance = plane.w;

        // Get positive vertex (farthest point from plane)
        let positive_vertex = Vec3::new(
            if normal.x >= 0.0 { max.x } else { min.x },
            if normal.y >= 0.0 { max.y } else { min.y },
            if normal.z >= 0.0 { max.z } else { min.z },
        );

        // If positive vertex is behind plane, AABB is outside frustum
        if normal.dot(positive_vertex) + distance < 0.0 {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_direction_decreases_under_target() {
        let direction = quality_adjustment_direction(42.0, 60.0, 5.0, false);
        assert_eq!(direction, QualityAdjustment::Decrease);
    }

    #[test]
    fn quality_direction_increases_above_target() {
        let direction = quality_adjustment_direction(90.0, 60.0, 5.0, false);
        assert_eq!(direction, QualityAdjustment::Increase);
    }

    #[test]
    fn quality_direction_holds_inside_tolerance() {
        let direction = quality_adjustment_direction(62.0, 60.0, 5.0, false);
        assert_eq!(direction, QualityAdjustment::None);
    }

    #[test]
    fn quality_direction_prefers_memory_pressure() {
        let direction = quality_adjustment_direction(120.0, 60.0, 5.0, true);
        assert_eq!(direction, QualityAdjustment::Decrease);
    }

    #[test]
    fn average_frame_time_uses_history_entries() {
        let mut history = VecDeque::new();
        history.push_back(FrameTimingStats {
            frame_time_ms: 10.0,
            gpu_time_ms: 3.0,
            draw_calls: 1,
            triangle_count: 10,
        });
        history.push_back(FrameTimingStats {
            frame_time_ms: 20.0,
            gpu_time_ms: 6.0,
            draw_calls: 2,
            triangle_count: 20,
        });

        let avg = average_frame_time_ms(&history).expect("avg frame time");
        assert!((avg - 15.0).abs() < 1.0e-6);
    }

    #[test]
    fn render_object_batch_key_separates_transparent_and_opaque() {
        let base = RenderObject {
            mesh_id: "mesh_a".to_string(),
            material_id: Some("mat_a".to_string()),
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            world_bounds: BoundingBox::new([0.0; 3], [1.0; 3]),
            lod_bias: 0.0,
            cast_shadows: true,
            receive_shadows: true,
            visible: true,
            transparent: false,
            material_params: [0.0, 0.5, 1.0, 0.0],
            priority: 10,
        };

        let mut transparent = base.clone();
        transparent.transparent = true;

        assert_ne!(
            render_object_batch_key(&base),
            render_object_batch_key(&transparent)
        );
    }

    #[test]
    fn sort_render_batches_renders_transparent_back_to_front() {
        let mut batches = vec![
            RenderBatch {
                mesh_id: "opaque".to_string(),
                mesh: None,
                material_id: Some("mat".to_string()),
                material: None,
                instances: Vec::new(),
                camera_distance: 100.0,
                priority: 0,
                transparent: false,
            },
            RenderBatch {
                mesh_id: "transparent_near".to_string(),
                mesh: None,
                material_id: Some("mat".to_string()),
                material: None,
                instances: Vec::new(),
                camera_distance: 10.0,
                priority: 0,
                transparent: true,
            },
            RenderBatch {
                mesh_id: "transparent_far".to_string(),
                mesh: None,
                material_id: Some("mat".to_string()),
                material: None,
                instances: Vec::new(),
                camera_distance: 50.0,
                priority: 0,
                transparent: true,
            },
        ];

        sort_render_batches(&mut batches);

        assert_eq!(batches[0].mesh_id, "opaque");
        assert_eq!(batches[1].mesh_id, "transparent_far");
        assert_eq!(batches[2].mesh_id, "transparent_near");
    }

    #[test]
    fn render_object_batch_key_separates_priority_variants() {
        let mut base = RenderObject {
            mesh_id: "mesh_a".to_string(),
            material_id: Some("mat_a".to_string()),
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            world_bounds: BoundingBox::new([0.0; 3], [1.0; 3]),
            lod_bias: 0.0,
            cast_shadows: true,
            receive_shadows: true,
            visible: true,
            transparent: false,
            material_params: [0.0, 0.5, 1.0, 0.0],
            priority: 10,
        };

        let base_key = render_object_batch_key(&base);
        base.priority = 5;

        assert_ne!(base_key, render_object_batch_key(&base));
    }

    #[test]
    fn sort_render_batches_prioritizes_opaque_specialization_before_distance() {
        let mut batches = vec![
            RenderBatch {
                mesh_id: "mesh_a".to_string(),
                mesh: None,
                material_id: Some("mat_b".to_string()),
                material: None,
                instances: Vec::new(),
                camera_distance: 5.0,
                priority: 10,
                transparent: false,
            },
            RenderBatch {
                mesh_id: "mesh_b".to_string(),
                mesh: None,
                material_id: Some("mat_a".to_string()),
                material: None,
                instances: Vec::new(),
                camera_distance: 50.0,
                priority: 5,
                transparent: false,
            },
        ];

        sort_render_batches(&mut batches);

        assert_eq!(batches[0].priority, 5);
        assert_eq!(batches[1].priority, 10);
    }
}
