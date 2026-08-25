//! Production `Radar::newMap` ingest when GameLogic/TerrainLogic load a map.

use super::{Coord3D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, RadarSystem};
use std::sync::{Arc, OnceLock};

/// Terrain sample used to seed `m_xSample` / averages / per-cell heights.
pub trait RadarMapSource: Send + Sync {
    fn map_extent(&self) -> Option<(Coord3D, Coord3D)>;
    /// `(height, is_water)` for one radar cell, or `None` if unmapped.
    fn sample_cell(&self, world_x: f32, world_y: f32) -> Option<(f32, bool)>;
}

static MAP_SOURCE: OnceLock<Arc<dyn RadarMapSource>> = OnceLock::new();

pub fn register_radar_map_source(source: Arc<dyn RadarMapSource>) -> bool {
    MAP_SOURCE.set(source).is_ok()
}

pub fn radar_map_source() -> Option<&'static dyn RadarMapSource> {
    MAP_SOURCE.get().map(|s| s.as_ref())
}

impl RadarSystem {
    /// True after a live `newMap` computed nonzero sample intervals.
    #[must_use]
    pub fn has_map_extent(&self) -> bool {
        self.x_sample > f32::EPSILON && self.y_sample > f32::EPSILON
    }

    /// Pull extent + every-other-cell averages from the registered terrain source.
    pub fn try_new_map_from_source(&mut self) -> bool {
        let Some(source) = radar_map_source() else {
            return false;
        };
        let Some((min, max)) = source.map_extent() else {
            return false;
        };
        if (max.x - min.x).abs() <= f32::EPSILON || (max.y - min.y).abs() <= f32::EPSILON {
            return false;
        }
        if self.has_map_extent()
            && self.map_extent.lo.x == min.x
            && self.map_extent.lo.y == min.y
            && self.map_extent.hi.x == max.x
            && self.map_extent.hi.y == max.y
            && !self.terrain_samples.is_empty()
        {
            return false;
        }

        let x_sample = (max.x - min.x) / RADAR_CELL_WIDTH as f32;
        let y_sample = (max.y - min.y) / RADAR_CELL_HEIGHT as f32;
        let expected = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut heights = Vec::with_capacity(expected);
        for y in 0..RADAR_CELL_HEIGHT {
            for x in 0..RADAR_CELL_WIDTH {
                let wx = min.x + x as f32 * x_sample;
                let wy = min.y + y as f32 * y_sample;
                let (z, is_water) = source.sample_cell(wx, wy).unwrap_or((0.0, false));
                heights.push((wx, z, is_water));
            }
        }
        self.new_map(min, max, &heights);
        true
    }

    /// Re-sample every radar cell from the registered source without `reset`.
    /// Used by `refreshTerrain` so bridge/water changes update the texture.
    pub(crate) fn resample_terrain_from_source(&mut self) -> bool {
        let Some(source) = radar_map_source() else {
            return false;
        };
        if !self.has_map_extent() {
            return false;
        }
        let expected = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut heights = Vec::with_capacity(expected);
        let mut terrain_sum = 0.0;
        let mut water_sum = 0.0;
        let mut terrain_count = 0u32;
        let mut water_count = 0u32;
        for y in 0..RADAR_CELL_HEIGHT {
            for x in 0..RADAR_CELL_WIDTH {
                let wx = self.map_extent.lo.x + x as f32 * self.x_sample;
                let wy = self.map_extent.lo.y + y as f32 * self.y_sample;
                let (z, is_water) = source.sample_cell(wx, wy).unwrap_or((0.0, false));
                if is_water {
                    water_sum += z;
                    water_count += 1;
                } else {
                    terrain_sum += z;
                    terrain_count += 1;
                }
                heights.push(super::RadarTerrainSample {
                    height: z,
                    is_water,
                });
            }
        }
        self.terrain_average_z = if terrain_count > 0 {
            terrain_sum / terrain_count as f32
        } else {
            0.0
        };
        self.water_average_z = if water_count > 0 {
            water_sum / water_count as f32
        } else {
            0.0
        };
        self.terrain_samples = heights;
        self.terrain_dirty = true;
        true
    }
}
