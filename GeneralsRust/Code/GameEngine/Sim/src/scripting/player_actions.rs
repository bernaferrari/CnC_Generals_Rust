//! Authored player actions from ScriptActions.cpp.

use super::driver::ScriptExecutionDriver;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{
    HostScriptMoneyRequest as Money, ScriptOwnerQuery, ScriptPlayerConstructionRequest as Build,
    ScriptPlayerRankRequest as Rank,
};
use super::{ScriptActionResult, ScriptError};
use crate::science::ScienceAvailabilityType;
use game_engine::common::game_common::Relationship;

/// Decoded player effects. The receiving match resolves the authored selectors.
pub enum PlayerAction<'a> {
    Money(Money),
    Science {
        player: &'a str,
        science: &'a str,
        grant: bool,
    },
    ScienceAvailability {
        player: &'a str,
        science: &'a str,
        availability: ScienceAvailabilityType,
    },
    Construction(Build),
    Rank(Rank),
    PrioritizeSkirmishBuilding(&'a str),
    Kill(&'a str),
    Hunt(&'a str),
    Sell(&'a str),
    ExcludeFromScore(&'a str),
    Relate {
        source: &'a str,
        destination: &'a str,
        relationship: Relationship,
    },
    Transfer {
        from: &'a str,
        to: &'a str,
    },
    IgnoreInvalidScienceAvailability,
}

impl<'a> PlayerAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let param = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let string = |index| param(index).map(|param| param.get_string());
        let player = || string(0).map(str::to_owned);
        let int = |index| param(index).map(|param| param.get_int());
        let kind = action.get_action_type();

        let request = match kind {
            PlayerSetMoney => Self::Money(Money::Set {
                player: player()?,
                amount: int(1)?,
            }),
            PlayerGiveMoney => Self::Money(Money::Give {
                player: player()?,
                amount: int(1)?,
            }),
            PlayerGrantScience | PlayerPurchaseScience => Self::Science {
                player: string(0)?,
                science: string(1)?,
                grant: kind == PlayerGrantScience,
            },
            PlayerScienceAvailability => {
                let (player, science, name) = (string(0)?, string(1)?, string(2)?);
                if let Some(availability) = ScienceAvailabilityType::from_script_name(name) {
                    Self::ScienceAvailability {
                        player,
                        science,
                        availability,
                    }
                } else {
                    log::warn!("Invalid science availability '{name}' for '{science}'");
                    Self::IgnoreInvalidScienceAvailability
                }
            }
            PlayerDisableBaseConstruction | PlayerEnableBaseConstruction => {
                Self::Construction(Build::Base {
                    player: player()?,
                    enabled: kind == PlayerEnableBaseConstruction,
                })
            }
            PlayerDisableUnitConstruction | PlayerEnableUnitConstruction => {
                Self::Construction(Build::Units {
                    player: player()?,
                    enabled: kind == PlayerEnableUnitConstruction,
                })
            }
            PlayerDisableFactories | PlayerEnableFactories => {
                Self::Construction(Build::Factories {
                    player: player()?,
                    template: string(1)?.to_owned(),
                    enabled: kind == PlayerEnableFactories,
                })
            }
            PlayerAddSkillpoints => Self::Rank(Rank::AddSkillPoints {
                player: player()?,
                delta: int(1)?,
            }),
            PlayerAddRanklevel => Self::Rank(Rank::AddRankLevel {
                player: player()?,
                delta: int(1)?,
            }),
            PlayerSetRanklevel => Self::Rank(Rank::SetRankLevel {
                player: player()?,
                level: int(1)?,
            }),
            PlayerSetRanklevellimit => Self::Rank(Rank::SetRankLevelLimit { limit: int(0)? }),
            PlayerAffectReceivingExperience => Self::Rank(Rank::AffectReceivingExperience {
                player: player()?,
                modifier: param(1)?.get_real(),
            }),
            SkirmishBuildBuilding => Self::PrioritizeSkirmishBuilding(string(0)?),
            PlayerKill => Self::Kill(string(0)?),
            PlayerHunt => Self::Hunt(string(0)?),
            PlayerSellEverything => Self::Sell(string(0)?),
            PlayerExcludeFromScoreScreen => Self::ExcludeFromScore(string(0)?),
            PlayerRelatesPlayer => Self::Relate {
                source: string(0)?,
                destination: string(1)?,
                relationship: match int(2)? {
                    0 => Relationship::Enemies,
                    2 => Relationship::Allies,
                    _ => Relationship::Neutral,
                },
            },
            PlayerTransferOwnershipPlayer => Self::Transfer {
                from: string(0)?,
                to: string(1)?,
            },
            _ => return Ok(None),
        };
        Ok(Some(request))
    }

    /// An explicit receiving result, including a missing player, ends dispatch.
    /// `None` requests the standalone world adapter.
    pub fn apply_to<Interpreter: ?Sized>(
        &self,
        world: &mut dyn ScriptExecutionDriver<Interpreter>,
        current_player: Option<&str>,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let result = match self {
            Self::Money(request) => world.money(request, current_player),
            Self::Science {
                player,
                science,
                grant,
            } => world.player_science_action(player, science, *grant, current_player),
            Self::ScienceAvailability {
                player,
                science,
                availability,
            } => {
                match world.player_science_availability(
                    player,
                    science,
                    *availability,
                    current_player,
                ) {
                    ScriptOwnerQuery::Unavailable => None,
                    ScriptOwnerQuery::Missing | ScriptOwnerQuery::Present(()) => Some(Ok(())),
                }
            }
            Self::Construction(request) => world.player_construction(request, current_player),
            Self::Rank(request) => world.player_rank(request, current_player),
            Self::PrioritizeSkirmishBuilding(building) => {
                world.prioritize_skirmish_building(building, current_player)
            }
            Self::Kill(player) => world.kill_player(player, current_player),
            Self::Hunt(player) => world.player_hunt(player, current_player),
            Self::Sell(player) => world.sell_everything(player, current_player),
            Self::ExcludeFromScore(player) => {
                world.exclude_player_from_score(player, current_player)
            }
            Self::Relate {
                source,
                destination,
                relationship,
            } => world.player_relates(source, destination, *relationship, current_player),
            Self::Transfer { from, to } => world.transfer_player_assets(from, to, current_player),
            Self::IgnoreInvalidScienceAvailability => Some(Ok(())),
        }?;
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
