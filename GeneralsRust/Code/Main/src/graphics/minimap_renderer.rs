//! Minimap FOW Texture Renderer
//!
//! This module handles GPU texture management for the minimap fog-of-war system.
//! It bridges between the game logic FOW state and the UI rendering system.
//!
//! Prefer `PresentationFowGrid` (frozen on `PresentationFrame`) when available so
//! minimap texture regeneration does not re-lock the live shroud manager mid-render.

use crate::fow_rendering::PresentationFowGrid;
use crate::ui::UiTextureId;
use anyhow::{anyhow, Result};
use gamelogic::common::Coord3D as LogicCoord3D;
use gamelogic::system::shroud_manager::get_shroud_manager;
use glam::{Vec2, Vec3};
use log::{debug, trace};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureFormat, TextureUsages, TextureView};

pub trait UiTextureRegistrar {
    fn free_texture(&mut self, texture_id: UiTextureId);
    fn register_native_texture(
        &mut self,
        device: &Device,
        texture_view: &TextureView,
        filter_mode: wgpu::FilterMode,
    ) -> UiTextureId;
}

// --- Wave 79 minimap residual honesty (host-testable; no GPU claim) ---

/// Retail / host standard minimap texture resolution residual (square).
pub const MINIMAP_STANDARD_SIZE: u32 = 256;
/// Default world span residual when map bounds are unknown (square map).
pub const MINIMAP_DEFAULT_WORLD_SPAN: f32 = 1024.0;
/// Default minimap panel screen origin residual (top-left padding).
pub const MINIMAP_DEFAULT_SCREEN_ORIGIN: f32 = 10.0;
/// FOW shade residual when blending with terrain base (Visible / Explored / Hidden).
pub const MINIMAP_FOW_SHADE_VISIBLE: f32 = 1.0;
pub const MINIMAP_FOW_SHADE_EXPLORED: f32 = 0.5;
pub const MINIMAP_FOW_SHADE_HIDDEN: f32 = 0.12;
/// Pure FOW grayscale residual (no terrain base).
pub const MINIMAP_FOW_RGBA_HIDDEN: [u8; 4] = [0, 0, 0, 255];
pub const MINIMAP_FOW_RGBA_EXPLORED: [u8; 4] = [90, 90, 90, 255];
pub const MINIMAP_FOW_RGBA_VISIBLE: [u8; 4] = [255, 255, 255, 255];
/// Soft-edge residual: keep 3/4 own pixel + 1/4 3×3 neighborhood average.
pub const MINIMAP_SOFTEN_SELF_WEIGHT: u16 = 3;
pub const MINIMAP_SOFTEN_NEIGHBOR_WEIGHT: u16 = 1;
pub const MINIMAP_SOFTEN_WEIGHT_SUM: u16 = 4;

// Define minimap types locally until FOW system is properly integrated
#[derive(Debug, Clone, Copy)]
pub struct MinimapDimensions {
    pub width: u32,
    pub height: u32,
}

impl MinimapDimensions {
    pub fn standard() -> Self {
        MinimapDimensions {
            width: MINIMAP_STANDARD_SIZE,
            height: MINIMAP_STANDARD_SIZE,
        }
    }
}

