use super::{
    base_height_map::BaseHeightMap, flat_height_map::FlatHeightMap,
    world_height_map::WorldHeightMap,
};


pub const VERTEX_BUFFER_TILE_LENGTH: usize = 32;
pub const FLIP_TRIANGLES: bool = true;
pub const DEFAULT_MAX_FRAME_EXTRA_BLEND_TILES: usize = 256;
pub const DEFAULT_MAX_MAP_EXTRA_BLEND_TILES: usize = 2048;
pub const DEFAULT_MAX_BATCH_SHORELINE_TILES: usize = 512;
pub const DEFAULT_MAX_MAP_SHORELINE_TILES: usize = 4096;

#[derive(Debug, Clone, Copy, Default)]
struct CachedHeightSample {
    world_x: f32,
    world_y: f32,
    height: f32,
    valid: bool,
}

pub struct HeightMap {
    pub base: BaseHeightMap,
    pub flat: FlatHeightMap,
    world: WorldHeightMap,
    cached_height: CachedHeightSample,
    /// C++ `m_extraBlendTilePositions` packed as `i | (j << 16)`.
    extra_blend_tile_positions: Vec<i32>,
}

impl HeightMap {
    pub fn new(width: i32, height: i32, border_size: i32) -> Self {
        let world = WorldHeightMap::with_dimensions(width, height, border_size);
        Self::from_world_map(world)
    }

    /// Borrow-first ownership migration: `HeightMap` is the single owner of the
    /// `WorldHeightMap`; `BaseHeightMap`/`FlatHeightMap` receive `&WorldHeightMap`
    /// per query instead of shared `Arc<RwLock>` pins (C++ m_map back-pointer).
    pub fn from_world_map(world: WorldHeightMap) -> Self {
        let mut base = BaseHeightMap::new();
        base.x = world.get_x_extent();
        base.y = world.get_y_extent();

        let mut flat = FlatHeightMap::new();
        flat.init_height_data(&world);

        let extra_blend_tile_positions = world.collect_extra_blend_tile_positions();
        Self {
            base,
            flat,
            world,
            cached_height: CachedHeightSample::default(),
            extra_blend_tile_positions,
        }
    }

    pub fn width(&self) -> i32 {
        self.world.get_x_extent()
    }

    pub fn height(&self) -> i32 {
        self.world.get_y_extent()
    }

    pub fn get_height(&mut self, x: f32, y: f32) -> f32 {
        if self.cached_height.valid
            && self.cached_height.world_x == x
            && self.cached_height.world_y == y
        {
            return self.cached_height.height;
        }

        let height = self.base.get_height_map_height(&self.world, x, y, None);
        self.cached_height = CachedHeightSample {
            world_x: x,
            world_y: y,
            height,
            valid: true,
        };
        height
    }

    pub fn get_height_lod(&self, x: f32, y: f32, lod: u32) -> f32 {
        self.base.get_height_map_height_lod(&self.world, x, y, lod, None)
    }

    pub fn get_grid_height(&self, x_index: i32, y_index: i32) -> u8 {
        self.world.get_height(x_index, y_index)
    }

    pub fn get_grid_height_lod(&self, x_index: i32, y_index: i32, lod: u32) -> u8 {
        self.world.get_height_lod(x_index, y_index, lod)
    }

    pub fn world_to_grid(&self, x: f32, y: f32) -> Option<(i32, i32)> {
        self.base.world_to_grid(&self.world, x, y)
    }

    pub fn get_max_cell_height(&self, x: f32, y: f32) -> f32 {
        self.base.get_max_cell_height(&self.world, x, y)
    }

    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        self.base.is_cliff_cell(&self.world, x, y)
    }

    pub fn create_crater(&mut self, cx: f32, cy: f32, radius: f32, depth: f32) {
        self.world.create_crater(cx, cy, radius, depth);
        self.invalidate_cache();
    }

    pub fn flatten_area(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.world.flatten_area(x0, y0, x1, y1);
        self.invalidate_cache();
    }

    pub fn snapshot_height_data(&self) -> Vec<u8> {
        self.world.snapshot_height_data()
    }

    pub fn restore_height_data(&mut self, data: &[u8]) -> bool {
        let ok = self.world.restore_height_data(data);
        if ok {
            self.invalidate_cache();
        }
        ok
    }

    pub fn height_data_for_render(&self) -> Vec<u8> {
        self.world.to_height_data()
    }

    pub fn update_view_impassable_areas(&mut self) {
        self.base
            .update_view_impassable_areas(&self.world, false, 0, 0, 0, 0);
    }

    pub fn invalidate_cache(&mut self) {
        self.cached_height.valid = false;
    }

    /// Rebuild extra-blend tile positions from the world height map (C++ initHeightData).
    pub fn rebuild_extra_blend_tile_positions(&mut self) {
        self.extra_blend_tile_positions = self.world.collect_extra_blend_tile_positions();
    }

    pub fn extra_blend_tile_count(&self) -> usize {
        self.extra_blend_tile_positions.len()
    }

    pub fn extra_blend_tile_positions(&self) -> &[i32] {
        &self.extra_blend_tile_positions
    }
}
