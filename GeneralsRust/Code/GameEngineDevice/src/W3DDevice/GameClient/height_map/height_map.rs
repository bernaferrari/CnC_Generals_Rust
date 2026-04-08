use super::{
    base_height_map::BaseHeightMap, flat_height_map::FlatHeightMap,
    world_height_map::WorldHeightMap,
};
use std::sync::{Arc, RwLock};

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
    world: Arc<RwLock<WorldHeightMap>>,
    cached_height: RwLock<CachedHeightSample>,
}

impl HeightMap {
    pub fn new(width: i32, height: i32, border_size: i32) -> Self {
        let world = Arc::new(RwLock::new(WorldHeightMap::with_dimensions(
            width,
            height,
            border_size,
        )));
        Self::from_world_map(world)
    }

    pub fn from_world_map(world: Arc<RwLock<WorldHeightMap>>) -> Self {
        let mut base = BaseHeightMap::new();
        base.set_map(Arc::clone(&world));

        let mut flat = FlatHeightMap::new();
        flat.init_height_data(Arc::clone(&world));

        let (x, y) = {
            let map = world.read().unwrap();
            (map.get_x_extent(), map.get_y_extent())
        };
        base.x = x;
        base.y = y;

        Self {
            base,
            flat,
            world,
            cached_height: RwLock::new(CachedHeightSample::default()),
        }
    }

    pub fn world_map(&self) -> Arc<RwLock<WorldHeightMap>> {
        Arc::clone(&self.world)
    }

    pub fn width(&self) -> i32 {
        self.world.read().unwrap().get_x_extent()
    }

    pub fn height(&self) -> i32 {
        self.world.read().unwrap().get_y_extent()
    }

    pub fn get_height(&self, x: f32, y: f32) -> f32 {
        if let Ok(cache) = self.cached_height.read() {
            if cache.valid && cache.world_x == x && cache.world_y == y {
                return cache.height;
            }
        }

        let height = self.base.get_height_map_height(x, y, None);
        if let Ok(mut cache) = self.cached_height.write() {
            *cache = CachedHeightSample {
                world_x: x,
                world_y: y,
                height,
                valid: true,
            };
        }
        height
    }

    pub fn get_height_lod(&self, x: f32, y: f32, lod: u32) -> f32 {
        self.base.get_height_map_height_lod(x, y, lod, None)
    }

    pub fn get_grid_height(&self, x_index: i32, y_index: i32) -> u8 {
        self.world.read().unwrap().get_height(x_index, y_index)
    }

    pub fn get_grid_height_lod(&self, x_index: i32, y_index: i32, lod: u32) -> u8 {
        self.world
            .read()
            .unwrap()
            .get_height_lod(x_index, y_index, lod)
    }

    pub fn world_to_grid(&self, x: f32, y: f32) -> Option<(i32, i32)> {
        self.base.world_to_grid(x, y)
    }

    pub fn get_max_cell_height(&self, x: f32, y: f32) -> f32 {
        self.base.get_max_cell_height(x, y)
    }

    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        self.base.is_cliff_cell(x, y)
    }

    pub fn create_crater(&self, cx: f32, cy: f32, radius: f32, depth: f32) {
        self.world
            .write()
            .unwrap()
            .create_crater(cx, cy, radius, depth);
        self.invalidate_cache();
    }

    pub fn flatten_area(&self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.world.write().unwrap().flatten_area(x0, y0, x1, y1);
        self.invalidate_cache();
    }

    pub fn snapshot_height_data(&self) -> Vec<u8> {
        self.world.read().unwrap().snapshot_height_data()
    }

    pub fn restore_height_data(&self, data: &[u8]) -> bool {
        let ok = self.world.write().unwrap().restore_height_data(data);
        if ok {
            self.invalidate_cache();
        }
        ok
    }

    pub fn height_data_for_render(&self) -> Vec<u8> {
        self.world.read().unwrap().to_height_data()
    }

    pub fn update_view_impassable_areas(&mut self) {
        self.base.update_view_impassable_areas(false, 0, 0, 0, 0);
    }

    pub fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cached_height.write() {
            cache.valid = false;
        }
    }
}
