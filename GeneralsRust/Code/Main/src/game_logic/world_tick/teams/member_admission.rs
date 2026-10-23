//! One actual Team admission: prepare synchronous creation before external
//! Object publication, then commit membership and receiving Energy influence.
use super::*;

struct TeamAdmissionTarget {
    name: String,
    previous: Option<u32>,
    owner: Option<u32>,
    faction: crate::game_logic::Team,
}
impl TeamAdmissionTarget {
    fn from_object(object: &Object) -> Self {
        Self {
            name: object.team_instance_name.trim().to_owned(),
            previous: object.team_instance_id,
            owner: object.owner_player_id,
            faction: object.team,
        }
    }
}
enum HostTeamPreparation {
    NoTeam,
    Ready(PreparedHostTeamAdmission),
    Invalidated,
}
pub(in crate::game_logic) struct PreparedHostTeamAdmission {
    target: TeamAdmissionTarget,
    team: std::sync::Arc<std::sync::RwLock<gamelogic::team::Team>>,
    identity: u32,
}

impl GameLogic {
    /// The ordinary factory already owns a registered primary. Preserve its
    /// exact generation across synchronous Team-created actions before commit.
    pub(crate) fn admit_team_for_host_object_with_effects_checked(
        &mut self,
        id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        activate: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let expected = self.live_draw_playback_identity(object);
        let target = TeamAdmissionTarget::from_object(object);
        let HostTeamPreparation::Ready(prepared) =
            self.prepare_host_team_admission(target, interpreter, effects)?
        else {
            return Ok(());
        };
        if !self.objects.get(&id).is_some_and(|object| {
            self.live_draw_playback_identity(object) == expected
                && object.team_instance_name.trim() == prepared.target.name
                && object.team_instance_id == prepared.target.previous
                && object.owner_player_id == prepared.target.owner
                && object.team == prepared.target.faction
        }) {
            return Ok(());
        }
        let owner = self.team_factory.clone();
        let factory = owner.lock().unwrap_or_else(|error| error.into_inner());
        if !Self::prepared_host_team_is_current(&factory, &prepared) {
            return Ok(());
        }
        let first = self.commit_host_team_member(id, &prepared, &factory, activate);
        drop(factory);
        if first {
            self.admit_object_energy_influence(id);
        }
        Ok(())
    }

