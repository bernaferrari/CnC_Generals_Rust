//! Consumable player conditions in ScriptConditions.cpp:1443–1554.
use super::ScriptError;
use super::events::PlayerEventKind;
use super::interpreter::ScriptInterpreter;
use super::program::{Condition, ConditionType, Parameter};
use super::world::ScriptOwnerQuery;
use game_engine::common::rts::science::ScienceType;

/// The receiving world's identities for script-visible event history.
pub trait ScriptPlayerEventQueries {
    fn player_slot(
        &mut self,
        _player: &str,
        _mask: u32,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<usize> {
        ScriptOwnerQuery::Unavailable
    }

    fn named_source(&self, _unit: &str, _this_object: Option<u32>) -> Option<u32> {
        None
    }

    fn science_source(
        &mut self,
        _player: &str,
        _mask: u32,
        _science: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(usize, Option<ScienceType>)> {
        ScriptOwnerQuery::Unavailable
    }
}

fn cache_player_slot(parameter: &mut Parameter, slot: Option<usize>) {
    parameter
        .cache_condition_player_mask(slot.map(|slot| 1u32.checked_shl(slot as u32).unwrap_or(0)));
}

impl ScriptInterpreter {
    pub fn try_evaluate_player_event(
        &self,
        condition: &mut Condition,
        queries: &mut impl ScriptPlayerEventQueries,
    ) -> Result<Option<bool>, ScriptError> {
        let condition_type = condition.get_condition_type();
        let named_event = PlayerEventKind::condition(condition_type);
        if named_event.is_none() && condition_type != ConditionType::PlayerAcquiredScience {
            return Ok(None);
        }

        let (player, other) = condition
            .parameters
            .split_first_mut()
            .ok_or_else(|| ScriptError::missing_parameter(0))?;
        let player = player
            .as_mut()
            .ok_or_else(|| ScriptError::missing_parameter(0))?;
        let event_name = other
            .first()
            .and_then(Option::as_ref)
            .ok_or_else(|| ScriptError::missing_parameter(1))?
            .get_string();
        let current = self.current_player();
        let Some((kind, from_named)) = named_event else {
            return Ok(
                match queries.science_source(
                    player.get_string(),
                    player.get_int() as u32,
                    event_name,
                    current.as_deref(),
                ) {
                    ScriptOwnerQuery::Unavailable => None,
                    ScriptOwnerQuery::Missing => {
                        cache_player_slot(player, None);
                        Some(false)
                    }
                    ScriptOwnerQuery::Present((slot, science)) => {
                        cache_player_slot(player, Some(slot));
                        Some(
                            science.is_some_and(|science| {
                                self.events().has_science(slot, science, true)
                            }),
                        )
                    }
                },
            );
        };

        let source_name = if from_named {
            Some(
                other
                    .get(1)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| ScriptError::missing_parameter(2))?
                    .get_string(),
            )
        } else {
            None
        };
        let slot = match queries.player_slot(
            player.get_string(),
            player.get_int() as u32,
            current.as_deref(),
        ) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Present(slot) => Some(slot),
        };
        // playerFromParam caches before C++ checks the named object.
        cache_player_slot(player, slot);
        let source = if let Some(name) = source_name {
            let Some(source) = queries.named_source(name, self.script_object_id()) else {
                return Ok(Some(false));
            };
            Some(source)
        } else {
            None
        };
        Ok(Some(slot.is_some_and(|slot| {
            self.events()
                .has_player_event(kind, slot, event_name, source, true)
        })))
    }
}
