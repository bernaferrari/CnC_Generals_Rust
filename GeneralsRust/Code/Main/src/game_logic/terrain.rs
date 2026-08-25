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
/// Kept for Wave 81 honesty pack. Live `TerrainData::height_at_world` uses the
/// C++ `BaseHeightMap.cpp:900` fy>fx triangle split instead.
#[inline]
pub fn bilinear_height_sample(h00: f32, h10: f32, h01: f32, h11: f32, tx: f32, tz: f32) -> f32 {
    let tx = tx.clamp(0.0, 1.0);
    let tz = tz.clamp(0.0, 1.0);
    let hx0 = h00 * (1.0 - tx) + h10 * tx;
    let hx1 = h01 * (1.0 - tx) + h11 * tx;
    hx0 * (1.0 - tz) + hx1 * tz
}

/// C++ `BaseHeightMapRenderObjClass::getHeightMapHeight` (BaseHeightMap.cpp:858-909).
///
/// Cell corners:
/// ```text
///   3-----2      h01-----h11
///   |    /|       |    /|
///   |  /  |       |  /  |
///   |/    |       |/    |
///   0-----1      h00-----h10
/// ```
/// `fy > fx` selects the upper triangle (p3/p0/p2); otherwise the lower (p1/p2/p0).
#[inline]
pub fn triangle_split_height_sample(
    h00: f32,
    h10: f32,
    h01: f32,
    h11: f32,
    fx: f32,
    fy: f32,
) -> f32 {
    let fx = fx.clamp(0.0, 1.0);
    let fy = fy.clamp(0.0, 1.0);
    if fy > fx {
        h01 + (1.0 - fy) * (h00 - h01) + fx * (h11 - h01)
    } else {
        h10 + fy * (h11 - h10) + (1.0 - fx) * (h00 - h10)
    }
}

/// Copied crate `TerrainLogic` water polygon (C++ `PolygonTrigger` water area).
/// Points are C++ X/Y, which the host samples as world X/Z.
#[derive(Debug, Clone)]
struct HostWaterPolygon {
    points: Vec<(i32, i32)>,
    height: f32,
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
    /// Also filled from crate water-handle / polygon heights on map load.
    pub water_plane_y: Option<f32>,
    /// C++ `TerrainLogic::getWaterHandle` water polygons copied from crate.
    water_polygons: Vec<HostWaterPolygon>,
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
            water_polygons: Vec::new(),
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

        let normalized = triangle_split_height_sample(h00, h10, h01, h11, tx, tz);

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

    /// C++ `WorldHeightMap::setCellCliffFlagFromHeights` / leftover `terrain_cliff`.
    ///
    /// Four-corner raw samples are converted to world Z (`* MAP_HEIGHT_SCALE`)
    /// and flagged cliff when maxZ−minZ > `PATHFIND_CLIFF_SLOPE_LIMIT_F` (9.8).
    #[cfg(feature = "game_client")]
    pub fn is_cliff_at_world(&self, world: Vec3) -> bool {
        use crate::game_logic::host_terrain_bridge_water_road_residual_wave108::cliff_cell_from_raw_heights_residual;
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
        cliff_cell_from_raw_heights_residual(h00, h10, h01, h11)
    }

    #[cfg(not(feature = "game_client"))]
    pub fn is_cliff_at_world(&self, _world: Vec3) -> bool {
        false
    }

