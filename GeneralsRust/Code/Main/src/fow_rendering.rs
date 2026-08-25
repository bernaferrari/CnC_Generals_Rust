//! FOW (Fog of War) Rendering Integration
//!
//! This module bridges the FOW system (shroud manager) with the rendering pipeline.
//! It provides visibility queries for objects and updates shader uniforms with
//! visibility information for per-object rendering.
//!
//! ## Integration Points:
//! - During rendering, query `get_object_visibility()` to get FOW state
//! - Pass `visibility_alpha` and `is_explored` to shader uniforms
//! - Supports per-player and per-object visibility queries
//! - `PresentationFowGrid` freezes cell-grid state for terrain / minimap overlay
//!   so GPU upload does not re-lock the shroud manager mid-render
//!
//! ## Architecture:
//! ```text
//! ShroudManager::can_see_object(player, obj_id)
//!    ↓
//! FOWRenderingBridge::get_object_visibility()
//!    ↓
//! Shader uniform (visibility_alpha, is_explored)
//!    ↓
//! Fragment shader applies FOW effects
//!
//! ShroudManager::snapshot_grid_for_player(local)
//!    ↓
//! PresentationFowGrid (owned cells)
//!    ↓
//! FowTerrainOverlay::update_texture / minimap R8-RGBA
//! ```
//!
//! Fail-closed claim: unit FOW + compact cell-grid snapshot for local player.
//! Not full SAGE dirty-rect streaming / multi-player simultaneous grid parity.

use crate::game_logic::ObjectId as ObjectID;
use gamelogic::system::shroud_manager::{ShroudState, get_shroud_manager};
use log::{trace, warn};

fn shroud_runtime_active(
    shroud_mgr: &gamelogic::system::shroud_manager::ShroudManager,
    player_id: u32,
) -> bool {
    // Host residual: ShroudManager::update() queries the gamelogic ObjectManager.
    // Main GameLogic objects are not in that registry on the default host path, so an
    // "update" can clear player_visible_objects and leave them empty while still bumping
    // last_update_frame. That must NOT activate FOW filtering (would hide the whole world).
    // Fail-open unless this player has real visible/explored object membership.
    !shroud_mgr.get_visible_objects(player_id).is_empty()
        || !shroud_mgr.get_explored_objects(player_id).is_empty()
}

/// FOW visibility state for rendering an object
///
/// Snapshot-friendly (Copy + Serialize) so `PresentationFrame` can own unit FOW
/// without re-locking the shroud manager mid-render.
/// Serialize tests that mutate the process-wide shroud manager / FOW bridge.
pub fn shroud_test_isolation_lock() -> &'static std::sync::Mutex<()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObjectVisibility {
    /// Alpha blend factor (0.0 = hidden, 1.0 = fully visible)
    pub visibility_alpha: f32,
    /// Explored state (1.0 = explored, 0.0 = unexplored)
    pub is_explored: f32,
    /// Falloff/gradient strength for smooth transitions
    pub visibility_falloff: f32,
}

impl Default for ObjectVisibility {
    fn default() -> Self {
        Self {
            visibility_alpha: 1.0,   // Default: fully visible
            is_explored: 1.0,        // Default: explored
            visibility_falloff: 1.0, // Default: sharp transition
        }
    }
}

/// Compact presentation-owned FOW cell grid for the local player.
///
/// Built once per logic frame into `PresentationFrame` so terrain overlay /
/// minimap texture update can consume frozen cells without mid-render shroud
/// re-queries. Values are SAGE-style buckets matching [`ShroudState`].
///
/// Fail-closed: full grid copy (not dirty rects); not full SAGE multi-layer
/// shroud texture streaming parity.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PresentationFowGrid {
    pub width: u32,
    pub height: u32,
    /// World-space origin of logical cell `(0, 0)` on the C++ shroud X/Y
    /// ground plane.  The current `ShroudGrid` starts at zero, but carrying
    /// this explicitly prevents a future map/sub-grid origin from being lost
    /// before the renderer projects the frozen texture onto Rust X/Z ground.
    #[serde(default)]
    pub world_origin_xy: [f32; 2],
    /// World units per cell (matches shroud partition cell size, typically 50).
    pub cell_size: f32,
    /// Row-major `y * width + x`: 0=Hidden, 1=Explored/Fogged, 2=Visible.
    pub cells: Vec<u8>,
    /// True when `cells` came from an initialized shroud grid.
    /// When false, consumers should fail-open (fully visible / no overlay).
    pub active: bool,
}

impl Default for PresentationFowGrid {
    fn default() -> Self {
        Self::inactive()
    }
}

impl PresentationFowGrid {
    pub const CELL_HIDDEN: u8 = 0;
    pub const CELL_EXPLORED: u8 = 1;
    pub const CELL_VISIBLE: u8 = 2;

    /// Terrain overlay R8: shrouded.
    pub const R8_SHROUDED: u8 = 0;
    /// Terrain overlay R8: fogged / explored.
    pub const R8_FOGGED: u8 = 128;
    /// Terrain overlay R8: clear / visible.
    pub const R8_VISIBLE: u8 = 255;

    /// Empty inactive grid — fail-open for consumers (no texture upload).
    pub fn inactive() -> Self {
        Self {
            width: 0,
            height: 0,
            world_origin_xy: [0.0, 0.0],
            cell_size: 50.0,
            cells: Vec::new(),
            active: false,
        }
    }

    /// Fully visible grid of the given size (shell-map bypass / observer).
    pub fn fully_visible(width: u32, height: u32, cell_size: f32) -> Self {
        Self::fully_visible_at_origin(width, height, [0.0, 0.0], cell_size)
    }

