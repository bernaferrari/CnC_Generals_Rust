// Bind the script interpreter to the world receiving its queries and effects.

use crate::scripting::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator,
};

struct ScriptProgramWorld<'engine, 'context> {
    engine: &'engine ScriptEngine,
    execution: &'engine mut ScriptExecution<'context>,
    dispatcher: ScriptActionDispatcher<'engine>,
    evaluator: ScriptConditionEvaluator<'engine>,
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
            standalone_player_names: None,
        }
    }

    fn run_script(&mut self, script: &mut Script) -> GameLogicResult<()> {
        self.engine.runtime.execute_script(script, self.execution.frame, self)
    }
}

impl SideScriptDriver for ScriptProgramWorld<'_, '_> {
    fn side_player(&mut self, side: usize) -> Option<String> {
        match self.execution.driver.side_script_current_player(side) {
            ScriptOwnerQuery::Present(player) => player,
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => self
                .standalone_player_names
                .get_or_insert_with(|| self.engine.standalone_side_player_names())
                .get(side)
                .cloned()
                .flatten(),
        }
    }

    fn execute_side_script(&mut self, script: &mut Script) -> GameLogicResult<()> {
        self.run_script(script)
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
                    self.engine.standalone_player_difficulty(name).map(script_difficulty)
                }
            })
            .unwrap_or_else(|| script_difficulty(self.engine.get_global_difficulty()))
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

    fn evaluate_world_condition(&mut self, condition: &mut Condition) -> GameLogicResult<bool> {
        self.evaluator
            .evaluate_condition_truth_with_driver(condition, self.execution.driver)
            .map_err(|e| GameLogicError::Configuration(format!("Script condition error: {e}")))
    }

    fn execute_world_action(&mut self, action: &ScriptAction) -> GameLogicResult<ScriptActionResult> {
        self.dispatcher
            .execute_action_with_driver(action, self.execution.driver)
            .map_err(|e| GameLogicError::Configuration(format!("Script action error: {e}")))
    }

    fn timer_random_int(&self, low: i32, high: i32) -> i32 {
        script_timer_random_int(self.execution.driver, low, high)
    }

    fn timer_random_real(&self, low: f32, high: f32) -> f32 {
        script_timer_random_real(self.execution.driver, low, high)
    }

    fn call_subroutine(&mut self, name: &str) -> GameLogicResult<()> {
        self.engine
            .execute_subroutine_with_execution(name, self.execution)
            .map(|_| ())
    }

    fn after_action(&mut self) -> GameLogicResult<()> {
        self.execution.driver.after_action()?;
        self.engine
            .observe_object_count_changed_frame(self.execution.driver);
        Ok(())
    }
}

pub(crate) fn script_timer_random_int(
    driver: &dyn ScriptExecutionDriver<ScriptEngine>,
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
    driver: &dyn ScriptExecutionDriver<ScriptEngine>,
    low: f32,
    high: f32,
) -> f32 {
    use game_engine::common::random_value::get_game_logic_random_value_real;

    match driver.logic_random() {
        Some(random) => random.next_real(low, high),
        None => get_game_logic_random_value_real(low, high),
    }
}

fn script_difficulty(difficulty: crate::player::GameDifficulty) -> ScriptDifficulty {
    use crate::player::GameDifficulty;

    match difficulty {
        GameDifficulty::Easy => ScriptDifficulty::Easy,
        GameDifficulty::Normal => ScriptDifficulty::Normal,
        GameDifficulty::Hard | GameDifficulty::Brutal => ScriptDifficulty::Hard,
    }
}

impl ScriptEngine {
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
