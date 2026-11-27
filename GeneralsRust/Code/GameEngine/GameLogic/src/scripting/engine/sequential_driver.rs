// World adapter for the sequential interpreter owned by generals-sim.

struct SequentialContextRestore<'a> {
    engine: &'a ScriptEngine,
    current_player: Option<String>,
    condition_team: Option<TeamID>,
    condition_object: Option<u32>,
}

impl Drop for SequentialContextRestore<'_> {
    fn drop(&mut self) {
        let mut inner = self.engine.lock_inner_mut();
        inner.current_player = self.current_player.take();
        inner.condition_team = self.condition_team;
        inner.condition_object = self.condition_object;
    }
}

struct SequentialActors {
    object: Option<Arc<RwLock<crate::object::Object>>>,
    team: Option<Arc<RwLock<crate::team::Team>>>,
    object_has_ai: bool,
    team_has_group: bool,
    host_object_was_dead: bool,
}

impl SequentialDriver for ScriptProgramWorld<'_, '_> {
    type Actors = SequentialActors;

    fn observe_actors(
        &mut self,
        target: SequentialTarget,
    ) -> Option<(SequentialActors, SequentialReadiness)> {
        let SequentialTarget { object_id, team_id } = target;
        let driving_object = if object_id == INVALID_ID {
            ScriptOwnerQuery::Missing
        } else {
            self.execution.driver.object_status(object_id)
        };
        let driving_team = team_id
            .map(|id| self.execution.driver.team_status(id))
            .unwrap_or(ScriptOwnerQuery::Missing);
        let team = team_id
            .filter(|_| matches!(driving_team, ScriptOwnerQuery::Unavailable))
            .and_then(|id| get_team_factory().lock().ok()?.find_team_by_id(id));
        let object = (object_id != INVALID_ID
            && matches!(driving_object, ScriptOwnerQuery::Unavailable))
        .then(|| TheGameLogic::find_object_by_id(object_id))
        .flatten();
        let host_object = (object_id != INVALID_ID
            && object.is_none()
            && matches!(driving_object, ScriptOwnerQuery::Unavailable))
        .then(|| {
            self.engine
                .host_query()
                .host_script_query_object_by_id(object_id)
        })
        .flatten();

        let live_object =
            object.is_some() || matches!(driving_object, ScriptOwnerQuery::Present(_));
        if !live_object
            && team.is_none()
            && host_object.is_none()
            && !matches!(driving_team, ScriptOwnerQuery::Present(_))
        {
            return None;
        }

        let current_player = match self
            .execution
            .driver
            .sequential_current_player(object_id, team_id)
        {
            ScriptOwnerQuery::Present(player) => player,
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => self
                .engine
                .resolve_sequential_current_player(object.as_ref(), team.as_ref()),
        };
        self.engine.lock_inner_mut().current_player = current_player;

        let host_object_was_dead = host_object
            .as_ref()
            .is_some_and(|host| host.effectively_dead || !host.alive);
        let (object_has_ai, object_idle) = if let ScriptOwnerQuery::Present(status) = driving_object
        {
            (status.has_ai, status.idle)
        } else if let Some(actor) = &object {
            let (has_ai, idle, _) = ScriptEngine::object_ai_status(actor);
            (has_ai, idle)
        } else if let Some(host) = &host_object {
            // The compatibility host projection has no AI pointer.
            (true, host.idle || host_object_was_dead)
        } else {
            (false, false)
        };
        let (team_has_group, team_idle) = if let ScriptOwnerQuery::Present(status) = driving_team {
            (status.has_group, status.idle)
        } else if let Some(team) = &team {
            (
                true,
                ScriptEngine::team_ai_status(&self.engine.host_query(), team).0,
            )
        } else {
            (false, false)
        };
        Some((
            SequentialActors {
                object,
                team,
                object_has_ai,
                team_has_group,
                host_object_was_dead,
            },
            SequentialReadiness {
                has_ai: object_has_ai || team_has_group,
                idle: (object_has_ai && object_idle) || (team_has_group && team_idle),
                condition_object: live_object.then_some(object_id),
            },
        ))
    }

    fn execute_action(
        &mut self,
        target: SequentialTarget,
        condition_object: Option<u32>,
        action: &ScriptAction,
    ) -> GameLogicResult<crate::scripting::executor::ScriptActionResult> {
        {
            let mut inner = self.engine.lock_inner_mut();
            inner.condition_team = target.team_id;
            inner.condition_object = condition_object;
        }
        program_execution::dispatch_action(action, self)
    }

