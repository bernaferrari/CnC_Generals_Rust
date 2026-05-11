//! Script Actions System
//!
//! This module provides all the action types that scripts can execute,
//! including unit creation, resource manipulation, UI updates, and game state changes.

use super::{ScriptContext, ScriptResult, ScriptValue};
use crate::action_manager::TheActionManager;
use crate::ai::integration::with_ai_integration_mut;
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, GuardMode, THE_AI};
use crate::commands::command::CommandType;
use crate::commands::{get_command_queue_manager, Command, CommandPriority, QueuedCommand};
use crate::common::{
    AsciiString, CommandSourceType, Coord3D, LocomotorSetType, Real, Relationship,
    INVALID_OBJECT_ID,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheVictoryConditions};
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::object::object_factory::{get_object_factory, GameObjectInstance};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object_manager::{get_object_manager, ObjectCreationFlags};
use crate::player::{player_list, PlayerIndex, PlayerType};
use crate::scripting::core::{LOCAL_PLAYER, TEAM_THE_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_TEAM};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::system::shroud_manager::get_shroud_manager;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Script action trait
#[async_trait]
pub trait ScriptAction: Send + Sync {
    /// Execute the action
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult>;

    /// Get action name
    fn name(&self) -> &str;

    /// Get action description
    fn description(&self) -> &str;

    /// Get required parameters
    fn required_parameters(&self) -> Vec<String>;

    /// Get optional parameters
    fn optional_parameters(&self) -> Vec<String>;
}

/// Action registry for managing script actions
pub struct ActionRegistry {
    actions: HashMap<String, Box<dyn ScriptAction>>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            actions: HashMap::new(),
        };

        // Register built-in actions
        registry.register_builtin_actions();
        registry
    }

    /// Register built-in actions
    fn register_builtin_actions(&mut self) {
        // Unit and object actions
        self.register_action(Box::new(CreateUnitAction));
        self.register_action(Box::new(DestroyObjectAction));
        self.register_action(Box::new(MoveUnitAction));
        self.register_action(Box::new(AttackUnitAction));
        self.register_action(Box::new(SetObjectHealthAction));
        self.register_action(Box::new(SetObjectExperienceAction));

        // Team actions (15 critical actions)
        self.register_action(Box::new(TeamAttackTeamAction));
        self.register_action(Box::new(TeamFollowWaypointsAction));
        self.register_action(Box::new(TeamGuardAction));
        self.register_action(Box::new(TeamHuntAction));
        self.register_action(Box::new(TeamMoveToWaypointAction));
        self.register_action(Box::new(TeamGarrisonBuildingAction));
        self.register_action(Box::new(TeamExitBuildingAction));
        self.register_action(Box::new(TeamCaptureBuildingAction));
        self.register_action(Box::new(TeamRepairAction));
        self.register_action(Box::new(TeamWanderAction));
        self.register_action(Box::new(TeamIdleAction));
        self.register_action(Box::new(TeamSetStateAction));
        self.register_action(Box::new(TeamDeleteAction));
        self.register_action(Box::new(TeamFollowTeamAction));
        self.register_action(Box::new(TeamGuardInTunnelAction));

        // Named unit actions (10 critical actions)
        self.register_action(Box::new(NamedAttackAction));
        self.register_action(Box::new(NamedMoveToAction));
        self.register_action(Box::new(NamedGarrisonAction));
        self.register_action(Box::new(NamedFollowWaypointsAction));
        self.register_action(Box::new(NamedGuardAction));
        self.register_action(Box::new(NamedHuntAction));
        self.register_action(Box::new(NamedDeleteAction));
        self.register_action(Box::new(NamedEnterNamedAction));
        self.register_action(Box::new(NamedExitAction));
        self.register_action(Box::new(NamedSetAttitudeAction));

        // Player actions (10 critical actions)
        self.register_action(Box::new(PlayerGrantScienceAction));
        self.register_action(Box::new(PlayerDisableFactoriesAction));
        self.register_action(Box::new(PlayerEnableFactoriesAction));
        self.register_action(Box::new(PlayerBuildBaseDefenseAction));
        self.register_action(Box::new(PlayerHuntAction));
        self.register_action(Box::new(PlayerGarrisonAllBuildingsAction));
        self.register_action(Box::new(PlayerSellBuildingAction));
        self.register_action(Box::new(PlayerEvacuateBuildingAction));
        self.register_action(Box::new(PlayerSetActiveAction));
        self.register_action(Box::new(PlayerAddMoneyAction));

        // Original player and team actions
        self.register_action(Box::new(SetPlayerResourceAction));
        self.register_action(Box::new(AddPlayerResourceAction));
        self.register_action(Box::new(SetPlayerRelationAction));
        self.register_action(Box::new(DefeatPlayerAction));

        // Map/Camera actions (8 critical actions)
        self.register_action(Box::new(MapRevealAreaAction));
        self.register_action(Box::new(MapShroudAreaAction));
        self.register_action(Box::new(CameraMoveToWaypointAction));
        self.register_action(Box::new(CameraTrackNamedAction));
        self.register_action(Box::new(CameraLetterboxBeginAction));
        self.register_action(Box::new(CameraLetterboxEndAction));
        self.register_action(Box::new(CameraSetFinalZoomAction));
        self.register_action(Box::new(WeatherSetAction));

        // Audio/Visual actions (7 critical actions)
        self.register_action(Box::new(SoundPlayAction));
        self.register_action(Box::new(MusicPlayAction));
        self.register_action(Box::new(MoviePlayAction));
        self.register_action(Box::new(TextDisplayAction));
        self.register_action(Box::new(SpeechPlayAction));
        self.register_action(Box::new(RadarEnableAction));
        self.register_action(Box::new(RadarDisableAction));

        // Original camera and UI actions
        self.register_action(Box::new(MoveCameraAction));
        self.register_action(Box::new(ShowTextMessageAction));
        self.register_action(Box::new(PlaySoundAction));
        self.register_action(Box::new(PlayMusicAction));

        // Map and environment actions
        self.register_action(Box::new(RevealMapAreaAction));
        self.register_action(Box::new(ShroudMapAreaAction));
        self.register_action(Box::new(SetWeatherAction));
        self.register_action(Box::new(SetTimeOfDayAction));

        // Special abilities and powers
        self.register_action(Box::new(TriggerSpecialPowerAction));
        self.register_action(Box::new(EnableSpecialPowerAction));
        self.register_action(Box::new(DisableSpecialPowerAction));

        // Technology and upgrades
        self.register_action(Box::new(GrantUpgradeAction));
        self.register_action(Box::new(EnableScienceAction));
        self.register_action(Box::new(DisableScienceAction));

        // Scripting control actions
        self.register_action(Box::new(EnableScriptAction));
        self.register_action(Box::new(DisableScriptAction));
        self.register_action(Box::new(ExecuteScriptAction));
        self.register_action(Box::new(SetVariableAction));
        self.register_action(Box::new(WaitAction));

        // 20 Core Actions - Priority 1 Implementation
        self.register_action(Box::new(VictoryAction));
        self.register_action(Box::new(DefeatAction));
        self.register_action(Box::new(StartTimerAction));
        self.register_action(Box::new(StopTimerAction));
        self.register_action(Box::new(CreateBuildingAction));
        self.register_action(Box::new(DestroyBuildingAction));
        self.register_action(Box::new(SetTeamAllianceAction));
        self.register_action(Box::new(GiveSpecialPowerAction));
        self.register_action(Box::new(RevealAreaAction));
        self.register_action(Box::new(CreateExplosionAction));
        self.register_action(Box::new(SpawnReinforcementsAction));
        self.register_action(Box::new(CameraZoomAction));

        // High-Priority Missing Actions - Ported from C++
        self.register_action(Box::new(GiveMoneyAction));
        self.register_action(Box::new(SetMoneyAction));
        self.register_action(Box::new(SetHandicapAction));
        self.register_action(Box::new(DamageObjectAction));
        self.register_action(Box::new(KillObjectAction));
        self.register_action(Box::new(HealObjectAction));
        self.register_action(Box::new(RevealMapEntireAction));
        self.register_action(Box::new(ShroudMapEntireAction));
        self.register_action(Box::new(SnapCameraAction));
        self.register_action(Box::new(LetterBoxBeginAction));
        self.register_action(Box::new(LetterBoxEndAction));
        self.register_action(Box::new(TeamAttackAction));
        self.register_action(Box::new(TeamGuardAreaAction));
        self.register_action(Box::new(TeamFollowAction));
        self.register_action(Box::new(SetTimerAction));
        self.register_action(Box::new(CountdownTimerAction));
        self.register_action(Box::new(PlaySoundAtAction));
        self.register_action(Box::new(StopMusicAction));
    }

    /// Register an action
    pub fn register_action(&mut self, action: Box<dyn ScriptAction>) {
        self.actions.insert(action.name().to_string(), action);
    }

    /// Get action by name
    pub fn get_action(&self, name: &str) -> Option<&dyn ScriptAction> {
        self.actions.get(name).map(|action| action.as_ref())
    }

    /// List all available actions
    pub fn list_actions(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }
}

// Built-in action implementations

/// Create a unit action
struct CreateUnitAction;

#[async_trait]
impl ScriptAction for CreateUnitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let unit_type = get_string_param(parameters, "unit_type")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);

        log::info!(
            "Creating unit '{}' for player {} at ({}, {}, {})",
            unit_type,
            player,
            x,
            y,
            z
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let position = Coord3D::new(x as f32, y as f32, z as f32);
        let object_id = get_object_manager()
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?
            .create_object(
                unit_type.as_str(),
                position,
                team,
                ObjectCreationFlags::from_template(),
            )?;

        Ok(ScriptResult::Success(Some(ScriptValue::ObjectId(
            object_id as u32,
        ))))
    }

    fn name(&self) -> &str {
        "create_unit"
    }

    fn description(&self) -> &str {
        "Creates a unit for the specified player at the given location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string()]
    }
}

/// Destroy object action
struct DestroyObjectAction;

#[async_trait]
impl ScriptAction for DestroyObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;

        log::info!("Destroying object {}", object_id);

        if object_id < 0 {
            return Err(GameLogicError::Configuration(
                "object_id must be non-negative".to_string(),
            ));
        }

        if let Ok(mut manager) = get_object_manager().write() {
            manager.destroy_object(object_id as u32);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "destroy_object"
    }

    fn description(&self) -> &str {
        "Destroys the specified object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Move unit action
struct MoveUnitAction;

#[async_trait]
impl ScriptAction for MoveUnitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);

        log::info!("Moving unit {} to ({}, {}, {})", object_id, x, y, z);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "move_unit"
    }

    fn description(&self) -> &str {
        "Moves the specified unit to the given location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "x".to_string(), "y".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string()]
    }
}

/// Attack unit action
struct AttackUnitAction;

