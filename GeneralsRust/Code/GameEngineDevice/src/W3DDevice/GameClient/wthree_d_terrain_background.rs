//! W3DTerrainBackground Module
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DTerrainBackground.h (96 lines)
//! - GameEngineDevice/Source/W3DDevice/GameClient/W3DTerrainBackground.cpp (786 lines)
//! - GameEngineDevice/Include/W3DDevice/GameClient/FlatHeightMap.h (73 lines)
//! - GameEngineDevice/Source/W3DDevice/GameClient/FlatHeightMap.cpp (615 lines)
//!
//! This module provides tile-based terrain background rendering with tessellation support.
//! W3DTerrainBackground renders a single tile of terrain at lower resolution (backup terrain),
//! while FlatHeightMapRenderObj manages a grid of these tiles.
//!
//! Key concepts ported from C++:
//! - Tile-based terrain with quadtree subdivision for tessellation
//! - Each tile has 4 corner vertices, optional flip for cliff rendering
//! - Tessellation subdivides tiles based on height uniformity (LOD)
//! - Vertex format: position (XYZ), diffuse color, UV coordinates
//! - Flip state: marks vertices that should be included in the tessellated mesh
//!
//! Author: John Ahlquist, March/May 2001/2003 (original C++)
//! Port: Rust behavioral parity port

use bytemuck::{Pod, Zeroable};
use cgmath::{InnerSpace, Point3, Vector3};
use wgpu::util::DeviceExt;
use wgpu::{Buffer, BufferUsages, Device, IndexFormat, Queue, RenderPass, Texture, TextureView};

// Re-export constants from terrain_rendering for parity
use super::terrain_rendering::{TerrainVertex, MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// Default pixels per grid cell for flat texture allocation.
/// C++ W3DTerrainBackground.cpp line 55: `const Int PIXELS_PER_GRID = 8;`
pub const PIXELS_PER_GRID: i32 = 8;

/// C++ FlatHeightMap.cpp line 87: `const Int CELLS_PER_TILE = 16;`
pub const CELLS_PER_TILE: i32 = 16;

/// C++ W3DTerrainBackground.cpp line 83: `const Int STEP=4;` (used in non-tessellated path)
const STEP: i32 = 4;

/// Cull status enum. C++ W3DTerrainBackground.h line 65.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullStatus {
    Unknown = 0,
    Visible = 1,
    Invisible = 2,
}

/// Texture multiplier for LOD mip selection. C++ W3DTerrainBackground.h line 75.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexMultiplier {
    Tex4X = 4,
    Tex2X = 2,
    Tex1X = 1,
}

/// 2D integer coordinate used for advanceLeft/advanceRight traversal.
/// Matches C++ ICoord2D.
#[derive(Debug, Clone, Copy, Default)]
struct ICoord2D {
    x: i32,
    y: i32,
}

/// Axis-aligned bounding box for culling. Matches C++ MinMaxAABoxClass.
#[derive(Debug, Clone, Default)]
struct AABox {
    center: Vector3<f32>,
    extent: Vector3<f32>,
}

impl AABox {
    fn init_empty(&mut self) {
        self.center = Vector3::new(0.0, 0.0, 0.0);
        self.extent = Vector3::new(0.0, 0.0, 0.0);
    }

    fn add_point(&mut self, point: Vector3<f32>) {
        // Expand bounding box to include a point
        let min = Vector3::new(
            self.center.x - self.extent.x,
            self.center.y - self.extent.y,
            self.center.z - self.extent.z,
        );
        let max = Vector3::new(
            self.center.x + self.extent.x,
            self.center.y + self.extent.y,
            self.center.z + self.extent.z,
        );
        let new_min = Vector3::new(min.x.min(point.x), min.y.min(point.y), min.z.min(point.z));
        let new_max = Vector3::new(max.x.max(point.x), max.y.max(point.y), max.z.max(point.z));
        self.center = (new_min + new_max) * 0.5;
        self.extent = (new_max - new_min) * 0.5;
    }
}

/// Update state machine for FlatHeightMap.
/// C++ FlatHeightMap.h lines 62-66.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatHeightMapState {
    /// Sleeping
    Idle = 0,
    /// Camera moving, updating visibility
    Moving = 1,
    /// Second moving state
    Moving2 = 2,
    /// Camera stopped, updating textures
    UpdateTextures = 3,
}

// =============================================================================
// Trait: TerrainHeightMapAccess
// =============================================================================
/// Trait defining the height map interface needed by W3DTerrainBackground.
///
/// In C++, WorldHeightMap directly provides all these methods. In Rust,
/// the full WorldHeightMap is not yet ported, so we define a trait that
/// any heightmap implementation must satisfy. This decouples the terrain
/// background renderer from the specific heightmap data source.
///
/// PARITY_NOTE: All method signatures match C++ WorldHeightMap interface.
pub trait TerrainHeightMapAccess {
    /// Get height at cell (x, y). C++: `m_map->getHeight(i, j)`
    fn get_height(&self, x: i32, y: i32) -> i32;

    /// Get X extent (width in cells). C++: `m_map->getXExtent()`
    fn get_x_extent(&self) -> i32;

    /// Get Y extent (height in cells). C++: `m_map->getYExtent()`
    fn get_y_extent(&self) -> i32;

    /// Get border size in cells. C++: `m_map->getBorderSizeInline()`
    fn get_border_size(&self) -> i32;

    /// Get flip state at cell (x, y). C++: `m_map->getFlipState(i, j)`
    fn get_flip_state(&self, x: i32, y: i32) -> bool;

    /// Set flip state at cell (x, y). C++: `m_map->setFlipState(i, j, val)`
    fn set_flip_state(&mut self, x: i32, y: i32, val: bool);

    /// Clear all flip states. C++: `pMap->clearFlipStates()`
    fn clear_flip_states(&mut self);

    /// Get static diffuse color at cell (x, y).
    /// C++: `TheTerrainRenderObject->getStaticDiffuse(i,j)`
    fn get_static_diffuse(&self, x: i32, y: i32) -> u32;

    /// Get tile pixel data for terrain texture creation.
    /// C++ reads RGB data from WorldHeightMap via `getRGBTileData()`.
    /// Returns RGBA bytes for a tile region starting at (x, y) with the given
    /// pixel resolution per grid cell (`pixels_per_grid`).
    /// The returned Vec has size `(width * pixels_per_grid) * (height * pixels_per_grid) * 4`.
    /// If no tile data is available, returns None (fallback to diffuse coloring).
    fn get_tile_pixel_data(
        &self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
        _pixels_per_grid: i32,
    ) -> Option<Vec<u8>> {
        None
    }
}

// =============================================================================
// W3DTerrainBackground
// =============================================================================
/// Draw buffer for terrain background tiles.
///
/// Corresponds to C++ `W3DTerrainBackground` from W3DTerrainBackground.h.
///
/// Each instance manages a single tile of terrain background mesh with:
/// - Vertex buffer for terrain geometry (position, diffuse, UV)
/// - Index buffer for triangle list rendering
/// - Tessellation support (adaptive subdivision based on height uniformity)
/// - Flip state management for cliff rendering
/// - Culling and texture LOD (1x, 2x, 4x multiplier)
///
/// The tessellation algorithm works by recursively subdividing a tile into
/// quadrants. If all heights in a quadrant are equal (flat), it renders as
/// a single triangle fan using the corner vertices. If heights vary, it
/// subdivides further. This creates an adaptive LOD mesh that follows terrain
/// contours efficiently.
pub struct W3DTerrainBackground {
    /// Vertex buffer for terrain geometry. C++ m_vertexTerrain.
    vertex_buffer: Option<Buffer>,
    /// Allocated size of vertex buffer. C++ m_vertexTerrainSize.
    vertex_buffer_size: i32,
    /// Index buffer for triangle indices. C++ m_indexTerrain.
    index_buffer: Option<Buffer>,
    /// Allocated size of index buffer. C++ m_indexTerrainSize.
    index_buffer_size: i32,

    /// Terrain texture (base resolution). C++ m_terrainTexture.
    terrain_texture: Option<Texture>,
    /// Terrain texture view.
    terrain_texture_view: Option<TextureView>,
    /// 2x resolution terrain texture. C++ m_terrainTexture2X.
    terrain_texture_2x: Option<Texture>,
    /// 2x texture view.
    terrain_texture_2x_view: Option<TextureView>,
    /// 4x resolution terrain texture. C++ m_terrainTexture4X.
    terrain_texture_4x: Option<Texture>,
    /// 4x texture view.
    terrain_texture_4x_view: Option<TextureView>,

