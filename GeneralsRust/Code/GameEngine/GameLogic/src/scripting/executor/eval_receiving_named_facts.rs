//! Bound named predicates observe the supplied Object owner before standalone discovery.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};

impl ScriptConditionEvaluator<'_> {
    pub(crate) fn eval_receiving_named_facts(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let kind = condition.get_condition_type();
        if !matches!(
            kind,
            ConditionType::NamedAttackedByObjecttype
                | ConditionType::NamedAttackedByPlayer
                | ConditionType::NamedDiscovered
                | ConditionType::UnitHasObjectStatus
                | ConditionType::BuildingEnteredByPlayer
                | ConditionType::UnitEmptied
        ) {
            return Ok(None);
        }
        let parameter = |index| {
            condition.get_parameter(index).ok_or_else(|| {
                ScriptError::ParameterNotFound(format!("Parameter {index} not found"))
            })
        };
        let name = parameter(if kind == ConditionType::BuildingEnteredByPlayer {
            1
        } else {
            0
        })?
        .get_string();
        let engine = self.context.borrowed_engine();
        let (frame, fact) = match driver.named_query_fact(name, engine.script_object_id()) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
            ScriptOwnerQuery::Present(fact) => fact,
        };
        let value = match kind {
            ConditionType::NamedAttackedByObjecttype => {
                let types = self.resolve_object_types_param(parameter(1)?.get_string());
                !fact.last_damage_template.is_empty()
                    && types.is_in_set(&AsciiString::from(fact.last_damage_template.as_str()))
            }
            ConditionType::UnitHasObjectStatus => {
                let mask = parameter(1)?.get_object_status().bits();
                fact.status_bits & mask != 0
            }
            ConditionType::UnitEmptied => engine.observe_receiving_transport_empty(
                fact.id,
                frame,
                fact.contain_count as usize,
            ),
            _ => {
                let token = parameter(if kind == ConditionType::BuildingEnteredByPlayer {
                    0
                } else {
                    1
                })?
                .get_string();
                let current = engine.get_current_player_name();
                let player = match driver.receiving_player_query_name(token, current.as_deref()) {
                    ScriptOwnerQuery::Unavailable => return Ok(None),
                    ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
                    ScriptOwnerQuery::Present(player) => player,
                };
                match kind {
                    ConditionType::NamedAttackedByPlayer => {
                        !fact.last_damage_player.is_empty() && fact.last_damage_player == player
                    }
                    ConditionType::NamedDiscovered => {
                        !fact.held
                            && !fact.stealthed_hidden
                            && fact.discovered_by.iter().any(|name| name == &player)
                    }
                    ConditionType::BuildingEnteredByPlayer => {
                        fact.has_contain
                            && !fact.player_who_entered.is_empty()
                            && fact.player_who_entered == player
                    }
                    _ => unreachable!(),
                }
            }
        };
        Ok(Some(Self::bool_result(value)))
    }
}

#[cfg(test)]
mod tests {
    use crate::scripting::engine::ScriptEngine;
    #[test]
    fn receiving_transport_history_wraps_repeats_and_is_not_xfer_history() {
        let mut engine = ScriptEngine::new().unwrap();
        assert!(!engine.observe_receiving_transport_empty(7, u32::MAX, 1));
        assert!(engine.observe_receiving_transport_empty(7, 0, 0));
        assert!(engine.observe_receiving_transport_empty(7, 0, 0));
        let tail = engine.snapshot_xfer_tail_with_named_objects(Vec::new());
        engine.restore_receiving_xfer_tail(&tail);
        assert!(!engine.observe_receiving_transport_empty(7, 0, 0));
        assert!(!engine.observe_receiving_transport_empty(7, 5, 1));
        engine.reset();
        assert!(!engine.observe_receiving_transport_empty(7, 6, 0));
    }
}
