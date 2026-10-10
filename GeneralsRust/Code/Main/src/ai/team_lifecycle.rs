//! C++ TeamFactory::teamAboutToBeDeleted followed by Team::~Team.
//! Main owns the objects and controllers; Core supplies only team metadata.

use super::*;
use gamelogic::scripting::engine::ScriptEngineHandle;

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
        scripts: &ScriptEngineHandle,
        notify: &mut TeamDestroyObserver<'_>,
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
        let key = deletion.name().trim().to_ascii_lowercase();
        for player in self.get_players_mut().values_mut() {
            player.remove_team_relationship_override(deletion.name());
            player.clear_team_instance_overrides(deletion.name());
            for relationships in player.team_instance_team_relations.values_mut() {
                relationships.remove(&key);
            }
        }
        // The engine is resolved once by the caller. Its process lifetime is
        // still a documented application seam; deletion cannot choose one.
        if let Some(engine) = scripts.write().unwrap_or_else(|e| e.into_inner()).as_mut() {
            engine.notify_of_team_destruction(deletion.name());
        }
        // C++ numeric PlayerList order, before Object::setTeam(NULL).
        notify(actor, self, id, deletion.name());
        for member in deletion.members() {
            if let Some(object) = self.host_object_mut(ObjectId(*member)) {
                if object
                    .team_instance_name
                    .eq_ignore_ascii_case(deletion.name())
                {
                    object.team_instance_name.clear();
                    object.set_team_and_owner(Team::Neutral, None);
                }
            }
        }
        self.team_factory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finalize_host_team_deletion(deletion)
    }
}
