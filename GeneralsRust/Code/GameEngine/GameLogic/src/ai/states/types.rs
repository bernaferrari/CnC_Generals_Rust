#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
use super::guard::*;
use super::hack::*;
use super::helpers::*;
use super::hunt::*;
use super::idle::*;
use super::r#move::*;
use super::rappel::*;
use super::state_machine::*;
use super::wait_busy::*;
use super::wander_panic::*;
use super::waypoint::*;
use super::*;

use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::group::AIGroup;
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::AIGuardRetaliateMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::Path;
use crate::ai::squad::Squad;
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, THE_AI,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, get_game_logic_random_value};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::GRAVITY;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::*;
use crate::team::{Team, TeamID, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{
    NO_MAX_SHOTS_LIMIT, Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus,
};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// AI state types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AIStateType {
    Idle,
    MoveTo,
    FollowWaypointPathAsTeam,
    FollowWaypointPathAsIndividuals,
    FollowWaypointPathAsTeamExact,
    FollowWaypointPathAsIndividualsExact,
    FollowPath,
    FollowExitProductionPath,
    Wait,
    AttackPosition,
    AttackObject,
    ForceAttackObject,
    AttackAndFollowObject,
    Dead,
    Dock,
    Enter,
    Guard,
    Hunt,
    Wander,
    Panic,
    AttackSquad,
    GuardTunnelNetwork,
    GetRepaired,
    MoveOutOfTheWay,
    MoveAndTighten,
    MoveAndEvacuate,
    MoveAndEvacuateAndExit,
    MoveAndDelete,
    AttackArea,
    HackInternet,
    AttackMoveTo,
    AttackFollowWaypointPathAsIndividuals,
    AttackFollowWaypointPathAsTeam,
    FaceObject,
    FacePosition,
    RappelInto,
    CombatDrop,
    Exit,
    PickUpCrate,
    MoveAwayFromRepulsors,
    WanderInPlace,
    Busy,
    ExitInstantly,
    GuardRetaliate,
}

impl From<AIStateType> for u32 {
    fn from(state: AIStateType) -> Self {
        state as u32
    }
}

pub type AICommandType = crate::ai::AiCommandType;

pub type AiCommandType = AICommandType;

/// AI command parameters
pub struct AICommandParms {
    /// The command type
    pub cmd: AICommandType,
    /// Command source
    pub cmd_source: CommandSourceType,
    /// Target position
    pub pos: Coord3D,
    /// Target object id (resolve for the duration of an op)
    pub obj: ObjectID,
    /// Other object parameter id
    pub other_obj: ObjectID,
    /// Target team
    pub team: Option<Arc<RwLock<Team>>>,
    /// Waypoint path
    pub waypoint: Option<Arc<Waypoint>>,
    /// Polygon area
    pub polygon: Option<Arc<PolygonTrigger>>,
    /// Integer parameter
    pub int_value: i32,
    /// Damage information
    pub damage: DamageInfo,
    /// Command button
    pub command_button: Option<Arc<CommandButton>>,
    pub command_button_name: String,
    /// Path to follow
    pub path: Option<Arc<Mutex<Path>>>,
    /// Coordinate list
    pub coords: Vec<Coord3D>,
}

impl AICommandParms {
    pub fn new(cmd: AICommandType, cmd_source: CommandSourceType) -> Self {
        Self {
            cmd,
            cmd_source,
            pos: Coord3D::new(0.0, 0.0, 0.0),
            obj: INVALID_ID,
            other_obj: INVALID_ID,
            team: None,
            waypoint: None,
            polygon: None,
            int_value: 0,
            damage: DamageInfo::new(),
            command_button: None,
            command_button_name: String::new(),
            path: None,
            coords: Vec::new(),
        }
    }
}

/// Storage for AI command parameters (for serialization)
pub struct AICommandParmsStorage {
    pub cmd: AICommandType,
    pub cmd_source: CommandSourceType,
    pub pos: Coord3D,
    pub obj: ObjectID,
    pub other_obj: ObjectID,
    pub team_name: String,
    pub coords: Vec<Coord3D>,
    pub waypoint: Option<Arc<Waypoint>>,
    pub polygon: Option<Arc<PolygonTrigger>>,
    pub int_value: i32,
    pub damage: DamageInfo,
    pub command_button: Option<Arc<CommandButton>>,
    pub command_button_name: String,
    pub path: Option<Arc<Mutex<Path>>>,
}

impl AICommandParmsStorage {
    /// Store command parameters for serialization
    pub fn store(&mut self, parms: &AICommandParms) {
        self.cmd = parms.cmd;
        self.cmd_source = parms.cmd_source;
        self.pos = parms.pos;
        self.obj = parms.obj;
        self.other_obj = parms.other_obj;
        self.team_name = parms
            .team
            .as_ref()
            .and_then(|t| t.read().ok())
            .map(|team_ref| team_ref.get_name().as_str().to_owned())
            .unwrap_or_default();
        self.coords = parms.coords.clone();
        self.waypoint = parms.waypoint.clone();
        self.polygon = parms.polygon.clone();
        self.int_value = parms.int_value;
        self.damage = parms.damage.clone();
        self.command_button = parms.command_button.clone();
        self.command_button_name = match parms.command_button.as_ref() {
            Some(button) => button.get_name().to_owned(),
            None => parms.command_button_name.clone(),
        };
        self.path = parms.path.clone();
    }

