//! Conditions that read named objects or teams in the receiving match.

use super::ScriptError;
use super::interpreter::ScriptInterpreter;
use super::program::{ComparisonType, Condition, ConditionType, Parameter};
use super::program_execution::ScriptConditionQueries;
use super::world::{ScriptOwnerQuery, ScriptTeamAreaTransition};

impl ScriptInterpreter {
    pub(super) fn try_evaluate_named_condition(
        &self,
        condition: &mut Condition,
        queries: &mut impl ScriptConditionQueries,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;

        let kind = condition.get_condition_type();
        let this_object = self.script_object_id();
        let driver = queries.receiving_world();

        let result = match kind {
            NamedSelected => {
                let name = parameter(condition, 0)?.get_string();
                match driver.named_selection(name, this_object) {
                    ScriptOwnerQuery::Unavailable => None,
                    ScriptOwnerQuery::Missing => Some(false),
                    ScriptOwnerQuery::Present(selection) => {
                        if selection.multiplayer {
                            return Ok(Some(false));
                        }
                        if !matches!(condition.custom_data, -1 | 1)
                            || condition.custom_frame != selection.changed_frame
                        {
                            condition.custom_data = if selection.selected { 1 } else { -1 };
                            condition.custom_frame = selection.changed_frame;
                        }
                        Some(condition.custom_data == 1)
                    }
                }
            }
            NamedBuildingIsEmpty | NamedHasFreeContainerSlots => {
                let name = parameter(condition, 0)?.get_string();
                answer(
                    driver.named_containment(name, this_object),
                    false,
                    |contain| {
                        if kind == NamedBuildingIsEmpty {
                            contain.count == 0
                        } else {
                            contain.count < contain.max
                        }
                    },
                )
            }
            NamedOwnedByPlayer => {
                let name = parameter(condition, 0)?.get_string();
                let player = parameter(condition, 1)?.get_string();
                boolean(driver.named_owned_by_player(
                    name,
                    player,
                    self.current_player().as_deref(),
                    this_object,
                ))
            }
            NamedDestroyed | NamedDying | NamedTotallyDead | NamedNotDestroyed | NamedCreated => {
                let name = parameter(condition, 0)?.get_string();
                answer(
                    driver.named_lifecycle(name, this_object),
                    false,
                    |state| match kind {
                        NamedDestroyed if state.exists => state.effectively_dead,
                        NamedDestroyed => state.ever_existed,
                        NamedDying => state.exists && state.effectively_dead,
                        NamedTotallyDead => !state.exists && state.ever_existed,
                        NamedCreated => state.exists,
                        _ => state.exists && !state.effectively_dead,
                    },
                )
            }
            UnitHealth => {
                let name = parameter(condition, 0)?.get_string();
                let comparison = ComparisonType::from_i32(parameter(condition, 1)?.get_int());
                let threshold = parameter(condition, 2)?.get_int();
                answer(
                    driver.unit_health_percent(name, this_object),
                    false,
                    |percent| {
                        comparison
                            .is_some_and(|comparison| comparison.compare_i32(percent, threshold))
                    },
                )
            }
            PlayerAllDestroyed => {
                let player = parameter(condition, 0)?.get_string();
                let inverted = condition
                    .get_parameter(1)
                    .is_some_and(|param| param.get_int() != 0);
                answer(
                    driver.player_has_any_objects(player, self.current_player().as_deref()),
                    !inverted,
                    |has_any| !has_any != inverted,
                )
            }
            NamedReachedWaypointsEnd => boolean(driver.named_waypoint_complete(
                parameter(condition, 0)?.get_string(),
                parameter(condition, 1)?.get_string(),
                this_object,
            )),
            NamedInsideArea | NamedOutsideArea | NamedEnteredArea | NamedExitedArea => {
                let unit = parameter(condition, 0)?.get_string();
                let area = parameter(condition, 1)?.get_string();
                let player = self.current_player();
                let current = player.as_deref();
                let query = match kind {
                    NamedEnteredArea => driver.named_entered_area(unit, area, current, this_object),
                    NamedExitedArea => driver.named_exited_area(unit, area, current, this_object),
                    _ => driver.named_inside_area(unit, area, current, this_object),
                };
                let outside = kind == NamedOutsideArea;
                answer(query, outside, |inside| inside != outside)
            }
            NamedAttackedByObjecttype
            | NamedAttackedByPlayer
            | NamedDiscovered
            | UnitHasObjectStatus
            | BuildingEnteredByPlayer
            | UnitEmptied => {
                return self.evaluate_named_fact(condition, queries);
            }
            _ => return Ok(None),
        };
        Ok(result)
    }

    fn evaluate_named_fact(
        &self,
        condition: &Condition,
        queries: &mut impl ScriptConditionQueries,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;

        let kind = condition.get_condition_type();
        let this_object = self.script_object_id();
        let building = kind == BuildingEnteredByPlayer;
        let unit = parameter(condition, usize::from(building))?.get_string();
        let driver = queries.receiving_world();
        if building {
            let query = driver.building_entered_by_player(
                parameter(condition, 0)?.get_string(),
                unit,
                self.current_player().as_deref(),
                this_object,
            );
            if let Some(entered) = boolean(query) {
                return Ok(Some(entered));
            }
        }
        let (frame, fact) = match driver.named_query_fact(unit, this_object) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => return Ok(Some(false)),
            ScriptOwnerQuery::Present(observation) => observation,
        };