    /// Fully visible grid with an explicit C++ shroud-plane origin.
    pub fn fully_visible_at_origin(
        width: u32,
        height: u32,
        world_origin_xy: [f32; 2],
        cell_size: f32,
    ) -> Self {
        let len = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            world_origin_xy: Self::sanitize_world_origin(world_origin_xy),
            cell_size: cell_size.max(1.0),
            cells: vec![Self::CELL_VISIBLE; len],
            active: true,
        }
    }

    /// Build from shroud manager snapshot bytes (0/1/2 per cell).
    pub fn from_snapshot(width: u32, height: u32, cell_size: f32, cells: Vec<u8>) -> Self {
        Self::from_snapshot_at_origin(width, height, [0.0, 0.0], cell_size, cells)
    }

    /// Build from shroud manager snapshot bytes with an explicit C++ shroud
    /// ground-plane origin.  The live Main shroud grid currently uses `(0, 0)`,
    /// while this preserves the projection contract for map-relative grids.
    pub fn from_snapshot_at_origin(
        width: u32,
        height: u32,
        world_origin_xy: [f32; 2],
        cell_size: f32,
        cells: Vec<u8>,
    ) -> Self {
        let expected = (width as usize).saturating_mul(height as usize);
        let mut cells = cells;
        if cells.len() != expected {
            // Fail-closed sizing: pad/truncate rather than panic at snapshot time.
            cells.resize(expected, Self::CELL_HIDDEN);
        }
        Self {
            width,
            height,
            world_origin_xy: Self::sanitize_world_origin(world_origin_xy),
            cell_size: cell_size.max(1.0),
            cells,
            active: expected > 0,
        }
    }

    #[inline]
    fn sanitize_world_origin(world_origin_xy: [f32; 2]) -> [f32; 2] {
        [
            world_origin_xy[0]
                .is_finite()
                .then_some(world_origin_xy[0])
                .unwrap_or(0.0),
            world_origin_xy[1]
                .is_finite()
                .then_some(world_origin_xy[1])
                .unwrap_or(0.0),
        ]
    }

    #[inline]
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Cell state at grid coords, or Hidden when OOB / inactive.
    #[inline]
    pub fn cell_at(&self, x: u32, y: u32) -> u8 {
        if !self.active || x >= self.width || y >= self.height {
            return if self.active {
                Self::CELL_HIDDEN
            } else {
                Self::CELL_VISIBLE
            };
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.cells.get(idx).copied().unwrap_or(Self::CELL_HIDDEN)
    }

    /// Sample cell using shroud world axes (X,Y) — matches `ShroudGrid::world_to_grid`.
    pub fn state_at_world_xy(&self, world_x: f32, world_y: f32) -> u8 {
        if !self.active || self.cell_size <= 0.0 {
            return Self::CELL_VISIBLE;
        }
        let gx = ((world_x - self.world_origin_xy[0]) / self.cell_size).floor() as i32;
        let gy = ((world_y - self.world_origin_xy[1]) / self.cell_size).floor() as i32;
        if gx < 0 || gy < 0 {
            return Self::CELL_HIDDEN;
        }
        self.cell_at(gx as u32, gy as u32)
    }

    /// Encode one cell to R8 for `FowTerrainOverlay::update_texture`.
    #[inline]
    pub fn cell_to_r8(cell: u8) -> u8 {
        match cell {
            Self::CELL_VISIBLE => Self::R8_VISIBLE,
            Self::CELL_EXPLORED => Self::R8_FOGGED,
            _ => Self::R8_SHROUDED,
        }
    }

    /// Full R8 texture payload (length = width * height) for terrain FOW overlay.
    ///
    /// Inactive grids return empty — callers should skip upload / fail-open.
    pub fn to_r8_texture(&self) -> Vec<u8> {
        if !self.active || self.cells.is_empty() {
            return Vec::new();
        }
        self.cells.iter().map(|&c| Self::cell_to_r8(c)).collect()
    }

    /// Map cell state to object-style visibility (for tests / shared encoding).
    pub fn cell_to_object_visibility(cell: u8) -> ObjectVisibility {
        match cell {
            Self::CELL_VISIBLE => ObjectVisibility::VISIBLE,
            Self::CELL_EXPLORED => ObjectVisibility::FOGGED,
            _ => ObjectVisibility::HIDDEN,
        }
    }

    /// Lightweight fingerprint for dual-run presentation determinism.
    pub fn content_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.width.hash(&mut h);
        self.height.hash(&mut h);
        self.active.hash(&mut h);
        self.world_origin_xy[0].to_bits().hash(&mut h);
        self.world_origin_xy[1].to_bits().hash(&mut h);
        self.cell_size.to_bits().hash(&mut h);
        self.cells.len().hash(&mut h);
        // Hash all cells for strict grid consistency (compact maps stay cheap).
        for c in &self.cells {
            c.hash(&mut h);
        }
        h.finish()
    }
}

/// Immutable color and level metadata paired with a projected shroud texture.
///
/// C++ `W3DShroud::setShroudLevel` multiplies its logical level by
/// `GlobalData::m_shroudColor`; `W3DShroudMaterialPass` then uses the result as
/// a multiplicative material pass.  WGPU keeps the level map as `R8Unorm`, so
/// the color and source alpha levels must travel alongside the frozen texture
/// for the later material pass instead of being guessed from object visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectedShroudMetadata {
    /// C++ `GlobalData::m_shroudColor`, quantized for the future material uniform.
    pub shroud_color_rgb: [u8; 3],
    /// C++ `ClearAlpha`: logical fully clear shroud level.
    pub clear_alpha: u8,
    /// C++ `FogAlpha`: logical explored/fogged shroud level.
    pub fog_alpha: u8,
    /// C++ `ShroudAlpha`: minimum logical level for hidden cells.
    pub shroud_alpha: u8,
    /// C++ `W3DShroud::m_boderShroudLevel` (sic), used for the padded border.
    pub border_alpha: u8,
}

impl Default for ProjectedShroudMetadata {
    fn default() -> Self {
        // GlobalData.cpp:874-877: white multiplicative color, 255 clear,
        // 127 fog, 0 shroud.  The border starts at ShroudAlpha in
        // W3DShroud.cpp:57-58.
        Self {
            shroud_color_rgb: [255, 255, 255],
            clear_alpha: 255,
            fog_alpha: 127,
            shroud_alpha: 0,
            border_alpha: 0,
        }
    }
}

impl ProjectedShroudMetadata {
    /// Freeze configured source shroud settings while the presentation frame is
    /// built.  This accepts Common's immutable read value; WGPU never reads it.
    pub fn from_global_data(global: &game_engine::common::global_data::GlobalData) -> Self {
        Self::from_global_data_with_border_override(global, None)
    }

    /// Freeze configured source shroud settings with the latest script-owned
    /// `W3DShroud::setBorderShroudLevel` value when one exists.  An absent
    /// override retains the C++ constructor's `ShroudAlpha` border default.
    pub fn from_global_data_with_border_override(
        global: &game_engine::common::global_data::GlobalData,
        border_alpha_override: Option<u8>,
    ) -> Self {
        Self {
            shroud_color_rgb: global.shroud_color.map(Self::color_channel_to_u8),
            clear_alpha: global.clear_alpha,
            fog_alpha: global.fog_alpha,
            shroud_alpha: global.shroud_alpha,
            // C++ initializes the border to ShroudAlpha and changes it only
            // through `setBorderShroudLevel` script actions.
            border_alpha: border_alpha_override.unwrap_or(global.shroud_alpha),
        }
    }

