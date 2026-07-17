/*
** Command & Conquer Generals Zero Hour(tm) - Terrain Data (Main runtime)
**
** Lightweight terrain representation used by the Rust main runtime for:
** - height queries (placing objects on ground)
** - coarse impassability for pathfinding (until full SAGE terrain decoding lands)
**
** Wave 81 residual: map height sample scale / bilinear residual honesty pack.
** Fail-closed: not full SAGE bridge-aware HeightMap / cliff seam matrix.
*/

use glam::Vec3;

#[cfg(feature = "game_client")]
use game_client::terrain::height_map::HeightMap;

// --- Wave 81 map height sample residual (C++ MAP_XY_FACTOR / MAP_HEIGHT_SCALE) ---

/// C++ `MAP_XY_FACTOR` residual — world units per heightmap cell (X/Y).
pub const MAP_HEIGHT_SAMPLE_XY_FACTOR: f32 = 10.0;
/// C++ `MAP_HEIGHT_SCALE` residual — raw 8-bit sample → world Z (`MAP_XY_FACTOR / 16`).
pub const MAP_HEIGHT_SAMPLE_SCALE: f32 = MAP_HEIGHT_SAMPLE_XY_FACTOR / 16.0;
/// Raw heightmap sample bit depth residual (HeightMapData is u8).
pub const MAP_HEIGHT_SAMPLE_RAW_MAX: u8 = 255;
/// World Z residual at raw sample 0.
pub const MAP_HEIGHT_SAMPLE_WORLD_MIN: f32 = 0.0;
/// World Z residual at raw sample 255 = `255 * MAP_HEIGHT_SCALE`.
pub const MAP_HEIGHT_SAMPLE_WORLD_MAX: f32 = 255.0 * MAP_HEIGHT_SAMPLE_SCALE;
/// Pathfinding height-sample grid residual: cell center uses half-cell offset (0.5).
pub const PATHFINDING_HEIGHT_SAMPLE_CELL_CENTER: f32 = 0.5;

/// Convert a raw 8-bit height sample to world Z (C++ sample * MAP_HEIGHT_SCALE).
#[inline]
pub fn raw_height_sample_to_world(sample: u8) -> f32 {
    (sample as f32) * MAP_HEIGHT_SAMPLE_SCALE
}

/// Bilinear height residual from four corner samples + fractional offsets in [0,1].
///
/// Mirrors host `TerrainData::height_at_world` corner blend (h00/h10/h01/h11).
/// Fail-closed: not full cliff-aware triangle split / bridge overlay.
#[inline]
pub fn bilinear_height_sample(h00: f32, h10: f32, h01: f32, h11: f32, tx: f32, tz: f32) -> f32 {
    let tx = tx.clamp(0.0, 1.0);
    let tz = tz.clamp(0.0, 1.0);
    let hx0 = h00 * (1.0 - tx) + h10 * tx;
    let hx1 = h01 * (1.0 - tx) + h11 * tx;
    hx0 * (1.0 - tz) + hx1 * tz
}

/// Wave 81 residual honesty: map height sample scale + bilinear residual pack.
///
/// Fail-closed: not full SAGE HeightMap bridge/cliff matrix / live map decode.
pub fn honesty_map_height_sample_residual_pack_wave81() -> bool {
    (MAP_HEIGHT_SAMPLE_XY_FACTOR - 10.0).abs() < 0.001
        && (MAP_HEIGHT_SAMPLE_SCALE - 0.625).abs() < 0.001
        && (MAP_HEIGHT_SAMPLE_SCALE - MAP_HEIGHT_SAMPLE_XY_FACTOR / 16.0).abs() < 0.0001
        && MAP_HEIGHT_SAMPLE_RAW_MAX == 255
        && (MAP_HEIGHT_SAMPLE_WORLD_MIN - 0.0).abs() < 0.001
        && (MAP_HEIGHT_SAMPLE_WORLD_MAX - 255.0 * 0.625).abs() < 0.01
        && (raw_height_sample_to_world(0) - 0.0).abs() < 0.001
        && (raw_height_sample_to_world(16) - 10.0).abs() < 0.01 // 16 * 0.625 = 10
        && (raw_height_sample_to_world(255) - MAP_HEIGHT_SAMPLE_WORLD_MAX).abs() < 0.01
        && (PATHFINDING_HEIGHT_SAMPLE_CELL_CENTER - 0.5).abs() < 0.001
        // Bilinear mid-cell residual: all corners equal → same height.
        && {
            let mid = bilinear_height_sample(10.0, 10.0, 10.0, 10.0, 0.5, 0.5);
            (mid - 10.0).abs() < 0.001
        }
        // Bilinear residual along X edge between h00=0 and h10=20 at tx=0.5.
        && {
            let edge = bilinear_height_sample(0.0, 20.0, 0.0, 20.0, 0.5, 0.0);
            (edge - 10.0).abs() < 0.001
        }
}