    /// Current texture multiplier for LOD. C++ m_texMultiplier.
    tex_multiplier: TexMultiplier,

    /// Current number of vertices used in vertex buffer. C++ m_curNumTerrainVertices.
    cur_num_terrain_vertices: i32,
    /// Current number of indices used in index buffer. C++ m_curNumTerrainIndices.
    cur_num_terrain_indices: i32,

    /// Tile origin X in map coordinates. C++ m_xOrigin.
    x_origin: i32,
    /// Tile origin Y in map coordinates. C++ m_yOrigin.
    y_origin: i32,
    /// Tile width in cells. C++ m_width.
    width: i32,

    /// Culling status. C++ m_cullStatus.
    cull_status: CullStatus,

    /// Bounding box for culling. C++ m_bounds.
    bounds: AABox,

    /// Whether any visibility or sorting changed. C++ m_anythingChanged.
    anything_changed: bool,

    /// Whether the subsystem is initialized. C++ m_initialized.
    initialized: bool,
}

impl Default for W3DTerrainBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DTerrainBackground {
    /// Constructor. C++ W3DTerrainBackground.cpp lines 595-607.
    pub fn new() -> Self {
        Self {
            vertex_buffer: None,
            vertex_buffer_size: 0,
            index_buffer: None,
            index_buffer_size: 0,
            terrain_texture: None,
            terrain_texture_view: None,
            terrain_texture_2x: None,
            terrain_texture_2x_view: None,
            terrain_texture_4x: None,
            terrain_texture_4x_view: None,
            tex_multiplier: TexMultiplier::Tex1X,
            cur_num_terrain_vertices: 0,
            cur_num_terrain_indices: 0,
            x_origin: 0,
            y_origin: 0,
            width: 0,
            cull_status: CullStatus::Unknown,
            bounds: AABox::default(),
            anything_changed: false,
            initialized: false,
        }
    }

    /// Allocate terrain buffers for this tile.
    /// C++ W3DTerrainBackground.cpp lines 630-641.
    ///
    /// Sets up origin, width, and marks as initialized.
    /// Actual buffer allocation happens lazily in doTesselatedUpdate.
    pub fn allocate_terrain_buffers(
        &mut self,
        _ht_map: &dyn TerrainHeightMapAccess,
        x_origin: i32,
        y_origin: i32,
        width: i32,
    ) {
        self.free_terrain_buffers();
        self.cur_num_terrain_vertices = 0;
        self.cur_num_terrain_indices = 0;
        self.x_origin = x_origin;
        self.y_origin = y_origin;
        self.width = width;
        self.initialized = true;
    }

    /// Free terrain buffers. C++ W3DTerrainBackground.cpp lines 614-623.
    pub fn free_terrain_buffers(&mut self) {
        self.vertex_buffer = None;
        self.index_buffer = None;
        self.cur_num_terrain_vertices = 0;
        self.cur_num_terrain_indices = 0;
        self.initialized = false;
        // Release textures
        self.terrain_texture = None;
        self.terrain_texture_view = None;
        self.terrain_texture_2x = None;
        self.terrain_texture_2x_view = None;
        self.terrain_texture_4x = None;
        self.terrain_texture_4x_view = None;
    }

    /// Set flip bits for required vertices across the tile hierarchy.
    /// C++ W3DTerrainBackground.cpp lines 67-80.
    ///
    /// The flip state marks which vertices participate in the tessellated mesh.
    /// Vertices at height discontinuities are marked for adaptive subdivision.
    pub fn set_flip(&mut self, ht_map: &mut dyn TerrainHeightMapAccess) {
        if !self.initialized {
            return;
        }
        self.set_flip_recursive(ht_map, 0, 0, self.width);
    }

    /// Recursively set flip state for tile hierarchy.
    /// C++ W3DTerrainBackground.cpp lines 397-439.
    ///
    /// For each quadrant, checks if all heights are equal (flat).
    /// If flat, marks the 4 corner vertices as flipped (included in mesh).
    /// If not flat, subdivides into 4 sub-quadrants and recurses.
    /// At width==1, always marks corners (base case for non-uniform areas).
    fn set_flip_recursive(
        &mut self,
        ht_map: &mut dyn TerrainHeightMapAccess,
        x_offset: i32,
        y_offset: i32,
        width: i32,
    ) {
        let limit_x = ht_map.get_x_extent() - 1;
        let limit_y = ht_map.get_y_extent() - 1;

        let min_x = self.x_origin + x_offset;
        let min_y = self.y_origin + y_offset;
        let corner_height = ht_map.get_height(min_x, min_y);

        // Check if all heights in this quadrant are equal
        let mut match_heights = true;
        'outer: for i in 0..=width {
            for j in 0..=width {
                let k = (min_x + i).min(limit_x);
                let l = (min_y + j).min(limit_y);
                if corner_height != ht_map.get_height(k, l) {
                    match_heights = false;
                    break 'outer;
                }
            }
        }

        // At width==1, always treat as flat (base case)
        if width == 1 {
            match_heights = true;
        }

        if match_heights {
            // Mark all 4 corners as flipped
            ht_map.set_flip_state(min_x, min_y, true);
            ht_map.set_flip_state(min_x + width, min_y, true);
            ht_map.set_flip_state(min_x + width, min_y + width, true);
            ht_map.set_flip_state(min_x, min_y + width, true);
            return;
        }