    /// External supplied Object stays owned by its caller operation and absent
    /// from public lookup throughout fallible Team creation actions, like the
    /// original Team preparation before Object constructor registration.
    pub(in crate::game_logic) fn prepare_external_host_object_team(
        &mut self,
        object: &Object,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<PreparedHostTeamAdmission>> {
        match self.prepare_host_team_admission(
            TeamAdmissionTarget::from_object(object),
            interpreter,
            effects,
        )? {
            HostTeamPreparation::NoTeam => Ok(None),
            HostTeamPreparation::Ready(team) => Ok(Some(team)),
            HostTeamPreparation::Invalidated => Err(gamelogic::GameLogicError::Configuration(
                "requested external Team was invalidated by its creation actions".into(),
            )),
        }
    }

    fn prepare_host_team_admission(
        &mut self,
        target: TeamAdmissionTarget,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<HostTeamPreparation> {
        if let Some(error) = self.team_creation_error() {
            return Err(error);
        }
        let invalid = |reason: &str| gamelogic::GameLogicError::Configuration(reason.into());
        if target
            .owner
            .is_some_and(|id| !self.players.contains_key(&id))
        {
            return Err(gamelogic::GameLogicError::Configuration(
                "Object admission names a missing receiving Player".into(),
            ));
        }
        if target.name.is_empty() {
            return Ok(HostTeamPreparation::NoTeam);
        }
        let mut factory = self
            .team_factory
            .lock()
            .map_err(|_| invalid("receiving Team owner is unavailable during admission"))?;
        if let Some(player) = target.owner.and_then(|id| self.players.get(&id)) {
            if target.name == self.default_host_team_instance_name(target.owner, target.faction) {
                super::super::team_membership::ensure_default_team_definition(
                    &mut factory,
                    &target.name,
                    default_team_owner_name(player).ok_or_else(|| {
                        gamelogic::GameLogicError::Configuration(
                            "receiving Player has no admitted script identity".into(),
                        )
                    })?,
                    player.id,
                );
            }
        }
        // Check capability before constructing a new effectful Team. Existing
        // instances (including a Running synchronous outer creation) remain
        // admissible and never replay their creation actions.
        let existing_by_name = factory.get_all_teams().into_iter().any(|team| {
            team.read()
                .unwrap_or_else(|error| error.into_inner())
                .get_name()
                .as_str()
                == target.name
        });
        let exact_matches = target
            .previous
            .and_then(|id| factory.find_team_by_id(id))
            .is_some_and(|team| {
                team.read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_name()
                    .as_str()
                    == target.name
            });
        if !exact_matches
            && !existing_by_name
            && interpreter.is_none()
            && self.owned_script_engine.is_none()
            && factory
                .find_team_prototype(&target.name)
                .is_some_and(|prototype| {
                    prototype.get_execute_actions_on_create()
                        && !prototype.get_production_condition().is_empty()
                })
        {
            return Err(invalid("Team creation requires driving interpreter"));
        }
        let selected = match target.previous {
            Some(identity) => {
                let existing = factory
                    .find_team_by_id(identity)
                    .ok_or_else(|| invalid("requested exact Team instance is missing"))?;
                let same = existing
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_name()
                    .as_str()
                    == target.name;
                if same {
                    Some((existing, Vec::new()))
                } else {
                    factory
                        .find_team_with_owned_effects(&target.name)
                        .or_else(|| factory.create_inactive_team_with_owned_effects(&target.name))
                }
            }
            None => factory
                .find_team_with_owned_effects(&target.name)
                .or_else(|| factory.create_inactive_team_with_owned_effects(&target.name)),
        };
        let (team, create_effects) = selected
            .ok_or_else(|| invalid("requested Team prototype/instance cannot be resolved"))?;
        let identity = {
            let team = team.read().unwrap_or_else(|error| error.into_inner());
            if let Some(reason) = team.creation_failure() {
                return Err(gamelogic::GameLogicError::TeamCreationFailed(reason.into()));
            }
            if team.get_controlling_player_id() != target.owner {
                return Err(invalid(
                    "requested Team controller differs from the receiving Object owner",
                ));
            }
            team.get_id()
        };
        drop(factory);
        self.run_owned_team_creation_actions(&team, create_effects, interpreter, effects)?;
        let prepared = PreparedHostTeamAdmission {
            target,
            team,
            identity,
        };
        let factory = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !Self::prepared_host_team_is_current(&factory, &prepared) {
            return Ok(HostTeamPreparation::Invalidated);
        }
        Ok(HostTeamPreparation::Ready(prepared))
    }

    fn prepared_host_team_is_current(
        factory: &gamelogic::team::TeamFactory,
        prepared: &PreparedHostTeamAdmission,
    ) -> bool {
        factory
            .find_team_by_id(prepared.identity)
            .is_some_and(|current| {
                std::sync::Arc::ptr_eq(&current, &prepared.team)
                    && current
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .get_controlling_player_id()
                        == prepared.target.owner
            })
    }

    /// No callback occurs between the final exact-Team/ID validation and
    /// storage+member commit. Keep the existing factory loan over that bounded
    /// non-callback interval; release it before recursive Energy callbacks.
    pub(in crate::game_logic) fn admit_external_object_to_prepared_team(
        &mut self,
        object: Object,
        prepared: Option<PreparedHostTeamAdmission>,
    ) -> gamelogic::GameLogicResult<ObjectId> {
        let id = object.id;
        self.validate_host_object_admission(id)?;
        let Some(prepared) = prepared else {
            if !object.team_instance_name.trim().is_empty() {
                return Err(gamelogic::GameLogicError::Configuration(
                    "named external Object Team has no prepared admission".into(),
                ));
            }
            self.admit_host_object_storage(object)?;
            return Ok(id);
        };
        let owner = self.team_factory.clone();
        let factory = owner.lock().unwrap_or_else(|error| error.into_inner());
        if !Self::prepared_host_team_is_current(&factory, &prepared)
            || object.team_instance_name.trim() != prepared.target.name
            || object.team_instance_id != prepared.target.previous
            || object.owner_player_id != prepared.target.owner
            || object.team != prepared.target.faction
        {
            return Err(gamelogic::GameLogicError::Configuration(
                "prepared external Object Team changed before admission".into(),
            ));
        }
        self.admit_host_object_storage(object)?;
        let first = self.commit_host_team_member(id, &prepared, &factory, false);
        drop(factory);
        if first {
            self.admit_object_energy_influence(id);
        }
        Ok(id)
    }

    fn commit_host_team_member(
        &mut self,
        id: ObjectId,
        prepared: &PreparedHostTeamAdmission,
        factory: &gamelogic::team::TeamFactory,
        activate: bool,
    ) -> bool {
        let was_member = factory.get_all_teams().iter().any(|team| {
            team.read()
                .unwrap_or_else(|error| error.into_inner())
                .get_members()
                .contains(&id.0)
        });
        for previous in factory.get_all_teams() {
            if !std::sync::Arc::ptr_eq(&previous, &prepared.team) {
                previous
                    .write()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove_member(id.0);
            }
        }
        {
            let mut team = prepared
                .team
                .write()
                .unwrap_or_else(|error| error.into_inner());
            team.add_member(id.0);
            if activate {
                team.set_active();
            }
        }
        self.objects
            .get_mut(&id)
            .expect("prepared receiving object")
            .team_instance_id = Some(prepared.identity);
        !was_member
    }
}

impl GameLogic {
    /// Derived from canonical Team state; no competing session failure slot.
    pub(crate) fn team_creation_error(&self) -> Option<gamelogic::GameLogicError> {
        let factory = match self.team_factory.lock() {
            Ok(factory) => factory,
            Err(_) => {
                return Some(gamelogic::GameLogicError::Configuration(
                    "receiving Team owner is unavailable".into(),
                ));
            }
        };
        let teams = factory.get_all_teams();
        teams.into_iter().find_map(|team| {
            team.read()
                .unwrap_or_else(|error| error.into_inner())
                .creation_failure()
                .map(|reason| gamelogic::GameLogicError::TeamCreationFailed(reason.into()))
        })
    }
}

impl GameLogic {
    pub(crate) fn require_owned_team_creation_interpreter(
        &self,
        name: &str,
        creates_new: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(error) = self.team_creation_error() {
            return Err(error);
        }
        if interpreter.is_some() || self.owned_script_engine.is_some() {
            return Ok(());
        }
        let factory = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        if !creates_new
            && factory.get_all_teams().iter().any(|team| {
                team.read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_name()
                    .as_str()
                    == name
            })
        {
            return Ok(());
        }
        if factory.find_team_prototype(name).is_some_and(|prototype| {
            prototype.get_execute_actions_on_create()
                && !prototype.get_production_condition().is_empty()
        }) {
            return Err(gamelogic::GameLogicError::Configuration(
                "Team creation requires driving interpreter".into(),
            ));
        }
        Ok(())
    }
    /// All actual creation producers share the exact owning Team continuation.
    pub(crate) fn run_owned_team_creation_actions(
        &mut self,
        team: &std::sync::Arc<std::sync::RwLock<gamelogic::team::Team>>,
        actions: Vec<String>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        {
            let team = team
                .read()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
            if let Some(reason) = team.creation_failure() {
                return Err(gamelogic::GameLogicError::TeamCreationFailed(reason.into()));
            }
        }
        if actions.is_empty() {
            return Ok(());
        }
        team.write()
            .unwrap_or_else(|error| error.into_inner())
            .begin_creation_actions();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_team_creation_effects_with_effects(actions, interpreter, ui)
        }));
        match outcome {
            Ok(result) => {
                team.write()
                    .unwrap_or_else(|error| error.into_inner())
                    .finish_creation_actions(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| error.to_string()),
                    );
                if result.is_ok() {
                    if let Some(reason) = team
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .creation_failure()
                    {
                        return Err(gamelogic::GameLogicError::TeamCreationFailed(reason.into()));
                    }
                }
                result
            }
            Err(payload) => {
                team.write()
                    .unwrap_or_else(|error| error.into_inner())
                    .finish_creation_actions(Err(
                        "Team creation actions panicked; exact partial Team is retained".into(),
                    ));
                std::panic::resume_unwind(payload)
            }
        }
    }
}