/// Wave 79 minimap residual honesty pack (dimensions / FOW shade / soft-edge).
///
/// Fail-closed: not full SAGE Radar/Minimap GPU atlas or live click-to-scroll camera.
pub fn honesty_minimap_residual_pack_wave79() -> bool {
    let std = MinimapDimensions::standard();
    let visible = MinimapFowManager::blend_fow_with_base([200, 180, 160, 255], MinimapFowState::Visible);
    let explored = MinimapFowManager::blend_fow_with_base([200, 180, 160, 255], MinimapFowState::Explored);
    let hidden = MinimapFowManager::blend_fow_with_base([200, 180, 160, 255], MinimapFowState::Hidden);
    std.width == MINIMAP_STANDARD_SIZE
        && std.height == MINIMAP_STANDARD_SIZE
        && (MINIMAP_DEFAULT_WORLD_SPAN - 1024.0).abs() < 0.01
        && (MINIMAP_DEFAULT_SCREEN_ORIGIN - 10.0).abs() < 0.01
        && (MINIMAP_FOW_SHADE_VISIBLE - 1.0).abs() < 0.001
        && (MINIMAP_FOW_SHADE_EXPLORED - 0.5).abs() < 0.001
        && (MINIMAP_FOW_SHADE_HIDDEN - 0.12).abs() < 0.001
        && MinimapFowManager::state_to_rgba(MinimapFowState::Hidden) == MINIMAP_FOW_RGBA_HIDDEN
        && MinimapFowManager::state_to_rgba(MinimapFowState::Explored) == MINIMAP_FOW_RGBA_EXPLORED
        && MinimapFowManager::state_to_rgba(MinimapFowState::Visible) == MINIMAP_FOW_RGBA_VISIBLE
        && visible == [200, 180, 160, 255]
        && explored[0] < visible[0]
        && hidden[0] < explored[0]
        && MINIMAP_SOFTEN_SELF_WEIGHT + MINIMAP_SOFTEN_NEIGHBOR_WEIGHT == MINIMAP_SOFTEN_WEIGHT_SUM
        && PresentationFowGrid::CELL_HIDDEN == 0
        && PresentationFowGrid::CELL_EXPLORED == 1
        && PresentationFowGrid::CELL_VISIBLE == 2
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MinimapFowState {
    Hidden,
    Explored,
    Visible,
}

fn shroud_runtime_active(
    shroud_mgr: &gamelogic::system::shroud_manager::ShroudManager,
    player_id: u32,
) -> bool {
    shroud_mgr.get_last_update_frame() > 0 || !shroud_mgr.get_visible_objects(player_id).is_empty()
}

pub struct MinimapFowManager {
    dimensions: MinimapDimensions,
    world_min: Vec3,
    world_max: Vec3,
    cached_pixel_states: HashMap<usize, Vec<MinimapFowState>>,
    cached_textures: HashMap<usize, Vec<u8>>,
    base_terrain_texture: Option<Vec<u8>>,
}

impl MinimapFowManager {
    pub fn new(dimensions: MinimapDimensions) -> Self {
        MinimapFowManager {
            dimensions,
            world_min: Vec3::new(0.0, 0.0, 0.0),
            world_max: Vec3::new(1024.0, 0.0, 1024.0),
            cached_pixel_states: HashMap::new(),
            cached_textures: HashMap::new(),
            base_terrain_texture: None,
        }
    }

    pub fn ensure_dimensions(&mut self, dimensions: MinimapDimensions) {
        if self.dimensions.width == dimensions.width && self.dimensions.height == dimensions.height
        {
            return;
        }
        self.dimensions = dimensions;
        self.cached_pixel_states.clear();
        self.cached_textures.clear();
        self.base_terrain_texture = None;
    }

    pub fn set_world_bounds(&mut self, world_min: Vec3, world_max: Vec3) {
        self.world_min = world_min;
        self.world_max = world_max;
    }

    pub fn get_pixel_state(&self, player_id: usize, x: u32, y: u32) -> MinimapFowState {
        if x >= self.dimensions.width || y >= self.dimensions.height {
            return MinimapFowState::Hidden;
        }

        let index = (y * self.dimensions.width + x) as usize;
        self.cached_pixel_states
            .get(&player_id)
            .and_then(|states| states.get(index).copied())
            .unwrap_or(MinimapFowState::Visible)
    }

    fn visible_texture(&self) -> Vec<u8> {
        let size = (self.dimensions.width * self.dimensions.height * 4) as usize;
        vec![255u8; size]
    }

    fn pixel_to_shroud_world(&self, x: u32, y: u32) -> LogicCoord3D {
        let width = self.dimensions.width.max(1) as f32;
        let height = self.dimensions.height.max(1) as f32;
        let x_ratio = (x as f32 + 0.5) / width;
        let y_ratio = (y as f32 + 0.5) / height;

        let world_x = self.world_min.x + x_ratio * (self.world_max.x - self.world_min.x);
        let world_y = self.world_min.z + y_ratio * (self.world_max.z - self.world_min.z);
        LogicCoord3D::new(world_x, world_y, 0.0)
    }

    pub fn state_to_rgba(state: MinimapFowState) -> [u8; 4] {
        match state {
            MinimapFowState::Hidden => MINIMAP_FOW_RGBA_HIDDEN,
            MinimapFowState::Explored => MINIMAP_FOW_RGBA_EXPLORED,
            MinimapFowState::Visible => MINIMAP_FOW_RGBA_VISIBLE,
        }
    }

    pub fn blend_fow_with_base(base: [u8; 4], state: MinimapFowState) -> [u8; 4] {
        let shade = match state {
            MinimapFowState::Visible => MINIMAP_FOW_SHADE_VISIBLE,
            MinimapFowState::Explored => MINIMAP_FOW_SHADE_EXPLORED,
            MinimapFowState::Hidden => MINIMAP_FOW_SHADE_HIDDEN,
        };
        [
            ((base[0] as f32) * shade).clamp(0.0, 255.0) as u8,
            ((base[1] as f32) * shade).clamp(0.0, 255.0) as u8,
            ((base[2] as f32) * shade).clamp(0.0, 255.0) as u8,
            255,
        ]
    }

    fn soften_fow_edges(&self, states: &[MinimapFowState], texture: &mut [u8]) {
        if self.dimensions.width < 3 || self.dimensions.height < 3 {
            return;
        }

        let width = self.dimensions.width as usize;
        let height = self.dimensions.height as usize;
        let source = texture.to_vec();

        let at = |x: usize, y: usize| -> usize { (y * width + x) * 4 };
        let state_at = |x: usize, y: usize| -> MinimapFowState { states[y * width + x] };

        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let state = state_at(x, y);
                let has_neighbor_state_change =
                    (y - 1..=y + 1).any(|ny| (x - 1..=x + 1).any(|nx| state_at(nx, ny) != state));
                if !has_neighbor_state_change {
                    continue;
                }

                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;
                for ny in (y - 1)..=(y + 1) {
                    for nx in (x - 1)..=(x + 1) {
                        let i = at(nx, ny);
                        sum_r += source[i] as u32;
                        sum_g += source[i + 1] as u32;
                        sum_b += source[i + 2] as u32;
                        count += 1;
                    }
                }

                let i = at(x, y);
                let avg_r = (sum_r / count) as u8;
                let avg_g = (sum_g / count) as u8;
                let avg_b = (sum_b / count) as u8;

                // Keep edge transitions readable but softer than hard per-pixel state bands.
                // Residual: self-weight 3 + neighbor-weight 1 over weight-sum 4.
                texture[i] = (((texture[i] as u16) * MINIMAP_SOFTEN_SELF_WEIGHT
                    + avg_r as u16 * MINIMAP_SOFTEN_NEIGHBOR_WEIGHT)
                    / MINIMAP_SOFTEN_WEIGHT_SUM) as u8;
                texture[i + 1] = (((texture[i + 1] as u16) * MINIMAP_SOFTEN_SELF_WEIGHT
                    + avg_g as u16 * MINIMAP_SOFTEN_NEIGHBOR_WEIGHT)
                    / MINIMAP_SOFTEN_WEIGHT_SUM) as u8;
                texture[i + 2] = (((texture[i + 2] as u16) * MINIMAP_SOFTEN_SELF_WEIGHT
                    + avg_b as u16 * MINIMAP_SOFTEN_NEIGHBOR_WEIGHT)
                    / MINIMAP_SOFTEN_WEIGHT_SUM) as u8;
                texture[i + 3] = 255;
            }
        }
    }

    pub fn set_base_terrain_texture(&mut self, data: Vec<u8>) -> Result<()> {
        let expected = (self.dimensions.width * self.dimensions.height * 4) as usize;
        if data.len() != expected {
            return Err(anyhow!(
                "Minimap base terrain texture size mismatch: expected {}, got {}",
                expected,
                data.len()
            ));
        }
        self.base_terrain_texture = Some(data);
        self.cached_textures.clear();
        Ok(())
    }

    fn cell_to_minimap_state(cell: u8) -> MinimapFowState {
        match cell {
            PresentationFowGrid::CELL_VISIBLE => MinimapFowState::Visible,
            PresentationFowGrid::CELL_EXPLORED => MinimapFowState::Explored,
            _ => MinimapFowState::Hidden,
        }
    }

    fn write_pixel_state(
        &self,
        texture: &mut [u8],
        states: &mut [MinimapFowState],
        index: usize,
        state: MinimapFowState,
    ) {
        states[index] = state;
        let rgba = if self.base_terrain_texture.is_some() {
            let base = [
                texture[index * 4],
                texture[index * 4 + 1],
                texture[index * 4 + 2],
                texture[index * 4 + 3],
            ];
            Self::blend_fow_with_base(base, state)
        } else {
            Self::state_to_rgba(state)
        };
        let base = index * 4;
        texture[base] = rgba[0];
        texture[base + 1] = rgba[1];
        texture[base + 2] = rgba[2];
        texture[base + 3] = rgba[3];
    }

    /// Regenerate minimap FOW from a presentation-owned grid (no live shroud lock).
    ///
    /// Preferred path when `PresentationFrame.fow_grid` is active.
    pub fn regenerate_texture_from_presentation_grid(
        &mut self,
        player_id: usize,
        grid: &PresentationFowGrid,
        _frame: u64,
    ) {
        let pixel_count = (self.dimensions.width * self.dimensions.height) as usize;
        let mut states = vec![MinimapFowState::Visible; pixel_count];
        let mut texture = self
            .base_terrain_texture
            .clone()
            .unwrap_or_else(|| vec![255u8; pixel_count * 4]);

        if grid.active {
            for y in 0..self.dimensions.height {
                for x in 0..self.dimensions.width {
                    let world_pos = self.pixel_to_shroud_world(x, y);
                    // Shroud partition uses world X/Y; minimap maps world Z → shroud Y.
                    let cell = grid.state_at_world_xy(world_pos.x, world_pos.y);
                    let state = Self::cell_to_minimap_state(cell);
                    let index = (y * self.dimensions.width + x) as usize;
                    self.write_pixel_state(&mut texture, &mut states, index, state);
                }
            }
            if self.base_terrain_texture.is_some() {
                self.soften_fow_edges(&states, &mut texture);
            }
        }

        self.cached_pixel_states.insert(player_id, states);
        self.cached_textures.insert(player_id, texture);
    }

    /// Live shroud-manager path (boot / no presentation frame).
    pub fn regenerate_texture(&mut self, player_id: usize, _frame: u64) {
        let pixel_count = (self.dimensions.width * self.dimensions.height) as usize;
        let mut states = vec![MinimapFowState::Visible; pixel_count];
        let mut texture = self
            .base_terrain_texture
            .clone()
            .unwrap_or_else(|| vec![255u8; pixel_count * 4]);

        let player_u32 = match u32::try_from(player_id) {
            Ok(id) => id,
            Err(_) => {
                self.cached_pixel_states.insert(player_id, states);
                self.cached_textures.insert(player_id, texture);
                return;
            }
        };

        let maybe_shroud = get_shroud_manager().lock().ok();
        let use_shroud = maybe_shroud
            .as_ref()
            .map(|shroud| shroud_runtime_active(shroud, player_u32))
            .unwrap_or(false);

        if use_shroud {
            if let Some(shroud) = maybe_shroud.as_ref() {
                for y in 0..self.dimensions.height {
                    for x in 0..self.dimensions.width {
                        let world_pos = self.pixel_to_shroud_world(x, y);
                        let state = if shroud.is_position_visible(player_u32, &world_pos) {
                            MinimapFowState::Visible
                        } else if shroud.is_position_explored(player_u32, &world_pos) {
                            MinimapFowState::Explored
                        } else {
                            MinimapFowState::Hidden
                        };

                        let index = (y * self.dimensions.width + x) as usize;
                        self.write_pixel_state(&mut texture, &mut states, index, state);
                    }
                }
            }
            if self.base_terrain_texture.is_some() {
                self.soften_fow_edges(&states, &mut texture);
            }
        }

        self.cached_pixel_states.insert(player_id, states);
        self.cached_textures.insert(player_id, texture);
    }

    pub fn get_texture_data(&self, player_id: usize) -> Vec<u8> {
        self.cached_textures
            .get(&player_id)
            .cloned()
            .unwrap_or_else(|| self.visible_texture())
    }
}

