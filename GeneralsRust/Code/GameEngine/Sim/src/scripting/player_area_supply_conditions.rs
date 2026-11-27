//! Player polygon and supply conditions from ScriptConditions.cpp.
use super::ScriptError;
use super::interpreter::ScriptInterpreter;
use super::player_census_conditions::ScriptCensusQueries;
use super::program::{ComparisonType, Condition, ConditionType};
use super::world::{
    ScriptOwnerQuery, ScriptPlayerAreaCensus, ScriptPlayerAreaQuery, ScriptSupplyQuery,
};
use crate::kind_of::classic::{ALL_KIND_OF, parse_kind_of};
use crate::time::LOGIC_HZ;

/// Spatial and AI facts lent by the receiving world for one authored condition.
pub trait ScriptAreaSupplyQueries: ScriptCensusQueries {
    /// The outside predicate resolves its player before checking the polygon.
    fn outside_preflight(
        &mut self,
        _player: &str,
        _mask: u32,
        _area: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        ScriptOwnerQuery::Unavailable
    }

    fn area_value(
        &mut self,
        _player: &str,
        _mask: u32,
        _area: &str,
        _current_player: Option<&str>,
        _query: ScriptPlayerAreaQuery<'_>,
    ) -> ScriptOwnerQuery<ScriptPlayerAreaCensus> {
        ScriptOwnerQuery::Unavailable
    }

    fn supply_source(
        &mut self,
        _player: &str,
        _mask: u32,
        _current_player: Option<&str>,
        _query: ScriptSupplyQuery,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        ScriptOwnerQuery::Unavailable
    }
}

impl ScriptInterpreter {
    pub fn try_evaluate_area_supply_condition(
        &self,
        condition: &mut Condition,
        frame: u32,
        queries: &mut impl ScriptAreaSupplyQueries,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;
        match condition.get_condition_type() {
            PlayerHasComparisonUnitTypeInTriggerArea
            | PlayerHasComparisonUnitKindInTriggerArea
            | SkirmishValueInArea
            | SkirmishPlayerHasUnitsInArea
            | SkirmishPlayerIsOutsideArea => self.evaluate_player_area(condition, queries),
            SupplySourceSafe | SupplySourceAttacked => {
                self.evaluate_player_supply(condition, frame, queries)
            }
            _ => Ok(None),
        }
    }