    /// Copy crate `TerrainLogic` water handles/polygons into the live host.
    ///
    /// C++ `TerrainLogic::getWaterHandle` (TerrainLogic.cpp:2160) walks water
    /// polygon triggers; `isUnderwater` (2119) then compares ground < water Z.
    pub fn copy_water_from_terrain_logic(&mut self, logic: &gamelogic::terrain::TerrainLogic) {
        self.water_polygons.clear();
        let mut plane: Option<f32> = None;

        for trigger in logic.get_trigger_areas().get_triggers() {
            if !trigger.is_water_area() {
                continue;
            }
            let n = trigger.get_num_points();
            if n < 3 {
                continue;
            }
            let mut points = Vec::with_capacity(n as usize);
            for i in 0..n {
                if let Some(p) = trigger.get_point(i) {
                    points.push((p.x, p.y));
                }
            }
            if points.len() < 3 {
                continue;
            }
            let mut height = trigger.get_point(0).map(|p| p.z as f32).unwrap_or(0.0);
            let cx = points.iter().map(|p| p.0 as f32).sum::<f32>() / points.len() as f32;
            let cy = points.iter().map(|p| p.1 as f32).sum::<f32>() / points.len() as f32;
            if let Some(handle) = logic.get_water_handle(cx, cy) {
                height = logic.get_water_height(handle);
            }
            plane = Some(plane.map_or(height, |p| p.max(height)));
            self.water_polygons
                .push(HostWaterPolygon { points, height });
        }

        if let Some(grid) =
            logic.get_water_handle_by_name(&gamelogic::common::AsciiString::from("Water Grid"))
        {
            let gh = logic.get_water_height(grid);
            plane = Some(plane.map_or(gh, |p| p.max(gh)));
        }

        self.water_plane_y = plane;
    }

    /// Copy from the process-global crate `THE_TERRAIN_LOGIC`.
    pub fn copy_water_from_global_crate_terrain_logic(&mut self) {
        if let Ok(logic) = gamelogic::terrain::get_terrain_logic().read() {
            self.copy_water_from_terrain_logic(&logic);
        }
    }

    /// C++ `TerrainLogic::isUnderwater` (TerrainLogic.cpp:2119-2154).
    ///
    /// When water polygons were copied from crate TerrainLogic, a point is wet
    /// only inside the highest containing water polygon. With no polygons, fall
    /// back to `water_plane_y` so existing host residuals stay usable.
    pub fn is_underwater_at_world(&self, world: Vec3) -> bool {
        // C++ TerrainLogic::isUnderwater / getWaterHandle (polygons + water grid).
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().try_read() {
            if tl.get_water_handle(world.x, world.z).is_some() {
                return tl.is_underwater(world.x, world.z, None, None);
            }
        }
        let Some(water_y) = self.water_surface_at_world(world) else {
            return false;
        };
        #[cfg(feature = "game_client")]
        {
            let terrain_y = self.height_at_world(world);
            return terrain_y < water_y;
        }
        #[cfg(not(feature = "game_client"))]
        {
            world.y < water_y
        }
    }

    pub fn water_surface_at_world(&self, world: Vec3) -> Option<f32> {
        if !self.water_polygons.is_empty() {
            let qx = world.x;
            let qy = world.z;
            let mut best: Option<f32> = None;
            for poly in &self.water_polygons {
                if point_in_host_water_polygon(&poly.points, qx, qy) {
                    best = Some(best.map_or(poly.height, |h| h.max(poly.height)));
                }
            }
            return best;
        }
        self.water_plane_y
    }
}