static MINIMAP_FOW_MANAGER: Lazy<Arc<Mutex<MinimapFowManager>>> = Lazy::new(|| {
    Arc::new(Mutex::new(MinimapFowManager::new(
        MinimapDimensions::standard(),
    )))
});

pub fn get_minimap_fow_manager() -> Arc<Mutex<MinimapFowManager>> {
    Arc::clone(&MINIMAP_FOW_MANAGER)
}

/// Minimap coordinate mapping data
#[derive(Debug, Clone)]
pub struct MinimapCoordinates {
    /// Minimap dimensions in pixels
    pub minimap_width: f32,
    pub minimap_height: f32,

    /// World coordinate bounds
    pub world_min: Vec3,
    pub world_max: Vec3,

    /// Screen position of minimap (top-left corner)
    pub screen_pos: Vec2,
}

impl MinimapCoordinates {
    /// Convert world position to minimap pixel coordinates
    pub fn world_to_minimap(&self, world_pos: Vec3) -> Vec2 {
        let span_x = (self.world_max.x - self.world_min.x).abs().max(1.0e-4);
        let span_z = (self.world_max.z - self.world_min.z).abs().max(1.0e-4);
        let x_ratio = ((world_pos.x - self.world_min.x) / span_x).clamp(0.0, 1.0);
        let z_ratio = ((world_pos.z - self.world_min.z) / span_z).clamp(0.0, 1.0);

        Vec2::new(
            self.screen_pos.x + x_ratio * self.minimap_width,
            self.screen_pos.y + z_ratio * self.minimap_height,
        )
    }

