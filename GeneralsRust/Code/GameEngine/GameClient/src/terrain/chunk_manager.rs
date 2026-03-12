//! # Enhanced Chunk Manager
//! 
//! Manages terrain chunks with integration to the new mesh system.
//! Provides efficient LOD, culling, and rendering coordination.

use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::{Device, Queue, RenderPass};

use crate::terrain::{
    TerrainConfig, TerrainError, TerrainResult, HeightMap, 
    TerrainMeshManager, TerrainMeshStats
};

/// Enhanced chunk manager that integrates with the mesh system
#[derive(Debug)]
pub struct EnhancedChunkManager {
    /// Integrated terrain mesh manager
    mesh_manager: TerrainMeshManager,
    
    /// Configuration
    config: TerrainConfig,
    
    /// GPU device reference
    device: Option<Device>,
    
    /// Performance statistics
    stats: ChunkManagerStats,
    
    /// Current camera position for LOD calculations
    camera_position: Vec3,
    
    /// Enabled state
    enabled: bool,
}

impl EnhancedChunkManager {
    pub fn new() -> Self {
        Self {
            mesh_manager: TerrainMeshManager::new(),
            config: TerrainConfig::default(),
            device: None,
            stats: ChunkManagerStats::default(),
            camera_position: Vec3::new(0.0, 0.0, 10.0),
            enabled: true,
        }
    }
    
    /// Initialize with GPU device
    pub fn init_gpu(&mut self, device: &Device) -> TerrainResult<()> {
        self.device = Some(device.clone());
        self.mesh_manager.init_gpu(device);
        Ok(())
    }
    
    /// Load heightmap and generate terrain chunks
    pub fn load_heightmap(&mut self, heightmap: &HeightMap, config: &TerrainConfig) -> TerrainResult<()> {
        self.config = config.clone();
        
        log::info!("Loading heightmap into chunk manager");
        
        // Generate terrain tiles using mesh manager
        self.mesh_manager.generate_tiles(heightmap)?;
        
        log::info!("Heightmap loaded and terrain chunks generated");
        Ok(())
    }
    
    /// Update all chunks and LOD calculations
    pub fn update(&mut self, camera_position: Vec3, view_matrix: &Mat4, projection_matrix: &Mat4) -> TerrainResult<()> {
        if !self.enabled {
            return Ok(());
        }
        
        self.camera_position = camera_position;
        
        // Update mesh manager with current camera and view parameters
        self.mesh_manager.update(camera_position, view_matrix, projection_matrix, &self.config);
        
        // Update statistics from mesh manager
        let mesh_stats = self.mesh_manager.get_statistics();
        self.stats.total_tiles = mesh_stats.total_tiles;
        self.stats.visible_tiles = mesh_stats.visible_tiles;
        self.stats.total_vertices = mesh_stats.total_vertices;
        self.stats.total_triangles = mesh_stats.total_triangles;
        
        Ok(())
    }
    
    /// Render all visible chunks
    pub fn render(&self, render_pass: &mut RenderPass) -> TerrainResult<()> {
        if !self.enabled {
            return Ok(());
        }

        self.mesh_manager.render(render_pass)
    }

    /// Render all visible chunks with depth-only output (used by depth pre-pass).
    pub fn render_depth(&self, render_pass: &mut RenderPass) -> TerrainResult<()> {
        if !self.enabled {
            return Ok(());
        }
        self.mesh_manager.render_depth_only(render_pass)
    }
    
    /// Mark region as dirty for regeneration
    pub fn mark_region_dirty(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        self.mesh_manager.mark_region_dirty(min_x, min_y, max_x, max_y);
    }
    
    /// Regenerate dirty chunks
    pub fn regenerate_dirty(&mut self, heightmap: &HeightMap) -> TerrainResult<()> {
        self.mesh_manager.regenerate_dirty_tiles(heightmap)
    }
    
    /// Get chunk manager statistics
    pub fn get_statistics(&self) -> &ChunkManagerStats {
        &self.stats
    }
    
    /// Set enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Get current camera position
    pub fn camera_position(&self) -> Vec3 {
        self.camera_position
    }
    
    /// Get configuration
    pub fn config(&self) -> &TerrainConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: TerrainConfig) {
        self.config = config;
    }
    
    /// Initialize chunk manager
    pub fn init(&mut self) -> TerrainResult<()> {
        log::info!("Initializing Enhanced Chunk Manager");
        self.enabled = true;
        Ok(())
    }
    
    /// Reset chunk manager
    pub fn reset(&mut self) -> TerrainResult<()> {
        log::info!("Resetting Enhanced Chunk Manager");
        self.stats = ChunkManagerStats::default();
        self.enabled = true;
        Ok(())
    }
}

impl Default for EnhancedChunkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance statistics for enhanced chunk manager
#[derive(Debug, Default)]
pub struct ChunkManagerStats {
    pub total_tiles: usize,
    pub visible_tiles: usize,
    pub total_vertices: usize,
    pub total_triangles: usize,
    pub lod_transitions: u64,
    pub geometry_updates: u64,
    pub culling_time_ms: f64,
    pub update_time_ms: f64,
    pub render_time_ms: f64,
}

impl ChunkManagerStats {
    /// Reset frame-specific statistics
    pub fn reset_frame_stats(&mut self) {
        // Keep persistent stats, reset per-frame stats
        self.culling_time_ms = 0.0;
        self.update_time_ms = 0.0;
        self.render_time_ms = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::HeightMap;
    
    #[test]
    fn test_chunk_manager_creation() {
        let manager = EnhancedChunkManager::new();
        assert!(manager.enabled);
        assert_eq!(manager.stats.total_tiles, 0);
    }
    
    #[test]
    fn test_chunk_manager_config() {
        let mut manager = EnhancedChunkManager::new();
        let config = TerrainConfig {
            world_size: (2048.0, 2048.0),
            chunk_size: 32,
            ..Default::default()
        };
        
        manager.update_config(config.clone());
        assert_eq!(manager.config().world_size, (2048.0, 2048.0));
        assert_eq!(manager.config().chunk_size, 32);
    }
    
    #[test]
    fn test_chunk_manager_enable_disable() {
        let mut manager = EnhancedChunkManager::new();
        
        assert!(manager.enabled);
        manager.set_enabled(false);
        assert!(!manager.enabled);
        manager.set_enabled(true);
        assert!(manager.enabled);
    }
    
    #[test]
    fn test_statistics_reset() {
        let mut stats = ChunkManagerStats::default();
        stats.total_tiles = 100;
        stats.culling_time_ms = 5.0;
        stats.render_time_ms = 10.0;
        
        stats.reset_frame_stats();
        
        assert_eq!(stats.total_tiles, 100); // Persistent
        assert_eq!(stats.culling_time_ms, 0.0); // Reset
        assert_eq!(stats.render_time_ms, 0.0); // Reset
    }
}