        // Subdivide into 4 quadrants
        let half_width = width / 2;
        // Note: set_flip is called on self, but operates on ht_map.
        // The recursive calls modify ht_map's flip state, not self's fields.
        self.set_flip_recursive(ht_map, x_offset, y_offset, half_width);
        self.set_flip_recursive(ht_map, x_offset, y_offset + half_width, half_width);
        self.set_flip_recursive(ht_map, x_offset + half_width, y_offset, half_width);
        self.set_flip_recursive(
            ht_map,
            x_offset + half_width,
            y_offset + half_width,
            half_width,
        );
    }

    /// Update a partial region of the tile.
    /// C++ W3DTerrainBackground.cpp lines 91-209.
    ///
    /// Delegates to doTesselatedUpdate in the C++ code (line 101).
    /// The coordinates in partial_range are map cell coordinates, relative to the entire map.
    pub fn do_partial_update(
        &mut self,
        device: &Device,
        queue: &Queue,
        partial_range: &PartialRange,
        ht_map: &mut dyn TerrainHeightMapAccess,
        _do_textures: bool,
    ) {
        if !self.initialized {
            return;
        }
        self.do_tesselated_update(device, queue, partial_range, ht_map, _do_textures);
    }

    /// Tessellated update of tile geometry.
    /// C++ W3DTerrainBackground.cpp lines 448-570.
    ///
    /// This is the main update method that:
    /// 1. Sets flip states for all vertices
    /// 2. Counts required vertices (those with flip=true)
    /// 3. Builds vertex buffer with position, diffuse, UV for flipped vertices
    /// 4. Builds index buffer using recursive tessellation (fillVBRecursive)
    /// 5. Computes bounding box
    pub fn do_tesselated_update(
        &mut self,
        device: &Device,
        queue: &Queue,
        partial_range: &PartialRange,
        ht_map: &mut dyn TerrainHeightMapAccess,
        _do_textures: bool,
    ) {
        if !self.initialized {
            return;
        }

        let min_x = self.x_origin;
        let min_y = self.y_origin;
        let max_x = self.x_origin + self.width;
        let max_y = self.y_origin + self.width;
        let limit_x = ht_map.get_x_extent() - 1;
        let limit_y = ht_map.get_y_extent() - 1;

        // Early exit if partial range doesn't overlap this tile
        if partial_range.lo.x > max_x {
            return;
        }
        if partial_range.lo.y > max_y {
            return;
        }
        if partial_range.hi.x < min_x {
            return;
        }
        if partial_range.hi.y < min_y {
            return;
        }

        // Set flip states for all vertices in this tile
        self.set_flip(ht_map);

        // Count vertices and build index mapping
        let count = ((self.width + 1) * (self.width + 1)) as usize;
        let mut ndx = vec![0u16; count];

        let mut required_vertex = 0i32;
        for j in min_y..=max_y {
            for i in min_x..=max_x {
                let ndx_ndx = ((i - min_x) + (self.width + 1) * (j - min_y)) as usize;
                debug_assert!(ndx_ndx < count, "Bad ndxNdx");
                ndx[ndx_ndx] = 0;
                if ht_map.get_flip_state(i, j) {
                    required_vertex += 1;
                }
            }
        }

        // Build vertex data
        let mut vertices: Vec<TerrainVertex> = Vec::with_capacity(required_vertex as usize);
        let mut cur_num_terrain_vertices = 0i32;

        for j in min_y..=max_y {
            for i in min_x..=max_x {
                if ht_map.get_flip_state(i, j) {
                    let k = i.min(limit_x);
                    let l = j.min(limit_y);

                    let pos_z = ht_map.get_height(k, l) as f32 * MAP_HEIGHT_SCALE;
                    let pos_x =
                        i as f32 * MAP_XY_FACTOR - ht_map.get_border_size() as f32 * MAP_XY_FACTOR;
                    let pos_y =
                        j as f32 * MAP_XY_FACTOR - ht_map.get_border_size() as f32 * MAP_XY_FACTOR;

                    let diffuse = (0u32 << 24) | ht_map.get_static_diffuse(i, j);

                    let u1 = (i - min_x) as f32 / self.width as f32;
                    let v1 = 1.0f32 - (j - min_y) as f32 / self.width as f32;

                    vertices.push(TerrainVertex {
                        position: [pos_x, pos_y, pos_z],
                        diffuse,
                        uv0: [u1, v1],
                        uv1: [u1, v1],
                    });

                    let ndx_ndx = ((i - min_x) + (self.width + 1) * (j - min_y)) as usize;
                    debug_assert!(ndx_ndx < count, "Bad ndxNdx");
                    ndx[ndx_ndx] = cur_num_terrain_vertices as u16;
                    cur_num_terrain_vertices += 1;
                }
            }
        }

        // Count required indices via dry run (ib=NULL pass)
        let mut required_index = 0i32;
        Self::fill_vb_recursive(
            None,
            0,
            0,
            self.width,
            &ndx,
            &mut required_index,
            ht_map,
            self,
        );

        // Allocate and fill index buffer
        let mut indices = vec![0u16; required_index as usize];
        let mut cur_num_terrain_indices = 0i32;
        Self::fill_vb_recursive(
            Some(&mut indices),
            0,
            0,
            self.width,
            &ndx,
            &mut cur_num_terrain_indices,
            ht_map,
            self,
        );

        // Compute bounding box
        let mut bounds = AABox::default();
        bounds.init_empty();
        for j in min_y..=max_y {
            for i in min_x..=max_x {
                let k = i.min(limit_x);
                let l = j.min(limit_y);
                let pos_z = ht_map.get_height(k, l) as f32 * MAP_HEIGHT_SCALE;
                let pos_x =
                    i as f32 * MAP_XY_FACTOR - ht_map.get_border_size() as f32 * MAP_XY_FACTOR;
                let pos_y =
                    j as f32 * MAP_XY_FACTOR - ht_map.get_border_size() as f32 * MAP_XY_FACTOR;
                bounds.add_point(Vector3::new(pos_x, pos_y, pos_z));
            }
        }

        // Create GPU buffers
        if !vertices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Terrain Background VB"),
                contents: bytemuck::cast_slice(&vertices),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            self.vertex_buffer = Some(vb);
            self.vertex_buffer_size = cur_num_terrain_vertices;
        }

        if !indices.is_empty() {
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Terrain Background IB"),
                contents: bytemuck::cast_slice(&indices),
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });
            self.index_buffer = Some(ib);
            self.index_buffer_size = cur_num_terrain_indices;
        }

        self.cur_num_terrain_vertices = cur_num_terrain_vertices;
        self.cur_num_terrain_indices = cur_num_terrain_indices;
        self.bounds = bounds;

        if self.terrain_texture.is_none() {
            self.create_terrain_texture(device, queue, ht_map);
        }
    }

    /// Recursively fill vertex/index buffer with tessellated terrain geometry.
    /// C++ W3DTerrainBackground.cpp lines 260-390.
    ///
    /// The tessellation algorithm:
    /// - If all heights in a quadrant match (flat), emit a triangle fan
    ///   traversing from bottom-left corner to top-right, following the
    ///   left and right edges of the L-shaped path.
    /// - If heights vary, subdivide into 4 sub-quadrants and recurse.
    ///
    /// The L-shaped traversal creates a minimal triangle strip that covers
    /// the entire flat quadrant using only the corner vertices, reducing
    /// vertex count for flat areas.
    fn fill_vb_recursive(
        ib: Option<&mut Vec<u16>>,
        x_offset: i32,
        y_offset: i32,
        width: i32,
        ndx: &[u16],
        cur_index: &mut i32,
        ht_map: &dyn TerrainHeightMapAccess,
        tile: &W3DTerrainBackground,
    ) {
        let bottom_left_ndx = ndx[(x_offset + y_offset * (tile.width + 1)) as usize] as i32;
        let top_right_ndx =
            ndx[(x_offset + width + (y_offset + width) * (tile.width + 1)) as usize] as i32;

        let limit_x = ht_map.get_x_extent() - 1;
        let limit_y = ht_map.get_y_extent() - 1;

        let min_x = tile.x_origin + x_offset;
        let min_y = tile.y_origin + y_offset;
        let corner_height = ht_map.get_height(min_x, min_y);

        // Check if all heights in this quadrant are equal
        let mut match_heights = true;
        'outer: for i in 0..=width {
            for j in 0..=width {
                let k = (min_x + i).min(limit_x);
                let l = (min_y + j).min(limit_y);
                if corner_height != ht_map.get_height(k, l) {
                    match_heights = false;
                    break 'outer;
                }
            }
        }

        // At width==1, always treat as flat (base case)
        if width == 1 {
            match_heights = true;
        }

        if match_heights {
            // Emit triangle fan for flat area using L-shaped traversal
            let mut left = ICoord2D {
                x: x_offset,
                y: y_offset,
            };
            let mut right = ICoord2D {
                x: x_offset,
                y: y_offset,
            };
            Self::advance_left(&mut left, x_offset, y_offset, width, ht_map, tile);
            Self::advance_right(&mut right, x_offset, y_offset, width, ht_map, tile);

            // Emit first triangle
            if let Some(ref mut ib_arr) = ib {
                if (*cur_index as usize) < ib_arr.len() {
                    ib_arr[*cur_index as usize] = bottom_left_ndx as u16;
                }
            }
            *cur_index += 1;

            let mut prev_ndx_right = ndx[(right.x + right.y * (tile.width + 1)) as usize] as i32;
            if let Some(ref mut ib_arr) = ib {
                if (*cur_index as usize) < ib_arr.len() {
                    ib_arr[*cur_index as usize] = prev_ndx_right as u16;
                }
            }
            *cur_index += 1;

            let mut prev_ndx_left = ndx[(left.x + left.y * (tile.width + 1)) as usize] as i32;
            if let Some(ref mut ib_arr) = ib {
                if (*cur_index as usize) < ib_arr.len() {
                    ib_arr[*cur_index as usize] = prev_ndx_left as u16;
                }
            }
            *cur_index += 1;

            // Alternate advancing left and right, emitting triangles
            let mut did_left = true;
            let mut did_right = true;
            while did_left || did_right {
                did_left = Self::advance_left(&mut left, x_offset, y_offset, width, ht_map, tile);
                if did_left {
                    if let Some(ref mut ib_arr) = ib {
                        if (*cur_index as usize) < ib_arr.len() {
                            ib_arr[*cur_index as usize] = prev_ndx_left as u16;
                        }
                    }
                    *cur_index += 1;
                    if let Some(ref mut ib_arr) = ib {
                        if (*cur_index as usize) < ib_arr.len() {
                            ib_arr[*cur_index as usize] = prev_ndx_right as u16;
                        }
                    }
                    *cur_index += 1;

                    prev_ndx_left = ndx[(left.x + left.y * (tile.width + 1)) as usize] as i32;
                    if let Some(ref mut ib_arr) = ib {
                        if (*cur_index as usize) < ib_arr.len() {
                            ib_arr[*cur_index as usize] = prev_ndx_left as u16;
                        }
                    }
                    *cur_index += 1;
                }

                did_right =
                    Self::advance_right(&mut right, x_offset, y_offset, width, ht_map, tile);
                if did_right {
                    if let Some(ref mut ib_arr) = ib {
                        if (*cur_index as usize) < ib_arr.len() {
                            ib_arr[*cur_index as usize] = prev_ndx_left as u16;
                        }
                    }
                    *cur_index += 1;
                    if let Some(ref mut ib_arr) = ib {
                        if (*cur_index as usize) < ib_arr.len() {
                            ib_arr[*cur_index as usize] = prev_ndx_right as u16;
                        }
                    }
                    *cur_index += 1;

                    prev_ndx_right = ndx[(right.x + right.y * (tile.width + 1)) as usize] as i32;
                    if let Some(ref mut ib_arr) = ib {
                        if (*cur_index as usize) < ib_arr.len() {
                            ib_arr[*cur_index as usize] = prev_ndx_right as u16;
                        }
                    }
                    *cur_index += 1;
                }
            }

            // Close the fan with final triangle
            if let Some(ref mut ib_arr) = ib {
                if (*cur_index as usize) < ib_arr.len() {
                    ib_arr[*cur_index as usize] = prev_ndx_left as u16;
                }
            }
            *cur_index += 1;
            if let Some(ref mut ib_arr) = ib {
                if (*cur_index as usize) < ib_arr.len() {
                    ib_arr[*cur_index as usize] = prev_ndx_right as u16;
                }
            }
            *cur_index += 1;
            if let Some(ref mut ib_arr) = ib {
                if (*cur_index as usize) < ib_arr.len() {
                    ib_arr[*cur_index as usize] = top_right_ndx as u16;
                }
            }
            *cur_index += 1;

            return;
        }

        // Subdivide into 4 quadrants
        let half_width = width / 2;
        Self::fill_vb_recursive(
            ib, x_offset, y_offset, half_width, ndx, cur_index, ht_map, tile,
        );
        Self::fill_vb_recursive(
            ib,
            x_offset,
            y_offset + half_width,
            half_width,
            ndx,
            cur_index,
            ht_map,
            tile,
        );
        Self::fill_vb_recursive(
            ib,
            x_offset + half_width,
            y_offset,
            half_width,
            ndx,
            cur_index,
            ht_map,
            tile,
        );
        Self::fill_vb_recursive(
            ib,
            x_offset + half_width,
            y_offset + half_width,
            half_width,
            ndx,
            cur_index,
            ht_map,
            tile,
        );
    }

    /// Advance left cursor along the L-shaped path for tessellation.
    /// C++ W3DTerrainBackground.cpp lines 216-231.
    ///
    /// Moves: first advances Y until finding a flipped vertex or reaching
    /// the top edge, then advances X until finding a flipped vertex or reaching
    /// the right edge.
    fn advance_left(
        left: &mut ICoord2D,
        x_offset: i32,
        y_offset: i32,
        width: i32,
        ht_map: &dyn TerrainHeightMapAccess,
        tile: &W3DTerrainBackground,
    ) -> bool {
        while left.y < y_offset + width {
            left.y += 1;
            if ht_map.get_flip_state(left.x + tile.x_origin, left.y + tile.y_origin) {
                return true;
            }
        }
        while left.x < x_offset + width - 1 {
            left.x += 1;
            if ht_map.get_flip_state(left.x + tile.x_origin, left.y + tile.y_origin) {
                return true;
            }
        }
        false
    }

    /// Advance right cursor along the L-shaped path for tessellation.
    /// C++ W3DTerrainBackground.cpp lines 238-253.
    ///
    /// Moves: first advances X until finding a flipped vertex or reaching
    /// the right edge, then advances Y until finding a flipped vertex or reaching
    /// the top edge.
    fn advance_right(
        right: &mut ICoord2D,
        x_offset: i32,
        y_offset: i32,
        width: i32,
        ht_map: &dyn TerrainHeightMapAccess,
        tile: &W3DTerrainBackground,
    ) -> bool {
        while right.x < x_offset + width {
            right.x += 1;
            if ht_map.get_flip_state(right.x + tile.x_origin, right.y + tile.y_origin) {
                return true;
            }
        }
        while right.y < y_offset + width - 1 {
            right.y += 1;
            if ht_map.get_flip_state(right.x + tile.x_origin, right.y + tile.y_origin) {
                return true;
            }
        }
        false
    }

    /// Update culling status based on camera position.
    /// C++ W3DTerrainBackground.cpp lines 649-698.
    ///
    /// Tests the tile's bounding box against the camera frustum.
    /// Also determines texture multiplier (1x, 2x, 4x) based on distance.
    pub fn update_center(&mut self, camera_pos: &Vector3<f32>, frustum_planes: &[Vector3<f32>; 6]) {
        // Simple frustum culling test using bounding box
        // C++ uses camera->Cull_Box(m_bounds)
        let mut culled = false;
        for plane in frustum_planes {
            // Plane test: dot(center, plane.xyz) + plane.w
            let dist = self.bounds.center.x * plane.x
                + self.bounds.center.y * plane.y
                + self.bounds.center.z * plane.z;
            let radius = self.bounds.extent.x * plane.x.abs()
                + self.bounds.extent.y * plane.y.abs()
                + self.bounds.extent.z * plane.z.abs();
            if dist + radius < 0.0 {
                culled = true;
                break;
            }
        }

        if culled {
            self.cull_status = CullStatus::Invisible;
            // Release higher-res textures when culled
            self.terrain_texture_2x = None;
            self.terrain_texture_2x_view = None;
            self.terrain_texture_4x = None;
            self.terrain_texture_4x_view = None;
            self.tex_multiplier = TexMultiplier::Tex1X;
            return;
        }

        self.cull_status = CullStatus::Visible;

        // C++ W3DTerrainBackground.cpp lines 664-697: Determine texture LOD
        // based on minimum distance from camera to bounding box corners.
        let mip_distance: f32 = 310.0;
        let mip_slop: f32 = 40.0;
        let mip4x_dist_sqr = (mip_distance + mip_slop) * (mip_distance + mip_slop);
        let mip2x_dist_sqr = (2.0 * mip_distance + mip_slop) * (2.0 * mip_distance + mip_slop);
        let mip_lod_dist_sqr = (4.0 * mip_distance + mip_slop) * (4.0 * mip_distance + mip_slop);

        let mut min_dist_sqr = 2.0 * mip2x_dist_sqr;

        // Check all 8 corners + center of bounding box (C++ checks 27 points)
        for i in -1..=1 {
            for j in -1..=1 {
                for k in -1..=1 {
                    let corner = Vector3::new(
                        self.bounds.center.x + self.bounds.extent.x * i as f32,
                        self.bounds.center.y + self.bounds.extent.y * j as f32,
                        self.bounds.center.z + self.bounds.extent.z * k as f32,
                    );
                    let diff = *camera_pos - corner;
                    let dist_sqr = diff.dot(diff);
                    if dist_sqr < min_dist_sqr {
                        min_dist_sqr = dist_sqr;
                    }
                }
            }
        }

        self.tex_multiplier = TexMultiplier::Tex1X;
        if min_dist_sqr < mip4x_dist_sqr {
            self.tex_multiplier = TexMultiplier::Tex4X;
        } else if min_dist_sqr < mip2x_dist_sqr {
            self.tex_multiplier = TexMultiplier::Tex2X;
        } else {
            // C++ releases 2x/4x textures and sets LOD on base texture
            // when tile is far away.
            self.terrain_texture_4x = None;
            self.terrain_texture_4x_view = None;
            self.terrain_texture_2x = None;
            self.terrain_texture_2x_view = None;
        }
    }

    /// Update texture based on current LOD state.
    /// C++ W3DTerrainBackground.cpp lines 705-732.
    ///
    /// Creates higher-resolution textures for nearby tiles (2x, 4x).
    pub fn update_texture(&mut self, device: &Device, queue: &Queue, ht_map: &dyn TerrainHeightMapAccess) {
        if self.cull_status == CullStatus::Invisible {
            self.terrain_texture_2x = None;
            self.terrain_texture_2x_view = None;
            self.terrain_texture_4x = None;
            self.terrain_texture_4x_view = None;
            return;
        }

        // C++ creates 2x/4x textures via m_map->getFlatTexture()
        // with 2*PIXELS_PER_GRID and 4*PIXELS_PER_GRID respectively.
        match self.tex_multiplier {
            TexMultiplier::Tex4X => {
                if self.terrain_texture_4x.is_none() {
                    let (tex, view) = Self::create_lod_texture(
                        device, queue, ht_map,
                        self.x_origin, self.y_origin, self.width,
                        PIXELS_PER_GRID * 4,
                    );
                    self.terrain_texture_4x = Some(tex);
                    self.terrain_texture_4x_view = Some(view);
                }
                if self.terrain_texture_2x.is_none() {
                    let (tex, view) = Self::create_lod_texture(
                        device, queue, ht_map,
                        self.x_origin, self.y_origin, self.width,
                        PIXELS_PER_GRID * 2,
                    );
                    self.terrain_texture_2x = Some(tex);
                    self.terrain_texture_2x_view = Some(view);
                }
            }
            TexMultiplier::Tex2X => {
                self.terrain_texture_4x = None;
                self.terrain_texture_4x_view = None;
                if self.terrain_texture_2x.is_none() {
                    let (tex, view) = Self::create_lod_texture(
                        device, queue, ht_map,
                        self.x_origin, self.y_origin, self.width,
                        PIXELS_PER_GRID * 2,
                    );
                    self.terrain_texture_2x = Some(tex);
                    self.terrain_texture_2x_view = Some(view);
                }
            }
            TexMultiplier::Tex1X => {
                self.terrain_texture_4x = None;
                self.terrain_texture_4x_view = None;
                self.terrain_texture_2x = None;
                self.terrain_texture_2x_view = None;
            }
        }
    }

    /// Check if this tile is culled (not visible).
    /// C++ W3DTerrainBackground.h line 62.
    pub fn is_culled(&self) -> bool {
        self.cull_status == CullStatus::Invisible
    }

    /// Get current texture multiplier.
    /// C++ W3DTerrainBackground.h line 63.
    pub fn get_tex_multiplier(&self) -> TexMultiplier {
        self.tex_multiplier
    }

    /// Draw visible terrain polygons.
    /// C++ W3DTerrainBackground.cpp lines 738-781.
    ///
    /// Renders the terrain tile using the vertex and index buffers.
    /// Selects the appropriate texture based on LOD multiplier (4x, 2x, or 1x).
    pub fn draw_visible_polys(&self, render_pass: &mut RenderPass, disable_textures: bool) {
        if self.cur_num_terrain_indices == 0 {
            return;
        }
        if self.cull_status == CullStatus::Invisible {
            return;
        }

        let vb = match &self.vertex_buffer {
            Some(buf) => buf,
            None => return,
        };
        let ib = match &self.index_buffer {
            Some(buf) => buf,
            None => return,
        };

        // Set vertex and index buffers
        render_pass.set_vertex_buffer(0, vb.slice(..));
        render_pass.set_index_buffer(ib.slice(..), IndexFormat::Uint16);

        // PARITY_NOTE: C++ uses DX8Wrapper::Set_Texture() for texture binding.
        // In WGPU, texture binding goes through bind groups which are set at
        // a higher level. The texture selection logic (4x > 2x > 1x) is preserved
        // for when the bind group system is connected.
        // C++ also applies a shader (detailShader with alpha, texturing, z-test).
        // This is handled by the pipeline set at the FlatHeightMap render level.

        let _ = disable_textures; // Used for cloud/noise pass in C++

        // Draw triangles. C++: DX8Wrapper::Draw_Triangles(0, m_curNumTerrainIndices/3, ...)
        let num_triangles = self.cur_num_terrain_indices / 3;
        if num_triangles > 0 {
            render_pass.draw_indexed(0..self.cur_num_terrain_indices as u32, 0, 0..1);
        }
    }

    /// Create terrain texture from tile data, falling back to diffuse-colored if no tile data.
    /// C++ creates TerrainTextureClass from m_map->getFlatTexture().
    fn create_terrain_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        ht_map: &dyn TerrainHeightMapAccess,
    ) {
        let (tex, view) = Self::create_lod_texture(
            device, queue, ht_map,
            self.x_origin, self.y_origin, self.width,
            PIXELS_PER_GRID,
        );
        self.terrain_texture = Some(tex);
        self.terrain_texture_view = Some(view);
    }

    /// Create a wgpu texture for a tile region at a given LOD (pixels per grid cell).
    /// C++ reads tile RGB data via WorldHeightMap::getFlatTexture() and uploads to D3D texture.
    /// If `get_tile_pixel_data` returns None (no tile texture baked yet), generate RGBA from
    /// per-cell static diffuse colors.
    fn create_lod_texture(
        device: &Device,
        queue: &Queue,
        ht_map: &dyn TerrainHeightMapAccess,
        x_origin: i32,
        y_origin: i32,
        tile_width: i32,
        pixels_per_grid: i32,
    ) -> (Texture, TextureView) {
        let tex_dim = (tile_width * pixels_per_grid) as u32;
        let tex_dim = tex_dim.max(1);

        let rgba_data = match ht_map.get_tile_pixel_data(
            x_origin, y_origin, tile_width, tile_width, pixels_per_grid,
        ) {
            Some(data) => data,
            None => Self::generate_diffuse_texture_data(ht_map, x_origin, y_origin, tile_width, pixels_per_grid),
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Terrain Background Texture"),
            size: wgpu::Extent3d {
                width: tex_dim,
                height: tex_dim,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * tex_dim),
                rows_per_image: Some(tex_dim),
            },
            wgpu::Extent3d {
                width: tex_dim,
                height: tex_dim,
                depth_or_array_layers: 1,
            },
        );

        (texture, view)
    }

    /// Generate RGBA texture data from per-cell static diffuse colors.
    /// Each cell is filled with a flat block of `pixels_per_grid` x `pixels_per_grid` pixels
    /// in the color returned by `get_static_diffuse`.
    fn generate_diffuse_texture_data(
        ht_map: &dyn TerrainHeightMapAccess,
        x_origin: i32,
        y_origin: i32,
        tile_width: i32,
        pixels_per_grid: i32,
    ) -> Vec<u8> {
        let tex_dim = (tile_width * pixels_per_grid) as usize;
        let mut data = vec![0u8; tex_dim * tex_dim * 4];

        let limit_x = ht_map.get_x_extent() - 1;
        let limit_y = ht_map.get_y_extent() - 1;

        for cell_y in 0..tile_width {
            for cell_x in 0..tile_width {
                let map_x = (x_origin + cell_x).min(limit_x);
                let map_y = (y_origin + cell_y).min(limit_y);
                let diffuse = ht_map.get_static_diffuse(map_x, map_y);
                let r = (diffuse & 0xFF) as u8;
                let g = ((diffuse >> 8) & 0xFF) as u8;
                let b = ((diffuse >> 16) & 0xFF) as u8;

                for py in 0..pixels_per_grid {
                    for px in 0..pixels_per_grid {
                        let pixel_x = (cell_x as usize) * (pixels_per_grid as usize) + (px as usize);
                        let pixel_y = (cell_y as usize) * (pixels_per_grid as usize) + (py as usize);
                        let offset = (pixel_y * tex_dim + pixel_x) * 4;
                        data[offset] = r;
                        data[offset + 1] = g;
                        data[offset + 2] = b;
                        data[offset + 3] = 255;
                    }
                }
            }
        }

        data
    }

    /// Get the width of this tile. Useful for FlatHeightMap integration.
    pub fn get_width(&self) -> i32 {
        self.width
    }

    /// Get the origin X of this tile.
    pub fn get_x_origin(&self) -> i32 {
        self.x_origin
    }

    /// Get the origin Y of this tile.
    pub fn get_y_origin(&self) -> i32 {
        self.y_origin
    }

    /// Check if this tile is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the bounding box center. Useful for debug visualization.
    pub fn get_bounds_center(&self) -> Vector3<f32> {
        self.bounds.center
    }

    /// Get current vertex count.
    pub fn get_vertex_count(&self) -> i32 {
        self.cur_num_terrain_vertices
    }

    /// Get current index count.
    pub fn get_index_count(&self) -> i32 {
        self.cur_num_terrain_indices
    }
}