    /// Convert minimap click position to world coordinates
    pub fn minimap_to_world(&self, minimap_pos: Vec2) -> Vec3 {
        let width = self.minimap_width.max(1.0e-4);
        let height = self.minimap_height.max(1.0e-4);
        let x_ratio = ((minimap_pos.x - self.screen_pos.x) / width).clamp(0.0, 1.0);
        let z_ratio = ((minimap_pos.y - self.screen_pos.y) / height).clamp(0.0, 1.0);

        Vec3::new(
            self.world_min.x + x_ratio * (self.world_max.x - self.world_min.x),
            0.0, // Y coordinate will be determined by terrain height
            self.world_min.z + z_ratio * (self.world_max.z - self.world_min.z),
        )
    }

    /// Check if a screen position is within the minimap bounds
    pub fn contains_screen_pos(&self, screen_pos: Vec2) -> bool {
        screen_pos.x >= self.screen_pos.x
            && screen_pos.x <= self.screen_pos.x + self.minimap_width
            && screen_pos.y >= self.screen_pos.y
            && screen_pos.y <= self.screen_pos.y + self.minimap_height
    }
}

/// Minimap Texture Renderer
///
/// Manages GPU texture for minimap FOW visualization
pub struct MinimapTextureRenderer {
    /// WGPU device reference
    device: Arc<Device>,

