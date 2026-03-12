//! Terrain editing system for World Builder

use crate::map::Map;
use anyhow::Result;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Terrain editor with advanced sculpting tools
pub struct TerrainEditor {
    current_map: Option<Arc<RwLock<Map>>>,
    active_tool: TerrainTool,
    brush: TerrainBrush,
    textures: Vec<TerrainTexture>,
    undo_stack: Vec<TerrainUndoAction>,
    redo_stack: Vec<TerrainUndoAction>,
    dirty: bool,
}

impl TerrainEditor {
    pub fn new() -> Self {
        Self {
            current_map: None,
            active_tool: TerrainTool::Raise,
            brush: TerrainBrush::default(),
            textures: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        // Load default terrain textures
        self.load_default_textures()?;
        Ok(())
    }

    /// Set the current map to edit
    pub fn set_map(&mut self, map: Option<Arc<RwLock<Map>>>) -> Result<()> {
        self.current_map = map;
        self.clear_undo_history();
        self.dirty = false;
        Ok(())
    }

    /// Update terrain editor (called each frame)
    pub fn update(&mut self) -> Result<()> {
        // Update any ongoing operations
        Ok(())
    }

    /// Apply terrain modification at world position
    pub fn apply_tool(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        if self.current_map.is_none() {
            return Ok(());
        }

        // Create undo action before modifying
        let undo_action = self.create_undo_action(world_pos)?;

        match self.active_tool {
            TerrainTool::Raise => self.raise_terrain(world_pos, intensity)?,
            TerrainTool::Lower => self.lower_terrain(world_pos, intensity)?,
            TerrainTool::Smooth => self.smooth_terrain(world_pos, intensity)?,
            TerrainTool::Flatten => self.flatten_terrain(world_pos, intensity)?,
            TerrainTool::Paint => self.paint_texture(world_pos, intensity)?,
            TerrainTool::Ramp => self.create_ramp(world_pos, intensity)?,
        }

        self.undo_stack.push(undo_action);
        self.redo_stack.clear();
        self.dirty = true;

        Ok(())
    }

    fn raise_terrain(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        let strength = self.brush.strength;
        self.modify_height_in_brush(world_pos, |height, factor| {
            height + (intensity * factor * strength)
        })
    }

    fn lower_terrain(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        let strength = self.brush.strength;
        self.modify_height_in_brush(world_pos, |height, factor| {
            height - (intensity * factor * strength)
        })
    }

    fn smooth_terrain(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        // TODO: Implement smoothing algorithm
        Ok(())
    }

    fn flatten_terrain(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        let target_height = world_pos.y;
        let strength = self.brush.strength;
        self.modify_height_in_brush(world_pos, |height, factor| {
            let blend = intensity * factor * strength;
            height * (1.0 - blend) + target_height * blend
        })
    }

    fn paint_texture(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        // TODO: Implement texture painting
        Ok(())
    }

    fn create_ramp(&mut self, world_pos: Vec3, intensity: f32) -> Result<()> {
        // TODO: Implement ramp creation
        Ok(())
    }

    /// Apply height modification function to all points within brush
    fn modify_height_in_brush<F>(&mut self, center: Vec3, modifier: F) -> Result<()>
    where
        F: Fn(f32, f32) -> f32,
    {
        if let Some(ref map_arc) = self.current_map {
            let map = map_arc.read().unwrap();
            let map_x = center.x as i32;
            let map_z = center.z as i32;
            let radius = self.brush.size as i32;

            // Collect modifications first
            let mut modifications = Vec::new();
            for y in (map_z - radius).max(0)..=(map_z + radius).min(map.height() as i32 - 1) {
                for x in (map_x - radius).max(0)..=(map_x + radius).min(map.width() as i32 - 1) {
                    let dx = x - map_x;
                    let dy = y - map_z;
                    let distance = ((dx * dx + dy * dy) as f32).sqrt();

                    if distance <= self.brush.size {
                        let factor = self.brush.get_falloff_factor(distance);
                        let current_height = map.get_height(x as u32, y as u32).unwrap_or(0.0);
                        let new_height = modifier(current_height, factor);
                        modifications.push((x as u32, y as u32, new_height));
                    }
                }
            }

            // Drop read lock before acquiring write lock
            drop(map);

            // Apply modifications with write lock
            let mut map_mut = map_arc.write().unwrap();
            for (x, y, height) in modifications {
                map_mut.set_height(x, y, height);
            }
        }

        Ok(())
    }

    /// Create an undo action for the current operation
    fn create_undo_action(&self, center: Vec3) -> Result<TerrainUndoAction> {
        // TODO: Capture affected terrain data for undo
        Ok(TerrainUndoAction {
            center,
            affected_area: BoundingBox {
                min: center - Vec3::splat(self.brush.size),
                max: center + Vec3::splat(self.brush.size),
            },
            height_data: HashMap::new(),
            texture_data: HashMap::new(),
        })
    }

    /// Undo the last terrain operation
    pub fn undo(&mut self) -> Result<()> {
        if let Some(action) = self.undo_stack.pop() {
            // TODO: Apply undo action
            self.redo_stack.push(action);
            self.dirty = true;
        }
        Ok(())
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) -> Result<()> {
        if let Some(action) = self.redo_stack.pop() {
            // TODO: Apply redo action
            self.undo_stack.push(action);
            self.dirty = true;
        }
        Ok(())
    }

    /// Clear undo/redo history
    fn clear_undo_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Save terrain changes to the map
    pub fn save_to_map(&self, map: &mut Map) -> Result<()> {
        // Terrain changes are applied directly to the map
        Ok(())
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    /// Load default terrain textures
    fn load_default_textures(&mut self) -> Result<()> {
        self.textures = vec![
            TerrainTexture {
                name: "Grass".to_string(),
                diffuse_path: "textures/grass_diffuse.png".into(),
                normal_path: Some("textures/grass_normal.png".into()),
                tiling: 16.0,
            },
            TerrainTexture {
                name: "Rock".to_string(),
                diffuse_path: "textures/rock_diffuse.png".into(),
                normal_path: Some("textures/rock_normal.png".into()),
                tiling: 8.0,
            },
            TerrainTexture {
                name: "Sand".to_string(),
                diffuse_path: "textures/sand_diffuse.png".into(),
                normal_path: None,
                tiling: 12.0,
            },
        ];

        Ok(())
    }

    // Getters and setters
    pub fn active_tool(&self) -> TerrainTool {
        self.active_tool
    }
    pub fn set_active_tool(&mut self, tool: TerrainTool) {
        self.active_tool = tool;
    }

    pub fn brush(&self) -> &TerrainBrush {
        &self.brush
    }
    pub fn brush_mut(&mut self) -> &mut TerrainBrush {
        &mut self.brush
    }

    pub fn textures(&self) -> &[TerrainTexture] {
        &self.textures
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

/// Available terrain editing tools
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerrainTool {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Paint,
    Ramp,
}

impl TerrainTool {
    pub fn name(&self) -> &'static str {
        match self {
            TerrainTool::Raise => "Raise",
            TerrainTool::Lower => "Lower",
            TerrainTool::Smooth => "Smooth",
            TerrainTool::Flatten => "Flatten",
            TerrainTool::Paint => "Paint",
            TerrainTool::Ramp => "Ramp",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            TerrainTool::Raise => "Raise terrain height",
            TerrainTool::Lower => "Lower terrain height",
            TerrainTool::Smooth => "Smooth terrain irregularities",
            TerrainTool::Flatten => "Flatten terrain to target height",
            TerrainTool::Paint => "Paint terrain textures",
            TerrainTool::Ramp => "Create terrain ramps",
        }
    }
}

/// Brush for terrain editing
#[derive(Debug, Clone)]
pub struct TerrainBrush {
    pub size: f32,
    pub strength: f32,
    pub hardness: f32, // 0.0 = soft, 1.0 = hard
    pub falloff: BrushFalloff,
}

impl Default for TerrainBrush {
    fn default() -> Self {
        Self {
            size: 10.0,
            strength: 0.5,
            hardness: 0.5,
            falloff: BrushFalloff::Linear,
        }
    }
}

impl TerrainBrush {
    /// Calculate falloff factor based on distance from brush center
    pub fn get_falloff_factor(&self, distance: f32) -> f32 {
        if distance >= self.size {
            return 0.0;
        }

        let normalized_distance = distance / self.size;
        let base_factor = match self.falloff {
            BrushFalloff::Linear => 1.0 - normalized_distance,
            BrushFalloff::Smooth => {
                let t = 1.0 - normalized_distance;
                t * t * (3.0 - 2.0 * t)
            }
            BrushFalloff::Sharp => {
                let t = 1.0 - normalized_distance;
                t * t
            }
        };

        // Apply hardness
        if self.hardness > 0.5 {
            // Sharper falloff
            let sharpness = (self.hardness - 0.5) * 2.0;
            base_factor.powf(1.0 + sharpness * 3.0)
        } else {
            // Softer falloff
            let softness = (0.5 - self.hardness) * 2.0;
            base_factor.powf(1.0 / (1.0 + softness * 3.0))
        }
    }
}

/// Brush falloff patterns
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrushFalloff {
    Linear,
    Smooth,
    Sharp,
}

/// Terrain texture definition
#[derive(Debug, Clone)]
pub struct TerrainTexture {
    pub name: String,
    pub diffuse_path: std::path::PathBuf,
    pub normal_path: Option<std::path::PathBuf>,
    pub tiling: f32,
}

/// Undo/redo action for terrain editing
#[derive(Debug, Clone)]
struct TerrainUndoAction {
    center: Vec3,
    affected_area: BoundingBox,
    height_data: HashMap<(u32, u32), f32>,
    texture_data: HashMap<(u32, u32), u8>,
}

/// Bounding box for affected areas
#[derive(Debug, Clone)]
struct BoundingBox {
    min: Vec3,
    max: Vec3,
}

/// Terrain editing settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainSettings {
    pub enable_height_constraints: bool,
    pub min_height: f32,
    pub max_height: f32,
    pub default_texture: u8,
    pub auto_smooth_enabled: bool,
    pub texture_blend_sharpness: f32,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            enable_height_constraints: true,
            min_height: -50.0,
            max_height: 200.0,
            default_texture: 0,
            auto_smooth_enabled: false,
            texture_blend_sharpness: 0.5,
        }
    }
}