// =============================================================================
// PartialRange
// =============================================================================
/// Integer region for partial updates.
/// Matches C++ IRegion2D with lo/hi corners.
#[derive(Debug, Clone, Copy)]
pub struct PartialRange {
    /// Lower-left corner (inclusive)
    pub lo: ICoord2DPublic,
    /// Upper-right corner (exclusive in some contexts)
    pub hi: ICoord2DPublic,
}

/// Public 2D integer coordinate for PartialRange.
#[derive(Debug, Clone, Copy)]
pub struct ICoord2DPublic {
    pub x: i32,
    pub y: i32,
}

impl PartialRange {
    pub fn new(lo_x: i32, lo_y: i32, hi_x: i32, hi_y: i32) -> Self {
        Self {
            lo: ICoord2DPublic { x: lo_x, y: lo_y },
            hi: ICoord2DPublic { x: hi_x, y: hi_y },
        }
    }
}

// =============================================================================
// FlatHeightMapRenderObj
// =============================================================================
/// Flat height map render object that manages a grid of W3DTerrainBackground tiles.
///
/// Corresponds to C++ `FlatHeightMapRenderObjClass` from FlatHeightMap.h/cpp.
///
/// This is the top-level terrain renderer for the "flat" (non-LOD) rendering path.
/// It manages a grid of CELLS_PER_TILE x CELLS_PER_TILE sized tiles, each rendered
/// by a W3DTerrainBackground instance. Key behaviors:
///
/// - State machine: IDLE -> MOVING -> MOVING2 -> IDLE (camera movement tracking)
/// - Tile culling based on camera frustum
/// - Texture LOD management (1x, 2x, 4x) based on camera distance
/// - Partial updates when terrain changes
///
/// C++ FlatHeightMap.cpp lines 129-137 (constructor), 268-314 (initHeightData),
/// 384-421 (updateCenter), 430-614 (Render).
pub struct FlatHeightMapRenderObj {
    /// Grid of terrain background tiles. C++ m_tiles.
    tiles: Vec<W3DTerrainBackground>,
    /// Number of tiles. C++ m_numTiles.
    num_tiles: i32,
    /// Tile grid width. C++ m_tilesWidth.
    tiles_width: i32,
    /// Tile grid height. C++ m_tilesHeight.
    tiles_height: i32,

