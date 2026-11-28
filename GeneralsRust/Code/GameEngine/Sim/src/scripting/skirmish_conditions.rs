//! Skirmish and science conditions over facts from the receiving world.

use super::ScriptError;
use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ComparisonType, Condition, ConditionType, Parameter};
use super::world::{ScriptOwnerQuery, ScriptSkirmishCensus, ScriptSpecialPowerReadiness};
use crate::time::LOGIC_HZ;

impl ScriptInterpreter {
    pub(super) fn try_evaluate_skirmish_condition(
        &self,
        condition: &mut Condition,
        frame: u32,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;

        let kind = condition.get_condition_type();
        if kind == SkirmishSpecialPowerReady {
            return self.evaluate_power_ready(condition, frame, driver);
        }
        if kind == SkirmishUnownedFactionUnitExists {
            // C++2307 ignores the authored SIDE and uses the receiving Neutral player.
            return compare_census(
                condition,
                driver.skirmish_census(ScriptSkirmishCensus::NeutralUnmanned),
            );
        }
        if kind == PlayerDestroyedNBuildingsPlayer {
            // CPP1929 resolves both SIDE operands even when the first fails.
            // The original unfinished condition then returns false.
            let current = self.current_player();
            if let ScriptOwnerQuery::Unavailable =
                resolve_side(condition, 0, current.as_deref(), driver)?
            {
                return Ok(None);
            }
            resolve_side(condition, 2, current.as_deref(), driver)?;
            return Ok(Some(false));
        }
        if !matches!(
            kind,
            SkirmishPlayerHasComparisonGarrisoned
                | SkirmishPlayerHasComparisonCapturedUnits
                | SkirmishPlayerHasDiscoveredPlayer
                | SkirmishPlayerHasBeenAttackedByPlayer
                | SkirmishSuppliesValueWithinDistance
                | SkirmishTechBuildingWithinDistance
                | PlayerCanPurchaseScience
        ) {
            return Ok(None);
        }

        let current_player = self.current_player();
        let current = current_player.as_deref();
        let owner = match resolve_side(condition, 0, current, driver)? {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => return Ok(Some(false)),
            ScriptOwnerQuery::Present(owner) => owner,
        };
        match kind {
            SkirmishPlayerHasComparisonGarrisoned | SkirmishPlayerHasComparisonCapturedUnits => {
                let query = if kind == SkirmishPlayerHasComparisonGarrisoned {
                    ScriptSkirmishCensus::Garrisoned(owner)
                } else {
                    ScriptSkirmishCensus::Captured(owner)
                };
                let result = compare_census(condition, driver.skirmish_census(query))?;
                Ok(Some(result.unwrap_or(false)))
            }
            SkirmishPlayerHasDiscoveredPlayer | SkirmishPlayerHasBeenAttackedByPlayer => {
                let other = match resolve_side(condition, 1, current, driver)? {
                    ScriptOwnerQuery::Present(other) => other,
                    ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => {
                        return Ok(Some(false));
                    }
                };
                Ok(Some(if kind == SkirmishPlayerHasDiscoveredPlayer {
                    matches!(
                        driver.player_discovered_by(owner, other),
                        ScriptOwnerQuery::Present(true)
                    )
                } else {
                    driver.player_was_attacked(owner, other)
                }))
            }
            SkirmishSuppliesValueWithinDistance => {
                let distance = parameter(condition, 1)?.get_real();
                let area = parameter(condition, 2)?.get_string();
                let threshold = parameter(condition, 3)?.get_real();
                let value = driver.supplies_value_in_perimeter(owner, distance, area, current);
                Ok(Some(
                    matches!(value, ScriptOwnerQuery::Present(value) if value > threshold),
                ))
            }
            SkirmishTechBuildingWithinDistance => {
                // CPP2233 resolves SIDE even when the condition is already latched.
                match condition.custom_data {
                    1 => return Ok(Some(true)),
                    -1 => return Ok(Some(false)),
                    _ => {}
                }
                let distance = parameter(condition, 1)?.get_real();
                let area = parameter(condition, 2)?.get_string();
                match driver.tech_building_within_distance(owner, distance, area, current) {
                    ScriptOwnerQuery::Present(found) => {
                        condition.custom_data = if found { 1 } else { -1 };
                        Ok(Some(found))
                    }
                    ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => Ok(Some(false)),
                }
            }
            PlayerCanPurchaseScience => Ok(Some(matches!(
                driver.player_can_purchase_science(owner, parameter(condition, 1)?.get_string()),
                ScriptOwnerQuery::Present(true)
            ))),
            _ => unreachable!("selected receiving-world skirmish or science predicate"),
        }
    }