        let value = match kind {
            NamedAttackedByObjecttype => {
                let types = self.object_type_candidates(parameter(condition, 1)?.get_string());
                !fact.last_damage_template.is_empty() && types.contains(&fact.last_damage_template)
            }
            UnitHasObjectStatus => {
                let mask = parameter(condition, 1)?.get_object_status().bits();
                fact.status_bits & mask != 0
            }
            UnitEmptied => {
                if !queries.can_observe_transport() {
                    return Err(ScriptError::EvaluationFailed(
                        "Script engine state is borrowed while observing transport history".into(),
                    ));
                }
                self.observe_transport_empty(fact.id, frame, fact.contain_count as usize)
            }
            _ => {
                let token = parameter(condition, usize::from(!building))?.get_string();
                let query =
                    driver.receiving_player_query_name(token, self.current_player().as_deref());
                return Ok(answer(query, false, |player| match kind {
                    NamedAttackedByPlayer => {
                        !fact.last_damage_player.is_empty() && fact.last_damage_player == player
                    }
                    NamedDiscovered => {
                        !fact.held && !fact.stealthed_hidden && fact.discovered_by.contains(&player)
                    }
                    _ => {
                        fact.has_contain
                            && !fact.player_who_entered.is_empty()
                            && fact.player_who_entered == player
                    }
                }));
            }
        };
        Ok(Some(value))
    }

    pub(super) fn try_evaluate_team_condition(
        &self,
        condition: &Condition,
        queries: &mut impl ScriptConditionQueries,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;

        let kind = condition.get_condition_type();
        let calling = self.calling_team();
        let contextual = self.condition_team();
        let driver = queries.receiving_world();

        let result = match kind {
            TeamHasUnits | TeamCreated | TeamDestroyed => {
                let name = parameter(condition, 0)?.get_string();
                if kind == TeamHasUnits {
                    boolean(driver.team_has_units(name, calling, contextual))
                } else {
                    answer(
                        driver.team_presence(name, calling, contextual),
                        false,
                        |presence| {
                            if kind == TeamCreated {
                                presence.created
                            } else {
                                !presence.has_objects
                            }
                        },
                    )
                }
            }
            TeamStateIs | TeamStateIsNot => {
                let name = parameter(condition, 0)?.get_string();
                let expected = parameter(condition, 1)?.get_string();
                answer(
                    driver.team_state(name, calling, contextual),
                    false,
                    |state| (state == expected) == (kind == TeamStateIs),
                )
            }
            TeamReachedWaypointsEnd => boolean(driver.team_waypoint_complete(
                parameter(condition, 0)?.get_string(),
                parameter(condition, 1)?.get_string(),
                calling,
                contextual,
            )),
            TeamEnteredAreaEntirely
            | TeamEnteredAreaPartially
            | TeamExitedAreaEntirely
            | TeamExitedAreaPartially => {
                let transition = match kind {
                    TeamEnteredAreaEntirely => ScriptTeamAreaTransition::AllEntered,
                    TeamEnteredAreaPartially => ScriptTeamAreaTransition::PartiallyEntered,
                    TeamExitedAreaEntirely => ScriptTeamAreaTransition::AllExited,
                    _ => ScriptTeamAreaTransition::PartiallyExited,
                };
                boolean(driver.team_area_transition(
                    transition,
                    parameter(condition, 0)?.get_string(),
                    parameter(condition, 1)?.get_string(),
                    self.current_player().as_deref(),
                    calling,
                    contextual,
                    condition.get_parameter(2).map_or(0, Parameter::get_int) as u32,
                ))
            }
            TeamInsideAreaPartially | TeamInsideAreaEntirely | TeamOutsideAreaEntirely => {
                let query = driver.team_area_membership(
                    parameter(condition, 0)?.get_string(),
                    parameter(condition, 1)?.get_string(),
                    self.current_player().as_deref(),
                    calling,
                    contextual,
                    condition.get_parameter(2).map_or(1, Parameter::get_int) as u32,
                );
                answer(query, kind == TeamOutsideAreaEntirely, |membership| {
                    let all_inside = membership.any_considered && !membership.any_outside;
                    let partially_inside = membership.any_considered && membership.any_inside;
                    match kind {
                        TeamInsideAreaPartially => partially_inside,
                        TeamInsideAreaEntirely => all_inside,
                        _ => !(all_inside || partially_inside),
                    }
                })
            }
            _ => return Ok(None),
        };
        Ok(result)
    }
}

fn parameter(condition: &Condition, index: usize) -> Result<&Parameter, ScriptError> {
    condition
        .get_parameter(index)
        .ok_or_else(|| ScriptError::missing_parameter(index))
}

fn answer<T>(
    query: ScriptOwnerQuery<T>,
    missing: bool,
    found: impl FnOnce(T) -> bool,
) -> Option<bool> {
    match query {
        ScriptOwnerQuery::Unavailable => None,
        ScriptOwnerQuery::Missing => Some(missing),
        ScriptOwnerQuery::Present(value) => Some(found(value)),
    }
}

fn boolean(query: ScriptOwnerQuery<bool>) -> Option<bool> {
    answer(query, false, |value| value)
}
