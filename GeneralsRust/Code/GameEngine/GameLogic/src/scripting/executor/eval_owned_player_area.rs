//! CPP ScriptConditions420/513/2088: an owner-selected polygon census.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery, ScriptPlayerAreaQuery};
impl ScriptConditionEvaluator<'_> {
    pub(super) fn eval_owned_player_area(
        &self,
        condition: &mut Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let kind = condition.get_condition_type();
        if !matches!(
            kind,
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea
                | ConditionType::PlayerHasComparisonUnitKindInTriggerArea
                | ConditionType::SkirmishValueInArea
                | ConditionType::SkirmishPlayerHasUnitsInArea
                | ConditionType::SkirmishPlayerIsOutsideArea
        ) {
            return Ok(None);
        }
        let value_query = kind == ConditionType::SkirmishValueInArea;
        let units_query = matches!(
            kind,
            ConditionType::SkirmishPlayerHasUnitsInArea
                | ConditionType::SkirmishPlayerIsOutsideArea
        );
        let inverted = kind == ConditionType::SkirmishPlayerIsOutsideArea;
        let player = self.get_condition_string_param(condition, 0)?;
        let area = self.get_condition_string_param(
            condition,
            if units_query {
                1
            } else if value_query {
                3
            } else {
                4
            },
        )?;
        let current = self.context.borrowed_engine().get_current_player_name();
        let types;
        let query = match kind {
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea => {
                let name = self.get_condition_string_param(condition, 3)?;
                types = self
                    .resolve_object_types_param(&name)
                    .iter()
                    .map(|name| name.to_string())
                    .collect::<Vec<_>>();
                ScriptPlayerAreaQuery::Templates(&types)
            }
            ConditionType::PlayerHasComparisonUnitKindInTriggerArea => {
                let parameter = condition.get_parameter(3).ok_or_else(|| {
                    ScriptError::ParameterNotFound("Parameter 3 not found".into())
                })?;
                let index = parameter.get_int();
                let selected = (index >= 0)
                    .then(|| crate::common::ALL_KIND_OF.get(index as usize).copied())
                    .flatten()
                    .or_else(|| parse_kind_of(parameter.get_string()));
                let Some(selected) = selected else {
                    return Ok(Some(Self::bool_result(false)));
                };
                ScriptPlayerAreaQuery::Kind(selected)
            }
            _ if units_query => ScriptPlayerAreaQuery::Units,
            _ => ScriptPlayerAreaQuery::Value,
        };
        let (count, changed) =
            match driver.player_area_value(&player, &area, current.as_deref(), query) {
                ScriptOwnerQuery::Unavailable => return Ok(None),
                ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
                ScriptOwnerQuery::Present(value) => value,
            };
        let frame = match driver.object_count_changed_frame() {
            ScriptOwnerQuery::Present(frame) => frame,
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => {
                return Ok(Some(Self::bool_result(false)));
            }
        };
        let frame_changed = if value_query {
            frame != condition.custom_frame
        } else {
            frame > condition.custom_frame
        };
        if condition.custom_data != 0
            && !changed
            && !frame_changed
            && matches!(condition.custom_data, -1 | 1)
        {
            return Ok(Some(Self::bool_result(
                (condition.custom_data == 1) != inverted,
            )));
        }
        let result = if units_query {
            count > 0
        } else {
            Self::compare_i32(
                self.get_condition_comparison_param(condition, 1)?,
                count,
                self.get_condition_int_param(condition, 2)?,
            )
        };
        if kind == ConditionType::PlayerHasComparisonUnitKindInTriggerArea {
            // CPP590 intentionally writes the frame into custom_data only.
            condition.custom_data = frame as i32;
        } else {
            condition.custom_data = if result { 1 } else { -1 };
            condition.custom_frame = frame;
        }
        Ok(Some(Self::bool_result(result != inverted)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;
    struct Owner {
        frame: u32,
        count: i32,
        changed: bool,
        missing: bool,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn player_area_value(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: ScriptPlayerAreaQuery<'_>,
        ) -> ScriptOwnerQuery<(i32, bool)> {
            if self.missing {
                ScriptOwnerQuery::Missing
            } else {
                ScriptOwnerQuery::Present((self.count, self.changed))
            }
        }
        fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
            ScriptOwnerQuery::Present(self.frame)
        }
    }
    fn condition(kind: ConditionType) -> Condition {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        if kind != ConditionType::SkirmishValueInArea {
            condition
                .add_parameter(Parameter::with_string(
                    ParameterType::ObjectType,
                    "Infantry".into(),
                ))
                .unwrap();
        }
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::TriggerArea,
                "Area".into(),
            ))
            .unwrap();
        condition
    }
    #[test]
    fn area_type_and_value_cache_keep_distinct_cpp_frame_gates_and_missing_is_final() {
        let engine = ScriptEngine::new().unwrap();
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let evaluator = ScriptConditionEvaluator::new(&engine, &context);
        for kind in [
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea,
            ConditionType::SkirmishValueInArea,
        ] {
            let mut condition = condition(kind);
            let mut owner = Owner {
                frame: 10,
                count: 1,
                changed: false,
                missing: false,
            };
            assert_eq!(
                evaluator
                    .eval_owned_player_area(&mut condition, &owner)
                    .unwrap(),
                Some(ScriptConditionResult::True)
            );
            owner.frame = 9;
            owner.count = 0;
            let expected = if kind == ConditionType::SkirmishValueInArea {
                ScriptConditionResult::False
            } else {
                ScriptConditionResult::True
            };
            assert_eq!(
                evaluator
                    .eval_owned_player_area(&mut condition, &owner)
                    .unwrap(),
                Some(expected)
            );
            owner.changed = true;
            assert_eq!(
                evaluator
                    .eval_owned_player_area(&mut condition, &owner)
                    .unwrap(),
                Some(ScriptConditionResult::False)
            );
            owner.missing = true;
            owner.count = 1;
            assert_eq!(
                evaluator
                    .eval_owned_player_area(&mut condition, &owner)
                    .unwrap(),
                Some(ScriptConditionResult::False)
            );
        }
    }
    #[test]
    fn area_kind_preserves_cpp_frame_in_custom_data_quirk() {
        let engine = ScriptEngine::new().unwrap();
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let evaluator = ScriptConditionEvaluator::new(&engine, &context);
        let mut condition = condition(ConditionType::PlayerHasComparisonUnitKindInTriggerArea);
        let owner = Owner {
            frame: 18,
            count: 1,
            changed: false,
            missing: false,
        };
        assert_eq!(
            evaluator
                .eval_owned_player_area(&mut condition, &owner)
                .unwrap(),
            Some(ScriptConditionResult::True)
        );
        assert_eq!((condition.custom_data, condition.custom_frame), (18, 0));
    }
}
