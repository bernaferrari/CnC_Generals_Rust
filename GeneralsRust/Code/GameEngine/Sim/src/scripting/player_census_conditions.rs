//! Player census and production conditions in ScriptConditions.cpp.
use super::ScriptError;
use super::interpreter::ScriptInterpreter;
use super::program::{ComparisonType, Condition, ConditionType};
use super::world::{HostScriptPlayerCensus, ScriptOwnerQuery, ScriptPlayerCensusScope};

/// Facts supplied by the world running a census or production condition.
pub trait ScriptCensusQueries {
    fn census(
        &mut self,
        _player: &str,
        _mask: u32,
        _current_player: Option<&str>,
        _scope: ScriptPlayerCensusScope,
    ) -> ScriptOwnerQuery<(i32, HostScriptPlayerCensus)> {
        ScriptOwnerQuery::Unavailable
    }

    fn count_changed_frame(&self) -> Option<u32> {
        None
    }

    fn template_exists(&self, _name: &str) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// Present(None) means no candidate exists in the receiving catalog.
    fn template_count(
        &self,
        _player: i32,
        _templates: &[String],
        _ignore_dead: bool,
    ) -> ScriptOwnerQuery<Option<i32>> {
        ScriptOwnerQuery::Unavailable
    }

    fn player_count_active(&self, _player: i32) -> bool {
        true
    }

    fn can_build_any(
        &mut self,
        _player: &str,
        _mask: u32,
        _templates: &[String],
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        ScriptOwnerQuery::Unavailable
    }
}

impl ScriptInterpreter {
    /// C++ objectTypesFromParam distinguishes an empty list from a literal.
    pub fn object_type_candidates(&self, name: &str) -> Vec<String> {
        if name.is_empty() {
            return Vec::new();
        }
        self.frame().object_types.get(name).map_or_else(
            || vec![name.to_owned()],
            |list| list.iter().map(|name| name.as_str().to_owned()).collect(),
        )
    }