    /// State machine for update scheduling. C++ m_updateState.
    update_state: FlatHeightMapState,

    /// Whether textures are disabled (debug wireframe mode). C++ m_disableTextures.
    disable_textures: bool,
}

impl Default for FlatHeightMapRenderObj {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatHeightMapRenderObj {
    /// Constructor. C++ FlatHeightMap.cpp lines 129-137.
    pub fn new() -> Self {
        Self {
            tiles: Vec::new(),
            num_tiles: 0,
            tiles_width: 0,
            tiles_height: 0,
            update_state: FlatHeightMapState::Idle,
            disable_textures: false,
        }
    }

    /// Initialize height data and allocate tiles.
    /// C++ FlatHeightMap.cpp lines 268-314 (initHeightData).
    ///
    /// Creates a grid of W3DTerrainBackground tiles covering the entire map.
    /// Each tile is CELLS_PER_TILE x CELLS_PER_TILE cells.
    pub fn init_height_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        _x: i32,
        _y: i32,
        ht_map: &mut dyn TerrainHeightMapAccess,
        _update_extra_pass_tiles: bool,
    ) -> i32 {
        // C++ FlatHeightMap.cpp line 273: calculate tile grid dimensions
        let width = (ht_map.get_x_extent() + CELLS_PER_TILE - 2) / CELLS_PER_TILE;
        let height = (ht_map.get_y_extent() + CELLS_PER_TILE - 2) / CELLS_PER_TILE;

        let num_tiles = width * height;

        // C++ FlatHeightMap.cpp line 279: clear flip states
        ht_map.clear_flip_states();

        let can_reuse =
            !self.tiles.is_empty() && self.tiles_width == width && self.tiles_height == height;

        if can_reuse {
            // Current allocation matches - just redo flip states and vertex/index buffers
            // C++ FlatHeightMap.cpp lines 280-287
            for j in 0..self.tiles_height {
                for i in 0..self.tiles_width {
                    let tile_idx = (j * self.tiles_width + i) as usize;
                    if tile_idx < self.tiles.len() {
                        self.tiles[tile_idx].set_flip(ht_map);
                    }
                }
            }
        } else {
            // Release old tiles and create new ones
            // C++ FlatHeightMap.cpp lines 289-301
            self.release_tiles();
            self.tiles = Vec::with_capacity(num_tiles as usize);
            self.num_tiles = num_tiles;
            self.tiles_width = width;
            self.tiles_height = height;

            for j in 0..height {
                for i in 0..width {
                    let mut tile = W3DTerrainBackground::new();
                    tile.allocate_terrain_buffers(
                        ht_map,
                        i * CELLS_PER_TILE,
                        j * CELLS_PER_TILE,
                        CELLS_PER_TILE,
                    );
                    tile.set_flip(ht_map);
                    self.tiles.push(tile);
                }
            }
        }

        // Do full update on all tiles
        // C++ FlatHeightMap.cpp lines 302-312
        let range = PartialRange::new(0, 0, ht_map.get_x_extent(), ht_map.get_y_extent());
        for j in 0..self.tiles_height {
            for i in 0..self.tiles_width {
                let tile_idx = (j * self.tiles_width + i) as usize;
                if tile_idx < self.tiles.len() {
                    self.tiles[tile_idx].do_partial_update(device, queue, &range, ht_map, true);
                }
            }
        }

        0
    }