    fn evaluate_player_area(
        &self,
        condition: &mut Condition,
        queries: &mut impl ScriptAreaSupplyQueries,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;
        let kind = condition.get_condition_type();
        let (player, other) = condition
            .parameters
            .split_first_mut()
            .ok_or_else(|| ScriptError::missing_parameter(0))?;
        let player = player
            .as_mut()
            .ok_or_else(|| ScriptError::missing_parameter(0))?;
        let parameter = |index| {
            other
                .get(index - 1)
                .and_then(Option::as_ref)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let templates;
        let (area_index, query) = match kind {
            PlayerHasComparisonUnitTypeInTriggerArea => {
                templates = self.object_type_candidates(parameter(3)?.get_string());
                (4, ScriptPlayerAreaQuery::Templates(&templates))
            }
            PlayerHasComparisonUnitKindInTriggerArea => {
                let selected = parameter(3)?;
                // KindOf parameters are normalized to this table by the chunk codec.
                let kind = ALL_KIND_OF
                    .get(selected.get_int() as usize)
                    .copied()
                    .or_else(|| parse_kind_of(selected.get_string()));
                let Some(kind) = kind else {
                    return Ok(Some(false));
                };
                (4, ScriptPlayerAreaQuery::Kind(kind))
            }
            SkirmishValueInArea => (3, ScriptPlayerAreaQuery::Value),
            SkirmishPlayerHasUnitsInArea | SkirmishPlayerIsOutsideArea => {
                (1, ScriptPlayerAreaQuery::Units)
            }
            _ => unreachable!("selected player polygon condition"),
        };
        let area = parameter(area_index)?.get_string();
        let current_player = self.current_player();
        let inverted = kind == SkirmishPlayerIsOutsideArea;
        if inverted {
            match queries.outside_preflight(
                player.get_string(),
                player.get_int() as u32,
                area,
                current_player.as_deref(),
            ) {
                ScriptOwnerQuery::Unavailable => return Ok(None),
                ScriptOwnerQuery::Missing => {
                    player.cache_condition_player_mask(None);
                    return Ok(Some(false));
                }
                ScriptOwnerQuery::Present((mask, area_exists)) => {
                    player.cache_condition_player_mask(Some(mask));
                    if !area_exists {
                        return Ok(Some(false));
                    }
                }
            }
        }
        let (count, changed) = match queries.area_value(
            player.get_string(),
            player.get_int() as u32,
            area,
            current_player.as_deref(),
            query,
        ) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => return Ok(Some(inverted)),
            ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::MissingPlayer) => {
                player.cache_condition_player_mask(None);
                return Ok(Some(inverted));
            }
            ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::MissingArea { player_mask }) => {
                player.cache_condition_player_mask(Some(player_mask));
                return Ok(Some(inverted));
            }
            ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::Measured {
                player_mask,
                value,
                membership_changed,
            }) => {
                player.cache_condition_player_mask(Some(player_mask));
                (value, membership_changed)
            }
        };
        let Some(frame) = queries.count_changed_frame() else {
            return Ok(Some(false));
        };
        let frame_changed = if kind == SkirmishValueInArea {
            frame != condition.custom_frame
        } else {
            frame > condition.custom_frame
        };
        if !changed && !frame_changed && matches!(condition.custom_data, -1 | 1) {
            return Ok(Some((condition.custom_data == 1) != inverted));
        }
        let result = if matches!(
            kind,
            SkirmishPlayerHasUnitsInArea | SkirmishPlayerIsOutsideArea
        ) {
            count > 0
        } else {
            let target = parameter(2)?.get_int();
            ComparisonType::from_i32(parameter(1)?.get_int())
                .is_some_and(|cmp| cmp.compare_i32(count, target))
        };
        if kind == PlayerHasComparisonUnitKindInTriggerArea {
            // C++ writes the frame into custom_data and leaves custom_frame alone.
            condition.custom_data = frame as i32;
        } else {
            condition.custom_data = if result { 1 } else { -1 };
            condition.custom_frame = frame;
        }
        Ok(Some(result != inverted))
    }

    fn evaluate_player_supply(
        &self,
        condition: &mut Condition,
        frame: u32,
        queries: &mut impl ScriptAreaSupplyQueries,
    ) -> Result<Option<bool>, ScriptError> {
        let safe_query = condition.get_condition_type() == ConditionType::SupplySourceSafe;
        let query = if safe_query {
            if frame <= condition.custom_frame && matches!(condition.custom_data, -1 | 1) {
                return Ok(Some(condition.custom_data == 1));
            }
            ScriptSupplyQuery::Safe {
                minimum_cash: condition
                    .get_parameter(1)
                    .ok_or_else(|| ScriptError::missing_parameter(1))?
                    .get_int(),
            }
        } else {
            ScriptSupplyQuery::Attacked
        };
        let player = condition
            .parameters
            .first_mut()
            .and_then(Option::as_mut)
            .ok_or_else(|| ScriptError::missing_parameter(0))?;
        let current_player = self.current_player();
        let result = match queries.supply_source(
            player.get_string(),
            player.get_int() as u32,
            current_player.as_deref(),
            query,
        ) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => {
                player.cache_condition_player_mask(None);
                None
            }
            ScriptOwnerQuery::Present((mask, result)) => {
                player.cache_condition_player_mask(Some(mask));
                Some(result)
            }
        };
        if safe_query {
            condition.custom_frame = frame.wrapping_add(2 * LOGIC_HZ);
            if let Some(result) = result {
                condition.custom_data = if result { 1 } else { -1 };
            }
        }
        Ok(Some(result.unwrap_or(false)))
    }
}
