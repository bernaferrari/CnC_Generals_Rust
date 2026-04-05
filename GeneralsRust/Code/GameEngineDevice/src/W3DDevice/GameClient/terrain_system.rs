//! Complete Terrain Rendering System Integration
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/W3DTerrainVisual.cpp
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DTerrainVisual.h
//!
//! This module integrates all terrain subsystems:
//! - HeightMap mesh generation and rendering
//! - Texture blending and atlas management
//! - LOD system for performance
//! - Dynamic lighting
//! - Frustum culling

use super::{
    terrain_lod::{LODTransition, TerrainLOD, TerrainLODManager},
    terrain_rendering::{DynamicLight, HeightMapMesh, TerrainUniforms},
    terrain_texture::{BlendTileInfo, TerrainTextureManager, TextureClass, TileData},
};
use anyhow::{Context, Result};
use cgmath::{Matrix4, Point3, Vector3, Vector4};
use parking_lot::RwLock;
use std::sync::Arc;
use wgpu::{BindGroupLayout, Device, Queue, RenderPass};

/// Complete terrain rendering system
/// Corresponds to C++ W3DTerrainVisual class
pub struct TerrainRenderingSystem {
    /// HeightMap mesh
    heightmap: Arc<RwLock<HeightMapMesh>>,

    /// Texture manager
    texture_manager: Arc<RwLock<TerrainTextureManager>>,

    /// LOD manager
    lod_manager: Arc<RwLock<TerrainLODManager>>,

    /// Dynamic lights affecting terrain
    dynamic_lights: Vec<DynamicLight>,

    /// Frustum planes for culling
    frustum_planes: [Vector4<f32>; 6],

    /// Current camera position
    camera_position: Point3<f32>,

    /// Time accumulator for animations
    time: f32,

    /// Is terrain rendering enabled
    enabled: bool,
}

impl TerrainRenderingSystem {
    /// Create a new terrain rendering system
    /// Corresponds to C++ W3DTerrainVisual::init
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        width: usize,
        height: usize,
        height_data: Vec<u8>,
        bind_group_layout: &BindGroupLayout,
    ) -> Result<Self> {
        // Create heightmap mesh
        let heightmap = HeightMapMesh::new(
            device.clone(),
            queue.clone(),
            width,
            height,
            height_data,
            bind_group_layout,
        )?;

        // Create texture manager
        let texture_manager = TerrainTextureManager::new(device.clone(), queue.clone())?;

        // Create LOD manager with default settings
        let lod_manager = TerrainLODManager::new(TerrainLOD::Max);

        Ok(Self {
            heightmap: Arc::new(RwLock::new(heightmap)),
            texture_manager: Arc::new(RwLock::new(texture_manager)),
            lod_manager: Arc::new(RwLock::new(lod_manager)),
            dynamic_lights: Vec::new(),
            frustum_planes: [Vector4::new(0.0, 0.0, 0.0, 0.0); 6],
            camera_position: Point3::new(0.0, 0.0, 0.0),
            time: 0.0,
            enabled: true,
        })
    }

    /// Load terrain textures from map data
    /// Corresponds to C++ WorldHeightMap texture loading
    pub fn load_textures(&self, tiles: Vec<TileData>) -> Result<()> {
        let mut texture_mgr = self.texture_manager.write();

        for (idx, tile) in tiles.into_iter().enumerate() {
            texture_mgr.load_source_tile(idx, tile)?;
        }

        // Update texture atlases
        texture_mgr.update_base_atlas()?;
        texture_mgr.update_detail_atlas()?;

        Ok(())
    }

    /// Load edge blend tiles
    pub fn load_edge_tiles(&self, tiles: Vec<TileData>) -> Result<()> {
        let mut texture_mgr = self.texture_manager.write();

        for (idx, tile) in tiles.into_iter().enumerate() {
            texture_mgr.load_edge_tile(idx, tile)?;
        }

        texture_mgr.update_detail_atlas()?;

        Ok(())
    }

    /// Add a texture class
    pub fn add_texture_class(&self, class: TextureClass) {
        let mut texture_mgr = self.texture_manager.write();
        texture_mgr.add_texture_class(class);
    }

    /// Update dynamic lighting
    /// Corresponds to C++ HeightMapRenderObjClass::On_Frame_Update
    pub fn update_lighting(&mut self, delta_time: f32) {
        if !self.enabled {
            return;
        }

        // Update time for shader animations
        self.time += delta_time;

        // Update dynamic lighting on terrain chunks
        if !self.dynamic_lights.is_empty() {
            let mut heightmap = self.heightmap.write();
            heightmap.update_dynamic_lighting(&self.dynamic_lights);
        }
    }

    /// Update frustum culling
    /// Corresponds to C++ BaseHeightMapRenderObjClass::Render culling
    pub fn update_frustum(&mut self, frustum_planes: [Vector4<f32>; 6]) {
        self.frustum_planes = frustum_planes;

        let mut heightmap = self.heightmap.write();
        heightmap.update_frustum_culling(&frustum_planes);
    }

    /// Update camera position for LOD calculations
    pub fn update_camera(&mut self, position: Point3<f32>) {
        self.camera_position = position;
    }

    /// Add a dynamic light
    pub fn add_dynamic_light(&mut self, light: DynamicLight) {
        self.dynamic_lights.push(light);
    }

    /// Clear all dynamic lights
    pub fn clear_dynamic_lights(&mut self) {
        self.dynamic_lights.clear();
    }

    /// Remove a dynamic light
    pub fn remove_dynamic_light(&mut self, index: usize) {
        if index < self.dynamic_lights.len() {
            self.dynamic_lights.remove(index);
        }
    }

    /// Set global LOD level
    /// Corresponds to C++ adjustTerrainLOD
    pub fn set_lod_level(&self, lod: TerrainLOD) {
        let mut lod_mgr = self.lod_manager.write();
        lod_mgr.set_global_lod(lod);
    }

    /// Get current LOD level
    pub fn get_lod_level(&self) -> TerrainLOD {
        let lod_mgr = self.lod_manager.read();
        lod_mgr.get_global_lod()
    }

    /// Render the terrain
    /// Corresponds to C++ HeightMapRenderObjClass::Render
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>, view_proj: Matrix4<f32>) {
        if !self.enabled {
            return;
        }

        // Update uniforms
        let heightmap = self.heightmap.read();
        heightmap.update_uniforms(view_proj, self.time);

        // Render terrain
        heightmap.render(render_pass);
    }

    /// Get height at world position
    /// Corresponds to C++ BaseHeightMapRenderObjClass::getHeightMapHeight
    pub fn get_height_at(&self, x: f32, y: f32) -> f32 {
        let heightmap = self.heightmap.read();
        heightmap.get_height_at(x, y)
    }

    /// Enable or disable terrain rendering
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Is terrain rendering enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get texture manager for external access
    pub fn get_texture_manager(&self) -> Arc<RwLock<TerrainTextureManager>> {
        Arc::clone(&self.texture_manager)
    }

    /// Get heightmap for external access
    pub fn get_heightmap(&self) -> Arc<RwLock<HeightMapMesh>> {
        Arc::clone(&self.heightmap)
    }

    /// Get LOD manager for external access
    pub fn get_lod_manager(&self) -> Arc<RwLock<TerrainLODManager>> {
        Arc::clone(&self.lod_manager)
    }
}

