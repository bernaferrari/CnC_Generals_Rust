use super::*;
use crate::game_logic::ScriptPresentationEffects;
use gamelogic::scripting::engine::ScriptEngine;
use generals_sim::GameLogicResult;
use generals_sim::scripting::object_status_actions::{ObjectStatusAction, ObjectStatusTarget};

impl GameLogic {
    pub(super) fn apply_owned_script_object_status(
        &mut self,
        action: &ObjectStatusAction,
        this_object: Option<ObjectId>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        interpreter: Option<&ScriptEngine>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        use ObjectStatusAction::*;
        let target = match action {
            SetUnmanned(target) | SetStealth { target, .. } | PlantBoobytrap { target, .. } => {
                Some(target)
            }
            DeleteAllUnmanned => None,
        };
        let ids: Vec<_> = match target {
            Some(ObjectStatusTarget::Named(unit)) => self
                .owned_named_script_object(unit, this_object)
                .into_iter()
                .collect(),
            Some(ObjectStatusTarget::Team(team)) => self
                .resolve_owned_script_team(team, calling_team, condition_team)?
                .into_iter()
                .flat_map(|(_, members)| members.into_iter().map(ObjectId))
                .collect(),
            None => self
                .objects
                .registered_values()
                .filter(|object| object.is_unmanned())
                .map(|object| object.id)
                .collect(),
        };
        // Unmanning moves members to the neutral Team. Planting creates a charge in
        // the victim's Team. Keep the original roster and guard against ID reuse.
        let targets: Vec<_> = ids
            .into_iter()
            .filter_map(|id| {
                let object = self.objects.get(&id)?;
                Some((
                    id,
                    std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                ))
            })
            .collect();
        for (id, birth) in targets {
            if !self.object_birth_matches(id, &birth) {
                continue;
            }
            match action {
                SetUnmanned(_) => self.set_script_unmanned(id, effects)?,
                DeleteAllUnmanned => self.destroy_object_with_effects(id, effects)?,
                SetStealth { enabled, .. } => {
                    // StealthUpdate observes SCRIPT_UNSTEALTHED at its next update.
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.set_script_unstealthed(!enabled);
                    }
                }
                PlantBoobytrap { template, .. } => {
                    self.create_script_sticky_bomb_with_effects(
                        template,
                        id,
                        interpreter,
                        effects,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// ScriptActions.cpp sets DISABLED_UNMANNED, deselects for all players, then
    /// transfers the unit to the neutral player's default Team.
    fn set_script_unmanned(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        let Some(object) = self.objects.get_mut(&id) else {
            return Ok(());
        };
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        if !object.is_unmanned() {
            object.status.unmanned_owner_team = Some(object.team);
            object.status.unmanned_owner_player_id = object.owner_player_id;
        }
        self.set_owned_unmanned(id, effects)?;
        if !self.object_birth_matches(id, &birth) {
            return Ok(());
        }
        self.deselect_drawable(id);
        self.transfer_unmanned_to_neutral_team(id, effects)
    }
}
