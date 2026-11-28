// Bind the script interpreter to the world receiving its queries and effects.

use crate::scripting::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator, ScriptError,
};
use game_engine::common::system::kind_of::KindOfMask;
use generals_sim::scripting::media_conditions::ScriptMediaQueries;
use generals_sim::scripting::object_actions::ObjectAction;
use generals_sim::scripting::player_actions::PlayerAction;
use generals_sim::scripting::player_area_supply_conditions::ScriptAreaSupplyQueries;
use generals_sim::scripting::player_census_conditions::ScriptCensusQueries;
use generals_sim::scripting::player_event_conditions::ScriptPlayerEventQueries;
use generals_sim::scripting::program_execution::ScriptConditionQueries;
use generals_sim::scripting::world::HostScriptPlayerCensus;

/// Core adapter for observations and effects that still use the legacy executor.
pub struct ScriptProgramWorld<'engine, 'context> {
    engine: &'engine ScriptEngine,
    execution: &'engine mut ScriptExecution<'context>,
    dispatcher: ScriptActionDispatcher<'engine>,
    evaluator: ScriptConditionEvaluator<'engine>,
    #[cfg(feature = "script_profiling")]
    started_at: Instant,
    standalone_player_names: Option<Vec<Option<String>>>,
}

impl<'engine, 'context> ScriptProgramWorld<'engine, 'context> {
    fn new(
        engine: &'engine ScriptEngine,
        execution: &'engine mut ScriptExecution<'context>,
    ) -> Self {
        Self {
            engine,
            dispatcher: ScriptActionDispatcher::new(engine, execution.context),
            evaluator: ScriptConditionEvaluator::new(engine, execution.context),
            execution,
            #[cfg(feature = "script_profiling")]
            started_at: Instant::now(),
            standalone_player_names: None,
        }
    }
}

impl ScriptProgramDriver for ScriptProgramWorld<'_, '_> {
    fn difficulty(&self) -> ScriptDifficulty {
        let current_player = self.engine.get_current_player_name();
        current_player
            .as_deref()
            .and_then(|name| match self.execution.driver.current_player_ai_difficulty(name) {
                ScriptOwnerQuery::Present(difficulty) => Some(difficulty),
                ScriptOwnerQuery::Missing => None,
                ScriptOwnerQuery::Unavailable => {
                    self.engine.standalone_player_difficulty(name).map(|difficulty| {
                        use generals_sim::scripting::frame::AIDifficulty;
                        ScriptDifficulty::from(AIDifficulty::from(difficulty))
                    })
                }
            })
            .unwrap_or_else(|| ScriptDifficulty::from(self.engine.interpreter().frame().difficulty))
    }

    fn condition_team_after(&self, name: &str, after: Option<TeamID>) -> Option<TeamID> {
        match self.execution.driver.condition_team_after(name, after) {
            ScriptOwnerQuery::Present(id) => id,
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => self
                .engine
                .team_factory
                .lock()
                .ok()
                .and_then(|factory| factory.condition_team_after(name, after)),
        }
    }

    fn condition_queries(&mut self) -> impl ScriptConditionQueries + '_ {
        self.engine.condition_queries(self.execution.driver)
    }

    fn evaluate_world_condition(&mut self, condition: &mut Condition) -> GameLogicResult<bool> {
        self.evaluator
            .evaluate_world_condition_truth_with_driver(condition, self.execution.driver)
            .map_err(|e| GameLogicError::Configuration(format!("Script condition error: {e}")))
    }

    fn execute_world_action(&mut self, action: &ScriptAction) -> GameLogicResult<ScriptActionResult> {
        self.dispatcher
            .execute_other_action_with_driver(action, self.execution.driver)
            .map_err(|e| GameLogicError::Configuration(format!("Script action error: {e}")))
    }

    fn receiving_world(&mut self) -> &mut dyn ScriptExecutionDriver {
        self.execution.driver
    }

    fn execute_standalone_player_action(
        &mut self,
        action: PlayerAction<'_>,
    ) -> Result<ScriptActionResult, ScriptError> {
        self.dispatcher
            .execute_standalone_player_action(action, self.execution.driver)
    }

    fn execute_standalone_object_action(&mut self, action: ObjectAction<'_>) -> ScriptActionResult {
        self.dispatcher.execute_standalone_object_action(action)
    }

    fn timer_random_int(&self, low: i32, high: i32) -> i32 {
        script_timer_random_int(self.execution.driver, low, high)
    }

    fn timer_random_real(&self, low: f32, high: f32) -> f32 {
        script_timer_random_real(self.execution.driver, low, high)
    }

    fn script_activation_changed(&self, name: &str, active: bool) {
        self.engine.notify_script_activation(name, active);
    }

    fn priority_template_name(&self, name: &str) -> Option<String> {
        script_priority_template_name(self.execution.driver, name)
    }

    fn priority_templates_with_kind(&self, kind: KindOfMask) -> Option<Vec<String>> {
        script_priority_templates_with_kind(self.execution.driver, kind)
    }

    fn after_action(&mut self) -> GameLogicResult<()> {
        self.execution.driver.after_action()
    }
}