    /// WGPU queue reference
    queue: Arc<Queue>,

    /// Current FOW texture on GPU
    texture: Option<Texture>,

    /// Texture view for binding
    texture_view: Option<TextureView>,

    /// Framework-neutral texture ID for UI rendering
    ui_texture_id: Option<UiTextureId>,

    /// Minimap dimensions
    dimensions: MinimapDimensions,

    /// Current player ID being rendered
    current_player_id: usize,

    /// Frame counter for update tracking
    last_update_frame: u64,

    /// Coordinate mapping data
    coordinates: MinimapCoordinates,

    /// Texture format (RGBA8)
    texture_format: TextureFormat,

    /// Force the next update call to regenerate/upload regardless of frame cadence.
    force_refresh: bool,
}

impl MinimapTextureRenderer {
    /// Create new minimap texture renderer
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        dimensions: MinimapDimensions,
        world_bounds: (Vec3, Vec3),
    ) -> Result<Self> {
        debug!(
            "Creating MinimapTextureRenderer with dimensions {}x{}",
            dimensions.width, dimensions.height
        );

        let coordinates = MinimapCoordinates {
            minimap_width: dimensions.width as f32,
            minimap_height: dimensions.height as f32,
            world_min: world_bounds.0,
            world_max: world_bounds.1,
            screen_pos: Vec2::new(10.0, 10.0), // Default position, will be updated
        };

        let mut renderer = Self {
            device,
            queue,
            texture: None,
            texture_view: None,
            ui_texture_id: None,
            dimensions,
            current_player_id: 0,
            last_update_frame: 0,
            coordinates,
            texture_format: TextureFormat::Rgba8Unorm,
            force_refresh: true,
        };

        {
            let manager = get_minimap_fow_manager();
            match manager.lock() {
                Ok(mut fow) => {
                    fow.ensure_dimensions(dimensions);
                    fow.set_world_bounds(world_bounds.0, world_bounds.1);
                }
                Err(err) => {
                    debug!("Failed to lock minimap FOW manager during init: {}", err);
                }
            };
        }

        // Create initial texture
        renderer.create_texture()?;

        Ok(renderer)
    }

    /// Create or recreate the GPU texture
    fn create_texture(&mut self) -> Result<()> {
        let texture_desc = TextureDescriptor {
            label: Some("Minimap FOW Texture"),
            size: wgpu::Extent3d {
                width: self.dimensions.width,
                height: self.dimensions.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.texture_format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = self.device.create_texture(&texture_desc);
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.texture = Some(texture);
        self.texture_view = Some(texture_view);
        self.ui_texture_id = None;

        trace!(
            "Created minimap texture {}x{}",
            self.dimensions.width,
            self.dimensions.height
        );
        Ok(())
    }

    /// Update texture from FOW manager data (live shroud path).
    pub fn update_texture_from_fow(&mut self, player_id: usize, frame_number: u64) -> Result<()> {
        self.update_texture_from_fow_with_grid(player_id, frame_number, None)
    }

    /// Update minimap FOW texture, preferring a presentation-owned grid when active.
    ///
    /// When `fow_grid` is active, regenerates from the snapshot only (no mid-render
    /// shroud lock). Falls back to live `ShroudManager` queries otherwise.
    pub fn update_texture_from_fow_with_grid(
        &mut self,
        player_id: usize,
        frame_number: u64,
        fow_grid: Option<&PresentationFowGrid>,
    ) -> Result<()> {
        // Only update if player changed or enough frames have passed
        if self.force_refresh
            || self.current_player_id != player_id
            || frame_number > self.last_update_frame + 2
        {
            self.current_player_id = player_id;
            self.last_update_frame = frame_number;

            // Get FOW manager and regenerate texture
            let fow_manager = get_minimap_fow_manager();
            let mut fow = fow_manager
                .lock()
                .map_err(|e| anyhow!("Failed to lock FOW manager: {}", e))?;

            // Regenerate texture for current player
            fow.ensure_dimensions(self.dimensions);
            fow.set_world_bounds(self.coordinates.world_min, self.coordinates.world_max);
            if let Some(grid) = fow_grid.filter(|g| g.active) {
                fow.regenerate_texture_from_presentation_grid(player_id, grid, frame_number);
            } else {
                fow.regenerate_texture(player_id, frame_number);
            }

            // Get texture data
            let texture_data = fow.get_texture_data(player_id);

            // Upload to GPU
            self.upload_texture_to_gpu(&texture_data)?;

            trace!(
                "Updated minimap FOW texture for player {} at frame {} (presentation_grid={})",
                player_id,
                frame_number,
                fow_grid.map(|g| g.active).unwrap_or(false)
            );
            self.force_refresh = false;
        }

        Ok(())
    }

    /// Upload texture data to GPU
    fn upload_texture_to_gpu(&mut self, data: &[u8]) -> Result<()> {
        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| anyhow!("No texture created"))?;

        // Ensure data size is correct
        let expected_size = (self.dimensions.width * self.dimensions.height * 4) as usize;
        if data.len() != expected_size {
            return Err(anyhow!(
                "Texture data size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            ));
        }

        // Upload texture data to GPU
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.dimensions.width * 4),
                rows_per_image: Some(self.dimensions.height),
            },
            wgpu::Extent3d {
                width: self.dimensions.width,
                height: self.dimensions.height,
                depth_or_array_layers: 1,
            },
        );

        trace!("Uploaded {} bytes to GPU texture", data.len());
        Ok(())
    }

    /// Bind texture to the active UI renderer.
    pub fn bind_to_ui_renderer<T: UiTextureRegistrar>(
        &mut self,
        renderer: &mut T,
    ) -> Result<UiTextureId> {
        if let Some(texture_view) = &self.texture_view {
            if let Some(prev) = self.ui_texture_id.take() {
                renderer.free_texture(prev);
            }

            let ui_texture_id = renderer.register_native_texture(
                self.device.as_ref(),
                texture_view,
                wgpu::FilterMode::Linear,
            );

            self.ui_texture_id = Some(ui_texture_id);
            trace!(
                "Bound minimap texture to UI renderer with ID {:?}",
                ui_texture_id
            );
            Ok(ui_texture_id)
        } else {
            Err(anyhow!("No texture view available for binding"))
        }
    }

    /// Get current framework-neutral texture ID.
    pub fn get_texture_id(&self) -> Option<UiTextureId> {
        self.ui_texture_id
    }

    pub fn dimensions(&self) -> MinimapDimensions {
        self.dimensions
    }

    pub fn set_base_terrain_texture(&mut self, texture_data: Vec<u8>) -> Result<()> {
        let manager = get_minimap_fow_manager();
        let mut fow = manager
            .lock()
            .map_err(|e| anyhow!("Failed to lock FOW manager for base terrain: {}", e))?;
        fow.ensure_dimensions(self.dimensions);
        fow.set_world_bounds(self.coordinates.world_min, self.coordinates.world_max);
        fow.set_base_terrain_texture(texture_data)?;
        self.force_refresh = true;
        Ok(())
    }

    /// Update the minimap's on-screen rectangle so click conversion matches the UI.
    pub fn set_screen_rect(&mut self, top_left: Vec2, size: Vec2) {
        self.coordinates.screen_pos = top_left;
        self.coordinates.minimap_width = size.x.max(1.0);
        self.coordinates.minimap_height = size.y.max(1.0);
        self.force_refresh = true;
    }

    /// Get coordinate mapping
    pub fn get_coordinates(&self) -> &MinimapCoordinates {
        &self.coordinates
    }

    /// Update world bounds used for conversions without rebuilding the renderer.
    pub fn set_world_bounds(&mut self, world_bounds: (Vec3, Vec3)) {
        self.coordinates.world_min = world_bounds.0;
        self.coordinates.world_max = world_bounds.1;
        self.force_refresh = true;
        let manager = get_minimap_fow_manager();
        match manager.lock() {
            Ok(mut fow) => {
                fow.set_world_bounds(world_bounds.0, world_bounds.1);
            }
            Err(err) => {
                debug!(
                    "Failed to lock minimap FOW manager for world-bounds update: {}",
                    err
                );
            }
        };
    }

    /// Convert screen position to world coordinates (for minimap clicks)
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Option<Vec3> {
        if self.coordinates.contains_screen_pos(screen_pos) {
            Some(self.coordinates.minimap_to_world(screen_pos))
        } else {
            None
        }
    }

    /// Convert world position to minimap screen coordinates (for unit dots)
    pub fn world_to_screen(&self, world_pos: Vec3) -> Vec2 {
        self.coordinates.world_to_minimap(world_pos)
    }

    /// Check if a world position is visible based on FOW state
    pub fn is_position_visible(&self, world_pos: Vec3) -> Result<bool> {
        // Convert world position to minimap pixel
        let minimap_pos = self.world_to_screen(world_pos);
        let pixel_x = (minimap_pos.x - self.coordinates.screen_pos.x) as u32;
        let pixel_y = (minimap_pos.y - self.coordinates.screen_pos.y) as u32;

        // Check bounds
        if pixel_x >= self.dimensions.width || pixel_y >= self.dimensions.height {
            return Ok(false);
        }

        // Query FOW state
        let fow_manager = get_minimap_fow_manager();
        let fow = fow_manager
            .lock()
            .map_err(|e| anyhow!("Failed to lock FOW manager: {}", e))?;

        let state = fow.get_pixel_state(self.current_player_id, pixel_x, pixel_y);

        // Position is visible if explored or currently visible
        Ok(matches!(
            state,
            MinimapFowState::Explored | MinimapFowState::Visible
        ))
    }

    /// Check if a world position is currently visible (not just explored)
    pub fn is_position_currently_visible(&self, world_pos: Vec3) -> Result<bool> {
        // Convert world position to minimap pixel
        let minimap_pos = self.world_to_screen(world_pos);
        let pixel_x = (minimap_pos.x - self.coordinates.screen_pos.x) as u32;
        let pixel_y = (minimap_pos.y - self.coordinates.screen_pos.y) as u32;

        // Check bounds
        if pixel_x >= self.dimensions.width || pixel_y >= self.dimensions.height {
            return Ok(false);
        }

        // Query FOW state
        let fow_manager = get_minimap_fow_manager();
        let fow = fow_manager
            .lock()
            .map_err(|e| anyhow!("Failed to lock FOW manager: {}", e))?;

        let state = fow.get_pixel_state(self.current_player_id, pixel_x, pixel_y);

        // Position is currently visible only if fully visible
        Ok(matches!(state, MinimapFowState::Visible))
    }
}

