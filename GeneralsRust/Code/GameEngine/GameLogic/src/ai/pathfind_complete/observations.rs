//! Immutable observations of the objects and terrain owned by a queued search.
use super::*;
use crate::object::Object;
use crate::object::object_factory::ObjectFactory;
use crate::player::PlayerList;
use crate::terrain::TerrainLogic;
use std::sync::{Arc, RwLock};

pub(crate) struct PathfindObservations<'a> {
    pub factory: &'a ObjectFactory,
    pub players: &'a PlayerList,
    pub terrain: &'a TerrainLogic,
}
impl PathfindObservations<'_> {
    pub(crate) fn admit<'a>(
        factory: &'a ObjectFactory,
        players: &'a PlayerList,
        terrain: &'a TerrainLogic,
    ) -> Result<PathfindObservations<'a>, String> {
        for id in factory.get_objects_by_type(|_| true) {
            let object = factory
                .get_object(id)
                .and_then(|object| object.get_base_object())
                .ok_or("admitted path object missing")?;
            let object = object.read().map_err(|_| "admitted path object poisoned")?;
            object.controlling_player_id_from_pin()?;
        }
        for player in players.iter() {
            let player = player.read().map_err(|_| "admitted path player poisoned")?;
            if let Some(team) = player.get_default_team() {
                drop(
                    team.read()
                        .map_err(|_| "admitted path default team poisoned")?,
                );
            }
        }
        Ok(PathfindObservations {
            factory,
            players,
            terrain,
        })
    }
    pub(crate) fn object(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.factory.get_object(id)?.get_base_object()
    }
    /// C++ AIPathfind.cpp:6369: only a computer controller is non-human.
    pub(crate) fn owner_is_human(&self, id: ObjectID) -> Result<bool, String> {
        let object = self.object(id).ok_or("path request owner missing")?;
        let controller = object
            .read()
            .map_err(|_| "path owner poisoned")?
            .controlling_player_id_from_pin()?;
        let Some(player) = controller.and_then(|id| self.players.get_player(id as i32)) else {
            return Ok(true);
        };
        Ok(player
            .read()
            .map_err(|_| "path controller poisoned")?
            .get_player_type()
            != crate::player::PlayerType::Computer)
    }
    pub(crate) fn relationship(&self, source: &Object, target: &Object) -> Relationship {
        source.relationship_to_with_players(target, self.players)
    }
    pub(crate) fn ignored_cells(&self, id: Option<ObjectID>) -> Option<HashSet<GridCoord>> {
        let object = self.object(id.filter(|id| *id != INVALID_ID)?)?;
        let object = object
            .read()
            .expect("admitted path observation object poisoned");
        let cells: HashSet<_> =
            crate::ai::object_footprint_positions_with_height(&object, |x, y| {
                self.terrain.get_layer_height(
                    x,
                    y,
                    crate::path::PathfindLayerEnum::Ground,
                    None,
                    false,
                )
            })?
            .into_iter()
            .map(|position| GridCoord::from_world(&position))
            .collect();
        (!cells.is_empty()).then_some(cells)
    }
}