    #[inline]
    fn color_channel_to_u8(channel: f32) -> u8 {
        if !channel.is_finite() {
            return 255;
        }
        (channel.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// C++ clamps every written cell level to at least `ShroudAlpha` before it
    /// copies the source surface to the padded destination texture.
    #[inline]
    pub fn encoded_level_for_cell(&self, cell: u8) -> u8 {
        let level = match cell {
            PresentationFowGrid::CELL_VISIBLE => self.clear_alpha,
            PresentationFowGrid::CELL_EXPLORED => self.fog_alpha,
            _ => self.shroud_alpha,
        };
        level.max(self.shroud_alpha)
    }

    #[inline]
    pub fn encoded_border_level(&self) -> u8 {
        self.border_alpha.max(self.shroud_alpha)
    }
}

/// Immutable, presentation-owned W3D shroud projection input.
///
/// The source `W3DShroud::render` copies logical cells to destination `(1, 1)`
/// and reserves a one-texel border on every edge (`W3DShroud.cpp:673-698`).
/// Before allocating, `W3DShroud::init` adds that border and calls
/// `TextureLoader::Validate_Texture_Size`, which rounds the destination to
/// powers of two and keeps its aspect ratio at most 8:1.  This stores that
/// validated, row-major R8 layout, including every trailing padding texel.
/// It deliberately contains no live shroud manager or GameLogic reference;
/// renderer code can only consume the frozen payload, projection geometry, and
/// material metadata.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectedShroudSnapshot {
    /// Logical shroud cell dimensions before the source-shaped border is added.
    pub grid_width: u32,
    pub grid_height: u32,
    /// Validated destination texture dimensions.  These cover the `grid + 2`
    /// border and may be larger because W3D rounds them to compatible powers
    /// of two and corrects an over-8:1 aspect ratio.
    pub texture_width: u32,
    pub texture_height: u32,
    /// C++ X/Y world origin of logical grid cell `(0, 0)`.
    pub draw_origin_xy: [f32; 2],
    /// C++ X/Y world size of a logical shroud cell.  Main's current partition
    /// is square, but storing both axes matches W3DShroud's width/height API.
    pub cell_size_xy: [f32; 2],
    /// Frozen tint and logical levels used by the later multiplicative pass.
    pub metadata: ProjectedShroudMetadata,
    /// Row-major `texture_height * texture_width` R8 levels, including border.
    pub texels: Vec<u8>,
    /// False means no initialized shroud source; consumers must fail open and
    /// release any prior GPU resource rather than reusing stale fog.
    pub active: bool,
}

impl Default for ProjectedShroudSnapshot {
    fn default() -> Self {
        Self::inactive()
    }
}

impl ProjectedShroudSnapshot {
    pub const BORDER_TEXELS_PER_EDGE: u32 = 1;
    /// `TextureLoader::Validate_Texture_Size` grows the smaller power-of-two
    /// dimension until no texture side exceeds the other by this ratio.
    pub const W3D_MAX_TEXTURE_ASPECT_RATIO: u32 = 8;
    /// Fixed presentation compatibility ceiling for this CPU-only snapshot.
    ///
    /// W3D derives a device-dependent cap from `D3DCAPS8`, but a presentation
    /// frame must not query a live WGPU device while it is being frozen.  The
    /// original shroud assumes a largest terrain dimension of 1024
    /// (`W3DShroud.cpp:48`); adding the source border and validating it needs
    /// at most 2048 texels per side.  Values beyond that fail closed instead
    /// of truncating the logical `(1, 1)` copy into an unsafe layout.
    pub const W3D_COMPAT_TEXTURE_DIMENSION_CAP: u32 = 2048;

    pub fn inactive() -> Self {
        Self {
            grid_width: 0,
            grid_height: 0,
            texture_width: 0,
            texture_height: 0,
            draw_origin_xy: [0.0, 0.0],
            cell_size_xy: [50.0, 50.0],
            metadata: ProjectedShroudMetadata::default(),
            texels: Vec::new(),
            active: false,
        }
    }

    /// Source-shaped destination allocation extent for a logical shroud grid.
    ///
    /// This follows the non-device-specific portion of
    /// `TextureLoader::Validate_Texture_Size`: add W3D's one-texel border on
    /// each side, round both dimensions up to powers of two, then expand the
    /// smaller side until the ratio is no greater than 8:1.  Unlike the C++
    /// runtime's legacy D3D-cap clamp, this frozen CPU path never truncates a
    /// grid: an extent outside the fixed compatibility ceiling is rejected.
    pub fn validated_texture_extent_for_grid(
        grid_width: u32,
        grid_height: u32,
    ) -> Option<(u32, u32)> {
        let border_extent = Self::BORDER_TEXELS_PER_EDGE.checked_mul(2)?;
        let padded_width = grid_width.checked_add(border_extent)?;
        let padded_height = grid_height.checked_add(border_extent)?;
        let mut texture_width = padded_width.checked_next_power_of_two()?;
        let mut texture_height = padded_height.checked_next_power_of_two()?;

        // `TextureLoader::Validate_Texture_Size` compares powers of two, so
        // integer division reproduces its `while (large / small > 8)` logic.
        while texture_width / texture_height > Self::W3D_MAX_TEXTURE_ASPECT_RATIO {
            texture_height = texture_height.checked_mul(2)?;
        }
        while texture_height / texture_width > Self::W3D_MAX_TEXTURE_ASPECT_RATIO {
            texture_width = texture_width.checked_mul(2)?;
        }

        (texture_width <= Self::W3D_COMPAT_TEXTURE_DIMENSION_CAP
            && texture_height <= Self::W3D_COMPAT_TEXTURE_DIMENSION_CAP)
            .then_some((texture_width, texture_height))
    }

    /// Freeze a full R8 projection texture from an already-frozen presentation
    /// grid.  This is intentionally a pure conversion: it must be called at
    /// presentation-build time, never from a WGPU draw path.
    pub fn from_grid(grid: &PresentationFowGrid, metadata: ProjectedShroudMetadata) -> Self {
        let Some(grid_len) = (grid.width as usize).checked_mul(grid.height as usize) else {
            return Self::inactive();
        };
        if !grid.active
            || grid.width == 0
            || grid.height == 0
            || grid.cells.len() != grid_len
            || !grid.cell_size.is_finite()
            || grid.cell_size <= 0.0
        {
            return Self::inactive();
        }

        let Some((texture_width, texture_height)) =
            Self::validated_texture_extent_for_grid(grid.width, grid.height)
        else {
            return Self::inactive();
        };
        let Some(texture_len) = (texture_width as usize).checked_mul(texture_height as usize)
        else {
            return Self::inactive();
        };

        // W3DShroud::fillBorderShroudData clears the whole destination first,
        // then `render` copies logical rows to (1, 1).  Initializing all pixels
        // to the border value reproduces that ordering without a GPU clear pass.
        let mut texels = vec![metadata.encoded_border_level(); texture_len];
        let source_width = grid.width as usize;
        let destination_width = texture_width as usize;
        for y in 0..grid.height as usize {
            let source_row = y * source_width;
            let destination_row = (y + Self::BORDER_TEXELS_PER_EDGE as usize) * destination_width
                + Self::BORDER_TEXELS_PER_EDGE as usize;
            for x in 0..source_width {
                texels[destination_row + x] =
                    metadata.encoded_level_for_cell(grid.cells[source_row + x]);
            }
        }

        Self {
            grid_width: grid.width,
            grid_height: grid.height,
            texture_width,
            texture_height,
            draw_origin_xy: [
                grid.world_origin_xy[0]
                    .is_finite()
                    .then_some(grid.world_origin_xy[0])
                    .unwrap_or(0.0),
                grid.world_origin_xy[1]
                    .is_finite()
                    .then_some(grid.world_origin_xy[1])
                    .unwrap_or(0.0),
            ],
            cell_size_xy: [grid.cell_size, grid.cell_size],
            metadata,
            texels,
            active: true,
        }
    }

