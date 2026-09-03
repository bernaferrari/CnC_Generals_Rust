//! CPU-side parity port for C++ `W3DDevice/GameClient/W3DTreeBuffer.cpp`.

use std::collections::HashMap;

use glam::{Mat4, Vec2, Vec3};

pub const MAX_TREE_VERTEX: usize = 30_000;
pub const MAX_TREE_INDEX: usize = 60_000;
pub const MAX_TREES: usize = 4000;
pub const MAX_TYPES: usize = 64;
pub const MAX_TILES: usize = 512;
pub const NUM_SWAY_ENTRIES: usize = 100;
pub const MAX_SWAY_TYPES: usize = 10;
pub const MAX_BUFFERS: usize = 1;
/// C++ `TILE_PIXEL_EXTENT` from `TileData.h`.
pub const TILE_PIXEL_EXTENT: i32 = 64;
/// C++ `TILE_BYTES_PER_PIXEL`.
pub const TILE_BYTES_PER_PIXEL: usize = 4;
/// C++ `DATA_LEN_BYTES`.
pub const TREE_TILE_DATA_LEN: usize =
    TILE_PIXEL_EXTENT as usize * TILE_PIXEL_EXTENT as usize * TILE_BYTES_PER_PIXEL;
/// C++ `W3DTreeBuffer::updateTexture` `MAX_TEX_WIDTH`.
pub const MAX_TEX_WIDTH: i32 = 2048;
pub const SORT_ITERATIONS_PER_FRAME: usize = 10;
pub const PARTITION_WIDTH_HEIGHT: usize = 100;
pub const END_OF_PARTITION: i16 = -1;
pub const DELETED_TREE_TYPE: i32 = -2;
pub const TREE_RADIUS_APPROX: f32 = 7.0;
pub const CONSTRUCTION_TREE_COLLISION_RADIUS: f32 = 2.0 * TREE_RADIUS_APPROX;
pub const W3D_TOPPLE_OPTIONS_NONE: u32 = 0x0000_0000;
pub const W3D_TOPPLE_OPTIONS_NO_BOUNCE: u32 = 0x0000_0001;
pub const W3D_TOPPLE_OPTIONS_NO_FX: u32 = 0x0000_0002;
pub const ANGULAR_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - std::f32::consts::PI / 64.0;

/// C++ `W3DToppleState`, including the typoed `TOPPPLE_SHROUDED` slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum W3DToppleState {
    Upright = 0,
    Falling = 1,
    Fogged = 2,
    Shrouded = 3,
    Down = 4,
}

/// Minimal C++ `Region2D` equivalent used by the tree partition grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeRegion2D {
    pub lo: Vec2,
    pub hi: Vec2,
}

impl Default for TreeRegion2D {
    fn default() -> Self {
        Self {
            lo: Vec2::ZERO,
            hi: Vec2::ONE,
        }
    }
}

impl TreeRegion2D {
    pub fn new(lo: Vec2, hi: Vec2) -> Self {
        Self { lo, hi }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Default for TreeSphere {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 1.0,
        }
    }
}

/// C++ `W3DTreeDrawModuleData` subset consumed by `W3DTreeBuffer`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeModuleData {
    pub model_name: String,
    pub texture_name: String,
    pub topple_fx: Option<String>,
    pub bounce_fx: Option<String>,
    pub frames_to_move_outward: u32,
    pub frames_to_move_inward: u32,
    pub max_outward_movement: f32,
    pub darkening: f32,
    pub initial_velocity_percent: f32,
    pub initial_accel_percent: f32,
    pub bounce_velocity_percent: f32,
    pub minimum_topple_speed: f32,
    pub kill_when_toppled: bool,
    pub do_topple: bool,
    pub sink_frames: u32,
    pub sink_distance: f32,
    pub do_shadow: bool,
}

impl Default for TreeModuleData {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            texture_name: String::new(),
            topple_fx: None,
            bounce_fx: None,
            frames_to_move_outward: 1,
            frames_to_move_inward: 1,
            max_outward_movement: 1.0,
            darkening: 0.0,
            initial_velocity_percent: 0.2,
            initial_accel_percent: 0.01,
            bounce_velocity_percent: 0.3,
            minimum_topple_speed: 0.5,
            kill_when_toppled: true,
            do_topple: false,
            sink_frames: 10 * 30,
            sink_distance: 20.0,
            do_shadow: false,
        }
    }
}

/// C++ W3D mesh snapshot consumed by `loadTreesInVertexAndIndexBuffers`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeTypeMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Option<Vec<u32>>,
    pub polygons: Vec<[u16; 3]>,
    pub emissive: [f32; 3],
}

/// C++ `VertexFormatXYZNDUV1` tree vertex written into the DX/wgpu VB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeVertexXyznduv1 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
}

/// wgpu upload vertex: Y-up position + unpacked doLighting RGB + packed BGRA.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeGpuVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub tex_coords: [f32; 2],
    pub diffuse: u32,
}

// SAFETY: `#[repr(C)]` f32 arrays plus packed u32 diffuse; no padding holes,
// SAFETY: contents are opaque bytes once uploaded to the vertex buffer.
unsafe impl bytemuck::Pod for TreeGpuVertex {}
// SAFETY: 0.0 / 0u32 are valid field values with no niche.
unsafe impl bytemuck::Zeroable for TreeGpuVertex {}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeTypeInfo {
    pub data: TreeModuleData,
    pub bounds: TreeSphere,
    pub texture_origin: (i32, i32),
    pub num_tiles: i32,
    pub first_tile: i32,
    pub tile_width: i32,
    pub half_tile: bool,
    pub offset: Vec3,
    pub shadow_size: f32,
    pub do_shadow: bool,
    pub mesh: Option<TreeTypeMesh>,
}

impl TreeTypeInfo {
    fn from_module(data: TreeModuleData, bounds: TreeSphere) -> Self {
        Self {
            shadow_size: bounds.radius * 2.0,
            do_shadow: data.do_shadow,
            data,
            bounds,
            texture_origin: (0, 0),
            num_tiles: 0,
            first_tile: 0,
            tile_width: 0,
            half_tile: false,
            offset: Vec3::ZERO,
            mesh: None,
        }
    }
}

/// C++ `TTree`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub location: Vec3,
    pub scale: f32,
    pub sin: f32,
    pub cos: f32,
    pub tree_type: i32,
    pub visible: bool,
    pub bounds: TreeSphere,
    pub sort_key: f32,
    pub drawable_id: u32,
    pub push_aside: f32,
    pub push_aside_delta: f32,
    pub push_aside_sin: f32,
    pub push_aside_cos: f32,
    pub push_aside_source: u32,
    pub last_frame_updated: u32,
    pub next_in_partition: i16,
    pub sway_type: i32,
    pub first_index: i32,
    pub buffer_ndx: i32,
    pub angular_velocity: f32,
    pub angular_acceleration: f32,
    pub topple_direction: Vec3,
    pub topple_state: W3DToppleState,
    pub angular_accumulation: f32,
    pub options: u32,
    pub matrix: Mat4,
    pub sink_frames_left: u32,
}

