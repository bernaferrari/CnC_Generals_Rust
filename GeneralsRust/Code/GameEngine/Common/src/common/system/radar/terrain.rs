//! C++ `W3DRadar::buildTerrainTexture` (W3DRadar.cpp:997-1228).

use super::{
    Coord3D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, RadarSystem, interpolate_color_for_height,
};
use crate::common::ini::ini_water::get_water_transparency;
use std::sync::{Arc, OnceLock};

/// Intact bridge sampled at a radar cell.
#[derive(Debug, Clone, Copy)]
pub struct RadarBridgeSample {
    pub color: [f32; 3],
    pub height: f32,
}

/// TerrainVisual / Water / bridge lookup for one radar cell.
pub trait RadarTerrainPaintSource: Send + Sync {
    fn terrain_color_at(&self, world_x: f32, world_y: f32) -> Option<[f32; 3]>;
    fn bridge_at(&self, world: &Coord3D) -> Option<RadarBridgeSample>;
}

static PAINT_SOURCE: OnceLock<Arc<dyn RadarTerrainPaintSource>> = OnceLock::new();

pub fn register_radar_terrain_paint_source(source: Arc<dyn RadarTerrainPaintSource>) -> bool {
    PAINT_SOURCE.set(source).is_ok()
}

fn paint_source() -> Option<&'static dyn RadarTerrainPaintSource> {
    PAINT_SOURCE.get().map(|s| s.as_ref())
}

fn water_radar_color() -> [f32; 3] {
    get_water_transparency()
        .and_then(|wt| wt.read().ok().map(|g| g.radar_water_color))
        .map(|(r, g, b)| {
            // INI stores 0-255 style floats in this port.
            if r > 1.0 || g > 1.0 || b > 1.0 {
                [r / 255.0, g / 255.0, b / 255.0]
            } else {
                [r, g, b]
            }
        })
        .unwrap_or([140.0 / 255.0, 140.0 / 255.0, 255.0 / 255.0])
}

fn sample_world(radar: &RadarSystem, x: i32, y: i32) -> Option<Coord3D> {
    if x < 0 || y < 0 || x >= RADAR_CELL_WIDTH as i32 || y >= RADAR_CELL_HEIGHT as i32 {
        return None;
    }
    radar.radar_to_world(&super::ICoord2D { x, y })
}

fn average_color(samples: &[[f32; 3]]) -> [f32; 3] {
    if samples.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let n = samples.len() as f32;
    let mut acc = [0.0f32; 3];
    for s in samples {
        acc[0] += s[0];
        acc[1] += s[1];
        acc[2] += s[2];
    }
    [acc[0] / n, acc[1] / n, acc[2] / n]
}

impl RadarSystem {
    /// C++ `W3DRadar::buildTerrainTexture` software path.
    pub fn build_terrain_texture_cpp(&mut self) {
        let expected = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let hi_z = self.map_extent.hi.z;
        let lo_z = self.map_extent.lo.z;
        let mid_z = self.terrain_average_z;
        let water = water_radar_color();
        let source = paint_source();

        for y in 0..RADAR_CELL_HEIGHT as i32 {
            for x in 0..RADAR_CELL_WIDTH as i32 {
                let Some(world) = sample_world(self, x, y) else {
                    continue;
                };
                let cell = self
                    .terrain_samples
                    .get((y as u32 * RADAR_CELL_WIDTH + x as u32) as usize);
                let is_water = cell.map(|c| c.is_water).unwrap_or(false);
                let height = cell.map(|c| c.height).unwrap_or(world.z);
                let bridge = source.and_then(|s| s.bridge_at(&world));

                let color = if let Some(bridge) = bridge {
                    let mut neighborhood = Vec::new();
                    for j in (y - 1)..=(y + 1) {
                        for i in (x - 1)..=(x + 1) {
                            if sample_world(self, i, j).is_some() {
                                // C++ W3DRadar.cpp:1165-1167 call-site:
                                // interpolateColorForHeight(&color, bridgeHeight,
                                //     getTerrainAverageZ(), mapExtent.hi.z, mapExtent.lo.z)
                                neighborhood.push(interpolate_color_for_height(
                                    bridge.color,
                                    bridge.height,
                                    mid_z,
                                    hi_z,
                                    lo_z,
                                ));
                            }
                        }
                    }
                    average_color(&neighborhood)
                } else if is_water {
                    let mut neighborhood = Vec::new();
                    for j in (y - 1)..=(y + 1) {
                        for i in (x - 1)..=(x + 1) {
                            let Some(sample) = sample_world(self, i, j) else {
                                continue;
                            };
                            let sample_cell = self.terrain_samples.get(
                                (j.clamp(0, RADAR_CELL_HEIGHT as i32 - 1) as u32 * RADAR_CELL_WIDTH
                                    + i.clamp(0, RADAR_CELL_WIDTH as i32 - 1) as u32)
                                    as usize,
                            );
                            if sample_cell.is_some_and(|c| c.is_water) || sample_cell.is_none() {
                                let underwater_z =
                                    sample_cell.map(|c| c.height).unwrap_or(sample.z);
                                neighborhood.push(interpolate_color_for_height(
                                    water,
                                    underwater_z,
                                    self.water_average_z,
                                    self.water_average_z,
                                    lo_z,
                                ));
                            }
                        }
                    }
                    average_color(&neighborhood)
                } else {
                    let mut neighborhood = Vec::new();
                    for j in (y - 1)..=(y + 1) {
                        for i in (x - 1)..=(x + 1) {
                            let Some(sample) = sample_world(self, i, j) else {
                                continue;
                            };
                            let sample_h = self
                                .terrain_samples
                                .get(
                                    (j.clamp(0, RADAR_CELL_HEIGHT as i32 - 1) as u32
                                        * RADAR_CELL_WIDTH
                                        + i.clamp(0, RADAR_CELL_WIDTH as i32 - 1) as u32)
                                        as usize,
                                )
                                .map(|c| c.height)
                                .unwrap_or(sample.z);
                            let base = source
                                .and_then(|s| s.terrain_color_at(sample.x, sample.y))
                                .unwrap_or([0.45, 0.42, 0.32]);
                            // C++ W3DRadar.cpp:1177-1178 call-site:
                            // interpolateColorForHeight(&color, z, getTerrainAverageZ(),
                            //     mapExtent.hi.z, mapExtent.lo.z)
                            neighborhood.push(interpolate_color_for_height(
                                base, sample_h, mid_z, hi_z, lo_z,
                            ));
                        }
                    }
                    average_color(&neighborhood)
                };

                let base = ((y as u32 * RADAR_CELL_WIDTH + x as u32) * 4) as usize;
                if base + 3 < self.terrain_texture.len() {
                    self.terrain_texture[base] = (color[0] * 255.0).clamp(0.0, 255.0) as u8;
                    self.terrain_texture[base + 1] = (color[1] * 255.0).clamp(0.0, 255.0) as u8;
                    self.terrain_texture[base + 2] = (color[2] * 255.0).clamp(0.0, 255.0) as u8;
                    self.terrain_texture[base + 3] = 255;
                }
            }
        }

        let _ = expected;
        self.terrain_dirty = false;
        self.queue_terrain_refresh_frame = None;
    }
}
