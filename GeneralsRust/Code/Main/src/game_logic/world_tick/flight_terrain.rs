//! Borrowed driving-world terrain observations for live locomotor Z.
use crate::game_logic::game_logic::PathfindingHeightSamples;
use crate::game_logic::{Object, PathfindingGrid, terrain::TerrainData};
use glam::Vec3;

pub(super) struct FlightTerrainView<'a> {
    pub(super) grid: &'a PathfindingGrid,
    pub(super) terrain: Option<&'a TerrainData>,
    pub(super) samples: Option<&'a PathfindingHeightSamples>,
}

impl FlightTerrainView<'_> {
    pub(super) fn raw_ground(&self, pos: Vec3, fallback: f32) -> f32 {
        if let Some(terrain) = self.terrain {
            return terrain.logic_height_at_world(pos);
        }
        if let Some(cache) = self.samples {
            let width = self.grid.width().max(0) as u32;
            let height = self.grid.height().max(0) as u32;
            let cell = self.grid.world_to_grid(pos);
            if cache.width == width
                && cache.height == height
                && cell.x >= 0
                && cell.y >= 0
                && cell.x < width as i32
                && cell.y < height as i32
            {
                if let Some(y) = cache
                    .values
                    .get((cell.y as u32 * width + cell.x as u32) as usize)
                {
                    return *y;
                }
            }
        }
        // Mapless objects can carry an explicit owned ground observation.
        // Loaded maps admit TerrainData (native and headless); snapshots may
        // instead provide the validated height cache above. No Core terrain
        // or bridge overlay is used as a substitute for raw map heights.
        fallback
    }

    pub(super) fn highest_surface(&self, obj: &Object) -> f32 {
        let pos = obj.get_position();
        let ground = self.raw_ground(pos, obj.ground_height);
        let layer = if obj.pathfind_layer == 1 {
            self.grid.highest_layer_at_or_below(pos, ground, false)
        } else {
            obj.pathfind_layer
        };
        self.grid.layer_height_unclipped(pos, layer, ground)
    }
}

#[cfg(test)]
#[path = "flight_terrain_tests.rs"]
mod tests;
