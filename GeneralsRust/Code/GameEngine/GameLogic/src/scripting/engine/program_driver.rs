// Bind the script interpreter to the world receiving its queries and effects.

use crate::scripting::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator,
};

/// C++ executeScript scopes the condition team across the entire live walk.
struct ConditionContextRestore<'a> {
    engine: &'a ScriptEngine,
    team: Option<TeamID>,
}

impl Drop for ConditionContextRestore<'_> {
    fn drop(&mut self) {
        self.engine.lock_inner_mut().condition_team = self.team;
    }
}

struct ScriptProgramWorld<'engine, 'context> {
    engine: &'engine ScriptEngine,
    execution: &'engine mut ScriptExecution<'context>,
    dispatcher: ScriptActionDispatcher<'engine>,
    evaluator: ScriptConditionEvaluator<'engine>,
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
        }
    }

    fn run_script(&mut self, script: &mut Script) -> GameLogicResult<()> {
        let _restore = ConditionContextRestore {
            engine: self.engine,
            team: self.engine.with_inner(|inner| inner.condition_team),
        };
        program_execution::execute_script(script, self.execution.frame, self)
    }
}

impl ScriptProgramDriver for ScriptProgramWorld<'_, '_> {
    fn difficulty(&self) -> ScriptDifficulty {
        use crate::player::GameDifficulty;

        let current_player = self.engine.with_inner(|inner| inner.current_player.clone());
        let difficulty = current_player
            .as_deref()
            .and_then(|name| {
                crate::player::player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(name))
                    .and_then(|p| p.read().ok().map(|p| p.get_player_difficulty()))
            })
            .unwrap_or(GameDifficulty::Normal);
        match difficulty {
            GameDifficulty::Easy => ScriptDifficulty::Easy,
            GameDifficulty::Normal => ScriptDifficulty::Normal,
            GameDifficulty::Hard | GameDifficulty::Brutal => ScriptDifficulty::Hard,
        }
    }

    fn condition_team_after(&self, name: &str, after: Option<TeamID>) -> Option<TeamID> {
        match self.execution.driver.condition_team_after(name, after) {
            ScriptOwnerQuery::Present(id) => id,
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => get_team_factory()
                .lock()
                .ok()
                .and_then(|factory| factory.condition_team_after(name, after)),
        }
    }

    fn set_condition_team(&mut self, team: Option<TeamID>) {
        self.engine.lock_inner_mut().condition_team = team;
    }

    fn evaluate_condition(&mut self, condition: &mut Condition) -> GameLogicResult<bool> {
        self.evaluator
            .evaluate_condition_truth_with_driver(condition, self.execution.driver)
            .map_err(|e| GameLogicError::Configuration(format!("Script condition error: {e}")))
    }

    fn execute_action(&mut self, action: &ScriptAction) -> GameLogicResult<ScriptActionResult> {
        self.dispatcher
            .execute_action_with_driver(action, self.execution.driver)
            .map_err(|e| GameLogicError::Configuration(format!("Script action error: {e}")))
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