/// Performance metrics for minimap rendering
#[derive(Debug, Default)]
pub struct MinimapRenderMetrics {
    /// Time taken for texture update (microseconds)
    pub texture_update_us: u64,

    /// Time taken for GPU upload (microseconds)
    pub gpu_upload_us: u64,

    /// Total minimap update time (microseconds)
    pub total_update_us: u64,

    /// Number of texture updates this second
    pub updates_per_second: u32,
}

impl MinimapRenderMetrics {
    /// Check if performance is within target (<1ms texture, <2ms total)
    pub fn is_within_target(&self) -> bool {
        self.texture_update_us < 1000 && self.total_update_us < 2000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimap_fow_manager_singleton_is_shared() {
        let manager_a = get_minimap_fow_manager();
        let manager_b = get_minimap_fow_manager();
        assert!(Arc::ptr_eq(&manager_a, &manager_b));
    }

    #[test]
    fn test_minimap_fow_manager_tracks_dimensions() {
        let manager = get_minimap_fow_manager();
        let mut manager = manager.lock().unwrap_or_else(|e| e.into_inner());

        manager.ensure_dimensions(MinimapDimensions {
            width: 8,
            height: 8,
        });
        manager.regenerate_texture(0, 0);

        let data = manager.get_texture_data(0);
        assert_eq!(data.len(), 8 * 8 * 4);
    }

    #[test]
    fn test_coordinate_mapping() {
        let coords = MinimapCoordinates {
            minimap_width: 256.0,
            minimap_height: 256.0,
            world_min: Vec3::new(0.0, 0.0, 0.0),
            world_max: Vec3::new(1024.0, 0.0, 1024.0),
            screen_pos: Vec2::new(10.0, 10.0),
        };

        // Test world to minimap
        let world_pos = Vec3::new(512.0, 0.0, 512.0); // Center of world
        let minimap_pos = coords.world_to_minimap(world_pos);
        assert_eq!(minimap_pos.x, 138.0); // 10 + 128
        assert_eq!(minimap_pos.y, 138.0); // 10 + 128

        // Test minimap to world
        let minimap_click = Vec2::new(138.0, 138.0);
        let world_result = coords.minimap_to_world(minimap_click);
        assert!((world_result.x - 512.0).abs() < 0.1);
        assert!((world_result.z - 512.0).abs() < 0.1);

        // Test bounds checking
        assert!(coords.contains_screen_pos(Vec2::new(50.0, 50.0)));
        assert!(!coords.contains_screen_pos(Vec2::new(300.0, 300.0)));
    }

    #[test]
    fn test_coordinate_mapping_clamps_out_of_bounds_samples() {
        let coords = MinimapCoordinates {
            minimap_width: 128.0,
            minimap_height: 128.0,
            world_min: Vec3::new(0.0, 0.0, 0.0),
            world_max: Vec3::new(1000.0, 0.0, 1000.0),
            screen_pos: Vec2::new(10.0, 10.0),
        };

        let off_world = Vec3::new(-500.0, 0.0, 1800.0);
        let px = coords.world_to_minimap(off_world);
        assert!((px.x - 10.0).abs() < 0.1);
        assert!((px.y - 138.0).abs() < 0.1);

        let off_screen = Vec2::new(-50.0, 400.0);
        let world = coords.minimap_to_world(off_screen);
        assert!((world.x - 0.0).abs() < 0.1);
        assert!((world.z - 1000.0).abs() < 0.1);
    }

    #[test]
    fn test_blend_fow_with_base_darkens_non_visible_states() {
        let base = [200, 180, 160, 255];
        let visible = MinimapFowManager::blend_fow_with_base(base, MinimapFowState::Visible);
        let explored = MinimapFowManager::blend_fow_with_base(base, MinimapFowState::Explored);
        let hidden = MinimapFowManager::blend_fow_with_base(base, MinimapFowState::Hidden);

        assert_eq!(visible, [200, 180, 160, 255]);
        assert!(explored[0] < visible[0]);
        assert!(explored[1] < visible[1]);
        assert!(hidden[0] < explored[0]);
        assert!(hidden[1] < explored[1]);
    }

    #[test]
    fn minimap_residual_pack_wave79_honesty() {
        assert!(honesty_minimap_residual_pack_wave79());
    }

    #[test]
    fn test_soften_fow_edges_blends_transition_pixels() {
        let manager = MinimapFowManager::new(MinimapDimensions {
            width: 3,
            height: 3,
        });
        let mut states = vec![MinimapFowState::Visible; 9];
        states[4] = MinimapFowState::Hidden;

        let mut texture = vec![200u8; 3 * 3 * 4];
        texture[4 * 4] = 20;
        texture[4 * 4 + 1] = 20;
        texture[4 * 4 + 2] = 20;
        texture[4 * 4 + 3] = 255;

        manager.soften_fow_edges(&states, &mut texture);

        let center = 4 * 4;
        assert!(texture[center] > 20);
        assert!(texture[center] < 200);
    }
}
