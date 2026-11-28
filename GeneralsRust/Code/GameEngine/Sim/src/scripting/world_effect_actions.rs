//! Immediate team, containment, and session effects authored in ScriptActions.cpp.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{
    HostScriptGarrisonEnterExitRequest as Garrison, ScriptNamedCommand, ScriptNamedRuntimeRequest,
    ScriptOwnerQuery, ScriptPlayerTeamRelationship, ScriptTeamRelationship,
};
use super::{ScriptActionResult, ScriptError};
use game_engine::common::game_common::Relationship;

/// Authored REL_ENEMY, REL_NEUTRAL and REL_FRIEND ordinals.
pub fn relationship_from_script_value(value: i32) -> Relationship {
    match value {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

impl ScriptInterpreter {
    /// Apply an effect in the receiving world. `None` leaves the standalone
    /// compatibility implementation responsible for the action.
    pub fn try_execute_world_effect_action(
        &self,
        action: &ScriptAction,
        world: &mut dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptActionResult>, ScriptError> {
        use ScriptActionType::*;

        let parameter = |slot| {
            action
                .get_parameter(slot)
                .ok_or_else(|| ScriptError::missing_parameter(slot))
        };
        let name = |slot| parameter(slot).map(|value| value.get_string());
        let int = |slot| parameter(slot).map(|value| value.get_int());
        let relation = || int(2).map(relationship_from_script_value);

        let kind = action.get_action_type();
        let result = match kind {
            TeamSetState => world.set_team_state(
                name(0)?,
                name(1)?,
                self.calling_team(),
                self.condition_team(),
            ),
            TeamMergeIntoTeam => world.merge_team(
                name(0)?,
                name(1)?,
                self.calling_team(),
                self.condition_team(),
            ),
            TeamTransferToPlayer => world.team_transfer(
                name(0)?,
                name(1)?,
                self.current_player().as_deref(),
                self.calling_team(),
                self.condition_team(),
            ),
            TeamCaptureNearestUnownedFactionUnit => world
                .team_capture_nearest_unowned_faction_unit(
                    name(0)?,
                    self.calling_team(),
                    self.condition_team(),
                ),
            TeamLoadTransports => {
                world.team_load_transports(name(0)?, self.calling_team(), self.condition_team())
            }
            TeamExitAll => {
                world.team_exit_all(name(0)?, self.calling_team(), self.condition_team())
            }
            TeamWaitForNotContainedAll | TeamWaitForNotContainedPartial => {
                let contained = world.team_contained(
                    name(0)?,
                    self.calling_team(),
                    self.condition_team(),
                    kind == TeamWaitForNotContainedAll,
                );
                return Ok(match contained {
                    ScriptOwnerQuery::Present(true) => Some(ScriptActionResult::Pending(1.0)),
                    ScriptOwnerQuery::Present(false) | ScriptOwnerQuery::Missing => {
                        Some(ScriptActionResult::Success)
                    }
                    ScriptOwnerQuery::Unavailable => None,
                });
            }
            NamedEnterNamed | NamedExitAll => {
                let unit = name(0)?;
                let request = if kind == NamedEnterNamed {
                    ScriptNamedCommand::Enter {
                        unit,
                        target: name(1)?,
                    }
                } else {
                    ScriptNamedCommand::Evacuate { unit }
                };
                world.named_command(request, self.script_object_id())
            }
            NamedApplyAttackPrioritySet => {
                world.named_attack_priority(name(0)?, name(1)?, self.script_object_id())
            }
            NamedSetEvacLeftOrRight => world.named_evac_disposition(
                name(0)?,
                action.get_parameter(1).map_or(0, |value| value.get_int()),
                self.script_object_id(),
            ),
            WarehouseSetValue => {
                world.named_warehouse_value(name(0)?, int(1)?, self.script_object_id())
            }
            NamedTransferOwnershipPlayer | NamedFireWeaponFollowingWaypointPath => {
                let unit = name(0)?;
                let target = name(1)?;
                let request = if kind == NamedTransferOwnershipPlayer {
                    ScriptNamedRuntimeRequest::TransferOwnership {
                        unit,
                        player: target,
                    }
                } else {
                    ScriptNamedRuntimeRequest::FireWeaponPath { unit, path: target }
                };
                world.named_runtime(request, self.script_object_id())
            }
            PlayerSetOverrideRelationToTeam | PlayerRemoveOverrideRelationToTeam => {
                let player = name(0)?;
                let team = name(1)?;
                let request = if kind == PlayerSetOverrideRelationToTeam {
                    ScriptPlayerTeamRelationship::Set {
                        player,
                        team,
                        relationship: relation()?,
                    }
                } else {
                    ScriptPlayerTeamRelationship::Remove { player, team }
                };
                world.player_team_relationship_override(
                    request,
                    self.calling_team(),
                    self.condition_team(),
                )
            }
            TeamSetOverrideRelationToTeam
            | TeamRemoveOverrideRelationToTeam
            | TeamSetOverrideRelationToPlayer
            | TeamRemoveOverrideRelationToPlayer
            | TeamRemoveAllOverrideRelations => {
                use ScriptTeamRelationship as Request;
                let source = name(0)?;
                let request = match kind {
                    TeamSetOverrideRelationToTeam => Request::SetTeam {
                        source,
                        target: name(1)?,
                        relationship: relation()?,
                    },
                    TeamRemoveOverrideRelationToTeam => Request::RemoveTeam {
                        source,
                        target: name(1)?,
                    },
                    TeamSetOverrideRelationToPlayer => Request::SetPlayer {
                        source,
                        target: name(1)?,
                        relationship: relation()?,
                    },
                    TeamRemoveOverrideRelationToPlayer => Request::RemovePlayer {
                        source,
                        target: name(1)?,
                    },
                    _ => Request::RemoveAll { source },
                };
                world.team_relationship_override(
                    request,
                    self.calling_team(),
                    self.condition_team(),
                )
            }
            NamedGarrisonSpecificBuilding
            | NamedGarrisonNearestBuilding
            | NamedExitBuilding
            | TeamEnterNamed
            | TeamGarrisonSpecificBuilding
            | ExitSpecificBuilding
            | TeamGarrisonNearestBuilding
            | TeamExitAllBuildings
            | PlayerGarrisonAllBuildings
            | PlayerExitAllBuildings => {
                let first = name(0)?.to_owned();
                let request = match kind {
                    NamedGarrisonSpecificBuilding => Garrison::NamedGarrisonSpecific {
                        unit: first,
                        building: name(1)?.to_owned(),
                    },
                    NamedGarrisonNearestBuilding => Garrison::NamedGarrisonNearest { unit: first },
                    NamedExitBuilding => Garrison::NamedExitBuilding { unit: first },
                    TeamEnterNamed => Garrison::TeamEnter {
                        team: first,
                        dest: name(1)?.to_owned(),
                    },
                    TeamGarrisonSpecificBuilding => Garrison::TeamGarrisonSpecific {
                        team: first,
                        building: name(1)?.to_owned(),
                    },
                    ExitSpecificBuilding => Garrison::ExitSpecificBuilding { building: first },
                    TeamGarrisonNearestBuilding => Garrison::TeamGarrisonNearest { team: first },
                    TeamExitAllBuildings => Garrison::TeamExitAllBuildings { team: first },
                    PlayerGarrisonAllBuildings => Garrison::PlayerGarrisonAll { player: first },
                    _ => Garrison::PlayerExitAll { player: first },
                };
                world.garrison_request(
                    request,
                    self.script_object_id(),
                    self.calling_team(),
                    self.condition_team(),
                )
            }
            OptionsSetDrawiconUiMode => world.set_draw_icon_ui(int(0)? != 0),
            ObjectAllowBonuses => {
                let enable = int(0)? != 0;
                return Ok(world.set_object_difficulty_bonus(enable).map(|()| {
                    // CPP6339–6350 updates existing bodies before recording the rule.
                    self.frame_mut().objects_should_receive_difficulty_bonus = enable;
                    ScriptActionResult::Success
                }));
            }
            EvaSetEnabledDisabled | EnableScoring | DisableScoring => {
                let applied = if kind == EvaSetEnabledDisabled {
                    world.eva_enabled(int(0)? != 0)
                } else {
                    world.scoring_enabled(kind == EnableScoring)
                };
                return Ok(match applied {
                    ScriptOwnerQuery::Present(()) | ScriptOwnerQuery::Missing => {
                        Some(ScriptActionResult::Success)
                    }
                    ScriptOwnerQuery::Unavailable => None,
                });
            }
            _ => return Ok(None),
        };
        result
            .map(|result| {
                result
                    .map(|()| ScriptActionResult::Success)
                    .map_err(|error| ScriptError::ExecutionFailed(error.to_string()))
            })
            .transpose()
    }
}