    #[inline]
    pub fn texture_extent(&self) -> Option<(u32, u32)> {
        let expected = (self.texture_width as usize).checked_mul(self.texture_height as usize)?;
        (self.active
            && self.texture_width > 0
            && self.texture_height > 0
            && Self::validated_texture_extent_for_grid(self.grid_width, self.grid_height)
                == Some((self.texture_width, self.texture_height))
            && self.texels.len() == expected)
            .then_some((self.texture_width, self.texture_height))
    }

    #[inline]
    pub fn is_uploadable(&self) -> bool {
        self.texture_extent().is_some()
    }

    #[inline]
    pub fn texel_at(&self, x: u32, y: u32) -> Option<u8> {
        let (width, height) = self.texture_extent()?;
        if x >= width || y >= height {
            return None;
        }
        self.texels
            .get((y as usize) * width as usize + x as usize)
            .copied()
    }

    /// Exact C++ ground-plane projection equation from
    /// `ShroudTextureShader::set`: `((world - draw_origin) + cell_size) /
    /// (cell_size * texture_extent)`.  The `+ cell_size` selects the first
    /// interior texel after W3D's reserved border.  Values are intentionally
    /// not clamped here; the source texture uses clamp addressing at sampling.
    pub fn uv_for_cpp_world_xy(&self, world_x: f32, world_y: f32) -> Option<[f32; 2]> {
        let (texture_width, texture_height) = self.texture_extent()?;
        let [cell_width, cell_height] = self.cell_size_xy;
        if !world_x.is_finite()
            || !world_y.is_finite()
            || !self.draw_origin_xy[0].is_finite()
            || !self.draw_origin_xy[1].is_finite()
            || !cell_width.is_finite()
            || !cell_height.is_finite()
            || cell_width <= 0.0
            || cell_height <= 0.0
        {
            return None;
        }
        Some([
            (world_x - self.draw_origin_xy[0] + cell_width) / (cell_width * texture_width as f32),
            (world_y - self.draw_origin_xy[1] + cell_height)
                / (cell_height * texture_height as f32),
        ])
    }

    /// Rust render world uses X/Z as the ground plane; C++ W3D shroud uses
    /// X/Y.  Keep this axis adapter explicit so a later material pass cannot
    /// accidentally project vertical Rust Y into the texture.
    #[inline]
    pub fn uv_for_rust_world_xz(&self, world_x: f32, world_z: f32) -> Option<[f32; 2]> {
        self.uv_for_cpp_world_xy(world_x, world_z)
    }

    /// Fingerprint only bytes and allocation shape.  The uploader uses this to
    /// avoid redundant R8 writes; projection/tint-only changes remain visible
    /// to the future material uniform through [`Self::content_fingerprint`].
    pub fn texture_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h = DefaultHasher::new();
        self.active.hash(&mut h);
        self.texture_width.hash(&mut h);
        self.texture_height.hash(&mut h);
        self.texels.len().hash(&mut h);
        self.texels.hash(&mut h);
        h.finish()
    }

    /// Fingerprint the complete frozen renderer input, including geometry and
    /// tint/level metadata.  Presentation determinism checks should use this,
    /// while a texture uploader should use [`Self::texture_fingerprint`].
    pub fn content_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h = DefaultHasher::new();
        self.texture_fingerprint().hash(&mut h);
        self.grid_width.hash(&mut h);
        self.grid_height.hash(&mut h);
        self.draw_origin_xy[0].to_bits().hash(&mut h);
        self.draw_origin_xy[1].to_bits().hash(&mut h);
        self.cell_size_xy[0].to_bits().hash(&mut h);
        self.cell_size_xy[1].to_bits().hash(&mut h);
        self.metadata.hash(&mut h);
        h.finish()
    }
}

impl ObjectVisibility {
    /// Fully visible (no FOW darkening / shell-map bypass).
    pub const FULLY_VISIBLE: Self = Self {
        visibility_alpha: 1.0,
        is_explored: 1.0,
        visibility_falloff: 1.0,
    };

    /// Currently visible.
    pub const VISIBLE: Self = Self::FULLY_VISIBLE;

    /// Explored earlier, not currently in vision (darkened).
    pub const FOGGED: Self = Self {
        visibility_alpha: 0.3,
        is_explored: 1.0,
        visibility_falloff: 1.0,
    };

    /// Never explored — must not be drawn for the local player.
    pub const HIDDEN: Self = Self {
        visibility_alpha: 0.0,
        is_explored: 0.0,
        visibility_falloff: 1.0,
    };

    /// Encode shroud flags into render visibility (parity with FOW bridge states).
    pub fn from_shroud_flags(is_visible: bool, is_explored: bool) -> Self {
        if is_visible {
            Self::VISIBLE
        } else if is_explored {
            Self::FOGGED
        } else {
            Self::HIDDEN
        }
    }

    /// True when the object should enter the mesh pass (visible or fogged, not never-seen).
    #[inline]
    pub fn should_render(&self) -> bool {
        self.visibility_alpha > 0.0 || self.is_explored > 0.0
    }

    /// C++ GameClient drawable shroud residual: Fogged|Shrouded|InvalidButPreviousValid
    /// → `setFullyObscuredByShroud(true)`. Only currently-visible cells keep models lit.
    #[inline]
    pub fn fully_obscures_drawable(&self) -> bool {
        self.visibility_alpha < 1.0
    }

    /// True when never explored (skip mesh entirely for local player).
    #[inline]
    pub fn never_explored(&self) -> bool {
        self.visibility_alpha <= 0.0 && self.is_explored <= 0.0
    }
}

/// FOW rendering bridge - connects shroud system to rendering pipeline
pub struct FOWRenderingBridge;