struct ScriptWorldConditions<'a> {
    engine: &'a ScriptEngine,
    driver: &'a mut dyn ScriptExecutionDriver,
}

impl ScriptConditionQueries for ScriptWorldConditions<'_> {
    fn receiving_world(&mut self) -> &mut dyn ScriptExecutionDriver {
        self.driver
    }

    fn can_observe_transport(&self) -> bool {
        // Preserve the legacy evaluator's error for an exclusive observation
        // requested during an existing ScriptEngineInner loan.
        self.engine.inner.try_borrow_mut().is_ok()
    }
}

impl ScriptMediaQueries for ScriptWorldConditions<'_> {
    fn audio_duration_ms(&self, name: &str) -> f32 {
        self.driver
            .script_audio_duration_ms(name)
            .unwrap_or_else(|| self.engine.standalone_audio_duration_ms(name))
    }

    fn music_track_completed(&mut self, name: &str, times: i32) -> Result<bool, ScriptError> {
        if let Some(result) = self.driver.music_track_completed(name, times) {
            return result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()));
        }
        if let Some(Some(finished)) = self.engine.with_execution_read(|engine| {
            engine
                .action_handler()
                .map(|handler| handler.has_music_track_completed(name, times))
        }) {
            return Ok(finished);
        }
        Ok(self.engine.owning_world_services().is_some_and(|world| {
            TheAudio::of(&world).has_music_track_completed(name, times)
        }))
    }
}

impl ScriptPlayerEventQueries for ScriptWorldConditions<'_> {
    fn player_slot(
        &mut self,
        player: &str,
        mask: u32,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<usize> {
        self.driver.script_event_player_slot(player, mask, current_player)
    }

    fn named_source(&self, unit: &str, this_object: Option<u32>) -> Option<u32> {
        match self.driver.named_object_id(unit, this_object) {
            ScriptOwnerQuery::Present(id) => Some(id),
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => None,
        }
    }

    fn science_source(
        &mut self,
        player: &str,
        mask: u32,
        science: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(usize, Option<ScienceType>)> {
        self.driver.player_acquired_science_source(player, mask, science, current_player)
    }
}

impl ScriptCensusQueries for ScriptWorldConditions<'_> {
    fn census(
        &mut self, player: &str, mask: u32, current: Option<&str>, scope: ScriptPlayerCensusScope,
    ) -> ScriptOwnerQuery<(i32, HostScriptPlayerCensus)> {
        self.driver.player_census(player, mask, current, scope)
    }

    fn count_changed_frame(&self) -> Option<u32> {
        match self.driver.object_count_changed_frame() {
            ScriptOwnerQuery::Present(frame) => Some(frame),
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => None,
        }
    }

    fn template_exists(&self, name: &str) -> ScriptOwnerQuery<bool> {
        self.driver.literal_template_exists(name)
    }

    fn template_count(&self, player: i32, names: &[String], ignore_dead: bool) -> ScriptOwnerQuery<Option<i32>> {
        self.driver.player_template_count(player, names, ignore_dead)
    }

    fn player_count_active(&self, player: i32) -> bool {
        self.driver.player_count_active(player)
    }

    fn can_build_any(&mut self, player: &str, mask: u32, names: &[String], current: Option<&str>) -> ScriptOwnerQuery<(u32, bool)> {
        self.driver.skirmish_player_can_build_any(player, mask, names, current)
    }
}

impl ScriptAreaSupplyQueries for ScriptWorldConditions<'_> {
    fn outside_preflight(
        &mut self, player: &str, mask: u32, area: &str, current: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        self.driver.player_outside_area_preflight(player, mask, area, current)
    }

    fn area_value(
        &mut self, player: &str, mask: u32, area: &str, current: Option<&str>, query: ScriptPlayerAreaQuery<'_>,
    ) -> ScriptOwnerQuery<ScriptPlayerAreaCensus> {
        self.driver.player_area_value(player, mask, area, current, query)
    }

    fn supply_source(
        &mut self, player: &str, mask: u32, current: Option<&str>, query: ScriptSupplyQuery,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        self.driver.player_supply_source(player, mask, current, query)
    }
}

