//! C++ TeamFactory::teamAboutToBeDeleted followed by Team::~Team.
//! Main owns the objects and controllers; Core supplies only team metadata.

use super::*;
use generals_sim::scripting::interpreter::ScriptInterpreter;

pub(super) type TeamDestroyObserver<'a> = dyn FnMut(&mut AIPlayer, &mut GameLogic, u32, &str) + 'a;

/// Direct AIPlayer callers notify the installed peers and the borrowed actor.
/// The frame manager supplies its actual borrowed peers instead.
pub(super) fn notify_installed_team_destroy(
    current: &mut AIPlayer,
    world: &mut GameLogic,
    id: u32,
    name: &str,
) {
    for (_, peer) in world.ai_manager.ai_players.range_mut(..current.player_id) {
        peer.ai_pre_team_destroy(Some(id), name);
    }
    current.ai_pre_team_destroy(Some(id), name);
    for (_, peer) in world.ai_manager.ai_players.range_mut((
        std::ops::Bound::Excluded(current.player_id),
        std::ops::Bound::Unbounded,
    )) {
        peer.ai_pre_team_destroy(Some(id), name);
    }
}

impl GameLogic {
    /// Keep the team visible until every synchronous destruction callback has
    /// completed. No Core player/controller/object lookup is permitted here.
    pub(super) fn delete_ai_team_owned(
        &mut self,
        actor: &mut AIPlayer,
        id: u32,
        notify: &mut TeamDestroyObserver<'_>,
    ) -> bool {
        self.delete_ai_team_with_observer(id, None, |world, name| notify(actor, world, id, name))
    }

    pub(crate) fn delete_canonical_ai_team(&mut self, id: u32) -> bool {
        self.delete_canonical_ai_team_with_scripts(id, None)
    }

    pub(crate) fn delete_canonical_ai_team_with_interpreter(
        &mut self,
        id: u32,
        interpreter: &ScriptInterpreter,
    ) -> bool {
        self.delete_canonical_ai_team_with_scripts(id, Some(interpreter))
    }

    fn delete_canonical_ai_team_with_scripts(
        &mut self,
        id: u32,
        interpreter: Option<&ScriptInterpreter>,
    ) -> bool {
        self.delete_ai_team_with_observer(id, interpreter, |world, name| {
            for ai in world.ai_manager.ai_players.values_mut() {
                ai.ai_pre_team_destroy(Some(id), name);
            }
        })
    }

    fn delete_ai_team_with_observer(
        &mut self,
        id: u32,
        interpreter: Option<&ScriptInterpreter>,
        notify: impl FnOnce(&mut Self, &str),
    ) -> bool {
        let deletion = self
            .team_factory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare_host_team_deletion(id);
        let Some(deletion) = deletion else {
            return false;
        };

        // C++ teamAboutToBeDeleted removes relationship entries before dtor.
        for player in self.get_players_mut().values_mut() {
            player.remove_team_relationship_override(id);
        }
        // A running frame can lend its interpreter while the installed slot
        // holds a different one. Destruction has one receiving queue.
        if let Some(interpreter) = interpreter.or_else(|| {
            self.owned_script_engine
                .as_ref()
                .map(|engine| engine.interpreter())
        }) {
            interpreter.notify_team_destruction(id);
        }
        // C++ numeric PlayerList order, before Object::setTeam(NULL).
        notify(self, deletion.name());
        for member in deletion.members() {
            if let Some((object, ownership_events, partition)) =
                self.host_object_ownership_partition_mut(ObjectId(*member))
            {
                if object.team_instance_id == Some(id) {
                    object.team_instance_id = None;
                    object.team_instance_name.clear();
                    object.set_team_and_owner_with_partition_owner(
                        Team::Neutral,
                        None,
                        ownership_events,
                        partition,
                    );
                }
            }
        }
        self.team_factory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finalize_host_team_deletion(deletion)
    }
}
