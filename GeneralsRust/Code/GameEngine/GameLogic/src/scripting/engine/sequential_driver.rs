// Exact handles for standalone sequences whose receiving driver has no actor observations.

/// Standalone actor observations retained across a sequential instruction.
#[doc(hidden)]
pub struct CoreSequentialActors<'a> {
    engine: &'a ScriptEngine,
    object: Option<crate::object::registry::SharedObjectHandle>,
    team: Option<Arc<RwLock<crate::team::Team>>>,
    projected_object: Option<ScriptObjectStatus>,
}

impl<'engine> SequentialProgramDriver for ScriptProgramWorld<'engine, '_> {
    type StandaloneActors = CoreSequentialActors<'engine>;

    fn resolve_standalone_sequence_actors(
        &self,
        object_id: Option<u32>,
        team_id: Option<TeamID>,
    ) -> Self::StandaloneActors {
        let engine = self.engine;
        let team = team_id.and_then(|id| engine.standalone_team_by_id(id));
        let object = object_id.and_then(|id| engine.object_registry.get_object(id));
        let projected_object = if object.is_none() {
            object_id.and_then(|id| {
                let host = engine.host_query().host_script_query_object_by_id(id)?;
                let dead = host.effectively_dead || !host.alive;
                Some(ScriptObjectStatus {
                    // The compatibility host projection has no AI pointer.
                    has_ai: true,
                    idle: host.idle || dead,
                    effectively_dead: dead,
                })
            })
        } else {
            None
        };
        CoreSequentialActors {
            engine,
            object,
            team,
            projected_object,
        }
    }
}

impl StandaloneSequentialActors for CoreSequentialActors<'_> {
    fn object_exists(&self) -> bool {
        self.object.is_some()
    }

    fn team_exists(&self) -> bool {
        self.team.is_some()
    }

    fn object_status(&self) -> Option<ScriptObjectStatus> {
        let object = self.object.as_ref()?;
        let status = match object.read() {
            Ok(object) => ScriptObjectStatus {
                has_ai: object.get_ai_update_interface().is_some(),
                idle: object.is_idle(),
                effectively_dead: object.is_effectively_dead(),
            },
            Err(_) => ScriptObjectStatus {
                has_ai: false,
                idle: false,
                effectively_dead: true,
            },
        };
        Some(status)
    }

    fn projected_object_status(&self) -> Option<ScriptObjectStatus> {
        self.projected_object
    }

    fn team_status(&self) -> Option<ScriptTeamStatus> {
        let team = self.team.as_ref()?;
        let registry = &self.engine.object_registry;
        let (idle, dead) = if registry.is_empty() {
            let name = team
                .read()
                .ok()
                .map(|team| team.get_name().to_string())
                .unwrap_or_default();
            self.engine.host_query().host_team_sequential_status(&name)
        } else if let Ok(team) = team.read() {
            let idle = team.is_idle_in_registry(registry);
            let dead = team.get_members().iter().all(|&id| {
                registry
                    .with_object(id, |object| object.is_effectively_dead())
                    .unwrap_or(true)
            });
            (idle, dead)
        } else {
            (false, true)
        };
        Some(ScriptTeamStatus {
            has_group: true,
            idle,
            dead,
        })
    }

    fn team_idle_after_action(&self) -> bool {
        // Retain the standalone projection's one-instruction-per-frame behavior.
        !self.engine.object_registry.is_empty() && self.team_status().is_some_and(|team| team.idle)
    }

    fn current_skirmish_player(&self) -> Option<String> {
        let id = if let Some(object) = &self.object {
            object.read().ok()?.get_controlling_player_id()
        } else {
            self.team.as_ref()?.read().ok()?.get_controlling_player_id()
        }?;
        let player = self
            .engine
            .players
            .read()
            .ok()?
            .get_player(id as i32)
            .cloned()?;
        let player = player.read().ok()?;
        if !player.is_skirmish_ai() {
            return None;
        }
        self.engine
            .common
            .name_key_namespace
            .lock()
            .ok()?
            .key_to_name(player.get_player_name_key())
    }
}

#[cfg(test)]
impl ScriptEngine {
    #[cfg(test)]
    fn evaluate_sequential_scripts_canonical_for_test(&self) -> GameLogicResult<()> {
        let mut driver = CanonicalScriptExecutionDriver;
        let context = std::cell::RefCell::new(ScriptContext::at_frame(self.owning_logic_frame()));
        let mut execution = ScriptExecution::new(&context, &mut driver);
        let frame = execution.frame;
        let mut program = ScriptProgramWorld::new(self, &mut execution);
        self.runtime.advance_sequences(frame, &mut program)
    }

    #[cfg(test)]
    fn sequential_script_count(&self) -> usize {
        self.runtime.sequential().len()
    }
}