    /// Reconstitute command parameters from storage
    pub fn reconstitute(&self, parms: &mut AICommandParms) {
        parms.cmd = self.cmd;
        parms.cmd_source = self.cmd_source;
        parms.pos = self.pos;
        parms.obj = self.obj;
        parms.other_obj = self.other_obj;
        parms.team = if self.team_name.is_empty() {
            None
        } else {
            TheTeamFactory()
                .lock()
                .ok()
                .and_then(|mut factory| factory.find_team(&self.team_name))
        };
        parms.coords = self.coords.clone();
        parms.waypoint = self.waypoint.clone();
        parms.polygon = self.polygon.clone();
        parms.int_value = self.int_value;
        parms.damage = self.damage.clone();
        parms.command_button = self.command_button.clone();
        parms.command_button_name = self.command_button_name.clone();
        parms.path = self.path.clone();
    }
    pub fn do_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut cmd_value = self.cmd as i32;
        xfer.xfer_i32(&mut cmd_value)
            .map_err(|e| format!("Failed to xfer cmd: {:?}", e))?;
        if xfer.is_loading() {
            self.cmd = ai_command_type_from_i32(cmd_value);
        }

        let mut cmd_source_value = self.cmd_source as i32;
        xfer.xfer_i32(&mut cmd_source_value)
            .map_err(|e| format!("Failed to xfer cmd_source: {:?}", e))?;
        if xfer.is_loading() {
            self.cmd_source = command_source_type_from_i32(cmd_source_value);
        }

        xfer.xfer_real(&mut self.pos.x)
            .map_err(|e| format!("Failed to xfer pos.x: {:?}", e))?;
        xfer.xfer_real(&mut self.pos.y)
            .map_err(|e| format!("Failed to xfer pos.y: {:?}", e))?;
        xfer.xfer_real(&mut self.pos.z)
            .map_err(|e| format!("Failed to xfer pos.z: {:?}", e))?;
        xfer.xfer_object_id(&mut self.obj)
            .map_err(|e| format!("Failed to xfer obj: {:?}", e))?;
        xfer.xfer_object_id(&mut self.other_obj)
            .map_err(|e| format!("Failed to xfer other_obj: {:?}", e))?;
        xfer.xfer_ascii_string(&mut self.team_name)
            .map_err(|e| format!("Failed to xfer team_name: {:?}", e))?;

        let mut num_coords = self.coords.len() as i32;
        xfer.xfer_int(&mut num_coords)
            .map_err(|e| format!("Failed to xfer coords size: {:?}", e))?;
        if xfer.is_loading() {
            self.coords.clear();
        }
        for idx in 0..num_coords.max(0) {
            let mut pos = if xfer.is_loading() {
                Coord3D::new(0.0, 0.0, 0.0)
            } else {
                self.coords
                    .get(idx as usize)
                    .copied()
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0))
            };
            xfer.xfer_real(&mut pos.x)
                .map_err(|e| format!("Failed to xfer coords[{idx}].x: {:?}", e))?;
            xfer.xfer_real(&mut pos.y)
                .map_err(|e| format!("Failed to xfer coords[{idx}].y: {:?}", e))?;
            xfer.xfer_real(&mut pos.z)
                .map_err(|e| format!("Failed to xfer coords[{idx}].z: {:?}", e))?;
            if xfer.is_loading() {
                self.coords.push(pos);
            }
        }

        let mut waypoint_id = self
            .waypoint
            .as_ref()
            .map(|waypoint| waypoint.id)
            .unwrap_or(INVALID_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut waypoint_id)
            .map_err(|e| format!("Failed to xfer waypoint_id: {:?}", e))?;
        if xfer.is_loading() {
            self.waypoint = None;
            if waypoint_id != INVALID_WAYPOINT_ID {
                if let Ok(terrain) = get_terrain_logic().read() {
                    if let Some(waypoint) = terrain.get_waypoint_by_id(waypoint_id) {
                        self.waypoint = Some(Arc::new(Waypoint::from_terrain(waypoint)));
                    }
                }
            }
        }

        let mut trigger_name = String::new();
        if let Some(polygon) = &self.polygon {
            trigger_name = polygon.get_trigger_name().str().to_string();
        }
        xfer.xfer_ascii_string(&mut trigger_name)
            .map_err(|e| format!("Failed to xfer trigger name: {:?}", e))?;
        if xfer.is_loading() {
            self.polygon = None;
            if !trigger_name.is_empty() {
                if let Ok(terrain) = get_terrain_logic().read() {
                    if let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name) {
                        self.polygon = Some(Arc::new(trigger.clone()));
                    }
                }
            }
        }

        xfer.xfer_int(&mut self.int_value)
            .map_err(|e| format!("Failed to xfer int_value: {:?}", e))?;

        self.damage.xfer(xfer);

        let mut command_name = self.command_button_name.clone();
        xfer.xfer_ascii_string(&mut command_name)
            .map_err(|e| format!("Failed to xfer command button name: {:?}", e))?;
        if xfer.is_loading() {
            self.command_button_name = command_name;
            self.command_button = None;
            if !self.command_button_name.is_empty() {
                if let Some(control_bar) = get_control_bar_bridge() {
                    if let Some(button) =
                        control_bar.find_command_button_by_name(&self.command_button_name)
                    {
                        self.command_button = Some(Arc::new(button.clone()));
                    }
                }
            }
        }

        let mut has_path = self.path.is_some();
        xfer.xfer_bool(&mut has_path)
            .map_err(|e| format!("Failed to xfer has_path: {:?}", e))?;
        if xfer.is_loading() {
            if has_path && self.path.is_none() {
                self.path = Some(Arc::new(Mutex::new(Path::new())));
            }
            if !has_path {
                self.path = None;
            }
        }
        if has_path {
            if let Some(path_arc) = &self.path {
                if let Ok(mut guard) = path_arc.lock() {
                    guard
                        .xfer(xfer)
                        .map_err(|e| format!("Failed to xfer path: {}", e))?;
                }
            }
        }

        Ok(())
    }
}