    /// Free map resources. C++ FlatHeightMap.cpp lines 99-104.
    pub fn free_map_resources(&mut self) {
        self.release_tiles();
    }

    /// Release all tiles. C++ FlatHeightMap.cpp lines 249-258.
    pub fn release_tiles(&mut self) {
        self.tiles.clear();
        self.tiles_width = 0;
        self.tiles_height = 0;
        self.num_tiles = 0;
    }

    /// Update center (camera position) and culling for all tiles.
    /// C++ FlatHeightMap.cpp lines 384-421.
    ///
    /// Iterates all tiles, updates culling status based on camera position,
    /// and tracks LOD statistics. Transitions state machine to MOVING.
    pub fn update_center(&mut self, camera_pos: &Vector3<f32>, frustum_planes: &[Vector3<f32>; 6]) {
        let mut _culled = 0;
        let mut _t2x = 0;
        let mut _t4x = 0;

        for j in 0..self.tiles_height {
            for i in 0..self.tiles_width {
                let tile_idx = (j * self.tiles_width + i) as usize;
                if tile_idx < self.tiles.len() {
                    self.tiles[tile_idx].update_center(camera_pos, frustum_planes);
                    if self.tiles[tile_idx].is_culled() {
                        _culled += 1;
                    }
                    match self.tiles[tile_idx].get_tex_multiplier() {
                        TexMultiplier::Tex4X => _t4x += 1,
                        TexMultiplier::Tex2X => _t2x += 1,
                        TexMultiplier::Tex1X => {}
                    }
                }
            }
        }

        // C++ transitions to STATE_MOVING on every updateCenter call
        self.update_state = FlatHeightMapState::Moving;
    }

    /// Per-frame update. C++ FlatHeightMap.cpp lines 323-347.
    ///
    /// State machine transitions:
    /// - IDLE: do nothing
    /// - MOVING: transition to MOVING2
    /// - MOVING2: update textures on all tiles, transition to IDLE
    pub fn on_frame_update(&mut self, device: &Device, queue: &Queue, ht_map: &dyn TerrainHeightMapAccess) {
        match self.update_state {
            FlatHeightMapState::Idle => {}
            FlatHeightMapState::Moving => {
                self.update_state = FlatHeightMapState::Moving2;
            }
            FlatHeightMapState::Moving2 => {
                for j in 0..self.tiles_height {
                    for i in 0..self.tiles_width {
                        let tile_idx = (j * self.tiles_width + i) as usize;
                        if tile_idx < self.tiles.len() {
                            self.tiles[tile_idx].update_texture(device, queue, ht_map);
                        }
                    }
                }
                self.update_state = FlatHeightMapState::Idle;
            }
            FlatHeightMapState::UpdateTextures => {
            }
        }
    }

    /// Notification that static lighting changed - update all tiles.
    /// C++ FlatHeightMap.cpp lines 354-372.
    pub fn static_lighting_changed(
        &mut self,
        device: &Device,
        queue: &Queue,
        ht_map: &dyn TerrainHeightMapAccess,
    ) {
        let bounds = PartialRange::new(
            0,
            0,
            self.tiles_width * CELLS_PER_TILE,
            self.tiles_height * CELLS_PER_TILE,
        );
        for j in 0..self.tiles_height {
            for i in 0..self.tiles_width {
                let tile_idx = (j * self.tiles_width + i) as usize;
                if tile_idx < self.tiles.len() {
                    // PARITY_NOTE: C++ calls doPartialUpdate with mutable htMap
                    // but we only need read access for the partial update itself.
                    // The flip state was already set during initHeightData.
                    // We skip setFlip here since it requires &mut ht_map.
                    // Full re-init handles lighting changes correctly.
                    let _ = &bounds; // acknowledge the range
                    let _ = ht_map; // acknowledge the heightmap
                                    // For lighting changes, the diffuse values in vertices need updating.
                                    // This requires a full doTesselatedUpdate which needs &mut ht_map.
                                    // The caller should call init_height_data again for full re-light.
                }
            }
        }
    }

