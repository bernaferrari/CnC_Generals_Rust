//! Original ScriptEngine.cpp:7558–7604 condition admission on the driving owner.
use super::*;
use crate::scripting::executor::{ScriptConditionEvaluator};
use generals_sim::scripting::context::ScriptContext;

impl ScriptEngine {
    /// Evaluate the supplied runtime script using this interpreter and this
    /// operation's world inputs. Cached condition state remains in `script`.
    /// Calling context is restored; conditionTeam retains its separate CPP
    /// lifetime. No engine/world is published through an active slot or TLS.
    pub fn evaluate_conditions_with_driver(
        &self,
        script: &mut Script,
        team_id: Option<TeamID>,
        player_name: Option<&str>,
        context: ScriptContext,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> bool {
        self.evaluate_conditions_with_driver_checked(script, team_id, player_name, context, driver)
            .unwrap_or(false)
    }

    /// Checked form used by synchronous owner callbacks that preserve the
    /// original condition-error logging and continuation policy.
    pub(crate) fn evaluate_conditions_with_driver_checked(
        &self,
        script: &mut Script,
        team_id: Option<TeamID>,
        player_name: Option<&str>,
        context: ScriptContext,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<bool, String> {
        let saved_player = self.get_current_player_name();
        let _restore = self.runtime.preserve_team_call();
        // CPP thisTeam replaces the explicit player with its controller;
        // a null controller falls back to the interpreter's current player.
        let player = if let Some(id) = team_id {
            match driver.team_current_player(id) {
                ScriptOwnerQuery::Present(player) => {
                    player.or_else(|| saved_player)
                }
                ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => {
                    log::warn!("Condition evaluation has no driving controller for team {id}");
                    return Ok(false);
                }
            }
        } else {
            player_name
                .map(str::to_owned)
                .or_else(|| saved_player)
        };
        self.runtime.set_team_call(team_id, player);
        self.evaluate_condition_tree(script, context, driver)
    }

    pub(crate) fn evaluate_standalone_conditions_checked(
        &self,
        script: &mut Script,
        team_id: Option<TeamID>,
        player_name: Option<&str>,
        frame: u32,
    ) -> Result<bool, String> {
        let player = team_id
            .and_then(|id| self.standalone_team_player(id))
            .or_else(|| player_name.map(str::to_owned))
            .or_else(|| self.get_current_player_name());
        self.with_active(|| {
            let _restore = self.runtime.enter_team_call(team_id, player);
            self.evaluate_condition_tree(
                script,
                ScriptContext::at_frame(frame),
                &mut CanonicalScriptExecutionDriver,
            )
        })
    }

    fn evaluate_condition_tree(
        &self,
        script: &mut Script,
        context: ScriptContext,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<bool, String> {
        let state = std::cell::RefCell::new(context);
        let result = if let Some(condition) = script.condition.as_deref_mut() {
            ScriptConditionEvaluator::new(self, &state)
                .evaluate_or_condition_with_driver(condition, driver)
                .map_err(|error| error.to_string())?
        } else {
            false
        };
        Ok(result)
    }
}