impl FOWRenderingBridge {
    /// Get visibility state for an object from the shroud manager
    ///
    /// This method queries the current FOW state and returns visibility
    /// parameters that should be passed to the shader for this object.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing (0-7)
    /// * `object_id` - Which object to check visibility for
    ///
    /// # Returns
    ///
    /// ObjectVisibility with:
    /// - `visibility_alpha`: 0.0 (hidden) to 1.0 (fully visible)
    /// - `is_explored`: 1.0 (explored) or 0.0 (never seen)
    /// - `visibility_falloff`: Gradient strength (1.0 for sharp, lower for smoother)
    pub fn get_object_visibility(player_id: u32, object_id: ObjectID) -> ObjectVisibility {
        // Default to fully visible if shroud manager not available
        // This ensures the game continues to work even without FOW
        let mut visibility = ObjectVisibility::default();

        // Query ShroudManager for visibility state
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            if !shroud_runtime_active(&shroud_mgr, player_id) {
                return visibility;
            }
            visibility = if let Some(status) =
                shroud_mgr.get_host_object_shroud_status(player_id, object_id.0)
            {
                use gamelogic::common::ObjectShroudStatus;
                match status {
                    ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear => {
                        ObjectVisibility::from_shroud_flags(true, true)
                    }
                    ObjectShroudStatus::Fogged => ObjectVisibility::from_shroud_flags(false, true),
                    _ => ObjectVisibility::from_shroud_flags(false, false),
                }
            } else {
                ObjectVisibility::from_shroud_flags(
                    shroud_mgr.can_see_object(player_id, object_id.0),
                    shroud_mgr.has_explored_object(player_id, object_id.0),
                )
            };
            let is_visible = visibility.visibility_alpha >= 1.0;
            let is_explored = visibility.is_explored >= 1.0;

            trace!(
                "FOW visibility for object {}: alpha={}, explored={}, visible={}",
                object_id, visibility.visibility_alpha, is_explored, is_visible
            );
        } else {
            trace!(
                "Shroud manager unavailable, using default visibility for object {}",
                object_id
            );
        }

        visibility
    }

    /// Get visibility state considering stealth/detection systems
    ///
    /// This variant checks stealth systems in addition to basic FOW.
    /// Objects may be visible in FOW but invisible due to stealth.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing
    /// * `object_id` - Which object to check
    ///
    /// # Returns
    ///
    /// ObjectVisibility with stealth considerations applied
    pub fn get_object_visibility_with_stealth(
        player_id: u32,
        object_id: ObjectID,
    ) -> ObjectVisibility {
        // Start with basic FOW visibility
        let mut visibility = Self::get_object_visibility(player_id, object_id);

        // If not visible due to FOW, stealth doesn't matter
        if visibility.visibility_alpha <= 0.0 {
            return visibility;
        }

        // Check stealth system - this would check if object is stealthed
        // and whether the player has detection capability
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            if !shroud_runtime_active(&shroud_mgr, player_id) {
                return visibility;
            }

            match shroud_mgr.can_see_object_with_stealth(player_id, object_id.0) {
                Ok(can_see_with_stealth) => {
                    if !can_see_with_stealth {
                        // Object is stealthed and not detected
                        visibility.visibility_alpha = 0.0;
                    }
                }
                Err(_) => {
                    // On error, keep current visibility
                    // (fail-open for gameplay)
                }
            }
        }

        visibility
    }

    /// Update all object visibilities for a player
    ///
    /// Batch query for all visible objects. Used during rendering to
    /// efficiently determine which objects to render and with what visibility.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing
    /// * `object_ids` - List of objects to check
    ///
    /// # Returns
    ///
    /// Map of object_id to visibility state
    pub fn get_all_object_visibilities(
        player_id: u32,
        object_ids: &[ObjectID],
    ) -> std::collections::HashMap<ObjectID, ObjectVisibility> {
        let mut visibilities = std::collections::HashMap::with_capacity(object_ids.len());

        for &object_id in object_ids {
            let visibility = Self::get_object_visibility(player_id, object_id);
            visibilities.insert(object_id, visibility);
        }

        visibilities
    }

    /// Check if an object should be rendered at all for a player
    ///
    /// Returns true if object is either visible or explored (darkened).
    /// Objects that have never been seen return false.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing
    /// * `object_id` - Which object to check
    ///
    /// # Returns
    ///
    /// true if object should be rendered (even if darkened)
    pub fn should_render_object(player_id: u32, object_id: ObjectID) -> bool {
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            if !shroud_runtime_active(&shroud_mgr, player_id) {
                return true;
            }
            // Render if visible or explored (explored objects show as darkened)
            shroud_mgr.can_see_object(player_id, object_id.0)
                || shroud_mgr.has_explored_object(player_id, object_id.0)
        } else {
            // No shroud manager, render everything
            true
        }
    }

    /// Force visibility recalculation for next frame
    ///
    /// Called when significant events occur:
    /// - Units created or destroyed
    /// - Vision upgrades completed
    /// - Special powers used
    pub fn force_visibility_update() {
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.force_update();
            trace!("FOW visibility recalculation forced");
        }
    }

    /// Snapshot the partition cell grid for `player_id` into a presentation-owned buffer.
    ///
    /// Returns an inactive empty grid when the shroud manager is unavailable or the
    /// grid is not initialized (fail-open for terrain overlay). Shell-map callers
    /// should pass `shell_bypass=true` to force fully-visible cells when dimensions
    /// are known.
    pub fn snapshot_terrain_grid(player_id: u32, shell_bypass: bool) -> PresentationFowGrid {
        let Ok(shroud_mgr) = get_shroud_manager().lock() else {
            return PresentationFowGrid::inactive();
        };

        let Some((width, height, cell_size)) = shroud_mgr.grid_dimensions() else {
            return PresentationFowGrid::inactive();
        };
        let width_u = width as u32;
        let height_u = height as u32;

        if shell_bypass {
            return PresentationFowGrid::fully_visible(width_u, height_u, cell_size);
        }

        // When shroud has never updated, fail-open (match unit FOW startup safeguard)
        // so terrain is not painted fully black during boot.
        if !shroud_runtime_active(&shroud_mgr, player_id) {
            return PresentationFowGrid::fully_visible(width_u, height_u, cell_size);
        }

        match shroud_mgr.snapshot_grid_for_player(player_id) {
            Some(cells) => {
                trace!(
                    "FOW terrain grid snapshot player={} {}x{} cells={}",
                    player_id,
                    width_u,
                    height_u,
                    cells.len()
                );
                PresentationFowGrid::from_snapshot(width_u, height_u, cell_size, cells)
            }
            None => PresentationFowGrid::inactive(),
        }
    }

    /// Encode a live [`ShroudState`] into the compact presentation cell value.
    #[inline]
    pub fn shroud_state_to_cell(state: ShroudState) -> u8 {
        state as u8
    }
}