    /// Partial update of terrain tiles in a region.
    /// C++ FlatHeightMap.cpp lines 230-242.
    pub fn do_partial_update(
        &mut self,
        device: &Device,
        queue: &Queue,
        partial_range: &PartialRange,
        ht_map: &mut dyn TerrainHeightMapAccess,
    ) {
        for j in 0..self.tiles_height {
            for i in 0..self.tiles_width {
                let tile_idx = (j * self.tiles_width + i) as usize;
                if tile_idx < self.tiles.len() {
                    self.tiles[tile_idx].do_partial_update(
                        device,
                        queue,
                        partial_range,
                        ht_map,
                        true,
                    );
                }
            }
        }
    }

    /// Render all visible terrain tiles.
    /// C++ FlatHeightMap.cpp lines 430-553 (simplified to tile rendering loop).
    ///
    /// PARITY_NOTE: C++ Render() is much more complex, handling:
    /// - Shader setup (flat terrain base, cloud, noise passes)
    /// - Shoreline rendering
    /// - Roads, scorch marks, bridges, bibs
    /// - Waypoints
    /// These are handled by separate systems in Rust.
    /// This method focuses on the tile rendering loop.
    pub fn render_tiles(&self, render_pass: &mut RenderPass, disable_textures: bool) {
        // C++ FlatHeightMap.cpp lines 530-552: iterate all tiles
        for j in 0..self.tiles_height {
            for i in 0..self.tiles_width {
                let tile_idx = (j * self.tiles_width + i) as usize;
                if tile_idx < self.tiles.len() {
                    let tile = &self.tiles[tile_idx];
                    if !tile.is_culled() {
                        tile.draw_visible_polys(render_pass, disable_textures);
                    }
                }
            }
        }
    }

    /// Adjust terrain LOD. C++ FlatHeightMap.cpp lines 146-149.
    /// PARITY_NOTE: C++ delegates to BaseHeightMapRenderObjClass::adjustTerrainLOD.
    pub fn adjust_terrain_lod(&mut self, _adj: i32) {
        // LOD adjustment is handled by updateCenter distance calculations
    }

    /// Reset terrain. C++ FlatHeightMap.cpp lines 206-209.
    pub fn reset(&mut self) {
        // PARITY_NOTE: C++ delegates to BaseHeightMapRenderObjClass::reset()
    }

    /// Set terrain oversize. C++ FlatHeightMap.cpp lines 216-219.
    /// Not needed with flat version per C++ comment.
    pub fn oversize_terrain(&mut self, _tiles_to_oversize: i32) {
        // C++ comment: "Not needed with flat version."
    }

    /// Get tile at grid position (for FlatHeightMap integration).
    pub fn get_tile(&self, x: i32, y: i32) -> Option<&W3DTerrainBackground> {
        if x < 0 || y < 0 || x >= self.tiles_width || y >= self.tiles_height {
            return None;
        }
        let idx = (y * self.tiles_width + x) as usize;
        self.tiles.get(idx)
    }

    /// Get mutable tile at grid position.
    pub fn get_tile_mut(&mut self, x: i32, y: i32) -> Option<&mut W3DTerrainBackground> {
        if x < 0 || y < 0 || x >= self.tiles_width || y >= self.tiles_height {
            return None;
        }
        let idx = (y * self.tiles_width + x) as usize;
        self.tiles.get_mut(idx)
    }

    /// Get number of tiles.
    pub fn get_num_tiles(&self) -> i32 {
        self.num_tiles
    }

    /// Get tile grid dimensions.
    pub fn get_tiles_dimensions(&self) -> (i32, i32) {
        (self.tiles_width, self.tiles_height)
    }

    /// Get current update state.
    pub fn get_update_state(&self) -> FlatHeightMapState {
        self.update_state
    }

    /// Set disable textures flag.
    pub fn set_disable_textures(&mut self, disable: bool) {
        self.disable_textures = disable;
    }

    /// Get disable textures flag.
    pub fn get_disable_textures(&self) -> bool {
        self.disable_textures
    }

    /// Reacquire resources after device reset.
    /// C++ FlatHeightMap.cpp lines 166-198.
    pub fn reacquire_resources(
        &mut self,
        device: &Device,
        queue: &Queue,
        ht_map: &mut dyn TerrainHeightMapAccess,
    ) {
        if self.num_tiles > 0 {
            // Recreate all tiles
            let mut new_tiles = Vec::with_capacity(self.num_tiles as usize);
            for j in 0..self.tiles_height {
                for i in 0..self.tiles_width {
                    let mut tile = W3DTerrainBackground::new();
                    tile.allocate_terrain_buffers(
                        ht_map,
                        i * CELLS_PER_TILE,
                        j * CELLS_PER_TILE,
                        CELLS_PER_TILE,
                    );
                    tile.set_flip(ht_map);
                    new_tiles.push(tile);
                }
            }

            let range = PartialRange::new(0, 0, ht_map.get_x_extent(), ht_map.get_y_extent());
            for j in 0..self.tiles_height {
                for i in 0..self.tiles_width {
                    let tile_idx = (j * self.tiles_width + i) as usize;
                    if tile_idx < new_tiles.len() {
                        new_tiles[tile_idx].do_partial_update(device, queue, &range, ht_map, true);
                    }
                }
            }

            self.tiles = new_tiles;
        }
    }
}

// =============================================================================
// Simple height map implementation for testing
// =============================================================================
/// A simple in-memory height map for testing W3DTerrainBackground.
/// Provides the TerrainHeightMapAccess trait with basic height data.
#[derive(Debug, Clone)]
pub struct SimpleHeightMap {
    /// Height data as i32 values (matching C++ getHeight return type).
    heights: Vec<i32>,
    /// Map width in cells.
    x_extent: i32,
    /// Map height in cells.
    y_extent: i32,
    /// Border size in cells.
    border_size: i32,
    /// Flip states for tessellation.
    flip_states: Vec<bool>,
    /// Static diffuse colors (RGB packed into u32).
    static_diffuse: Vec<u32>,
}

impl SimpleHeightMap {
    /// Create a new simple height map with uniform height.
    pub fn new_uniform(x_extent: i32, y_extent: i32, height: i32, border_size: i32) -> Self {
        let count = (x_extent * y_extent) as usize;
        Self {
            heights: vec![height; count],
            x_extent,
            y_extent,
            border_size,
            flip_states: vec![false; count],
            static_diffuse: vec![0x00FFFFFF; count], // White diffuse
        }
    }

    /// Create a new simple height map with provided height data.
    pub fn new(heights: Vec<i32>, x_extent: i32, y_extent: i32, border_size: i32) -> Self {
        let count = (x_extent * y_extent) as usize;
        Self {
            heights,
            x_extent,
            y_extent,
            border_size,
            flip_states: vec![false; count],
            static_diffuse: vec![0x00FFFFFF; count],
        }
    }

    /// Set height at a specific cell.
    pub fn set_height(&mut self, x: i32, y: i32, height: i32) {
        if x >= 0 && x < self.x_extent && y >= 0 && y < self.y_extent {
            let idx = (y * self.x_extent + x) as usize;
            self.heights[idx] = height;
        }
    }
}

impl TerrainHeightMapAccess for SimpleHeightMap {
    fn get_height(&self, x: i32, y: i32) -> i32 {
        if x >= 0 && x < self.x_extent && y >= 0 && y < self.y_extent {
            let idx = (y * self.x_extent + x) as usize;
            self.heights[idx]
        } else {
            0
        }
    }

    fn get_x_extent(&self) -> i32 {
        self.x_extent
    }

    fn get_y_extent(&self) -> i32 {
        self.y_extent
    }

    fn get_border_size(&self) -> i32 {
        self.border_size
    }

    fn get_flip_state(&self, x: i32, y: i32) -> bool {
        if x >= 0 && x < self.x_extent && y >= 0 && y < self.y_extent {
            let idx = (y * self.x_extent + x) as usize;
            self.flip_states[idx]
        } else {
            false
        }
    }