fn script_priority_template_name(
    driver: &dyn ScriptExecutionDriver,
    name: &str,
) -> Option<String> {
    match driver.literal_template_exists(name) {
        ScriptOwnerQuery::Present(true) => Some(name.to_owned()),
        ScriptOwnerQuery::Present(false) | ScriptOwnerQuery::Missing => None,
        ScriptOwnerQuery::Unavailable => {
            TheThingFactory::find_template(name).map(|template| template.get_name().to_string())
        }
    }
}

fn script_priority_templates_with_kind(
    driver: &dyn ScriptExecutionDriver,
    kind: KindOfMask,
) -> Option<Vec<String>> {
    match driver.priority_templates_with_kind(kind) {
        ScriptOwnerQuery::Present(names) => Some(names),
        ScriptOwnerQuery::Missing => None,
        ScriptOwnerQuery::Unavailable => {
            legacy_priority_template_names(|template| template.is_any_kind_of_bits(kind.bits()))
        }
    }
}

fn legacy_priority_template_names(
    matches_kind: impl Fn(&EngineThingTemplate) -> bool,
) -> Option<Vec<String>> {
    let factory = get_thing_factory().ok()?;
    let mut current = factory.as_ref()?.first_template().cloned();
    let mut names = Vec::new();
    while let Some(template) = current {
        if matches_kind(&template) {
            names.push(template.get_name().to_string());
        }
        current = template.get_next_template().as_ref().cloned();
    }
    Some(names)
}

pub(crate) fn script_timer_random_int(
    driver: &dyn ScriptExecutionDriver,
    low: i32,
    high: i32,
) -> i32 {
    use game_engine::common::random_value::get_game_logic_random_value;

    match driver.logic_random() {
        Some(random) => random.next_int(low, high),
        None => get_game_logic_random_value(low, high),
    }
}

pub(crate) fn script_timer_random_real(
    driver: &dyn ScriptExecutionDriver,
    low: f32,
    high: f32,
) -> f32 {
    use game_engine::common::random_value::get_game_logic_random_value_real;

    match driver.logic_random() {
        Some(random) => random.next_real(low, high),
        None => get_game_logic_random_value_real(low, high),
    }
}

impl ScriptEngine {
    /// Lend the simulation interpreter and receiving program adapter to one
    /// operation. Legacy fallback actions keep their lexical Core scope.
    pub fn with_script_program<R>(
        &self,
        context: ScriptContext,
        driver: &mut dyn ScriptExecutionDriver,
        execute: impl FnOnce(&ScriptInterpreter, u32, &mut ScriptProgramWorld<'_, '_>) -> R,
    ) -> R {
        let context = std::cell::RefCell::new(context);
        let mut execution = ScriptExecution::new(&context, driver);
        let frame = execution.frame;
        self.with_active(|| {
            execute(&self.runtime, frame, &mut ScriptProgramWorld::new(self, &mut execution))
        })
    }

    pub(crate) fn condition_queries<'a>(
        &'a self,
        driver: &'a mut dyn ScriptExecutionDriver,
    ) -> impl ScriptConditionQueries + 'a {
        ScriptWorldConditions { engine: self, driver }
    }

    pub(crate) fn execute_local_control_action(
        &self,
        action: &ScriptAction,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let handled = self.runtime.try_execute_control_action(
            action,
            |name, active| self.notify_script_activation(name, active),
            |name| script_priority_template_name(driver, name),
            |kind| script_priority_templates_with_kind(driver, kind),
        )?;
        if handled {
            Ok(ScriptActionResult::Success)
        } else {
            Err(ScriptError::ExecutionFailed(format!(
                "Expected interpreter control, got {:?}", action.action_type,
            )))
        }
    }

    /// The standalone side walk releases its roster loans before running actions.
    fn standalone_side_player_names(&self) -> Vec<Option<String>> {
        let Ok(roster) = self.players.try_read() else {
            return Vec::new();
        };
        let count = roster.get_player_count().min(Self::MAX_PLAYER_COUNT);
        (0..count)
            .map(|side| {
                roster.get_player(side as i32).and_then(|player| {
                    let player = player.read().ok()?;
                    self.common
                        .name_key_namespace
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .key_to_name(player.get_player_name_key())
                })
            })
            .collect()
    }

    fn standalone_player_difficulty(&self, name: &str) -> Option<crate::player::GameDifficulty> {
        let key = self.common
            .name_key_namespace
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .name_to_key(name);
        self.players
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name_key(key))
            .and_then(|player| player.read().ok().map(|player| player.get_player_difficulty()))
    }
}