/// Reveal the entire map as explored/fogged (addLooker+removeLooker).
/// C++ PartitionManager::revealMapForPlayer — shroud crates and RevealMap scripts.
pub fn reveal_entire_map_explored_for_player(player_id: u32) {
    if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
        if let Err(err) = shroud_mgr.reveal_map_for_player(player_id) {
            warn!("Failed to reveal map for player {player_id}: {err}");
        }
    }
}

/// Reveal the entire map permanently (add lookers only).
/// C++ PartitionManager::revealMapForPlayerPermanently — observer/defeat only.
pub fn reveal_entire_map_for_player(player_id: u32) {
    if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
        if let Err(err) = shroud_mgr.reveal_map_for_player_permanently(player_id) {
            warn!("Failed to permanently reveal map for player {player_id}: {err}");
        }
    }
}

// --- Wave 77: FOW residual honesty pack ---

/// Retail / SAGE shroud partition cell size residual (world units).
///
/// C++ PartitionManager::m_cellSize = GlobalData PartitionCellSize (40).
pub const PRESENTATION_FOW_DEFAULT_CELL_SIZE: f32 = 40.0;

/// Honesty: FOW cell encoding / R8 terrain overlay / inactive fail-open residual.
///
/// Host-testable pack for presentation-owned FOW grid residual (Wave 77).
/// Fail-closed: not full SAGE dirty-rect / multi-layer shroud texture streaming.
pub fn honesty_fow_residual_pack_wave77() -> bool {
    // SAGE-style cell buckets residual (0/1/2).
    PresentationFowGrid::CELL_HIDDEN == 0
        && PresentationFowGrid::CELL_EXPLORED == 1
        && PresentationFowGrid::CELL_VISIBLE == 2
        // Terrain overlay R8 residual (shrouded / fogged / clear).
        && PresentationFowGrid::R8_SHROUDED == 0
        && PresentationFowGrid::R8_FOGGED == 128
        && PresentationFowGrid::R8_VISIBLE == 255
        && PresentationFowGrid::cell_to_r8(PresentationFowGrid::CELL_HIDDEN)
            == PresentationFowGrid::R8_SHROUDED
        && PresentationFowGrid::cell_to_r8(PresentationFowGrid::CELL_EXPLORED)
            == PresentationFowGrid::R8_FOGGED
        && PresentationFowGrid::cell_to_r8(PresentationFowGrid::CELL_VISIBLE)
            == PresentationFowGrid::R8_VISIBLE
        && (PRESENTATION_FOW_DEFAULT_CELL_SIZE - 40.0).abs() < 0.01
        && {
            let inactive = PresentationFowGrid::inactive();
            !inactive.active
                && inactive.cells.is_empty()
                && (inactive.cell_size - PRESENTATION_FOW_DEFAULT_CELL_SIZE).abs() < 0.01
                // Inactive fail-open: sample as visible, empty R8 payload.
                && inactive.cell_at(0, 0) == PresentationFowGrid::CELL_VISIBLE
                && inactive.to_r8_texture().is_empty()
        }
        && {
            // Fully-visible residual (shell-map / observer bypass).
            let full = PresentationFowGrid::fully_visible(4, 3, PRESENTATION_FOW_DEFAULT_CELL_SIZE);
            full.active
                && full.cell_count() == 12
                && full.cells.iter().all(|&c| c == PresentationFowGrid::CELL_VISIBLE)
                && full.to_r8_texture().iter().all(|&v| v == PresentationFowGrid::R8_VISIBLE)
        }
        && {
            // from_snapshot resize residual (pad with Hidden when undersized).
            let g = PresentationFowGrid::from_snapshot(
                2,
                2,
                PRESENTATION_FOW_DEFAULT_CELL_SIZE,
                vec![PresentationFowGrid::CELL_VISIBLE],
            );
            g.active
                && g.cell_count() == 4
                && g.cell_at(0, 0) == PresentationFowGrid::CELL_VISIBLE
                && g.cell_at(1, 1) == PresentationFowGrid::CELL_HIDDEN
                && g.content_fingerprint() != 0
        }
        && {
            // ObjectVisibility residual encoding for FOW consumers.
            ObjectVisibility::from_shroud_flags(true, true) == ObjectVisibility::VISIBLE
                && ObjectVisibility::from_shroud_flags(false, true) == ObjectVisibility::FOGGED
                && ObjectVisibility::from_shroud_flags(false, false) == ObjectVisibility::HIDDEN
                && ObjectVisibility::HIDDEN.never_explored()
                && !ObjectVisibility::HIDDEN.should_render()
                && ObjectVisibility::FOGGED.should_render()
                && (ObjectVisibility::FOGGED.visibility_alpha - 0.3).abs() < 0.01
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_visibility_default() {
        let vis = ObjectVisibility::default();
        assert_eq!(vis.visibility_alpha, 1.0);
        assert_eq!(vis.is_explored, 1.0);
        assert_eq!(vis.visibility_falloff, 1.0);
        assert!(vis.should_render());
    }

    #[test]
    fn test_object_visibility_custom() {
        let vis = ObjectVisibility {
            visibility_alpha: 0.5,
            is_explored: 1.0,
            visibility_falloff: 0.8,
        };
        assert_eq!(vis.visibility_alpha, 0.5);
        assert_eq!(vis.is_explored, 1.0);
        assert_eq!(vis.visibility_falloff, 0.8);
    }

    #[test]
    fn test_object_visibility_from_shroud_flags() {
        assert_eq!(
            ObjectVisibility::from_shroud_flags(true, true),
            ObjectVisibility::VISIBLE
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, true),
            ObjectVisibility::FOGGED
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, false),
            ObjectVisibility::HIDDEN
        );
        assert!(ObjectVisibility::HIDDEN.never_explored());
        assert!(!ObjectVisibility::HIDDEN.should_render());
        assert!(ObjectVisibility::FOGGED.should_render());
    }

    #[test]
    fn presentation_fow_grid_r8_encoding_and_sample() {
        let mut cells = vec![PresentationFowGrid::CELL_HIDDEN; 4];
        cells[0] = PresentationFowGrid::CELL_VISIBLE; // (0,0)
        cells[1] = PresentationFowGrid::CELL_EXPLORED; // (1,0)
        cells[2] = PresentationFowGrid::CELL_HIDDEN; // (0,1)
        cells[3] = PresentationFowGrid::CELL_VISIBLE; // (1,1)
        let grid = PresentationFowGrid::from_snapshot(2, 2, 50.0, cells);
        assert!(grid.active);
        assert_eq!(grid.cell_at(0, 0), PresentationFowGrid::CELL_VISIBLE);
        assert_eq!(grid.cell_at(1, 0), PresentationFowGrid::CELL_EXPLORED);
        assert_eq!(
            grid.state_at_world_xy(10.0, 10.0),
            PresentationFowGrid::CELL_VISIBLE
        );
        assert_eq!(
            grid.state_at_world_xy(60.0, 10.0),
            PresentationFowGrid::CELL_EXPLORED
        );

        let r8 = grid.to_r8_texture();
        assert_eq!(
            r8,
            vec![
                PresentationFowGrid::R8_VISIBLE,
                PresentationFowGrid::R8_FOGGED,
                PresentationFowGrid::R8_SHROUDED,
                PresentationFowGrid::R8_VISIBLE,
            ]
        );
        assert_eq!(
            PresentationFowGrid::cell_to_object_visibility(PresentationFowGrid::CELL_EXPLORED),
            ObjectVisibility::FOGGED
        );
        assert_eq!(
            FOWRenderingBridge::shroud_state_to_cell(ShroudState::Visible),
            PresentationFowGrid::CELL_VISIBLE
        );
        assert_eq!(
            FOWRenderingBridge::shroud_state_to_cell(ShroudState::Explored),
            PresentationFowGrid::CELL_EXPLORED
        );
        assert_eq!(
            FOWRenderingBridge::shroud_state_to_cell(ShroudState::Hidden),
            PresentationFowGrid::CELL_HIDDEN
        );
    }

    #[test]
    fn presentation_fow_grid_inactive_fail_open() {
        let g = PresentationFowGrid::inactive();
        assert!(!g.active);
        assert!(g.to_r8_texture().is_empty());
        assert_eq!(g.cell_at(0, 0), PresentationFowGrid::CELL_VISIBLE);
        assert_eq!(
            g.state_at_world_xy(999.0, 999.0),
            PresentationFowGrid::CELL_VISIBLE
        );
    }

    #[test]
    fn projected_shroud_snapshot_uses_cxx_plus_two_border_and_source_levels() {
        let grid = PresentationFowGrid::from_snapshot_at_origin(
            2,
            2,
            [100.0, -40.0],
            10.0,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
                PresentationFowGrid::CELL_HIDDEN,
                PresentationFowGrid::CELL_VISIBLE,
            ],
        );
        let snapshot =
            ProjectedShroudSnapshot::from_grid(&grid, ProjectedShroudMetadata::default());

        assert!(snapshot.active);
        assert_eq!(snapshot.grid_width, 2);
        assert_eq!(snapshot.grid_height, 2);
        assert_eq!(snapshot.texture_extent(), Some((4, 4)));
        assert_eq!(snapshot.draw_origin_xy, [100.0, -40.0]);
        assert_eq!(snapshot.cell_size_xy, [10.0, 10.0]);
        assert_eq!(
            snapshot.texels,
            vec![
                0, 0, 0, 0, // top border
                0, 255, 127, 0, // logical source row 0 at destination (1, 1)
                0, 0, 255, 0, // logical source row 1 at destination (1, 2)
                0, 0, 0, 0, // bottom border
            ]
        );
        assert_eq!(snapshot.texel_at(0, 0), Some(0));
        assert_eq!(snapshot.texel_at(3, 3), Some(0));
        assert_eq!(snapshot.texel_at(1, 1), Some(255));
        assert_eq!(snapshot.texel_at(2, 1), Some(127));
        assert_eq!(snapshot.metadata.shroud_color_rgb, [255, 255, 255]);
    }

    #[test]
    fn projected_shroud_snapshot_uses_validated_w3d_extent_and_fills_all_padding() {
        let grid = PresentationFowGrid::from_snapshot(
            3,
            2,
            10.0,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
                PresentationFowGrid::CELL_HIDDEN,
                PresentationFowGrid::CELL_HIDDEN,
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
            ],
        );
        let snapshot =
            ProjectedShroudSnapshot::from_grid(&grid, ProjectedShroudMetadata::default());

        // W3D adds the border first (5x4), then Validate_Texture_Size rounds
        // the destination to 8x4.  The logical copy still begins at (1, 1).
        assert_eq!(snapshot.texture_extent(), Some((8, 4)));
        assert_eq!(snapshot.texel_at(1, 1), Some(255));
        assert_eq!(snapshot.texel_at(2, 1), Some(127));
        assert_eq!(snapshot.texel_at(3, 1), Some(0));
        assert_eq!(snapshot.texel_at(1, 2), Some(0));
        assert_eq!(snapshot.texel_at(2, 2), Some(255));
        assert_eq!(snapshot.texel_at(3, 2), Some(127));

        // fillBorderShroudData clears the complete validated destination, not
        // merely its four-edge logical border.  This includes row/column
        // padding introduced by the power-of-two allocation.
        for y in 0..snapshot.texture_height {
            for x in 0..snapshot.texture_width {
                let is_logical_texel = (1..=3).contains(&x) && (1..=2).contains(&y);
                if !is_logical_texel {
                    assert_eq!(snapshot.texel_at(x, y), Some(0), "padding at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn projected_shroud_validation_matches_w3d_power_of_two_aspect_and_cap_policy() {
        assert_eq!(
            ProjectedShroudSnapshot::validated_texture_extent_for_grid(2, 2),
            Some((4, 4))
        );
        assert_eq!(
            ProjectedShroudSnapshot::validated_texture_extent_for_grid(3, 2),
            Some((8, 4))
        );
        // Padded 3x128 first becomes 4x128, then W3D grows the smaller
        // dimension until no side is more than eight times the other.
        assert_eq!(
            ProjectedShroudSnapshot::validated_texture_extent_for_grid(1, 126),
            Some((16, 128))
        );
        // A CPU snapshot has no live D3D/WGPU device cap to consult.  The
        // fixed legacy-compatible policy fails closed instead of truncating a
        // logical copy that would no longer fit in its destination.
        assert_eq!(
            ProjectedShroudSnapshot::validated_texture_extent_for_grid(2047, 1),
            None
        );
    }

    #[test]
    fn projected_shroud_metadata_freezes_script_border_override_without_weakening_shroud_minimum() {
        let mut global = game_engine::common::global_data::GlobalData::default();
        global.shroud_alpha = 32;

        let default_border = ProjectedShroudMetadata::from_global_data(&global);
        assert_eq!(default_border.border_alpha, 32);

        let script_border =
            ProjectedShroudMetadata::from_global_data_with_border_override(&global, Some(255));
        assert_eq!(script_border.border_alpha, 255);
        assert_eq!(script_border.encoded_border_level(), 255);

        // C++ fillBorderShroudData still clamps an override below
        // GlobalData::m_shroudAlpha before writing destination texels.
        let clamped =
            ProjectedShroudMetadata::from_global_data_with_border_override(&global, Some(4));
        assert_eq!(clamped.encoded_border_level(), 32);
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn projected_shroud_enable_disable_border_override_is_frozen() {
        use game_client::core::script_action_handler::{
            GameClientScriptActionHandler, clear_script_display_border_shroud_level,
            script_display_border_shroud_level,
        };
        use gamelogic::scripting::engine::ScriptActionHandler;

        struct ClearBorderOverrideOnDrop;
        impl Drop for ClearBorderOverrideOnDrop {
            fn drop(&mut self) {
                clear_script_display_border_shroud_level();
            }
        }

        clear_script_display_border_shroud_level();
        let _clear_on_drop = ClearBorderOverrideOnDrop;
        let mut global = game_engine::common::global_data::GlobalData::default();
        global.shroud_alpha = 37;
        global.clear_alpha = 211;
        let grid = PresentationFowGrid::fully_visible(3, 2, 10.0);
        let handler = GameClientScriptActionHandler::new();

        // C++ DisableBorderShroud dispatches the configured ClearAlpha to the
        // display handler.  Freeze its override into the complete padded R8
        // allocation before later script state can change.
        handler
            .set_border_shroud_level(global.clear_alpha)
            .expect("DisableBorderShroud display handoff");
        assert_eq!(script_display_border_shroud_level(), Some(211));
        let disabled = ProjectedShroudSnapshot::from_grid(
            &grid,
            ProjectedShroudMetadata::from_global_data_with_border_override(
                &global,
                script_display_border_shroud_level(),
            ),
        );
        assert_eq!(disabled.metadata.border_alpha, 211);
        assert_eq!(disabled.texel_at(0, 0), Some(211));
        assert_eq!(disabled.texel_at(7, 3), Some(211));

        // C++ EnableBorderShroud sends ShroudAlpha.  A fresh snapshot sees the
        // new border while the prior presentation frame stays fully frozen.
        handler
            .set_border_shroud_level(global.shroud_alpha)
            .expect("EnableBorderShroud display handoff");
        assert_eq!(script_display_border_shroud_level(), Some(37));
        let enabled = ProjectedShroudSnapshot::from_grid(
            &grid,
            ProjectedShroudMetadata::from_global_data_with_border_override(
                &global,
                script_display_border_shroud_level(),
            ),
        );
        assert_eq!(enabled.metadata.border_alpha, 37);
        assert_eq!(enabled.texel_at(0, 0), Some(37));
        assert_eq!(disabled.metadata.border_alpha, 211);
        assert_eq!(disabled.texel_at(0, 0), Some(211));
    }

    #[test]
    fn projected_shroud_projection_adapts_rust_xz_to_cxx_xy_with_nonzero_origin() {
        let grid = PresentationFowGrid::fully_visible_at_origin(3, 2, [100.0, -40.0], 10.0);
        let snapshot =
            ProjectedShroudSnapshot::from_grid(&grid, ProjectedShroudMetadata::default());

        // C++ shader: ((world - draw_origin) + cell_size) /
        // (cell_size * texture_extent).  Rust ground Z is C++ shroud Y.
        assert_eq!(
            snapshot.uv_for_cpp_world_xy(100.0, -40.0),
            Some([0.125, 0.25])
        );
        assert_eq!(
            snapshot.uv_for_rust_world_xz(100.0, -40.0),
            Some([0.125, 0.25])
        );
        assert_eq!(
            snapshot.uv_for_rust_world_xz(120.0, -30.0),
            Some([0.375, 0.5])
        );
        // Values outside the grid are not CPU-clamped; the texture uses C++/WGPU
        // clamp addressing at sampling time.
        assert_eq!(snapshot.uv_for_rust_world_xz(90.0, -50.0), Some([0.0, 0.0]));
    }

    #[test]
    fn projected_shroud_fingerprints_distinguish_content_from_gpu_texels() {
        let grid = PresentationFowGrid::from_snapshot(
            2,
            1,
            10.0,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
            ],
        );
        let snapshot =
            ProjectedShroudSnapshot::from_grid(&grid, ProjectedShroudMetadata::default());
        let mut tinted = snapshot.clone();
        tinted.metadata.shroud_color_rgb = [64, 128, 255];
        assert_eq!(snapshot.texture_fingerprint(), tinted.texture_fingerprint());
        assert_ne!(snapshot.content_fingerprint(), tinted.content_fingerprint());

        let changed_grid = PresentationFowGrid::from_snapshot(
            2,
            1,
            10.0,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_HIDDEN,
            ],
        );
        let changed =
            ProjectedShroudSnapshot::from_grid(&changed_grid, ProjectedShroudMetadata::default());
        assert_ne!(
            snapshot.texture_fingerprint(),
            changed.texture_fingerprint()
        );

        let resized_grid = PresentationFowGrid::fully_visible(3, 1, 10.0);
        let resized =
            ProjectedShroudSnapshot::from_grid(&resized_grid, ProjectedShroudMetadata::default());
        assert_eq!(resized.texture_extent(), Some((8, 4)));
        assert_ne!(
            snapshot.texture_fingerprint(),
            resized.texture_fingerprint()
        );
    }

    #[test]
    fn projected_shroud_inactive_fails_open_without_stale_texture_payload() {
        let inactive = ProjectedShroudSnapshot::from_grid(
            &PresentationFowGrid::inactive(),
            ProjectedShroudMetadata::default(),
        );
        assert!(!inactive.active);
        assert!(!inactive.is_uploadable());
        assert!(inactive.texels.is_empty());
        assert_eq!(inactive.texture_extent(), None);
        assert_eq!(inactive.uv_for_rust_world_xz(0.0, 0.0), None);
    }

    /// Wave 77 residual: FOW cell/R8/inactive/fail-open honesty pack.
    #[test]
    fn fow_residual_pack_wave77_honesty() {
        assert!(honesty_fow_residual_pack_wave77());
        assert_eq!(PRESENTATION_FOW_DEFAULT_CELL_SIZE, 40.0);
        assert_eq!(PresentationFowGrid::CELL_HIDDEN, 0);
        assert_eq!(PresentationFowGrid::CELL_EXPLORED, 1);
        assert_eq!(PresentationFowGrid::CELL_VISIBLE, 2);
        assert_eq!(PresentationFowGrid::R8_SHROUDED, 0);
        assert_eq!(PresentationFowGrid::R8_FOGGED, 128);
        assert_eq!(PresentationFowGrid::R8_VISIBLE, 255);
    }
}

#[cfg(test)]
mod host_fow_fail_open_tests {
    #[test]
    fn host_fow_fail_open_without_object_membership() {
        let src = include_str!("fow_rendering.rs");
        let start = src.find("fn shroud_runtime_active").expect("fn");
        let body = &src[start..src.len().min(start + 900)];
        assert!(
            body.contains("get_visible_objects(player_id)"),
            "must require visible membership"
        );
        assert!(
            body.contains("get_explored_objects(player_id)"),
            "must require explored membership"
        );
        assert!(
            !body.contains("get_last_update_frame() > 0"),
            "last_update_frame alone must not activate FOW object filtering"
        );
    }
}