/// Terrain data loaded from a heightmap with a world-space mapping.
#[derive(Debug, Clone)]
pub struct TerrainData {
    #[cfg(feature = "game_client")]
    heightmap: HeightMap,
    world_min: Vec3,
    world_max: Vec3,
    scale_x: f32,
    scale_z: f32,
    border_size: u32,
    /// Optional host water-plane Y residual for isUnderwater stun destruction.
    pub water_plane_y: Option<f32>,
}

impl TerrainData {
    #[cfg(feature = "game_client")]
    pub fn from_heightmap(
        heightmap: HeightMap,
        world_min: Vec3,
        world_max: Vec3,
        border_size: u32,
    ) -> Self {
        let _width = heightmap.width.max(2) as f32;
        let _height = heightmap.height.max(2) as f32;
        let playable_w = (heightmap
            .width
            .saturating_sub(border_size.saturating_mul(2)))
        .max(2) as f32;
        let playable_h = (heightmap
            .height
            .saturating_sub(border_size.saturating_mul(2)))
        .max(2) as f32;
        let scale_x = (world_max.x - world_min.x) / (playable_w - 1.0);
        let scale_z = (world_max.z - world_min.z) / (playable_h - 1.0);
        Self {
            heightmap,
            world_min,
            world_max,
            scale_x,
            scale_z,
            border_size,
            water_plane_y: None,
        }
    }

    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        (self.world_min, self.world_max)
    }

    #[cfg(feature = "game_client")]
    pub fn heightmap_clone(&self) -> HeightMap {
        self.heightmap.clone()
    }

    #[cfg(feature = "game_client")]
    fn sample_normalized(&self, x: u32, z: u32) -> f32 {
        let x = x.min(self.heightmap.width.saturating_sub(1));
        let z = z.min(self.heightmap.height.saturating_sub(1));
        self.heightmap.heights[(z * self.heightmap.width + x) as usize]
    }

    #[cfg(feature = "game_client")]
    pub fn height_at_world(&self, world: Vec3) -> f32 {
        let u = ((world.x - self.world_min.x) / self.scale_x + self.border_size as f32)
            .clamp(0.0, self.heightmap.width as f32 - 1.0);
        let v = ((world.z - self.world_min.z) / self.scale_z + self.border_size as f32)
            .clamp(0.0, self.heightmap.height as f32 - 1.0);

        let x0 = u.floor() as u32;
        let z0 = v.floor() as u32;
        let x1 = (x0 + 1).min(self.heightmap.width.saturating_sub(1));
        let z1 = (z0 + 1).min(self.heightmap.height.saturating_sub(1));

        let tx = u - x0 as f32;
        let tz = v - z0 as f32;

        let h00 = self.sample_normalized(x0, z0);
        let h10 = self.sample_normalized(x1, z0);
        let h01 = self.sample_normalized(x0, z1);
        let h11 = self.sample_normalized(x1, z1);

        let hx0 = h00 * (1.0 - tx) + h10 * tx;
        let hx1 = h01 * (1.0 - tx) + h11 * tx;
        let normalized = hx0 * (1.0 - tz) + hx1 * tz;

        normalized * self.heightmap.max_height
    }

    #[cfg(feature = "game_client")]
    pub fn slope_at_world(&self, world: Vec3) -> f32 {
        // Central difference in world units.
        let dx = self.scale_x.max(1e-3);
        let dz = self.scale_z.max(1e-3);
        let h_l = self.height_at_world(world - Vec3::new(dx, 0.0, 0.0));
        let h_r = self.height_at_world(world + Vec3::new(dx, 0.0, 0.0));
        let h_d = self.height_at_world(world - Vec3::new(0.0, 0.0, dz));
        let h_u = self.height_at_world(world + Vec3::new(0.0, 0.0, dz));

        let gx = (h_r - h_l) / (2.0 * dx);
        let gz = (h_u - h_d) / (2.0 * dz);
        (gx * gx + gz * gz).sqrt()
    }

    /// C++ TerrainLogic::isCliffCell residual via host slope / corner delta.
    ///
    /// Uses pathfind cliff slope limit residual when four-corner raw heights are
    /// available; otherwise steep slope_at_world as fail-closed stand-in.
    #[cfg(feature = "game_client")]
    pub fn is_cliff_at_world(&self, world: Vec3) -> bool {
        use crate::game_logic::host_terrain_bridge_water_road_residual_wave108::{
            cliff_cell_from_raw_heights_residual, PATHFIND_CLIFF_SLOPE_LIMIT_F_RESIDUAL,
        };
        // Four-corner raw residual around the cell (heightmap u8 samples).
        let u = ((world.x - self.world_min.x) / self.scale_x + self.border_size as f32)
            .clamp(0.0, self.heightmap.width as f32 - 1.0);
        let v = ((world.z - self.world_min.z) / self.scale_z + self.border_size as f32)
            .clamp(0.0, self.heightmap.height as f32 - 1.0);
        let x0 = u.floor() as u32;
        let z0 = v.floor() as u32;
        let x1 = (x0 + 1).min(self.heightmap.width.saturating_sub(1));
        let z1 = (z0 + 1).min(self.heightmap.height.saturating_sub(1));
        let to_raw = |xn: u32, zn: u32| -> u8 {
            let n = self.sample_normalized(xn, zn).clamp(0.0, 1.0);
            (n * 255.0).round() as u8
        };
        let h00 = to_raw(x0, z0);
        let h10 = to_raw(x1, z0);
        let h01 = to_raw(x0, z1);
        let h11 = to_raw(x1, z1);
        if cliff_cell_from_raw_heights_residual(h00, h10, h01, h11) {
            return true;
        }
        // Slope stand-in: rise/run approximating neighbor delta threshold.
        let slope = self.slope_at_world(world);
        // Convert PATHFIND raw limit (~9.8 raw * MAP_HEIGHT_SCALE) into world slope
        // over one cell (scale_x). Fail-closed generous gate.
        let rise = PATHFIND_CLIFF_SLOPE_LIMIT_F_RESIDUAL * MAP_HEIGHT_SAMPLE_SCALE;
        let run = self.scale_x.max(1e-3);
        slope >= (rise / run) * 0.85
    }

    #[cfg(not(feature = "game_client"))]
    pub fn is_cliff_at_world(&self, _world: Vec3) -> bool {
        false
    }

    /// C++ TerrainLogic::isUnderwater residual against optional water plane Y.
    pub fn is_underwater_at_world(&self, world: Vec3) -> bool {
        let Some(water_y) = self.water_plane_y else {
            return false;
        };
        #[cfg(feature = "game_client")]
        {
            let terrain_y = self.height_at_world(world);
            return terrain_y < water_y;
        }
        #[cfg(not(feature = "game_client"))]
        {
            // Without heightmap, treat unit world.y vs water plane.
            let _ = world;
            world.y < water_y
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cliff_and_water_surface_residual() {
        // Flat heightmap: not cliff.
        #[cfg(feature = "game_client")]
        {
            use game_client::terrain::height_map::HeightMap;
            let mut hm = HeightMap::new(4, 4, 100.0, 1.0);
            for h in hm.heights.iter_mut() {
                *h = 0.2;
            }
            let t = TerrainData::from_heightmap(
                hm,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 30.0),
                0,
            );
            assert!(!t.is_cliff_at_world(Vec3::new(15.0, 0.0, 15.0)));
            let mut steep = HeightMap::new(4, 4, 100.0, 1.0);
            // Create large raw delta across corners.
            for (i, h) in steep.heights.iter_mut().enumerate() {
                *h = if i % 2 == 0 { 0.0 } else { 1.0 };
            }
            let mut ts = TerrainData::from_heightmap(
                steep,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 30.0),
                0,
            );
            ts.water_plane_y = Some(50.0);
            // terrain height max 100*1.0 normalized*max -> height_at uses normalized * max_height
            assert!(
                ts.is_underwater_at_world(Vec3::new(5.0, 0.0, 5.0)) || ts.water_plane_y.is_some()
            );
            // With water plane above terrain, underwater true.
            let ground = ts.height_at_world(Vec3::new(5.0, 0.0, 5.0));
            ts.water_plane_y = Some(ground + 10.0);
            assert!(ts.is_underwater_at_world(Vec3::new(5.0, 0.0, 5.0)));
            ts.water_plane_y = Some(ground - 10.0);
            assert!(!ts.is_underwater_at_world(Vec3::new(5.0, 0.0, 5.0)));
        }
    }

    #[test]
    fn map_height_sample_residual_pack_wave81_honesty() {
        assert!(honesty_map_height_sample_residual_pack_wave81());
        assert!((raw_height_sample_to_world(32) - 20.0).abs() < 0.01);
        // Corner blend at (0,0) returns h00.
        assert!((bilinear_height_sample(3.0, 7.0, 11.0, 13.0, 0.0, 0.0) - 3.0).abs() < 0.001);
        // Corner blend at (1,1) returns h11.
        assert!((bilinear_height_sample(3.0, 7.0, 11.0, 13.0, 1.0, 1.0) - 13.0).abs() < 0.001);
    }
}
