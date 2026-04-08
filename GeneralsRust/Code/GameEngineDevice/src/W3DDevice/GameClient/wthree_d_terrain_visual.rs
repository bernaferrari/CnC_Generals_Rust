use super::{
    fow_terrain_overlay::FowTerrainOverlay,
    terrain_rendering::{DynamicLight, HeightMapMesh, MAP_XY_FACTOR},
};
use anyhow::Result;
use cgmath::{InnerSpace, Matrix4, Vector3};
use parking_lot::RwLock;
use std::sync::Arc;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
    BufferBindingType, Device, Queue, RenderPass, ShaderStages, TextureFormat, TextureSampleType,
    TextureViewDimension,
};

#[derive(Debug, Clone)]
pub struct TerrainLightingState {
    pub ambient: Vector3<f32>,
    pub light_direction: Vector3<f32>,
    pub light_color: Vector3<f32>,
    pub time_of_day: f32,
}

impl Default for TerrainLightingState {
    fn default() -> Self {
        Self {
            ambient: Vector3::new(0.3, 0.3, 0.3),
            light_direction: Vector3::new(0.0, -1.0, 0.0),
            light_color: Vector3::new(0.7, 0.7, 0.7),
            time_of_day: 12.0,
        }
    }
}

pub struct WthreeDTerrainVisual {
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    bind_group_layout: Option<BindGroupLayout>,
    terrain_mesh: Option<Arc<RwLock<HeightMapMesh>>>,
    shroud_overlay: Option<FowTerrainOverlay>,
    terrain_size: (usize, usize),
    lighting: TerrainLightingState,
    accumulated_time: f32,
}

impl Default for WthreeDTerrainVisual {
    fn default() -> Self {
        Self::new()
    }
}

