//! Borrowed height observations for one synchronous pathfinding operation.
//!
//! Loaded matches borrow their actual TerrainLogic. Detached sources are explicit
//! fixtures/old-save adapters; this value is never retained by the grid or search.
use super::*;

pub enum PathfindingGround<'a> {
    Receiving(&'a gamelogic::terrain::TerrainLogic),
    Detached(&'a super::super::terrain::TerrainData),
    Coarse {
        values: &'a [f32],
        width: i32,
        height: i32,
        origin: Vec3,
        cell_size: f32,
    },
    Flat(f32),
}

impl<'a> PathfindingGround<'a> {
    pub fn receiving(terrain: &'a gamelogic::terrain::TerrainLogic) -> Self {
        Self::Receiving(terrain)
    }

    /// Choose from explicitly borrowed inputs. A loaded receiving logical map
    /// always wins; an unadmitted world can use its declared fixture/old-save
    /// source. This selection performs no owner lookup or lock acquisition.
    pub(crate) fn for_terrain(
        logical: &'a gamelogic::terrain::TerrainLogic,
        map_loaded: bool,
        detached: Option<&'a super::super::terrain::TerrainData>,
        samples: Option<&'a super::super::game_logic::PathfindingHeightSamples>,
        grid: &PathfindingGrid,
    ) -> Self {
        if map_loaded || logical.has_height_map() {
            Self::Receiving(logical)
        } else {
            Self::mapless(detached, samples, grid)
        }
    }

    /// Explicit unadmitted-world adapter. Callers choose this only when their
    /// receiving logical owner has no loaded map, never after a failed borrow.
    /// Exact historical/coarse samples precede their optional normalized
    /// presentation geometry; loaded logical bytes were already selected above.
    pub(crate) fn mapless(
        terrain: Option<&'a super::super::terrain::TerrainData>,
        samples: Option<&'a super::super::game_logic::PathfindingHeightSamples>,
        grid: &PathfindingGrid,
    ) -> Self {
        if let Some(samples) = samples.filter(|samples| {
            samples.width == grid.width().max(0) as u32
                && samples.height == grid.height().max(0) as u32
        }) {
            return Self::Coarse {
                values: &samples.values,
                width: samples.width as i32,
                height: samples.height as i32,
                origin: grid.origin(),
                cell_size: grid.grid_size(),
            };
        }
        if let Some(terrain) = terrain {
            return Self::Detached(terrain);
        }
        Self::Flat(0.0)
    }

    /// The receiving owner's authored boundary table, not the active boundary
    /// or an invented map/grid rectangle. Explicit mapless profiles have none.
    pub(super) fn maximum_pathfind_extent(&self) -> Option<gamelogic::common::Region3D> {
        match self {
            Self::Receiving(terrain) => Some(terrain.get_maximum_pathfind_extent()),
            Self::Detached(_) | Self::Coarse { .. } | Self::Flat(_) => None,
        }
    }

    pub fn height_at(&self, position: Vec3) -> f32 {
        match self {
            Self::Receiving(terrain) => terrain.get_ground_height(position.x, position.z, None),
            Self::Detached(terrain) => terrain.logic_height_at_world(position),
            Self::Coarse {
                values,
                width,
                height,
                origin,
                cell_size,
            } => {
                let x = ((position.x - origin.x) / cell_size) as i32;
                let y = ((position.z - origin.z) / cell_size) as i32;
                if x < 0 || y < 0 || x >= *width || y >= *height {
                    return 0.0;
                }
                values
                    .get((y as usize) * (*width as usize) + x as usize)
                    .copied()
                    .unwrap_or(0.0)
            }
            Self::Flat(height) => *height,
        }
    }
}
