//! Receiving team identity, synchronous creation effects and membership lifetime.
//! C++ TeamFactory/ScriptActions/Object methods keep their original observation order.
use super::*;

impl GameLogic {
    /// C++ GameLogic.cpp:1888 `team->setActive()` when the team first has members.
    pub fn activate_leftover_team_for_host_object(&mut self, id: ObjectId) {
        self.admit_team_for_host_object(id, None, true);
    }

    pub(crate) fn admit_team_for_host_object(
        &mut self,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        activate: bool,
    ) {
        self.admit_team_for_host_object_with_ui(id, interpreter, activate, &mut None)
    }

    pub(crate) fn admit_team_for_host_object_with_ui(
        &mut self,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        activate: bool,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let team_name = obj.team_instance_name.trim().to_string();
        let previous_identity = obj.team_instance_id;
        let generation = obj.visual_object_generation;
        if team_name.is_empty() {
            return;
        }
        let Ok(mut factory) = self.team_factory.lock() else {
            return;
        };
        if let Some(player_id) = obj.owner_player_id {
            if let Some(player) = self.players.get(&player_id) {
                if team_name == self.default_host_team_instance_name(Some(player_id), obj.team) {
                    super::super::team_membership::ensure_default_team_definition(
                        &mut factory,
                        &team_name,
                        default_team_owner_name(player).unwrap_or(&player.name),
                        player_id,
                    );
                }
            }
        }
        let team = match obj.team_instance_id {
            Some(identity) => {
                // A missing exact identity cannot become another same-name
                // instance. Only an explicit authored-name change selects anew.
                let Some(existing) = factory.find_team_by_id(identity) else {
                    return;
                };
                let same_definition = existing
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_name()
                    .as_str()
                    == team_name;
                if same_definition {
                    Some((existing, Vec::new()))
                } else {
                    factory
                        .find_team_with_owned_effects(&team_name)
                        .or_else(|| factory.create_inactive_team_with_owned_effects(&team_name))
                }
            }
            None => factory
                .find_team_with_owned_effects(&team_name)
                .or_else(|| factory.create_inactive_team_with_owned_effects(&team_name)),
        };
        let Some((team, effects)) = team else {
            return;
        };
        let identity = team
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get_id();
        drop(factory);
        if let Err(error) = self.run_team_creation_effects_with_ui(effects, interpreter, ui) {
            log::error!("team admission callback failed: {error}");
            return;
        }
        // A synchronous create action may delete/reassign the object or retire
        // the admitted instance. Never attach through a stale captured handle.
        if !self.objects.get(&id).is_some_and(|object| {
            object.team_instance_name.trim() == team_name
                && object.team_instance_id == previous_identity
                && object.visual_object_generation == generation
        }) {
            return;
        }
        let factory = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(current) = factory.find_team_by_id(identity) else {
            return;
        };
        if !std::sync::Arc::ptr_eq(&current, &team) {
            return;
        }
        // A map/script admission may replace the initial default team. Move
        // the member before adding it; this operation is not a frame-end repair.
        for previous in factory.get_all_teams() {
            if !std::sync::Arc::ptr_eq(&previous, &team) {
                previous
                    .write()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove_member(id.0);
            }
        }
        let identity = {
            let mut guard = team.write().unwrap_or_else(|error| error.into_inner());
            guard.add_member(id.0);
            if activate {
                guard.set_active();
            }
            guard.get_id()
        };
        drop(factory);
        self.objects
            .get_mut(&id)
            .expect("admitted receiving object")
            .team_instance_id = Some(identity);
    }

    pub(crate) fn run_team_creation_effects(
        &mut self,
        effects: Vec<String>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.run_team_creation_effects_with_ui(effects, interpreter, &mut None)
    }

    pub(crate) fn run_team_creation_effects_with_ui(
        &mut self,
        effects: Vec<String>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) -> gamelogic::GameLogicResult<()> {
        if effects.is_empty() {
            return Ok(());
        }
        if let Some(engine) = interpreter {
            for name in effects {
                self.execute_owned_ai_condition_with_ui(engine, &name, None, ui);
            }
            Ok(())
        } else {
            self.with_owned_script_engine(|engine, world| {
                for name in effects {
                    world.execute_owned_ai_condition_with_ui(engine, &name, None, ui);
                }
            })
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "team creation requires the driving interpreter".into(),
                )
            })
        }
    }

    /// C++ Object.cpp:4592 `m_team->notifyTeamOfObjectDeath()`.
    pub fn notify_leftover_team_of_host_object_death(&self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let Some(identity) = obj.team_instance_id else {
            return;
        };
        let Ok(factory) = self.team_factory.lock() else {
            return;
        };
        // C++ uses the object's resident team pointer. Death cannot select a
        // same-name sibling or implicitly create a new team and run scripts.
        let Some(team) = factory.find_team_by_id(identity) else {
            return;
        };
        if let Ok(mut guard) = team.write() {
            if !guard.has_member(id.0) {
                return;
            }
            guard.notify_team_of_object_death();
            guard.remove_member(id.0);
        }
        drop(factory);
        gamelogic::team::flush_pending_team_script_events();
    }

    /// C++ Object destructor unlinks from TeamMemberList.
    pub fn unlink_leftover_team_host_member(&self, id: ObjectId, team_name: &str) {
        let team_name = team_name.trim();
        if team_name.is_empty() {
            return;
        }
        let factory = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // Resolve existing instances only. A destructor must not create an
        // inactive instance or select a different instance sharing this name.
        let instances = match self
            .objects
            .get(&id)
            .and_then(|object| object.team_instance_id)
        {
            Some(identity) => factory.find_team_by_id(identity).into_iter().collect(),
            None => factory.find_team_instances(team_name),
        };
        for team in instances {
            let mut guard = team.write().unwrap_or_else(|error| error.into_inner());
            if guard.has_member(id.0) {
                guard.remove_member(id.0);
            }
        }
    }

    /// C++ `ScriptActions::doMergeTeamIntoTeam` live drain.
    /// Leftover `set_team` misses empty `OBJECT_REGISTRY`; rewrite live
    /// `team_instance_name` so TEAM/NAMED census follows dest.
    pub fn apply_host_merge_team_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_script_merge_team_requests() {
            self.host_script_merge_team_into_team(&req.source, &req.dest);
        }
    }

    /// C++ `obj->setTeam(teamDest)` + `updateTeamAndPlayerStuff` + dest `setActive`.
    fn host_script_merge_team_into_team(&mut self, source: &str, dest: &str) {
        if let Err(error) = self.merge_owned_script_team(source, dest, None, None, None) {
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
        self.merge_owned_script_team_with_ui(
            source,
            destination,
            calling,
            condition,
            interpreter,
            &mut None,
        )
    }

    pub(crate) fn merge_owned_script_team_with_ui(
        &mut self,
        source: &str,
        destination: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) -> gamelogic::GameLogicResult<()> {
        if interpreter.is_none() && self.owned_script_engine.is_some() {
            return self
                .with_owned_script_engine(|engine, world| {
                    world.merge_owned_script_team_with_ui(
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
                    if !effects.is_empty() {
                        let engine = interpreter.ok_or_else(|| {
                            gamelogic::GameLogicError::Configuration(
                                "team-create callback requires the driving interpreter".into(),
                            )
                        })?;
                        for name in effects {
                            self.execute_owned_ai_condition_with_ui(engine, &name, None, ui);
                        }
                    }
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