    fn observe_after_action(
        &mut self,
        target: SequentialTarget,
        actors: &SequentialActors,
    ) -> SequentialAfterAction {
        // C++ re-queries live AI after the immediate effects of the instruction.
        let object_after = self.execution.driver.object_status(target.object_id);
        let team_after = target
            .team_id
            .map(|id| self.execution.driver.team_status(id))
            .unwrap_or(ScriptOwnerQuery::Missing);
        let object_idle = match object_after {
            ScriptOwnerQuery::Present(status) => status.has_ai && status.idle,
            ScriptOwnerQuery::Missing => true,
            ScriptOwnerQuery::Unavailable => actors
                .object
                .as_ref()
                .is_some_and(|object| ScriptEngine::object_ai_status(object).1),
        };
        let team_idle = match team_after {
            ScriptOwnerQuery::Present(status) => status.has_group && status.idle,
            ScriptOwnerQuery::Missing => true,
            ScriptOwnerQuery::Unavailable => {
                !dual_world_registry_unavailable()
                    && actors.team.as_ref().is_some_and(|team| {
                        ScriptEngine::team_ai_status(&self.engine.host_query(), team).0
                    })
            }
        };
        if actors.host_object_was_dead {
            return SequentialAfterAction::RemoveChain;
        }
        if !(actors.object_has_ai && object_idle || actors.team_has_group && team_idle) {
            return SequentialAfterAction::Yield;
        }
        let object_dead = match object_after {
            ScriptOwnerQuery::Present(status) => status.effectively_dead,
            ScriptOwnerQuery::Missing => actors.object_has_ai,
            ScriptOwnerQuery::Unavailable => actors
                .object
                .as_ref()
                .is_some_and(|object| ScriptEngine::object_ai_status(object).2),
        };
        let team_dead = match team_after {
            ScriptOwnerQuery::Present(status) => status.dead,
            ScriptOwnerQuery::Missing => actors.team_has_group,
            ScriptOwnerQuery::Unavailable => actors.team.as_ref().is_some_and(|team| {
                ScriptEngine::team_ai_status(&self.engine.host_query(), team).1
            }),
        };
        if object_dead || team_dead {
            SequentialAfterAction::RemoveChain
        } else {
            SequentialAfterAction::Continue
        }
    }
}

impl ScriptEngine {
    fn evaluate_and_progress_all_sequential_scripts(
        &self,
        execution: &mut ScriptExecution<'_>,
    ) -> GameLogicResult<()> {
        let _restore = self.with_inner(|inner| SequentialContextRestore {
            engine: self,
            current_player: inner.current_player.clone(),
            condition_team: inner.condition_team,
            condition_object: inner.condition_object,
        });
        self.sequential_scripts
            .tick(&mut ScriptProgramWorld::new(self, execution))
    }

    #[cfg(test)]
    fn evaluate_sequential_scripts_canonical_for_test(&self) -> GameLogicResult<()> {
        let mut driver = CanonicalScriptExecutionDriver;
        let context = std::cell::RefCell::new(crate::scripting::executor::ScriptContext::new());
        let mut execution = ScriptExecution::new(&context, &mut driver);
        self.evaluate_and_progress_all_sequential_scripts(&mut execution)
    }

    #[cfg(test)]
    fn sequential_script_count(&self) -> usize {
        self.sequential_scripts.len()
    }

    fn object_ai_status(object_arc: &Arc<RwLock<crate::object::Object>>) -> (bool, bool, bool) {
        let Ok(object) = object_arc.read() else {
            return (false, false, true);
        };
        let has_ai = object.get_ai_update_interface().is_some();
        let idle = object.is_idle();
        let dead = object.is_effectively_dead();
        (has_ai, idle, dead)
    }

    fn team_ai_status(
        q: &crate::scripting::HostScriptQuery,
        team_arc: &Arc<RwLock<crate::team::Team>>,
    ) -> (bool, bool) {
        if dual_world_registry_unavailable() {
            let name = team_arc
                .read()
                .ok()
                .map(|team| team.get_name().to_string())
                .unwrap_or_default();
            return q.host_team_sequential_status(&name);
        }

        let Ok(team) = team_arc.read() else {
            return (false, true);
        };

        let idle = team.is_idle();
        let mut all_dead = true;
        for &member_id in team.get_members() {
            let Some(object_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(object) = object_arc.read() else {
                continue;
            };
            if !object.is_effectively_dead() {
                all_dead = false;
                break;
            }
        }

        (idle, all_dead)
    }

    fn resolve_sequential_current_player(
        &self,
        object_arc: Option<&Arc<RwLock<crate::object::Object>>>,
        team_arc: Option<&Arc<RwLock<crate::team::Team>>>,
    ) -> Option<String> {
        let player_id = if let Some(object_arc) = object_arc {
            object_arc
                .read()
                .ok()
                .and_then(|object| object.get_controlling_player_id())
        } else if let Some(team_arc) = team_arc {
            team_arc
                .read()
                .ok()
                .and_then(|team| team.get_controlling_player_id())
        } else {
            None
        }?;

        crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player| {
                player.read().ok().and_then(|player| {
                    if player.is_skirmish_ai() {
                        NameKeyGenerator::key_to_name(player.get_player_name_key())
                    } else {
                        None
                    }
                })
            })
    }
}