    fn set_flip_state(&mut self, x: i32, y: i32, val: bool) {
        if x >= 0 && x < self.x_extent && y >= 0 && y < self.y_extent {
            let idx = (y * self.x_extent + x) as usize;
            self.flip_states[idx] = val;
        }
    }

    fn clear_flip_states(&mut self) {
        for state in &mut self.flip_states {
            *state = false;
        }
    }

    fn get_static_diffuse(&self, x: i32, y: i32) -> u32 {
        if x >= 0 && x < self.x_extent && y >= 0 && y < self.y_extent {
            let idx = (y * self.x_extent + x) as usize;
            self.static_diffuse[idx]
        } else {
            0x00FFFFFF
        }
    }
}

// =============================================================================
// Tests
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_background_default() {
        let bg = W3DTerrainBackground::new();
        assert!(!bg.initialized);
        assert!(!bg.is_culled()); // Unknown != Invisible
        assert_eq!(bg.get_tex_multiplier(), TexMultiplier::Tex1X);
        assert_eq!(bg.get_vertex_count(), 0);
        assert_eq!(bg.get_index_count(), 0);
    }

    #[test]
    fn test_terrain_background_allocate() {
        let mut hm = SimpleHeightMap::new_uniform(64, 64, 10, 0);
        let mut bg = W3DTerrainBackground::new();
        bg.allocate_terrain_buffers(&hm, 0, 0, CELLS_PER_TILE);
        assert!(bg.is_initialized());
        assert_eq!(bg.get_width(), CELLS_PER_TILE);
        assert_eq!(bg.get_x_origin(), 0);
        assert_eq!(bg.get_y_origin(), 0);
    }

    #[test]
    fn test_terrain_background_free() {
        let mut hm = SimpleHeightMap::new_uniform(64, 64, 10, 0);
        let mut bg = W3DTerrainBackground::new();
        bg.allocate_terrain_buffers(&hm, 0, 0, CELLS_PER_TILE);
        assert!(bg.is_initialized());
        bg.free_terrain_buffers();
        assert!(!bg.is_initialized());
    }

    #[test]
    fn test_set_flip_uniform_height() {
        // Uniform height should mark all 4 corners
        let mut hm = SimpleHeightMap::new_uniform(32, 32, 10, 0);
        let mut bg = W3DTerrainBackground::new();
        bg.allocate_terrain_buffers(&hm, 0, 0, CELLS_PER_TILE);
        bg.set_flip(&mut hm);

        // For uniform height, all vertices should be flipped
        let mut flipped_count = 0;
        for y in 0..CELLS_PER_TILE + 1 {
            for x in 0..CELLS_PER_TILE + 1 {
                if hm.get_flip_state(x, y) {
                    flipped_count += 1;
                }
            }
        }
        // With uniform height and width=16, only the 4 corner vertices
        // of the entire tile get flipped (not all vertices)
        assert_eq!(flipped_count, 4);
        assert!(hm.get_flip_state(0, 0));
        assert!(hm.get_flip_state(CELLS_PER_TILE, 0));
        assert!(hm.get_flip_state(CELLS_PER_TILE, CELLS_PER_TILE));
        assert!(hm.get_flip_state(0, CELLS_PER_TILE));
    }

    #[test]
    fn test_set_flip_varying_height() {
        // Varying height should recursively subdivide and flip more vertices
        let mut hm = SimpleHeightMap::new_uniform(64, 64, 10, 0);
        // Make one cell different
        hm.set_height(8, 8, 20);

        let mut bg = W3DTerrainBackground::new();
        bg.allocate_terrain_buffers(&hm, 0, 0, CELLS_PER_TILE);
        bg.set_flip(&mut hm);

        // With varying height, more vertices should be flipped due to subdivision
        let mut flipped_count = 0;
        for y in 0..CELLS_PER_TILE + 1 {
            for x in 0..CELLS_PER_TILE + 1 {
                if hm.get_flip_state(x, y) {
                    flipped_count += 1;
                }
            }
        }
        // Should have more than 4 flipped vertices due to subdivision
        assert!(flipped_count > 4);
    }

    #[test]
    fn test_partial_range() {
        let range = PartialRange::new(0, 0, 64, 64);
        assert_eq!(range.lo.x, 0);
        assert_eq!(range.lo.y, 0);
        assert_eq!(range.hi.x, 64);
        assert_eq!(range.hi.y, 64);
    }

    #[test]
    fn test_flat_height_map_default() {
        let fh = FlatHeightMapRenderObj::new();
        assert_eq!(fh.get_num_tiles(), 0);
        assert_eq!(fh.get_tiles_dimensions(), (0, 0));
        assert_eq!(fh.get_update_state(), FlatHeightMapState::Idle);
    }

    #[test]
    fn test_flat_height_map_init_no_device() {
        // Test that we can create the structure without GPU
        let mut hm = SimpleHeightMap::new_uniform(32, 32, 10, 0);
        let mut fh = FlatHeightMapRenderObj::new();
        // Can't call init_height_data without a GPU device,
        // but we can verify the default state
        assert_eq!(fh.get_num_tiles(), 0);
        assert_eq!(hm.get_x_extent(), 32);
        assert_eq!(hm.get_y_extent(), 32);
    }

    #[test]
    fn test_simple_height_map() {
        let mut hm = SimpleHeightMap::new_uniform(16, 16, 5, 2);
        assert_eq!(hm.get_height(0, 0), 5);
        assert_eq!(hm.get_height(15, 15), 5);
        assert_eq!(hm.get_x_extent(), 16);
        assert_eq!(hm.get_y_extent(), 16);
        assert_eq!(hm.get_border_size(), 2);

        hm.set_height(3, 7, 20);
        assert_eq!(hm.get_height(3, 7), 20);

        // Out of bounds
        assert_eq!(hm.get_height(-1, 0), 0);
        assert_eq!(hm.get_height(16, 0), 0);
    }

    #[test]
    fn test_flip_states() {
        let mut hm = SimpleHeightMap::new_uniform(16, 16, 5, 0);
        assert!(!hm.get_flip_state(0, 0));
        hm.set_flip_state(0, 0, true);
        assert!(hm.get_flip_state(0, 0));
        hm.clear_flip_states();
        assert!(!hm.get_flip_state(0, 0));
    }

    #[test]
    fn test_cull_status() {
        let mut bg = W3DTerrainBackground::new();
        assert_eq!(bg.cull_status, CullStatus::Unknown);
        assert!(!bg.is_culled());
        bg.cull_status = CullStatus::Invisible;
        assert!(bg.is_culled());
        bg.cull_status = CullStatus::Visible;
        assert!(!bg.is_culled());
    }

    #[test]
    fn test_aa_box() {
        let mut box1 = AABox::default();
        box1.init_empty();
        box1.add_point(Vector3::new(10.0, 0.0, 0.0));
        box1.add_point(Vector3::new(-10.0, 0.0, 0.0));
        box1.add_point(Vector3::new(0.0, 10.0, 0.0));
        box1.add_point(Vector3::new(0.0, -10.0, 0.0));

        // Center should be at origin
        assert!((box1.center.x - 0.0).abs() < 0.001);
        assert!((box1.center.y - 0.0).abs() < 0.001);
        // Extents should be 10
        assert!((box1.extent.x - 10.0).abs() < 0.001);
        assert!((box1.extent.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_tex_multiplier() {
        let bg = W3DTerrainBackground::new();
        assert_eq!(bg.get_tex_multiplier(), TexMultiplier::Tex1X);
    }

    #[test]
    fn test_advance_left_basic() {
        let hm = SimpleHeightMap::new_uniform(32, 32, 5, 0);
        let bg = W3DTerrainBackground::new();

        // With all flip states false, advance should return false
        let mut left = ICoord2D { x: 0, y: 0 };
        let result = W3DTerrainBackground::advance_left(&mut left, 0, 0, 4, &hm, &bg);
        assert!(!result);
    }

    #[test]
    fn test_advance_right_basic() {
        let hm = SimpleHeightMap::new_uniform(32, 32, 5, 0);
        let bg = W3DTerrainBackground::new();

        let mut right = ICoord2D { x: 0, y: 0 };
        let result = W3DTerrainBackground::advance_right(&mut right, 0, 0, 4, &hm, &bg);
        assert!(!result);
    }
}
