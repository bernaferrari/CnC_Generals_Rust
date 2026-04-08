//! Flat Height Map Module
//!
//! Port of C++ FlatHeightMap.h and FlatHeightMap.cpp
//! Original: GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/FlatHeightMap.cpp (615 lines)
//! Original: GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/FlatHeightMap.h
//! Author: Mark W., John Ahlquist, April/May 2001
//!
//! PARITY_NOTE: FlatHeightMap is a simplified terrain renderer used for
//! lower LOD levels. It divides terrain into 16x16-cell tiles with pre-baked
//! textures. This file provides the height-map interface compatible with
//! BaseHeightMap for the flat rendering path.

use super::base_height_map::BaseHeightMap;
use super::world_height_map::WorldHeightMap;
use std::sync::{Arc, RwLock};

/// Cells per tile for flat height map (C++ FlatHeightMap.cpp line 87)
pub const CELLS_PER_TILE: i32 = 16;

/// Flat height map update states (C++ FlatHeightMap.h line 62-66)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatUpdateState {
    Idle,
    Moving,
    Moving2,
    UpdateTextures,
}

pub struct FlatHeightMap {
    pub base: BaseHeightMap,
    pub update_state: FlatUpdateState,
    pub tiles_width: i32,
    pub tiles_height: i32,
    pub num_tiles: i32,
    uniform_height: u8,
}

impl FlatHeightMap {
    pub fn new() -> Self {
        Self {
            base: BaseHeightMap::new(),
            update_state: FlatUpdateState::Idle,
            tiles_width: 0,
            tiles_height: 0,
            num_tiles: 0,
            uniform_height: 0,
        }
    }

    pub fn init_height_data(&mut self, map: Arc<RwLock<WorldHeightMap>>) {
        self.base.set_map(Arc::clone(&map));
        let map_guard = map.read().unwrap();
        self.base.x = map_guard.get_x_extent();
        self.base.y = map_guard.get_y_extent();
        self.uniform_height = map_guard.get_height(0, 0);
        self.tiles_width = (map_guard.get_x_extent() + CELLS_PER_TILE - 2) / CELLS_PER_TILE;
        self.tiles_height = (map_guard.get_y_extent() + CELLS_PER_TILE - 2) / CELLS_PER_TILE;
        self.num_tiles = self.tiles_width * self.tiles_height;
    }

    pub fn set_uniform_height(&mut self, height: u8) {
        self.uniform_height = height;
    }

    pub fn get_grid_height(&self, x_index: i32, y_index: i32) -> u8 {
        if let Some(map) = self.base.get_map() {
            map.read().unwrap().get_height(x_index, y_index)
        } else {
            self.uniform_height
        }
    }

    pub fn get_height(&self, x: f32, y: f32) -> f32 {
        self.base.get_height_map_height(x, y, None)
    }

    pub fn get_height_lod(&self, x: f32, y: f32) -> f32 {
        let sample_x = (x / CELLS_PER_TILE as f32).floor() * CELLS_PER_TILE as f32;
        let sample_y = (y / CELLS_PER_TILE as f32).floor() * CELLS_PER_TILE as f32;
        self.base.get_height_map_height(sample_x, sample_y, None)
    }

    pub fn update_center(&mut self) {
        self.update_state = match self.update_state {
            FlatUpdateState::Idle => FlatUpdateState::Idle,
            FlatUpdateState::Moving => FlatUpdateState::Moving2,
            FlatUpdateState::Moving2 => FlatUpdateState::UpdateTextures,
            FlatUpdateState::UpdateTextures => FlatUpdateState::Idle,
        };
    }

    pub fn reset(&mut self) {
        self.base.reset();
        self.update_state = FlatUpdateState::Idle;
    }
}

impl Default for FlatHeightMap {
    fn default() -> Self {
        Self::new()
    }
}
