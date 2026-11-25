//! PathfindLayer's authored bridge cells and ground connections (AIPathfind.cpp).

use super::{
    AStarPathfinder, BridgeLayer, GridCoord, PathfindCellType, PathfindLayerEnum, PathfindingSystem,
};
use crate::common::{Coord2D, Coord3D, Region2D};
use crate::path::{LAYER_Z_CLOSE_ENOUGH_F, PATHFIND_CELL_SIZE_F};
use crate::terrain::{Bridge, TerrainLogic};

impl PathfindingSystem {
    pub(super) fn available_bridge_layer(&self) -> Option<u32> {
        ((PathfindLayerEnum::Ground as u32 + 1)..PathfindLayerEnum::Wall as u32)
            .find(|id| !self.bridges.iter().any(|bridge| bridge.layer_id == *id))
    }

    pub(crate) fn add_terrain_bridge(&mut self, bridge: &Bridge, terrain: &TerrainLogic) -> u32 {
        let Some(id) = self.available_bridge_layer() else {
            return PathfindLayerEnum::Ground as u32;
        };
        let info = bridge.get_bridge_info();
        let direction = info.to - info.from;
        let length = direction.length();
        let offset = if length > 0.0 {
            direction / length * (PATHFIND_CELL_SIZE_F * 0.7)
        } else {
            Coord3D::ZERO
        };
        let mut layer = BridgeLayer::with_geometry(
            id,
            layer_bounds(bridge, self.width, self.height),
            info.bridge_object_id,
            GridCoord::from_world(&(info.from - offset)),
            GridCoord::from_world(&(info.to + offset)),
        );
        if self.width > 0 && self.height > 0 {
            layer.classify_deck(bridge, terrain, &mut self.pathfinder);
            sync_bridge(&layer, &mut self.pathfinder);
            self.zones.mark_zones_dirty(true);
        }
        self.bridges.push(layer);
        id
    }

    /// Core allocates the ground grid after admitting the terrain bridges.
    /// Reclassification reads the current ground from this same terrain.
    pub(crate) fn classify_terrain_bridges(&mut self, terrain: &TerrainLogic) {
        for layer in self.bridges.iter_mut().filter(|layer| layer.from_terrain) {
            let bridge =
                std::iter::successors(terrain.get_first_bridge(), |bridge| bridge.get_next())
                    .find(|bridge| bridge.get_layer() as u32 == layer.layer_id);
            if let Some(bridge) = bridge {
                layer.bounds = layer_bounds(bridge, self.width, self.height);
                layer.classify_deck(bridge, terrain, &mut self.pathfinder);
            }
        }
    }

    pub fn set_bridge_destroyed(&mut self, layer_id: u32, destroyed: bool) {
        self.update_bridge(layer_id, destroyed, None);
    }

    pub(crate) fn set_terrain_bridge_destroyed(
        &mut self,
        layer_id: u32,
        destroyed: bool,
        bridge: &Bridge,
        terrain: &TerrainLogic,
    ) {
        self.update_bridge(layer_id, destroyed, Some((bridge, terrain)));
    }

    fn update_bridge(
        &mut self,
        layer_id: u32,
        destroyed: bool,
        geometry: Option<(&Bridge, &TerrainLogic)>,
    ) {
        let Some(layer) = self
            .bridges
            .iter_mut()
            .find(|layer| layer.layer_id == layer_id)
        else {
            return;
        };
        if layer.destroyed == destroyed {
            return;
        }

        layer.destroyed = destroyed;
        if !layer.from_terrain {
            layer.reclassify_cells();
        } else if let Some((bridge, terrain)) = geometry {
            layer.classify_deck(bridge, terrain, &mut self.pathfinder);
        }
        sync_bridge(layer, &mut self.pathfinder);
        self.zones.mark_zones_dirty(!destroyed);
    }

    pub(crate) fn sync_bridge_layers_to_pathfinder(&mut self) {
        for bridge in &self.bridges {
            sync_bridge(bridge, &mut self.pathfinder);
        }
    }
}