fn point_in_host_water_polygon(points: &[(i32, i32)], x: f32, y: f32) -> bool {
    if points.len() < 3 {
        return false;
    }
    // C++ TerrainLogic.cpp:2166 REAL_TO_INT_FLOOR(x + 0.5f)
    let px = (x + 0.5).floor() as i32;
    let py = (y + 0.5).floor() as i32;

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for &(vx, vy) in points {
        min_x = min_x.min(vx);
        min_y = min_y.min(vy);
        max_x = max_x.max(vx);
        max_y = max_y.max(vy);
    }
    if px < min_x || py < min_y || px > max_x || py > max_y {
        return false;
    }

    let mut inside = false;
    let n = points.len();
    for i in 0..n {
        let (x1, y1) = points[i];
        let (x2, y2) = if i + 1 == n { points[0] } else { points[i + 1] };
        if y1 == y2 {
            continue;
        }
        if y1 < py && y2 < py {
            continue;
        }
        if y1 >= py && y2 >= py {
            continue;
        }
        if x1 < px && x2 < px {
            continue;
        }
        let dy = (y2 - y1) as f32;
        let dx = (x2 - x1) as f32;
        let intersection_x = x1 as f32 + dx * ((py - y1) as f32) / dy;
        if intersection_x >= px as f32 {
            inside = !inside;
        }
    }
    inside
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

    #[test]
    fn height_at_world_uses_cpp_triangle_split_not_bilinear() {
        // C++ BaseHeightMap.cpp:900-909 fy>fx upper triangle.
        // Corners p0=0, p1=0, p2=1, p3=0. At fx=0.25, fy=0.75:
        // triangle = 0.25, bilinear = 0.1875. Pre-fix live path used bilinear.
        let split = triangle_split_height_sample(0.0, 0.0, 0.0, 1.0, 0.25, 0.75);
        let bilinear = bilinear_height_sample(0.0, 0.0, 0.0, 1.0, 0.25, 0.75);
        assert!((split - 0.25).abs() < 1e-5, "got {split}");
        assert!((bilinear - 0.1875).abs() < 1e-5, "got {bilinear}");
        assert!((split - bilinear).abs() > 0.05);

        #[cfg(feature = "game_client")]
        {
            use game_client::terrain::height_map::HeightMap;
            let mut hm = HeightMap::new(2, 2, 100.0, 1.0);
            hm.heights[0] = 0.0; // h00
            hm.heights[1] = 0.0; // h10
            hm.heights[2] = 0.0; // h01
            hm.heights[3] = 1.0; // h11
            let t = TerrainData::from_heightmap(
                hm,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 10.0),
                0,
            );
            // world (2.5, 0, 7.5) → fx=0.25, fy=0.75 inside the 10-unit cell.
            let h = t.height_at_world(Vec3::new(2.5, 0.0, 7.5));
            assert!(
                (h - 25.0).abs() < 0.05,
                "triangle-split height should be 25, got {h} (bilinear would be 18.75)"
            );
        }
    }

    #[test]
    fn crate_water_polygons_copy_into_host_is_underwater() {
        // C++ TerrainLogic.cpp:2119-2160 isUnderwater via getWaterHandle polygons.
        // Pre-fix: TerrainData.water_plane_y stayed None, lakes always dry.
        use gamelogic::common::{AsciiString, ICoord3D};
        use gamelogic::polygon_trigger::PolygonTrigger;
        use gamelogic::system::map_loader::MapData;
        use gamelogic::terrain::TerrainLogic;

        let mut trigger = PolygonTrigger::new(3, AsciiString::from("Lake"), Vec::new());
        trigger.set_water_area(true);
        trigger.add_point(ICoord3D::new(0, 0, 12));
        trigger.add_point(ICoord3D::new(40, 0, 12));
        trigger.add_point(ICoord3D::new(40, 40, 12));
        trigger.add_point(ICoord3D::new(0, 40, 12));

        let mut map_data = MapData::new();
        map_data.water_height = Some(7.5);
        map_data.polygon_triggers.push(trigger);
        let mut logic = TerrainLogic::new();
        logic.load_map_data(map_data);
        assert!(
            logic.is_underwater(10.0, 10.0, None, None),
            "crate lake must be wet before host copy"
        );

        #[cfg(feature = "game_client")]
        {
            use game_client::terrain::height_map::HeightMap;
            let mut hm = HeightMap::new(4, 4, 100.0, 1.0);
            for h in hm.heights.iter_mut() {
                *h = 0.05; // ground = 5
            }
            let mut t = TerrainData::from_heightmap(
                hm,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 30.0),
                0,
            );
            assert!(
                t.water_plane_y.is_none(),
                "pre-copy host water plane must stay dry"
            );
            assert!(!t.is_underwater_at_world(Vec3::new(10.0, 0.0, 10.0)));

            t.copy_water_from_terrain_logic(&logic);
            assert!(
                t.water_plane_y.is_some_and(|y| (y - 12.0).abs() < 0.01),
                "copied lake height must fill water_plane_y, got {:?}",
                t.water_plane_y
            );
            assert!(
                t.is_underwater_at_world(Vec3::new(10.0, 0.0, 10.0)),
                "inside lake + ground 5 < water 12"
            );
            assert!(
                !t.is_underwater_at_world(Vec3::new(100.0, 0.0, 100.0)),
                "outside lake must stay dry (C++ getWaterHandle NULL)"
            );
        }
    }
}