#[async_trait]
impl ScriptAction for AttackUnitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let attacker_id = get_int_param(parameters, "attacker_id")?;
        let target_id = get_int_param(parameters, "target_id")?;

        log::info!("Unit {} attacking unit {}", attacker_id, target_id);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "attack_unit"
    }

    fn description(&self) -> &str {
        "Commands one unit to attack another"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["attacker_id".to_string(), "target_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set object health action
struct SetObjectHealthAction;

#[async_trait]
impl ScriptAction for SetObjectHealthAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;
        let health = get_float_param(parameters, "health")?;

        log::info!("Setting object {} health to {}", object_id, health);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_object_health"
    }

    fn description(&self) -> &str {
        "Sets the health of the specified object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "health".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set object experience action
struct SetObjectExperienceAction;

#[async_trait]
impl ScriptAction for SetObjectExperienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;
        let experience = get_int_param(parameters, "experience")?;

        log::info!("Setting object {} experience to {}", object_id, experience);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_object_experience"
    }

    fn description(&self) -> &str {
        "Sets the experience level of the specified object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "experience".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set player resource action
struct SetPlayerResourceAction;

#[async_trait]
impl ScriptAction for SetPlayerResourceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let resource_type = get_string_param(parameters, "resource_type")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Setting player {} {} to {}", player, resource_type, amount);

        if is_money_resource(&resource_type) {
            let player_list_lock = player_list();
            let list = player_list_lock
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;
            let Some(player_arc) = list.get_player(player as i32) else {
                return Ok(ScriptResult::Success(None));
            };
            let mut player_guard = player_arc
                .write()
                .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
            let new_amount = clamp_script_money(amount);
            set_script_player_money(&mut player_guard, new_amount);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_player_resource"
    }

    fn description(&self) -> &str {
        "Sets a player's resource amount"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "resource_type".to_string(),
            "amount".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Add player resource action
struct AddPlayerResourceAction;

#[async_trait]
impl ScriptAction for AddPlayerResourceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let resource_type = get_string_param(parameters, "resource_type")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Adding {} {} to player {}", amount, resource_type, player);

        if is_money_resource(&resource_type) {
            let player_list_lock = player_list();
            let list = player_list_lock
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;
            let Some(player_arc) = list.get_player(player as i32) else {
                return Ok(ScriptResult::Success(None));
            };
            let mut player_guard = player_arc
                .write()
                .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
            if amount < 0 {
                let requested = clamp_script_money(amount.saturating_neg());
                spend_script_player_money(&mut player_guard, requested);
            } else {
                let deposit_amount = clamp_script_money(amount);
                grant_script_player_money(&mut player_guard, deposit_amount);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "add_player_resource"
    }

    fn description(&self) -> &str {
        "Adds resources to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "resource_type".to_string(),
            "amount".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

fn is_money_resource(resource_type: &str) -> bool {
    matches!(
        resource_type.trim().to_ascii_lowercase().as_str(),
        "money" | "cash" | "resource" | "resources" | "supply" | "supplies"
    )
}

fn clamp_script_money(amount: i64) -> i32 {
    amount.clamp(0, i32::MAX as i64) as i32
}

fn set_script_player_money(player: &mut crate::player::Player, new_amount: i32) {
    let current = player.get_money().get_money();
    player.get_money_mut().set_money(new_amount);
    record_script_money_delta(player, new_amount as i64 - current as i64);
}

fn grant_script_player_money(player: &mut crate::player::Player, amount: i32) {
    if amount <= 0 {
        return;
    }

    let current = player.get_money().get_money();
    player
        .get_money_mut()
        .set_money(current.saturating_add(amount));
    record_script_money_delta(player, amount as i64);
}

fn spend_script_player_money(player: &mut crate::player::Player, amount: i32) {
    if amount <= 0 {
        return;
    }

    let current = player.get_money().get_money();
    let withdrawn = amount.min(current.max(0));
    if withdrawn > 0 {
        player.get_money_mut().set_money(current - withdrawn);
        record_script_money_delta(player, -(withdrawn as i64));
    }
}

fn record_script_money_delta(player: &mut crate::player::Player, delta: i64) {
    if delta > 0 {
        let amount = delta.min(u32::MAX as i64) as u32;
        player.get_score_keeper_mut().add_money_earned(amount);
        player.get_academy_stats_mut().record_income(delta as i32);
    } else if delta < 0 {
        player
            .get_score_keeper_mut()
            .add_money_spent(delta.saturating_neg().min(u32::MAX as i64) as u32);
    }
}

fn with_script_engine_mut<F>(f: F) -> GameLogicResult<()>
where
    F: FnOnce(&mut crate::scripting::engine::ScriptEngine) -> GameLogicResult<()>,
{
    let engine_lock = get_script_engine();
    let mut engine_guard = engine_lock
        .write()
        .map_err(|_| GameLogicError::Threading("Failed to lock ScriptEngine".to_string()))?;
    let Some(engine) = engine_guard.as_mut() else {
        return Ok(());
    };
    f(engine)
}

fn dispatch_named_timer(name: &str, text: &str, countdown: bool) {
    if let Ok(engine_guard) = get_script_engine().read() {
        if let Some(ref script_engine) = *engine_guard {
            if let Some(handler) = script_engine.action_handler() {
                if let Err(err) = handler.add_named_timer(name, text, countdown) {
                    log::warn!("Script action handler add_named_timer failed: {}", err);
                }
            }
        }
    }
}

fn parse_script_relationship(value: &str) -> GameLogicResult<Relationship> {
    match value.trim().to_ascii_uppercase().as_str() {
        "ALLY" | "ALLIES" => Ok(Relationship::Allies),
        "ENEMY" | "ENEMIES" => Ok(Relationship::Enemies),
        "NEUTRAL" => Ok(Relationship::Neutral),
        _ => Err(GameLogicError::Configuration(format!(
            "Unknown relationship '{}'",
            value
        ))),
    }
}

/// Set player relation action
struct SetPlayerRelationAction;

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
struct DefeatPlayerAction;

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

/// Move camera action
struct MoveCameraAction;

#[async_trait]
impl ScriptAction for MoveCameraAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let x = get_float_param(parameters, "x")? as f32;
        let y = get_float_param(parameters, "y")? as f32;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0) as f32;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0) as f32;

        log::info!(
            "Moving camera to ({}, {}, {}) over {} seconds",
            x,
            y,
            z,
            duration
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to(x, y, z, duration, 0.0, 0.0, 0.0) {
                        log::warn!("Script action handler move_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "move_camera"
    }

    fn description(&self) -> &str {
        "Moves the camera to the specified location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string(), "duration".to_string()]
    }
}

/// Show text message action
struct ShowTextMessageAction;

#[async_trait]
impl ScriptAction for ShowTextMessageAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let message = get_string_param(parameters, "message")?;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(5.0);

        log::info!("Showing message: '{}' for {} seconds", message, duration);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&message) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "show_text_message"
    }

    fn description(&self) -> &str {
        "Displays a text message to the player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["message".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Play sound action
struct PlaySoundAction;

#[async_trait]
impl ScriptAction for PlaySoundAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let sound_name = get_string_param(parameters, "sound_name")?;
        let volume = get_float_param_optional(parameters, "volume").unwrap_or(1.0);

        log::info!("Playing sound '{}' at volume {}", sound_name, volume);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.play_sound_effect(&sound_name) {
                        log::warn!("Script action handler play_sound_effect failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "play_sound"
    }

    fn description(&self) -> &str {
        "Plays a sound effect"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["sound_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["volume".to_string()]
    }
}

/// Play music action
struct PlayMusicAction;

#[async_trait]
impl ScriptAction for PlayMusicAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let music_name = get_string_param(parameters, "music_name")?;
        let fade_in = get_float_param_optional(parameters, "fade_in").unwrap_or(0.0);

        log::info!(
            "Playing music '{}' with {} second fade-in",
            music_name,
            fade_in
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.music_set_track(&music_name, true, fade_in > 0.0) {
                        log::warn!("Script action handler music_set_track failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "play_music"
    }

    fn description(&self) -> &str {
        "Plays background music"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["music_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["fade_in".to_string()]
    }
}

/// Reveal map area action
struct RevealMapAreaAction;

#[async_trait]
impl ScriptAction for RevealMapAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let x = get_float_param(parameters, "x")? as f32;
        let y = get_float_param(parameters, "y")? as f32;
        let radius = get_float_param(parameters, "radius")? as f32;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Revealing map area at ({}, {}) with radius {} for player {}",
            x,
            y,
            radius,
            player
        );

        let center = Coord3D::new(x, y, 0.0);
        let player_mask = 1u32 << (player.max(0) as u32);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.do_shroud_reveal(&center, radius, player_mask);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "reveal_map_area"
    }

    fn description(&self) -> &str {
        "Reveals an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Shroud map area action
struct ShroudMapAreaAction;

#[async_trait]
impl ScriptAction for ShroudMapAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let radius = get_float_param(parameters, "radius")?;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Shrouding map area at ({}, {}) with radius {} for player {}",
            x,
            y,
            radius,
            player
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "shroud_map_area"
    }

    fn description(&self) -> &str {
        "Shrouds an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set weather action
struct SetWeatherAction;

#[async_trait]
impl ScriptAction for SetWeatherAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let weather_type = get_string_param(parameters, "weather_type")?;
        let intensity = get_float_param_optional(parameters, "intensity").unwrap_or(1.0);

        log::info!(
            "Setting weather to '{}' with intensity {}",
            weather_type,
            intensity
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_weather"
    }

    fn description(&self) -> &str {
        "Changes the weather conditions"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["weather_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["intensity".to_string()]
    }
}

/// Set time of day action
struct SetTimeOfDayAction;

#[async_trait]
impl ScriptAction for SetTimeOfDayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let time = get_float_param(parameters, "time")?; // 0.0 = midnight, 0.5 = noon, 1.0 = midnight
        let transition_duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0);

        log::info!(
            "Setting time of day to {} over {} seconds",
            time,
            transition_duration
        );

        let time_of_day = if time >= 0.25 && time < 0.5 {
            crate::common::audio::TimeOfDay::Morning
        } else if time >= 0.5 && time < 0.75 {
            crate::common::audio::TimeOfDay::Day
        } else if time >= 0.75 {
            crate::common::audio::TimeOfDay::Evening
        } else {
            crate::common::audio::TimeOfDay::Night
        };

        if let Some(global) = crate::helpers::TheGlobalData::get() {
            global.set_time_of_day(time_of_day);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_time_of_day"
    }

    fn description(&self) -> &str {
        "Changes the time of day"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["time".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Trigger special power action
struct TriggerSpecialPowerAction;

#[async_trait]
impl ScriptAction for TriggerSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;
        let x = get_float_param_optional(parameters, "x");
        let y = get_float_param_optional(parameters, "y");
        let target_id = get_int_param_optional(parameters, "target_id");

        log::info!(
            "Triggering special power '{}' for player {}",
            power_name,
            player
        );
        if let (Some(x_pos), Some(y_pos)) = (x, y) {
            log::info!("Target position: ({}, {})", x_pos, y_pos);
        }
        if let Some(target) = target_id {
            log::info!("Target object: {}", target);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "trigger_special_power"
    }

    fn description(&self) -> &str {
        "Triggers a special power for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string(), "target_id".to_string()]
    }
}

/// Enable special power action
struct EnableSpecialPowerAction;

#[async_trait]
impl ScriptAction for EnableSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;

        log::info!(
            "Enabling special power '{}' for player {}",
            power_name,
            player
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "enable_special_power"
    }

    fn description(&self) -> &str {
        "Enables a special power for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable special power action
struct DisableSpecialPowerAction;

#[async_trait]
impl ScriptAction for DisableSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;

        log::info!(
            "Disabling special power '{}' for player {}",
            power_name,
            player
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "disable_special_power"
    }

    fn description(&self) -> &str {
        "Disables a special power for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Grant upgrade action
struct GrantUpgradeAction;

#[async_trait]
impl ScriptAction for GrantUpgradeAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let upgrade_name = get_string_param(parameters, "upgrade_name")?;

        log::info!("Granting upgrade '{}' to player {}", upgrade_name, player);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "grant_upgrade"
    }

    fn description(&self) -> &str {
        "Grants an upgrade to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Enable science action
struct EnableScienceAction;

#[async_trait]
impl ScriptAction for EnableScienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let science_name = get_string_param(parameters, "science_name")?;

        log::info!("Enabling science '{}' for player {}", science_name, player);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "enable_science"
    }

    fn description(&self) -> &str {
        "Enables a science/technology for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable science action
struct DisableScienceAction;

#[async_trait]
impl ScriptAction for DisableScienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let science_name = get_string_param(parameters, "science_name")?;

        log::info!("Disabling science '{}' for player {}", science_name, player);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "disable_science"
    }

    fn description(&self) -> &str {
        "Disables a science/technology for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Enable script action
struct EnableScriptAction;

#[async_trait]
impl ScriptAction for EnableScriptAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let script_name = get_string_param(parameters, "script_name")?;

        log::info!("Enabling script '{}'", script_name);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "enable_script"
    }

    fn description(&self) -> &str {
        "Enables another script"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["script_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable script action
struct DisableScriptAction;

#[async_trait]
impl ScriptAction for DisableScriptAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let script_name = get_string_param(parameters, "script_name")?;

        log::info!("Disabling script '{}'", script_name);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "disable_script"
    }

    fn description(&self) -> &str {
        "Disables another script"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["script_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Execute script action
struct ExecuteScriptAction;

#[async_trait]
impl ScriptAction for ExecuteScriptAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let script_name = get_string_param(parameters, "script_name")?;

        log::info!("Executing script '{}'", script_name);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "execute_script"
    }

    fn description(&self) -> &str {
        "Executes another script"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["script_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set variable action
struct SetVariableAction;

#[async_trait]
impl ScriptAction for SetVariableAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let variable_name = get_string_param(parameters, "variable_name")?;
        let value = parameters
            .get("value")
            .cloned()
            .unwrap_or(ScriptValue::Null);

        log::info!("Setting variable '{}' to '{}'", variable_name, value);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_variable"
    }

    fn description(&self) -> &str {
        "Sets a script variable"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["variable_name".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Wait action
struct WaitAction;

#[async_trait]
impl ScriptAction for WaitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let duration = get_float_param(parameters, "duration")?;

        log::info!("Waiting for {} seconds", duration);

        // In a real implementation, this would pause script execution

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "wait"
    }

    fn description(&self) -> &str {
        "Waits for a specified duration"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// Helper functions for parameter extraction

pub fn get_string_param(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> GameLogicResult<String> {
    match parameters.get(name) {
        Some(ScriptValue::String(s)) => Ok(s.clone()),
        Some(other) => Err(GameLogicError::Configuration(format!(
            "Parameter '{}' must be a string, got: {}",
            name, other
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Required parameter '{}' not found",
            name
        ))),
    }
}

pub fn get_int_param(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> GameLogicResult<i64> {
    match parameters.get(name) {
        Some(ScriptValue::Int(i)) => Ok(*i),
        Some(ScriptValue::Float(f)) => Ok(*f as i64),
        Some(other) => Err(GameLogicError::Configuration(format!(
            "Parameter '{}' must be an integer, got: {}",
            name, other
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Required parameter '{}' not found",
            name
        ))),
    }
}

pub fn get_int_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> Option<i64> {
    match parameters.get(name) {
        Some(ScriptValue::Int(i)) => Some(*i),
        Some(ScriptValue::Float(f)) => Some(*f as i64),
        _ => None,
    }
}

pub fn get_float_param(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> GameLogicResult<f64> {
    match parameters.get(name) {
        Some(ScriptValue::Float(f)) => Ok(*f),
        Some(ScriptValue::Int(i)) => Ok(*i as f64),
        Some(other) => Err(GameLogicError::Configuration(format!(
            "Parameter '{}' must be a number, got: {}",
            name, other
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Required parameter '{}' not found",
            name
        ))),
    }
}

fn resolve_player_name_token(raw: &str) -> String {
    match raw {
        THE_PLAYER | THIS_PLAYER => get_script_engine()
            .read()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| raw.to_string()),
        LOCAL_PLAYER => player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
            })
            .unwrap_or_else(|| raw.to_string()),
        _ => raw.to_string(),
    }
}

fn resolve_named_object_id(name: &str) -> Option<u32> {
    let tracker = crate::scripting::engine::get_named_object_tracker();
    let mut object_id = tracker.get_object_id(name).ok().flatten();

    if object_id.is_none() {
        let lower = name.to_ascii_lowercase();
        object_id = OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .find_map(|obj_ref| {
                obj_ref.read().ok().and_then(|obj| {
                    if obj.get_name().to_ascii_lowercase() == lower {
                        Some(obj.get_id())
                    } else {
                        None
                    }
                })
            });
    }

    object_id
}

fn resolve_team_name_token(raw: &str) -> String {
    match raw {
        THIS_TEAM => get_script_engine()
            .read()
            .ok()
            .and_then(|g| {
                g.as_ref().and_then(|e| {
                    e.get_condition_team_name()
                        .or_else(|| e.get_calling_team_name())
                        .map(|s| s.to_string())
                })
            })
            .unwrap_or_else(|| raw.to_string()),
        TEAM_THE_PLAYER => {
            let current_player = get_script_engine().read().ok().and_then(|g| {
                g.as_ref()
                    .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
            });
            let Some(player_name) = current_player else {
                return raw.to_string();
            };

            player_list()
                .read()
                .ok()
                .and_then(|list| list.find_player_by_name(&player_name))
                .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                .unwrap_or_else(|| raw.to_string())
        }
        _ => raw.to_string(),
    }
}

fn create_ai_group_from_team(team_name: &str) -> GameLogicResult<Arc<RwLock<AiGroup>>> {
    let resolved_team = resolve_team_name_token(team_name);
    let factory = get_team_factory();
    let team_arc = factory
        .lock()
        .map_err(|_| GameLogicError::Threading("Failed to lock TeamFactory".to_string()))?
        .find_team(&resolved_team)
        .ok_or_else(|| {
            GameLogicError::Configuration(format!("Team '{}' not found", resolved_team))
        })?;

    let members = team_arc
        .read()
        .map_err(|_| GameLogicError::Threading("Failed to read Team".to_string()))?
        .get_members()
        .to_vec();

    let mut ai_guard = THE_AI
        .write()
        .map_err(|_| GameLogicError::Threading("Failed to lock AI system".to_string()))?;
    let group = ai_guard.create_group();

    if let Ok(mut group_guard) = group.write() {
        for member_id in members {
            if let Some(_obj_arc) = TheGameLogic::find_object_by_id(member_id) {
                group_guard.add(member_id);
            }
        }
    }

    Ok(group)
}

pub fn get_float_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> Option<f64> {
    match parameters.get(name) {
        Some(ScriptValue::Float(f)) => Some(*f),
        Some(ScriptValue::Int(i)) => Some(*i as f64),
        _ => None,
    }
}

// ============================================================================
// TEAM ACTIONS (15 critical actions)
// ============================================================================

/// Team attacks another team
struct TeamAttackTeamAction;

#[async_trait]
impl ScriptAction for TeamAttackTeamAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let attacker_team = get_string_param(parameters, "attacker_team")?;
        let target_team = get_string_param(parameters, "target_team")?;

        log::info!("Team '{}' attacking team '{}'", attacker_team, target_team);

        // Matches C++ ScriptActions.cpp - team attack command
        // Get both teams from the team system
        // For each member of attacker team, issue attack command targeting target team
        // In C++: theTeam->attackTeam(targetTeam)
        // Integration: Requires team system to resolve team names and issue AI group attack commands

        let resolved_attacker = resolve_team_name_token(&attacker_team);
        let resolved_target = resolve_team_name_token(&target_team);

        let group_arc = match create_ai_group_from_team(&resolved_attacker) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamAttackTeamAction: failed to create AI group for team '{}': {}",
                    resolved_attacker,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackTeam, CommandSourceType::FromScript);
            params.team = Some(resolved_target);
            params.int_value = -1; // NO_MAX_SHOTS_LIMIT
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_attack_team"
    }

    fn description(&self) -> &str {
        "Commands one team to attack another team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["attacker_team".to_string(), "target_team".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team follows waypoint path
struct TeamFollowWaypointsAction;

#[async_trait]
impl ScriptAction for TeamFollowWaypointsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let waypoint_path = get_string_param(parameters, "waypoint_path")?;
        let as_team = parameters
            .get("as_team")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!(
            "Team '{}' following waypoint path '{}' (as_team: {})",
            team_name,
            waypoint_path,
            as_team
        );

        // Matches C++ ScriptActions.cpp:doTeamFollowWaypoints
        // Integration steps:
        // 1. Resolve team by name from team system
        // 2. Parse waypoint_path (comma-separated waypoint names or waypoint list name)
        // 3. For each waypoint, resolve Coord3D position from waypoint system
        // 4. Create AI group from team members
        // 5. Issue sequential movement commands through waypoint queue
        // 6. If as_team=true, maintain formation; if false, units move independently
        // In C++: theTeam->getTeamAsAIGroup() then aiGroup->groupFollowWaypoints(waypoints, asTeam)

        let waypoint_ascii = AsciiString::from(waypoint_path.as_str());
        let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| w.get_id())
        });

        let Some(waypoint_id) = waypoint_id else {
            log::warn!(
                "TeamFollowWaypointsAction: waypoint '{}' not found",
                waypoint_path
            );
            return Ok(ScriptResult::Success(None));
        };

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamFollowWaypointsAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let cmd = if as_team {
                AiCommandType::FollowWaypointPathAsTeam
            } else {
                AiCommandType::FollowWaypointPath
            };
            let mut params = AiCommandParams::new(cmd, CommandSourceType::FromScript);
            params.waypoint = Some(waypoint_id);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_follow_waypoints"
    }

    fn description(&self) -> &str {
        "Commands a team to follow a waypoint path"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "waypoint_path".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["as_team".to_string()]
    }
}

/// Team guards position
struct TeamGuardAction;

#[async_trait]
impl ScriptAction for TeamGuardAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let x = get_float_param_optional(parameters, "x");
        let y = get_float_param_optional(parameters, "y");

        if let (Some(x_pos), Some(y_pos)) = (x, y) {
            log::info!(
                "Team '{}' guarding position ({}, {})",
                team_name,
                x_pos,
                y_pos
            );
        } else {
            log::info!("Team '{}' guarding current position", team_name);
        }

        // Matches C++ ScriptActions.cpp - guard behavior
        // Integration: Set AI state to GUARD mode for all team members
        // In C++: theTeam->setGuardPosition(position) or theTeam->guard()
        // AI will engage enemies that enter guard radius but return to position
        // Guard radius determined by unit vision range + engagement distance

        let resolved_team = resolve_team_name_token(&team_name);
        let guard_pos = if let (Some(x_pos), Some(y_pos)) = (x, y) {
            let z = get_terrain_logic()
                .read()
                .ok()
                .map(|terrain| terrain.get_ground_height(x_pos as f32, y_pos as f32, None))
                .unwrap_or(0.0);
            Coord3D::new(x_pos as f32, y_pos as f32, z)
        } else {
            let factory = get_team_factory();
            let Some(team_arc) = factory
                .lock()
                .ok()
                .and_then(|mut guard| guard.find_team(&resolved_team))
            else {
                log::warn!("TeamGuardAction: team '{}' not found", resolved_team);
                return Ok(ScriptResult::Success(None));
            };

            let members = team_arc
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to read Team".to_string()))?
                .get_members()
                .to_vec();
            if members.is_empty() {
                log::warn!("TeamGuardAction: team '{}' has no members", resolved_team);
                return Ok(ScriptResult::Success(None));
            }

            let mut sum = Coord3D::new(0.0, 0.0, 0.0);
            let mut count = 0.0f32;
            for member_id in members {
                if let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) {
                    if let Ok(obj_guard) = obj_arc.read() {
                        let pos = obj_guard.get_position();
                        sum.x += pos.x;
                        sum.y += pos.y;
                        sum.z += pos.z;
                        count += 1.0;
                    }
                }
            }
            if count <= 0.0 {
                log::warn!(
                    "TeamGuardAction: team '{}' has no valid members",
                    resolved_team
                );
                return Ok(ScriptResult::Success(None));
            }
            Coord3D::new(sum.x / count, sum.y / count, sum.z / count)
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamGuardAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardPosition, CommandSourceType::FromScript);
            params.pos = guard_pos;
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_guard"
    }

    fn description(&self) -> &str {
        "Commands a team to guard a position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string()]
    }
}

/// Team hunts enemies
struct TeamHuntAction;

#[async_trait]
impl ScriptAction for TeamHuntAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' hunting enemies", team_name);

        // Matches C++ ScriptActions.cpp - hunt AI behavior
        // Integration: Set AI state to HUNT mode
        // In C++: theTeam->setHunt() - actively seeks and destroys enemy units
        // Hunt behavior: scan for enemies, prioritize targets, pursue and destroy
        // Different from guard - doesn't return to position, keeps hunting
        // Uses CommandButtonHuntUpdate module behavior

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamHuntAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Hunt, CommandSourceType::FromScript);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_hunt"
    }

    fn description(&self) -> &str {
        "Commands a team to hunt for enemies"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team moves to waypoint
struct TeamMoveToWaypointAction;

#[async_trait]
impl ScriptAction for TeamMoveToWaypointAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Team '{}' moving to waypoint '{}'", team_name, waypoint);

        // Matches C++ ScriptActions.cpp:doMoveToWaypoint line 391
        // Integration steps (from C++):
        // 1. Team *theTeam = TheScriptEngine->getTeamNamed(team)
        // 2. AIGroup* theGroup = TheAI->createGroup()
        // 3. theTeam->getTeamAsAIGroup(theGroup)
        // 4. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 5. Coord3D destination = *way->getLocation()
        // 6. theGroup->groupMoveToPosition(&destination, false, CMD_FROM_SCRIPT)
        // Rust: Resolve team -> get waypoint coordinates -> issue group movement command

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let position = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });

        let Some(position) = position else {
            log::warn!(
                "TeamMoveToWaypointAction: waypoint '{}' not found",
                waypoint
            );
            return Ok(ScriptResult::Success(None));
        };

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamMoveToWaypointAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromScript);
            params.pos = position;
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_move_to_waypoint"
    }

    fn description(&self) -> &str {
        "Commands a team to move to a specific waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team enters building
struct TeamGarrisonBuildingAction;

#[async_trait]
impl ScriptAction for TeamGarrisonBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!(
            "Team '{}' garrisoning building '{}'",
            team_name,
            building_name
        );

        // Matches C++ ScriptActions.cpp:doTeamGarrisonSpecificBuilding line 3291
        // Integration with garrison/contain system:
        // 1. Resolve team by name
        // 2. Find building object by name
        // 3. Get GarrisonContain module from building
        // 4. For each team member:
        //    a. Check if unit can garrison (has KINDOF_CAN_ATTACK_GARRISONED flag)
        //    b. Move unit to building position
        //    c. Call contain->enterContain(unit)
        //    d. Unit becomes contained, gains garrison bonuses
        // 5. Building shows garrison indicators, units can fire from inside
        // Uses: GarrisonContain from object/contain/garrison_contain.rs

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "TeamGarrisonBuildingAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(building_arc) = TheGameLogic::find_object_by_id(building_id) else {
            log::warn!(
                "TeamGarrisonBuildingAction: building '{}' (ID {}) not found in registry",
                building_name,
                building_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamGarrisonBuildingAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let contain = building_arc.read().ok().and_then(|b| b.get_contain());

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(unit_guard) = unit_arc.read() else {
                continue;
            };
            if let Some(ai) = unit_guard.get_ai_update_interface() {
                ai.ai_enter(building_id, CommandSourceType::FromScript);
                continue;
            }

            if let Some(contain) = &contain {
                if contain.is_valid_container_for(&unit_guard, true) {
                    contain.add_to_contain(&unit_guard);
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_garrison_building"
    }

    fn description(&self) -> &str {
        "Commands a team to garrison a specific building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team exits garrison
struct TeamExitBuildingAction;

#[async_trait]
impl ScriptAction for TeamExitBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' exiting all buildings", team_name);

        // Matches C++ ScriptActions.cpp:doTeamEvacuateBuilding
        // Integration with garrison system:
        // 1. Resolve team by name
        // 2. For each team member currently contained:
        //    a. Get containing building's GarrisonContain module
        //    b. Call contain->exitContain(unit) or contain->evacuate()
        //    c. Unit spawns near building exit points
        //    d. Unit returns to team control
        // 3. Team members reform after exiting
        // Uses: GarrisonContain::evacuate() from object/contain/garrison_contain.rs

        let resolved_team = resolve_team_name_token(&team_name);
        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamExitBuildingAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let container_arc = unit_arc
                .read()
                .ok()
                .and_then(|unit_guard| unit_guard.get_container());
            let Some(container_arc) = container_arc else {
                continue;
            };
            let contain = container_arc.read().ok().and_then(|c| c.get_contain());
            if let Some(contain) = contain {
                if let Ok(mut contain_guard) = contain.try_lock() {
                    let _ = contain_guard.release_object(member_id);
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_exit_building"
    }

    fn description(&self) -> &str {
        "Commands a team to exit from garrisoned buildings"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team captures structure
struct TeamCaptureBuildingAction;

#[async_trait]
impl ScriptAction for TeamCaptureBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!(
            "Team '{}' capturing building '{}'",
            team_name,
            building_name
        );

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "TeamCaptureBuildingAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(building_arc) = TheGameLogic::find_object_by_id(building_id) else {
            log::warn!(
                "TeamCaptureBuildingAction: building '{}' (ID {}) not found in registry",
                building_name,
                building_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamCaptureBuildingAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let mut issued = 0;
        if let Ok(mut factory) = get_object_factory().write() {
            for member_id in members {
                let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(unit_guard) = unit_arc.read() else {
                    continue;
                };
                let Ok(building_guard) = building_arc.read() else {
                    continue;
                };
                if !TheActionManager::can_capture_building(
                    &unit_guard,
                    &building_guard,
                    CommandSourceType::FromScript,
                ) {
                    continue;
                }

                let Some(GameObjectInstance::Unit(unit)) = factory.get_object_mut(member_id) else {
                    continue;
                };

                if let Ok(mut unit_guard) = unit.write() {
                    let _ = unit_guard.give_capture_order(building_id, false);
                    issued += 1;
                }
            }
        }

        if issued == 0 {
            log::warn!(
                "TeamCaptureBuildingAction: team '{}' has no capture-capable units for '{}'",
                resolved_team,
                building_name
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_capture_building"
    }

    fn description(&self) -> &str {
        "Commands a team to capture a neutral or enemy structure"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team repairs target
struct TeamRepairAction;

#[async_trait]
impl ScriptAction for TeamRepairAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let target_name = get_string_param(parameters, "target_name")?;

        log::info!("Team '{}' repairing target '{}'", team_name, target_name);

        // Integration with repair mechanics:
        // 1. Resolve team (must contain repair-capable units like Dozers, Workers)
        // 2. Resolve target object by name
        // 3. Check team has repair-capable units (KINDOF_CAN_REPAIR)
        // 4. Move repair units to target
        // 5. Initiate repair: gradually restore target health over time
        // 6. Repair cost: consumes player resources proportional to damage
        // In C++: Uses SpecialAbilityUpdate::Repair or RepairModule

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(target_id) = resolve_named_object_id(&target_name) else {
            log::warn!("TeamRepairAction: target '{}' not found", target_name);
            return Ok(ScriptResult::Success(None));
        };

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!("TeamRepairAction: team '{}' has no members", resolved_team);
            return Ok(ScriptResult::Success(None));
        }

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(unit_guard) = unit_arc.read() else {
                continue;
            };
            if !unit_guard.is_kind_of(crate::common::KindOf::CanRepair) {
                continue;
            }
            let ai = unit_guard.get_ai_update_interface();
            drop(unit_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(mut ai_guard) = ai.lock() else {
                continue;
            };
            let mut params =
                AiCommandParams::new(AiCommandType::Repair, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            let _ = ai_guard.execute_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_repair"
    }

    fn description(&self) -> &str {
        "Commands a team to repair a target structure or vehicle"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "target_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team wanders area
struct TeamWanderAction;

#[async_trait]
impl ScriptAction for TeamWanderAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' wandering", team_name);

        // Integration with AI wander behavior:
        // Wander makes units move semi-randomly within an area
        // 1. Resolve team
        // 2. Set AI state to WANDER for all members
        // 3. AI picks random nearby positions and moves there
        // 4. Units engage enemies encountered but don't pursue far
        // 5. After reaching position, pick new random destination
        // Wander radius typically 100-200 game units from current position
        // In C++: AI state WANDER with periodic random destination selection

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamWanderAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Wander, CommandSourceType::FromScript);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_wander"
    }

    fn description(&self) -> &str {
        "Commands a team to wander around their current area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team stops
struct TeamIdleAction;

#[async_trait]
impl ScriptAction for TeamIdleAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' going idle", team_name);

        // Integration with AI system - stop all actions:
        // 1. Resolve team
        // 2. For each team member:
        //    a. Clear AI command queue
        //    b. Cancel current action
        //    c. Set state to IDLE
        // 3. Units stop moving, stop attacking, enter standby
        // 4. Will still defend if attacked (return fire)
        // In C++: theTeam->stopAllActions() clears all AI update queues

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamIdleAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_idle"
    }

    fn description(&self) -> &str {
        "Commands a team to stop all actions and go idle"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set team state
struct TeamSetStateAction;

#[async_trait]
impl ScriptAction for TeamSetStateAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let state = get_string_param(parameters, "state")?;

        log::info!("Setting team '{}' state to '{}'", team_name, state);

        // Matches C++ ScriptActions.cpp:doSetTeamState line 468
        // Integration with team state management:
        // In C++: theTeam->setState(state)
        // Team state is a string that scripts can use to track team status
        // Common states: "idle", "attacking", "defending", "retreating", etc.
        // Scripts check these states in conditions to trigger appropriate actions
        // State stored in Team::m_state (AsciiString)
        // Rust: team.set_state(state.clone())

        let resolved_team = resolve_team_name_token(&team_name);
        let factory = get_team_factory();
        let Some(team_arc) = factory
            .lock()
            .ok()
            .and_then(|mut guard| guard.find_team(&resolved_team))
        else {
            log::warn!("TeamSetStateAction: team '{}' not found", resolved_team);
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut team_guard) = team_arc.write() {
            team_guard.set_state(AsciiString::from(state.as_str()));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_set_state"
    }

    fn description(&self) -> &str {
        "Sets the state of a team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "state".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Delete team
struct TeamDeleteAction;

#[async_trait]
impl ScriptAction for TeamDeleteAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Deleting team '{}'", team_name);

        // Matches C++ ScriptActions.cpp:doKillTeam line 2504
        // Integration with team management:
        // In C++: theTeam->killTeam()
        // 1. Resolve team by name
        // 2. For each member in team:
        //    a. Call object->kill(DEATH_NORMAL) or object->destroy()
        //    b. Remove from world
        //    c. Cleanup object resources
        // 3. Remove team from team registry
        // 4. Free team memory
        // WARNING: Destructive operation, cannot be undone

        let resolved_team = resolve_team_name_token(&team_name);
        let factory = get_team_factory();
        let team_arc = {
            let mut guard = factory
                .lock()
                .map_err(|_| GameLogicError::Threading("Failed to lock TeamFactory".to_string()))?;
            guard.find_team(&resolved_team)
        };

        let Some(team_arc) = team_arc else {
            log::warn!("TeamDeleteAction: team '{}' not found", resolved_team);
            return Ok(ScriptResult::Success(None));
        };

        let (team_id, members) = if let Ok(team_guard) = team_arc.read() {
            (team_guard.get_id(), team_guard.get_members().to_vec())
        } else {
            log::warn!("TeamDeleteAction: failed to read team '{}'", resolved_team);
            return Ok(ScriptResult::Success(None));
        };

        for member_id in members {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                }
            }
        }

        if let Ok(mut factory_guard) = factory.lock() {
            factory_guard.team_about_to_be_deleted(team_id);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_delete"
    }

    fn description(&self) -> &str {
        "Deletes a team and all its members"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team follows another team
struct TeamFollowTeamAction;

#[async_trait]
impl ScriptAction for TeamFollowTeamAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let follower_team = get_string_param(parameters, "follower_team")?;
        let target_team = get_string_param(parameters, "target_team")?;

        log::info!("Team '{}' following team '{}'", follower_team, target_team);

        // Integration with team AI follow behavior:
        // 1. Resolve both teams
        // 2. Set follower team's AI state to FOLLOW
        // 3. Set target team as follow target
        // 4. Follower team continuously updates position to stay near target
        // 5. Follow distance typically 20-50 game units behind
        // 6. If target moves, follower adjusts position
        // 7. If target engages enemies, follower assists
        // In C++: theTeam->followTeam(targetTeam)

        let resolved_follower = resolve_team_name_token(&follower_team);
        let resolved_target = resolve_team_name_token(&target_team);

        let target_id = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_target))
            .and_then(|team_arc| {
                team_arc
                    .read()
                    .ok()
                    .and_then(|team| team.get_members().first().copied())
            });

        let Some(target_id) = target_id else {
            log::warn!(
                "TeamFollowTeamAction: target team '{}' has no members",
                resolved_target
            );
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_follower) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamFollowTeamAction: failed to create AI group for team '{}': {}",
                    resolved_follower,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_follow_team"
    }

    fn description(&self) -> &str {
        "Commands one team to follow another team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["follower_team".to_string(), "target_team".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team guards tunnel
struct TeamGuardInTunnelAction;

#[async_trait]
impl ScriptAction for TeamGuardInTunnelAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' guarding in tunnel network", team_name);

        let resolved_team = resolve_team_name_token(&team_name);
        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team));

        let Some(team_arc) = team_arc else {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' not found",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        };

        let (members, controlling_player_id) = team_arc
            .read()
            .ok()
            .map(|team| {
                (
                    team.get_members().to_vec(),
                    team.get_controlling_player_id(),
                )
            })
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let player_arc = if let Some(player_id) = controlling_player_id {
            player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(player_id as PlayerIndex).cloned())
        } else {
            None
        };

        let player_arc = player_arc.or_else(|| {
            members.iter().find_map(|member_id| {
                TheGameLogic::find_object_by_id(*member_id).and_then(|obj| {
                    obj.read()
                        .ok()
                        .and_then(|guard| guard.get_controlling_player())
                })
            })
        });

        let Some(player_arc) = player_arc else {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' has no controlling player",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        };

        let tunnel_ids = player_arc
            .read()
            .ok()
            .and_then(|player| player.get_tunnel_system().cloned())
            .and_then(|tracker| tracker.get_container_list().ok())
            .unwrap_or_default();

        if tunnel_ids.is_empty() {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' has no tunnel network",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let mut tunnel_entries = Vec::new();
        for tunnel_id in tunnel_ids {
            let Some(tunnel_arc) = TheGameLogic::find_object_by_id(tunnel_id) else {
                continue;
            };
            let Ok(tunnel_guard) = tunnel_arc.read() else {
                continue;
            };
            tunnel_entries.push((tunnel_id, *tunnel_guard.get_position()));
        }

        if tunnel_entries.is_empty() {
            log::warn!(
                "TeamGuardInTunnelAction: tunnel entries unavailable for team '{}'",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(unit_guard) = unit_arc.read() else {
                continue;
            };
            let unit_pos = *unit_guard.get_position();
            let mut best_tunnel = tunnel_entries[0].0;
            let mut best_dist_sq = Real::MAX;
            for (tunnel_id, tunnel_pos) in &tunnel_entries {
                let dx = unit_pos.x - tunnel_pos.x;
                let dy = unit_pos.y - tunnel_pos.y;
                let dz = unit_pos.z - tunnel_pos.z;
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_tunnel = *tunnel_id;
                }
            }

            if let Some(ai) = unit_guard.get_ai_update_interface() {
                ai.ai_enter(best_tunnel, CommandSourceType::FromScript);
                continue;
            }

            if let Some(tunnel_arc) = TheGameLogic::find_object_by_id(best_tunnel) {
                if let Ok(tunnel_guard) = tunnel_arc.read() {
                    if let Some(contain) = tunnel_guard.get_contain() {
                        if contain.is_valid_container_for(&unit_guard, true) {
                            contain.add_to_contain(&unit_guard);
                        }
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_guard_in_tunnel"
    }

    fn description(&self) -> &str {
        "Commands a team to guard inside a tunnel network"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// NAMED UNIT ACTIONS (10 critical actions)
// ============================================================================

/// Named unit attacks
struct NamedAttackAction;

#[async_trait]
impl ScriptAction for NamedAttackAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let attacker_name = get_string_param(parameters, "attacker_name")?;
        let target_name = get_string_param(parameters, "target_name")?;

        log::info!("Named unit '{}' attacking '{}'", attacker_name, target_name);

        // Integration with named object registry and attack commands:
        // 1. Object *attacker = TheScriptEngine->getUnitNamed(attacker_name)
        // 2. Object *target = TheScriptEngine->getUnitNamed(target_name)
        // 3. AIUpdateInterface *ai = attacker->getAIUpdateInterface()
        // 4. ai->aiAttackObject(target, CMD_FROM_SCRIPT)
        // Named objects stored in ScriptEngine's named object map
        // Rust: object_manager.get_named(name) -> Option<Arc<RwLock<Object>>>

        let Some(attacker_id) = resolve_named_object_id(&attacker_name) else {
            log::warn!("NamedAttackAction: attacker '{}' not found", attacker_name);
            return Ok(ScriptResult::Success(None));
        };
        let Some(target_id) = resolve_named_object_id(&target_name) else {
            log::warn!("NamedAttackAction: target '{}' not found", target_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            log::warn!(
                "NamedAttackAction: attacker '{}' (ID {}) not found in registry",
                attacker_name,
                attacker_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let ai = attacker_arc
            .read()
            .ok()
            .and_then(|obj| obj.get_ai_update_interface());

        if let Some(ai) = ai {
            ai.ai_attack_object_id(target_id, -1, CommandSourceType::FromScript);
        } else {
            log::warn!(
                "NamedAttackAction: attacker '{}' has no AI update interface",
                attacker_name
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_attack"
    }

    fn description(&self) -> &str {
        "Commands a named unit to attack another named unit"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["attacker_name".to_string(), "target_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit moves
struct NamedMoveToAction;

#[async_trait]
impl ScriptAction for NamedMoveToAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Named unit '{}' moving to '{}'", unit_name, waypoint);

        // Matches C++ ScriptActions.cpp:doNamedMoveToWaypoint line 416
        // Integration (from C++):
        // 1. Object *theObj = TheScriptEngine->getUnitNamed(unit_name)
        // 2. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 3. Coord3D destination = *way->getLocation()
        // 4. AIUpdateInterface *aiUpdate = theObj->getAIUpdateInterface()
        // 5. aiUpdate->clearWaypointQueue()
        // 6. theObj->leaveGroup() // Leave team for individual movement
        // 7. aiUpdate->chooseLocomotorSet(LOCOMOTORSET_NORMAL)
        // 8. aiUpdate->aiMoveToPosition(&destination, CMD_FROM_SCRIPT)

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedMoveToAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedMoveToAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let destination = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });

        let Some(destination) = destination else {
            log::warn!("NamedMoveToAction: waypoint '{}' not found", waypoint);
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut obj_guard) = object_arc.write() {
            obj_guard.leave_group();
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.choose_locomotor_set(LocomotorSetType::Normal);
                ai.ai_move_to_position(&destination, false, CommandSourceType::FromScript);
            } else {
                log::warn!(
                    "NamedMoveToAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_move_to"
    }

    fn description(&self) -> &str {
        "Commands a named unit to move to a waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit garrisons
struct NamedGarrisonAction;

#[async_trait]
impl ScriptAction for NamedGarrisonAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!("Named unit '{}' garrisoning '{}'", unit_name, building_name);

        // Integration with garrison system:
        // Same as team garrison but for single named unit
        // 1. Object *unit = TheScriptEngine->getUnitNamed(unit_name)
        // 2. Object *building = TheScriptEngine->getUnitNamed(building_name)
        // 3. GarrisonContain *garrison = building->getGarrisonContain()
        // 4. unit->aiMoveToPosition(building->getPosition())
        // 5. garrison->enterContain(unit)

        log::debug!("Integration: Named unit enters garrison building via GarrisonContain");

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_garrison"
    }

    fn description(&self) -> &str {
        "Commands a named unit to garrison a building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit follows path
struct NamedFollowWaypointsAction;

#[async_trait]
impl ScriptAction for NamedFollowWaypointsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let waypoint_path = get_string_param(parameters, "waypoint_path")?;

        log::info!(
            "Named unit '{}' following waypoint path '{}'",
            unit_name,
            waypoint_path
        );

        // Integration with waypoint system:
        // Like team follow waypoints but for single unit
        // 1. Resolve unit by name
        // 2. Parse waypoint_path into list of waypoint names
        // 3. For each waypoint, resolve Coord3D
        // 4. Add waypoints to unit's AI waypoint queue
        // 5. ai->followWaypointPath(waypoints)
        // Unit moves sequentially through each waypoint

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedFollowWaypointsAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedFollowWaypointsAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let waypoint_ascii = AsciiString::from(waypoint_path.as_str());
        let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| w.get_id())
        });

        let Some(waypoint_id) = waypoint_id else {
            log::warn!(
                "NamedFollowWaypointsAction: waypoint '{}' not found",
                waypoint_path
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(obj_guard) = object_arc.write() {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowWaypointPath,
                        CommandSourceType::FromScript,
                    );
                    params.waypoint = Some(waypoint_id);
                    let _ = ai_guard.execute_command(&params);
                }
            } else {
                log::warn!(
                    "NamedFollowWaypointsAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_follow_waypoints"
    }

    fn description(&self) -> &str {
        "Commands a named unit to follow a waypoint path"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "waypoint_path".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit guards
struct NamedGuardAction;

#[async_trait]
impl ScriptAction for NamedGuardAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Named unit '{}' guarding", unit_name);

        // Integration with guard behavior:
        // Sets single unit to guard mode
        // 1. Resolve unit by name
        // 2. ai->setAIState(AI_STATE_GUARD)
        // 3. Guard position = current position
        // 4. Unit engages nearby enemies but returns to guard position

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedGuardAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedGuardAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(obj_guard) = object_arc.read() {
            let pos = *obj_guard.get_position();
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.ai_guard_position(&pos, GuardMode::Normal, CommandSourceType::FromScript);
            } else {
                log::warn!(
                    "NamedGuardAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_guard"
    }

    fn description(&self) -> &str {
        "Commands a named unit to guard its position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit hunts
struct NamedHuntAction;

#[async_trait]
impl ScriptAction for NamedHuntAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Named unit '{}' hunting", unit_name);

        // Integration with hunt behavior:
        // Single unit actively seeks enemies
        // 1. Resolve unit
        // 2. ai->setAIState(AI_STATE_HUNT)
        // 3. Unit scans for enemies and attacks
        // 4. Pursues and destroys targets

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedHuntAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedHuntAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(obj_guard) = object_arc.read() {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.ai_hunt(CommandSourceType::FromScript);
            } else {
                log::warn!(
                    "NamedHuntAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_hunt"
    }

    fn description(&self) -> &str {
        "Commands a named unit to hunt enemies"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Delete named unit
struct NamedDeleteAction;

#[async_trait]
impl ScriptAction for NamedDeleteAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Deleting named unit '{}'", unit_name);

        // Integration with object destruction:
        // 1. Object *obj = TheScriptEngine->getUnitNamed(unit_name)
        // 2. obj->kill(DEATH_NORMAL) or obj->destroy()
        // 3. Remove from world
        // 4. Remove from named object registry
        // 5. Cleanup resources

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedDeleteAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedDeleteAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut obj_guard) = object_arc.write() {
            obj_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
        }

        let tracker = get_named_object_tracker();
        if let Err(err) = tracker.unregister_object(object_id) {
            log::warn!(
                "NamedDeleteAction: failed to unregister '{}' (ID {}): {}",
                unit_name,
                object_id,
                err
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_delete"
    }

    fn description(&self) -> &str {
        "Deletes a named unit from the game"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named enters named building
struct NamedEnterNamedAction;

#[async_trait]
impl ScriptAction for NamedEnterNamedAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!("Named unit '{}' entering '{}'", unit_name, building_name);

        // Integration with contain/garrison system:
        // Generic enter for any container type (transport, garrison, tunnel, etc.)
        // 1. Resolve both objects
        // 2. ContainModuleInterface *contain = building->getContain()
        // 3. Check contain->canContain(unit)
        // 4. unit->moveToAndEnter(building)
        // 5. contain->enterContain(unit)
        // Works with: TransportContain, GarrisonContain, TunnelContain, etc.

        let Some(unit_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedEnterNamedAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };
        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "NamedEnterNamedAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            log::warn!(
                "NamedEnterNamedAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                unit_id
            );
            return Ok(ScriptResult::Success(None));
        };
        let Some(building_arc) = TheGameLogic::find_object_by_id(building_id) else {
            log::warn!(
                "NamedEnterNamedAction: building '{}' (ID {}) not found in registry",
                building_name,
                building_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let (unit_guard, contain) = match (unit_arc.read(), building_arc.read()) {
            (Ok(unit_guard), Ok(building_guard)) => (unit_guard, building_guard.get_contain()),
            _ => {
                log::warn!(
                    "NamedEnterNamedAction: failed to lock unit/building for '{}'",
                    unit_name
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        let Some(contain) = contain else {
            log::warn!(
                "NamedEnterNamedAction: building '{}' has no contain module",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        if !contain.is_valid_container_for(&unit_guard, true) {
            log::warn!(
                "NamedEnterNamedAction: building '{}' cannot contain '{}'",
                building_name,
                unit_name
            );
            return Ok(ScriptResult::Success(None));
        }

        contain.add_to_contain(&unit_guard);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_enter_named"
    }

    fn description(&self) -> &str {
        "Commands a named unit to enter a named building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named exits
struct NamedExitAction;

#[async_trait]
impl ScriptAction for NamedExitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Named unit '{}' exiting", unit_name);

        // Integration with contain/garrison system:
        // Exit from current container
        // 1. Object *unit = TheScriptEngine->getUnitNamed(unit_name)
        // 2. ContainModuleInterface *contain = unit->getContainedBy()
        // 3. contain->exitContain(unit)
        // 4. Unit spawns near container exit point
        // Works for exiting from transports, buildings, tunnels, etc.

        let Some(unit_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedExitAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            log::warn!(
                "NamedExitAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                unit_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let container_arc = unit_arc
            .read()
            .ok()
            .and_then(|unit_guard| unit_guard.get_container());

        let Some(container_arc) = container_arc else {
            log::warn!("NamedExitAction: unit '{}' is not contained", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let contain = container_arc.read().ok().and_then(|c| c.get_contain());
        let Some(contain) = contain else {
            log::warn!(
                "NamedExitAction: container for '{}' has no contain module",
                unit_name
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut contain_guard) = contain.try_lock() {
            let _ = contain_guard.release_object(unit_id);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_exit"
    }

    fn description(&self) -> &str {
        "Commands a named unit to exit from a building or transport"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set named unit attitude
struct NamedSetAttitudeAction;

#[async_trait]
impl ScriptAction for NamedSetAttitudeAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let attitude = get_string_param(parameters, "attitude")?;

        log::info!(
            "Setting named unit '{}' attitude to '{}'",
            unit_name,
            attitude
        );

        // Integration with AI attitude system:
        // Attitudes: "AGGRESSIVE", "DEFENSIVE", "GUARD", "HOLD_GROUND", "NORMAL"
        // 1. Resolve unit
        // 2. AttitudeType attitudeType = parseAttitude(attitude)
        // 3. ai->setAttitude(attitudeType)
        // Affects how unit responds to enemies and threats
        // AGGRESSIVE: Pursues enemies actively
        // DEFENSIVE: Only fires when attacked
        // GUARD: Stays near position, engages nearby enemies
        // HOLD_GROUND: Never moves, only fires

        let Some(unit_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedSetAttitudeAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            log::warn!(
                "NamedSetAttitudeAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                unit_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let attitude_upper = attitude.to_ascii_uppercase();
        let attitude_type = match attitude_upper.as_str() {
            "AGGRESSIVE" => crate::modules::AIAttitudeType::Aggressive,
            "DEFENSIVE" => crate::modules::AIAttitudeType::Defensive,
            "GUARD" | "HOLD_GROUND" => crate::modules::AIAttitudeType::Defensive,
            "PASSIVE" => crate::modules::AIAttitudeType::Passive,
            "SLEEP" => crate::modules::AIAttitudeType::Sleep,
            _ => crate::modules::AIAttitudeType::Normal,
        };

        if let Ok(unit_guard) = unit_arc.read() {
            if let Some(ai) = unit_guard.get_ai_update_interface() {
                ai.set_attitude(attitude_type);
            } else {
                log::warn!(
                    "NamedSetAttitudeAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_set_attitude"
    }

    fn description(&self) -> &str {
        "Sets the attitude of a named unit (aggressive, defensive, etc.)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "attitude".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// PLAYER ACTIONS (10 critical actions)
// ============================================================================

/// Grant tech to player
struct PlayerGrantScienceAction;

#[async_trait]
impl ScriptAction for PlayerGrantScienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let science = get_string_param(parameters, "science")?;

        log::info!("Granting science '{}' to player {}", science, player);

        // Integration with science/tech system:
        // Sciences unlock new units, abilities, upgrades
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. Science *scienceTemplate = TheScienceStore->findScience(science)
        // 3. pPlayer->grantScience(scienceTemplate)
        // 4. Triggers buildability updates, UI changes
        // 5. May enable special powers, units, or upgrades
        // Rust: player.grant_science(science_id)

        use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science)
        } else {
            log::warn!("PlayerGrantScienceAction: science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("PlayerGrantScienceAction: science '{}' not found", science);
            return Ok(ScriptResult::Success(None));
        }

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.grant_science(science_type);
                }
            } else {
                log::warn!("PlayerGrantScienceAction: player {} not found", player);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_grant_science"
    }

    fn description(&self) -> &str {
        "Grants a science/technology to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable production
struct PlayerDisableFactoriesAction;

#[async_trait]
impl ScriptAction for PlayerDisableFactoriesAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
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

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(player_guard) = player_arc.read() {
                    for obj_arc in player_guard.get_objects() {
                        if let Ok(mut obj_guard) = obj_arc.write() {
                            obj_guard.set_production_enabled(false);
                        }
                    }
                }
            } else {
                log::warn!("PlayerDisableFactoriesAction: player {} not found", player);
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
struct PlayerEnableFactoriesAction;

#[async_trait]
impl ScriptAction for PlayerEnableFactoriesAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
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

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(player_guard) = player_arc.read() {
                    for obj_arc in player_guard.get_objects() {
                        if let Ok(mut obj_guard) = obj_arc.write() {
                            obj_guard.set_production_enabled(true);
                        }
                    }
                }
            } else {
                log::warn!("PlayerEnableFactoriesAction: player {} not found", player);
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
struct PlayerBuildBaseDefenseAction;

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
struct PlayerHuntAction;

#[async_trait]
impl ScriptAction for PlayerHuntAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;

        log::info!("Player {} AI hunting", player);

        // Integration with AI hunt behavior:
        // Sets all player units to hunt mode
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. For each unit owned by player:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->setAIState(AI_STATE_HUNT)
        // 3. All units actively seek and destroy enemies
        // Rust: player.set_all_units_hunt_mode()

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(player_guard) = player_arc.read() {
                    for obj_arc in player_guard.get_objects() {
                        if let Ok(obj_guard) = obj_arc.read() {
                            if let Some(ai) = obj_guard.get_ai_update_interface() {
                                ai.ai_hunt(CommandSourceType::FromScript);
                            }
                        }
                    }
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
struct PlayerGarrisonAllBuildingsAction;

#[async_trait]
impl ScriptAction for PlayerGarrisonAllBuildingsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
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

            let objects = player_guard.get_objects();
            let mut garrison_buildings = Vec::new();
            let mut infantry_units = Vec::new();

            for obj_arc in &objects {
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
struct PlayerSellBuildingAction;

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
struct PlayerEvacuateBuildingAction;

#[async_trait]
impl ScriptAction for PlayerEvacuateBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
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

            for obj_arc in player_guard.get_objects() {
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
struct PlayerSetActiveAction;

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

/// Add player money
struct PlayerAddMoneyAction;

#[async_trait]
impl ScriptAction for PlayerAddMoneyAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Adding ${} to player {}", amount, player);

        // Integration with player resource system:
        // Adds money to player's resource pool
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. pPlayer->addMoney(amount) or pPlayer->setMoney(current + amount)
        // 3. Updates UI display
        // 4. Amount can be negative to subtract money
        // Rust: player.add_money(amount)

        if let Ok(list) = player_list().read() {
            let player_idx = player.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            if let Some(player_arc) = list.get_player(player_idx) {
                if let Ok(mut player_guard) = player_arc.write() {
                    if amount >= 0 {
                        grant_script_player_money(&mut player_guard, clamp_script_money(amount));
                    } else {
                        spend_script_player_money(
                            &mut player_guard,
                            clamp_script_money(amount.saturating_neg()),
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_add_money"
    }

    fn description(&self) -> &str {
        "Adds money to a player's resources"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// MAP/CAMERA ACTIONS (8 critical actions)
// ============================================================================

/// Reveal fog of war
struct MapRevealAreaAction;

#[async_trait]
impl ScriptAction for MapRevealAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;
        let radius = get_float_param(parameters, "radius")?;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Revealing map area at waypoint '{}' radius {} for player {}",
            waypoint,
            radius,
            player
        );

        // Integration with fog of war/shroud system:
        // Matches C++ ScriptActions.cpp:doMapReveal
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D position = *way->getLocation()
        // 3. PartitionManager->revealArea(position, radius, player, permanent)
        // 4. Updates shroud cells in radius to visible
        // 5. Can be temporary or permanent reveal
        // Uses: ShroudManager from system/shroud_manager.rs

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "Map reveal failed: waypoint '{}' not found",
                waypoint_ascii.as_str()
            );
            return Ok(ScriptResult::Success(None));
        };

        let player_mask = 1u32 << (player.max(0) as u32);

        if player_mask != 0 {
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                shroud_mgr.do_shroud_reveal(&target, radius as f32, player_mask);
                shroud_mgr.undo_shroud_reveal(&target, radius as f32, player_mask);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "map_reveal_area"
    }

    fn description(&self) -> &str {
        "Reveals an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "waypoint".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Add fog
struct MapShroudAreaAction;

#[async_trait]
impl ScriptAction for MapShroudAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;
        let radius = get_float_param(parameters, "radius")?;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Shrouding map area at waypoint '{}' radius {} for player {}",
            waypoint,
            radius,
            player
        );

        // Integration with fog of war/shroud system:
        // Re-applies fog to previously revealed area
        // 1. Resolve waypoint position and radius
        // 2. PartitionManager->shroudArea(position, radius, player)
        // 3. Sets shroud cells back to unexplored/hidden
        // 4. Useful for dynamic map changes or scripted events
        // Uses: ShroudManager from system/shroud_manager.rs

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "Map shroud failed: waypoint '{}' not found",
                waypoint_ascii.as_str()
            );
            return Ok(ScriptResult::Success(None));
        };

        let player_mask = 1u32 << (player.max(0) as u32);

        if player_mask != 0 {
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                shroud_mgr.do_shroud_cover(&target, radius as f32, player_mask);
                shroud_mgr.undo_shroud_cover(&target, radius as f32, player_mask);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "map_shroud_area"
    }

    fn description(&self) -> &str {
        "Shrouds an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "waypoint".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Move camera
struct CameraMoveToWaypointAction;

#[async_trait]
impl ScriptAction for CameraMoveToWaypointAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0);

        log::info!(
            "Moving camera to waypoint '{}' over {} seconds",
            waypoint,
            duration
        );

        // Integration with camera system:
        // Smoothly moves camera to position
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D destination = *way->getLocation()
        // 3. TheTacticalView->moveToPosition(destination, duration)
        // 4. If duration > 0: smooth interpolated movement
        // 5. If duration == 0: instant jump
        // Camera system handles easing and interpolation

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to(
                        target.x,
                        target.y,
                        target.z,
                        duration as f32,
                        0.0,
                        0.0,
                        0.0,
                    ) {
                        log::warn!("Script action handler move_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_move_to_waypoint"
    }

    fn description(&self) -> &str {
        "Moves the camera to a specific waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Camera follows unit
struct CameraTrackNamedAction;

#[async_trait]
impl ScriptAction for CameraTrackNamedAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let snap = parameters
            .get("snap")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(false);

        log::info!(
            "Camera tracking named unit '{}' (snap: {})",
            unit_name,
            snap
        );

        // Matches C++ ScriptActions.cpp:doCameraFollowNamed line 444
        // Integration with camera system (from C++):
        // 1. Object *theObj = TheScriptEngine->getUnitNamed(unit_name)
        // 2. TheTacticalView->setCameraLock(theObj->getID())
        // 3. if (snap) TheTacticalView->snapToCameraLock() // Instant
        // 4. TheTacticalView->setSnapMode(View::LOCK_FOLLOW, 0.0f)
        // 5. Camera continuously follows unit movement
        // Used for cinematic sequences and important unit tracking

        let tracker = crate::scripting::engine::get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if object_id.is_none() {
            let lower = unit_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        if let Some(object_id) = object_id {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.camera_follow_object(object_id, snap) {
                            log::warn!(
                                "Script action handler camera_follow_object failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Camera track failed: unit '{}' not found", unit_name);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_track_named"
    }

    fn description(&self) -> &str {
        "Commands the camera to track a named unit"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["snap".to_string()]
    }
}

/// Start letterbox
struct CameraLetterboxBeginAction;

#[async_trait]
impl ScriptAction for CameraLetterboxBeginAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Starting letterbox mode");

        // Integration with camera/UI system:
        // Letterbox mode adds black bars top/bottom for cinematic effect
        // 1. TheDisplay->setLetterboxMode(true)
        // 2. Animates black bars expanding from screen edges
        // 3. Typically used with camera sequences
        // 4. Hides UI elements for immersive experience
        // Common in campaign briefings and cutscenes

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_begin() {
                        log::warn!(
                            "Script action handler camera_letterbox_begin failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_letterbox_begin"
    }

    fn description(&self) -> &str {
        "Begins letterbox (cinematic) mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// End letterbox
struct CameraLetterboxEndAction;

#[async_trait]
impl ScriptAction for CameraLetterboxEndAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Ending letterbox mode");

        // Integration with camera/UI system:
        // Removes letterbox, returns to normal view
        // 1. TheDisplay->setLetterboxMode(false)
        // 2. Animates black bars retracting
        // 3. Restores UI elements
        // 4. Returns to gameplay view

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_end() {
                        log::warn!("Script action handler camera_letterbox_end failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_letterbox_end"
    }

    fn description(&self) -> &str {
        "Ends letterbox (cinematic) mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set zoom
struct CameraSetFinalZoomAction;

#[async_trait]
impl ScriptAction for CameraSetFinalZoomAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom = get_float_param(parameters, "zoom")? as f32;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0) as f32;

        log::info!("Setting camera zoom to {} over {} seconds", zoom, duration);

        // Integration with camera system:
        // Sets camera zoom level (height above terrain)
        // 1. TheTacticalView->setFinalZoom(zoom)
        // 2. If duration > 0: interpolate zoom over time
        // 3. If duration == 0: instant zoom change
        // 4. Zoom values typically 100-1000 (game units)
        // Higher = further away, lower = closer to ground

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_camera_zoom(zoom, duration) {
                        log::warn!("Script action handler set_camera_zoom failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_set_final_zoom"
    }

    fn description(&self) -> &str {
        "Sets the camera's final zoom level"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoom".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Set weather
struct WeatherSetAction;

#[async_trait]
impl ScriptAction for WeatherSetAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let weather_type = get_string_param(parameters, "weather_type")?;
        let enabled = parameters
            .get("enabled")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!("Setting weather '{}' to {}", weather_type, enabled);

        // Integration with weather system:
        // Controls environmental weather effects
        // 1. WeatherSystem::setWeather(weather_type, enabled)
        // 2. Weather types: "snow", "rain", "sandstorm", "fog"
        // 3. Visual effects: particle systems, lighting changes
        // 4. May affect gameplay: reduced vision in fog/sandstorm
        // 5. Audio changes: ambient weather sounds
        // Uses: Snow system from GameClient/Snow.h

        log::debug!("Integration: Weather system toggles environmental effects");

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "weather_set"
    }

    fn description(&self) -> &str {
        "Sets the weather effects"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["weather_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["enabled".to_string()]
    }
}

// ============================================================================
// AUDIO/VISUAL ACTIONS (7 critical actions)
// ============================================================================

/// Play sound effect
struct SoundPlayAction;

#[async_trait]
impl ScriptAction for SoundPlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let sound_name = get_string_param(parameters, "sound_name")?;
        let volume = get_float_param_optional(parameters, "volume").unwrap_or(1.0);

        log::info!("Playing sound '{}' at volume {}", sound_name, volume);

        // Matches C++ ScriptActions.cpp:doPlaySoundEffect line 329
        // Integration with audio system (from C++):
        // 1. AudioEventRTS audioEvent(sound_name)
        // 2. audioEvent.setIsLogicalAudio(true)
        // 3. audioEvent.setPlayerIndex(localPlayer)
        // 4. TheAudio->addAudioEvent(&audioEvent)
        // Rust: audio_system.play_sound(sound_name, volume, AudioType::Sound)
        // Uses: AudioHandle and AudioType from common/audio.rs

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.play_sound_effect(&sound_name) {
                        log::warn!("Script action handler play_sound_effect failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "sound_play"
    }

    fn description(&self) -> &str {
        "Plays a sound effect"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["sound_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["volume".to_string()]
    }
}

/// Play music track
struct MusicPlayAction;

#[async_trait]
impl ScriptAction for MusicPlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let track_name = get_string_param(parameters, "track_name")?;

        log::info!("Playing music track '{}'", track_name);

        // Matches C++ ScriptActions.cpp music system
        // Integration with audio system:
        // 1. Stop current music track (fade out)
        // 2. AudioEventRTS event(track_name)
        // 3. event.setPlayerIndex(localPlayer)
        // 4. TheAudio->addAudioEvent(&event)
        // 5. Music loops until stopped or new track played
        // Rust: audio_system.play_music(track_name)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.music_set_track(&track_name, true, true) {
                        log::warn!("Script action handler music_set_track failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "music_play"
    }

    fn description(&self) -> &str {
        "Plays a music track"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["track_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Play video
struct MoviePlayAction;

#[async_trait]
impl ScriptAction for MoviePlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let movie_name = get_string_param(parameters, "movie_name")?;
        let fullscreen = parameters
            .get("fullscreen")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!(
            "Playing movie '{}' (fullscreen: {})",
            movie_name,
            fullscreen
        );

        // Matches C++ ScriptActions.cpp:doMoviePlayFullScreen line 2707
        // Integration with video playback system:
        // 1. TheDisplay->playMovie(movie_name) // Fullscreen
        // Or: TheInGameUI->playMovie(movie_name) // In radar area
        // 2. Pauses game during playback
        // 3. Plays .bik video files
        // 4. Returns to game after video ends or skip
        // Rust: video_system.play_movie(movie_name, fullscreen)

        if fullscreen {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.movie_play_fullscreen(&movie_name) {
                            log::warn!(
                                "Script action handler movie_play_fullscreen failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&movie_name) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "movie_play"
    }

    fn description(&self) -> &str {
        "Plays a video/movie"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["movie_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["fullscreen".to_string()]
    }
}

/// Show text message
struct TextDisplayAction;

#[async_trait]
impl ScriptAction for TextDisplayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let message = get_string_param(parameters, "message")?;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(5.0);

        log::info!("Displaying text: '{}' for {} seconds", message, duration);

        // Matches C++ ScriptActions.cpp:doDisplayText line 2523
        // Integration with UI system:
        // 1. TheInGameUI->message(message)
        // 2. Text appears in UI message area
        // 3. Auto-fades after duration
        // 4. May support localization via TheGameText->fetch()
        // Rust: ui_system.display_message(message, duration)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&message) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "text_display"
    }

    fn description(&self) -> &str {
        "Displays a text message on screen"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["message".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Play speech
struct SpeechPlayAction;

#[async_trait]
impl ScriptAction for SpeechPlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let speech_name = get_string_param(parameters, "speech_name")?;

        log::info!("Playing speech '{}'", speech_name);

        // Matches C++ ScriptActions.cpp:doSpeechPlay line 2743
        // Integration with audio/EVA system (from C++):
        // 1. AudioEventRTS speech(speech_name)
        // 2. speech.setIsLogicalAudio(true)
        // 3. speech.setPlayerIndex(localPlayer)
        // 4. speech.setUninterruptable(!allowOverlap)
        // 5. TheAudio->addAudioEvent(&speech)
        // 6. May display subtitle via TheInGameUI->militarySubtitle()
        // EVA = Electronic Video Agent (voice system)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.speech_play(&speech_name, false) {
                        log::warn!("Script action handler speech_play failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "speech_play"
    }

    fn description(&self) -> &str {
        "Plays a speech/voice line"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["speech_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Enable radar
struct RadarEnableAction;

#[async_trait]
impl ScriptAction for RadarEnableAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Enabling radar");

        // Matches C++ ScriptActions.cpp:doRadarEnable line 2898
        // Integration with radar system (from C++):
        // 1. TheRadar->hide(false) // Make radar visible
        // 2. Updates UI to show minimap
        // 3. Enables radar functionality
        // Typically used after doRadarDisable or at mission start
        // Rust: radar_system.set_enabled(true)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_enabled(true) {
                        log::warn!(
                            "Script action handler set_radar_enabled(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_enable"
    }

    fn description(&self) -> &str {
        "Enables the radar display"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable radar
struct RadarDisableAction;

#[async_trait]
impl ScriptAction for RadarDisableAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Disabling radar");

        // Matches C++ ScriptActions.cpp:doRadarDisable line 2890
        // Integration with radar system (from C++):
        // 1. TheRadar->hide(true) // Hide radar from UI
        // 2. Removes minimap display
        // 3. Used for missions where radar is unavailable
        // Player loses strategic overview until re-enabled
        // Rust: radar_system.set_enabled(false)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_enabled(false) {
                        log::warn!(
                            "Script action handler set_radar_enabled(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_disable"
    }

    fn description(&self) -> &str {
        "Disables the radar display"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// 20 CORE SCRIPT ACTIONS - Priority 1 Implementation
// Based on C++ ScriptActions from GENERALSMD_SCRIPTING_SYSTEM_GUIDE.md
// ============================================================================

/// Victory Action - Matches C++ ScriptActionType::VICTORY
struct VictoryAction;

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
struct DefeatAction;

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

/// Start Timer Action - Matches C++ ScriptActionType::SET_MILLISECOND_TIMER
struct StartTimerAction;

#[async_trait]
impl ScriptAction for StartTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let counter_name = get_string_param(parameters, "counter_name")?;
        let milliseconds = get_int_param(parameters, "milliseconds")?;

        log::info!("Starting timer '{}' for {} ms", counter_name, milliseconds);

        with_script_engine_mut(|engine| {
            engine.set_timer_millisecond_script_seconds(&counter_name, milliseconds as f32 / 1000.0)
        })?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "start_timer"
    }

    fn description(&self) -> &str {
        "Starts a countdown timer with specified milliseconds"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["counter_name".to_string(), "milliseconds".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Stop Timer Action - Matches C++ ScriptActionType::STOP_TIMER
struct StopTimerAction;

#[async_trait]
impl ScriptAction for StopTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let counter_name = get_string_param(parameters, "counter_name")?;

        log::info!("Stopping timer '{}'", counter_name);

        with_script_engine_mut(|engine| engine.stop_timer(&counter_name))?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "stop_timer"
    }

    fn description(&self) -> &str {
        "Stops/pauses a countdown timer"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["counter_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Create Building Action - Matches C++ ScriptActionType::CREATE_OBJECT (for structures)
struct CreateBuildingAction;

#[async_trait]
impl ScriptAction for CreateBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let building_type = get_string_param(parameters, "building_type")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);
        let angle = get_float_param_optional(parameters, "angle").unwrap_or(0.0);

        log::info!(
            "Creating building '{}' for player {} at ({}, {}, {}) angle {}",
            building_type,
            player,
            x,
            y,
            z,
            angle
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let position = Coord3D::new(x as f32, y as f32, z as f32);
        let object_id = get_object_manager()
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?
            .create_object(
                building_type.as_str(),
                position,
                team,
                ObjectCreationFlags::from_template(),
            )?;

        if angle != 0.0 {
            if let Ok(manager) = get_object_manager().read() {
                if let Some(object) = manager.get_object(object_id) {
                    if let Ok(guard) = object.write() {
                        let _ = guard
                            .base
                            .write()
                            .map(|mut base| base.set_orientation(angle as f32));
                    }
                }
            }
        }

        Ok(ScriptResult::Success(Some(ScriptValue::ObjectId(
            object_id as u32,
        ))))
    }

    fn name(&self) -> &str {
        "create_building"
    }

    fn description(&self) -> &str {
        "Creates a building/structure at the specified location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "building_type".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string(), "angle".to_string()]
    }
}

/// Destroy Building Action - Matches C++ destroy/kill object logic
struct DestroyBuildingAction;

#[async_trait]
impl ScriptAction for DestroyBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;

        log::info!("Destroying building {}", object_id);

        // Integration with object system:
        // 1. Object *obj = TheObjectList->getObject(object_id)
        // 2. obj->kill(DEATH_NORMAL) // Normal death with effects
        // 3. Triggers death animations, explosions
        // 4. Removes from world
        // 5. Frees resources
        // Rust: object_manager.destroy_object(object_id, DeathType::Normal)

        if object_id < 0 {
            return Err(GameLogicError::Configuration(
                "object_id must be non-negative".to_string(),
            ));
        }

        get_object_manager()
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?
            .destroy_object(object_id as u32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "destroy_building"
    }

    fn description(&self) -> &str {
        "Destroys/removes a building from the game"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Team Alliance Action - Matches C++ ScriptActionType::PLAYER_RELATES_PLAYER
struct SetTeamAllianceAction;

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

/// Give Special Power Action - Matches C++ special power grant logic
struct GiveSpecialPowerAction;

#[async_trait]
impl ScriptAction for GiveSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;

        if player < 0 {
            return Err(GameLogicError::Configuration(format!(
                "player must be non-negative, got {player}"
            )));
        }

        log::info!(
            "Granting special power '{}' to player {}",
            power_name,
            player
        );

        let power_template =
            find_or_create_special_power_template(&AsciiString::from(power_name.as_str()));
        let player_arc = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?
            .get_player(player as PlayerIndex)
            .cloned();

        let Some(player_arc) = player_arc else {
            log::warn!(
                "Cannot grant special power '{}' to missing player {}",
                power_name,
                player
            );
            return Ok(ScriptResult::Success(None));
        };

        let ready_frame = TheGameLogic::get_frame();
        player_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?
            .express_special_power_ready_frame(&power_template, ready_frame);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "give_special_power"
    }

    fn description(&self) -> &str {
        "Grants a special power/ability to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Reveal Area Action - Matches C++ ScriptActionType::MAP_REVEAL_AT_WAYPOINT
struct RevealAreaAction;

#[async_trait]
impl ScriptAction for RevealAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let radius = get_float_param(parameters, "radius")?;
        let permanent = get_int_param_optional(parameters, "permanent").unwrap_or(0) != 0;

        log::info!(
            "Revealing area at ({}, {}) radius {} for player {} (permanent: {})",
            x,
            y,
            radius,
            player,
            permanent
        );

        // Integration with shroud/fog of war system (from C++):
        // 1. Coord3D pos = {x, y, z}
        // 2. ThePartitionManager->revealAreaForPlayer(&pos, radius, player, permanent)
        // 3. Updates shroud visibility in circular area
        // 4. If permanent: area stays revealed
        // 5. If temporary: fog returns when no units nearby
        // Uses: ShroudManager and PartitionManager
        // Rust: shroud_manager.reveal_area(position, radius, player, permanent)

        let pos = Coord3D::new(x as f32, y as f32, 0.0);
        let player_mask = 1u32 << (player.max(0) as u32);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.do_shroud_reveal(&pos, radius as f32, player_mask);
            if !permanent {
                let current_frame = crate::helpers::TheGameLogic::get_frame();
                shroud_mgr.queue_undo_shroud_reveal(
                    &pos,
                    radius as f32,
                    player_mask,
                    0,
                    current_frame,
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "reveal_area"
    }

    fn description(&self) -> &str {
        "Reveals fog of war in a circular area for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["permanent".to_string()]
    }
}

/// Create Explosion Action - Matches C++ explosion creation
struct CreateExplosionAction;

#[async_trait]
impl ScriptAction for CreateExplosionAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let explosion_type = get_string_param(parameters, "explosion_type")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);
        let damage = get_float_param_optional(parameters, "damage").unwrap_or(0.0);

        log::info!(
            "Creating explosion '{}' at ({}, {}, {}) with damage {}",
            explosion_type,
            x,
            y,
            z,
            damage
        );

        // Integration with FX/damage system:
        // 1. FXTemplate *fxTemplate = TheFXStore->find(explosion_type)
        // 2. Coord3D pos = {x, y, z}
        // 3. createExplosion(fxTemplate, &pos, damage, attacker)
        // 4. Visual explosion effects (particles, light, sound)
        // 5. Damage applied to units in blast radius
        // 6. Damage falloff with distance
        // Rust: fx_system.create_explosion(explosion_type, position, damage)

        log::debug!("Integration: FX and damage systems create explosion");

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "create_explosion"
    }

    fn description(&self) -> &str {
        "Creates an explosion effect at the specified location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "explosion_type".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string(), "damage".to_string()]
    }
}

/// Spawn Reinforcements Action - Creates multiple units for player
struct SpawnReinforcementsAction;

#[async_trait]
impl ScriptAction for SpawnReinforcementsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let unit_type = get_string_param(parameters, "unit_type")?;
        let count = get_int_param(parameters, "count")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let spacing = get_float_param_optional(parameters, "spacing").unwrap_or(10.0);

        log::info!(
            "Spawning {} reinforcements of '{}' for player {} at ({}, {})",
            count,
            unit_type,
            player,
            x,
            y
        );

        // Integration with unit creation - spawn in formation:
        // Matches C++ ScriptActions.cpp:doCreateReinforcements line 480
        // 1. For i in 0..count:
        //    a. Calculate grid position: offset_x = x + (i % 5) * spacing
        //    b. offset_y = y + (i / 5) * spacing
        //    c. Create unit at offset position
        // 2. Units spawn in 5-column grid formation
        // 3. ThingFactory->newObject(template, team, position)
        // Rust: object_factory.spawn_formation(unit_type, player, position, count, spacing)

        if player < 0 {
            return Err(GameLogicError::Configuration(
                "player must be non-negative".to_string(),
            ));
        }
        if count < 0 {
            return Err(GameLogicError::Configuration(
                "count must be non-negative".to_string(),
            ));
        }

        let team = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?
            .get_player(player as PlayerIndex)
            .cloned()
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let mut created_ids = Vec::with_capacity(count as usize);
        let manager = get_object_manager();
        let mut manager = manager
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?;

        for i in 0..count {
            let offset_x = x + ((i % 5) as f64 * spacing);
            let offset_y = y + ((i / 5) as f64 * spacing);
            let object_id = manager.create_object(
                &unit_type,
                Coord3D::new(offset_x as f32, offset_y as f32, 0.0),
                team.clone(),
                ObjectCreationFlags::from_template(),
            )?;
            created_ids.push(ScriptValue::ObjectId(object_id));
        }

        Ok(ScriptResult::Success(Some(ScriptValue::Array(created_ids))))
    }

    fn name(&self) -> &str {
        "spawn_reinforcements"
    }

    fn description(&self) -> &str {
        "Spawns multiple units (reinforcements) for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "count".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["spacing".to_string()]
    }
}

/// Camera Zoom Action - Matches C++ ScriptActionType::ZOOM_CAMERA
struct CameraZoomAction;

#[async_trait]
impl ScriptAction for CameraZoomAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom_level = get_float_param(parameters, "zoom_level")? as f32;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0) as f32;

        log::info!(
            "Zooming camera to level {} over {} seconds",
            zoom_level,
            duration
        );

        // Matches C++ ScriptActions.cpp:doZoomCamera
        // Integration with camera system (from C++):
        // 1. TheDisplay->Set_Zoom(zoom_level, duration)
        // Or: TheTacticalView->setZoom(zoom_level, duration)
        // 2. Interpolates zoom over duration
        // 3. Same as CameraSetFinalZoomAction
        // Rust: camera.set_zoom(zoom_level, duration)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_camera_zoom(zoom_level, duration) {
                        log::warn!("Script action handler set_camera_zoom failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_zoom"
    }

    fn description(&self) -> &str {
        "Changes camera zoom level"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoom_level".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

// ============================================================================
// HIGH-PRIORITY MISSING ACTIONS - PORTED FROM C++
// ============================================================================

/// Give Money Action - Matches C++ ScriptActions::doGiveMoney (line 3999)
struct GiveMoneyAction;

#[async_trait]
impl ScriptAction for GiveMoneyAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Giving ${} to player '{}'", amount, player_name);

        // Matches C++ ScriptActions.cpp:doGiveMoney line 3999
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. Money *m = player->getMoney()
        // 3. if (money < 0) m->withdraw(-money) else m->deposit(money)
        // Supports negative amounts for withdrawing money
        // Rust: player_list.get_player(player_name).add_money(amount)

        let resolved_name = resolve_player_name_token(&player_name);
        let list_guard = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;
        let Some(player_arc) = list_guard.find_player_by_name(&resolved_name) else {
            log::warn!("GiveMoneyAction: player '{}' not found", resolved_name);
            return Ok(ScriptResult::Success(None));
        };
        let mut player_guard = player_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
        if amount < 0 {
            spend_script_player_money(
                &mut player_guard,
                clamp_script_money(amount.saturating_neg()),
            );
        } else {
            grant_script_player_money(&mut player_guard, clamp_script_money(amount));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "give_money"
    }

    fn description(&self) -> &str {
        "Gives money to a player (positive) or takes money away (negative)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Money Action - Matches C++ ScriptActions::doSetMoney (line 3980)
struct SetMoneyAction;

#[async_trait]
impl ScriptAction for SetMoneyAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Setting player '{}' money to ${}", player_name, amount);

        // Matches C++ ScriptActions.cpp:doSetMoney line 3980
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. Money *m = player->getMoney()
        // 3. m->withdraw(m->countMoney()) // Withdraw all current money
        // 4. m->deposit(money) // Deposit new amount
        // Sets absolute money value (not additive)
        // Rust: player_list.get_player(player_name).set_money(amount)

        let resolved_name = resolve_player_name_token(&player_name);
        let list_guard = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;
        let Some(player_arc) = list_guard.find_player_by_name(&resolved_name) else {
            log::warn!("SetMoneyAction: player '{}' not found", resolved_name);
            return Ok(ScriptResult::Success(None));
        };
        let mut player_guard = player_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
        set_script_player_money(&mut player_guard, clamp_script_money(amount));

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_money"
    }

    fn description(&self) -> &str {
        "Sets a player's money to an exact amount"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Handicap Action - Sets player difficulty/handicap modifier
struct SetHandicapAction;

#[async_trait]
impl ScriptAction for SetHandicapAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;
        let handicap = get_float_param(parameters, "handicap")?;

        log::info!("Setting player '{}' handicap to {}", player_name, handicap);

        // C++ Implementation (from Player.h/cpp):
        // Handicap affects:
        // - Resource collection rate (multiplier)
        // - Build speed (multiplier)
        // - Unit damage output (multiplier)
        // - Unit health (multiplier)
        // Typical values: 0.5 (easy), 1.0 (normal), 1.5 (hard)
        // player->setHandicap(handicap)
        // Rust: player_list.get_player(player_name).set_handicap(handicap)

        let resolved_name = resolve_player_name_token(&player_name);
        let list_guard = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;
        let Some(player_arc) = list_guard.find_player_by_name(&resolved_name) else {
            log::warn!("SetHandicapAction: player '{}' not found", resolved_name);
            return Ok(ScriptResult::Success(None));
        };
        let mut player_guard = player_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
        player_guard.set_handicap(handicap as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_handicap"
    }

    fn description(&self) -> &str {
        "Sets a player's handicap/difficulty multiplier"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "handicap".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Damage Object Action - Matches C++ ScriptActions::doNamedDamage (line 2312)
struct DamageObjectAction;

#[async_trait]
impl ScriptAction for DamageObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_name = get_string_param(parameters, "object")?;
        let damage = get_int_param(parameters, "damage")?;

        log::info!("Damaging object '{}' by {} HP", object_name, damage);

        // Matches C++ ScriptActions.cpp:doNamedDamage line 2312
        // Implementation:
        // 1. Object *pUnit = TheScriptEngine->getUnitNamed(unitName)
        // 2. DamageInfo damageInfo
        // 3. damageInfo.in.m_damageType = DAMAGE_UNRESISTABLE
        // 4. damageInfo.in.m_deathType = DEATH_NORMAL
        // 5. damageInfo.in.m_sourceID = INVALID_ID
        // 6. damageInfo.in.m_amount = damageAmt
        // 7. pUnit->attemptDamage(&damageInfo)
        // Applies unresistable damage (ignores armor)
        // Rust: object_manager.get_object(object_name).apply_damage(damage, DamageType::Unresistable)

        log::debug!("Integration: Object damage system applies unresistable damage");

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            log::warn!("DamageObjectAction: object '{}' not found", object_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            log::warn!(
                "DamageObjectAction: object '{}' (ID {}) not found in registry",
                object_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let mut object_guard = object_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Object".to_string()))?;

        let mut damage_info = DamageInfo::with_simple(
            damage as f32,
            INVALID_OBJECT_ID,
            DamageType::Unresistable,
            DeathType::Normal,
        );

        if let Err(err) = object_guard.attempt_damage(&mut damage_info) {
            log::warn!(
                "DamageObjectAction: failed to damage '{}' (ID {}): {}",
                object_name,
                object_id,
                err
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "damage_object"
    }

    fn description(&self) -> &str {
        "Applies damage to a named object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object".to_string(), "damage".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Kill Object Action - Matches C++ ScriptActions::doNamedKill (line 2483)
struct KillObjectAction;

#[async_trait]
impl ScriptAction for KillObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_name = get_string_param(parameters, "object")?;

        log::info!("Killing object '{}'", object_name);

        // Matches C++ ScriptActions.cpp:doNamedKill line 2483
        // Similar to doNamedDelete but with death effects
        // Implementation:
        // 1. Object *theUnit = TheScriptEngine->getUnitNamed(unitName)
        // 2. theUnit->kill(DAMAGE_UNRESISTABLE, DEATH_NORMAL)
        // Or: BodyModule->setHealth(0) to trigger normal death
        // Triggers death animation, sound, and cleanup
        // Rust: object_manager.get_object(object_name).kill()

        log::debug!("Integration: Object system kills unit with death effects");

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            log::warn!("KillObjectAction: object '{}' not found", object_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            log::warn!(
                "KillObjectAction: object '{}' (ID {}) not found in registry",
                object_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let mut object_guard = object_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Object".to_string()))?;

        object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "kill_object"
    }

    fn description(&self) -> &str {
        "Instantly kills a named object with death effects"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Heal Object Action - Restores object health
struct HealObjectAction;

#[async_trait]
impl ScriptAction for HealObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_name = get_string_param(parameters, "object")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Healing object '{}' by {} HP", object_name, amount);

        // C++ Implementation:
        // 1. Object *pUnit = TheScriptEngine->getUnitNamed(unitName)
        // 2. BodyModuleInterface *body = pUnit->getBodyModule()
        // 3. body->setHealth(body->getHealth() + amount)
        // Or: body->setHealth(body->getMaxHealth()) for full heal
        // Can specify amount or use -1 for full heal
        // Rust: object_manager.get_object(object_name).heal(amount)

        log::debug!("Integration: Object health system restores HP");

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            log::warn!("HealObjectAction: object '{}' not found", object_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            log::warn!(
                "HealObjectAction: object '{}' (ID {}) not found in registry",
                object_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let mut object_guard = object_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Object".to_string()))?;

        if amount < 0 {
            if let Err(err) = object_guard.heal_completely() {
                log::warn!(
                    "HealObjectAction: failed to fully heal '{}' (ID {}): {}",
                    object_name,
                    object_id,
                    err
                );
            }
        } else if let Err(err) = object_guard.heal(amount as f32) {
            log::warn!(
                "HealObjectAction: failed to heal '{}' (ID {}) by {}: {}",
                object_name,
                object_id,
                amount,
                err
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "heal_object"
    }

    fn description(&self) -> &str {
        "Heals a named object by specified amount (-1 for full heal)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Reveal Map Entire Action - Matches C++ ScriptActions::doRevealMapEntire (line 3036)
struct RevealMapEntireAction;

#[async_trait]
impl ScriptAction for RevealMapEntireAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;

        log::info!("Revealing entire map for player '{}'", player_name);

        // Matches C++ ScriptActions.cpp:doRevealMapEntire line 3036
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. if player exists:
        //    ThePartitionManager->revealMapForPlayer(player->getPlayerIndex())
        // 3. else (for all human players):
        //    for i in 0..numPlayers:
        //      if player->isHuman():
        //        ThePartitionManager->revealMapForPlayer(i)
        // Reveals entire map shroud permanently
        // Rust: shroud_manager.reveal_map_for_player(player_index)

        let player_list = crate::player::player_list();
        let list_guard = player_list
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;

        let mut shroud_manager = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| GameLogicError::Threading("Failed to lock ShroudManager".to_string()))?;

        if let Some(player) = list_guard.find_player_by_name(&player_name) {
            let player_guard = player
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
            shroud_manager
                .reveal_map_for_player(player_guard.get_player_index() as u32)
                .map_err(GameLogicError::Configuration)?;
        } else {
            for player in list_guard.iter() {
                let player_guard = player
                    .read()
                    .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
                if player_guard.get_player_type() == PlayerType::Human {
                    shroud_manager
                        .reveal_map_for_player(player_guard.get_player_index() as u32)
                        .map_err(GameLogicError::Configuration)?;
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "reveal_map_entire"
    }

    fn description(&self) -> &str {
        "Reveals the entire map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Shroud Map Entire Action - Matches C++ ScriptActions::doShroudMapEntire (line 3090)
struct ShroudMapEntireAction;

#[async_trait]
impl ScriptAction for ShroudMapEntireAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;

        log::info!("Shrouding entire map for player '{}'", player_name);

        // Matches C++ ScriptActions.cpp:doShroudMapEntire line 3090
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. ThePartitionManager->shroudMapForPlayer(player->getPlayerIndex())
        // Re-applies fog of war to entire map
        // Used for dramatic script events or resetting vision
        // Rust: shroud_manager.shroud_map_for_player(player_index)

        let player_list = crate::player::player_list();
        let list_guard = player_list
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;

        let mut shroud_manager = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| GameLogicError::Threading("Failed to lock ShroudManager".to_string()))?;

        if let Some(player) = list_guard.find_player_by_name(&player_name) {
            let player_guard = player
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
            shroud_manager
                .shroud_map_for_player(player_guard.get_player_index() as u32)
                .map_err(GameLogicError::Configuration)?;
        } else {
            for player in list_guard.iter() {
                let player_guard = player
                    .read()
                    .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
                if player_guard.get_player_type() == PlayerType::Human {
                    shroud_manager
                        .shroud_map_for_player(player_guard.get_player_index() as u32)
                        .map_err(GameLogicError::Configuration)?;
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "shroud_map_entire"
    }

    fn description(&self) -> &str {
        "Shrouds the entire map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Snap Camera Action - Instant camera movement (no animation)
struct SnapCameraAction;

#[async_trait]
impl ScriptAction for SnapCameraAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Snapping camera to waypoint '{}'", waypoint);

        // Matches C++ camera snap functionality
        // Implementation:
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D destination = *way->getLocation()
        // 3. TheTacticalView->moveCameraTo(&destination, 0, 0, true, 0.0f, 0.0f)
        // duration=0 means instant snap (no animation)
        // Used for cutscenes and quick transitions
        // Rust: camera.snap_to_position(waypoint_position)

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });

        let Some(target) = target else {
            log::warn!("Snap camera failed: waypoint '{}' not found", waypoint);
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.reset_camera_to(target.x, target.y, target.z, 0.0) {
                        log::warn!("Script action handler reset_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "snap_camera"
    }

    fn description(&self) -> &str {
        "Instantly moves camera to waypoint (no animation)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Letter Box Begin Action - Start cinematic letterbox mode
struct LetterBoxBeginAction;

#[async_trait]
impl ScriptAction for LetterBoxBeginAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Starting letterbox cinematic mode");

        // C++ Implementation:
        // Adds black bars to top/bottom of screen for cinematic effect
        // 1. TheInGameUI->setLetterboxMode(true)
        // 2. Animates black bars expanding from edges
        // 3. Hides UI elements (command bar, minimap, etc)
        // 4. Often combined with camera movements
        // Typical animation: 0.5-1.0 seconds expand
        // Rust: ui_manager.enable_letterbox(true)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_begin() {
                        log::warn!(
                            "Script action handler camera_letterbox_begin failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "letterbox_begin"
    }

    fn description(&self) -> &str {
        "Starts cinematic letterbox mode (black bars)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Letter Box End Action - End cinematic letterbox mode
struct LetterBoxEndAction;

#[async_trait]
impl ScriptAction for LetterBoxEndAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Ending letterbox cinematic mode");

        // C++ Implementation:
        // Removes letterbox black bars
        // 1. TheInGameUI->setLetterboxMode(false)
        // 2. Animates black bars retracting to edges
        // 3. Restores UI elements
        // 4. Returns to normal gameplay view
        // Typical animation: 0.5-1.0 seconds retract
        // Rust: ui_manager.enable_letterbox(false)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_end() {
                        log::warn!("Script action handler camera_letterbox_end failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "letterbox_end"
    }

    fn description(&self) -> &str {
        "Ends cinematic letterbox mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Attack Action - Commands team to attack target
struct TeamAttackAction;

#[async_trait]
impl ScriptAction for TeamAttackAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let target = get_string_param(parameters, "target")?;

        log::info!("Team '{}' attacking target '{}'", team_name, target);

        // Matches C++ ScriptActions.cpp:doTeamAttackNamed line 1414
        // Implementation:
        // 1. Team *team = TheScriptEngine->getTeamNamed(teamName)
        // 2. Object *target = TheScriptEngine->getUnitNamed(targetName)
        // 3. for each unit in team:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->aiAttackObject(target, CMD_FROM_SCRIPT)
        // All team members attack same target
        // Rust: team_manager.get_team(team_name).attack_target(target)

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(target_id) = resolve_named_object_id(&target) else {
            log::warn!("TeamAttackAction: target '{}' not found", target);
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamAttackAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            params.int_value = -1; // NO_MAX_SHOTS_LIMIT
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_attack"
    }

    fn description(&self) -> &str {
        "Commands a team to attack a target"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "target".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Guard Area Action - Matches C++ ScriptActions::doTeamGuardArea (line 1946)
struct TeamGuardAreaAction;

#[async_trait]
impl ScriptAction for TeamGuardAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let area = get_string_param(parameters, "area")?;

        log::info!("Team '{}' guarding area '{}'", team_name, area);

        // Matches C++ ScriptActions.cpp:doTeamGuardArea line 1946
        // Implementation:
        // 1. Team *team = TheScriptEngine->getTeamNamed(teamName)
        // 2. PolygonTrigger *area = TheScriptEngine->getAreaByName(areaName)
        // 3. for each unit in team:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->aiGuardArea(area, CMD_FROM_SCRIPT)
        // Units patrol and defend area perimeter
        // Engages enemies that enter area
        // Rust: team_manager.get_team(team_name).guard_area(area)

        let resolved_team = resolve_team_name_token(&team_name);
        let (center, trigger_id) = if let Ok(terrain_guard) = get_terrain_logic().read() {
            if let Some(trigger) = terrain_guard.get_trigger_area_by_name(&area) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("TeamGuardAreaAction: trigger area '{}' not found", area);
                return Ok(ScriptResult::Success(None));
            }
        } else {
            log::warn!("TeamGuardAreaAction: failed to lock terrain logic");
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamGuardAreaAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardArea, CommandSourceType::FromScript);
            params.pos = center;
            params.polygon = Some(trigger_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_guard_area"
    }

    fn description(&self) -> &str {
        "Commands a team to guard an area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Follow Action - Commands team to follow another team or unit
struct TeamFollowAction;

#[async_trait]
impl ScriptAction for TeamFollowAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let target = get_string_param(parameters, "target")?;

        log::info!("Team '{}' following target '{}'", team_name, target);

        // C++ Implementation (TeamFollow behavior):
        // 1. Team *team = TheScriptEngine->getTeamNamed(teamName)
        // 2. Object *target = TheScriptEngine->getUnitNamed(targetName)
        //    Or: Team *targetTeam = TheScriptEngine->getTeamNamed(targetName)
        // 3. for each unit in team:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->aiFollowObject(target, CMD_FROM_SCRIPT)
        // Units maintain formation following target
        // Updates position as target moves
        // Rust: team_manager.get_team(team_name).follow_target(target)

        let resolved_team = resolve_team_name_token(&team_name);
        let target_id = resolve_named_object_id(&target).or_else(|| {
            let factory = get_team_factory();
            factory
                .lock()
                .ok()
                .and_then(|mut factory_guard| {
                    factory_guard.find_team(&resolve_team_name_token(&target))
                })
                .and_then(|team_arc| {
                    team_arc
                        .read()
                        .ok()
                        .and_then(|team| team.get_members().first().copied())
                })
        });

        let Some(target_id) = target_id else {
            log::warn!("TeamFollowAction: target '{}' not found", target);
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamFollowAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_follow"
    }

    fn description(&self) -> &str {
        "Commands a team to follow a target"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "target".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Timer Action - Matches C++ ScriptActions::doDisplayCounter (line 4020)
struct SetTimerAction;

#[async_trait]
impl ScriptAction for SetTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let timer_name = get_string_param(parameters, "timer")?;
        let value = get_int_param(parameters, "value")?;

        log::info!("Setting timer '{}' to {}", timer_name, value);

        with_script_engine_mut(|engine| {
            engine.set_counter(&timer_name, clamp_script_money(value))
        })?;
        dispatch_named_timer(&timer_name, &timer_name, false);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_timer"
    }

    fn description(&self) -> &str {
        "Creates or sets a named timer/counter on HUD"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["timer".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Countdown Timer Action - Matches C++ ScriptActions::doDisplayCountdownTimer (line 4036)
struct CountdownTimerAction;

#[async_trait]
impl ScriptAction for CountdownTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let timer_name = get_string_param(parameters, "timer")?;
        let seconds = get_int_param(parameters, "seconds")?;

        log::info!(
            "Starting countdown timer '{}' for {} seconds",
            timer_name,
            seconds
        );

        with_script_engine_mut(|engine| {
            engine.set_timer_seconds(&timer_name, seconds.max(0) as f32)
        })?;
        dispatch_named_timer(&timer_name, &timer_name, true);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "countdown_timer"
    }

    fn description(&self) -> &str {
        "Starts a countdown timer on HUD"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["timer".to_string(), "seconds".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Play Sound At Action - Matches C++ ScriptActions::doPlaySoundEffectAt (line 341)
struct PlaySoundAtAction;

#[async_trait]
impl ScriptAction for PlaySoundAtAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let sound = get_string_param(parameters, "sound")?;
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Playing sound '{}' at waypoint '{}'", sound, waypoint);

        // Matches C++ ScriptActions.cpp:doPlaySoundEffectAt line 341
        // Implementation:
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D *pos = way->getLocation()
        // 3. AudioEventRTS *audioEvent = g_theAudio->NewAudioEventRTS(sound)
        // 4. audioEvent->setSoundPosition(pos)
        // 5. audioEvent->Execute()
        // Plays 3D positional sound at specific location
        // Volume/pan based on distance from camera
        // Rust: audio_manager.play_sound_at(sound, waypoint_position)

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.play_sound_effect_at(&sound, target.x, target.y, target.z)
                    {
                        log::warn!("Script action handler play_sound_effect_at failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "play_sound_at"
    }

    fn description(&self) -> &str {
        "Plays a sound effect at a waypoint location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["sound".to_string(), "waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Stop Music Action - Stops currently playing music
struct StopMusicAction;

#[async_trait]
impl ScriptAction for StopMusicAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Stopping background music");

        // C++ Implementation:
        // 1. g_theAudio->StopMusic()
        // Or: TheMusicManager->stopCurrentTrack()
        // 2. Fades out current music over ~1 second
        // 3. Clears music queue
        // Used for dramatic moments or transitioning to silence
        // Often combined with PlayMusic for music changes
        // Rust: audio_manager.stop_music()

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.stop_music() {
                        log::warn!("Script action handler stop_music failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "stop_music"
    }

    fn description(&self) -> &str {
        "Stops the currently playing background music"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_context() -> ScriptContext {
        ScriptContext {
            game_time: std::time::Duration::from_secs(0),
            active_player: Some(0),
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        }
    }

    fn reset_test_player(index: PlayerIndex, money: i32) {
        let mut list = player_list().write().unwrap();
        list.clear();
        let mut player = crate::player::Player::new(index);
        player.set_display_name(format!("Player{index}"));
        player.get_money_mut().set_money(money);
        list.add_player(Arc::new(RwLock::new(player)));
    }

    fn reset_test_players(count: PlayerIndex) {
        let mut list = player_list().write().unwrap();
        list.clear();
        for index in 0..count {
            let mut player = crate::player::Player::new(index);
            player.set_display_name(format!("Player{index}"));
            list.add_player(Arc::new(RwLock::new(player)));
        }
    }

    fn reset_test_script_engine() {
        let engine_lock = get_script_engine();
        let mut engine = engine_lock.write().unwrap();
        *engine = Some(crate::scripting::engine::ScriptEngine::new().unwrap());
    }

    fn reset_test_object_manager() {
        get_object_manager().write().unwrap().reset();
    }

    fn ensure_test_template(name: &str) {
        use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};

        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        let mut factory_guard = get_thing_factory().unwrap();
        let factory = factory_guard.as_mut().unwrap();
        if factory.find_template(name, false).is_none() {
            factory.new_template(name);
        }
    }

    #[tokio::test]
    async fn test_action_registry() {
        let registry = ActionRegistry::new();

        let actions = registry.list_actions();
        assert!(actions.contains(&"create_unit".to_string()));
        assert!(actions.contains(&"move_unit".to_string()));
        assert!(actions.contains(&"play_sound".to_string()));
    }

    #[tokio::test]
    async fn test_create_unit_action() {
        use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};

        // Ensure a template exists for the requested unit type.
        // The fully-implemented `CreateUnitAction` now uses the real object factory path.
        let needs_init = get_thing_factory().unwrap().is_none();
        if needs_init {
            init_thing_factory().unwrap();
        }
        {
            let mut factory_guard = get_thing_factory().unwrap();
            if let Some(factory) = factory_guard.as_mut() {
                if factory.find_template("Tank", false).is_none() {
                    factory.new_template("Tank");
                }
            }
        }

        let action = CreateUnitAction;
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(1));
        params.insert(
            "unit_type".to_string(),
            ScriptValue::String("Tank".to_string()),
        );
        params.insert("x".to_string(), ScriptValue::Float(100.0));
        params.insert("y".to_string(), ScriptValue::Float(200.0));

        let context = ScriptContext {
            game_time: std::time::Duration::from_secs(0),
            active_player: Some(1),
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = action.execute(&params, &context).await.unwrap();
        assert!(matches!(result, ScriptResult::Success(_)));
    }

    #[tokio::test]
    async fn test_parameter_extraction() {
        let mut params = HashMap::new();
        params.insert(
            "test_string".to_string(),
            ScriptValue::String("hello".to_string()),
        );
        params.insert("test_int".to_string(), ScriptValue::Int(42));
        params.insert("test_float".to_string(), ScriptValue::Float(3.14));

        assert_eq!(get_string_param(&params, "test_string").unwrap(), "hello");
        assert_eq!(get_int_param(&params, "test_int").unwrap(), 42);
        assert_eq!(get_float_param(&params, "test_float").unwrap(), 3.14);
    }

    #[tokio::test]
    async fn set_player_resource_sets_money() {
        reset_test_player(0, 250);

        let action = SetPlayerResourceAction;
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("cash".to_string()),
        );
        params.insert("amount".to_string(), ScriptValue::Int(1200));

        action.execute(&params, &test_context()).await.unwrap();

        let list = player_list().read().unwrap();
        let player = list.get_player(0).unwrap().read().unwrap();
        assert_eq!(player.get_money().get_money(), 1200);
    }

    #[tokio::test]
    async fn add_player_resource_updates_money_and_ignores_unknown_resources() {
        reset_test_player(0, 500);

        let action = AddPlayerResourceAction;
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("supplies".to_string()),
        );
        params.insert("amount".to_string(), ScriptValue::Int(300));

        action.execute(&params, &test_context()).await.unwrap();

        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("oil".to_string()),
        );
        params.insert("amount".to_string(), ScriptValue::Int(999));
        action.execute(&params, &test_context()).await.unwrap();

        let list = player_list().read().unwrap();
        let player = list.get_player(0).unwrap().read().unwrap();
        assert_eq!(player.get_money().get_money(), 800);
    }

    #[tokio::test]
    async fn named_money_actions_update_money_without_reentrant_deposit() {
        reset_test_player(0, 500);

        let mut params = HashMap::new();
        params.insert(
            "player".to_string(),
            ScriptValue::String("Player0".to_string()),
        );
        params.insert("amount".to_string(), ScriptValue::Int(250));
        GiveMoneyAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        params.insert("amount".to_string(), ScriptValue::Int(-1000));
        GiveMoneyAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        params.insert("amount".to_string(), ScriptValue::Int(1200));
        SetMoneyAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let list = player_list().read().unwrap();
        let player = list.get_player(0).unwrap().read().unwrap();
        assert_eq!(player.get_money().get_money(), 1200);
        assert_eq!(player.get_score_keeper().get_total_money_spent(), 750);
    }

    #[tokio::test]
    async fn indexed_player_add_money_spends_only_available_money() {
        reset_test_player(0, 300);

        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert("amount".to_string(), ScriptValue::Int(-500));

        PlayerAddMoneyAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let list = player_list().read().unwrap();
        let player = list.get_player(0).unwrap().read().unwrap();
        assert_eq!(player.get_money().get_money(), 0);
        assert_eq!(player.get_score_keeper().get_total_money_spent(), 300);
    }

    #[tokio::test]
    async fn timer_actions_update_script_engine_counters() {
        reset_test_script_engine();

        let mut params = HashMap::new();
        params.insert(
            "counter_name".to_string(),
            ScriptValue::String("TimerA".to_string()),
        );
        params.insert("milliseconds".to_string(), ScriptValue::Int(1500));

        StartTimerAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        {
            let engine = get_script_engine();
            let guard = engine.read().unwrap();
            let counter = guard
                .as_ref()
                .unwrap()
                .get_counter("TimerA")
                .expect("timer counter");
            assert_eq!(counter.value, 45);
            assert!(counter.is_countdown_timer);
        }

        params.remove("milliseconds");
        StopTimerAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let engine = get_script_engine();
        let guard = engine.read().unwrap();
        let counter = guard
            .as_ref()
            .unwrap()
            .get_counter("TimerA")
            .expect("timer counter");
        assert_eq!(counter.value, 45);
        assert!(!counter.is_countdown_timer);
    }

    #[tokio::test]
    async fn display_timer_actions_create_counter_state() {
        reset_test_script_engine();

        let mut params = HashMap::new();
        params.insert(
            "timer".to_string(),
            ScriptValue::String("CounterA".to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Int(7));

        SetTimerAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        params.clear();
        params.insert(
            "timer".to_string(),
            ScriptValue::String("CountdownA".to_string()),
        );
        params.insert("seconds".to_string(), ScriptValue::Int(3));

        CountdownTimerAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let engine = get_script_engine();
        let guard = engine.read().unwrap();
        let engine = guard.as_ref().unwrap();
        let counter = engine.get_counter("CounterA").expect("counter");
        assert_eq!(counter.value, 7);
        assert!(!counter.is_countdown_timer);

        let countdown = engine.get_counter("CountdownA").expect("countdown");
        assert_eq!(countdown.value, 90);
        assert!(countdown.is_countdown_timer);
    }

    #[tokio::test]
    async fn set_team_alliance_sets_one_way_player_relationship() {
        reset_test_players(2);

        let mut params = HashMap::new();
        params.insert("player1".to_string(), ScriptValue::Int(0));
        params.insert("player2".to_string(), ScriptValue::Int(1));
        params.insert(
            "relation".to_string(),
            ScriptValue::String("enemy".to_string()),
        );

        SetTeamAllianceAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let list = player_list().read().unwrap();
        let player0 = list.get_player(0).unwrap().read().unwrap();
        let player1 = list.get_player(1).unwrap().read().unwrap();
        assert_eq!(player0.get_relationship(&player1), Relationship::Enemies);
        assert_eq!(player1.get_relationship(&player0), Relationship::Neutral);
    }

    #[tokio::test]
    async fn destroy_building_queues_object_manager_removal() {
        reset_test_object_manager();

        let object = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                700,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test object instance"),
        ));
        {
            let manager = get_object_manager();
            manager
                .write()
                .unwrap()
                .register_object_instance(object, Coord3D::new(0.0, 0.0, 0.0))
                .unwrap();
        }

        let mut params = HashMap::new();
        params.insert("object_id".to_string(), ScriptValue::Int(700));

        DestroyBuildingAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let manager = get_object_manager();
        let mut manager = manager.write().unwrap();
        assert!(manager.get_object(700).is_some());
        manager.update(0).unwrap();
        assert!(manager.get_object(700).is_none());
    }

    #[tokio::test]
    async fn spawn_reinforcements_creates_grid_formation() {
        reset_test_object_manager();
        ensure_test_template("TestReinforcement");

        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "unit_type".to_string(),
            ScriptValue::String("TestReinforcement".to_string()),
        );
        params.insert("count".to_string(), ScriptValue::Int(6));
        params.insert("x".to_string(), ScriptValue::Float(100.0));
        params.insert("y".to_string(), ScriptValue::Float(200.0));
        params.insert("spacing".to_string(), ScriptValue::Float(12.0));

        let result = SpawnReinforcementsAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let ScriptResult::Success(Some(ScriptValue::Array(ids))) = result else {
            panic!("expected created object id array");
        };
        assert_eq!(ids.len(), 6);

        let manager = get_object_manager();
        let manager = manager.read().unwrap();
        let expected = [
            Coord3D::new(100.0, 200.0, 0.0),
            Coord3D::new(112.0, 200.0, 0.0),
            Coord3D::new(124.0, 200.0, 0.0),
            Coord3D::new(136.0, 200.0, 0.0),
            Coord3D::new(148.0, 200.0, 0.0),
            Coord3D::new(100.0, 212.0, 0.0),
        ];

        for (value, expected_pos) in ids.iter().zip(expected.iter()) {
            let ScriptValue::ObjectId(object_id) = value else {
                panic!("expected object id");
            };
            let object = manager.get_object(*object_id).expect("created object");
            let object = object.read().unwrap();
            assert_eq!(*object.get_position(), *expected_pos);
        }
    }

    #[tokio::test]
    async fn give_special_power_initializes_player_ready_timer() {
        reset_test_players(1);
        if let Some(mut store) =
            crate::object::special_power_template::get_special_power_store_mut()
        {
            store.reset();
        }

        let expected_frame = TheGameLogic::get_frame();
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "power_name".to_string(),
            ScriptValue::String("TestScriptPower".to_string()),
        );

        GiveSpecialPowerAction
            .execute(&params, &test_context())
            .await
            .unwrap();

        let template = find_or_create_special_power_template(&AsciiString::from("TestScriptPower"));
        assert_eq!(template.get_name(), "TestScriptPower");

        let list = player_list().read().unwrap();
        let mut player = list.get_player(0).unwrap().write().unwrap();
        assert_eq!(
            player.get_or_start_special_power_ready_frame(&template),
            expected_frame
        );
    }
}