impl WthreeDTerrainVisual {
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            bind_group_layout: None,
            terrain_mesh: None,
            shroud_overlay: None,
            terrain_size: (0, 0),
            lighting: TerrainLightingState::default(),
            accumulated_time: 0.0,
        }
    }

    pub fn init(
        &mut self,
        device: Arc<Device>,
        queue: Arc<Queue>,
        width: usize,
        height: usize,
        height_data: Vec<u8>,
    ) -> Result<()> {
        let bind_group_layout = Self::create_bind_group_layout(&device);
        let mut terrain_mesh = HeightMapMesh::new(
            device.clone(),
            queue.clone(),
            width,
            height,
            height_data,
            &bind_group_layout,
        )?;
        terrain_mesh.set_global_lighting(
            self.lighting.ambient,
            self.lighting.light_direction,
            self.lighting.light_color,
        );

        let world_bounds = (
            0.0,
            0.0,
            width as f32 * MAP_XY_FACTOR,
            height as f32 * MAP_XY_FACTOR,
        );
        let shroud_overlay = FowTerrainOverlay::new(
            device.clone(),
            queue.clone(),
            width.max(1) as u32,
            height.max(1) as u32,
            world_bounds,
            TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Depth32Float,
        );

        self.device = Some(device);
        self.queue = Some(queue);
        self.bind_group_layout = Some(bind_group_layout);
        self.terrain_mesh = Some(Arc::new(RwLock::new(terrain_mesh)));
        self.shroud_overlay = Some(shroud_overlay);
        self.terrain_size = (width, height);
        self.accumulated_time = 0.0;
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) {
        self.accumulated_time += delta_time.max(0.0);
    }

    pub fn update_height_region(
        &mut self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
        heights: &[u8],
    ) -> Result<()> {
        if let Some(mesh) = &self.terrain_mesh {
            mesh.write()
                .update_height_region(start_x, start_y, width, height, heights)?;
        }
        Ok(())
    }

    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>, view_proj: Matrix4<f32>) {
        if let Some(mesh) = &self.terrain_mesh {
            let mesh = mesh.read();
            mesh.update_uniforms(view_proj, self.accumulated_time);
            mesh.render(render_pass);
        }
        if let Some(shroud_overlay) = &self.shroud_overlay {
            shroud_overlay.render(render_pass);
        }
    }

    pub fn do_lighting(
        &mut self,
        ambient: Vector3<f32>,
        light_direction: Vector3<f32>,
        light_color: Vector3<f32>,
        time_of_day: f32,
    ) {
        self.lighting = TerrainLightingState {
            ambient,
            light_direction: if light_direction.magnitude2() > 0.0 {
                light_direction.normalize()
            } else {
                Vector3::new(0.0, -1.0, 0.0)
            },
            light_color,
            time_of_day,
        };

        if let Some(mesh) = &self.terrain_mesh {
            mesh.write().set_global_lighting(
                self.lighting.ambient,
                self.lighting.light_direction,
                self.lighting.light_color,
            );
        }
    }

    pub fn update_shroud(&mut self, shroud_data: &[u8], player_id: u32, intensity: f32) {
        if let Some(shroud_overlay) = &mut self.shroud_overlay {
            shroud_overlay.set_player_id(player_id);
            shroud_overlay.set_fog_intensity(intensity);
            shroud_overlay.update_texture(shroud_data);
        }
    }

    pub fn update_dynamic_lighting(&mut self, dynamic_lights: &[DynamicLight]) {
        if let Some(mesh) = &self.terrain_mesh {
            mesh.write().update_dynamic_lighting(dynamic_lights);
        }
    }

    pub fn get_height_at(&self, world_x: f32, world_y: f32) -> f32 {
        self.terrain_mesh
            .as_ref()
            .map(|mesh| mesh.read().get_height_at(world_x, world_y))
            .unwrap_or(0.0)
    }

    /// Intersect a ray with the terrain heightmap.
    ///
    /// Corresponds to C++ `W3DTerrainVisual::intersectTerrain(Coord3D*, Coord3D*, Coord3D*)`.
    /// C++ uses `RayCollisionTestClass` which does a DDA-style traversal of the
    /// heightmap grid. This Rust implementation does the same: walk along the ray
    /// in world-space steps and test if the ray passes below the terrain surface.
    ///
    /// Returns `Some(Coord3D)` with the intersection point or `None`.
    pub fn intersect_terrain(
        &self,
        ray_start_x: f32,
        ray_start_y: f32,
        ray_start_z: f32,
        ray_end_x: f32,
        ray_end_y: f32,
        ray_end_z: f32,
    ) -> Option<(f32, f32, f32)> {
        let mesh = self.terrain_mesh.as_ref()?;
        let mesh_lock = mesh.read();

        let (map_w, map_h) = self.terrain_size;
        if map_w == 0 || map_h == 0 {
            return None;
        }

        let dx = ray_end_x - ray_start_x;
        let dy = ray_end_y - ray_start_y;
        let dz = ray_end_z - ray_start_z;
        let ray_len = (dx * dx + dy * dy + dz * dz).sqrt();
        if ray_len < 0.001 {
            return None;
        }

        let step = MAP_XY_FACTOR * 0.5;
        let num_steps = (ray_len / step).ceil() as usize;
        if num_steps == 0 {
            return None;
        }

        let inv_steps = 1.0 / num_steps as f32;

        for i in 0..=num_steps {
            let t = i as f32 * inv_steps;
            let sample_x = ray_start_x + dx * t;
            let sample_y = ray_start_y + dy * t;
            let sample_z = ray_start_z + dz * t;

            let terrain_h = mesh_lock.get_height_at(sample_x, sample_y);

            if sample_y <= terrain_h {
                let prev_t = if i > 0 {
                    (i - 1) as f32 * inv_steps
                } else {
                    0.0
                };
                let (hit_x, hit_y, hit_z) = Self::refine_intersection(
                    ray_start_x,
                    ray_start_y,
                    ray_start_z,
                    dx,
                    dy,
                    dz,
                    prev_t,
                    t,
                    &|wx, wy| mesh_lock.get_height_at(wx, wy),
                );
                return Some((hit_x, hit_y, hit_z));
            }
        }

        None
    }

    /// Binary search refinement of the terrain intersection point.
    ///
    /// Given a ray parameter range [t_lo, t_hi] where the ray crosses the
    /// terrain surface, refine to find the exact intersection point.
    fn refine_intersection<F>(
        ox: f32,
        oy: f32,
        oz: f32,
        dx: f32,
        dy: f32,
        dz: f32,
        t_lo: f32,
        t_hi: f32,
        get_height: &F,
    ) -> (f32, f32, f32)
    where
        F: Fn(f32, f32) -> f32,
    {
        let mut lo = t_lo;
        let mut hi = t_hi;

        for _ in 0..8 {
            let mid = (lo + hi) * 0.5;
            let sx = ox + dx * mid;
            let sy = oy + dy * mid;
            let _sz = oz + dz * mid;
            let th = get_height(sx, _sz);

            if sy <= th {
                hi = mid;
            } else {
                lo = mid;
            }
        }

        let t = (lo + hi) * 0.5;
        let x = ox + dx * t;
        let y = oy + dy * t;
        let z = oz + dz * t;
        (x, y, z)
    }

    /// Set shroud level for a specific cell.
    ///
    /// Corresponds to C++ `W3DDisplay::setShroudLevel(Int x, Int y, CellShroudStatus setting)`.
    pub fn set_shroud_level(&mut self, cell_x: u32, cell_y: u32, alpha: u8) {
        if let Some(ref mut shroud_overlay) = self.shroud_overlay {
            shroud_overlay.set_fog_intensity(alpha as f32 / 255.0);
        }
    }

    pub fn update_shroud_texture(&self, shroud_data: &[u8]) {
        if let Some(ref shroud_overlay) = self.shroud_overlay {
            shroud_overlay.update_texture(shroud_data);
        }
    }

    pub fn terrain_mesh(&self) -> Option<Arc<RwLock<HeightMapMesh>>> {
        self.terrain_mesh.as_ref().map(Arc::clone)
    }

    fn create_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Visual Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_terrain_visual_defaults() {
        let terrain_visual = WthreeDTerrainVisual::new();
        assert_eq!(terrain_visual.terrain_size, (0, 0));
        assert!(terrain_visual.terrain_mesh.is_none());
    }
}
