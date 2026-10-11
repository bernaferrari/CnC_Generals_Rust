//! CPP ScriptConditions player queries use the driving player's completed census.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};

impl ScriptConditionEvaluator<'_> {
    pub(super) fn eval_owned_player_census(
        &self,
        condition: &mut Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let kind = condition.get_condition_type();
        let player_index = match kind {
            ConditionType::PlayerHasCredits => 2,
            ConditionType::BuiltByPlayer => 1,
            ConditionType::PlayerHasPower
            | ConditionType::PlayerHasNoPower
            | ConditionType::PlayerAllBuildfacilitiesDestroyed
            | ConditionType::PlayerHasNOrFewerBuildings
            | ConditionType::PlayerHasObjectComparison
            | ConditionType::PlayerLostObjectType => 0,
            _ => return Ok(None),
        };
        let count_frame = if kind == ConditionType::BuiltByPlayer {
            match driver.object_count_changed_frame() {
                ScriptOwnerQuery::Present(frame) => Some(frame),
                ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => None,
            }
        } else {
            None
        };
        if condition.custom_data != 0 && count_frame == Some(condition.custom_frame) {
            return Ok(Some(Self::bool_result(condition.custom_data == 1)));
        }
        let name = condition
            .get_parameter(player_index)
            .ok_or_else(|| {
                ScriptError::ParameterNotFound(format!("Parameter {player_index} not found"))
            })?
            .get_string();
        let current_player = self.context.borrowed_engine().get_current_player_name();
        let (id, census) = match driver.player_census(name, current_player.as_deref()) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => {
                return Ok(Some(Self::bool_result(
                    kind == ConditionType::PlayerAllBuildfacilitiesDestroyed,
                )));
            }
            ScriptOwnerQuery::Present(census) => census,
        };
        if kind == ConditionType::BuiltByPlayer {
            let template = self.get_condition_string_param(condition, 0)?;
            match driver.literal_template_exists(&template) {
                ScriptOwnerQuery::Unavailable => return Ok(None),
                ScriptOwnerQuery::Missing | ScriptOwnerQuery::Present(false) => {
                    return Ok(Some(Self::bool_result(false)));
                }
                ScriptOwnerQuery::Present(true) => {}
            }
        }
        let count = |type_name: &str, ignore_dead: bool| {
            let types = self.resolve_object_types_param(type_name);
            let names: Vec<String> = types.iter().map(|name| name.to_string()).collect();
            census.count_templates(&names, ignore_dead)
        };
        let value = match kind {
            ConditionType::PlayerHasCredits => Self::compare_i32(
                self.get_condition_comparison_param(condition, 1)?,
                self.get_condition_int_param(condition, 0)?,
                census.money,
            ),
            ConditionType::PlayerHasPower => census.has_sufficient_power(),
            ConditionType::PlayerHasNoPower => !census.has_sufficient_power(),
            ConditionType::PlayerAllBuildfacilitiesDestroyed => !census.has_any_build_facility,
            ConditionType::PlayerHasNOrFewerBuildings => {
                self.get_condition_int_param(condition, 1)? >= census.building_count
            }
            ConditionType::BuiltByPlayer => {
                count(&self.get_condition_string_param(condition, 0)?, false) > 0
            }
            ConditionType::PlayerHasObjectComparison => Self::compare_i32(
                self.get_condition_comparison_param(condition, 1)?,
                count(&self.get_condition_string_param(condition, 3)?, false),
                self.get_condition_int_param(condition, 2)?,
            ),
            ConditionType::PlayerLostObjectType => {
                let object_type = self.get_condition_string_param(condition, 1)?;
                return Ok(Some(self.finish_player_lost_object_type(
                    id,
                    &object_type,
                    count(&object_type, true),
                )));
            }
            _ => unreachable!("selected player census condition"),
        };
        if let Some(frame) = count_frame {
            condition.custom_data = if value { 1 } else { -1 };
            condition.custom_frame = frame;
        }
        Ok(Some(Self::bool_result(value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    struct Owner {
        frame: u32,
        count: i32,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn literal_template_exists(&self, _: &str) -> ScriptOwnerQuery<bool> {
            ScriptOwnerQuery::Present(true)
        }
        fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
            ScriptOwnerQuery::Present(self.frame)
        }
        fn player_census(
            &self,
            _: &str,
            _: Option<&str>,
        ) -> ScriptOwnerQuery<(i32, crate::scripting::HostScriptPlayerCensus)> {
            let mut census = crate::scripting::HostScriptPlayerCensus::default();
            census.template_counts.insert("ranger".into(), self.count);
            ScriptOwnerQuery::Present((0, census))
        }
    }
    #[test]
    fn built_by_player_cache_uses_only_the_driving_object_count_frame() {
        let engine = ScriptEngine::new().unwrap();
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
        let mut condition = Condition::new(ConditionType::BuiltByPlayer);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                "Ranger".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
            .unwrap();
        let mut owner = Owner {
            frame: 10,
            count: 1,
        };
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::True
        );
        assert_eq!((condition.custom_data, condition.custom_frame), (1, 10));
        owner.count = 0;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::True
        );
        owner.frame = 11;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
        assert_eq!((condition.custom_data, condition.custom_frame), (-1, 11));
    }
}
