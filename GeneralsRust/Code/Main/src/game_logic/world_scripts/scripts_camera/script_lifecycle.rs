//! CPP Team2223/2414: evacuation precedes a fresh canonical roster walk.
use super::*;
use gamelogic::scripting::HostScriptKillDeleteDamageRequest as Request;

impl GameLogic {
    pub(super) fn apply_owned_script_lifecycle(
        &mut self,
        request: &Request,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_owned_script_lifecycle_with_effects(
            request,
            this_object,
            calling,
            condition,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_owned_script_lifecycle_with_effects(
        &mut self,
        request: &Request,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        match request {
            Request::NamedDelete { unit } => {
                if let Some(id) = self.owned_named_script_object(unit, this_object) {
                    self.destroy_object_with_effects(id, effects);
                }
            }
            Request::NamedKill { unit } => {
                if let Some(id) = self.owned_named_script_object(unit, this_object) {
                    self.host_script_damage_with_interpreter_with_effects(
                        id,
                        0.0,
                        true,
                        interpreter,
                        effects,
                    );
                }
            }
            Request::NamedDamage { unit, amount } => {
                if let Some(id) = self.owned_named_script_object(unit, this_object) {
                    self.host_script_damage_with_interpreter_with_effects(
                        id,
                        *amount as f32,
                        false,
                        interpreter,
                        effects,
                    );
                }
            }
            Request::TeamDamage { team, amount } => {
                self.apply_owned_team_damage_with_effects(
                    team,
                    *amount,
                    calling,
                    condition,
                    interpreter,
                    effects,
                )?;
            }
            Request::TeamDelete { team, ignore_dead } => {
                self.apply_owned_team_lifecycle_with_effects(
                    team,
                    calling,
                    condition,
                    Some(*ignore_dead),
                    interpreter,
                    effects,
                )?;
            }
            Request::TeamKill { team } => {
                self.apply_owned_team_lifecycle_with_effects(
                    team,
                    calling,
                    condition,
                    None,
                    interpreter,
                    effects,
                )?;
            }
            Request::DestroyAllContained { unit } => {
                if let Some(id) = self.owned_named_script_object(unit, this_object) {
                    let container = self.redirected_containment_owner(id);
                    let occupants = self
                        .host_object(container)
                        .map(|o| o.contained_units())
                        .unwrap_or_default();
                    for id in occupants {
                        self.host_script_damage_with_interpreter_with_effects(
                            id,
                            0.0,
                            true,
                            interpreter,
                            effects,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn apply_owned_team_lifecycle(
        &mut self,
        raw: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        delete: Option<bool>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_owned_team_lifecycle_with_effects(
            raw,
            calling,
            condition,
            delete,
            interpreter,
            &mut Default::default(),
        )
    }

    fn apply_owned_team_lifecycle_with_effects(
        &mut self,
        raw: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        delete: Option<bool>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((team_id, initial)) = self.resolve_owned_script_team(raw, calling, condition)?
        else {
            return Ok(());
        };
        let (name, controller) = {
            let factory = self.team_factory.lock().expect("owned lifecycle team");
            let Some(team) = factory.find_team_by_id(team_id) else {
                return Ok(());
            };
            let team = team.read().expect("owned lifecycle identity");
            (
                team.get_name().to_string(),
                team.get_controlling_player_id(),
            )
        };
        let default = controller
            .and_then(|id| self.players.get(&id))
            .is_some_and(|p| self.default_host_team_instance_name(Some(p.id), p.team) == name);
        if delete.is_none() || default {
            let containers: Vec<_> = initial
                .into_iter()
                .map(ObjectId)
                .filter(|id| {
                    self.host_object(*id).is_some_and(|o| {
                        (delete.is_some() || (!o.status.destroyed && !o.status.effectively_dead))
                            && !o.contained_units().is_empty()
                    })
                })
                .collect();
            for id in containers {
                self.evacuate_container_now_with_effects(id, false, effects);
            }
        }
        let members = {
            let factory = self
                .team_factory
                .lock()
                .expect("fresh owned lifecycle roster");
            let Some(team) = factory.find_team_by_id(team_id) else {
                return Ok(());
            };
            let members = team.read().expect("fresh roster").get_members().to_vec();
            members
        };
        let beacon = controller
            .and_then(|id| self.resolved_player_template(id))
            .map(|p| p.get_beacon_template().to_string());
        let neutral = self
            .players
            .values()
            .find(|p| p.team == Team::Neutral && !p.is_observer)
            .map(|p| p.id);
        // This owned ID list is frozen after evacuation; callback mutations are
        // observed again before applying each original operation.
        for member in members {
            let id = ObjectId(member);
            let Some(object) = self.host_object(id) else {
                continue;
            };
            if let Some(ignore_dead) = delete {
                if ignore_dead && object.status.effectively_dead {
                    continue;
                }
                self.destroy_object_with_effects(id, effects);
                continue;
            }
            if object.status.destroyed
                || (object.status.effectively_dead
                    && !beacon
                        .as_ref()
                        .is_some_and(|name| object.template_name == *name))
            {
                continue;
            }
            if object.team_instance_name != name {
                continue;
            }
            if object.is_kind_of(KindOf::TechBuilding) {
                let default_name = self.default_host_team_instance_name(neutral, Team::Neutral);
                let destination = {
                    let mut factory = self
                        .team_factory
                        .lock()
                        .expect("neutral lifecycle destination");
                    factory.find_team(&default_name)
                };
                let Some(destination) = destination else {
                    continue;
                };
                let source = self
                    .team_factory
                    .lock()
                    .expect("owned source")
                    .find_team_by_id(team_id);
                if let Some((object, ownership_events)) =
                    self.host_object_and_ownership_events_mut(id)
                {
                    object.set_team_and_owner(Team::Neutral, neutral, ownership_events);
                    object.team_instance_name = default_name;
                }
                if let Some(source) = source {
                    source
                        .write()
                        .expect("owned transfer source")
                        .remove_member(member);
                }
                destination
                    .write()
                    .expect("neutral transfer roster")
                    .add_member(member);
            } else {
                self.host_script_damage_with_interpreter_with_effects(
                    id,
                    0.0,
                    true,
                    interpreter,
                    effects,
                );
            }
        }
        Ok(())
    }
}
