//! Production `Radar::newMap` ingest when GameLogic/TerrainLogic load a map.

use super::{Coord3D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, RadarSystem};
use std::sync::{Arc, OnceLock};

/// Terrain sample used to seed `m_xSample` / averages / per-cell heights.
pub trait RadarMapSource: Send + Sync {
    fn map_extent(&self) -> Option<(Coord3D, Coord3D)>;
    /// `(height, is_water)` for one radar cell, or `None` if unmapped.
    fn sample_cell(&self, world_x: f32, world_y: f32) -> Option<(f32, bool)>;

    /// Row-major 128x128 samples, using the same coordinates and unmapped
    /// fallback as scalar sampling. Shared sources may borrow once per grid.
    fn sample_grid(&self, origin: Coord3D, x_sample: f32, y_sample: f32) -> Vec<(f32, f32, bool)> {
        sample_radar_map_grid(origin, x_sample, y_sample, |x, y| self.sample_cell(x, y))
    }
}

pub fn sample_radar_map_grid(
    origin: Coord3D,
    x_sample: f32,
    y_sample: f32,
    mut sample: impl FnMut(f32, f32) -> Option<(f32, bool)>,
) -> Vec<(f32, f32, bool)> {
    let mut heights = Vec::with_capacity((RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize);
    for y in 0..RADAR_CELL_HEIGHT {
        for x in 0..RADAR_CELL_WIDTH {
            let wx = origin.x + x as f32 * x_sample;
            let wy = origin.y + y as f32 * y_sample;
            let (z, is_water) = sample(wx, wy).unwrap_or((0.0, false));
            heights.push((wx, z, is_water));
        }
    }
    heights
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
        let heights = source.sample_grid(min, x_sample, y_sample);
        self.new_map(min, max, &heights);
        true
    }

    /// Re-sample every radar cell from the registered source without `reset`.
    /// Used by `refreshTerrain` so bridge/water changes update the texture.
    pub(crate) fn resample_terrain_from_source(&mut self) -> bool {
        let Some(source) = radar_map_source() else {
            return false;
        };
        self.resample_terrain_with_source(source)
    }

    pub(super) fn resample_terrain_with_source(&mut self, source: &dyn RadarMapSource) -> bool {
        if !self.has_map_extent() {
            return false;
        }
        // C++ refreshTerrain repaints using the averages established by newMap.
        self.terrain_samples = source
            .sample_grid(self.map_extent.lo, self.x_sample, self.y_sample)
            .into_iter()
            .map(|(_, height, is_water)| super::RadarTerrainSample { height, is_water })
            .collect();
        self.terrain_dirty = true;
        true
    }
}