    pub fn try_evaluate_player_census(
        &self,
        condition: &mut Condition,
        queries: &mut impl ScriptCensusQueries,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;
        use ScriptPlayerCensusScope::{MoneyAndPower, ObjectCounts, TemplateCounts};
        let kind = condition.get_condition_type();
        if kind == SkirmishPlayerHasPrerequisiteToBuild {
            return self.try_evaluate_build_prerequisites(condition, queries);
        }
        let (player_index, scope) = match kind {
            PlayerHasCredits => (2, MoneyAndPower),
            PlayerHasPower | PlayerHasNoPower | PlayerPowerComparePercent
            | PlayerExcessPowerCompareValue | PlayerHasSciencepurchasepoints => (0, MoneyAndPower),
            BuiltByPlayer => (1, TemplateCounts),
            PlayerHasObjectComparison | PlayerLostObjectType => (0, TemplateCounts),
            PlayerAllBuildfacilitiesDestroyed
            | PlayerHasNOrFewerBuildings
            | PlayerHasNOrFewerFactionBuildings => (0, ObjectCounts),
            _ => return Ok(None),
        };

        let cache_frame = if matches!(kind, BuiltByPlayer | PlayerHasObjectComparison) {
            queries.count_changed_frame()
        } else {
            None
        };
        if cache_frame == Some(condition.custom_frame) {
            match condition.custom_data {
                1 => return Ok(Some(true)),
                -1 => return Ok(Some(false)),
                _ => {}
            }
        }

        let player = condition
            .parameters
            .get_mut(player_index)
            .and_then(Option::as_mut)
            .ok_or_else(|| ScriptError::missing_parameter(player_index))?;
        let current = self.current_player();
        let (id, census) = match queries.census(
            player.get_string(),
            player.get_int() as u32,
            current.as_deref(),
            scope,
        ) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => {
                player.cache_condition_player_mask(None);
                return Ok(Some(matches!(
                    kind,
                    PlayerAllBuildfacilitiesDestroyed | PlayerHasNoPower
                )));
            }
            ScriptOwnerQuery::Present((id, census)) => {
                player.cache_condition_player_mask(Some(1u32.checked_shl(id as u32).unwrap_or(0)));
                (id, census)
            }
        };
        let parameter = |i| {
            condition
                .get_parameter(i)
                .ok_or_else(|| ScriptError::missing_parameter(i))
        };
        let comparison =
            |i| -> Result<_, ScriptError> { Ok(ComparisonType::from_i32(parameter(i)?.get_int())) };
        let count = |name, ignore_dead| {
            let types = self.object_type_candidates(name);
            match queries.template_count(id, &types, ignore_dead) {
                ScriptOwnerQuery::Present(count) => count,
                ScriptOwnerQuery::Missing => None,
                ScriptOwnerQuery::Unavailable => {
                    (!types.is_empty()).then(|| census.count_templates(&types, ignore_dead))
                }
            }
        };
        let value = match kind {
            PlayerHasCredits => {
                let requested = parameter(0)?.get_int() as u32;
                comparison(1)?.is_some_and(|op| op.compare(requested, census.money as u32))
            }
            PlayerHasPower => census.has_sufficient_power(),
            PlayerHasNoPower => !census.has_sufficient_power(),
            PlayerPowerComparePercent => {
                let threshold = parameter(2)?.get_int() as f32 / 100.0;
                comparison(1)?.is_some_and(|op| op.compare(census.supply_ratio(), threshold))
            }
            PlayerExcessPowerCompareValue => {
                let threshold = parameter(2)?.get_int();
                comparison(1)?.is_some_and(|op| op.compare_i32(census.excess_power(), threshold))
            }
            PlayerHasSciencepurchasepoints => {
                if let Some(threshold) = condition.get_parameter(2) {
                    // Historical Rust scripts also accepted a comparison operand.
                    comparison(1)?.unwrap_or(ComparisonType::Equal)
                        .compare_i32(census.science_purchase_points, threshold.get_int())
                } else {
                    census.science_purchase_points >= parameter(1)?.get_int()
                }
            }
            PlayerAllBuildfacilitiesDestroyed => !census.has_any_build_facility,
            PlayerHasNOrFewerBuildings => parameter(1)?.get_int() >= census.building_count,
            PlayerHasNOrFewerFactionBuildings => {
                parameter(1)?.get_int() >= census.faction_building_count
            }
            BuiltByPlayer => {
                let name = parameter(0)?.get_string();
                match queries.template_exists(name) {
                    ScriptOwnerQuery::Unavailable => return Ok(None),
                    ScriptOwnerQuery::Missing | ScriptOwnerQuery::Present(false) => {
                        return Ok(Some(false));
                    }
                    ScriptOwnerQuery::Present(true) => {}
                }
                let Some(sum) = count(name, false) else {
                    return Ok(Some(false));
                };
                sum != 0
            }
            PlayerHasObjectComparison => {
                let sum = count(parameter(3)?.get_string(), false).unwrap_or(0);
                let requested = parameter(2)?.get_int();
                comparison(1)?.is_some_and(|op| op.compare_i32(sum, requested))
            }
            PlayerLostObjectType => {
                let name = parameter(1)?.get_string();
                let Some(sum) = count(name, true) else {
                    return Ok(Some(false));
                };
                let active = queries.player_count_active(id);
                return Ok(Some(
                    self.frame_mut().observe_object_count(id, name, sum, active),
                ));
            }
            _ => unreachable!("selected player census condition"),
        };
        if let Some(frame) = cache_frame {
            condition.custom_data = if value { 1 } else { -1 };
            condition.custom_frame = frame;
        }
        Ok(Some(value))
    }

    fn try_evaluate_build_prerequisites(
        &self,
        condition: &mut Condition,
        queries: &mut impl ScriptCensusQueries,
    ) -> Result<Option<bool>, ScriptError> {
        let types = self.object_type_candidates(
            condition
                .get_parameter(1)
                .ok_or_else(|| ScriptError::missing_parameter(1))?
                .get_string(),
        );
        let player = condition
            .parameters
            .first_mut()
            .and_then(Option::as_mut)
            .ok_or_else(|| ScriptError::missing_parameter(0))?;
        let current = self.current_player();
        match queries.can_build_any(
            player.get_string(),
            player.get_int() as u32,
            &types,
            current.as_deref(),
        ) {
            ScriptOwnerQuery::Unavailable => Ok(None),
            ScriptOwnerQuery::Missing => {
                player.cache_condition_player_mask(None);
                Ok(Some(false))
            }
            ScriptOwnerQuery::Present((mask, can_build)) => {
                player.cache_condition_player_mask(Some(mask));
                Ok(Some(can_build))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::program::{Parameter, ParameterType};

    struct Counts {
        count: Option<i32>,
        active: bool,
    }

    impl ScriptCensusQueries for Counts {
        fn census(
            &mut self,
            _: &str,
            _: u32,
            _: Option<&str>,
            scope: ScriptPlayerCensusScope,
        ) -> ScriptOwnerQuery<(i32, HostScriptPlayerCensus)> {
            assert_eq!(scope, ScriptPlayerCensusScope::TemplateCounts);
            ScriptOwnerQuery::Present((1, HostScriptPlayerCensus::default()))
        }

        fn template_count(
            &self,
            id: i32,
            names: &[String],
            ignore_dead: bool,
        ) -> ScriptOwnerQuery<Option<i32>> {
            assert_eq!(id, 1);
            assert_eq!(names, ["Tank"]);
            assert!(ignore_dead);
            ScriptOwnerQuery::Present(self.count)
        }

        fn player_count_active(&self, id: i32) -> bool {
            assert_eq!(id, 1);
            self.active
        }
    }

    #[test]
    fn lost_type_history_is_local_and_only_samples_a_resolved_type() {
        let mut interpreter = ScriptInterpreter::new(2);
        let peer = ScriptInterpreter::new(2);
        let observe = |interpreter: &ScriptInterpreter, count, active| {
            let mut condition = Condition::new(ConditionType::PlayerLostObjectType);
            for (kind, value) in [
                (ParameterType::Side, "Owner"),
                (ParameterType::ObjectType, "Tank"),
            ] {
                condition
                    .add_parameter(Parameter::with_string(kind, value.into()))
                    .unwrap();
            }
            interpreter
                .try_evaluate_player_census(&mut condition, &mut Counts { count, active })
                .unwrap()
                .unwrap()
        };
        assert!(!observe(&interpreter, Some(3), true));
        assert!(!observe(&interpreter, None, true));
        assert!(!observe(&peer, Some(1), true));
        assert!(!observe(&interpreter, Some(0), false));
        assert!(observe(&interpreter, Some(1), true));
        assert!(!observe(&interpreter, Some(1), true));
        interpreter.reset();
        assert!(!observe(&interpreter, Some(0), true));
        assert!(observe(&peer, Some(0), true));
    }
}
