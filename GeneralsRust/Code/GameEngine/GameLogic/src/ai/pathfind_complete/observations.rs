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
/// One synchronous owned pathfinder drain. Its source objects are borrowed
/// for the entire phase, and searches keep reading live observations.
pub struct QueuedPathObservations<'a> {
    pub(crate) world: PathfindObservations<'a>,
    admitted: bool,
}

impl<'a> QueuedPathObservations<'a> {
    pub(crate) fn new(
        factory: &'a ObjectFactory,
        players: &'a PlayerList,
        terrain: &'a TerrainLogic,
    ) -> Self {
        Self {
            world: PathfindObservations {
                factory,
                players,
                terrain,
            },
            admitted: false,
        }
    }

    /// A canceled request does not reach admission. On the first supported
    /// request validate the roster once, before any queue callback mutates AI.
    pub(crate) fn admit(&mut self) -> Result<&PathfindObservations<'a>, String> {
        if !self.admitted {
            self.world.validate()?;
            self.admitted = true;
        }
        Ok(&self.world)
    }
}

impl PathfindObservations<'_> {
    fn validate(&self) -> Result<(), String> {
        for instance in self.factory.instances() {
            let object = instance
                .get_base_object()
                .ok_or("admitted path object missing")?;
            let object = object.read().map_err(|_| "admitted path object poisoned")?;
            object.controlling_player_id_from_pin()?;
        }
        for player in self.players.iter() {
            let player = player.read().map_err(|_| "admitted path player poisoned")?;
            if let Some(team) = player.get_default_team() {
                drop(
                    team.read()
                        .map_err(|_| "admitted path default team poisoned")?,
                );
            }
        }
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_is_shared_within_one_queue_drain_and_rechecked_on_the_next() {
        let factory = ObjectFactory::new();
        let terrain = TerrainLogic::new();
        let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
        let mut players = PlayerList::new();
        players.add_player(Arc::clone(&player));

        let mut batch = QueuedPathObservations::new(&factory, &players, &terrain);
        batch.admit().unwrap();
        let rejected = std::panic::catch_unwind(|| {
            let _held = player.write().unwrap();
            panic!("deliberately poison the validation-only roster entry");
        });
        assert!(rejected.is_err());

        batch
            .admit()
            .expect("a second request reuses this drain's preflight");
        let mut next = QueuedPathObservations::new(&factory, &players, &terrain);
        assert_eq!(
            next.admit().err().as_deref(),
            Some("admitted path player poisoned")
        );
    }
}