impl BridgeLayer {
    fn classify_deck(
        &mut self,
        bridge: &Bridge,
        terrain: &TerrainLogic,
        ground: &mut AStarPathfinder,
    ) {
        self.cell_types.clear();
        self.ground_connect_cells.clear();
        let (lo, hi) = self.bounds;
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                let coord = GridCoord::new(x, y);
                let left = x as f32 * PATHFIND_CELL_SIZE_F;
                let top = y as f32 * PATHFIND_CELL_SIZE_F;
                let right = left + PATHFIND_CELL_SIZE_F;
                let bottom = top + PATHFIND_CELL_SIZE_F;
                let tile = Region2D::new(Coord2D::new(left, top), Coord2D::new(right, bottom));
                let corners = [
                    Coord3D::new(left, top, 0.0),
                    Coord3D::new(left, bottom, 0.0),
                    Coord3D::new(right, bottom, 0.0),
                    Coord3D::new(right, top, 0.0),
                ]
                .into_iter()
                .filter(|point| bridge.is_point_on_bridge(point))
                .count();

                // AIPathfind.cpp:3676–3727. Full deck cells take priority over
                // the side/end tests. Only the entry line connects to ground.
                let mut entry = false;
                let cell_type = if corners == 4 {
                    PathfindCellType::Clear
                } else if bridge.is_cell_on_side(&tile) {
                    PathfindCellType::BridgeImpassable
                } else {
                    entry = bridge.is_cell_entry_point(&tile);
                    if entry || bridge.is_cell_on_end(&tile) {
                        PathfindCellType::Clear
                    } else if corners != 0 {
                        PathfindCellType::BridgeImpassable
                    } else {
                        continue;
                    }
                };
                self.cell_types.insert(coord, cell_type);
                if entry {
                    self.ground_connect_cells.push(coord);
                    continue;
                }
                let center = Coord3D::new(
                    left + PATHFIND_CELL_SIZE_F * 0.5,
                    top + PATHFIND_CELL_SIZE_F * 0.5,
                    0.0,
                );
                if ground.get_cell_type(coord) != Some(PathfindCellType::Obstacle)
                    && terrain.get_ground_height(center.x, center.y, None) + LAYER_Z_CLOSE_ENOUGH_F
                        > bridge.get_bridge_height(&center, None)
                {
                    ground.set_cell_type(coord, PathfindCellType::BridgeImpassable);
                }
            }
        }
    }
}

/// AIPathfind.cpp:3366–3402 pads the deck by one cell, then clips it.
/// The clipped upper edge is exclusive in C++ (width = maxX - m_xOrigin).
fn layer_bounds(bridge: &Bridge, width: usize, height: usize) -> (GridCoord, GridCoord) {
    let bounds = bridge.get_bounds();
    let lower = |value: f32| {
        ((value / PATHFIND_CELL_SIZE_F).floor() as i32)
            .saturating_sub(1)
            .max(0)
    };
    let upper = |value: f32, size: usize| {
        ((value / PATHFIND_CELL_SIZE_F).ceil() as i32)
            .min(i32::try_from(size).unwrap_or(i32::MAX) - 2)
    };
    (
        GridCoord::new(lower(bounds.lo.x), lower(bounds.lo.y)),
        GridCoord::new(upper(bounds.hi.x, width), upper(bounds.hi.y, height)),
    )
}

fn sync_bridge(bridge: &BridgeLayer, pathfinder: &mut AStarPathfinder) {
    let layer = PathfindLayerEnum::from_u32(bridge.layer_id);
    let (lo, hi) = bridge.bounds;
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            let coord = GridCoord::new(x, y);
            let kind = bridge
                .cell_type_at(coord)
                .unwrap_or(PathfindCellType::Impassable);
            if bridge.from_terrain {
                // AIPathfind.cpp:3675 calls PathfindCell::reset before stamping
                // the authored deck. Ground occupancy is independent.
                pathfinder.reclassify_cell_on_layer(coord, layer, kind);
            } else {
                pathfinder.set_cell_type_on_layer(coord, layer, kind);
            }
        }
    }
    for &coord in &bridge.ground_connect_cells {
        if bridge.destroyed
            || bridge
                .cell_type_at(coord)
                .is_none_or(|ty| ty == PathfindCellType::Impassable)
        {
            if pathfinder.get_cell_connect_layer(coord) == Some(layer) {
                pathfinder.set_cell_connect_layer(coord, PathfindLayerEnum::Invalid);
            }
            // C++ keeps the authored deck-side entry for connectsZones.
            let connect = if bridge.destroyed && bridge.from_terrain {
                PathfindLayerEnum::Ground
            } else {
                PathfindLayerEnum::Invalid
            };
            pathfinder.set_cell_connect_layer_on_layer(coord, layer, connect);
        } else {
            pathfinder.set_cell_connect_layer(coord, layer);
            pathfinder.set_cell_connect_layer_on_layer(coord, layer, PathfindLayerEnum::Ground);
        }
    }
}
