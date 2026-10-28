//! Receiving team identity, synchronous creation effects and membership lifetime.
//! C++ TeamFactory/ScriptActions/Object methods keep their original observation order.
use super::*;

impl GameLogic {
    /// C++ GameLogic.cpp:1888 `team->setActive()` when the team first has members.
    pub fn activate_leftover_team_for_host_object(&mut self, id: ObjectId) {
        self.activate_leftover_team_for_host_object_with_effects(id, &mut Default::default())
    }

    pub(crate) fn activate_leftover_team_for_host_object_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.admit_team_for_host_object_with_effects(id, None, true, effects);
    }

    pub(crate) fn admit_team_for_host_object(
        &mut self,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        activate: bool,
    ) {
        self.admit_team_for_host_object_with_effects(
            id,
            interpreter,
            activate,
            &mut Default::default(),
        )
    }

    pub(crate) fn admit_team_for_host_object_with_effects(
        &mut self,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        activate: bool,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) =
            self.admit_team_for_host_object_with_effects_checked(id, interpreter, activate, ui)
        {
            log::error!("{error}");
        }
    }

    pub(crate) fn run_team_creation_effects(
        &mut self,
        effects: Vec<String>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.run_team_creation_effects_with_effects(effects, interpreter, &mut Default::default())
    }

    pub(crate) fn run_team_creation_effects_with_effects(
        &mut self,
        effects: Vec<String>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if effects.is_empty() {
            return Ok(());
        }
        if let Some(engine) = interpreter {
            for name in effects {
                self.execute_owned_ai_condition_with_effects_checked(engine, &name, None, ui)?;
            }
            Ok(())
        } else {
            self.with_owned_script_engine(|engine, world| {
                for name in effects {
                    world
                        .execute_owned_ai_condition_with_effects_checked(engine, &name, None, ui)?;
                }
                Ok(())
            })
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "team creation requires the driving interpreter".into(),
                )
            })?
        }
    }

    /// C++ Object::onDie calls the resident Team's script synchronously after
    /// Die modules. Resolve its identity and script under the receiving owner,
    /// then release the Team borrow before executing potentially reentrant work.
    pub(crate) fn notify_owned_team_of_object_death_with_effects(
        &mut self,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(team_id) = self
            .objects
            .get(&id)
            .and_then(|object| object.team_instance_id)
        else {
            return Ok(());
        };
        let callback = {
            let factory = self
                .team_factory
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(team) = factory.find_team_by_id(team_id) else {
                return Ok(());
            };
            let team = team.read().unwrap_or_else(|error| error.into_inner());
            team.object_death_script()
                .map(|name| (name.to_owned(), team.get_controlling_player_id()))
        };
        let Some((name, player_id)) = callback else {
            return Ok(());
        };
        if let Some(recipient) = interpreter
            .map(crate::game_logic::ScriptEventRecipient::new)
            .or(effects.script_events)
        {
            return recipient.run_team_script(self, &name, team_id, player_id, effects);
        }
        self.with_owned_script_engine(|engine, world| {
            world.run_owned_team_script_with_effects_checked(
                engine, &name, team_id, player_id, effects,
            )
        })
        .ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "team death callback requires the receiving interpreter".into(),
            )
        })?
    }

    /// C++ Object destructor unlinks only its retained resident team pointer.
    /// A missing identity never selects a same-name sibling or creates a team.
    pub(in crate::game_logic) fn unlink_retiring_team_host_member(
        &self,
        id: ObjectId,
        identity: Option<u32>,
    ) {
        let Some(identity) = identity else {
            return;
        };
        let factory = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(team) = factory.find_team_by_id(identity) else {
            return;
        };
        let mut guard = team.write().unwrap_or_else(|error| error.into_inner());
        if guard.has_member(id.0) {
            guard.remove_member(id.0);
        }
    }

    /// C++ `ScriptActions::doMergeTeamIntoTeam` live drain.
    /// Leftover `set_team` misses empty `OBJECT_REGISTRY`; rewrite live
    /// `team_instance_name` so TEAM/NAMED census follows dest.
    pub fn apply_host_merge_team_script_requests(&mut self) {
        self.apply_host_merge_team_script_requests_with_effects(&mut Default::default())
    }

    pub(crate) fn apply_host_merge_team_script_requests_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        for req in gamelogic::scripting::take_host_script_merge_team_requests() {
            self.host_script_merge_team_into_team_with_effects(&req.source, &req.dest, effects);
        }
    }

    /// C++ `obj->setTeam(teamDest)` + `updateTeamAndPlayerStuff` + dest `setActive`.
    fn host_script_merge_team_into_team(&mut self, source: &str, dest: &str) {
        self.host_script_merge_team_into_team_with_effects(source, dest, &mut Default::default())
    }

    fn host_script_merge_team_into_team_with_effects(
        &mut self,
        source: &str,
        dest: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Err(error) =
            self.merge_owned_script_team_with_effects(source, dest, None, None, None, effects)
        {
            log::error!("owned team merge failed: {error}");
        }
    }

    pub(crate) fn merge_owned_script_team(
        &mut self,
        source: &str,
        destination: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.merge_owned_script_team_with_effects(
            source,
            destination,
            calling,
            condition,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn merge_owned_script_team_with_effects(
        &mut self,
        source: &str,
        destination: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if interpreter.is_none() && self.owned_script_engine.is_some() {
            return self
                .with_owned_script_engine(|engine, world| {
                    world.merge_owned_script_team_with_effects(
                        source,
                        destination,
                        calling,
                        condition,
                        Some(engine),
                        ui,
                    )
                })
                .expect("installed interpreter taken synchronously");
        }
        // ScriptActions.cpp3137 resolves both operands before its null check.
        // A missing source must not suppress destination creation callbacks;
        // nor may a callback-created source replace the original null pointer.
        let source_selection = self
            .resolve_owned_script_team(source, calling, condition)?
            .and_then(|(id, _)| {
                self.team_factory
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .find_team_by_id(id)
                    .map(|identity| (id, identity))
            });
        let destination_id =
            match self.resolve_owned_script_team(destination, calling, condition)? {
                Some((id, _)) => id,
                None => {
                    // Only the destination fallback calls TeamFactory::findTeam;
                    // singleton absence stays absent. Callback runs while inactive.
                    self.require_owned_team_creation_interpreter(destination, false, interpreter)?;
                    let found = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .find_team_with_owned_effects(destination);
                    let Some((team, effects)) = found else {
                        return Ok(());
                    };
                    let id = team
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .get_id();
                    self.run_owned_team_creation_actions(&team, effects, interpreter, ui)?;
                    let current = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .find_team_by_id(id);
                    if current.is_none_or(|current| !std::sync::Arc::ptr_eq(&current, &team)) {
                        return Ok(());
                    }
                    id
                }
            };
        let Some((source_id, source_identity)) = source_selection else {
            return Ok(());
        };
        if source_id == destination_id {
            return Ok(());
        }
        // Create callbacks may alter either roster; re-read exact identities.
        let (source, destination) = {
            let factory = self
                .team_factory
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            (
                factory.find_team_by_id(source_id),
                factory.find_team_by_id(destination_id),
            )
        };
        let (Some(source), Some(destination)) = (source, destination) else {
            return Ok(());
        };
        if !std::sync::Arc::ptr_eq(&source_identity, &source) {
            return Ok(());
        }
        let members = source
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get_members()
            .to_vec();
        let (name, owner) = {
            let team = destination
                .read()
                .unwrap_or_else(|error| error.into_inner());
            (
                team.get_name().to_string(),
                team.get_controlling_player_id(),
            )
        };
        let factory = self.team_factory.clone();
        for raw_id in members {
            let id = ObjectId(raw_id);
            let Some(object) = self.host_object_mut(id) else {
                continue;
            };
            if object.team_instance_id != Some(source_id) {
                continue;
            }
            object.team_instance_name = name.clone();
            object.team_instance_id = Some(destination_id);
            if let Some(owner) = owner {
                object.owner_player_id = Some(owner);
            }
            object.apply_named_team_ai_profile(&factory, true);
            source
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .remove_member(raw_id);
            destination
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .add_member(raw_id);
        }
        // C++ retires source before activating destination. The existing owned
        // destruction path notifies the receiving interpreter/controllers.
        if let Some(engine) = interpreter {
            self.delete_canonical_ai_team_with_interpreter(source_id, engine);
        } else {
            self.delete_canonical_ai_team(source_id);
        }
        if let Some(team) = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .find_team_by_id(destination_id)
        {
            if std::sync::Arc::ptr_eq(&team, &destination) {
                team.write()
                    .unwrap_or_else(|error| error.into_inner())
                    .set_active();
            }
        }
        Ok(())
    }
}
