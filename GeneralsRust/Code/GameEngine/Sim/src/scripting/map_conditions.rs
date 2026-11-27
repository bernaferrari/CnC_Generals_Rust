//! Receiving map, player identity, camera and sighting conditions.

use super::ScriptError;
use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{Condition, ConditionType, Parameter};
use super::skirmish_conditions::resolve_side;
use super::world::{
    ScriptOwnerQuery, ScriptSightingCandidate, ScriptSightingFilter, ScriptSightingObserver,
};
use game_engine::common::game_common::Relationship;

impl ScriptInterpreter {
    pub(super) fn try_evaluate_map_condition<T: ?Sized>(
        &self,
        condition: &mut Condition,
        driver: &mut dyn ScriptExecutionDriver<T>,
    ) -> Result<Option<bool>, ScriptError> {
        use ConditionType::*;

        let kind = condition.get_condition_type();
        match kind {
            // ScriptConditions.cpp:1208, 2730 and 2849.
            MissionAttempts
            | DefunctPlayerSelectedGeneral
            | DefunctPlayerSelectedGeneralFromNamed => Ok(Some(false)),
            CameraMovementFinished => Ok(driver.camera_movement_finished()),
            BridgeBroken | BridgeRepaired => {
                // ScriptConditions.cpp:242–270 reads the aggregate edge first.
                match driver.bridge_damage_states_changed() {
                    None => Ok(None),
                    Some(false) => Ok(Some(false)),
                    Some(true) => {
                        let name = parameter(condition, 0)?.get_string();
                        let status = driver.bridge_status(name, self.script_object_id());
                        Ok(Some(status.is_some_and(|status| match kind {
                            BridgeBroken => status.broken,
                            _ => status.repaired,
                        })))
                    }
                }
            }
            SkirmishNamedAreaExist => {
                // ScriptConditions.cpp:2465 ignores the authored first SIDE.
                let area = parameter(condition, 1)?.get_string();
                Ok(driver.script_area_exists(area, self.current_player().as_deref()))
            }
            SkirmishPlayerFaction | StartPositionIs => {
                let current = self.current_player();
                match resolve_side(condition, 0, current.as_deref(), driver)? {
                    ScriptOwnerQuery::Unavailable => Ok(None),
                    ScriptOwnerQuery::Missing => Ok(Some(false)),
                    ScriptOwnerQuery::Present(player) => {
                        let expected = parameter(condition, 1)?;
                        let matches = if kind == SkirmishPlayerFaction {
                            driver.player_faction(player) == Some(expected.get_string())
                        } else {
                            expected.get_int().checked_sub(1).is_some_and(|index| {
                                driver.player_start_index(player) == Some(index)
                            })
                        };
                        Ok(Some(matches))
                    }
                }
            }
            EnemySighted | TypeSighted => self.try_evaluate_sighting(condition, driver),
            _ => Ok(None),
        }
    }

    fn try_evaluate_sighting<T: ?Sized>(
        &self,
        condition: &mut Condition,
        driver: &mut dyn ScriptExecutionDriver<T>,
    ) -> Result<Option<bool>, ScriptError> {
        let enemy = condition.get_condition_type() == ConditionType::EnemySighted;
        if enemy && condition.get_num_parameters() < 3 {
            return Ok(Some(false));
        }

        // ScriptConditions.cpp:1029–1131 resolves the observer, SIDE, then filter.
        let name = parameter(condition, 0)?.get_string();
        let observer = match driver.sighting_observer(name, self.script_object_id()) {
            ScriptOwnerQuery::Present(id) => id,
            ScriptOwnerQuery::Missing => return Ok(Some(false)),
            ScriptOwnerQuery::Unavailable => return Ok(None),
        };
        let current = self.current_player();
        let player = match resolve_side(condition, 2, current.as_deref(), driver)? {
            ScriptOwnerQuery::Present(player) => player,
            ScriptOwnerQuery::Missing | ScriptOwnerQuery::Unavailable => return Ok(Some(false)),
        };
        let filter = parameter(condition, 1)?;
        let templates;
        let filter = if enemy {
            let relationship = match filter.get_int() {
                0 => Relationship::Enemies,
                1 => Relationship::Neutral,
                2 => Relationship::Allies,
                _ => return Ok(Some(false)),
            };
            ScriptSightingFilter::Relationship(relationship)
        } else {
            templates = self.object_type_candidates(filter.get_string());
            ScriptSightingFilter::Types(&templates)
        };
        Ok(Some(driver.any_sighted_object(observer, player, filter)))
    }
}

impl ScriptSightingFilter<'_> {
    /// Test one live partition member. Relationship is only needed for EnemySighted.
    pub fn accepts(
        self,
        observer: ScriptSightingObserver,
        candidate: ScriptSightingCandidate<'_>,
        owner: u32,
        relationship: impl FnOnce() -> Relationship,
    ) -> bool {
        if candidate.effectively_stealthed
            || candidate.off_map != observer.off_map
            || candidate.owner != Some(owner)
        {
            return false;
        }
        let dx = candidate.position.x - observer.position.x;
        let dz = candidate.position.z - observer.position.z;
        // C++ FROM_CENTER_2D rejects equality. Rust Y is altitude.
        dx * dx + dz * dz < observer.vision_range * observer.vision_range
            && match self {
                Self::Types(names) => names.iter().any(|name| name == candidate.template),
                Self::Relationship(expected) => relationship() == expected,
            }
    }
}

fn parameter(condition: &Condition, index: usize) -> Result<&Parameter, ScriptError> {
    condition
        .get_parameter(index)
        .ok_or_else(|| ScriptError::missing_parameter(index))
}
