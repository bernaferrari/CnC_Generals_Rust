//! Authored command-button and skirmish orders executed on the receiving world.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ComparisonType, ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandButtonAction<'a> {
    Named {
        unit: &'a str,
        button: &'a str,
    },
    NamedOnNamed {
        unit: &'a str,
        button: &'a str,
        target: &'a str,
    },
    NamedAtWaypoint {
        unit: &'a str,
        button: &'a str,
        waypoint: &'a str,
    },
    NamedUsingWaypointPath {
        unit: &'a str,
        button: &'a str,
        path: &'a str,
    },
    Team {
        team: &'a str,
        button: &'a str,
    },
    TeamOnNamed {
        team: &'a str,
        button: &'a str,
        target: &'a str,
    },
    TeamAtWaypoint {
        team: &'a str,
        button: &'a str,
        waypoint: &'a str,
    },
    TeamOnNearestEnemy {
        team: &'a str,
        button: &'a str,
    },
    TeamOnNearestGarrisonedBuilding {
        team: &'a str,
        button: &'a str,
    },
    TeamOnNearestKindof {
        team: &'a str,
        button: &'a str,
        kindof: &'a str,
    },
    TeamOnNearestEnemyBuilding {
        team: &'a str,
        button: &'a str,
    },
    TeamOnNearestEnemyBuildingClass {
        team: &'a str,
        button: &'a str,
        kindof: &'a str,
    },
    TeamOnNearestObjectType {
        team: &'a str,
        button: &'a str,
        object_type: &'a str,
    },
}

impl<'a> CommandButtonAction<'a> {
    pub fn named_target(&self) -> Option<&'a str> {
        match *self {
            Self::NamedOnNamed { target, .. } | Self::TeamOnNamed { target, .. } => Some(target),
            _ => None,
        }
    }

    pub fn waypoint(&self) -> Option<&'a str> {
        match *self {
            Self::NamedAtWaypoint { waypoint, .. } | Self::TeamAtWaypoint { waypoint, .. } => {
                Some(waypoint)
            }
            _ => None,
        }
    }
}

pub enum ScriptAbilityAction<'a> {
    CommandButton(CommandButtonAction<'a>),
    Nearest(CommandButtonAction<'a>),
    Partial {
        team: &'a str,
        button: &'a str,
        percentage: f32,
    },
    AttackNearestGroup {
        team: &'a str,
        comparison: Option<ComparisonType>,
        value: i32,
    },
    MostValuable {
        team: &'a str,
        button: &'a str,
        range: f32,
    },
}

impl<'a> ScriptAbilityAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use CommandButtonAction as Button;

        let param = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let name = |index| param(index).map(|p| p.get_string());
        let decoded = match action.get_action_type() {
            ScriptActionType::NamedUseCommandbuttonAbility => Self::CommandButton(Button::Named {
                unit: name(0)?,
                button: name(1)?,
            }),
            ScriptActionType::NamedUseCommandbuttonAbilityOnNamed => {
                Self::CommandButton(Button::NamedOnNamed {
                    unit: name(0)?,
                    button: name(1)?,
                    target: name(2)?,
                })
            }
            ScriptActionType::NamedUseCommandbuttonAbilityAtWaypoint => {
                Self::CommandButton(Button::NamedAtWaypoint {
                    unit: name(0)?,
                    button: name(1)?,
                    waypoint: name(2)?,
                })
            }
            ScriptActionType::NamedUseCommandbuttonAbilityUsingWaypointPath => {
                Self::CommandButton(Button::NamedUsingWaypointPath {
                    unit: name(0)?,
                    button: name(1)?,
                    path: name(2)?,
                })
            }
            ScriptActionType::TeamUseCommandbuttonAbility => Self::CommandButton(Button::Team {
                team: name(0)?,
                button: name(1)?,
            }),
            ScriptActionType::TeamUseCommandbuttonAbilityOnNamed
            | ScriptActionType::TeamAllUseCommandbuttonOnNamed => {
                Self::CommandButton(Button::TeamOnNamed {
                    team: name(0)?,
                    button: name(1)?,
                    target: name(2)?,
                })
            }
            ScriptActionType::TeamUseCommandbuttonAbilityAtWaypoint => {
                Self::CommandButton(Button::TeamAtWaypoint {
                    team: name(0)?,
                    button: name(1)?,
                    waypoint: name(2)?,
                })
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyUnit => {
                Self::Nearest(Button::TeamOnNearestEnemy {
                    team: name(0)?,
                    button: name(1)?,
                })
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestGarrisonedBuilding => {
                Self::Nearest(Button::TeamOnNearestGarrisonedBuilding {
                    team: name(0)?,
                    button: name(1)?,
                })
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestKindof => {
                Self::Nearest(Button::TeamOnNearestKindof {
                    team: name(0)?,
                    button: name(1)?,
                    kindof: name(2)?,
                })
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyBuilding => {
                Self::Nearest(Button::TeamOnNearestEnemyBuilding {
                    team: name(0)?,
                    button: name(1)?,
                })
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyBuildingClass => {
                Self::Nearest(Button::TeamOnNearestEnemyBuildingClass {
                    team: name(0)?,
                    button: name(1)?,
                    kindof: name(2)?,
                })
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestObjecttype => {
                Self::Nearest(Button::TeamOnNearestObjectType {
                    team: name(0)?,
                    button: name(1)?,
                    object_type: name(2)?,
                })
            }
            ScriptActionType::TeamPartialUseCommandbutton => Self::Partial {
                percentage: param(0)?.get_real(),
                team: name(1)?,
                button: name(2)?,
            },
            ScriptActionType::SkirmishAttackNearestGroupWithValue => Self::AttackNearestGroup {
                team: name(0)?,
                comparison: ComparisonType::from_i32(param(1)?.get_int()),
                value: param(2)?.get_int(),
            },
            ScriptActionType::SkirmishPerformCommandbuttonOnMostValuableObject => {
                // ScriptActions.cpp:5364 takes allTeamMembers but always issues to the group.
                Self::MostValuable {
                    team: name(0)?,
                    button: name(1)?,
                    range: param(2)?.get_real(),
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(decoded))
    }

    pub fn apply_to<I: ?Sized>(
        &self,
        world: &mut dyn ScriptExecutionDriver<I>,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let calling = interpreter.calling_team();
        let condition = interpreter.condition_team();
        let result = match *self {
            Self::CommandButton(ref button) => world.command_button_request(
                button,
                interpreter.script_object_id(),
                calling,
                condition,
            ),
            Self::Nearest(ref button) => world.team_nearest_button(button, calling, condition),
            Self::Partial {
                team,
                button,
                percentage,
            } => world.team_partial_button(team, button, percentage, calling, condition),
            Self::AttackNearestGroup {
                team,
                comparison,
                value,
            } => world.skirmish_attack_nearest_group(team, comparison, value, calling, condition),
            Self::MostValuable {
                team,
                button,
                range,
            } => world.skirmish_button_on_most_valuable(team, button, range, calling, condition),
        }?;
        // Core historically logged partial-button failures and continued.
        if matches!(self, Self::Partial { .. }) {
            if let Err(error) = result {
                log::warn!("Owned partial button failed: {error}");
            }
            return Some(Ok(ScriptActionResult::Success));
        }
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
