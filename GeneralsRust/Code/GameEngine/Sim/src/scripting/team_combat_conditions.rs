//! Team conditions over the receiving roster and its live or recorded combat state.

use super::ScriptError;
use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{Condition, ConditionType, Parameter};
use super::skirmish_conditions::resolve_side;
use super::world::{ScriptConditionTeam, ScriptOwnerQuery};
use crate::object::ObjectShroudStatus;

impl ScriptInterpreter {
    pub(super) fn try_evaluate_team_combat_condition<T: ?Sized>(
        &self,
        condition: &mut Condition,
        driver: &mut dyn ScriptExecutionDriver<T>,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;

        let kind = condition.get_condition_type();
        if kind == TeamOwnedByPlayer {
            // CPP1236 resolves SIDE even when no such team exists.
            let current = self.current_player();
            let player = match resolve_side(condition, 1, current.as_deref(), driver)? {
                ScriptOwnerQuery::Present(player) => player,
                ScriptOwnerQuery::Missing => return Ok(Some(false)),
                ScriptOwnerQuery::Unavailable => return Ok(None),
            };
            return Ok(Some(matches!(
                self.condition_team_roster(condition, driver)?,
                ScriptOwnerQuery::Present(team) if team.controller == Some(player)
            )));
        }
        if !matches!(
            kind,
            TeamAttackedByObjecttype
                | TeamAttackedByPlayer
                | TeamDiscovered
                | TeamAllHasObjectStatus
                | TeamSomeHaveObjectStatus
        ) {
            return Ok(None);
        }

        let team = match self.condition_team_roster(condition, driver)? {
            ScriptOwnerQuery::Present(team) => team,
            ScriptOwnerQuery::Missing => return Ok(Some(false)),
            ScriptOwnerQuery::Unavailable => return Ok(None),
        };
        let result = match kind {
            TeamAttackedByObjecttype => {
                let types = self.object_type_candidates(parameter(condition, 1)?.get_string());
                // CPP715 prefers the hit's recorded template even when its object
                // is gone. Only a legacy record without one uses the live attacker.
                team.members.into_iter().any(|member| {
                    let Some(damage) = driver.script_last_damage_source(member) else {
                        return false;
                    };
                    let template = damage
                        .recorded_template
                        .or_else(|| damage.live_attacker.map(|attacker| attacker.template));
                    template.is_some_and(|template| types.iter().any(|name| name == template))
                })
            }
            TeamAttackedByPlayer => {
                let current = self.current_player();
                for member in team.members {
                    let Some(attacker) = driver
                        .script_last_damage_source(member)
                        .and_then(|damage| damage.live_attacker)
                    else {
                        continue;
                    };
                    let controller = attacker.controller;
                    // CPP817 reads SIDE only after finding a still-resolvable attacker.
                    let matched = match resolve_side(condition, 1, current.as_deref(), driver)? {
                        ScriptOwnerQuery::Present(player) => controller == Some(player),
                        ScriptOwnerQuery::Missing => controller.is_none(),
                        ScriptOwnerQuery::Unavailable => false,
                    };
                    if matched {
                        return Ok(Some(true));
                    }
                }
                false
            }
            TeamDiscovered => {
                let current = self.current_player();
                let ScriptOwnerQuery::Present(viewer) =
                    resolve_side(condition, 1, current.as_deref(), driver)?
                else {
                    return Ok(Some(false));
                };
                team.members.into_iter().any(|member| {
                    driver.script_discovery_candidate(member)
                        && matches!(
                            driver.script_object_shroud(member, viewer),
                            Some(ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear)
                        )
                })
            }
            _ => {
                let all = kind == TeamAllHasObjectStatus;
                // CPP2050 stops on the first missing member and considers an
                // empty admitted roster a successful All check.
                for member in team.members {
                    let Some(bits) = driver.script_object_status_bits(member) else {
                        return Ok(Some(false));
                    };
                    let matches = bits & parameter(condition, 1)?.get_object_status().bits() != 0;
                    if matches != all {
                        return Ok(Some(matches));
                    }
                }
                all
            }
        };
        Ok(Some(result))
    }

    fn condition_team_roster<T: ?Sized>(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver<T>,
    ) -> Result<ScriptOwnerQuery<ScriptConditionTeam>, ScriptError> {
        driver
            .condition_team_roster(
                parameter(condition, 0)?.get_string(),
                self.calling_team(),
                self.condition_team(),
            )
            .map_err(|error| ScriptError::EvaluationFailed(error.to_string()))
    }
}

fn parameter(condition: &Condition, index: usize) -> Result<&Parameter, ScriptError> {
    condition
        .get_parameter(index)
        .ok_or_else(|| ScriptError::missing_parameter(index))
}