pub(crate) fn ai_command_type_from_i32(value: i32) -> AICommandType {
    match value {
        -1 => AiCommandType::NoCommand,
        0 => AiCommandType::MoveToPosition,
        1 => AiCommandType::MoveToObject,
        2 => AiCommandType::TightenToPosition,
        3 => AiCommandType::MoveToPositionAndEvacuate,
        4 => AiCommandType::MoveToPositionAndEvacuateAndExit,
        5 => AiCommandType::Idle,
        6 => AiCommandType::FollowWaypointPath,
        7 => AiCommandType::FollowWaypointPathAsTeam,
        8 => AiCommandType::FollowUserPath,
        9 => AiCommandType::FollowPath,
        10 => AiCommandType::FollowExitProductionPath,
        11 => AiCommandType::AttackObject,
        12 => AiCommandType::ForceAttackObject,
        13 => AiCommandType::AttackTeam,
        14 => AiCommandType::AttackPosition,
        15 => AiCommandType::AttackMoveToPosition,
        16 => AiCommandType::AttackFollowWaypointPath,
        17 => AiCommandType::AttackFollowWaypointPathAsTeam,
        18 => AiCommandType::Hunt,
        19 => AiCommandType::Repair,
        20 => AiCommandType::PickUpPrisoner,
        21 => AiCommandType::ReturnPrisoners,
        22 => AiCommandType::ResumeConstruction,
        23 => AiCommandType::GetHealed,
        24 => AiCommandType::GetRepaired,
        25 => AiCommandType::Enter,
        26 => AiCommandType::Dock,
        27 => AiCommandType::Exit,
        28 => AiCommandType::Evacuate,
        29 => AiCommandType::ExecuteRailedTransport,
        30 => AiCommandType::GoProne,
        31 => AiCommandType::GuardPosition,
        32 => AiCommandType::GuardObject,
        33 => AiCommandType::GuardArea,
        34 => AiCommandType::DeployAssaultReturn,
        35 => AiCommandType::AttackArea,
        36 => AiCommandType::HackInternet,
        37 => AiCommandType::FaceObject,
        38 => AiCommandType::FacePosition,
        39 => AiCommandType::RappelInto,
        40 => AiCommandType::CombatDrop,
        41 => AiCommandType::CommandButtonPos,
        42 => AiCommandType::CommandButtonObj,
        43 => AiCommandType::CommandButton,
        44 => AiCommandType::Wander,
        45 => AiCommandType::WanderInPlace,
        46 => AiCommandType::Panic,
        47 => AiCommandType::Busy,
        48 => AiCommandType::FollowWaypointPathExact,
        49 => AiCommandType::FollowWaypointPathAsTeamExact,
        50 => AiCommandType::MoveAwayFromUnit,
        51 => AiCommandType::FollowPathAppend,
        52 => AiCommandType::MoveToPositionEvenIfSleeping,
        53 => AiCommandType::GuardTunnelNetwork,
        54 => AiCommandType::EvacuateInstantly,
        55 => AiCommandType::ExitInstantly,
        56 => AiCommandType::GuardRetaliate,
        _ => AiCommandType::NoCommand,
    }
}

pub(crate) fn command_source_type_from_i32(value: i32) -> CommandSourceType {
    match value {
        0 => CommandSourceType::FromPlayer,
        1 => CommandSourceType::FromScript,
        2 => CommandSourceType::FromAi,
        3 => CommandSourceType::FromDozer,
        4 => CommandSourceType::DefaultSwitchWeapon,
        _ => CommandSourceType::FromAi,
    }
}
