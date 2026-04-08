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
