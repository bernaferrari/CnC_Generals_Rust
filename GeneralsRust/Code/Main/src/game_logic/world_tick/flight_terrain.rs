//! Borrowed driving-world terrain observations for live locomotor Z.
use crate::game_logic::game_logic::PathfindingHeightSamples;
use crate::game_logic::{Object, PathfindingGrid, terrain::TerrainData};
use glam::Vec3;

pub(crate) struct FlightTerrainView<'a> {
    pub(crate) grid: &'a PathfindingGrid,
    pub(crate) logical: Option<&'a std::sync::RwLock<gamelogic::terrain::TerrainLogic>>,
    pub(crate) terrain: Option<&'a TerrainData>,
    pub(crate) samples: Option<&'a PathfindingHeightSamples>,
}

impl FlightTerrainView<'_> {
    pub(crate) fn raw_ground(&self, pos: Vec3, fallback: f32) -> f32 {
        if let Some(owner) = self.logical {
            // One bounded observation: callbacks may change the same terrain
            // between queries, so no read guard crosses an object operation.
            let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
            if terrain.logic_height_map_extents() != (0, 0)
                || !terrain.logic_height_map_bytes().is_empty()
            {
                return terrain.get_ground_height(pos.x, pos.z, None);
            }
        }
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
        // TerrainData and coarse caches support explicit mapless fixtures.
        // A loaded receiving logical map is authoritative above. Bridge
        // overlays remain a separate highest_surface operation.
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

impl crate::game_logic::GameLogic {
    /// AIGroup::groupEvacuate chooses the highest surface below the current
    /// aircraft, independently of its current layer. The selected wall/bridge
    /// necessarily contains this point, so clip=true equals this height read.
    pub(crate) fn evacuation_surface_height(&self, pos: Vec3, fallback: f32) -> f32 {
        let view = FlightTerrainView {
            logical: Some(self.world_services.terrain().as_ref()),
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        let ground = view.raw_ground(pos, fallback);
        let layer = view.grid.highest_layer_at_or_below(pos, ground, false);
        view.grid.layer_height_unclipped(pos, layer, ground)
    }
}

#[cfg(test)]
#[path = "flight_terrain_tests.rs"]
mod tests;
