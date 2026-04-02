//! # Terrain Manager
//!
//! Central coordinator for terrain operations including heightmap loading,
//! chunk management, and terrain modifications.

use glam::{Mat4, Vec3};
use log::warn;
use std::path::Path;

use super::{
    chunk::ChunkManager, height_map::HeightModOperation, terrain_visual::get_terrain_visual,
    HeightMap, TerrainConfig, TerrainError, TerrainModification, TerrainStats, TerrainVisual,
};
use crate::system::SubsystemInterface;

/// Terrain manager coordinating all terrain systems
pub struct TerrainManager {
    config: TerrainConfig,
    stats: TerrainStats,
    enabled: bool,
    height_map: Option<HeightMap>,
    heightmap_path: Option<String>,
    chunk_manager: ChunkManager,
    texture_layers: Vec<String>,
}

impl TerrainManager {
    pub fn new() -> Self {
        Self::with_config(TerrainConfig::default())
    }

    pub fn with_config(config: TerrainConfig) -> Self {
        Self {
            chunk_manager: ChunkManager::with_config(config.clone()),
            config,
            stats: TerrainStats::default(),
            enabled: true,
            height_map: None,
            heightmap_path: None,
            texture_layers: Vec::new(),
        }
    }

    pub fn load_heightmap(&mut self, path: &str) -> Result<(), TerrainError> {
        let extension = Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .ok_or_else(|| {
                TerrainError::HeightmapError(format!("Heightmap path has no extension: {}", path))
            })?;

        let height_map = match extension.as_str() {
            "hmp" => HeightMap::load_hmp(path)?,
            "tga" => HeightMap::load_tga(path)?,
            "raw" => HeightMap::load_raw(path)?,
            other => {
                return Err(TerrainError::HeightmapError(format!(
                    "Unsupported heightmap format: .{}",
                    other
                )))
            }
        };

        self.chunk_manager
            .load_heightmap(&height_map, &self.config)?;
        self.stats.loaded_chunks = self.chunk_manager.get_stats().total_chunks as usize;
        self.height_map = Some(height_map);
        self.heightmap_path = Some(path.to_string());

        if let Ok(mut visual_guard) = get_terrain_visual() {
            if let Some(visual) = visual_guard.as_mut() {
                if let Err(err) = visual.load_heightmap(path) {
                    warn!(
                        "Terrain visual failed to load heightmap '{}': {}",
                        path, err
                    );
                }

                if !self.texture_layers.is_empty() {
                    let layer_refs: Vec<&str> =
                        self.texture_layers.iter().map(|s| s.as_str()).collect();
                    if let Err(err) = visual.load_textures(&layer_refs) {
                        warn!(
                            "Terrain visual failed to reload texture layers {:?}: {}",
                            self.texture_layers, err
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn load_texture_layers(&mut self, paths: &[&str]) -> Result<(), TerrainError> {
        if paths.is_empty() {
            return Ok(());
        }

        self.texture_layers = paths.iter().map(|p| p.to_string()).collect();

        match super::terrain_visual::get_terrain_visual() {
            Ok(mut guard) => {
                if let Some(visual) = guard.as_mut() {
                    visual.load_textures(paths)?;
                } else {
                    warn!("TerrainVisual is not initialised; cannot load texture layers");
                }
            }
            Err(err) => {
                warn!("Failed to access TerrainVisual: {}", err);
            }
        }

        Ok(())
    }

    pub fn apply_modification(
        &mut self,
        modification: TerrainModification,
    ) -> Result<(), TerrainError> {
        if let Some(ref mut height_map) = self.height_map {
            let max_height = height_map.max_height.max(f32::EPSILON);
            match &modification {
                TerrainModification::Raise {
                    position,
                    radius,
                    strength,
                    ..
                } => {
                    let normalized = (*strength / max_height).clamp(-1.0, 1.0);
                    height_map.apply_modification(
                        Vec3::new(position.x, position.z, position.y),
                        *radius,
                        normalized,
                        HeightModOperation::Raise,
                    );
                }
                TerrainModification::Lower {
                    position,
                    radius,
                    strength,
                    ..
                } => {
                    let normalized = (*strength / max_height).clamp(-1.0, 1.0);
                    height_map.apply_modification(
                        Vec3::new(position.x, position.z, position.y),
                        *radius,
                        normalized,
                        HeightModOperation::Lower,
                    );
                }
                TerrainModification::Flatten {
                    position,
                    radius,
                    target_height,
                    ..
                } => {
                    height_map.apply_modification(
                        Vec3::new(position.x, position.z, position.y),
                        *radius,
                        1.0,
                        HeightModOperation::Flatten(*target_height),
                    );
                }
                TerrainModification::CreateCrater {
                    position,
                    radius,
                    depth,
                } => {
                    let normalized = (*depth / max_height).clamp(0.0, 1.0);
                    height_map.apply_modification(
                        Vec3::new(position.x, position.z, position.y),
                        *radius,
                        normalized,
                        HeightModOperation::Lower,
                    );
                }
                TerrainModification::Smooth {
                    position,
                    radius,
                    strength,
                } => {
                    height_map.apply_modification(
                        Vec3::new(position.x, position.z, position.y),
                        *radius,
                        strength.clamp(0.0, 1.0),
                        HeightModOperation::Smooth,
                    );
                }
            }
        }

        self.chunk_manager.apply_modification(&modification)?;
        if let Some(ref height_map) = self.height_map {
            self.chunk_manager.refresh_dirty_chunks(height_map);
        }
        self.stats.modifications_applied += 1;
        Ok(())
    }

    pub fn update(&mut self, _delta_time: f32) {
        if !self.enabled {
            return;
        }

        self.stats.reset();

        if let Err(err) = self.chunk_manager.update() {
            log::warn!("Terrain chunk update failed: {}", err);
        }

        let chunk_stats = self.chunk_manager.get_stats();
        self.stats.loaded_chunks = chunk_stats.total_chunks as usize;
        self.stats.rendered_chunks = chunk_stats.rendered_chunks as usize;
        self.stats.culled_chunks = chunk_stats
            .total_chunks
            .saturating_sub(chunk_stats.visible_chunks) as usize;
        self.stats.update_time_ms = chunk_stats.update_time.as_secs_f64() * 1000.0;
        self.stats.triangles_rendered = self
            .chunk_manager
            .get_visible_chunks()
            .iter()
            .map(|chunk| chunk.stats.triangle_count as usize)
            .sum();
    }

    pub fn config(&self) -> &TerrainConfig {
        &self.config
    }

    pub fn stats(&self) -> &TerrainStats {
        &self.stats
    }
}

impl Default for TerrainManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for TerrainManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing TerrainManager subsystem");
        self.chunk_manager.init()?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting TerrainManager subsystem");
        self.chunk_manager.reset()?;
        self.height_map = None;
        self.stats = TerrainStats::default();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update(1.0 / 60.0);
        Ok(())
    }
}

impl TerrainVisual for TerrainManager {
    fn render(&mut self, view_matrix: &Mat4, projection_matrix: &Mat4) -> Result<(), TerrainError> {
        if let Ok(mut guard) = super::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.render(view_matrix, projection_matrix)?;
            }
        }
        Ok(())
    }

    fn get_height_at(&self, x: f32, y: f32) -> Result<f32, TerrainError> {
        if let Ok(mut guard) = super::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                return visual.get_height_at(x, y);
            }
        }
        Ok(0.0)
    }

    fn get_normal_at(&self, x: f32, y: f32) -> Result<Vec3, TerrainError> {
        if let Ok(mut guard) = super::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                return visual.get_normal_at(x, y);
            }
        }
        Ok(Vec3::new(0.0, 0.0, 1.0))
    }

    fn is_valid_position(&self, x: f32, y: f32) -> bool {
        x >= 0.0 && y >= 0.0 && x < self.config.world_size.0 && y < self.config.world_size.1
    }

    fn chunk_manager(&self) -> &crate::terrain::chunk::ChunkManager {
        &self.chunk_manager
    }

    fn chunk_draw_count(&self) -> usize {
        self.stats.rendered_chunks
    }

    fn oversize_terrain(&mut self, amount: i32) {
        if let Ok(mut guard) = super::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.oversize_terrain(amount);
            }
        }
    }
}