impl Default for TreeEntry {
    fn default() -> Self {
        Self {
            location: Vec3::ZERO,
            scale: 1.0,
            sin: 0.0,
            cos: 1.0,
            tree_type: DELETED_TREE_TYPE,
            visible: false,
            bounds: TreeSphere::default(),
            sort_key: 0.0,
            drawable_id: 0,
            push_aside: 0.0,
            push_aside_delta: 0.0,
            push_aside_sin: 1.0,
            push_aside_cos: 1.0,
            push_aside_source: u32::MAX,
            last_frame_updated: 0,
            next_in_partition: END_OF_PARTITION,
            sway_type: 0,
            first_index: 0,
            buffer_ndx: -1,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Vec3::ZERO,
            topple_state: W3DToppleState::Upright,
            angular_accumulation: 0.0,
            options: W3D_TOPPLE_OPTIONS_NONE,
            matrix: Mat4::IDENTITY,
            sink_frames_left: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeShroudStatus {
    Clear,
    Fogged,
    Shrouded,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreezeInfo {
    pub breeze_version: i32,
    pub lean: f32,
    pub intensity: f32,
    pub direction_vec: Vec2,
    pub randomness: f32,
    pub breeze_period: i32,
}

impl Default for BreezeInfo {
    fn default() -> Self {
        Self {
            breeze_version: 0,
            lean: 0.0,
            intensity: 0.0,
            direction_vec: Vec2::X,
            randomness: 0.0,
            breeze_period: NUM_SWAY_ENTRIES as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeGeometryType {
    Cylinder,
    Box,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeCollisionUnit {
    pub object_id: u32,
    pub position: Vec3,
    pub direction_2d: Vec2,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub geometry_type: TreeGeometryType,
    pub crusher_level: i32,
    pub immobile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeConstructionGeometry {
    pub position: Vec3,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub geometry_type: TreeGeometryType,
    pub angle: f32,
}

impl TreeConstructionGeometry {
    fn collides_with_tree_cylinder(&self, tree_position: Vec3) -> bool {
        let dx = tree_position.x - self.position.x;
        let dy = tree_position.y - self.position.y;
        if self.geometry_type == TreeGeometryType::Box {
            let (sin, cos) = self.angle.sin_cos();
            let local_x = dx * cos + dy * sin;
            let local_y = -dx * sin + dy * cos;
            let half_y = self.minor_radius;
            let closest_x = local_x.clamp(-self.major_radius, self.major_radius);
            let closest_y = local_y.clamp(-half_y, half_y);
            let delta_x = local_x - closest_x;
            let delta_y = local_y - closest_y;
            delta_x * delta_x + delta_y * delta_y
                <= CONSTRUCTION_TREE_COLLISION_RADIUS * CONSTRUCTION_TREE_COLLISION_RADIUS
        } else {
            let radius = self.major_radius + CONSTRUCTION_TREE_COLLISION_RADIUS;
            dx * dx + dy * dy <= radius * radius
        }
    }
}

/// Snapshot record order used by C++ `W3DTreeBuffer::xfer`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSaveRecord {
    pub model_name: String,
    pub model_texture: String,
    pub location: Vec3,
    pub scale: f32,
    pub sin: f32,
    pub cos: f32,
    pub drawable_id: u32,
    pub angular_velocity: f32,
    pub angular_acceleration: f32,
    pub topple_direction: Vec3,
    pub topple_state: W3DToppleState,
    pub angular_accumulation: f32,
    pub options: u32,
    pub matrix: Mat4,
    pub sink_frames_left: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeFxKind {
    Topple,
    Bounce,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeFxEvent {
    pub kind: TreeFxKind,
    pub fx_name: String,
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct W3DTreeBuffer {
    area_partition: Vec<i16>,
    bounds: TreeRegion2D,
    trees: Vec<TreeEntry>,
    tree_types: Vec<TreeTypeInfo>,
    anything_changed: bool,
    any_push_changed: bool,
    update_all_keys: bool,
    initialized: bool,
    is_terrain_pass: bool,
    need_to_update_texture: bool,
    num_tiles: i32,
    camera_look_at_vector: Vec3,
    sway_offsets: [Vec3; NUM_SWAY_ENTRIES],
    cur_sway_version: i32,
    cur_sway_offset: [f32; MAX_SWAY_TYPES],
    cur_sway_step: [f32; MAX_SWAY_TYPES],
    cur_sway_factor: [f32; MAX_SWAY_TYPES],
    cur_num_tree_vertices: [i32; MAX_BUFFERS],
    cur_num_tree_indices: [i32; MAX_BUFFERS],
    texture_width: i32,
    texture_height: i32,
    cpu_vertices: Vec<TreeVertexXyznduv1>,
    cpu_indices: Vec<u16>,
    tile_locations: Vec<(i32, i32)>,
    source_tiles: Vec<Vec<u8>>,
    atlas_mips: Vec<Vec<u8>>,
    atlas_lod: i32,
    last_tile_images: Vec<TreeTileImageSpec>,
    pending_fx_events: Vec<TreeFxEvent>,
}

impl Default for W3DTreeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DTreeBuffer {
    pub fn new() -> Self {
        let mut buffer = Self {
            area_partition: vec![END_OF_PARTITION; PARTITION_WIDTH_HEIGHT * PARTITION_WIDTH_HEIGHT],
            bounds: TreeRegion2D::default(),
            trees: Vec::new(),
            tree_types: Vec::new(),
            anything_changed: false,
            any_push_changed: false,
            update_all_keys: false,
            initialized: false,
            is_terrain_pass: false,
            need_to_update_texture: false,
            num_tiles: 0,
            camera_look_at_vector: Vec3::ZERO,
            sway_offsets: [Vec3::ZERO; NUM_SWAY_ENTRIES],
            cur_sway_version: -1,
            cur_sway_offset: [0.0; MAX_SWAY_TYPES],
            cur_sway_step: [0.0; MAX_SWAY_TYPES],
            cur_sway_factor: [0.0; MAX_SWAY_TYPES],
            cur_num_tree_vertices: [0; MAX_BUFFERS],
            cur_num_tree_indices: [0; MAX_BUFFERS],
            texture_width: TILE_PIXEL_EXTENT,
            texture_height: TILE_PIXEL_EXTENT,
            cpu_vertices: Vec::new(),
            cpu_indices: Vec::new(),
            tile_locations: Vec::new(),
            source_tiles: Vec::new(),
            atlas_mips: Vec::new(),
            atlas_lod: 0,
            last_tile_images: Vec::new(),
            pending_fx_events: Vec::new(),
        };
        buffer.clear_all_trees();
        buffer.initialized = true;
        buffer.cur_sway_version = -1;
        buffer
    }

    pub fn cur_sway_version(&self) -> i32 {
        self.cur_sway_version
    }

    pub fn trees(&self) -> &[TreeEntry] {
        &self.trees
    }

    pub fn tree_mut(&mut self, index: usize) -> Option<&mut TreeEntry> {
        self.trees.get_mut(index)
    }

    pub fn tree_types(&self) -> &[TreeTypeInfo] {
        &self.tree_types
    }

    pub fn area_partition(&self) -> &[i16] {
        &self.area_partition
    }

    pub fn bounds(&self) -> TreeRegion2D {
        self.bounds
    }

    pub fn anything_changed(&self) -> bool {
        self.anything_changed
    }

    /// Force `loadTreesInVertexAndIndexBuffers` to run. Used when the GPU
    /// mesh cache is empty after a prior empty fill consumed `anything_changed`.
    pub fn force_vertex_rebuild(&mut self) {
        self.anything_changed = true;
    }

    pub fn any_push_changed(&self) -> bool {
        self.any_push_changed
    }

    pub fn take_any_push_changed(&mut self) -> bool {
        let changed = self.any_push_changed;
        self.any_push_changed = false;
        changed
    }

    pub fn update_all_keys(&self) -> bool {
        self.update_all_keys
    }

    pub fn camera_look_at_vector(&self) -> Vec3 {
        self.camera_look_at_vector
    }

    pub fn need_to_update_texture(&self) -> bool {
        self.need_to_update_texture
    }

    pub fn pending_fx_events(&self) -> &[TreeFxEvent] {
        &self.pending_fx_events
    }

    pub fn take_pending_fx_events(&mut self) -> Vec<TreeFxEvent> {
        std::mem::take(&mut self.pending_fx_events)
    }

    pub fn set_texture_atlas_size(&mut self, width: i32, height: i32) {
        self.texture_width = width.max(1);
        self.texture_height = height.max(1);
        self.anything_changed = true;
    }

    pub fn texture_size(&self) -> (i32, i32) {
        (self.texture_width, self.texture_height)
    }

    pub fn num_tiles(&self) -> i32 {
        self.num_tiles
    }

    pub fn tile_location_in_texture(&self, ndx: usize) -> Option<(i32, i32)> {
        self.tile_locations.get(ndx).copied()
    }

    /// C++ `W3DTreeBuffer::updateTexture` (countTiles + availableGrid pack).
    ///
    /// `images` stand in for TGA open+header; missing names match C++ "file not found".
    pub fn update_texture(&mut self, images: &[TreeTileImageSpec]) {
        self.last_tile_images = images.to_vec();
        let previous_tiles = std::mem::take(&mut self.source_tiles);
        self.tile_locations.clear();
        self.atlas_mips.clear();
        self.num_tiles = 0;

        for i in 0..self.tree_types.len() {
            self.tree_types[i].num_tiles = 0;
            let texture_name = self.tree_types[i].data.texture_name.clone();
            let Some(spec) = images
                .iter()
                .find(|image| image.texture_name.eq_ignore_ascii_case(&texture_name))
            else {
                self.tree_types[i].first_tile = 0;
                self.tree_types[i].tile_width = 0;
                self.tree_types[i].num_tiles = 0;
                self.tree_types[i].half_tile = false;
                continue;
            };

            let (mut num_tiles, half_tile) = count_tree_tiles(spec.header);
            let width = square_width_from_tile_count(num_tiles);
            if width > 0 {
                num_tiles = width * width;
            }

            let mut tex_found = false;
            for j in 0..i {
                if self.tree_types[j]
                    .data
                    .texture_name
                    .eq_ignore_ascii_case(&texture_name)
                {
                    self.tree_types[i].first_tile = 0;
                    self.tree_types[i].tile_width = width;
                    self.tree_types[i].num_tiles = 0;
                    self.tree_types[i].half_tile = half_tile;
                    tex_found = true;
                    break;
                }
            }
            if tex_found {
                continue;
            }

            if self.num_tiles + num_tiles <= MAX_TILES as i32 {
                self.tree_types[i].first_tile = self.num_tiles;
                self.tree_types[i].tile_width = width;
                self.tree_types[i].num_tiles = num_tiles;
                self.tree_types[i].half_tile = half_tile;
                self.num_tiles += num_tiles;
            } else {
                self.tree_types[i].first_tile = 0;
                self.tree_types[i].tile_width = 0;
                self.tree_types[i].num_tiles = 0;
            }
        }

        let (tex_w, tex_h, overflow) = tree_atlas_pixel_size(self.num_tiles);
        self.texture_width = tex_w;
        self.texture_height = tex_h;
        if overflow {
            self.source_tiles = previous_tiles;
            self.need_to_update_texture = false;
            self.anything_changed = true;
            return;
        }

        self.tile_locations = vec![(-1, -1); self.num_tiles.max(0) as usize];
        self.source_tiles = vec![Vec::new(); self.num_tiles.max(0) as usize];
        for (ndx, tile) in previous_tiles.into_iter().enumerate() {
            if ndx < self.source_tiles.len() && tile.len() == TREE_TILE_DATA_LEN {
                self.source_tiles[ndx] = tile;
            }
        }
        pack_tree_atlas(
            &mut self.tree_types,
            &mut self.tile_locations,
            self.num_tiles,
        );
        self.ensure_source_tile_pixels();
        self.need_to_update_texture = false;
        self.anything_changed = true;
    }

    pub fn set_source_tile_bgra(&mut self, ndx: usize, bgra: &[u8]) -> bool {
        if bgra.len() != TREE_TILE_DATA_LEN {
            return false;
        }
        if ndx >= self.source_tiles.len() {
            self.source_tiles.resize(ndx + 1, Vec::new());
        }
        self.source_tiles[ndx] = bgra.to_vec();
        true
    }

    pub fn source_tile_bgra(&self, ndx: usize) -> Option<&[u8]> {
        self.source_tiles.get(ndx).map(Vec::as_slice)
    }

    /// C++ `W3DTreeTextureClass::update` + `SetLOD(textureReductionFactor)`.
    /// Returns atlas height (C++ `surface_desc.Height`).
    pub fn update_tree_texture_class(&mut self, lod: i32) -> i32 {
        self.ensure_source_tile_pixels();
        let width = self.texture_width.max(1);
        let height = self.texture_height.max(1);
        let mut level0 = vec![0u8; (width as usize) * (height as usize) * TILE_BYTES_PER_PIXEL];
        for (ndx, loc) in self.tile_locations.iter().enumerate() {
            if loc.0 < 0 {
                continue;
            }
            let Some(tile) = self.source_tiles.get(ndx) else {
                continue;
            };
            if tile.len() != TREE_TILE_DATA_LEN {
                continue;
            }
            blit_tree_tile_into_atlas(&mut level0, width, tile, loc.0, loc.1);
        }
        self.atlas_mips = generate_box_mip_chain(&level0, width, height);
        self.atlas_lod = lod.clamp(0, 4);
        height
    }

    /// C++ `W3DTreeBuffer::setTextureLOD`.
    pub fn set_texture_lod(&mut self, lod: i32) {
        self.atlas_lod = lod.clamp(0, 4);
    }

    /// C++ `drawTrees` texture refresh: dirty `updateTexture` + blit/mip/SetLOD.
    ///
    /// Called from `TerrainVisual::update_tree_meshes`. Rebuilds the atlas when
    /// the pack is dirty or tiles exist with no CPU mips yet.
    pub fn sync_tree_atlas_for_draw(&mut self, lod: i32) -> i32 {
        if self.need_to_update_texture {
            let images = self.images_for_texture_update();
            self.update_texture(&images);
        }
        let tiles_exist = self.num_tiles > 0 && !self.tile_locations.is_empty();
        if tiles_exist && self.atlas_mips.is_empty() {
            self.update_tree_texture_class(lod)
        } else {
            // C++ `drawTrees` still `SetLOD(textureReductionFactor)` every frame.
            self.set_texture_lod(lod);
            self.texture_height
        }
    }

    pub fn atlas_mips(&self) -> &[Vec<u8>] {
        &self.atlas_mips
    }

    pub fn atlas_lod(&self) -> i32 {
        self.atlas_lod
    }

    /// C++ `IDirect3DTexture8::SetLOD` — first mip used on GPU upload.
    pub fn atlas_upload_mip_index(&self) -> usize {
        let lod = self.atlas_lod.max(0) as usize;
        lod.min(self.atlas_mips.len().saturating_sub(1))
    }

    pub fn atlas_upload_levels(&self) -> &[Vec<u8>] {
        let start = self.atlas_upload_mip_index();
        &self.atlas_mips[start..]
    }

    pub fn set_tree_type_mesh(&mut self, type_index: usize, mesh: TreeTypeMesh) -> bool {
        let Some(tree_type) = self.tree_types.get_mut(type_index) else {
            return false;
        };
        tree_type.mesh = Some(mesh);
        self.anything_changed = true;
        true
    }

    pub fn tree_type_mut(&mut self, type_index: usize) -> Option<&mut TreeTypeInfo> {
        self.tree_types.get_mut(type_index)
    }

    pub fn cpu_vertices(&self) -> &[TreeVertexXyznduv1] {
        &self.cpu_vertices
    }

    pub fn cpu_indices(&self) -> &[u16] {
        &self.cpu_indices
    }

    pub fn cur_num_tree_vertices(&self) -> i32 {
        self.cur_num_tree_vertices[0]
    }

    pub fn cur_num_tree_indices(&self) -> i32 {
        self.cur_num_tree_indices[0]
    }

    pub fn set_bounds(&mut self, bounds: TreeRegion2D) {
        self.bounds = bounds;
    }

    pub fn set_is_terrain(&mut self) {
        self.is_terrain_pass = true;
    }

    pub fn need_to_draw(&self) -> bool {
        self.is_terrain_pass
    }

    pub fn do_full_update(&mut self) {
        self.update_all_keys = true;
    }

    pub fn cull_trees(
        &mut self,
        camera_look_at_vector: Vec3,
        mut is_visible: impl FnMut(&TreeSphere) -> bool,
    ) {
        self.camera_look_at_vector = camera_look_at_vector;
        for tree in &mut self.trees {
            let mut update_key = false;
            let visible = is_visible(&tree.bounds);
            if visible != tree.visible {
                tree.visible = visible;
                self.anything_changed = true;
                if visible {
                    update_key = true;
                }
            }
            if update_key || (visible && self.update_all_keys) {
                tree.sort_key = tree.location.dot(self.camera_look_at_vector);
            }
        }
        self.update_all_keys = false;
    }

    pub fn cull_trees_from_camera_transform(
        &mut self,
        camera_transform: Mat4,
        is_visible: impl FnMut(&TreeSphere) -> bool,
    ) {
        self.cull_trees(-camera_transform.z_axis.truncate(), is_visible);
    }

    pub fn clear_all_trees(&mut self) {
        self.trees.clear();
        self.bounds = TreeRegion2D::default();
        self.cur_num_tree_indices[0] = 0;
        self.cpu_vertices.clear();
        self.cpu_indices.clear();
        self.anything_changed = true;
        self.area_partition.fill(END_OF_PARTITION);
        self.tree_types.clear();
        self.num_tiles = 0;
        self.tile_locations.clear();
        self.source_tiles.clear();
        self.atlas_mips.clear();
        self.atlas_lod = 0;
        self.last_tile_images.clear();
        self.need_to_update_texture = false;
        self.pending_fx_events.clear();
    }

    pub fn add_tree_type(&mut self, data: TreeModuleData, bounds: TreeSphere) -> Option<usize> {
        if self.tree_types.len() >= MAX_TYPES {
            return Some(0);
        }
        self.need_to_update_texture = true;
        self.tree_types
            .push(TreeTypeInfo::from_module(data, bounds));
        Some(self.tree_types.len() - 1)
    }

    pub fn add_tree(
        &mut self,
        drawable_id: u32,
        location: Vec3,
        scale: f32,
        angle: f32,
        random_scale_amount: f32,
        data: TreeModuleData,
        base_bounds: TreeSphere,
    ) -> Option<usize> {
        let mut rng = DeterministicTreeRandom;
        self.add_tree_randomized(
            drawable_id,
            location,
            scale,
            angle,
            random_scale_amount,
            data,
            base_bounds,
            &mut rng,
        )
    }

    pub fn add_tree_randomized(
        &mut self,
        drawable_id: u32,
        location: Vec3,
        scale: f32,
        angle: f32,
        random_scale_amount: f32,
        data: TreeModuleData,
        base_bounds: TreeSphere,
        rng: &mut impl TreeRandom,
    ) -> Option<usize> {
        if self.trees.len() >= MAX_TREES || !self.initialized {
            return None;
        }

        let tree_type = self
            .tree_types
            .iter()
            .position(|existing| {
                existing
                    .data
                    .model_name
                    .eq_ignore_ascii_case(&data.model_name)
                    && existing
                        .data
                        .texture_name
                        .eq_ignore_ascii_case(&data.texture_name)
            })
            .or_else(|| self.add_tree_type(data.clone(), base_bounds))?;

        let random_scale = rng.real_range(1.0 - random_scale_amount, 1.0 + random_scale_amount);
        let final_scale = if random_scale_amount > 0.0 {
            scale * random_scale
        } else {
            scale
        };
        let mut entry = TreeEntry {
            location,
            scale: final_scale,
            sin: angle.sin(),
            cos: angle.cos(),
            tree_type: tree_type as i32,
            visible: false,
            drawable_id,
            first_index: 0,
            buffer_ndx: -1,
            sway_type: rng.int_range(0, MAX_SWAY_TYPES as i32 - 1),
            push_aside: 0.0,
            last_frame_updated: 0,
            push_aside_source: u32::MAX,
            push_aside_delta: 0.0,
            push_aside_cos: 1.0,
            push_aside_sin: 1.0,
            topple_state: W3DToppleState::Upright,
            ..TreeEntry::default()
        };
        entry.bounds = self.scaled_bounds(tree_type, location, entry.scale);

        if data.frames_to_move_outward > 2 || data.do_topple {
            let bucket = self.get_partition_bucket(location) as usize;
            entry.next_in_partition = self.area_partition[bucket];
            self.area_partition[bucket] = self.trees.len() as i16;
        }

        self.trees.push(entry);
        Some(self.trees.len() - 1)
    }

    pub fn update_tree_position(&mut self, drawable_id: u32, location: Vec3, angle: f32) -> bool {
        for i in 0..self.trees.len() {
            if self.trees[i].drawable_id == drawable_id {
                self.trees[i].location = location;
                self.trees[i].sin = angle.sin();
                self.trees[i].cos = angle.cos();
                let tree_type = self.trees[i].tree_type as usize;
                self.trees[i].bounds = self.scaled_bounds(tree_type, location, self.trees[i].scale);
                self.anything_changed = true;
                return true;
            }
        }
        false
    }

    pub fn remove_tree(&mut self, drawable_id: u32) {
        for tree in &mut self.trees {
            if tree.drawable_id == drawable_id {
                tree.location = Vec3::ZERO;
                tree.tree_type = DELETED_TREE_TYPE;
                tree.bounds = TreeSphere::default();
                self.anything_changed = true;
            }
        }
    }

    pub fn remove_trees_for_construction(&mut self, geom: TreeConstructionGeometry) {
        for tree in &mut self.trees {
            if tree.tree_type < 0 {
                continue;
            }
            if geom.collides_with_tree_cylinder(tree.location) {
                tree.tree_type = DELETED_TREE_TYPE;
                self.anything_changed = true;
            }
        }
    }

    pub fn get_partition_bucket(&self, pos: Vec3) -> i32 {
        let mut x = pos.x;
        let mut y = pos.y;
        if x < self.bounds.lo.x {
            x = self.bounds.lo.x;
        }
        if y < self.bounds.lo.y {
            y = self.bounds.lo.y;
        }
        if x > self.bounds.hi.x {
            x = self.bounds.hi.x;
        }
        if y > self.bounds.hi.y {
            y = self.bounds.hi.y;
        }
        let x_index = ((x / (self.bounds.hi.x - self.bounds.lo.x))
            * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
            .floor() as i32;
        let y_index = ((y / (self.bounds.hi.y - self.bounds.lo.y))
            * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
            .floor() as i32;
        y_index * PARTITION_WIDTH_HEIGHT as i32 + x_index
    }

    pub fn push_aside_tree(
        &mut self,
        drawable_id: u32,
        pusher_pos: Vec3,
        pusher_direction: Vec2,
        pusher_id: u32,
        frame: u32,
    ) {
        for tree in &mut self.trees {
            if tree.drawable_id != drawable_id {
                continue;
            }
            let last_frame = tree.last_frame_updated;
            tree.last_frame_updated = frame;
            if tree.push_aside_source == pusher_id && tree.last_frame_updated - last_frame < 3 {
                return;
            }
            if tree.push_aside != 0.0 {
                return;
            }
            tree.push_aside_source = pusher_id;
            let delta = tree.location - pusher_pos;
            if pusher_direction.x * delta.y - pusher_direction.y * delta.x > 0.0 {
                tree.push_aside_cos = -pusher_direction.y;
                tree.push_aside_sin = pusher_direction.x;
            } else {
                tree.push_aside_cos = pusher_direction.y;
                tree.push_aside_sin = -pusher_direction.x;
            }
            self.any_push_changed = true;
            let tree_type = tree.tree_type as usize;
            let outward = self.tree_types[tree_type]
                .data
                .frames_to_move_outward
                .max(1);
            tree.push_aside_delta = 1.0 / outward as f32;
        }
    }

    pub fn unit_moved(&mut self, unit: TreeCollisionUnit, frame: u32) {
        if unit.immobile {
            return;
        }
        let mut radius = unit.major_radius;
        if unit.geometry_type == TreeGeometryType::Box && radius > unit.minor_radius {
            radius = unit.minor_radius;
        }
        radius += TREE_RADIUS_APPROX;

        let (x_min, y_min) = self.partition_min_indices(unit.position, radius);
        let (x_max, y_max) = self.partition_max_indices(unit.position, radius);
        // Geometry clamp can still yield out-of-grid indices when the unit
        // sits outside the partition bounds (or bounds collapse); guard the
        // bucket index instead of panicking on a negative/out-of-range slot.
        for x in x_min..x_max {
            for y in y_min..y_max {
                let bucket = x + PARTITION_WIDTH_HEIGHT as i32 * y;
                if bucket < 0 {
                    continue;
                }
                let bucket = bucket as usize;
                if bucket >= self.area_partition.len() {
                    continue;
                }
                let mut tree_ndx = self.area_partition[bucket];
                while tree_ndx != END_OF_PARTITION {
                    let index = tree_ndx as usize;
                    if index >= self.trees.len() {
                        break;
                    }
                    tree_ndx = self.trees[index].next_in_partition;
                    if self.trees[index].tree_type < 0 {
                        continue;
                    }
                    let delta = self.trees[index].location - unit.position;
                    if radius * radius <= delta.length_squared() {
                        continue;
                    }
                    let tree_type = self.trees[index].tree_type as usize;
                    if unit.crusher_level > 1 && self.tree_types[tree_type].data.do_topple {
                        let topple_vector = Vec3::new(
                            self.trees[index].location.x - unit.position.x,
                            self.trees[index].location.y - unit.position.y,
                            0.0,
                        );
                        self.apply_toppling_force_by_index(
                            index,
                            topple_vector,
                            0.0,
                            W3D_TOPPLE_OPTIONS_NONE,
                        );
                    } else if self.tree_types[tree_type].data.frames_to_move_outward > 1 {
                        let drawable_id = self.trees[index].drawable_id;
                        self.push_aside_tree(
                            drawable_id,
                            unit.position,
                            unit.direction_2d,
                            unit.object_id,
                            frame,
                        );
                    }
                }
            }
        }
    }

    pub fn apply_toppling_force(
        &mut self,
        drawable_id: u32,
        topple_direction: Vec3,
        topple_speed: f32,
        options: u32,
    ) -> bool {
        let Some(index) = self
            .trees
            .iter()
            .position(|tree| tree.drawable_id == drawable_id)
        else {
            return false;
        };
        self.apply_toppling_force_by_index(index, topple_direction, topple_speed, options)
    }

    pub fn update_toppling_tree(&mut self, index: usize, shroud: TreeShroudStatus) {
        if index >= self.trees.len()
            || self.trees[index].topple_state == W3DToppleState::Upright
            || self.trees[index].topple_state == W3DToppleState::Down
        {
            return;
        }

        let data = self.tree_types[self.trees[index].tree_type as usize]
            .data
            .clone();
        if shroud == TreeShroudStatus::Fogged {
            self.trees[index].topple_state = W3DToppleState::Fogged;
            return;
        }
        if self.trees[index].topple_state == W3DToppleState::Fogged {
            self.trees[index].angular_velocity = 0.0;
            self.trees[index].topple_state = W3DToppleState::Down;
            pre_rotate_topple_matrix(&mut self.trees[index], ANGULAR_LIMIT);
            self.trees[index].angular_accumulation = ANGULAR_LIMIT;
            if data.kill_when_toppled {
                self.trees[index].sink_frames_left = 0;
            }
            return;
        }

        const VELOCITY_BOUNCE_LIMIT: f32 = 0.01;
        const VELOCITY_BOUNCE_SOUND_LIMIT: f32 = 0.03;
        let mut cur_vel_to_use = self.trees[index].angular_velocity;
        if self.trees[index].angular_accumulation + cur_vel_to_use > ANGULAR_LIMIT {
            cur_vel_to_use = ANGULAR_LIMIT - self.trees[index].angular_accumulation;
        }
        pre_rotate_topple_matrix(&mut self.trees[index], cur_vel_to_use);
        self.trees[index].angular_accumulation += cur_vel_to_use;
        if self.trees[index].angular_accumulation >= ANGULAR_LIMIT
            && self.trees[index].angular_velocity > 0.0
        {
            self.trees[index].angular_velocity *= -data.bounce_velocity_percent;
            if self.trees[index].options & W3D_TOPPLE_OPTIONS_NO_BOUNCE != 0
                || self.trees[index].angular_velocity.abs() < VELOCITY_BOUNCE_LIMIT
            {
                self.trees[index].angular_velocity = 0.0;
                self.trees[index].topple_state = W3DToppleState::Down;
                if data.kill_when_toppled {
                    self.trees[index].sink_frames_left = data.sink_frames;
                }
            } else if self.trees[index].angular_velocity.abs() >= VELOCITY_BOUNCE_SOUND_LIMIT
                && self.trees[index].options & W3D_TOPPLE_OPTIONS_NO_FX == 0
            {
                if let Some(fx_name) = data.bounce_fx {
                    let position = self.trees[index].matrix.transform_point3(Vec3::new(
                        0.0,
                        0.0,
                        3.0 * TREE_RADIUS_APPROX,
                    ));
                    self.pending_fx_events.push(TreeFxEvent {
                        kind: TreeFxKind::Bounce,
                        fx_name,
                        position,
                    });
                }
            }
        } else {
            self.trees[index].angular_velocity += self.trees[index].angular_acceleration;
        }
    }

    pub fn tick_cpu(&mut self, pause: bool, shroud: impl Fn(&TreeEntry) -> TreeShroudStatus) {
        self.is_terrain_pass = false;
        if pause {
            return;
        }
        for index in 0..self.trees.len() {
            let tree_type = self.trees[index].tree_type;
            if tree_type < 0 {
                continue;
            }
            match self.trees[index].topple_state {
                W3DToppleState::Falling | W3DToppleState::Fogged => {
                    let status = shroud(&self.trees[index]);
                    self.update_toppling_tree(index, status);
                }
                W3DToppleState::Down => {
                    let data = &self.tree_types[tree_type as usize].data;
                    if data.kill_when_toppled {
                        if self.trees[index].sink_frames_left == 0 {
                            self.trees[index].tree_type = DELETED_TREE_TYPE;
                            self.anything_changed = true;
                        }
                        self.trees[index].sink_frames_left =
                            self.trees[index].sink_frames_left.wrapping_sub(1);
                        self.trees[index].location.z -=
                            data.sink_distance / data.sink_frames.max(1) as f32;
                        set_matrix_translation(&mut self.trees[index]);
                    }
                }
                _ if self.trees[index].push_aside_delta != 0.0 => {
                    self.trees[index].push_aside += self.trees[index].push_aside_delta;
                    let data = &self.tree_types[tree_type as usize].data;
                    if self.trees[index].push_aside >= 1.0 {
                        self.trees[index].push_aside_delta =
                            -1.0 / data.frames_to_move_inward.max(1) as f32;
                    } else if self.trees[index].push_aside <= 0.0 {
                        self.trees[index].push_aside_delta = 0.0;
                        self.trees[index].push_aside = 0.0;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn update_sway(&mut self, info: BreezeInfo, rng: &mut impl TreeRandom) {
        for i in 0..NUM_SWAY_ENTRIES {
            let factor =
                (i as f32 * 2.0 * std::f32::consts::PI / (NUM_SWAY_ENTRIES as f32 + 1.0)).cos();
            let angle = info.lean + info.intensity * factor;
            let s = angle.sin();
            let c = angle.cos();
            self.sway_offsets[i] =
                Vec3::new(info.direction_vec.x * s, info.direction_vec.y * s, c - 1.0);
        }

        let delta = info.randomness * 0.5;
        for tree in &mut self.trees {
            tree.sway_type = 1 + rng.int_range(0, MAX_SWAY_TYPES as i32 - 1);
        }
        for i in 0..MAX_SWAY_TYPES {
            self.cur_sway_step[i] = NUM_SWAY_ENTRIES as f32 / info.breeze_period as f32;
            self.cur_sway_step[i] *= rng.real_range(1.0 - delta, 1.0 + delta);
            if self.cur_sway_step[i] < 0.0 {
                self.cur_sway_step[i] = 0.0;
            }
            self.cur_sway_offset[i] = 0.0;
            self.cur_sway_factor[i] = rng.real_range(1.0 - delta, 1.0 + delta);
        }
        self.cur_sway_version = info.breeze_version;
    }

    pub fn save_records(&self) -> Vec<TreeSaveRecord> {
        self.trees
            .iter()
            .map(|tree| {
                let (model_name, model_texture) = if tree.tree_type != DELETED_TREE_TYPE {
                    let tree_type = &self.tree_types[tree.tree_type as usize];
                    (
                        tree_type.data.model_name.clone(),
                        tree_type.data.texture_name.clone(),
                    )
                } else {
                    (String::new(), String::new())
                };
                TreeSaveRecord {
                    model_name,
                    model_texture,
                    location: tree.location,
                    scale: tree.scale,
                    sin: tree.sin,
                    cos: tree.cos,
                    drawable_id: tree.drawable_id,
                    angular_velocity: tree.angular_velocity,
                    angular_acceleration: tree.angular_acceleration,
                    topple_direction: tree.topple_direction,
                    topple_state: tree.topple_state,
                    angular_accumulation: tree.angular_accumulation,
                    options: tree.options,
                    matrix: tree.matrix,
                    sink_frames_left: tree.sink_frames_left,
                }
            })
            .collect()
    }

    pub fn load_records(&mut self, records: &[TreeSaveRecord]) {
        self.trees.clear();
        self.area_partition.fill(END_OF_PARTITION);
        self.cur_num_tree_vertices = [0; MAX_BUFFERS];
        self.cur_num_tree_indices = [0; MAX_BUFFERS];

        for record in records {
            let Some(tree_type) = self.tree_types.iter().position(|existing| {
                existing
                    .data
                    .model_name
                    .eq_ignore_ascii_case(&record.model_name)
                    && existing
                        .data
                        .texture_name
                        .eq_ignore_ascii_case(&record.model_texture)
            }) else {
                continue;
            };
            let data = self.tree_types[tree_type].data.clone();
            let base_bounds = self.tree_types[tree_type].bounds;
            let Some(index) = self.add_tree(
                record.drawable_id,
                record.location,
                record.scale,
                0.0,
                0.0,
                data,
                base_bounds,
            ) else {
                continue;
            };
            let tree = &mut self.trees[index];
            tree.angular_acceleration = record.angular_acceleration;
            tree.angular_velocity = record.angular_velocity;
            tree.topple_direction = record.topple_direction;
            tree.topple_state = record.topple_state;
            tree.options = record.options;
            tree.matrix = record.matrix;
            tree.sink_frames_left = record.sink_frames_left;
        }
        self.anything_changed = true;
    }

    /// C++ `W3DTreeBuffer::loadTreesInVertexAndIndexBuffers`.
    ///
    /// Fills the CPU VB/IB with `doLighting` on every vertex when `doVertexLighting`
    /// is true (live C++ path; POINT dynamic lights stay `#if 0`).
    pub fn load_trees_in_vertex_and_index_buffers(&mut self, object_lighting: &[TreeObjectLight]) {
        if !self.initialized {
            return;
        }
        if !self.anything_changed {
            return;
        }
        self.anything_changed = false;

        self.cpu_vertices.clear();
        self.cpu_indices.clear();
        self.cur_num_tree_vertices = [0; MAX_BUFFERS];
        self.cur_num_tree_indices = [0; MAX_BUFFERS];

        let tex_w = self.texture_width.max(1) as f32;
        let tex_h = self.texture_height.max(1) as f32;
        let mut cur_tree = 0usize;
        let num_trees = self.trees.len();

        for b_ndx in 0..MAX_BUFFERS {
            self.cur_num_tree_vertices[b_ndx] = 0;
            self.cur_num_tree_indices[b_ndx] = 0;
            if cur_tree >= num_trees {
                break;
            }

            while cur_tree < num_trees {
                let type_idx = self.trees[cur_tree].tree_type;
                if type_idx < 0 {
                    cur_tree += 1;
                    continue;
                }
                if !self.trees[cur_tree].visible {
                    cur_tree += 1;
                    continue;
                }
                let type_idx = type_idx as usize;
                let Some(tree_type) = self.tree_types.get(type_idx) else {
                    cur_tree += 1;
                    continue;
                };
                let stand_in;
                let mesh = if let Some(mesh) = tree_type.mesh.as_ref() {
                    mesh
                } else {
                    // Live types often have no WW3D mesh snapshot yet. A crossed
                    // card still consumes the atlas UVs so W3DTreeBuffer entries draw.
                    stand_in = stand_in_tree_type_mesh(&tree_type.bounds);
                    &stand_in
                };

                let scale = self.trees[cur_tree].scale;
                let loc = self.trees[cur_tree].location;
                let the_sin = self.trees[cur_tree].sin;
                let the_cos = self.trees[cur_tree].cos;
                let offset = tree_type.offset;
                let tile_width = tree_type.tile_width.max(1);
                let half_tile = tree_type.half_tile;
                let texture_origin = tree_type.texture_origin;
                let darkening = tree_type.data.darkening;
                let max_outward = tree_type.data.max_outward_movement;
                let emissive = mesh.emissive;
                let num_vertex = mesh.positions.len();
                let num_index = mesh.polygons.len();

                if self.cur_num_tree_vertices[b_ndx] as usize + num_vertex + 2 >= MAX_TREE_VERTEX {
                    break;
                }
                if self.cur_num_tree_indices[b_ndx] as usize + 3 * num_index + 6 >= MAX_TREE_INDEX {
                    break;
                }

                let mut do_vertex_lighting = true;
                let mut shared_diffuse = 0u32;
                if mesh.normals.is_none() {
                    do_vertex_lighting = false;
                    shared_diffuse =
                        do_lighting([0.0, 0.0, 1.0], object_lighting, emissive, 0xFFFF_FFFF, 1.0);
                }

                let start_vertex = self.cur_num_tree_vertices[b_ndx];
                self.trees[cur_tree].first_index = start_vertex;
                self.trees[cur_tree].buffer_ndx = b_ndx as i32;

                let mut u_scale = tile_width as f32 * TILE_PIXEL_EXTENT as f32 / tex_w;
                let mut v_scale = tile_width as f32 * TILE_PIXEL_EXTENT as f32 / tex_h;
                let u_offset = texture_origin.0 as f32 / tex_w;
                let mut v_offset = texture_origin.1 as f32 / tex_h;
                if half_tile {
                    u_scale *= 0.5;
                    v_scale *= 0.5;
                    v_offset += (TILE_PIXEL_EXTENT as f32 / 2.0) / tex_h;
                }

                let push_aside = self.trees[cur_tree].push_aside;
                let push_aside_sin = self.trees[cur_tree].push_aside_sin;
                let push_aside_cos = self.trees[cur_tree].push_aside_cos;
                let sway_type = self.trees[cur_tree].sway_type as f32;
                let topple_state = self.trees[cur_tree].topple_state;
                let topple_matrix = self.trees[cur_tree].matrix;

                for i in 0..num_vertex {
                    if self.cur_num_tree_vertices[b_ndx] as usize >= MAX_TREE_VERTEX {
                        break;
                    }
                    let uv = mesh.uvs.get(i).copied().unwrap_or([0.0, 0.0]);
                    let mut u = uv[0];
                    let mut v = uv[1];
                    if u > 1.0 {
                        u = 1.0;
                    }
                    if u < 0.0 {
                        u = 0.0;
                    }
                    if v > 1.0 {
                        v = 1.0;
                    }
                    if v < 0.0 {
                        v = 0.0;
                    }

                    let p = mesh.positions[i];
                    let x = p[0] + offset.x;
                    let y = p[1] + offset.y;
                    let mut v_loc = Vec3::new(
                        x * scale * the_cos - y * scale * the_sin,
                        y * scale * the_cos + x * scale * the_sin,
                        p[2] * scale + offset.z,
                    );

                    if topple_state != W3DToppleState::Upright {
                        v_loc = topple_matrix.transform_point3(v_loc);
                    } else {
                        if push_aside > 0.0 {
                            v_loc.x += p[2] * push_aside * push_aside_cos * max_outward;
                            v_loc.y += p[2] * push_aside * push_aside_sin * max_outward;
                        }
                        v_loc.x += loc.x;
                        v_loc.y += loc.y;
                        v_loc.z += loc.z;
                    }

                    let diffuse = if do_vertex_lighting {
                        let mut normal = [0.0, 0.0, 1.0];
                        if let Some(normals) = mesh.normals.as_ref() {
                            if let Some(n) = normals.get(i) {
                                normal = [
                                    n[0] * the_cos - n[1] * the_sin,
                                    n[1] * the_cos + n[0] * the_sin,
                                    n[2],
                                ];
                            }
                        }
                        let vertex_diffuse = mesh
                            .colors
                            .as_ref()
                            .and_then(|colors| colors.get(i).copied())
                            .unwrap_or(0xFFFF_FFFF);
                        do_lighting(normal, object_lighting, emissive, vertex_diffuse, 1.0)
                    } else {
                        shared_diffuse
                    };

                    self.cpu_vertices.push(TreeVertexXyznduv1 {
                        x: v_loc.x,
                        y: v_loc.y,
                        z: v_loc.z,
                        nx: sway_type,
                        ny: 1.0 - darkening * push_aside,
                        nz: loc.z,
                        diffuse,
                        u1: u * u_scale + u_offset,
                        v1: v * v_scale + v_offset,
                    });
                    self.cur_num_tree_vertices[b_ndx] += 1;
                }

                for poly in &mesh.polygons {
                    if self.cur_num_tree_indices[b_ndx] + 4 > MAX_TREE_INDEX as i32 {
                        break;
                    }
                    self.cpu_indices.push(start_vertex as u16 + poly[0]);
                    self.cpu_indices.push(start_vertex as u16 + poly[1]);
                    self.cpu_indices.push(start_vertex as u16 + poly[2]);
                    self.cur_num_tree_indices[b_ndx] += 3;
                }

                cur_tree += 1;
            }
        }
    }

    fn images_for_texture_update(&self) -> Vec<TreeTileImageSpec> {
        let mut images = self.last_tile_images.clone();
        for tree_type in &self.tree_types {
            let name = tree_type.data.texture_name.trim();
            if name.is_empty() {
                continue;
            }
            if images
                .iter()
                .any(|image| image.texture_name.eq_ignore_ascii_case(name))
            {
                continue;
            }
            let header = probe_tree_tga_header(name)
                .unwrap_or_else(|| TreeTgaHeader::truecolor(TILE_PIXEL_EXTENT, TILE_PIXEL_EXTENT));
            images.push(TreeTileImageSpec {
                texture_name: name.to_string(),
                header,
            });
        }
        images
    }

    fn ensure_source_tile_pixels(&mut self) {
        if self.num_tiles <= 0 {
            return;
        }
        let needed = self.num_tiles as usize;
        if self.source_tiles.len() < needed {
            self.source_tiles.resize(needed, Vec::new());
        }
        let mut loaded_by_name: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
        for ndx in 0..needed {
            if self.source_tiles[ndx].len() == TREE_TILE_DATA_LEN {
                continue;
            }
            let Some(type_ndx) = self.tree_type_index_for_tile(ndx) else {
                continue;
            };
            let texture_name = self.tree_types[type_ndx].data.texture_name.clone();
            if texture_name.is_empty() {
                continue;
            }
            let first_tile = self.tree_types[type_ndx].first_tile.max(0) as usize;
            let tile_width = self.tree_types[type_ndx].tile_width.max(1);
            let local_ndx = ndx.saturating_sub(first_tile);
            let cache_key = texture_name.to_ascii_lowercase();
            if !loaded_by_name.contains_key(&cache_key) {
                if let Some(tiles) = load_tree_texture_tiles(&texture_name, tile_width) {
                    loaded_by_name.insert(cache_key.clone(), tiles);
                }
            }
            if let Some(tiles) = loaded_by_name.get(&cache_key) {
                if let Some(tile) = tiles.get(local_ndx) {
                    if tile.len() == TREE_TILE_DATA_LEN {
                        self.source_tiles[ndx] = tile.clone();
                        continue;
                    }
                }
            }
            self.source_tiles[ndx] = stand_in_tile_bgra(&texture_name, local_ndx);
        }
    }

    fn tree_type_index_for_tile(&self, ndx: usize) -> Option<usize> {
        let ndx = ndx as i32;
        self.tree_types.iter().position(|tree_type| {
            tree_type.num_tiles > 0
                && ndx >= tree_type.first_tile
                && ndx < tree_type.first_tile + tree_type.num_tiles
        })
    }

    /// C++ `drawTrees` VB fill: cull then `loadTreesInVertexAndIndexBuffers`.
    /// Called on every TerrainVisual draw/update so the wgpu upload sees lit verts.
    pub fn draw_trees_fill_vertex_buffer(
        &mut self,
        object_lighting: &[TreeObjectLight],
        camera_look_at: Vec3,
        is_visible: impl FnMut(&TreeSphere) -> bool,
    ) -> (&[TreeVertexXyznduv1], &[u16]) {
        self.cull_trees(camera_look_at, is_visible);
        if self.cpu_vertices.is_empty() && self.trees.iter().any(|tree| tree.tree_type >= 0) {
            self.anything_changed = true;
        }
        self.load_trees_in_vertex_and_index_buffers(object_lighting);
        (&self.cpu_vertices, &self.cpu_indices)
    }

    fn scaled_bounds(&self, tree_type: usize, location: Vec3, scale: f32) -> TreeSphere {
        let base = self.tree_types[tree_type].bounds;
        TreeSphere {
            center: base.center * scale + location,
            radius: base.radius * scale,
        }
    }

    fn apply_toppling_force_by_index(
        &mut self,
        index: usize,
        topple_direction: Vec3,
        mut topple_speed: f32,
        options: u32,
    ) -> bool {
        if self.trees[index].topple_state != W3DToppleState::Upright {
            return false;
        }
        let tree_type = self.trees[index].tree_type as usize;
        let data = &self.tree_types[tree_type].data;
        if topple_speed < data.minimum_topple_speed {
            topple_speed = data.minimum_topple_speed;
        }
        let direction = topple_direction.normalize_or_zero();
        self.trees[index].topple_direction = direction;
        self.trees[index].angular_accumulation = 0.0;
        self.trees[index].angular_velocity = topple_speed * data.initial_velocity_percent;
        self.trees[index].angular_acceleration = topple_speed * data.initial_accel_percent;
        self.trees[index].topple_state = W3DToppleState::Falling;
        self.trees[index].options = options;
        self.any_push_changed = true;
        self.trees[index].matrix = Mat4::from_translation(self.trees[index].location);
        if let Some(fx_name) = data.topple_fx.clone() {
            self.pending_fx_events.push(TreeFxEvent {
                kind: TreeFxKind::Topple,
                fx_name,
                position: self.trees[index].location,
            });
        }
        true
    }

    fn partition_min_indices(&self, pos: Vec3, radius: f32) -> (i32, i32) {
        let mut x = (pos.x - radius).clamp(self.bounds.lo.x, self.bounds.hi.x);
        let mut y = (pos.y - radius).clamp(self.bounds.lo.y, self.bounds.hi.y);
        if x.is_nan() {
            x = self.bounds.lo.x;
        }
        if y.is_nan() {
            y = self.bounds.lo.y;
        }
        (
            ((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .floor() as i32,
            ((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .floor() as i32,
        )
    }

    fn partition_max_indices(&self, pos: Vec3, radius: f32) -> (i32, i32) {
        let mut x = (pos.x + radius).clamp(self.bounds.lo.x, self.bounds.hi.x);
        let mut y = (pos.y + radius).clamp(self.bounds.lo.y, self.bounds.hi.y);
        if x.is_nan() {
            x = self.bounds.hi.x;
        }
        if y.is_nan() {
            y = self.bounds.hi.y;
        }
        (
            ((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .ceil() as i32,
            ((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .ceil() as i32,
        )
    }
}

fn pre_rotate_topple_matrix(tree: &mut TreeEntry, angle: f32) {
    tree.matrix = Mat4::from_rotation_x(-angle * tree.topple_direction.y) * tree.matrix;
    tree.matrix = Mat4::from_rotation_y(angle * tree.topple_direction.x) * tree.matrix;
}

fn set_matrix_translation(tree: &mut TreeEntry) {
    tree.matrix.w_axis = tree.location.extend(1.0);
}

pub trait TreeRandom {
    fn int_range(&mut self, min: i32, max: i32) -> i32;
    fn real_range(&mut self, min: f32, max: f32) -> f32;
}

/// TGA header fields consumed by C++ `WorldHeightMap::countTiles`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeTgaHeader {
    pub color_map_type: u8,
    pub image_type: u8,
    pub pixel_depth: u8,
    pub image_width: i32,
    pub image_height: i32,
}

impl TreeTgaHeader {
    /// Uncompressed truecolor TGA (type 2) 32bpp.
    #[must_use]
    pub fn truecolor(image_width: i32, image_height: i32) -> Self {
        Self {
            color_map_type: 0,
            image_type: 0x2,
            pixel_depth: 32,
            image_width,
            image_height,
        }
    }
}

/// Named TGA stand-in for `W3DTreeBuffer::updateTexture` file open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeTileImageSpec {
    pub texture_name: String,
    pub header: TreeTgaHeader,
}

fn tree_texture_path_candidates(texture_name: &str) -> Vec<String> {
    let name = texture_name.trim();
    if name.is_empty() {
        return Vec::new();
    }
    let mut candidates = vec![name.to_string()];
    let has_sep = name.contains('/') || name.contains('\\');
    if !has_sep {
        candidates.push(format!("Art/Terrain/{name}"));
        candidates.push(format!("Art/Textures/{name}"));
    }
    candidates
}

fn probe_tree_tga_header(texture_name: &str) -> Option<TreeTgaHeader> {
    for path in tree_texture_path_candidates(texture_name) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.len() < 18 {
            continue;
        }
        return Some(TreeTgaHeader {
            color_map_type: bytes[1],
            image_type: bytes[2],
            pixel_depth: bytes[16],
            image_width: i16::from_le_bytes([bytes[12], bytes[13]]) as i32,
            image_height: i16::from_le_bytes([bytes[14], bytes[15]]) as i32,
        });
    }
    None
}

fn load_tree_texture_tiles(texture_name: &str, tile_width: i32) -> Option<Vec<Vec<u8>>> {
    let rows = tile_width.max(1) as usize;
    for path in tree_texture_path_candidates(texture_name) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(image) = image::load_from_memory(&bytes) else {
            continue;
        };
        return Some(split_image_into_tree_tiles(&image.to_rgba8(), rows));
    }
    None
}

fn split_image_into_tree_tiles(image: &image::RgbaImage, rows: usize) -> Vec<Vec<u8>> {
    let mut tiles = vec![vec![0u8; TREE_TILE_DATA_LEN]; rows.saturating_mul(rows)];
    let extent = TILE_PIXEL_EXTENT as usize;
    for tile_row in 0..rows {
        for tile_col in 0..rows {
            let ndx = tile_row * rows + tile_col;
            let tile = &mut tiles[ndx];
            for y in 0..extent {
                for x in 0..extent {
                    let src_x = tile_col * extent + x;
                    let src_y = tile_row * extent + y;
                    let dst = (y * extent + x) * TILE_BYTES_PER_PIXEL;
                    if src_x < image.width() as usize && src_y < image.height() as usize {
                        let rgba = image.get_pixel(src_x as u32, src_y as u32).0;
                        tile[dst] = rgba[2];
                        tile[dst + 1] = rgba[1];
                        tile[dst + 2] = rgba[0];
                        tile[dst + 3] = rgba[3];
                    }
                }
            }
        }
    }
    tiles
}

pub(crate) fn stand_in_tile_bgra(texture_name: &str, local_ndx: usize) -> Vec<u8> {
    let mut tile = vec![0u8; TREE_TILE_DATA_LEN];
    let mut hash = 2166136261u32 ^ local_ndx as u32;
    for byte in texture_name.bytes() {
        hash = hash.wrapping_mul(16777619) ^ u32::from(byte);
    }
    let b = hash as u8;
    let g = (hash >> 8) as u8;
    let r = (hash >> 16) as u8;
    for pixel in tile.chunks_exact_mut(TILE_BYTES_PER_PIXEL) {
        pixel[0] = b;
        pixel[1] = g;
        pixel[2] = r;
        pixel[3] = 0xFF;
    }
    tile
}

/// C++ `WorldHeightMap::countTiles` (width/height in pixels, no stream).
#[must_use]
pub fn count_tree_tiles(header: TreeTgaHeader) -> (i32, bool) {
    if header.color_map_type != 0 {
        return (0, false);
    }
    if header.image_type != 0x2 && header.image_type != 0xA {
        return (0, false);
    }
    if header.pixel_depth < 24 || header.pixel_depth > 32 {
        return (0, false);
    }
    let tile_width = header.image_width / TILE_PIXEL_EXTENT;
    let tile_height = header.image_height / TILE_PIXEL_EXTENT;
    if tile_width > 10 || tile_height > 10 {
        return (0, false);
    }
    for size in (1..=10).rev() {
        if tile_width >= size && tile_height >= size {
            return (size * size, false);
        }
    }
    if header.image_height == TILE_PIXEL_EXTENT / 2 && header.image_width == TILE_PIXEL_EXTENT / 2 {
        return (1, true);
    }
    (0, false)
}

/// C++ `updateTexture` width loop: largest square `width` with `width*width <= numTiles`.
#[must_use]
pub fn square_width_from_tile_count(num_tiles: i32) -> i32 {
    for width in (1..=10).rev() {
        if num_tiles >= width * width {
            return width;
        }
    }
    0
}

/// C++ `tmpWidth` / `m_textureWidth` from total unique tiles.
#[must_use]
pub fn tree_atlas_pixel_size(num_tiles: i32) -> (i32, i32, bool) {
    let mut tmp_width = 8;
    while tmp_width * tmp_width < num_tiles {
        tmp_width *= 2;
    }
    let texture_width = tmp_width * TILE_PIXEL_EXTENT;
    if texture_width > MAX_TEX_WIDTH {
        return (64, 64, true);
    }
    (texture_width, texture_width, false)
}

fn pack_tree_atlas(
    tree_types: &mut [TreeTypeInfo],
    tile_locations: &mut [(i32, i32)],
    num_tiles: i32,
) {
    let mut tmp_width = 8;
    while tmp_width * tmp_width < num_tiles {
        tmp_width *= 2;
    }
    let tiles_per_row = tmp_width;
    let grid = MAX_TEX_WIDTH / TILE_PIXEL_EXTENT;
    let mut available = vec![vec![true; grid as usize]; grid as usize];

    for tile_width in (1..=tiles_per_row).rev() {
        for tex_class in 0..tree_types.len() {
            let width = tree_types[tex_class].tile_width;
            if width != tile_width {
                continue;
            }
            let texture_name = tree_types[tex_class].data.texture_name.clone();
            let mut tex_found = false;
            for i in 0..tex_class {
                if tree_types[i]
                    .data
                    .texture_name
                    .eq_ignore_ascii_case(&texture_name)
                {
                    tree_types[tex_class].texture_origin = tree_types[i].texture_origin;
                    tex_found = true;
                    break;
                }
            }
            if tex_found {
                continue;
            }

            let mut found = false;
            let mut found_row = 0i32;
            let mut found_column = 0i32;
            let limit = (tiles_per_row - width) + 1;
            let mut row = 0i32;
            while row < limit && !found {
                let mut column = 0i32;
                while column < limit && !found {
                    if available[row as usize][column as usize] {
                        let mut open = true;
                        let mut i = 0;
                        while i < width && open {
                            let mut j = 0;
                            while j < width && open {
                                if !available[(row + j) as usize][(column + i) as usize] {
                                    open = false;
                                }
                                j += 1;
                            }
                            i += 1;
                        }
                        if open {
                            found = true;
                            found_row = row;
                            found_column = column;
                        }
                        break;
                    }
                    column += 1;
                }
                if found {
                    break;
                }
                row += 1;
            }
            if !found {
                tree_types[tex_class].texture_origin = (0, 0);
                continue;
            }

            let x_origin = found_column * TILE_PIXEL_EXTENT;
            let y_origin = found_row * TILE_PIXEL_EXTENT;
            tree_types[tex_class].texture_origin = (x_origin, y_origin);
            let first_tile = tree_types[tex_class].first_tile;
            for i in 0..width {
                for j in 0..width {
                    available[(found_row + j) as usize][(found_column + i) as usize] = false;
                    let base_ndx = (first_tile + i + j * width) as usize;
                    let x = x_origin + i * TILE_PIXEL_EXTENT;
                    let y = y_origin + ((width - j) - 1) * TILE_PIXEL_EXTENT;
                    if let Some(loc) = tile_locations.get_mut(base_ndx) {
                        *loc = (x, y);
                    }
                }
            }
        }
    }
}

/// C++ `W3DTreeTextureClass::update` row invert blit into A8R8G8B8 atlas.
pub fn blit_tree_tile_into_atlas(
    atlas: &mut [u8],
    atlas_width: i32,
    tile_bgra: &[u8],
    dest_x: i32,
    dest_y: i32,
) {
    if tile_bgra.len() < TREE_TILE_DATA_LEN || atlas_width <= 0 {
        return;
    }
    let atlas_height = (atlas.len() / (atlas_width as usize * TILE_BYTES_PER_PIXEL)) as i32;
    let stride = atlas_width as usize * TILE_BYTES_PER_PIXEL;
    for j in 0..TILE_PIXEL_EXTENT {
        let src_row = TILE_PIXEL_EXTENT - (1 + j);
        let dest_row = dest_y + j;
        if dest_row < 0 || dest_row >= atlas_height {
            continue;
        }
        let src_off = (src_row as usize) * TILE_PIXEL_EXTENT as usize * TILE_BYTES_PER_PIXEL;
        let dest_off =
            (dest_row as usize) * stride + (dest_x.max(0) as usize) * TILE_BYTES_PER_PIXEL;
        for i in 0..TILE_PIXEL_EXTENT {
            let column = dest_x + i;
            if column < 0 || column >= atlas_width {
                continue;
            }
            let s = src_off + (i as usize) * TILE_BYTES_PER_PIXEL;
            let d = dest_off + (i as usize) * TILE_BYTES_PER_PIXEL;
            if s + 4 <= tile_bgra.len() && d + 4 <= atlas.len() {
                atlas[d..d + 4].copy_from_slice(&tile_bgra[s..s + 4]);
            }
        }
    }
}

/// C++ `TileData::doMip` / D3DX_FILTER_BOX 2×2 average with `+2 / 4`.
pub fn do_tree_atlas_mip(hi: &[u8], hi_row: i32) -> Vec<u8> {
    if hi_row < 2 {
        return hi.to_vec();
    }
    let lo_row = hi_row / 2;
    let mut lo = vec![0u8; (lo_row as usize) * (lo_row as usize) * TILE_BYTES_PER_PIXEL];
    for i in (0..hi_row).step_by(2) {
        for j in (0..hi_row).step_by(2) {
            let ndx = ((j * hi_row + i) as usize) * TILE_BYTES_PER_PIXEL;
            let mut lo_ndx = ((j / 2) * lo_row + (i / 2)) as usize;
            lo_ndx *= TILE_BYTES_PER_PIXEL;
            for p in 0..TILE_BYTES_PER_PIXEL {
                let src = ndx + p;
                let pxl = hi[src] as i32
                    + hi[src + TILE_BYTES_PER_PIXEL] as i32
                    + hi[src + TILE_BYTES_PER_PIXEL * hi_row as usize] as i32
                    + hi[src + TILE_BYTES_PER_PIXEL * hi_row as usize + TILE_BYTES_PER_PIXEL]
                        as i32
                    + 2;
                lo[lo_ndx + p] = (pxl / 4) as u8;
            }
        }
    }
    lo
}

/// Full mip chain starting at level 0 (atlas width×height BGRA).
#[must_use]
pub fn generate_box_mip_chain(level0: &[u8], width: i32, height: i32) -> Vec<Vec<u8>> {
    let mut mips = vec![level0.to_vec()];
    let mut w = width.min(height);
    let mut current = level0.to_vec();
    while w > 1 {
        current = do_tree_atlas_mip(&current, w);
        mips.push(current.clone());
        w /= 2;
    }
    mips
}

/// C++ `MAX_GLOBAL_LIGHTS` used by `W3DTreeBuffer::doLighting`.
pub const TREE_MAX_GLOBAL_LIGHTS: usize = 3;

/// One `GlobalData::TerrainLighting` slot for object/tree lighting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeObjectLight {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub light_pos: [f32; 3],
}

impl Default for TreeObjectLight {
    fn default() -> Self {
        Self {
            ambient: [0.0; 3],
            diffuse: [0.0; 3],
            light_pos: [0.0, 0.0, -1.0],
        }
    }
}

fn tree_real_to_int(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}

/// C++ `W3DTreeBuffer::doLighting` (global object lights + optional vertex tint).
pub fn do_lighting(
    normal: [f32; 3],
    object_lighting: &[TreeObjectLight],
    emissive: [f32; 3],
    vert_diffuse: u32,
    scale: f32,
) -> u32 {
    let light0 = object_lighting.first().copied().unwrap_or_default();
    let mut shade_r = light0.ambient[0] + emissive[0];
    let mut shade_g = light0.ambient[1] + emissive[1];
    let mut shade_b = light0.ambient[2] + emissive[2];

    for i in 0..TREE_MAX_GLOBAL_LIGHTS {
        let light = object_lighting.get(i).copied().unwrap_or_default();
        let len = (light.light_pos[0] * light.light_pos[0]
            + light.light_pos[1] * light.light_pos[1]
            + light.light_pos[2] * light.light_pos[2])
            .sqrt();
        if len <= 1.0e-8 {
            continue;
        }
        let inv = 1.0 / len;
        let light_ray = [
            -light.light_pos[0] * inv,
            -light.light_pos[1] * inv,
            -light.light_pos[2] * inv,
        ];
        let mut shade =
            light_ray[0] * normal[0] + light_ray[1] * normal[1] + light_ray[2] * normal[2];
        shade = shade.clamp(0.0, 1.0);
        shade_r += shade * light.diffuse[0];
        shade_g += shade * light.diffuse[1];
        shade_b += shade * light.diffuse[2];
    }

    shade_r = (shade_r * scale).clamp(0.0, 1.0);
    shade_g = (shade_g * scale).clamp(0.0, 1.0);
    shade_b = (shade_b * scale).clamp(0.0, 1.0);

    if vert_diffuse != 0xFFFF_FFFF {
        shade_b *= (vert_diffuse & 0xFF) as f32 / 255.0;
        shade_g *= ((vert_diffuse >> 8) & 0xFF) as f32 / 255.0;
        shade_r *= ((vert_diffuse >> 16) & 0xFF) as f32 / 255.0;
    }

    shade_r *= 255.0;
    shade_g *= 255.0;
    shade_b *= 255.0;
    (tree_real_to_int(shade_b) as u32)
        | ((tree_real_to_int(shade_g) as u32) << 8)
        | ((tree_real_to_int(shade_r) as u32) << 16)
        | (255u32 << 24)
}

/// Crossed card used when `addTreeType` has no WW3D mesh snapshot.
fn stand_in_tree_type_mesh(bounds: &TreeSphere) -> TreeTypeMesh {
    let radius = bounds.radius.max(TREE_RADIUS_APPROX * 0.5);
    let height = radius * 2.0;
    TreeTypeMesh {
        positions: vec![
            [-radius, 0.0, 0.0],
            [radius, 0.0, 0.0],
            [radius, 0.0, height],
            [-radius, 0.0, height],
            [0.0, -radius, 0.0],
            [0.0, radius, 0.0],
            [0.0, radius, height],
            [0.0, -radius, height],
        ],
        normals: Some(vec![
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        ]),
        uvs: vec![
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [0.0, 0.0],
        ],
        colors: None,
        polygons: vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
        emissive: [0.0, 0.0, 0.0],
    }
}

/// Map C++ Z-up tree VB into the shipped wgpu Y-up upload layout.
/// `diffuse` is the exact `doLighting` packed BGRA written into the GPU VB.
pub fn fill_tree_gpu_upload_vertices(cpu: &[TreeVertexXyznduv1]) -> Vec<TreeGpuVertex> {
    cpu.iter()
        .map(|vertex| {
            let r = ((vertex.diffuse >> 16) & 0xFF) as f32 / 255.0;
            let g = ((vertex.diffuse >> 8) & 0xFF) as f32 / 255.0;
            let b = (vertex.diffuse & 0xFF) as f32 / 255.0;
            TreeGpuVertex {
                position: [vertex.x, vertex.z, vertex.y],
                color: [r, g, b],
                tex_coords: [vertex.u1, vertex.v1],
                diffuse: vertex.diffuse,
            }
        })
        .collect()
}

struct DeterministicTreeRandom;

impl TreeRandom for DeterministicTreeRandom {
    fn int_range(&mut self, min: i32, _max: i32) -> i32 {
        min
    }

    fn real_range(&mut self, min: f32, max: f32) -> f32 {
        (min + max) * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn do_lighting_matches_cpp_w3d_tree_buffer() {
        let lights = [TreeObjectLight {
            ambient: [0.2, 0.1, 0.0],
            diffuse: [0.8, 0.4, 0.0],
            light_pos: [0.0, 0.0, -1.0],
        }];
        // lightRay = (0,0,1), normal (0,0,1) → shade=1
        let lit = do_lighting([0.0, 0.0, 1.0], &lights, [0.0, 0.0, 0.0], 0xFFFF_FFFF, 1.0);
        let expect_r = ((0.2_f32 + 0.8).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let expect_g = ((0.1_f32 + 0.4).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        assert_eq!((lit >> 16) & 0xFF, expect_r);
        assert_eq!((lit >> 8) & 0xFF, expect_g);
        assert_eq!(lit & 0xFF, 0);
        assert_eq!(lit >> 24, 0xFF);

        let back = do_lighting(
            [0.0, 0.0, -1.0],
            &lights,
            [0.05, 0.0, 0.0],
            0xFFFF_FFFF,
            1.0,
        );
        let back_r = ((0.2_f32 + 0.05).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        assert_eq!((back >> 16) & 0xFF, back_r);
        assert_eq!((back >> 8) & 0xFF, (0.1 * 255.0 + 0.5) as u32);

        let tinted = do_lighting([0.0, 0.0, 1.0], &lights, [0.0, 0.0, 0.0], 0xFF80_0000, 1.0);
        // red * 128/255 after clamp-to-1
        let tint_r = (((0.2 + 0.8) * (0x80 as f32 / 255.0)) * 255.0 + 0.5) as u32;
        assert_eq!((tinted >> 16) & 0xFF, tint_r);
        assert_eq!(tinted & 0xFF, 0);
    }

    #[test]
    fn load_trees_vb_calls_do_lighting_every_vertex() {
        let mut buffer = W3DTreeBuffer::new();
        buffer.set_bounds(TreeRegion2D::new(Vec2::ZERO, Vec2::new(100.0, 100.0)));
        let type_idx = buffer
            .add_tree_type(
                TreeModuleData {
                    model_name: "Oak".into(),
                    texture_name: "OakT".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere {
                    center: Vec3::ZERO,
                    radius: 2.0,
                },
            )
            .unwrap();
        {
            let info = buffer.tree_type_mut(type_idx).unwrap();
            info.tile_width = 1;
            info.offset = Vec3::ZERO;
        }
        buffer.set_tree_type_mesh(
            type_idx,
            TreeTypeMesh {
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: Some(vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]),
                uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                colors: Some(vec![0xFFFF_FFFF, 0xFF80_0000, 0xFFFF_FFFF]),
                polygons: vec![[0, 1, 2]],
                emissive: [0.0, 0.0, 0.0],
            },
        );
        buffer
            .add_tree(
                1,
                Vec3::new(10.0, 20.0, 3.0),
                2.0,
                0.0,
                0.0,
                TreeModuleData {
                    model_name: "Oak".into(),
                    texture_name: "OakT".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere {
                    center: Vec3::ZERO,
                    radius: 2.0,
                },
            )
            .unwrap();

        let lights = [TreeObjectLight {
            ambient: [0.2, 0.1, 0.0],
            diffuse: [0.8, 0.4, 0.0],
            light_pos: [0.0, 0.0, -1.0],
        }];
        buffer.draw_trees_fill_vertex_buffer(&lights, Vec3::Z, |_| true);

        assert_eq!(buffer.cpu_vertices().len(), 3);
        assert_eq!(buffer.cpu_indices(), &[0, 1, 2]);

        let up = do_lighting([0.0, 0.0, 1.0], &lights, [0.0, 0.0, 0.0], 0xFFFF_FFFF, 1.0);
        let tinted = do_lighting([0.0, 0.0, 1.0], &lights, [0.0, 0.0, 0.0], 0xFF80_0000, 1.0);
        let side = do_lighting([1.0, 0.0, 0.0], &lights, [0.0, 0.0, 0.0], 0xFFFF_FFFF, 1.0);
        assert_eq!(buffer.cpu_vertices()[0].diffuse, up);
        assert_eq!(buffer.cpu_vertices()[1].diffuse, tinted);
        assert_eq!(buffer.cpu_vertices()[2].diffuse, side);
        assert_eq!(buffer.cpu_vertices()[0].x, 10.0);
        assert_eq!(buffer.cpu_vertices()[0].y, 20.0);
        assert_eq!(buffer.cpu_vertices()[0].z, 3.0);
        assert_eq!(buffer.cpu_vertices()[1].x, 12.0);
        assert_eq!(buffer.cpu_vertices()[1].y, 20.0);

        let gpu = fill_tree_gpu_upload_vertices(buffer.cpu_vertices());
        assert_eq!(gpu.len(), 3);
        assert_eq!(gpu[0].diffuse, up);
        assert_eq!(gpu[1].diffuse, tinted);
        assert_eq!(gpu[2].diffuse, side);
        assert_eq!(gpu[0].position, [10.0, 3.0, 20.0]);
        assert!((gpu[0].color[0] - (((up >> 16) & 0xFF) as f32 / 255.0)).abs() < 1e-5);
    }

    #[test]
    fn count_tree_tiles_matches_cpp_world_height_map() {
        assert_eq!(
            count_tree_tiles(TreeTgaHeader::truecolor(64, 64)),
            (1, false)
        );
        assert_eq!(
            count_tree_tiles(TreeTgaHeader::truecolor(128, 128)),
            (4, false)
        );
        assert_eq!(
            count_tree_tiles(TreeTgaHeader::truecolor(192, 192)),
            (9, false)
        );
        assert_eq!(
            count_tree_tiles(TreeTgaHeader::truecolor(640, 640)),
            (100, false)
        );
        assert_eq!(
            count_tree_tiles(TreeTgaHeader::truecolor(704, 704)),
            (0, false)
        );
        assert_eq!(
            count_tree_tiles(TreeTgaHeader::truecolor(32, 32)),
            (1, true)
        );
        let mut indexed = TreeTgaHeader::truecolor(64, 64);
        indexed.color_map_type = 1;
        assert_eq!(count_tree_tiles(indexed), (0, false));
        let mut bad_type = TreeTgaHeader::truecolor(64, 64);
        bad_type.image_type = 1;
        assert_eq!(count_tree_tiles(bad_type), (0, false));
        assert_eq!(square_width_from_tile_count(9), 3);
        assert_eq!(square_width_from_tile_count(5), 2);
        assert_eq!(square_width_from_tile_count(4), 2);
        assert_eq!(square_width_from_tile_count(1), 1);
        assert_eq!(tree_atlas_pixel_size(2), (512, 512, false));
        assert_eq!(tree_atlas_pixel_size(64), (512, 512, false));
        assert_eq!(tree_atlas_pixel_size(65), (1024, 1024, false));
        assert_eq!(tree_atlas_pixel_size(1025), (64, 64, true));
        assert_eq!(MAX_TEX_WIDTH, 2048);
        assert_eq!(MAX_TILES, 512);
    }

    #[test]
    fn update_texture_packs_atlas_like_cpp_w3d_tree_buffer() {
        let mut buffer = W3DTreeBuffer::new();
        buffer
            .add_tree_type(
                TreeModuleData {
                    model_name: "Oak".into(),
                    texture_name: "Oak.tga".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere::default(),
            )
            .unwrap();
        buffer
            .add_tree_type(
                TreeModuleData {
                    model_name: "Pine".into(),
                    texture_name: "Pine.tga".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere::default(),
            )
            .unwrap();
        // Duplicate texture name copies origin and does not consume tiles.
        buffer
            .add_tree_type(
                TreeModuleData {
                    model_name: "OakB".into(),
                    texture_name: "oak.tga".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere::default(),
            )
            .unwrap();

        buffer.update_texture(&[
            TreeTileImageSpec {
                texture_name: "Oak.tga".into(),
                header: TreeTgaHeader::truecolor(64, 64),
            },
            TreeTileImageSpec {
                texture_name: "Pine.tga".into(),
                header: TreeTgaHeader::truecolor(64, 64),
            },
        ]);

        assert_eq!(buffer.num_tiles(), 2);
        assert_eq!(buffer.texture_size(), (512, 512));
        assert!(!buffer.need_to_update_texture());
        let oak = &buffer.tree_types()[0];
        let pine = &buffer.tree_types()[1];
        let oak_b = &buffer.tree_types()[2];
        assert_eq!(oak.tile_width, 1);
        assert_eq!(oak.num_tiles, 1);
        assert_eq!(oak.texture_origin, (0, 0));
        assert_eq!(pine.tile_width, 1);
        assert_eq!(pine.num_tiles, 1);
        assert_eq!(pine.texture_origin, (64, 0));
        assert_eq!(oak_b.tile_width, 1);
        assert_eq!(oak_b.num_tiles, 0);
        assert_eq!(oak_b.texture_origin, oak.texture_origin);
        assert_eq!(buffer.tile_location_in_texture(0), Some((0, 0)));
        assert_eq!(buffer.tile_location_in_texture(1), Some((64, 0)));

        // 2x2 tile class: origin (0,0), tiles flipped in V like C++ ((width-j)-1).
        let mut two_by_two = W3DTreeBuffer::new();
        two_by_two
            .add_tree_type(
                TreeModuleData {
                    model_name: "Bush".into(),
                    texture_name: "Bush.tga".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere::default(),
            )
            .unwrap();
        two_by_two.update_texture(&[TreeTileImageSpec {
            texture_name: "Bush.tga".into(),
            header: TreeTgaHeader::truecolor(128, 128),
        }]);
        assert_eq!(two_by_two.num_tiles(), 4);
        assert_eq!(two_by_two.tree_types()[0].tile_width, 2);
        assert_eq!(two_by_two.tree_types()[0].texture_origin, (0, 0));
        // i=0,j=0 → y = ((2-0)-1)*64 = 64; i=0,j=1 → y = 0
        assert_eq!(two_by_two.tile_location_in_texture(0), Some((0, 64)));
        assert_eq!(two_by_two.tile_location_in_texture(2), Some((0, 0)));
        assert_eq!(two_by_two.tile_location_in_texture(1), Some((64, 64)));
        assert_eq!(two_by_two.tile_location_in_texture(3), Some((64, 0)));
    }

    #[test]
    fn tree_atlas_blit_inverts_rows_and_box_mip_matches_tiledata() {
        let mut tile = vec![0u8; TREE_TILE_DATA_LEN];
        // Source row 0, col 0 = unique (1,2,3,4); row 63 col 0 = (9,8,7,6)
        tile[0..4].copy_from_slice(&[1, 2, 3, 4]);
        let last_row = 63 * 64 * 4;
        tile[last_row..last_row + 4].copy_from_slice(&[9, 8, 7, 6]);

        let mut atlas = vec![0u8; 64 * 64 * 4];
        blit_tree_tile_into_atlas(&mut atlas, 64, &tile, 0, 0);
        // j=0 dest row 0 comes from src row 63
        assert_eq!(&atlas[0..4], &[9, 8, 7, 6]);
        // j=63 dest row 63 comes from src row 0
        let dest_last = 63 * 64 * 4;
        assert_eq!(&atlas[dest_last..dest_last + 4], &[1, 2, 3, 4]);

        let mut hi = vec![0u8; 2 * 2 * 4];
        hi[0] = 10;
        hi[4] = 20;
        hi[8] = 30;
        hi[12] = 40;
        let lo = do_tree_atlas_mip(&hi, 2);
        assert_eq!(lo.len(), 4);
        assert_eq!(lo[0], ((10 + 20 + 30 + 40 + 2) / 4) as u8);

        let mips = generate_box_mip_chain(&atlas, 64, 64);
        assert_eq!(mips.len(), 7);
        assert_eq!(mips[0].len(), 64 * 64 * 4);
        assert_eq!(mips[1].len(), 32 * 32 * 4);
        assert_eq!(mips[6].len(), 1 * 1 * 4);
    }

    #[test]
    fn update_tree_texture_class_uploads_lod_skipped_mips() {
        let mut buffer = W3DTreeBuffer::new();
        buffer
            .add_tree_type(
                TreeModuleData {
                    model_name: "Oak".into(),
                    texture_name: "Oak.tga".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere::default(),
            )
            .unwrap();
        buffer.update_texture(&[TreeTileImageSpec {
            texture_name: "Oak.tga".into(),
            header: TreeTgaHeader::truecolor(64, 64),
        }]);
        let mut tile = vec![0u8; TREE_TILE_DATA_LEN];
        tile[0..4].copy_from_slice(&[10, 20, 30, 40]);
        assert!(buffer.set_source_tile_bgra(0, &tile));
        let height = buffer.update_tree_texture_class(1);
        assert_eq!(height, 512);
        assert_eq!(buffer.atlas_lod(), 1);
        assert_eq!(buffer.atlas_upload_mip_index(), 1);
        let full = buffer.atlas_mips();
        let upload = buffer.atlas_upload_levels();
        assert_eq!(full.len(), 10); // 512..1
        assert_eq!(upload.len(), full.len() - 1);
        assert_eq!(upload[0].len(), full[1].len());
        // Blit invert: src (0,0) lands at dest (0,63) of the 64x64 tile at origin (0,0)
        let mip0 = &full[0];
        let dest = (63 * 512 + 0) * 4;
        assert_eq!(&mip0[dest..dest + 4], &[10, 20, 30, 40]);
    }

    #[test]
    fn update_tree_texture_live_draw_tree_atlas_matches_cpp_blit_mip_lod() {
        let mut buffer = W3DTreeBuffer::new();
        buffer
            .add_tree_type(
                TreeModuleData {
                    model_name: "Oak".into(),
                    texture_name: "Oak.tga".into(),
                    ..TreeModuleData::default()
                },
                TreeSphere::default(),
            )
            .unwrap();
        let mut tile = vec![0u8; TREE_TILE_DATA_LEN];
        tile[0..4].copy_from_slice(&[11, 22, 33, 44]);
        assert!(buffer.set_source_tile_bgra(0, &tile));
        buffer.update_texture(&[TreeTileImageSpec {
            texture_name: "Oak.tga".into(),
            header: TreeTgaHeader::truecolor(64, 64),
        }]);
        assert_eq!(
            &buffer.source_tile_bgra(0).unwrap()[0..4],
            &[11, 22, 33, 44]
        );
        assert!(buffer.atlas_mips().is_empty());

        let height = buffer.sync_tree_atlas_for_draw(1);
        assert_eq!(height, 512);
        assert_eq!(buffer.atlas_lod(), 1);
        assert_eq!(buffer.atlas_upload_mip_index(), 1);

        let (width, height_px) = buffer.texture_size();
        let mut level0 = vec![0u8; (width as usize) * (height_px as usize) * TILE_BYTES_PER_PIXEL];
        let loc = buffer.tile_location_in_texture(0).unwrap();
        blit_tree_tile_into_atlas(&mut level0, width, &tile, loc.0, loc.1);
        let expected = generate_box_mip_chain(&level0, width, height_px);
        let upload = buffer.atlas_upload_levels();
        assert_eq!(upload[0].len(), expected[1].len());
        assert_eq!(upload[0], expected[1]);
        assert_eq!(do_tree_atlas_mip(&expected[0], width), expected[1]);
        // Invert-row blit: src (0,0) lands at dest (0,63) of the 64x64 tile at origin (0,0)
        let dest = (63 * 512 + 0) * 4;
        assert_eq!(&expected[0][dest..dest + 4], &[11, 22, 33, 44]);
        assert_eq!(&buffer.atlas_mips()[0][dest..dest + 4], &[11, 22, 33, 44]);

        // Live SetLOD after mips exist (no rebuild) — C++ D3D SetLOD each draw.
        let height_again = buffer.sync_tree_atlas_for_draw(2);
        assert_eq!(height_again, 512);
        assert_eq!(buffer.atlas_lod(), 2);
        assert_eq!(buffer.atlas_upload_mip_index(), 2);
        assert_eq!(buffer.atlas_upload_levels()[0], expected[2]);
    }
}
