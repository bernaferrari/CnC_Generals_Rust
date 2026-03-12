/*
** Command & Conquer Generals Zero Hour(tm) - Terrain Data (Main runtime)
**
** Lightweight terrain representation used by the Rust main runtime for:
** - height queries (placing objects on ground)
** - coarse impassability for pathfinding (until full SAGE terrain decoding lands)
*/

use glam::Vec3;

#[cfg(feature = "game_client")]
use game_client::terrain::height_map::HeightMap;

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
}

impl TerrainData {
    #[cfg(feature = "game_client")]
    pub fn from_heightmap(
        heightmap: HeightMap,
        world_min: Vec3,
        world_max: Vec3,
        border_size: u32,
    ) -> Self {
        let width = heightmap.width.max(2) as f32;
        let height = heightmap.height.max(2) as f32;
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
        }
    }

    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        (self.world_min, self.world_max)
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
}