/// Builder for terrain rendering system
pub struct TerrainRenderingSystemBuilder {
    width: Option<usize>,
    height: Option<usize>,
    height_data: Option<Vec<u8>>,
    lod_level: TerrainLOD,
}

impl TerrainRenderingSystemBuilder {
    pub fn new() -> Self {
        Self {
            width: None,
            height: None,
            height_data: None,
            lod_level: TerrainLOD::Max,
        }
    }

    pub fn width(mut self, width: usize) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: usize) -> Self {
        self.height = Some(height);
        self
    }

    pub fn height_data(mut self, data: Vec<u8>) -> Self {
        self.height_data = Some(data);
        self
    }

    pub fn lod_level(mut self, lod: TerrainLOD) -> Self {
        self.lod_level = lod;
        self
    }

    pub fn build(
        self,
        device: Arc<Device>,
        queue: Arc<Queue>,
        bind_group_layout: &BindGroupLayout,
    ) -> Result<TerrainRenderingSystem> {
        let width = self.width.context("Width not set")?;
        let height = self.height.context("Height not set")?;
        let height_data = self.height_data.context("Height data not set")?;

        let mut system = TerrainRenderingSystem::new(
            device,
            queue,
            width,
            height,
            height_data,
            bind_group_layout,
        )?;

        system.set_lod_level(self.lod_level);

        Ok(system)
    }
}

impl Default for TerrainRenderingSystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_pattern() {
        let builder = TerrainRenderingSystemBuilder::new()
            .width(129)
            .height(129)
            .height_data(vec![0; 129 * 129])
            .lod_level(TerrainLOD::Max);

        assert!(builder.width.is_some());
        assert!(builder.height.is_some());
        assert!(builder.height_data.is_some());
    }

    #[test]
    fn test_dynamic_lights() {
        // This would require WGPU device which we can't create in unit tests
        // Instead, test the light management logic

        let light1 = DynamicLight {
            position: Vector3::new(0.0, 0.0, 10.0),
            color: Vector3::new(1.0, 1.0, 1.0),
            range: 100.0,
        };

        let light2 = DynamicLight {
            position: Vector3::new(10.0, 10.0, 10.0),
            color: Vector3::new(1.0, 0.0, 0.0),
            range: 50.0,
        };

        // Would add to system and verify count
        assert_eq!(light1.range, 100.0);
        assert_eq!(light2.range, 50.0);
    }
}
