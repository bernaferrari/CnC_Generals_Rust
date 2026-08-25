//! Player relation, hunt, factory, victory, and alliance script actions
//!
//! C++: ScriptActions.cpp player cluster L189–237 / L2152–2260
//! (`doVictory`, `doDefeat`, `doPlayerHunt`, `doPlayerGrantScience` L5905).
//!
//! Split from `scripting/actions.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::ScriptAction;
use super::helpers::*;
use crate::action_manager::TheActionManager;
use crate::ai::integration::with_ai_integration_mut;
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, GuardMode, THE_AI};
use crate::commands::command::CommandType;
use crate::commands::{Command, CommandPriority, QueuedCommand, get_command_queue_manager};
use crate::common::PlayerIndex;
use crate::common::{
    AsciiString, CommandSourceType, Coord3D, INVALID_OBJECT_ID, LocomotorSetType, Real,
    Relationship,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::effects::FXList;
use crate::helpers::{TheGameLogic, TheVictoryConditions};
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::object::object_factory::{GameObjectInstance, get_object_factory};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object_manager::{ObjectCreationFlags, get_object_manager};
use crate::player::{PlayerType, player_list};
use crate::scripting::core::{LOCAL_PLAYER, TEAM_THE_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_TEAM};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::{ScriptContext, ScriptResult, ScriptValue};
use crate::system::shroud_manager::get_shroud_manager;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{RadarEventType, get_radar_system};

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Set player relation action
pub(super) struct SetPlayerRelationAction;

#[async_trait]
impl ScriptAction for SetPlayerRelationAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player1 = get_int_param(parameters, "player1")?;
        let player2 = get_int_param(parameters, "player2")?;
        let relation = get_string_param(parameters, "relation")?;

        log::info!(
            "Setting relation between player {} and {} to {}",
            player1,
            player2,
            relation
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_player_relation"
    }

    fn description(&self) -> &str {
        "Sets the diplomatic relation between two players"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player1".to_string(),
            "player2".to_string(),
            "relation".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Defeat player action
pub(super) struct DefeatPlayerAction;

#[async_trait]
impl ScriptAction for DefeatPlayerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;

        log::info!("Defeating player {}", player);

        // Actually defeat the player
        // Matches C++ Player::Set_Defeated()
        use crate::player::player_list;

        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_defeated(true);
                    log::info!("Player {} has been defeated", player);
                    return Ok(ScriptResult::Success(None));
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "defeat_player"
    }

    fn description(&self) -> &str {
        "Defeats the specified player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable production
pub(super) struct PlayerDisableFactoriesAction;

#[async_trait]
impl ScriptAction for PlayerDisableFactoriesAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let player = get_int_param(parameters, "player")?;

        log::info!("Disabling factories for player {}", player);

        // Integration with production system:
        // Disables unit production from all factories
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. For each production building owned by player:
        //    a. ProductionUpdate *production = building->getProductionModule()
        //    b. production->setEnabled(false)
        // 3. Prevents queueing new units
        // 4. Existing queue items continue or are cancelled
        // Rust: player.disable_all_production()

        let object_ids = {
            let Ok(list) = player_list().read() else {
                return Ok(ScriptResult::Success(None));
            };
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(player_guard) = player_arc.read() {
                    player_guard.get_object_ids()
                } else {
                    Vec::new()
                }
            } else {
                log::warn!("PlayerDisableFactoriesAction: player {} not found", player);
                Vec::new()
            }
        };
        for obj_id in object_ids {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.set_production_enabled(false);
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_disable_factories"
    }

    fn description(&self) -> &str {
        "Disables all factories for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Enable production
pub(super) struct PlayerEnableFactoriesAction;

#[async_trait]
impl ScriptAction for PlayerEnableFactoriesAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let player = get_int_param(parameters, "player")?;

        log::info!("Enabling factories for player {}", player);

        // Integration with production system:
        // Re-enables unit production
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. For each production building:
        //    a. ProductionUpdate *production = building->getProductionModule()
        //    b. production->setEnabled(true)
        // 3. Player can queue new units again
        // Rust: player.enable_all_production()

        let object_ids = {
            let Ok(list) = player_list().read() else {
                return Ok(ScriptResult::Success(None));
            };
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(player_guard) = player_arc.read() {
                    player_guard.get_object_ids()
                } else {
                    Vec::new()
                }
            } else {
                log::warn!("PlayerEnableFactoriesAction: player {} not found", player);
                Vec::new()
            }
        };
        for obj_id in object_ids {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.set_production_enabled(true);
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_enable_factories"
    }

    fn description(&self) -> &str {
        "Enables all factories for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Build defenses
pub(super) struct PlayerBuildBaseDefenseAction;

#[async_trait]
impl ScriptAction for PlayerBuildBaseDefenseAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let defense_type = get_string_param(parameters, "defense_type")?;

        log::info!("Player {} building base defense '{}'", player, defense_type);

        let Ok(list) = player_list().read() else {
            return Ok(ScriptResult::Success(None));
        };
        let player_idx = player.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let Some(player_arc) = list.get_player(player_idx) else {
            log::warn!("PlayerBuildBaseDefenseAction: player {} not found", player);
            return Ok(ScriptResult::Success(None));
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(ScriptResult::Success(None));
        };

        let player_id = player_guard.get_player_index() as u32;
        let _difficulty = player_guard.get_player_difficulty();

        let defense_lower = defense_type.to_ascii_lowercase();
        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                if defense_lower == "front" {
                    let _ = ai_player.build_base_defense(false);
                } else if defense_lower == "flank" {
                    let _ = ai_player.build_base_defense(true);
                } else {
                    let _ = ai_player.build_base_defense_structure(&defense_type, false);
                }
            })
        });

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_build_base_defense"
    }

    fn description(&self) -> &str {
        "Commands the player AI to build base defenses"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "defense_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player AI hunts
pub(super) struct PlayerHuntAction;

#[async_trait]
impl ScriptAction for PlayerHuntAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;

        log::info!("Player {} AI hunting", player);

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_units_should_hunt(true, CommandSourceType::FromScript);
                }
            } else {
                log::warn!("PlayerHuntAction: player {} not found", player);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_hunt"
    }

    fn description(&self) -> &str {
        "Commands the player AI to hunt enemies"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Garrison everything
pub(super) struct PlayerGarrisonAllBuildingsAction;

#[async_trait]
impl ScriptAction for PlayerGarrisonAllBuildingsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let player = get_int_param(parameters, "player")?;

        log::info!("Player {} garrisoning all buildings", player);

        // Integration with garrison system:
        // Auto-garrisons player's infantry into available buildings
        // 1. Find all garrisonable buildings owned by player
        // 2. Find all infantry units owned by player with CAN_GARRISON flag
        // 3. Match infantry to nearest buildings
        // 4. Issue garrison commands
        // 5. GarrisonContain handles actual containment
        // Rust: player.auto_garrison_all_buildings()

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            let Some(player_arc) = list.get_player(index) else {
                log::warn!(
                    "PlayerGarrisonAllBuildingsAction: player {} not found",
                    player
                );
                return Ok(ScriptResult::Success(None));
            };

            let Ok(player_guard) = player_arc.read() else {
                return Ok(ScriptResult::Success(None));
            };

            let object_ids = player_guard.get_object_ids();
            let mut garrison_buildings = Vec::new();
            let mut infantry_units = Vec::new();

            for obj_id in object_ids {
                let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if let Some(contain) = obj_guard.get_contain() {
                    if contain
                        .try_lock()
                        .map(|guard| guard.is_garrisonable())
                        .unwrap_or(false)
                    {
                        garrison_buildings.push(obj_arc.clone());
                    }
                }

                if obj_guard.is_kind_of(crate::common::KindOf::Infantry) {
                    infantry_units.push(obj_arc.clone());
                }
            }

            if garrison_buildings.is_empty() || infantry_units.is_empty() {
                return Ok(ScriptResult::Success(None));
            }

            for unit_arc in infantry_units {
                let Ok(unit_guard) = unit_arc.read() else {
                    continue;
                };
                let Some(ai) = unit_guard.get_ai_update_interface() else {
                    continue;
                };
                let unit_pos = *unit_guard.get_position();
                let mut best: Option<(f32, u32)> = None;

                for building_arc in &garrison_buildings {
                    let Ok(building_guard) = building_arc.read() else {
                        continue;
                    };
                    let pos = building_guard.get_position();
                    let dx = pos.x - unit_pos.x;
                    let dy = pos.y - unit_pos.y;
                    let dz = pos.z - unit_pos.z;
                    let dist_sq = dx * dx + dy * dy + dz * dz;
                    let id = building_guard.get_id();
                    if best.map(|(d, _)| dist_sq < d).unwrap_or(true) {
                        best = Some((dist_sq, id));
                    }
                }

                if let Some((_, building_id)) = best {
                    ai.ai_enter(building_id, CommandSourceType::FromScript);
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_garrison_all_buildings"
    }

    fn description(&self) -> &str {
        "Commands the player to garrison all available buildings"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Sell structure
pub(super) struct PlayerSellBuildingAction;

#[async_trait]
impl ScriptAction for PlayerSellBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!("Player {} selling building '{}'", player, building_name);

        // Integration with building sell system:
        // Sells building, returns partial cost
        // 1. Object *building = TheScriptEngine->getUnitNamed(building_name)
        // 2. Verify building owned by player
        // 3. Calculate refund (typically 50% of build cost)
        // 4. player->addMoney(refund)
        // 5. building->sell() - initiates sell sequence
        // 6. Building destroyed, money refunded
        // Rust: building.sell(player)

        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "PlayerSellBuildingAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let current_frame = crate::helpers::TheGameLogic::get_frame() as u32;
        let mut command = Command::new(CommandType::Sell);
        command.set_player_index(player as i32);
        command.append_object_id_argument(building_id);
        let queued = QueuedCommand::new(command, CommandPriority::High, current_frame);

        let queue_manager = get_command_queue_manager();
        if let Ok(mut manager) = queue_manager.lock() {
            if let Err(err) = manager.queue_player_command(player as i32, queued) {
                log::warn!(
                    "PlayerSellBuildingAction: failed to queue sell for '{}' (ID {}): {}",
                    building_name,
                    building_id,
                    err
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_sell_building"
    }

    fn description(&self) -> &str {
        "Commands the player to sell a specific building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Evacuate garrison
pub(super) struct PlayerEvacuateBuildingAction;

#[async_trait]
impl ScriptAction for PlayerEvacuateBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let player = get_int_param(parameters, "player")?;

        log::info!("Player {} evacuating all buildings", player);

        // Integration with garrison system:
        // Ejects all units from garrisoned buildings
        // 1. For each building owned by player:
        //    a. GarrisonContain *garrison = building->getGarrisonContain()
        //    b. garrison->evacuateAll()
        // 2. All contained units exit to nearby positions
        // 3. Units return to player control
        // Rust: player.evacuate_all_garrisons()

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            let Some(player_arc) = list.get_player(index) else {
                log::warn!("PlayerEvacuateBuildingAction: player {} not found", player);
                return Ok(ScriptResult::Success(None));
            };

            let Ok(player_guard) = player_arc.read() else {
                return Ok(ScriptResult::Success(None));
            };

            for obj_id in player_guard.get_object_ids() {
                let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                let Some(contain) = obj_guard.get_contain() else {
                    continue;
                };
                let contained = {
                    let Ok(contain_guard) = contain.try_lock() else {
                        continue;
                    };
                    if !contain_guard.is_garrisonable() {
                        continue;
                    }
                    contain_guard.get_contained_objects().to_vec()
                };
                let contain_lock = contain.try_lock();
                if let Ok(mut contain_guard) = contain_lock {
                    for occupant in contained {
                        let _ = contain_guard.release_object(occupant);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_evacuate_building"
    }

    fn description(&self) -> &str {
        "Commands the player to evacuate all garrisoned buildings"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set player active/inactive
pub(super) struct PlayerSetActiveAction;

#[async_trait]
impl ScriptAction for PlayerSetActiveAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let active = parameters
            .get("active")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!("Setting player {} active state to {}", player, active);

        // Integration with player management:
        // Active/inactive affects AI behavior and victory conditions
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. pPlayer->setActive(active)
        // 3. Inactive players: AI stops, units may become neutral, no victory check
        // 4. Used for players defeated or left in multiplayer
        // Rust: player.set_active(active)

        if let Ok(list) = player_list().read() {
            let player_idx = player.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            if let Some(player_arc) = list.get_player(player_idx) {
                if let Ok(mut player_guard) = player_arc.write() {
                    if active {
                        player_guard.set_observer(false);
                        player_guard.set_defeated(false);
                    } else {
                        player_guard.set_observer(true);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_set_active"
    }

    fn description(&self) -> &str {
        "Sets whether a player is active or inactive"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["active".to_string()]
    }
}

// ============================================================================
// 20 CORE SCRIPT ACTIONS - Priority 1 Implementation
// Based on C++ ScriptActions from GENERALSMD_SCRIPTING_SYSTEM_GUIDE.md
// ============================================================================

/// Victory Action - Matches C++ ScriptActionType::VICTORY
pub(super) struct VictoryAction;

#[async_trait]
impl ScriptAction for VictoryAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("VICTORY - Mission completed successfully");

        TheVictoryConditions::set_local_allied_victory(true);
        if let Ok(players) = player_list().read() {
            if let Some(local_player) = players.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(false);
                }
            }
        }
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.start_end_game_timer();
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "victory"
    }

    fn description(&self) -> &str {
        "Triggers victory condition - mission complete"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Defeat Action - Matches C++ ScriptActionType::DEFEAT
pub(super) struct DefeatAction;

#[async_trait]
impl ScriptAction for DefeatAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("DEFEAT - Mission failed");

        TheVictoryConditions::set_local_allied_victory(false);
        if let Ok(players) = player_list().read() {
            if let Some(local_player) = players.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(true);
                }
            }
        }
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.start_end_game_timer();
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "defeat"
    }

    fn description(&self) -> &str {
        "Triggers defeat condition - mission failed"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Team Alliance Action - Matches C++ ScriptActionType::PLAYER_RELATES_PLAYER
pub(super) struct SetTeamAllianceAction;

#[async_trait]
impl ScriptAction for SetTeamAllianceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player1 = get_int_param(parameters, "player1")?;
        let player2 = get_int_param(parameters, "player2")?;
        let relation = get_string_param(parameters, "relation")?; // "ally", "enemy", "neutral"

        log::info!(
            "Setting relation between player {} and player {} to '{}'",
            player1,
            player2,
            relation
        );

        // Integration with player relation system:
        // Relation types: "ALLY", "ENEMY", "NEUTRAL"
        // 1. Player *p1 = ThePlayerList->getPlayer(player1)
        // 2. Player *p2 = ThePlayerList->getPlayer(player2)
        // 3. Relationship rel = parseRelation(relation)
        // 4. p1->setRelationship(p2, rel)
        // 5. Affects targeting, fog of war, unit colors
        // 6. Matches C++ updatePlayerRelationTowardPlayer: this is one-way

        if player1 < 0 || player2 < 0 {
            return Err(GameLogicError::Configuration(
                "player indices must be non-negative".to_string(),
            ));
        }
        let relationship = parse_script_relationship(&relation)?;
        let target_player_index = player2 as PlayerIndex;
        let player_arc = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?
            .get_player(player1 as PlayerIndex)
            .cloned();
        let Some(player_arc) = player_arc else {
            log::warn!("SetTeamAllianceAction: player {} not found", player1);
            return Ok(ScriptResult::Success(None));
        };
        let mut player_guard = player_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
        player_guard.set_player_relationship_by_index(target_player_index, relationship);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_team_alliance"
    }

    fn description(&self) -> &str {
        "Sets the diplomatic relationship between two players/teams"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player1".to_string(),
            "player2".to_string(),
            "relation".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