    fn evaluate_power_ready(
        &self,
        condition: &mut Condition,
        frame: u32,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<Option<bool>, ScriptError> {
        let power = parameter(condition, 1)?;
        let latch = power.get_int();
        if latch == -1 || (latch > 0 && latch as u32 > frame) {
            return Ok(Some(false));
        }

        // CPP1878 looks up the power before resolving SIDE. An unknown
        // definition must latch even when the side is absent.
        match driver.skirmish_special_power_exists(power.get_string()) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Present(false) => {
                parameter_mut(condition, 1)?.int_value = -1;
                return Ok(Some(false));
            }
            ScriptOwnerQuery::Present(true) => {}
        }
        let owner = match resolve_side(condition, 0, self.current_player().as_deref(), driver)? {
            ScriptOwnerQuery::Present(owner) => owner,
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => return Ok(Some(false)),
        };
        let readiness =
            driver.skirmish_special_power_readiness(owner, parameter(condition, 1)?.get_string());
        match readiness {
            ScriptOwnerQuery::Present(ScriptSpecialPowerReadiness::Ready) => Ok(Some(true)),
            ScriptOwnerQuery::Present(ScriptSpecialPowerReadiness::NotReady {
                earliest_ready_frame,
            }) => {
                let retry = frame.wrapping_add(10 * LOGIC_HZ);
                let retry = earliest_ready_frame.map_or(retry, |ready| retry.min(ready));
                parameter_mut(condition, 1)?.int_value = retry as i32;
                Ok(Some(false))
            }
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => Ok(Some(false)),
        }
    }
}

pub(super) fn resolve_side(
    condition: &mut Condition,
    index: usize,
    current: Option<&str>,
    driver: &mut dyn ScriptExecutionDriver,
) -> Result<ScriptOwnerQuery<u32>, ScriptError> {
    let player = parameter_mut(condition, index)?;
    let query = driver.condition_player_id(player.get_string(), player.get_int() as u32, current);
    match query {
        ScriptOwnerQuery::Present(id) => {
            player.cache_condition_player_mask(Some(1u32.checked_shl(id).unwrap_or(0)));
        }
        ScriptOwnerQuery::Missing => player.cache_condition_player_mask(None),
        ScriptOwnerQuery::Unavailable => {}
    }
    Ok(query)
}

fn compare_census(
    condition: &Condition,
    query: ScriptOwnerQuery<i32>,
) -> Result<Option<bool>, ScriptError> {
    match query {
        ScriptOwnerQuery::Unavailable => Ok(None),
        ScriptOwnerQuery::Missing => Ok(Some(false)),
        ScriptOwnerQuery::Present(count) => {
            let comparison = ComparisonType::from_i32(parameter(condition, 1)?.get_int());
            let threshold = parameter(condition, 2)?.get_int();
            Ok(Some(
                comparison.is_some_and(|op| op.compare_i32(count, threshold)),
            ))
        }
    }
}

fn parameter(condition: &Condition, index: usize) -> Result<&Parameter, ScriptError> {
    condition
        .get_parameter(index)
        .ok_or_else(|| ScriptError::missing_parameter(index))
}

fn parameter_mut(condition: &mut Condition, index: usize) -> Result<&mut Parameter, ScriptError> {
    condition
        .parameters
        .get_mut(index)
        .and_then(Option::as_mut)
        .ok_or_else(|| ScriptError::missing_parameter(index))
}
