#![allow(deprecated)]

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
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers, AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction,
    PartitionFilter, THE_AI,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{legacy_transition, register_classic_state, ClassicState};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{get_game_logic_random_value, TheAudio, TheGameLogic, ThePartitionManager};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::*;
use crate::team::{Team, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{Weapon, WeaponLockType, WeaponSlotType, WeaponStatus, NO_MAX_SHOTS_LIMIT};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

fn is_cliff_at(pos: &Coord3D) -> bool {
    get_terrain_logic()
        .read()
        .map(|terrain| terrain.is_cliff_cell(pos.x, pos.y))
        .unwrap_or(false)
}

fn normalize_angle(angle: Real) -> Real {
    let mut a = angle;
    let two_pi = std::f32::consts::PI * 2.0;
    while a > std::f32::consts::PI {
        a -= two_pi;
    }
    while a < -std::f32::consts::PI {
        a += two_pi;
    }
    a
}

fn is_in_region_no_z(region: &Region3D, position: &Coord3D) -> bool {
    position.x >= region.lo.x
        && position.x <= region.hi.x
        && position.y >= region.lo.y
        && position.y <= region.hi.y
}

fn is_point_on_wall(pos: &Coord3D) -> bool {
    let cell_pad = PATHFIND_CELL_SIZE_F * 0.5;
    for obj in OBJECT_REGISTRY.get_all_objects() {
        if let Ok(obj_guard) = obj.read() {
            if !obj_guard.is_any_kind_of(&[KindOf::Barrier]) {
                continue;
            }
            let wall_pos = obj_guard.get_position();
            let geom = obj_guard.get_template().get_template_geometry_info();
            let radius = geom.get_bounding_circle_radius();
            let dx = wall_pos.x - pos.x;
            let dy = wall_pos.y - pos.y;
            let dist_sq = dx * dx + dy * dy;
            let allowed = radius + cell_pad;
            if dist_sq <= allowed * allowed {
                return true;
            }
        }
    }
    false
}

fn get_wall_height() -> Real {
    THE_AI
        .read()
        .ok()
        .and_then(|ai| ai.get_ai_data().read().ok().map(|data| data.wall_height))
        .unwrap_or(0.0)
}

fn resolve_waypoint_by_id(id: WaypointId) -> Option<Arc<Waypoint>> {
    let terrain = get_terrain_logic().read().ok()?;
    let waypoint = terrain.get_waypoint_by_id(id)?;
    Some(Arc::new(Waypoint::from_terrain(waypoint)))
}

enum FollowPathStateKindMut<'a> {
    FollowPath(&'a mut AIFollowPathState),
    FollowExitProductionPath(&'a mut AIFollowExitProductionPathState),
}

impl<'a> FollowPathStateKindMut<'a> {
    fn set_path(self, path: Vec<Coord3D>, ignore_object: Option<ObjectID>) {
        match self {
            Self::FollowPath(state) => {
                state.set_path(path);
                state.set_ignore_object_id(ignore_object);
            }
            Self::FollowExitProductionPath(state) => {
                state.set_path(path);
                state.base.set_ignore_object_id(ignore_object);
            }
        }
    }

    fn append_path(self, position: Coord3D) {
        match self {
            Self::FollowPath(state) => state.append_path(position),
            Self::FollowExitProductionPath(state) => state.append_path(position),
        }
    }
}

fn state_follow_path_kind(
    state: &mut dyn StateImplementation,
) -> Option<FollowPathStateKindMut<'_>> {
    let any = state as &mut dyn std::any::Any;
    if any.is::<AIFollowExitProductionPathState>() {
        let state = any
            .downcast_mut::<AIFollowExitProductionPathState>()
            .expect("type check and downcast must match");
        return Some(FollowPathStateKindMut::FollowExitProductionPath(state));
    }
    if any.is::<AIFollowPathState>() {
        let state = any
            .downcast_mut::<AIFollowPathState>()
            .expect("type check and downcast must match");
        return Some(FollowPathStateKindMut::FollowPath(state));
    }

    None
}

#[derive(Debug)]
struct FollowWaypointPathCore {
    move_as_group: bool,
    is_follow_waypoint_path_state: bool,
    current_waypoint: Option<Arc<Waypoint>>,
    prior_waypoint: Option<Arc<Waypoint>>,
    group_offset: Coord2D,
    angle: Real,
    frames_sleeping: UnsignedInt,
    append_goal_position: bool,
    goal_position: Coord3D,
    goal_layer: PathfindLayerEnum,
}

impl FollowWaypointPathCore {
    fn new(move_as_group: bool, is_follow_waypoint_path_state: bool) -> Self {
        Self {
            move_as_group,
            is_follow_waypoint_path_state,
            current_waypoint: None,
            prior_waypoint: None,
            group_offset: Coord2D::new(0.0, 0.0),
            angle: 0.0,
            frames_sleeping: 0,
            append_goal_position: false,
            goal_position: Coord3D::origin(),
            goal_layer: PathfindLayerEnum::Ground,
        }
    }

    fn has_next_waypoint(&self) -> bool {
        self.current_waypoint
            .as_ref()
            .map(|waypoint| waypoint.get_num_links() > 0)
            .unwrap_or(false)
    }

    fn get_next_waypoint(&mut self, state: &State) -> Option<Arc<Waypoint>> {
        let current = self.current_waypoint.as_ref()?;
        let link_count = current.get_num_links();
        if link_count == 0 {
            self.prior_waypoint = self.current_waypoint.clone();
            return None;
        }

        let which = get_game_logic_random_value(0, (link_count - 1) as i32) as usize;
        let next_id = current.get_link(which)?;
        self.prior_waypoint = self.current_waypoint.clone();
        if let Ok(machine) = state.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(current.position);
            }
        }
        resolve_waypoint_by_id(next_id)
    }

    fn calc_extra_path_distance(&self) -> Real {
        let mut extra = PATHFIND_CELL_SIZE_F / 10.0;
        let mut cur = self.current_waypoint.clone();
        let mut limit = 5;
        while let Some(way) = cur.take() {
            if limit == 0 {
                break;
            }
            limit -= 1;
            if way.get_num_links() == 0 {
                break;
            }
            let next_id = way.get_link(0);
            let Some(next_id) = next_id else {
                break;
            };
            let next = resolve_waypoint_by_id(next_id);
            let Some(next_way) = next else {
                break;
            };
            let dx = next_way.position.x - way.position.x;
            let dy = next_way.position.y - way.position.y;
            extra += (dx * dx + dy * dy).sqrt();
            cur = Some(next_way);
        }
        extra
    }

    fn compute_goal(
        &mut self,
        state: &State,
        owner: &Object,
        ai: &mut dyn AIUpdateInterface,
        use_group_offsets: bool,
    ) -> Result<(), String> {
        let Some(current_waypoint) = self.current_waypoint.as_ref() else {
            return Ok(());
        };

        let mut dest = current_waypoint.position;
        self.goal_layer = PathfindLayerEnum::Ground;
        if is_point_on_wall(&dest) {
            dest.z = get_wall_height();
            self.goal_layer = PathfindLayerEnum::Wall;
        }
        self.goal_position = dest;

        if use_group_offsets {
            self.goal_position.x += self.group_offset.x;
            self.goal_position.y += self.group_offset.y;
        }

        if let Ok(terrain_guard) = get_terrain_logic().read() {
            if self.goal_layer == PathfindLayerEnum::Wall {
                if !is_point_on_wall(&self.goal_position) {
                    self.goal_position = dest;
                }
                self.goal_position.z = get_wall_height();
            } else {
                self.goal_layer = PathfindLayerEnum::Ground;
                self.goal_position.z = terrain_guard.get_ground_height(
                    self.goal_position.x,
                    self.goal_position.y,
                    None,
                );
            }
            let extent = terrain_guard.get_maximum_pathfind_extent();
            if is_in_region_no_z(&extent, &dest) && !is_in_region_no_z(&extent, &self.goal_position)
            {
                if self.goal_position.x < extent.lo.x + PATHFIND_CELL_SIZE_F {
                    self.goal_position.x = extent.lo.x + PATHFIND_CELL_SIZE_F;
                }
                if self.goal_position.y < extent.lo.y + PATHFIND_CELL_SIZE_F {
                    self.goal_position.y = extent.lo.y + PATHFIND_CELL_SIZE_F;
                }
                if self.goal_position.x > extent.hi.x - PATHFIND_CELL_SIZE_F {
                    self.goal_position.x = extent.hi.x - PATHFIND_CELL_SIZE_F;
                }
                if self.goal_position.y > extent.hi.y - PATHFIND_CELL_SIZE_F {
                    self.goal_position.y = extent.hi.y - PATHFIND_CELL_SIZE_F;
                }
            }
            if !is_in_region_no_z(&extent, &self.goal_position) {
                ai.set_adjusts_destination(false);
                let _ = ai.set_allow_invalid_position(true);
                self.append_goal_position = true;
            }
        }

        if self.has_next_waypoint() {
            ai.set_adjusts_destination(false);
        } else {
            ai.set_adjusts_destination(true);
            if owner.is_kind_of(KindOf::Projectile) {
                if let Some(locomotor) = ai.get_cur_locomotor() {
                    if let Ok(mut guard) = locomotor.lock() {
                        guard.set_precise_z_pos(true);
                    }
                }
            }
        }

        ai.set_path_extra_distance(self.calc_extra_path_distance())
            .map_err(|e| e.to_string())?;
        if let Ok(machine) = state.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(self.goal_position);
            }
        }
        dest = self.goal_position;
        Ok(())
    }

    pub(crate) fn compute_path(&mut self, ai: &mut dyn AIUpdateInterface) -> Result<(), String> {
        ai.set_movement_target(&self.goal_position).map_err(|err| {
            format!(
                "FollowWaypointPathState set_movement_target failed: {}",
                err
            )
        })
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_real(&mut self.group_offset.x)
            .map_err(|e| format!("Failed to xfer group_offset.x: {:?}", e))?;
        xfer.xfer_real(&mut self.group_offset.y)
            .map_err(|e| format!("Failed to xfer group_offset.y: {:?}", e))?;
        xfer.xfer_real(&mut self.angle)
            .map_err(|e| format!("Failed to xfer angle: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.frames_sleeping)
            .map_err(|e| format!("Failed to xfer frames_sleeping: {:?}", e))?;
        xfer.xfer_bool(&mut self.append_goal_position)
            .map_err(|e| format!("Failed to xfer append_goal_position: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.x)
            .map_err(|e| format!("Failed to xfer goal_position.x: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.y)
            .map_err(|e| format!("Failed to xfer goal_position.y: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.z)
            .map_err(|e| format!("Failed to xfer goal_position.z: {:?}", e))?;

        let mut current_id: WaypointId = self
            .current_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut current_id)
            .map_err(|e| format!("Failed to xfer current waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.current_waypoint = if current_id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(current_id)
            };
        }

        let mut prior_id: WaypointId = self
            .prior_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut prior_id)
            .map_err(|e| format!("Failed to xfer prior waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.prior_waypoint = if prior_id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(prior_id)
            };
        }

        Ok(())
    }
}

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
    /// Target object
    pub obj: Option<Arc<RwLock<Object>>>,
    /// Other object parameter
    pub other_obj: Option<Arc<RwLock<Object>>>,
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
            obj: None,
            other_obj: None,
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
        self.obj = parms
            .obj
            .as_ref()
            .map(|o| {
                o.try_read()
                    .map(|obj_ref| obj_ref.get_id())
                    .unwrap_or(INVALID_ID)
            })
            .unwrap_or(INVALID_ID);
        self.other_obj = parms
            .other_obj
            .as_ref()
            .map(|o| {
                o.try_read()
                    .map(|obj_ref| obj_ref.get_id())
                    .unwrap_or(INVALID_ID)
            })
            .unwrap_or(INVALID_ID);
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
        parms.obj = get_legacy_object(self.obj);
        parms.other_obj = get_legacy_object(self.other_obj);
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

fn ai_command_type_from_i32(value: i32) -> AICommandType {
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

fn command_source_type_from_i32(value: i32) -> CommandSourceType {
    match value {
        0 => CommandSourceType::FromPlayer,
        1 => CommandSourceType::FromScript,
        2 => CommandSourceType::FromAi,
        3 => CommandSourceType::FromDozer,
        4 => CommandSourceType::DefaultSwitchWeapon,
        _ => CommandSourceType::FromAi,
    }
}

/// The AI state machine - implements all AI commands
pub struct AIStateMachine {
    /// Base state machine
    base: StateMachine,
    /// Goal path to follow
    goal_path: Vec<Coord3D>,
    /// Goal waypoint
    goal_waypoint: Option<Arc<Waypoint>>,
    /// Goal squad to attack
    goal_squad: Option<Arc<Mutex<Squad>>>,
    /// Goal polygon area
    goal_polygon: Option<Arc<PolygonTrigger>>,
    /// Temporary state for short interruptions
    temporary_state_id: Option<u32>,
    /// Frame when temporary state ends
    temporary_state_frame_end: u32,
}

impl std::fmt::Debug for AIStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIStateMachine")
            .field("goal_path_len", &self.goal_path.len())
            .field("has_goal_waypoint", &self.goal_waypoint.is_some())
            .field("has_goal_squad", &self.goal_squad.is_some())
            .field("has_goal_polygon", &self.goal_polygon.is_some())
            .field("temporary_state_id", &self.temporary_state_id)
            .field("temporary_state_frame_end", &self.temporary_state_frame_end)
            .finish()
    }
}

impl AIStateMachine {
    pub fn new(owner: Weak<RwLock<Object>>, name: &str) -> Self {
        let mut machine = Self {
            base: StateMachine::new(Some(owner), name),
            goal_path: Vec::new(),
            goal_waypoint: None,
            goal_squad: None,
            goal_polygon: None,
            temporary_state_id: None,
            temporary_state_frame_end: 0,
        };

        // Define all AI states
        machine.define_ai_states();
        machine
    }

    /// Define all AI states and their transitions
    fn define_ai_states(&mut self) {
        // Define basic movement states
        let idle_state = AIIdleState::new(&self.base, true);
        register_classic_state(
            &mut self.base,
            AIStateType::Idle.into(),
            idle_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_to_state = AIMoveToState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveTo.into(),
            move_to_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_out_state = AIMoveOutOfTheWayState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveOutOfTheWay.into(),
            move_out_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let tighten_state = AIMoveAndTightenState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndTighten.into(),
            tighten_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_away_state = AIMoveAwayFromRepulsorsState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAwayFromRepulsors.into(),
            move_away_state,
            Some(AIStateType::WanderInPlace as u32),
            Some(AIStateType::WanderInPlace as u32),
            &[],
        );

        let wander_in_place_state = AIWanderInPlaceState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::WanderInPlace.into(),
            wander_in_place_state,
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let follow_team_state = AIFollowWaypointPathAsTeamState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsTeam.into(),
            follow_team_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_individuals_state = AIFollowWaypointPathAsIndividualsState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsIndividuals.into(),
            follow_individuals_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_team_exact_state = AIFollowWaypointPathAsTeamExactState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsTeamExact.into(),
            follow_team_exact_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_individuals_exact_state =
            AIFollowWaypointPathAsIndividualsExactState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsIndividualsExact.into(),
            follow_individuals_exact_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_path_state = AIFollowPathState::new(&self.base, false);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowPath.into(),
            follow_path_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_exit_path_state = AIFollowExitProductionPathState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowExitProductionPath.into(),
            follow_exit_path_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Define attack states
        let attack_object_state = AIAttackObjectState::new(&self.base, false, false);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackObject.into(),
            attack_object_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let force_attack_state = AIAttackObjectState::new(&self.base, true, false);
        register_classic_state(
            &mut self.base,
            AIStateType::ForceAttackObject.into(),
            force_attack_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_state = AIAttackObjectState::new(&self.base, false, true);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackAndFollowObject.into(),
            attack_follow_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_position_state = AIAttackPositionState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackPosition.into(),
            attack_position_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_squad_state = AIAttackSquadState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackSquad.into(),
            attack_squad_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_area_state = AIAttackAreaState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackArea.into(),
            attack_area_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_move_state = AIAttackMoveToState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackMoveTo.into(),
            attack_move_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_team_state = AIAttackFollowWaypointPathAsTeamState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackFollowWaypointPathAsTeam.into(),
            attack_follow_team_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_individual_state =
            AIAttackFollowWaypointPathAsIndividualsState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackFollowWaypointPathAsIndividuals.into(),
            attack_follow_individual_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_state = AIGuardState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Guard.into(),
            guard_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_retaliate_state = AIGuardRetaliateState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::GuardRetaliate.into(),
            guard_retaliate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_tunnel_state = AITunnelNetworkGuardState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::GuardTunnelNetwork.into(),
            guard_tunnel_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let hunt_state = AIHuntState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Hunt.into(),
            hunt_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Define utility states
        let enter_state = AIEnterState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Enter.into(),
            enter_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let dock_state = AIDockState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Dock.into(),
            dock_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_evacuate_state = AIMoveAndEvacuateState::new(&self.base, "AIMoveAndEvacuate");
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndEvacuate.into(),
            move_evacuate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_evacuate_exit_state =
            AIMoveAndEvacuateState::new(&self.base, "AIMoveAndEvacuateAndExit");
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndEvacuateAndExit.into(),
            move_evacuate_exit_state,
            Some(AIStateType::MoveAndDelete as u32),
            Some(AIStateType::MoveAndDelete as u32),
            &[],
        );

        let move_and_delete_state = AIMoveAndDeleteState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndDelete.into(),
            move_and_delete_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let wait_state = AIWaitState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Wait.into(),
            wait_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let exit_state = AIExitState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Exit.into(),
            exit_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let exit_instant_state = AIExitInstantlyState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::ExitInstantly.into(),
            exit_instant_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let pick_up_crate_state = AIPickUpCrateState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::PickUpCrate.into(),
            pick_up_crate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let wander_state = AIWanderState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Wander.into(),
            wander_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let panic_state = AIPanicState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Panic.into(),
            panic_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let dead_state = AIDeadState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Dead.into(),
            dead_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let hack_internet_state = AIHackInternetState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::HackInternet.into(),
            hack_internet_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let face_object_state = AIFaceObjectState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FaceObject.into(),
            face_object_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let face_position_state = AIFacePositionState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FacePosition.into(),
            face_position_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let rappel_state = AIRappelIntoState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::RappelInto.into(),
            rappel_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let combat_drop_state = AICombatDropState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::CombatDrop.into(),
            combat_drop_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let busy_state = AIBusyState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Busy.into(),
            busy_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Set default state
        self.base.set_current_state(AIStateType::Idle.into());
    }

    fn notify_state_machine_changed(&self) {
        let Some(owner) = self.base.get_owner() else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return;
        };
        if let Ok(mut ai_guard) = ai.lock() {
            // C++ AIUpdateInterface::friend_notifyStateMachineChanged() wakes the AI immediately.
            ai_guard.set_queue_for_path_time(0);
        };
    }

    /// Clear the state machine
    pub fn clear(&mut self) {
        // C++ AIStateMachine::clear() calls StateMachine::clear(), not reset().
        self.base.clear();
        self.goal_path.clear();
        self.goal_waypoint = None;
        self.goal_squad = None;
        self.goal_polygon = None;
        self.base.set_goal_squad(None);
        self.base.set_goal_polygon(None);
        self.notify_state_machine_changed();
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        let ret = self.base.reset_to_default_state();
        self.notify_state_machine_changed();
        ret
    }

    pub fn get_current_state_id(&self) -> Option<u32> {
        self.base.get_current_state_id()
    }

    pub fn get_goal_position(&self) -> Option<Coord3D> {
        Some(self.base.get_goal_position())
    }

    /// Set state
    pub fn set_state(&mut self, new_state_id: u32) -> StateReturnType {
        let old_id = self.base.get_current_state_id();
        let ret = self.base.set_current_state(new_state_id);

        if old_id != Some(new_state_id) {
            self.notify_state_machine_changed();
        }

        ret
    }

    pub fn lock(&mut self) {
        self.base.lock();
    }

    pub fn unlock(&mut self) {
        self.base.unlock();
    }

    pub fn is_locked(&self) -> bool {
        self.base.is_locked()
    }

    pub fn set_goal_object(&mut self, obj_id: ObjectID) {
        let obj = get_legacy_object(obj_id);
        self.base
            .set_goal_object(obj.map(|value| Arc::downgrade(&value)));
    }

    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.set_goal_position(pos);
    }

    pub fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.base.get_goal_object()
    }

    pub fn is_idle(&self) -> bool {
        self.base.is_in_idle_state()
    }

    pub fn is_busy(&self) -> bool {
        self.base.is_in_busy_state()
    }

    pub fn is_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }

    /// Set goal path
    pub fn set_goal_path(&mut self, path: &[Coord3D]) {
        self.goal_path = path.to_vec();
    }

    /// Add to goal path
    pub fn add_to_goal_path(&mut self, path_point: &Coord3D) {
        if self.goal_path.is_empty() {
            self.goal_path.push(*path_point);
            return;
        }

        if let Some(final_point) = self.goal_path.last() {
            if final_point.x == path_point.x
                && final_point.y == path_point.y
                && final_point.z == path_point.z
            {
                return;
            }
        }

        self.goal_path.push(*path_point);
    }

    /// Get goal path position at index
    pub fn get_goal_path_position(&self, i: usize) -> Option<&Coord3D> {
        self.goal_path.get(i)
    }

    /// Get goal path size
    pub fn get_goal_path_size(&self) -> usize {
        self.goal_path.len()
    }

    /// Set goal waypoint
    pub fn set_goal_waypoint(&mut self, waypoint: Option<Arc<Waypoint>>) {
        self.goal_waypoint = waypoint;
        let waypoint_id = self.goal_waypoint.as_ref().map(|w| w.id);
        self.base.set_goal_waypoint(waypoint_id);
    }

    /// Get goal waypoint
    pub fn get_goal_waypoint(&self) -> Option<&Arc<Waypoint>> {
        self.goal_waypoint.as_ref()
    }

    /// Set goal team (converts to squad)
    pub fn set_goal_team(&mut self, team: &Arc<RwLock<Team>>) {
        let squad = self
            .goal_squad
            .get_or_insert_with(|| Arc::new(Mutex::new(Squad::new())))
            .clone();
        if let (Ok(team_guard), Ok(mut squad_guard)) = (team.read(), squad.lock()) {
            squad_guard.squad_from_team(&team_guard, true);
        }
        self.set_goal_squad(Some(squad));
    }

    /// Set goal squad
    /// Set goal squad
    pub fn set_goal_squad(&mut self, squad: Option<Arc<Mutex<Squad>>>) {
        if let Some(source) = squad {
            let target = self
                .goal_squad
                .get_or_insert_with(|| Arc::new(Mutex::new(Squad::new())))
                .clone();

            if !Arc::ptr_eq(&target, &source) {
                if let Ok(source_guard) = source.lock() {
                    if let Ok(mut target_guard) = target.lock() {
                        *target_guard = source_guard.clone();
                    }
                }
            }

            self.goal_squad = Some(target);
        } else {
            self.goal_squad = None;
        }

        self.base
            .set_goal_squad(self.goal_squad.as_ref().map(Arc::downgrade));
    }

    pub fn set_goal_polygon(&mut self, polygon: Option<Arc<PolygonTrigger>>) {
        self.goal_polygon = polygon.clone();
        self.base
            .set_goal_polygon(polygon.map(|value| Arc::downgrade(&value)));
    }

    /// Set goal AI group (converts to squad)
    pub fn set_goal_ai_group(&mut self, group: &AIGroup) {
        let squad = self
            .goal_squad
            .get_or_insert_with(|| Arc::new(Mutex::new(Squad::new())))
            .clone();
        if let Ok(mut squad_guard) = squad.lock() {
            squad_guard.squad_from_ai_group(group, true);
        }
        self.set_goal_squad(Some(squad));
    }

    /// Get goal squad
    pub fn get_goal_squad(&self) -> Option<&Arc<Mutex<Squad>>> {
        self.goal_squad.as_ref()
    }

    /// Set temporary state
    pub fn set_temporary_state(&mut self, new_state_id: u32, frame_limit: u32) -> StateReturnType {
        if let Some(current_id) = self.temporary_state_id.take() {
            if let Some(state) = self.base.get_state_mut(current_id) {
                state.on_exit(StateExitType::Reset);
            }
        }

        if let Some(state) = self.base.get_state_mut(new_state_id) {
            let ret = state.on_enter();
            if ret != StateReturnType::Continue {
                state.on_exit(StateExitType::Normal);
                return ret;
            }

            let max_limit = 60 * LOGICFRAMES_PER_SECOND;
            let capped_limit = frame_limit.min(max_limit);
            self.temporary_state_frame_end = TheGameLogic::get_frame().saturating_add(capped_limit);
            self.temporary_state_id = Some(new_state_id);
            return ret;
        }

        StateReturnType::Failure
    }

    /// Get temporary state ID
    pub fn get_temporary_state(&self) -> Option<u32> {
        self.temporary_state_id
    }

    /// Update state machine
    pub fn update_state_machine(&mut self) -> StateReturnType {
        if let Some(temp_state_id) = self.temporary_state_id {
            if let Some(state) = self.base.get_state_mut(temp_state_id) {
                let mut status = state.update();
                if self.temporary_state_frame_end < TheGameLogic::get_frame() {
                    if status == StateReturnType::Continue {
                        status = StateReturnType::Success;
                    }
                }
                if status == StateReturnType::Continue {
                    return status;
                }
                state.on_exit(StateExitType::Normal);
            }
            self.temporary_state_id = None;
        }

        // Update main state machine
        self.base.update()
    }

    /// Get current state name (for debugging)
    pub fn get_current_state_name(&self) -> String {
        let mut name = self.base.get_current_state_name();

        if let Some(temp_state_id) = self.temporary_state_id {
            if let Some(temp_name) = self.base.get_state_name_by_id(temp_state_id) {
                name.push_str(" /T/");
                name.push_str(temp_name);
            }
        }

        name
    }
}

impl AiCommandInterface for AIStateMachine {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), crate::ai::AiError> {
        let is_follow_path_cmd = matches!(
            params.cmd,
            AiCommandType::FollowPath
                | AiCommandType::FollowExitProductionPath
                | AiCommandType::FollowUserPath
                | AiCommandType::FollowPathAppend
        );
        if !is_follow_path_cmd {
            if let Some(obj_id) = params.obj {
                let obj = get_legacy_object(obj_id);
                self.base
                    .set_goal_object(obj.map(|value| Arc::downgrade(&value)));
            } else {
                self.base.set_goal_object(None);
            }
        } else {
            self.base.set_goal_object(None);
        }

        if params.pos != Coord3D::new(0.0, 0.0, 0.0) {
            self.base.set_goal_position(params.pos);
        }

        if let Some(team_name) = params.team.as_ref() {
            if let Ok(mut factory) = TheTeamFactory().lock() {
                if let Some(team) = factory.find_team(team_name) {
                    self.set_goal_team(&team);
                }
            }
        }

        if let Some(trigger_id) = params.polygon {
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(trigger_id) {
                    let trigger_arc = Arc::new(trigger.clone());
                    self.set_goal_polygon(Some(trigger_arc));
                }
            }
        }

        if let Some(waypoint_id) = params.waypoint {
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(waypoint) = terrain_guard.get_waypoint_by_id(waypoint_id) {
                    let arc = Arc::new(Waypoint::from_terrain(waypoint));
                    self.set_goal_waypoint(Some(arc));
                } else {
                    self.set_goal_waypoint(None);
                }
            }
        }

        if matches!(
            params.cmd,
            AiCommandType::FollowPath
                | AiCommandType::FollowExitProductionPath
                | AiCommandType::FollowUserPath
        ) {
            self.set_goal_path(&params.coords);
            let target_state = if matches!(params.cmd, AiCommandType::FollowExitProductionPath) {
                AIStateType::FollowExitProductionPath
            } else {
                AIStateType::FollowPath
            };
            if let Some(state) = self.base.get_state_mut(target_state as u32) {
                if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                    path_state.set_path(params.coords.clone(), params.obj);
                }
            }
        } else if matches!(params.cmd, AiCommandType::FollowPathAppend) {
            let append_pos = params.pos;
            self.add_to_goal_path(&append_pos);
            if let Some(state_id) = self.base.get_current_state_id() {
                if let Some(state) = self.base.get_state_mut(state_id) {
                    if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                        path_state.append_path(append_pos);
                    }
                }
            } else if let Some(state) = self.base.get_state_mut(AIStateType::FollowPath as u32) {
                if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                    path_state.append_path(append_pos);
                }
            }
        }

        let state = match params.cmd {
            AiCommandType::Idle => AIStateType::Idle,
            AiCommandType::MoveToPosition
            | AiCommandType::MoveToObject
            | AiCommandType::MoveToPositionEvenIfSleeping => AIStateType::MoveTo,
            AiCommandType::FollowWaypointPath => AIStateType::FollowWaypointPathAsIndividuals,
            AiCommandType::FollowWaypointPathAsTeam => AIStateType::FollowWaypointPathAsTeam,
            AiCommandType::FollowWaypointPathExact => {
                AIStateType::FollowWaypointPathAsIndividualsExact
            }
            AiCommandType::FollowWaypointPathAsTeamExact => {
                AIStateType::FollowWaypointPathAsTeamExact
            }
            AiCommandType::FollowPath => AIStateType::FollowPath,
            AiCommandType::FollowExitProductionPath => AIStateType::FollowExitProductionPath,
            AiCommandType::FollowUserPath => AIStateType::FollowPath,
            AiCommandType::FollowPathAppend => AIStateType::FollowPath,
            AiCommandType::MoveToPositionAndEvacuate => AIStateType::MoveAndEvacuate,
            AiCommandType::MoveToPositionAndEvacuateAndExit => AIStateType::MoveAndEvacuateAndExit,
            AiCommandType::AttackObject => AIStateType::AttackObject,
            AiCommandType::ForceAttackObject => AIStateType::ForceAttackObject,
            AiCommandType::AttackPosition => AIStateType::AttackPosition,
            AiCommandType::AttackMoveToPosition => AIStateType::AttackMoveTo,
            AiCommandType::AttackFollowWaypointPath => {
                AIStateType::AttackFollowWaypointPathAsIndividuals
            }
            AiCommandType::AttackFollowWaypointPathAsTeam => {
                AIStateType::AttackFollowWaypointPathAsTeam
            }
            AiCommandType::AttackTeam => AIStateType::AttackSquad,
            AiCommandType::Hunt => AIStateType::Hunt,
            AiCommandType::AttackArea => AIStateType::AttackArea,
            AiCommandType::Repair => AIStateType::Busy,
            AiCommandType::ResumeConstruction => AIStateType::Busy,
            AiCommandType::GetHealed => AIStateType::Enter,
            AiCommandType::GetRepaired => AIStateType::Dock,
            AiCommandType::Enter => AIStateType::Enter,
            AiCommandType::Dock => AIStateType::Dock,
            AiCommandType::Exit => AIStateType::Exit,
            AiCommandType::ExitInstantly => AIStateType::ExitInstantly,
            AiCommandType::Evacuate => AIStateType::Exit,
            AiCommandType::EvacuateInstantly => AIStateType::ExitInstantly,
            AiCommandType::ExecuteRailedTransport => AIStateType::Busy,
            AiCommandType::GoProne => AIStateType::Busy,
            AiCommandType::GuardPosition => AIStateType::Guard,
            AiCommandType::GuardObject => AIStateType::Guard,
            AiCommandType::GuardArea => AIStateType::Guard,
            AiCommandType::GuardTunnelNetwork => AIStateType::GuardTunnelNetwork,
            AiCommandType::GuardRetaliate => AIStateType::GuardRetaliate,
            AiCommandType::HackInternet => AIStateType::HackInternet,
            AiCommandType::FaceObject => AIStateType::FaceObject,
            AiCommandType::FacePosition => AIStateType::FacePosition,
            AiCommandType::RappelInto => AIStateType::RappelInto,
            AiCommandType::CombatDrop => AIStateType::CombatDrop,
            AiCommandType::PickUpPrisoner => AIStateType::PickUpCrate,
            AiCommandType::Wander => AIStateType::Wander,
            AiCommandType::WanderInPlace => AIStateType::WanderInPlace,
            AiCommandType::Panic => AIStateType::Panic,
            AiCommandType::Busy => AIStateType::Busy,
            AiCommandType::MoveAwayFromUnit => AIStateType::MoveOutOfTheWay,
            AiCommandType::TightenToPosition => AIStateType::MoveAndTighten,
            AiCommandType::ReturnPrisoners => AIStateType::Busy,
            AiCommandType::DoSpecialPower => AIStateType::Busy,
            AiCommandType::DoSpecialPowerAtObject => AIStateType::Busy,
            AiCommandType::DoSpecialPowerAtLocation => AIStateType::Busy,
            AiCommandType::Sell => AIStateType::Busy,
            AiCommandType::ToggleOvercharge => AIStateType::Busy,
            AiCommandType::Surrender => AIStateType::Busy,
            AiCommandType::Cheer => AIStateType::Busy,
            _ => AIStateType::Idle,
        };

        if matches!(
            params.cmd,
            AiCommandType::GuardPosition
                | AiCommandType::GuardObject
                | AiCommandType::GuardArea
                | AiCommandType::GuardTunnelNetwork
        ) {
            self.base.set_guard_mode_raw(params.int_value);
        }

        self.set_state(state as u32);
        Ok(())
    }
}

impl Snapshotable for AIStateMachine {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base
            .crc(xfer)
            .map_err(|e| format!("Failed to crc AIStateMachine base: {}", e))
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer AIStateMachine version: {:?}", e))?;

        self.base
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer AIStateMachine base: {}", e))?;

        let mut count = self.goal_path.len() as i32;
        xfer.xfer_int(&mut count)
            .map_err(|e| format!("Failed to xfer AIStateMachine goal path size: {:?}", e))?;

        for i in 0..count.max(0) {
            let mut pos = if xfer.is_loading() {
                Coord3D::new(0.0, 0.0, 0.0)
            } else {
                self.goal_path
                    .get(i as usize)
                    .copied()
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0))
            };

            xfer.xfer_real(&mut pos.x)
                .map_err(|e| format!("Failed to xfer goal_path[{i}].x: {:?}", e))?;
            xfer.xfer_real(&mut pos.y)
                .map_err(|e| format!("Failed to xfer goal_path[{i}].y: {:?}", e))?;
            xfer.xfer_real(&mut pos.z)
                .map_err(|e| format!("Failed to xfer goal_path[{i}].z: {:?}", e))?;
            if xfer.is_loading() {
                self.goal_path.push(pos);
            }
        }

        let mut waypoint_name = self
            .goal_waypoint
            .as_ref()
            .map(|waypoint| waypoint.name.clone())
            .unwrap_or_default();

        xfer.xfer_ascii_string(&mut waypoint_name)
            .map_err(|e| format!("Failed to xfer AIStateMachine waypoint name: {:?}", e))?;

        if xfer.is_loading() && !waypoint_name.is_empty() {
            let mut loaded_waypoint = None;
            let lookup = AsciiString::from(waypoint_name.as_str());
            if let Ok(terrain) = get_terrain_logic().read() {
                if let Some(waypoint) = terrain.get_waypoint_by_name(&lookup) {
                    loaded_waypoint = Some(Arc::new(Waypoint::new(
                        waypoint.get_id(),
                        *waypoint.get_location(),
                        waypoint.get_name().as_str().to_string(),
                    )));
                }
            }
            self.goal_waypoint = loaded_waypoint;
        }
        let waypoint_id = self.goal_waypoint.as_ref().map(|waypoint| waypoint.id);
        self.base.set_goal_waypoint(waypoint_id);

        let mut has_squad = self.goal_squad.is_some();
        xfer.xfer_bool(&mut has_squad)
            .map_err(|e| format!("Failed to xfer has_squad: {:?}", e))?;

        if xfer.is_loading() {
            if has_squad && self.goal_squad.is_none() {
                self.goal_squad = Some(Arc::new(Mutex::new(Squad::new())));
            }
        }

        if has_squad {
            if let Some(squad) = self.goal_squad.as_ref() {
                let mut guard = squad
                    .lock()
                    .map_err(|_| "AIStateMachine squad lock poisoned".to_string())?;
                guard.xfer(xfer)?;
            }
        }

        self.base
            .set_goal_squad(self.goal_squad.as_ref().map(|value| Arc::downgrade(value)));

        let mut temp_state_id = self.temporary_state_id.unwrap_or(INVALID_STATE_ID);

        xfer.xfer_unsigned_int(&mut temp_state_id)
            .map_err(|e| format!("Failed to xfer temporary_state_id: {:?}", e))?;

        if xfer.is_loading() && temp_state_id != INVALID_STATE_ID {
            self.temporary_state_id = self
                .base
                .get_state_name_by_id(temp_state_id)
                .map(|_| temp_state_id);
        }

        if temp_state_id != INVALID_STATE_ID {
            if let Some(state) = self.base.get_state_mut(temp_state_id) {
                state.xfer_snapshot(xfer)?;
            }
        }

        xfer.xfer_unsigned_int(&mut self.temporary_state_frame_end)
            .map_err(|e| format!("Failed to xfer temporary_state_frame_end: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("Failed to load_post_process AIStateMachine base: {}", e))
    }
}

// AI State implementations

/// Idle state - do nothing
/// Matches C++ AIIdleState from AIStates.cpp lines 1246-1448
#[derive(Debug)]
pub struct AIIdleState {
    base: State,
    /// Random offset for idle sleep to avoid spikes
    initial_sleep_offset: u16,
    /// Whether to look for targets while idle
    should_look_for_targets: bool,
    /// Whether initialization has been done
    inited: bool,
}

impl AIIdleState {
    /// Create new idle state
    /// C++ constructor from AIStates.cpp line 1249
    pub fn new(machine: &StateMachine, should_look_for_targets: bool) -> Self {
        Self {
            base: State::new(machine, "AIIdle"),
            initial_sleep_offset: 0,
            should_look_for_targets,
            inited: false,
        }
    }

    pub fn is_idle(&self) -> bool {
        true
    }

    /// Initialize idle state - C++ AIIdleState::doInitIdleState() from AIStates.cpp line 1311
    fn do_init_idle_state(&mut self) {
        // Only do initialization once (C++ line 1315)
        if !self.inited {
            return;
        }

        self.inited = false;

        // Object *obj = getMachineOwner();
        // AIUpdateInterface *ai = obj->getAI();
        // const Locomotor* loco = ai->getCurLocomotor();
        // Bool ultraAccurate = (loco != NULL && loco->isUltraAccurate());

        // If idle and doing ground movement, snap to pathfind grid (C++ line 1325)
        // This handles the case where a unit is stopped mid-movement
        // if (ai->isIdle() && ai->isDoingGroundMovement()) {
        //     Coord3D goalPos = *obj->getPosition();
        //     if (goalPos.x || goalPos.y || goalPos.z) {
        //         TheAI->pathfinder()->updateGoal(obj, &goalPos, obj->getLayer());
        //         if (!ultraAccurate && TheAI->pathfinder()->goalPosition(obj, &goalPos)) {
        //             if (TheGameLogic->getFrame() <= 1) {
        //                 obj->setPosition(&goalPos);
        //             } else {
        //                 ai->setFinalPosition(&goalPos);
        //             }
        //             TheAI->pathfinder()->updateGoal(obj, &goalPos, obj->getLayer());
        //         }
        //     }
        // }

        // ai->setLocomotorGoalNone();
        // ai->setCurrentVictim(NULL);
    }
}

impl StateImplementation for AIIdleState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIIdleState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // C++ AIIdleState::onEnter() from AIStates.cpp line 1290
        // Reset mood checking timers
        // Object *obj = getMachineOwner();
        // AIUpdateInterface *ai = obj->getAI();
        // if (ai) ai->resetNextMoodCheckTime();

        self.inited = true;

        // Randomize idle countdown to avoid spikes (C++ line 1304)
        // const IDLE_COUNTDOWN_DELAY = LOGICFRAMES_PER_SECOND * 2
        self.initial_sleep_offset = (rand::random::<u16>() % 60) as u16; // 60 frames ~= 2 seconds at 30fps

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.reset_next_mood_check_time();
                    }
                }
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // C++ AIIdleState::update() from AIStates.cpp line 1369

        // Do initialization on first update (C++ line 1373)
        self.do_init_idle_state();

        let mut time_to_sleep = 60u32 + self.initial_sleep_offset as u32; // IDLE_COUNTDOWN_DELAY
        let old_sleep_offset = self.initial_sleep_offset;
        self.initial_sleep_offset = 0;

        // Check if we should look for targets (C++ line 1381)
        if self.should_look_for_targets {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(machine_guard) = machine.lock() {
                    if machine_guard.is_locked() {
                        return Ok(StateReturnType::Sleep(time_to_sleep));
                    }
                }
            }

            // Object *obj = getMachineOwner();
            // AIUpdateInterface *ai = obj->getAI();

            // Repulsor logic (C++ line 1388)
            // if (obj->isKindOf(KINDOF_CAN_BE_REPULSED) && ai->isIdle())
            // {
            //     Object* enemy = TheAI->findClosestRepulsor(obj, obj->getVisionRange());
            //     if (enemy) {
            //         getMachine()->setState(AI_MOVE_AWAY_FROM_REPULSORS);
            //         return Ok(StateReturnType::Continue);
            //     }
            // }

            // Check for crate to pickup (C++ line 1399)
            // Object* crate = ai->checkForCrateToPickup();
            // if (crate) {
            //     ai->aiMoveToObject(crate, CMD_FROM_AI);
            //     return Ok(StateReturnType::Continue);
            // }

            // Mood targeting - attack enemies based on mood settings (C++ line 1415)
            // if not disabled by paralysis/emp/etc
            // {
            //     UnsignedInt moodAdjust = ai->getMoodMatrixActionAdjustment(MM_Action_Idle);
            //     if ((moodAdjust & MAA_Affect_Range_IgnoreAll) == 0)
            //     {
            //         Object* enemy = ai->getNextMoodTarget(true, true);
            //         if (enemy) {
            //             ai->aiAttackObject(enemy, NO_MAX_SHOTS_LIMIT, CMD_FROM_AI);
            //             return Ok(StateReturnType::Continue);
            //         }
            //     }
            // }
            if let Some(owner) = self.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            if owner_guard.is_kind_of(KindOf::CanBeRepulsed) && ai_guard.is_idle() {
                                let enemy = THE_AI
                                    .read()
                                    .ok()
                                    .and_then(|ai| {
                                        ai.find_closest_repulsor(
                                            owner_guard.get_id(),
                                            owner_guard.get_vision_range(),
                                        )
                                        .ok()
                                        .flatten()
                                    })
                                    .and_then(get_legacy_object);
                                if enemy.is_some() {
                                    if let Ok(machine) = self.base.get_machine() {
                                        machine.lock().ok().map(|mut guard| {
                                            guard.set_current_state(
                                                AIStateType::MoveAwayFromRepulsors.into(),
                                            );
                                        });
                                    }
                                    return Ok(StateReturnType::Continue);
                                }
                            }

                            if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                                let crate_id = crate_obj.read().ok().map(|c| c.get_id());
                                if let Some(crate_id) = crate_id {
                                    ai.ai_move_to_object(crate_id, CommandSourceType::FromAi);
                                    return Ok(StateReturnType::Continue);
                                }
                            }

                            if !owner_guard.is_disabled_by_type(DisabledType::Paralyzed)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledUnmanned)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledEmp)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledSubdued)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledHacked)
                            {
                                let mood_adjust = ai_guard
                                    .get_mood_matrix_action_adjustment(MoodMatrixAction::Idle);
                                if (mood_adjust & mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL)
                                    == 0
                                {
                                    if let Some(enemy) = ai_guard.get_next_mood_target(true, true) {
                                        ai.ai_attack_object(
                                            &enemy,
                                            NO_MAX_SHOTS_LIMIT,
                                            CommandSourceType::FromAi,
                                        );
                                        return Ok(StateReturnType::Continue);
                                    }
                                }
                            }

                            let now = TheGameLogic::get_frame();
                            let next_mood_check = ai_guard.get_next_mood_check_time();
                            if next_mood_check > now {
                                let mood_sleep = next_mood_check - now;
                                if mood_sleep < time_to_sleep {
                                    time_to_sleep = mood_sleep;
                                    self.initial_sleep_offset = old_sleep_offset;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sleep until next check (C++ line 1446)
        // STATE_SLEEP(timeToSleep) macro
        Ok(StateReturnType::Sleep(time_to_sleep))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Idle state has no cleanup
        Ok(())
    }

    fn classic_is_idle(&self) -> bool {
        true
    }
}

/// Move away from repulsors state
/// Matches C++ AIMoveAwayFromRepulsorsState from AIStates.cpp lines 2263-2312.
#[derive(Debug)]
pub struct AIMoveAwayFromRepulsorsState {
    base: AIMoveToState,
    goal_position: Coord3D,
    ok_to_repath_times: i32,
    check_for_path: bool,
}

impl AIMoveAwayFromRepulsorsState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveAwayFromRepulsors".to_string();
        Self {
            base,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            ok_to_repath_times: 1,
            check_for_path: true,
        }
    }
}

/// Wander around a point
/// Matches C++ AIWanderInPlaceState from AIStates.cpp lines 4617-4714.
#[derive(Debug)]
pub struct AIWanderInPlaceState {
    base: AIMoveToState,
    origin: Coord3D,
    goal_position: Coord3D,
    wait_frames: i32,
    timer: i32,
}

impl AIWanderInPlaceState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIWanderInPlace".to_string();
        Self {
            base,
            origin: Coord3D::new(0.0, 0.0, 0.0),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn choose_new_goal(&mut self, ai: &dyn AIUpdateInterface) {
        let mut delta = 3;
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                delta = ((locomotor_guard.template.wander_about_point_radius
                    / PATHFIND_CELL_SIZE_F)
                    + 0.5)
                    .floor() as i32;
            }
        }

        let offset_x = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        let offset_y = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        self.goal_position = self.origin;
        self.goal_position.x += offset_x;
        self.goal_position.y += offset_y;
    }
}

impl StateImplementation for AIWanderInPlaceState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIWanderInPlaceState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander in place missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander in place owner lock poisoned".to_string())?;
        self.origin = *owner_guard.get_position();

        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "wander in place missing AIUpdateInterface".to_string())?;
        if let Ok(mut ai_guard) = ai.lock() {
            let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Wander);
            self.choose_new_goal(&*ai_guard);
        }

        self.timer = 0;
        self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);

        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.set_goal_position(self.goal_position);
            }
        }

        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let status = self.base.classic_on_update()?;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander in place missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander in place owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "wander in place missing AIUpdateInterface".to_string())?;

        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(
                            owner_guard.get_id(),
                            owner_guard.get_vision_range(),
                        )
                        .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        if status != StateReturnType::Continue {
            if let Ok(ai_guard) = ai.lock() {
                self.choose_new_goal(&*ai_guard);
            }
            if let Ok(machine) = self.base.base.get_machine() {
                if let Ok(mut machine_guard) = machine.lock() {
                    machine_guard.set_goal_position(self.goal_position);
                }
            }
            let _ = self.base.classic_on_enter();
            return Ok(StateReturnType::Continue);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)
    }
}

/// Move out of the way state
/// Matches C++ AIMoveOutOfTheWayState from AIStates.cpp lines 2125-2168.
#[derive(Debug)]
pub struct AIMoveOutOfTheWayState {
    base: AIMoveToState,
}

impl AIMoveOutOfTheWayState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveOutOfTheWay".to_string();
        Self { base }
    }
}

impl StateImplementation for AIMoveOutOfTheWayState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIMoveOutOfTheWayState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(true);

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move out of the way missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "move out of the way owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "move out of the way missing AIUpdateInterface".to_string())?;
        let ai_guard = ai
            .lock()
            .map_err(|_| "move out of the way AI lock poisoned".to_string())?;
        let goal = ai_guard
            .get_path_destination()
            .ok_or_else(|| "move out of the way missing path destination".to_string())?;
        drop(ai_guard);

        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(goal);
            }
        }

        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move out of the way missing owner".to_string())?;
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_effectively_dead() {
                return Ok(StateReturnType::Success);
            }

            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.is_blocked_and_stuck() {
                        let _ = ai_guard.set_can_path_through_units(true);
                    }
                }
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.destroy_path();
                        let _ = ai_guard.set_can_path_through_units(false);
                        ai_guard.clear_move_out_of_way();
                    }
                }
            }
        }
        Ok(())
    }
}

/// Move and tighten state
/// Matches C++ AIMoveAndTightenState from AIStates.cpp lines 2181-2250.
#[derive(Debug)]
pub struct AIMoveAndTightenState {
    base: AIMoveToState,
    ok_to_repath_times: i32,
    check_for_path: bool,
}

impl AIMoveAndTightenState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveAndTighten".to_string();
        Self {
            base,
            ok_to_repath_times: 1,
            check_for_path: true,
        }
    }
}

impl StateImplementation for AIMoveAndTightenState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIMoveAndTightenState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(false);
        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.base.set_repath_limit(1, true);
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.check_for_path {
            if let Some(owner) = self.base.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.lock() {
                            if ai_guard.get_path_destination().is_some()
                                && !ai_guard.is_waiting_for_path()
                            {
                                self.base.set_adjusts_destination(true);
                                self.check_for_path = false;
                            }
                        }
                    }
                }
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)
    }
}

/// Follow path state
/// Matches C++ AIFollowPathState from AIStates.cpp lines 3229-3389.
#[derive(Debug)]
pub struct AIFollowPathState {
    base: AIMoveToState,
    path: Vec<Coord3D>,
    index: usize,
    adjust_final: bool,
    adjust_final_override: bool,
    retry_count: i32,
    follow_exit_production: bool,
    ignore_object_id: Option<ObjectID>,
}

impl AIFollowPathState {
    pub fn new(machine: &StateMachine, follow_exit_production: bool) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = if follow_exit_production {
            "AIFollowExitProductionPath".to_string()
        } else {
            "AIFollowPath".to_string()
        };
        Self {
            base,
            path: Vec::new(),
            index: 0,
            adjust_final: true,
            adjust_final_override: true,
            retry_count: 0,
            follow_exit_production,
            ignore_object_id: None,
        }
    }

    pub fn set_path(&mut self, path: Vec<Coord3D>) {
        self.path = path;
        self.index = 0;
    }

    pub fn append_path(&mut self, pos: Coord3D) {
        self.path.push(pos);
    }

    pub fn set_ignore_object_id(&mut self, object_id: Option<ObjectID>) {
        self.ignore_object_id = object_id;
    }

    fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position = pos;
        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(pos);
            }
        }
    }

    fn configure_segment(
        &mut self,
        owner_guard: &Object,
        ai_guard: &mut dyn AIUpdateInterface,
        allow_adjust: bool,
    ) -> Result<(), String> {
        let next_pos = self.path.get(self.index + 1).copied();
        if let Some(next) = next_pos {
            let dx = next.x - self.base.goal_position.x;
            let dy = next.y - self.base.goal_position.y;
            let mut offset = (dx * dx + dy * dy).sqrt();
            if self.path.get(self.index + 2).is_some() {
                offset += 4.0 * PATHFIND_CELL_SIZE_F;
            }
            ai_guard
                .set_path_extra_distance(offset)
                .map_err(|e| format!("follow path set_path_extra_distance failed: {}", e))?;
            self.base.set_adjusts_destination(false);
        } else {
            let adjust_final = self.adjust_final
                && (self.adjust_final_override || ai_guard.is_doing_ground_movement());
            self.base.set_adjusts_destination(adjust_final);
            let _ = ai_guard.set_path_extra_distance(0.0);
            if allow_adjust && self.base.adjust_destinations {
                let mut adjusted_goal = self.base.goal_position;
                if !ai_guard.adjust_destination(&mut adjusted_goal) {
                    return Err("follow path failed to adjust destination".to_string());
                }
                self.set_goal_position(adjusted_goal);
                let _ = ai_guard.update_goal_position(&adjusted_goal, PathfindLayerEnum::Ground);
            }
            if owner_guard.is_kind_of(KindOf::Projectile) {
                let _ = ai_guard.set_precise_z_pos(true);
            }
        }
        Ok(())
    }
}

impl StateImplementation for AIFollowPathState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowPathState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if self.path.is_empty() {
            return Ok(StateReturnType::Failure);
        }
        self.index = 0;
        self.adjust_final = true;
        self.adjust_final_override = true;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow path missing owner".to_string())?;
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "follow path owner lock poisoned".to_string())?;
            let ai = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow path missing AIUpdateInterface".to_string())?;
            let mut ai_guard = ai
                .lock()
                .map_err(|_| "follow path AI lock poisoned".to_string())?;

            self.set_goal_position(self.path[0]);
            if let Some(ignore_id) = self.ignore_object_id {
                if let Some(ignore_obj) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(ignore_id)
                {
                    let _ = ai_guard.ignore_obstacle(Some(&ignore_obj));
                }
            }
            let _ = ai_guard.set_current_goal_path_index(self.index as i32);
            if self.follow_exit_production {
                let _ = ai_guard.set_can_path_through_units(true);
                self.base.set_adjusts_destination(false);
            }
        }

        let status = self.base.classic_on_enter()?;
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.get_formation_id() != FormationID::NONE {
                if let Some(group_id) = owner_guard.get_group_id() {
                    if let Ok(ai_lock) = THE_AI.read() {
                        if let Some(group) = ai_lock.find_group(group_id) {
                            if let Ok(mut group_guard) = group.write() {
                                if let Some(ai) = owner_guard.get_ai_update_interface() {
                                    if let Ok(mut ai_guard) = ai.lock() {
                                        ai_guard.set_desired_speed(group_guard.get_speed());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    self.configure_segment(&owner_guard, &mut *ai_guard, false)?;
                }
            }
        }
        Ok(status)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(self.base.goal_position);
            }
        }
        let status = self.base.classic_on_update()?;

        if status == StateReturnType::Continue {
            return Ok(status);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow path missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "follow path owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow path AI lock poisoned".to_string())?;

        if status == StateReturnType::Failure && self.retry_count > 0 {
            self.retry_count -= 1;
        } else {
            self.index = self.index.saturating_add(1);
        }

        while self.index < self.path.len() {
            let pos = self.path[self.index];
            let dx = pos.x - owner_guard.get_position().x;
            let dy = pos.y - owner_guard.get_position().y;
            if dx * dx + dy * dy >= PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
                break;
            }
            self.index = self.index.saturating_add(1);
        }

        let Some(pos) = self.path.get(self.index).copied() else {
            return Ok(StateReturnType::Success);
        };

        let _ = ai_guard.set_current_goal_path_index(self.index as i32);
        let _ = ai_guard.ignore_obstacle(None);
        ai_guard.friend_starting_move();

        self.set_goal_position(pos);
        self.configure_segment(&owner_guard, &mut *ai_guard, true)?;
        self.base.compute_path(&mut *ai_guard)?;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.set_precise_z_pos(false);
                        let _ = ai_guard.set_path_extra_distance(0.0);
                        let _ = ai_guard.set_current_goal_path_index(-1);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Follow exit-production path state
#[derive(Debug)]
pub struct AIFollowExitProductionPathState {
    base: AIFollowPathState,
}

impl AIFollowExitProductionPathState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: AIFollowPathState::new(machine, true),
        }
    }

    pub fn set_path(&mut self, path: Vec<Coord3D>) {
        self.base.set_path(path);
    }

    pub fn append_path(&mut self, pos: Coord3D) {
        self.base.append_path(pos);
    }
}

impl StateImplementation for AIFollowExitProductionPathState {
    fn on_enter(&mut self) -> StateReturnType {
        self.base.on_enter()
    }

    fn update(&mut self) -> StateReturnType {
        self.base.update()
    }

    fn on_exit(&mut self, status: StateExitType) {
        self.base.on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer_snapshot(xfer)
    }
}

impl ClassicState for AIFollowExitProductionPathState {
    fn base_state(&self) -> &State {
        self.base.base_state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.base_state_mut()
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer_snapshot(xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(exit)
    }
}

/// Move and delete state - move to a position then destroy self.
#[derive(Debug)]
pub struct AIMoveAndDeleteState {
    base: AIMoveToState,
}

impl AIMoveAndDeleteState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveAndDelete".to_string();
        Self { base }
    }
}

impl StateImplementation for AIMoveAndDeleteState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIMoveAndDeleteState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(true);
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let status = self.base.classic_on_update()?;
        if status != StateReturnType::Continue {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "move+delete missing owner".to_string())?;
            let owner_guard = owner
                .read()
                .map_err(|_| "move+delete owner lock poisoned".to_string())?;
            if owner_guard.is_effectively_dead() {
                return Ok(StateReturnType::Failure);
            }
            let owner_id = owner_guard.get_id();
            drop(owner_guard);
            let _ = TheGameLogic::destroy_object_by_id(owner_id);
        }
        Ok(status)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)
    }
}

/// Wait state - does nothing until interrupted.
#[derive(Debug)]
pub struct AIWaitState {
    base: State,
}

impl AIWaitState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIWait"),
        }
    }
}

impl StateImplementation for AIWaitState {
    fn on_enter(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {}
}

impl ClassicState for AIWaitState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Busy state - remain busy until AI reports idle.
#[derive(Debug)]
pub struct AIBusyState {
    base: State,
}

impl AIBusyState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIBusy"),
        }
    }
}

impl StateImplementation for AIBusyState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn is_busy(&self) -> bool {
        true
    }
}

impl ClassicState for AIBusyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "busy missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "busy owner lock poisoned".to_string())?;
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let params = AiCommandParams::new(AiCommandType::Busy, CommandSourceType::FromAi);
                let _ = ai_guard.execute_command(&params);
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "busy missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "busy owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "busy missing AIUpdateInterface".to_string())?;
        let ai_guard = ai.lock().map_err(|_| "busy AI lock poisoned".to_string())?;
        if ai_guard.is_idle() {
            Ok(StateReturnType::Success)
        } else {
            Ok(StateReturnType::Continue)
        }
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Wander along a waypoint path.
#[derive(Debug)]
pub struct AIWanderState {
    base: State,
    core: FollowWaypointPathCore,
    wait_frames: i32,
    timer: i32,
}

impl AIWanderState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIWander"),
            core: FollowWaypointPathCore::new(false, true),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn update_group_offset(&mut self, ai: &dyn AIUpdateInterface) {
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                let factor = locomotor_guard.template.wander_width_factor;
                if factor > 0.0 {
                    let mut delta = (factor + 0.5).floor() as i32;
                    if delta < 1 {
                        delta = 1;
                    }
                    let x =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    let y =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    self.core.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl StateImplementation for AIWanderState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIWanderState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "wander missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "wander AI lock poisoned".to_string())?;

        if self.core.current_waypoint.is_none() {
            return Ok(StateReturnType::Failure);
        }

        self.update_group_offset(&*ai_guard);
        self.timer = 0;
        self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);

        self.core
            .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
        self.core.compute_path(&mut *ai_guard)?;
        ai_guard
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .map_err(|e| format!("wander set_path_extra_distance failed: {}", e))?;

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "wander missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "wander AI lock poisoned".to_string())?;

        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(
                            owner_guard.get_id(),
                            owner_guard.get_vision_range(),
                        )
                        .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);
        let status = if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        };

        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Success);
            }
            self.update_group_offset(&*ai_guard);
            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            self.core.compute_path(&mut *ai_guard)?;
            return Ok(StateReturnType::Continue);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Panic state - wander while panicking.
#[derive(Debug)]
pub struct AIPanicState {
    base: State,
    core: FollowWaypointPathCore,
    wait_frames: i32,
    timer: i32,
}

impl AIPanicState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIPanic"),
            core: FollowWaypointPathCore::new(false, true),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn update_group_offset(&mut self, ai: &dyn AIUpdateInterface) {
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                let factor = locomotor_guard.template.wander_width_factor;
                if factor > 0.0 {
                    let mut delta = (factor + 0.5).floor() as i32;
                    if delta < 1 {
                        delta = 1;
                    }
                    let x =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    let y =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    self.core.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl StateImplementation for AIPanicState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIPanicState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "panic missing owner".to_string())?;
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "panic owner lock poisoned".to_string())?;
            let ai = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "panic missing AIUpdateInterface".to_string())?;
            let mut ai_guard = ai
                .lock()
                .map_err(|_| "panic AI lock poisoned".to_string())?;

            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Failure);
            }

            self.update_group_offset(&*ai_guard);
            self.timer = 0;
            self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);

            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            self.core.compute_path(&mut *ai_guard)?;
            ai_guard
                .set_path_extra_distance(self.core.calc_extra_path_distance())
                .map_err(|e| format!("panic set_path_extra_distance failed: {}", e))?;
        }

        if let Ok(mut owner_write) = owner.write() {
            owner_write.set_model_condition_state(ModelConditionFlags::PANICKING);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "panic missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "panic owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "panic missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "panic AI lock poisoned".to_string())?;

        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(
                            owner_guard.get_id(),
                            owner_guard.get_vision_range(),
                        )
                        .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);
        let status = if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        };

        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Success);
            }
            self.update_group_offset(&*ai_guard);
            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            self.core.compute_path(&mut *ai_guard)?;
            return Ok(StateReturnType::Continue);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::PANICKING);
            }
        }
        Ok(())
    }
}

/// Face object state
#[derive(Debug)]
pub struct AIFaceObjectState {
    base: State,
}

impl AIFaceObjectState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFaceObject"),
        }
    }
}

impl StateImplementation for AIFaceObjectState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIFaceObjectState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "face object missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "face object owner lock poisoned".to_string())?;
        let goal = self
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "face object missing goal".to_string())?;
        let goal_guard = goal
            .read()
            .map_err(|_| "face object goal lock poisoned".to_string())?;
        let dx = goal_guard.get_position().x - owner_guard.get_position().x;
        let dy = goal_guard.get_position().y - owner_guard.get_position().y;
        let angle = dy.atan2(dx);
        drop(goal_guard);
        drop(owner_guard);
        if let Ok(mut owner_write) = owner.write() {
            let _ = owner_write.set_orientation(angle);
        }
        Ok(StateReturnType::Success)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Face position state
#[derive(Debug)]
pub struct AIFacePositionState {
    base: State,
}

impl AIFacePositionState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFacePosition"),
        }
    }
}

impl StateImplementation for AIFacePositionState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIFacePositionState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "face position missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "face position owner lock poisoned".to_string())?;
        let goal = self
            .base
            .get_machine_goal_position()
            .ok_or_else(|| "face position missing goal position".to_string())?;
        let dx = goal.x - owner_guard.get_position().x;
        let dy = goal.y - owner_guard.get_position().y;
        let angle = dy.atan2(dx);
        drop(owner_guard);
        if let Ok(mut owner_write) = owner.write() {
            let _ = owner_write.set_orientation(angle);
        }
        Ok(StateReturnType::Success)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Hack internet state
#[derive(Debug)]
pub struct AIHackInternetState {
    base: State,
}

impl AIHackInternetState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIHackInternet"),
        }
    }
}

impl StateImplementation for AIHackInternetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIHackInternetState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "hack internet missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "hack internet owner lock poisoned".to_string())?;
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let params =
                    AiCommandParams::new(AiCommandType::HackInternet, CommandSourceType::FromAi);
                let _ = ai_guard.execute_command(&params);
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "hack internet missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "hack internet owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "hack internet missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "hack internet AI lock poisoned".to_string())?;
        let Some(hack) = ai_guard.get_hack_internet_ai_update_interface() else {
            return Ok(StateReturnType::Success);
        };
        if hack.is_hacking_packing_or_unpacking() {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Rappel into state (simplified; triggers AI command).
#[derive(Debug)]
pub struct AIRappelIntoState {
    base: State,
    issued_command: bool,
}

impl AIRappelIntoState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIRappelInto"),
            issued_command: false,
        }
    }
}

impl StateImplementation for AIRappelIntoState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIRappelIntoState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "rappel missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "rappel owner lock poisoned".to_string())?;
        if !owner_guard.is_kind_of(KindOf::CanRappel) {
            return Ok(StateReturnType::Failure);
        }
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "rappel missing AIUpdateInterface".to_string())?;
        drop(owner_guard);

        let mut ai_guard = ai
            .lock()
            .map_err(|_| "rappel AI lock poisoned".to_string())?;
        let mut params = AiCommandParams::new(AiCommandType::RappelInto, CommandSourceType::FromAi);
        if let Some(goal) = self.base.get_machine_goal_object() {
            params.obj = Some(
                goal.read()
                    .map_err(|_| "rappel goal lock poisoned".to_string())?
                    .get_id(),
            );
        }
        if let Some(goal_pos) = self.base.get_machine_goal_position() {
            params.pos = goal_pos;
        }
        let _ = ai_guard.execute_command(&params);
        self.issued_command = true;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if !self.issued_command {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "rappel missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "rappel owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "rappel missing AIUpdateInterface".to_string())?;
        let ai_guard = ai
            .lock()
            .map_err(|_| "rappel AI lock poisoned".to_string())?;
        if ai_guard.is_in_rappel_state() {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Combat drop state (simplified; triggers AI command).
#[derive(Debug)]
pub struct AICombatDropState {
    base: State,
    issued_command: bool,
}

impl AICombatDropState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AICombatDrop"),
            issued_command: false,
        }
    }
}

impl StateImplementation for AICombatDropState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AICombatDropState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "combat drop missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "combat drop owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "combat drop missing AIUpdateInterface".to_string())?;
        drop(owner_guard);
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "combat drop AI lock poisoned".to_string())?;
        let mut params = AiCommandParams::new(AiCommandType::CombatDrop, CommandSourceType::FromAi);
        if let Some(goal) = self.base.get_machine_goal_object() {
            params.obj = Some(
                goal.read()
                    .map_err(|_| "combat drop goal lock poisoned".to_string())?
                    .get_id(),
            );
        }
        if let Some(goal_pos) = self.base.get_machine_goal_position() {
            params.pos = goal_pos;
        }
        let _ = ai_guard.execute_command(&params);
        self.issued_command = true;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if !self.issued_command {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "combat drop missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "combat drop owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "combat drop missing AIUpdateInterface".to_string())?;
        let ai_guard = ai
            .lock()
            .map_err(|_| "combat drop AI lock poisoned".to_string())?;
        if ai_guard.is_doing_combat_drop() {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl StateImplementation for AIMoveAwayFromRepulsorsState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIMoveAwayFromRepulsorsState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(false);

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move away from repulsors missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "move away from repulsors owner lock poisoned".to_string())?;

        let enemy_id = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.find_closest_repulsor(owner_guard.get_id(), owner_guard.get_vision_range())
                    .ok()
            })
            .flatten()
            .ok_or_else(|| "move away from repulsors missing enemy".to_string())?;
        let enemy = get_legacy_object(enemy_id)
            .ok_or_else(|| "move away from repulsors missing enemy object".to_string())?;
        let enemy_guard = enemy
            .read()
            .map_err(|_| "move away from repulsors enemy lock poisoned".to_string())?;

        let mut has_safe_path = false;
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Panic);
                has_safe_path = ai_guard.request_safe_path(enemy_id).unwrap_or(false);
            }
        }

        if let Ok(mut owner_mut) = owner.write() {
            owner_mut.set_model_condition_state(ModelConditionFlags::PANICKING);
        }

        let owner_pos = *owner_guard.get_position();
        let enemy_pos = *enemy_guard.get_position();
        drop(enemy_guard);

        if has_safe_path {
            self.goal_position = owner_pos;
        } else {
            let mut dx = owner_pos.x - enemy_pos.x;
            let mut dy = owner_pos.y - enemy_pos.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                dx = 1.0;
                dy = 0.0;
            } else {
                dx /= len;
                dy /= len;
            }

            let flee_dist = owner_guard.get_vision_range();
            self.goal_position = Coord3D::new(
                owner_pos.x + dx * flee_dist,
                owner_pos.y + dy * flee_dist,
                owner_pos.z,
            );
        }

        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.set_goal_position(self.goal_position);
                machine_guard.set_goal_object(None);
            }
        }

        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.base.set_repath_limit(1, false);

        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if self.check_for_path
                            && !ai_guard.is_waiting_for_path()
                            && ai_guard.get_path_destination().is_some()
                        {
                            if let Some(dest) = ai_guard.get_path_destination() {
                                self.goal_position = dest;
                                if let Ok(machine) = self.base.base.get_machine() {
                                    if let Ok(mut machine_guard) = machine.lock() {
                                        machine_guard.set_goal_position(dest);
                                    }
                                }
                                self.base.set_adjusts_destination(false);
                                self.check_for_path = false;
                            }
                        }
                    }
                }
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::PANICKING);
            }
        }
        Ok(())
    }
}

/// Move to state - move to a specific position or object
/// Matches C++ AIMoveToState from AIStates.cpp lines 1992-2115
#[derive(Debug)]
pub struct AIMoveToState {
    base: State,
    /// Goal position to move to
    goal_position: Coord3D,
    /// Last path goal position (for detecting if goal moved)
    path_goal_position: Coord3D,
    /// Timestamp when path was computed
    path_timestamp: u32,
    /// Timestamp when blocked repath occurred
    blocked_repath_timestamp: u32,
    /// Whether to adjust destinations for pathfinding
    adjust_destinations: bool,
    /// Optional override for adjust-destination behavior (used by Enter/Exit)
    adjust_destinations_override: Option<bool>,
    /// Whether waiting for pathfinder
    waiting_for_path: bool,
    /// Whether we can try one more repath
    try_one_more_repath: bool,
    /// Goal layer for movement
    goal_layer: u8, // PathfindLayerEnum
    /// Whether this is truly a MoveTo (vs child class like AttackMove)
    is_move_to: bool,
    /// Handle for looping move sound
    ambient_playing_handle: u32,
    /// Optional repath limiter for derived states.
    repath_limit: Option<RepathLimit>,
}

const MIN_REPATH_TIME: u32 = 10;

#[derive(Debug, Clone, Copy)]
struct RepathLimit {
    remaining: i32,
    blocked_only: bool,
}

impl AIMoveToState {
    /// Create new move to state
    /// C++ constructor from AIStates.cpp line 1992
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIMoveTo"),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            adjust_destinations: true,
            adjust_destinations_override: None,
            waiting_for_path: false,
            try_one_more_repath: true,
            goal_layer: 0, // LAYER_GROUND
            is_move_to: true,
            ambient_playing_handle: 0,
            repath_limit: None,
        }
    }

    pub fn set_adjusts_destination(&mut self, adjust: bool) {
        self.adjust_destinations_override = Some(adjust);
        self.adjust_destinations = adjust;
    }

    pub fn set_repath_limit(&mut self, remaining: i32, blocked_only: bool) {
        self.repath_limit = Some(RepathLimit {
            remaining,
            blocked_only,
        });
    }

    pub fn clear_repath_limit(&mut self) {
        self.repath_limit = None;
    }

    /// Compute path to goal - C++ AIInternalMoveToState::computePath() from AIStates.cpp line 1577
    fn compute_path(&mut self, ai: &mut dyn AIUpdateInterface) -> Result<(), String> {
        self.waiting_for_path = false;
        ai.set_adjusts_destination(self.adjust_destinations);
        ai.set_movement_target(&self.goal_position)
            .map_err(|err| format!("AIMoveToState set_movement_target failed: {}", err))?;
        self.path_goal_position = self.goal_position;
        self.path_timestamp = TheGameLogic::get_frame();
        Ok(())
    }

    /// Force repath by resetting path state
    fn force_repath(&mut self) {
        self.path_goal_position = Coord3D::new(-100.0, -100.0, -100.0);
        self.path_timestamp = 0;
    }

    /// Check if position has changed enough to require repath
    /// C++ isSamePosition() from AIStates.cpp line 183
    fn is_same_position(
        &self,
        our_pos: &Coord3D,
        prev_target_pos: &Coord3D,
        cur_target_pos: &Coord3D,
    ) -> bool {
        // Calculate difference
        let diff_x = cur_target_pos.x - prev_target_pos.x;
        let diff_y = cur_target_pos.y - prev_target_pos.y;

        // Calculate distance to target
        let to_target_x = cur_target_pos.x - our_pos.x;
        let to_target_y = cur_target_pos.y - our_pos.y;

        // Tolerance is (dist/10)^2
        const TOLERANCE_FACTOR: f32 = 1.0 / (10.0 * 10.0);
        let tolerance_sqr =
            (to_target_x * to_target_x + to_target_y * to_target_y) * TOLERANCE_FACTOR;

        // Check if moved beyond tolerance
        diff_x * diff_x + diff_y * diff_y <= tolerance_sqr
    }
}

impl StateImplementation for AIMoveToState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIMoveToState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // C++ AIMoveToState::onEnter() from AIStates.cpp line 1999

        self.adjust_destinations = self.adjust_destinations_override.unwrap_or(true);
        self.ambient_playing_handle = 0;

        // If we have a goal object, move to it, otherwise move to goal position (C++ line 2022)
        if let Some(goal_obj) = self.base.get_machine_goal_object() {
            let goal_guard = goal_obj
                .read()
                .map_err(|_| "goal object lock poisoned".to_string())?;
            self.goal_position = *goal_guard.get_position();
            if let Some(owner) = self.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if owner_guard.is_kind_of(KindOf::Projectile) {
                        let half_height = goal_guard
                            .get_geometry_info()
                            .get_max_height_above_position()
                            * 0.5;
                        self.goal_position.z += half_height;
                        if goal_guard.get_position().z < self.goal_position.z {
                            self.goal_position.z += half_height;
                        }
                    }
                }
            }
        } else if let Some(goal_pos) = self.base.get_machine_goal_position() {
            self.goal_position = goal_pos;
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "AIMoveToState missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "AIMoveToState owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "AIMoveToState missing AIUpdateInterface".to_string())?;
        owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        if is_cliff_at(owner_guard.get_position()) {
            owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        }
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIMoveToState AI lock poisoned".to_string())?;

        if owner_guard.test_status(ObjectStatusTypes::Parachuting) {
            self.adjust_destinations = false;
        } else if !ai_guard.is_allowed_to_adjust_destination() {
            self.adjust_destinations = false;
        }

        ai_guard.set_adjusts_destination(self.adjust_destinations);
        self.compute_path(&mut *ai_guard)?;
        let _ = ai_guard.set_path_extra_distance(0.0);
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);
        self.start_move_sound(&owner_guard);

        if owner_guard.get_formation_id() != FormationID::NONE {
            if let Some(group_id) = owner_guard.get_group_id() {
                if let Ok(ai_lock) = THE_AI.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            let speed = group_guard.get_speed();
                            ai_guard.set_desired_speed(speed);
                        }
                    }
                }
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // C++ AIMoveToState::update() from AIStates.cpp line 2052

        // Update goal position if tracking an object (C++ line 2068)
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "AIMoveToState missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "AIMoveToState owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "AIMoveToState missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIMoveToState AI lock poisoned".to_string())?;

        let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
        if self.is_move_to && (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
            ai.ai_attack_move_to_position(
                &self.goal_position,
                NO_MAX_SHOTS_LIMIT,
                CommandSourceType::FromAi,
            );
        }

        let mut goal_moved = false;
        if let Some(goal_obj) = self.base.get_machine_goal_object() {
            let goal_guard = goal_obj
                .read()
                .map_err(|_| "goal object lock poisoned".to_string())?;
            let mut new_goal = *goal_guard.get_position();
            if owner_guard.is_kind_of(KindOf::Projectile) {
                let half_height = goal_guard
                    .get_geometry_info()
                    .get_max_height_above_position()
                    * 0.5;
                new_goal.z += half_height;
                if goal_guard.get_position().z < new_goal.z {
                    new_goal.z += half_height;
                }
            }
            self.goal_position = new_goal;
            if !self.is_same_position(
                owner_guard.get_position(),
                &self.path_goal_position,
                &new_goal,
            ) {
                goal_moved = true;
            }
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        } else {
            let frames_blocked = ai_guard.get_num_frames_blocked();
            let mut set_condition_flag = ModelConditionFlags::MOVING;
            if is_cliff_at(owner_guard.get_position()) {
                let moving_backwards = ai_guard
                    .get_cur_locomotor()
                    .and_then(|loc| loc.lock().ok().map(|loco| loco.is_moving_backwards()))
                    .unwrap_or(false);
                set_condition_flag = if moving_backwards {
                    ModelConditionFlags::RAPPELLING
                } else {
                    ModelConditionFlags::CLIMBING
                };
            }

            if frames_blocked > LOGICFRAMES_PER_SECOND / 4 {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
            } else {
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                if set_condition_flag == ModelConditionFlags::MOVING {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                    owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                } else {
                    let clear_flag = if set_condition_flag == ModelConditionFlags::CLIMBING {
                        ModelConditionFlags::RAPPELLING
                    } else {
                        ModelConditionFlags::CLIMBING
                    };
                    owner_guard.clear_model_condition_state(clear_flag);
                    owner_guard.set_model_condition_state(set_condition_flag);
                }
            }
        }
        let now = TheGameLogic::get_frame();
        let should_repath =
            blocked || (goal_moved && now.saturating_sub(self.path_timestamp) > MIN_REPATH_TIME);

        if should_repath {
            if let Some(limit) = self.repath_limit.as_mut() {
                if limit.blocked_only && !blocked {
                    // Do not repath when only blocked repaths are allowed.
                } else {
                    if limit.remaining <= 0 {
                        return Ok(StateReturnType::Failure);
                    }
                    limit.remaining -= 1;
                    self.compute_path(&mut *ai_guard)?;
                }
            } else {
                self.compute_path(&mut *ai_guard)?;
            }
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);
        if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn start_move_sound(&mut self, owner_guard: &Object) {
        let mut use_damaged = false;
        if let Some(body) = owner_guard.get_body_module() {
            if let Ok(body_guard) = body.lock() {
                use_damaged = body_guard.get_damage_state() > BodyDamageType::Damaged;
            }
        }

        let template = owner_guard.get_template();
        let mut start_sound = if use_damaged {
            template.get_sound_move_start_damaged()
        } else {
            template.get_sound_move_start()
        };
        let mut loop_sound = if use_damaged {
            template.get_sound_move_loop_damaged()
        } else {
            template.get_sound_move_loop()
        };

        if start_sound.get_event_name().is_empty() {
            start_sound = loop_sound.clone();
        }

        if start_sound.get_event_name().is_empty() {
            return;
        }

        start_sound.set_object_id(owner_guard.get_id());
        if let Some(audio) = TheAudio::get() {
            if start_sound.get_event_name() == loop_sound.get_event_name()
                && !loop_sound.get_event_name().is_empty()
            {
                let handle = audio.add_audio_event(&start_sound);
                self.ambient_playing_handle = handle;
            } else {
                audio.add_audio_event(&start_sound);
            }
        }
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // C++ AIMoveToState::onExit() from AIStates.cpp line 2046
        if self.ambient_playing_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.ambient_playing_handle);
            }
            self.ambient_playing_handle = 0;
        }
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                            if let Ok(loco_guard) = locomotor.lock() {
                                if loco_guard.is_ultra_accurate()
                                    && !matches!(
                                        loco_guard.get_appearance(),
                                        LocomotorAppearance::Hover
                                            | LocomotorAppearance::Thrust
                                            | LocomotorAppearance::Wings
                                            | LocomotorAppearance::Naval
                                    )
                                {
                                    let dx = self.goal_position.x - owner_guard.get_position().x;
                                    let dy = self.goal_position.y - owner_guard.get_position().y;
                                    if dx * dx + dy * dy
                                        < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F
                                    {
                                        let _ = owner_guard.set_position(&self.goal_position);
                                    }
                                }
                            }
                        }
                        ai_guard.destroy_path();
                    }
                }
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        // Moving units are busy
        true
    }
}

/// Follow waypoint path as team
#[derive(Debug)]
pub struct AIFollowWaypointPathAsTeamState {
    base: State,
    core: FollowWaypointPathCore,
}

impl AIFollowWaypointPathAsTeamState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsTeam"),
            core: FollowWaypointPathCore::new(true, true),
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsTeamState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowWaypointPathAsTeamState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;

        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return Ok(StateReturnType::Failure);
        }

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(current.position);
            }
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        let mut speed = FAST_AS_POSSIBLE;
        if self.core.move_as_group {
            if self.core.current_waypoint.is_none() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(team) = team_arc.read() {
                        self.core.current_waypoint = team
                            .get_current_waypoint_id()
                            .and_then(resolve_waypoint_by_id);
                    }
                }
            }
            if let Some(current) = self.core.current_waypoint.as_ref() {
                if let Some(team) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team.write() {
                        team_guard.set_current_waypoint_id(Some(current.id));
                    }
                }
            }
            if let Some(group_id) = owner_guard.get_group_id() {
                if let Ok(ai_lock) = THE_AI.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            speed = group_guard.get_speed();
                            if let Some(center) = group_guard.get_center() {
                                let pos = owner_guard.get_position();
                                self.core.group_offset.x = pos.x - center.x;
                                self.core.group_offset.y = pos.y - center.y;
                            }
                        }
                    }
                }
            }
        }

        self.core.compute_goal(
            &self.base,
            &owner_guard,
            &mut *ai_guard,
            self.core.move_as_group,
        )?;
        if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
            if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                return Ok(StateReturnType::Failure);
            }
        }
        self.core.compute_path(&mut *ai_guard)?;
        ai_guard.set_desired_speed(speed);
        ai_guard
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .map_err(|e| e.to_string())?;
        if ai_guard.is_doing_ground_movement() {
            let _ = ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return Ok(StateReturnType::Continue);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(mut guard) = machine.lock() {
                    guard.set_goal_position(current.position);
                }
            }
        } else {
            return Ok(StateReturnType::Success);
        }

        if self.core.is_follow_waypoint_path_state {
            let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
            if (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
                if let Some(current) = self.core.current_waypoint.as_ref() {
                    if self.core.move_as_group {
                        ai.ai_attack_follow_waypoint_path_as_team(
                            current,
                            NO_MAX_SHOTS_LIMIT,
                            CommandSourceType::FromAi,
                        );
                    } else {
                        ai.ai_attack_follow_waypoint_path(
                            current,
                            NO_MAX_SHOTS_LIMIT,
                            CommandSourceType::FromAi,
                        );
                    }
                }
            }
        }

        if self.core.append_goal_position
            && !ai_guard.is_waiting_for_path()
            && ai.get_path().is_some()
        {
            ai_guard.append_goal_position_to_path(&self.core.goal_position)?;
            self.core.append_goal_position = false;
        }

        if self.core.move_as_group {
            if let Some(team) = owner_guard.get_team() {
                if let Ok(team_guard) = team.read() {
                    if team_guard.get_current_waypoint_id()
                        != self.core.current_waypoint.as_ref().map(|w| w.id)
                    {
                        self.core.prior_waypoint = self.core.current_waypoint.clone();
                        self.core.current_waypoint = team_guard
                            .get_current_waypoint_id()
                            .and_then(resolve_waypoint_by_id);
                        if self.core.current_waypoint.is_none() {
                            return Ok(StateReturnType::Success);
                        }
                        self.core
                            .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
                        if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
                            if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                                return Ok(StateReturnType::Failure);
                            }
                        }
                        ai_guard.friend_starting_move();
                        self.core.compute_path(&mut *ai_guard)?;
                        if ai_guard.is_doing_ground_movement() {
                            let _ = ai_guard.update_goal_position(
                                &self.core.goal_position,
                                self.core.goal_layer,
                            );
                        }
                    }
                }
            }
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            let _ = self.core.compute_path(&mut *ai_guard);
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);

        let mut status = StateReturnType::Continue;
        if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            status = StateReturnType::Success;
        }

        if self.core.move_as_group {
            if let Some(player) = owner_guard.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    if player_guard.is_skirmish_ai() {
                        if let Some(group_id) = owner_guard.get_group_id() {
                            if let Ok(ai_lock) = THE_AI.read() {
                                if let Some(group) = ai_lock.find_group(group_id) {
                                    if let Ok(group_guard) = group.read() {
                                        if let Some(center) = group_guard.get_center() {
                                            let dx = center.x - self.core.goal_position.x;
                                            let dy = center.y - self.core.goal_position.y;
                                            let dist = (dx * dx + dy * dy).sqrt();
                                            let num = group_guard.get_count() as f32;
                                            let fudge = ai_lock
                                                .get_ai_data()
                                                .read()
                                                .map(|d| d.skirmish_group_fudge_value)
                                                .unwrap_or(0.0);
                                            if dist <= num * fudge {
                                                status = StateReturnType::Success;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if status != StateReturnType::Continue {
            let prior_id = self.core.prior_waypoint.as_ref().map(|w| w.id);
            if let Some(prior) = prior_id {
                ai_guard.set_prior_waypoint_id(prior);
            }
            let next = self.core.get_next_waypoint(&self.base);
            self.core.current_waypoint = next.clone();
            if let Some(current) = next.as_ref() {
                ai_guard.set_current_waypoint_id(current.id);
            }

            if next.is_none() {
                ai_guard.set_completed_waypoint_id(prior_id);
                return Ok(StateReturnType::Success);
            }

            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
                if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                    return Ok(StateReturnType::Failure);
                }
            }
            ai_guard.friend_starting_move();
            self.core.compute_path(&mut *ai_guard)?;
            if ai_guard.is_doing_ground_movement() {
                let _ =
                    ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
            }
            if let Some(current) = self.core.current_waypoint.as_ref() {
                if self.core.move_as_group {
                    if let Some(team) = owner_guard.get_team() {
                        if let Ok(mut team_guard) = team.write() {
                            team_guard.set_current_waypoint_id(Some(current.id));
                        }
                    }
                }
            }
        }

        Ok(status)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(loco) = ai_guard.get_cur_locomotor() {
                            if let Ok(mut guard) = loco.lock() {
                                guard.set_precise_z_pos(false);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }

    fn classic_is_attack(&self) -> bool {
        false
    }
}

/// Follow waypoint path exact as team (no pathfinding, follow waypoint links exactly).
#[derive(Debug)]
pub struct AIFollowWaypointPathAsTeamExactState {
    base: State,
    move_as_group: bool,
    last_waypoint: Option<Arc<Waypoint>>,
}

impl AIFollowWaypointPathAsTeamExactState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsTeamExact"),
            move_as_group: true,
            last_waypoint: None,
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsTeamExactState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowWaypointPathAsTeamExactState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        let current = waypoint_id.and_then(resolve_waypoint_by_id);
        let current =
            current.ok_or_else(|| "follow waypoint exact missing waypoint".to_string())?;

        if let Ok(mut guard) = machine.lock() {
            guard.set_goal_position(current.position);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let mut speed = FAST_AS_POSSIBLE;
        let mut group_offset = Coord2D::new(0.0, 0.0);
        if self.move_as_group {
            if let Some(group_id) = owner_guard.get_group_id() {
                if let Ok(ai_lock) = THE_AI.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            speed = group_guard.get_speed();
                            if let Some(center) = group_guard.get_center() {
                                let pos = owner_guard.get_position();
                                group_offset.x = pos.x - center.x;
                                group_offset.y = pos.y - center.y;
                            }
                        }
                    }
                }
            }
        }

        let _ = ai_guard.set_can_path_through_units(true);
        ai_guard.set_adjusts_destination(false);
        ai_guard.set_path_from_waypoint(&current, &group_offset)?;
        let _ = ai_guard.set_allow_invalid_position(true);
        ai_guard.set_desired_speed(speed);

        self.last_waypoint = Some(current);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let _ = ai_guard.set_can_path_through_units(true);
        if !ai_guard.is_moving() && ai_guard.is_waypoint_queue_empty() {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(last) = self.last_waypoint.as_ref() {
                            ai_guard.set_completed_waypoint_id(Some(last.id));
                        }
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.set_allow_invalid_position(false);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Attack move-to state (attack while moving)
#[derive(Debug)]
pub struct AIAttackMoveToState {
    base: AIMoveToState,
    attack_move_machine: Option<AIAttackMoveStateMachine>,
    frame_to_sleep_until: UnsignedInt,
    retry_count: i32,
    command_src: CommandSourceType,
}

impl AIAttackMoveToState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackMoveTo".to_string();
        base.is_move_to = false;
        Self {
            base,
            attack_move_machine: None,
            frame_to_sleep_until: 0,
            retry_count: ATTACK_RETRY_COUNT,
            command_src: CommandSourceType::FromAi,
        }
    }
}

impl StateImplementation for AIAttackMoveToState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackMoveToState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let result = self.base.classic_on_enter()?;
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack move-to missing machine owner".to_string())?;
        let mut attack_machine =
            AIAttackMoveStateMachine::new(Arc::downgrade(&owner), "AIAttackMoveMachine");
        attack_machine.clear();
        let _ = attack_machine.set_state(AIStateType::Idle);
        self.attack_move_machine = Some(attack_machine);

        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    self.command_src = ai_guard.get_last_command_source();
                }
            }
        }
        self.retry_count = ATTACK_RETRY_COUNT;
        self.frame_to_sleep_until = 0;

        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack move-to missing machine owner".to_string())?;
        let ai = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .ok_or_else(|| "attack move-to missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack move-to AI lock poisoned".to_string())?;

        let mut force_retarget_this_frame = false;
        let mut should_repath_this_frame = false;

        if let Some(machine) = self.attack_move_machine.as_mut() {
            if !machine.is_in_idle_state() {
                ai_guard.set_locomotor_goal_none();
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
                let _ = machine.update();

                if machine.is_in_idle_state() {
                    force_retarget_this_frame = true;
                    should_repath_this_frame = true;
                    ai_guard.set_last_command_source(self.command_src);
                } else {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if let Some(machine) = self.attack_move_machine.as_mut() {
            if machine.is_in_idle_state() {
                if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                    machine.set_goal_object(Some(Arc::downgrade(&crate_obj)));
                    let _ = machine.set_state(AIStateType::PickUpCrate);
                    return Ok(StateReturnType::Continue);
                }

                if let Some(target) =
                    ai_guard.get_next_mood_target(!force_retarget_this_frame, false)
                {
                    ai_guard.friend_ending_move();
                    machine.set_goal_object(Some(Arc::downgrade(&target)));
                    let _ = machine.set_state(AIStateType::AttackObject);
                    ai_guard.set_last_command_source(CommandSourceType::FromAi);
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        let current_frame = TheGameLogic::get_frame();
        if self.frame_to_sleep_until > current_frame {
            return Ok(StateReturnType::Continue);
        } else if self.frame_to_sleep_until == current_frame {
            should_repath_this_frame = true;
        }

        if should_repath_this_frame {
            let _ = self.base.classic_on_enter();
            self.base.force_repath();
        }

        let mut ret = self.base.classic_on_update()?;
        if ret != StateReturnType::Continue {
            if self.retry_count < 1 {
                return Ok(ret);
            }
            if let Ok(owner_guard) = owner.read() {
                let dx = owner_guard.get_position().x - self.base.path_goal_position.x;
                let dy = owner_guard.get_position().y - self.base.path_goal_position.y;
                let dist_sqr = dx * dx + dy * dy;
                let close_enough =
                    (ATTACK_CLOSE_ENOUGH_CELLS as f32 * PATHFIND_CELL_SIZE_F).powi(2);
                if dist_sqr < close_enough {
                    return Ok(ret);
                }
            }

            ret = StateReturnType::Continue;
            self.retry_count -= 1;
            self.frame_to_sleep_until = current_frame + 3 * LOGICFRAMES_PER_SECOND;
        }

        Ok(ret)
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_move_machine.take() {
            let _ = machine.halt();
        }
        self.base.classic_on_exit(exit)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }

    fn classic_is_attack(&self) -> bool {
        self.attack_move_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

const ATTACK_RETRY_COUNT: i32 = 3;
const ATTACK_CLOSE_ENOUGH_CELLS: f32 = 4.0;

/// Attack follow waypoint path as team
#[derive(Debug)]
pub struct AIAttackFollowWaypointPathAsTeamState {
    base: AIFollowWaypointPathAsTeamState,
    attack_follow_machine: Option<AIAttackMoveStateMachine>,
}

impl AIAttackFollowWaypointPathAsTeamState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIFollowWaypointPathAsTeamState::new(machine);
        base.base.name = "AIAttackFollowWaypointPathAsTeam".to_string();
        base.core.is_follow_waypoint_path_state = false;
        Self {
            base,
            attack_follow_machine: None,
        }
    }
}

impl StateImplementation for AIAttackFollowWaypointPathAsTeamState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackFollowWaypointPathAsTeamState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let result = self.base.classic_on_enter()?;
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let mut attack_machine =
            AIAttackMoveStateMachine::new(Arc::downgrade(&owner), "AIAttackFollowMachine");
        attack_machine.clear();
        let _ = attack_machine.set_state(AIStateType::Idle);
        self.attack_follow_machine = Some(attack_machine);

        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let ai = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .ok_or_else(|| "attack follow path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack follow path AI lock poisoned".to_string())?;

        let mut force_retarget_this_frame = false;
        let mut should_repath_this_frame = false;

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if !machine.is_in_idle_state() {
                ai_guard.set_locomotor_goal_none();
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
                let _ = machine.update();
                if machine.is_in_idle_state() {
                    force_retarget_this_frame = true;
                    should_repath_this_frame = true;
                } else {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if machine.is_in_idle_state() {
                if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                    machine.set_goal_object(Some(Arc::downgrade(&crate_obj)));
                    let _ = machine.set_state(AIStateType::PickUpCrate);
                    return Ok(StateReturnType::Continue);
                }

                if let Some(target) =
                    ai_guard.get_next_mood_target(!force_retarget_this_frame, false)
                {
                    machine.set_goal_object(Some(Arc::downgrade(&target)));
                    let _ = machine.set_state(AIStateType::AttackObject);
                    should_repath_this_frame = false;
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if should_repath_this_frame {
            if let Ok(owner_guard) = owner.read() {
                self.base.core.compute_goal(
                    &self.base.base,
                    &owner_guard,
                    &mut *ai_guard,
                    self.base.core.move_as_group,
                )?;
                self.base.core.compute_path(&mut *ai_guard)?;
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_follow_machine.take() {
            let _ = machine.set_state(AIStateType::Idle);
            let _ = machine.halt();
        }
        self.base.classic_on_exit(exit)
    }
}

/// Attack follow waypoint path as individuals
#[derive(Debug)]
pub struct AIAttackFollowWaypointPathAsIndividualsState {
    base: AIFollowWaypointPathAsIndividualsState,
    attack_follow_machine: Option<AIAttackMoveStateMachine>,
}

impl AIAttackFollowWaypointPathAsIndividualsState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIFollowWaypointPathAsIndividualsState::new(machine);
        base.base.name = "AIAttackFollowWaypointPathAsIndividuals".to_string();
        base.core.is_follow_waypoint_path_state = false;
        Self {
            base,
            attack_follow_machine: None,
        }
    }
}

impl StateImplementation for AIAttackFollowWaypointPathAsIndividualsState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackFollowWaypointPathAsIndividualsState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let result = self.base.classic_on_enter()?;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let mut attack_machine =
            AIAttackMoveStateMachine::new(Arc::downgrade(&owner), "AIAttackFollowMachine");
        attack_machine.clear();
        let _ = attack_machine.set_state(AIStateType::Idle);
        self.attack_follow_machine = Some(attack_machine);

        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let ai = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .ok_or_else(|| "attack follow path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack follow path AI lock poisoned".to_string())?;

        let mut force_retarget_this_frame = false;
        let mut should_repath_this_frame = false;

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if !machine.is_in_idle_state() {
                ai_guard.set_locomotor_goal_none();
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
                let _ = machine.update();
                if machine.is_in_idle_state() {
                    force_retarget_this_frame = true;
                    should_repath_this_frame = true;
                } else {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if machine.is_in_idle_state() {
                if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                    machine.set_goal_object(Some(Arc::downgrade(&crate_obj)));
                    let _ = machine.set_state(AIStateType::PickUpCrate);
                    return Ok(StateReturnType::Continue);
                }

                if let Some(target) =
                    ai_guard.get_next_mood_target(!force_retarget_this_frame, false)
                {
                    machine.set_goal_object(Some(Arc::downgrade(&target)));
                    let _ = machine.set_state(AIStateType::AttackObject);
                    should_repath_this_frame = false;
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if should_repath_this_frame {
            if let Ok(owner_guard) = owner.read() {
                self.base.core.compute_goal(
                    &self.base.base,
                    &owner_guard,
                    &mut *ai_guard,
                    false,
                )?;
                self.base.core.compute_path(&mut *ai_guard)?;
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_follow_machine.take() {
            let _ = machine.set_state(AIStateType::Idle);
            let _ = machine.halt();
        }
        self.base.classic_on_exit(exit)
    }
}
/// Follow waypoint path as individuals
#[derive(Debug)]
pub struct AIFollowWaypointPathAsIndividualsState {
    base: State,
    core: FollowWaypointPathCore,
}

impl AIFollowWaypointPathAsIndividualsState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsIndividuals"),
            core: FollowWaypointPathCore::new(false, true),
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsIndividualsState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowWaypointPathAsIndividualsState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;

        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return Ok(StateReturnType::Failure);
        }

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(current.position);
            }
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        self.core
            .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
        if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
            if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                return Ok(StateReturnType::Failure);
            }
        }
        self.core.compute_path(&mut *ai_guard)?;
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);
        ai_guard
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .map_err(|e| e.to_string())?;
        if ai_guard.is_doing_ground_movement() {
            let _ = ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return Ok(StateReturnType::Continue);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(mut guard) = machine.lock() {
                    guard.set_goal_position(current.position);
                }
            }
        } else {
            return Ok(StateReturnType::Success);
        }

        if self.core.is_follow_waypoint_path_state {
            let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
            if (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
                if let Some(current) = self.core.current_waypoint.as_ref() {
                    ai.ai_attack_follow_waypoint_path(
                        current,
                        NO_MAX_SHOTS_LIMIT,
                        CommandSourceType::FromAi,
                    );
                }
            }
        }

        if self.core.append_goal_position
            && !ai_guard.is_waiting_for_path()
            && ai.get_path().is_some()
        {
            ai_guard.append_goal_position_to_path(&self.core.goal_position)?;
            self.core.append_goal_position = false;
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            let _ = self.core.compute_path(&mut *ai_guard);
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);

        let mut status = StateReturnType::Continue;
        if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            status = StateReturnType::Success;
        }

        if status != StateReturnType::Continue {
            let prior_id = self.core.prior_waypoint.as_ref().map(|w| w.id);
            if let Some(prior) = prior_id {
                ai_guard.set_prior_waypoint_id(prior);
            }
            let next = self.core.get_next_waypoint(&self.base);
            self.core.current_waypoint = next.clone();
            if let Some(current) = next.as_ref() {
                ai_guard.set_current_waypoint_id(current.id);
            }

            if next.is_none() {
                ai_guard.set_completed_waypoint_id(prior_id);
                return Ok(StateReturnType::Success);
            }

            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
                if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                    return Ok(StateReturnType::Failure);
                }
            }
            ai_guard.friend_starting_move();
            self.core.compute_path(&mut *ai_guard)?;
            if ai_guard.is_doing_ground_movement() {
                let _ =
                    ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
            }
        }

        Ok(status)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(loco) = ai_guard.get_cur_locomotor() {
                            if let Ok(mut guard) = loco.lock() {
                                guard.set_precise_z_pos(false);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }

    fn classic_is_attack(&self) -> bool {
        false
    }
}

/// Follow waypoint path exact as individuals (no pathfinding).
#[derive(Debug)]
pub struct AIFollowWaypointPathAsIndividualsExactState {
    base: State,
    move_as_group: bool,
    last_waypoint: Option<Arc<Waypoint>>,
}

impl AIFollowWaypointPathAsIndividualsExactState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsIndividualsExact"),
            move_as_group: false,
            last_waypoint: None,
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsIndividualsExactState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowWaypointPathAsIndividualsExactState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        let current = waypoint_id.and_then(resolve_waypoint_by_id);
        let current =
            current.ok_or_else(|| "follow waypoint exact missing waypoint".to_string())?;

        if let Ok(mut guard) = machine.lock() {
            guard.set_goal_position(current.position);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let group_offset = Coord2D::new(0.0, 0.0);
        let _ = ai_guard.set_can_path_through_units(true);
        ai_guard.set_adjusts_destination(false);
        ai_guard.set_path_from_waypoint(&current, &group_offset)?;
        let _ = ai_guard.set_allow_invalid_position(true);
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);

        self.last_waypoint = Some(current);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let _ = ai_guard.set_can_path_through_units(true);
        if !ai_guard.is_moving() && ai_guard.is_waypoint_queue_empty() {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(last) = self.last_waypoint.as_ref() {
                            ai_guard.set_completed_waypoint_id(Some(last.id));
                        }
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.set_allow_invalid_position(false);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Attack object state
#[derive(Debug)]
pub struct AIAttackObjectState {
    base: State,
    target: Option<Arc<RwLock<Object>>>,
    force_attack: Bool,
    follow_target: Bool,
    issued_attack: Bool,
    attack_machine: Option<AttackStateMachine>,
    original_victim_pos: Coord3D,
}

impl AIAttackObjectState {
    pub fn new(machine: &StateMachine, force_attack: Bool, follow_target: Bool) -> Self {
        Self {
            base: State::new(machine, "AIAttackObject"),
            target: None,
            force_attack,
            follow_target,
            issued_attack: false,
            attack_machine: None,
            original_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    pub fn is_attack(&self) -> bool {
        true
    }
}

impl StateImplementation for AIAttackObjectState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackObjectState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let target = self
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "attack object state missing goal object".to_string())?;
        self.target = Some(target.clone());
        if let Ok(target_guard) = target.read() {
            self.original_victim_pos = *target_guard.get_position();
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack object state missing machine owner".to_string())?;
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.set_original_victim_pos(Some(self.original_victim_pos));
                }
            }
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackMachine",
            self.follow_target,
            true,
            self.force_attack,
        );
        attack_machine.set_goal_object(Some(&target));
        attack_machine.set_goal_position(self.original_victim_pos);
        let _ = attack_machine.init_default_state();
        self.attack_machine = Some(attack_machine);
        self.issued_attack = true;

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(target) = &self.target else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(target_guard) = target.read() {
            if target_guard.is_effectively_dead() {
                if let Some(owner) = self.base.get_machine_owner() {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard.notify_victim_is_dead();
                            }
                        }
                    }
                }
                return Ok(StateReturnType::Success);
            }
        }

        if let Some(attack_machine) = self.attack_machine.as_mut() {
            return Ok(attack_machine.update());
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Stop attacking
        self.target = None;
        self.issued_attack = false;
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_original_victim_pos(None);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }

    fn classic_is_busy(&self) -> bool {
        self.target.is_some()
    }
}

/// Attack position state
#[derive(Debug)]
pub struct AIAttackPositionState {
    base: State,
    target_position: Coord3D,
    issued_attack: Bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIAttackPositionState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIAttackPosition"),
            target_position: Coord3D::new(0.0, 0.0, 0.0),
            issued_attack: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        true
    }
}

impl StateImplementation for AIAttackPositionState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackPositionState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Some(pos) = self.base.get_machine_goal_position() {
            self.target_position = pos;
        } else if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                self.target_position = *owner_guard.get_position();
            }
        }

        if let Some(owner) = self.base.get_machine_owner() {
            let mut attack_machine = AttackStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
                false,
                false,
                false,
            );
            attack_machine.set_goal_position(self.target_position);
            let _ = attack_machine.init_default_state();
            self.attack_machine = Some(attack_machine);
            self.issued_attack = true;
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            return Ok(attack_machine.update());
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Stop attacking
        self.issued_attack = false;
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

const ENEMY_SCAN_RATE: u32 = LOGICFRAMES_PER_SECOND;
const CRATE_PICKUP_RANGE_SQR: f32 = 100.0;

#[derive(Debug)]
pub struct AIAttackThenIdleStateMachine {
    base: StateMachine,
}

impl AIAttackThenIdleStateMachine {
    pub fn new(owner: Weak<RwLock<Object>>, name: &str) -> Self {
        let mut base = StateMachine::new(Some(owner), name);
        let attack_state = AIAttackObjectState::new(&base, false, false);
        let pickup_state = AIPickUpCrateState::new(&base);
        let idle_state = AIIdleState::new(&base, false);
        register_classic_state(
            &mut base,
            AIStateType::AttackObject as u32,
            attack_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );
        register_classic_state(
            &mut base,
            AIStateType::PickUpCrate as u32,
            pickup_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );
        register_classic_state(
            &mut base,
            AIStateType::Idle as u32,
            idle_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );
        Self { base }
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_goal_object(&mut self, obj: Option<&Arc<RwLock<Object>>>) {
        self.base
            .set_goal_object(obj.map(|value| Arc::downgrade(value)));
    }

    pub fn set_state(&mut self, state: AIStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn get_current_state_id(&self) -> Option<u32> {
        self.base.get_current_state_id()
    }

    pub fn update(&mut self) -> StateReturnType {
        self.base.update()
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }
}

#[derive(Debug)]
pub struct AIPickUpCrateState {
    base: AIMoveToState,
    delay_counter: i32,
    goal_position: Coord3D,
}

impl AIPickUpCrateState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackPickUpCrateState".to_string();
        Self {
            base,
            delay_counter: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIPickUpCrateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIPickUpCrateState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let goal = self
            .base
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "pick up crate missing goal object".to_string())?;

        if let Ok(goal_guard) = goal.read() {
            self.goal_position = *goal_guard.get_position();
        }
        self.delay_counter = 3;
        self.base.set_adjusts_destination(true);

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.delay_counter > 0 {
            self.delay_counter -= 1;
            if self.delay_counter == 0 {
                return self.base.classic_on_enter();
            }
            return Ok(StateReturnType::Continue);
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct AIAttackSquadState {
    base: State,
    attack_squad_machine: Option<AIAttackThenIdleStateMachine>,
}

impl AIAttackSquadState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIAttackSquad"),
            attack_squad_machine: None,
        }
    }

    fn choose_victim(&mut self) -> Option<Arc<RwLock<Object>>> {
        let squad = self.base.get_machine_goal_squad()?;
        let owner = self.base.get_machine_owner()?;
        let owner_guard = owner.read().ok()?;
        let owner_pos = *owner_guard.get_position();
        let owner_off_map = owner_guard.is_off_map();
        let ai = owner_guard.get_ai_update_interface()?;

        let mood_val = ai
            .try_lock()
            .ok()
            .map(|guard| guard.get_mood_matrix_value())
            .unwrap_or(0);
        if (mood_val & mood_matrix_parameters::CONTROLLER_AI) != 0 {
            if (mood_val & mood_matrix_parameters::MOOD_SLEEP) != 0 {
                return None;
            }
            if (mood_val & mood_matrix_parameters::MOOD_PASSIVE) != 0 {
                let victim_id = owner_guard
                    .get_body_module()
                    .and_then(|body| body.get_last_damage_info())
                    .map(|info| info.input.source_id)
                    .unwrap_or(INVALID_ID);
                if victim_id == INVALID_ID {
                    return None;
                }
                return TheGameLogic::find_object_by_id(victim_id);
            }
        }

        let mut difficulty = owner_guard
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|player| player.get_player_difficulty())
            })
            .unwrap_or(crate::player::GameDifficulty::Normal);

        if ai.get_last_command_source() == CommandSourceType::FromPlayer {
            difficulty = crate::player::GameDifficulty::Hard;
        }
        if let Ok(script_guard) = get_script_engine().read() {
            if script_guard
                .as_ref()
                .map(|engine| engine.get_choose_victim_always_uses_normal())
                .unwrap_or(false)
            {
                difficulty = crate::player::GameDifficulty::Normal;
            }
        }
        drop(owner_guard);

        let mut squad_guard = squad.lock().ok()?;
        let objects = squad_guard.get_live_objects();

        match difficulty {
            crate::player::GameDifficulty::Easy => {
                if objects.is_empty() {
                    return None;
                }
                let idx = GameLogicRandomValue(0, objects.len().saturating_sub(1) as i32);
                objects.get(idx as usize).cloned()
            }
            crate::player::GameDifficulty::Normal => {
                let mut best: Option<Arc<RwLock<Object>>> = None;
                let mut best_dist_sqr = f32::MAX;
                for obj in objects {
                    if let Ok(obj_guard) = obj.read() {
                        if obj_guard.is_off_map() != owner_off_map {
                            continue;
                        }
                        let target_pos = *obj_guard.get_position();
                        let dx = owner_pos.x - target_pos.x;
                        let dy = owner_pos.y - target_pos.y;
                        let dist_sqr = dx * dx + dy * dy;
                        if dist_sqr < best_dist_sqr {
                            best_dist_sqr = dist_sqr;
                            best = Some(obj.clone());
                        }
                    }
                }
                best
            }
            crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal => {
                objects.first().cloned()
            }
        }
    }
}

impl StateImplementation for AIAttackSquadState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackSquadState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack squad missing owner".to_string())?;
        let mut attack_machine =
            AIAttackThenIdleStateMachine::new(Arc::downgrade(&owner), "AIAttackMachine");

        let victim = self.choose_victim();
        if let Some(victim) = victim.as_ref() {
            attack_machine.set_goal_object(Some(victim));
        }

        let result = attack_machine.init_default_state();
        self.attack_squad_machine = Some(attack_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let attack_status = {
            let Some(attack_machine) = self.attack_squad_machine.as_mut() else {
                return Ok(StateReturnType::Failure);
            };
            let status = match attack_machine.update() {
                StateReturnType::Sleep(_) => StateReturnType::Continue,
                other => other,
            };
            if attack_machine.get_current_state_id() != Some(AIStateType::Idle as u32) {
                return Ok(status);
            }
            status
        };
        let _ = attack_status;

        if let Ok(owner_guard) = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack squad missing owner".to_string())?
            .read()
        {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.try_lock() {
                    if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                        if let Some(attack_machine) = self.attack_squad_machine.as_mut() {
                            attack_machine.set_goal_object(Some(&crate_obj));
                            attack_machine.set_state(AIStateType::PickUpCrate);
                        }
                        return Ok(StateReturnType::Continue);
                    }
                }
            }
        }

        let victim = self.choose_victim();
        let Some(victim) = victim else {
            return Ok(StateReturnType::Success);
        };

        if let Some(attack_machine) = self.attack_squad_machine.as_mut() {
            attack_machine.set_goal_object(Some(&victim));
            attack_machine.set_state(AIStateType::AttackObject);
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_squad_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackAreaState {
    base: State,
    attack_machine: Option<AIAttackThenIdleStateMachine>,
    next_enemy_scan_time: u32,
}

impl AIAttackAreaState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIAttackArea"),
            attack_machine: None,
            next_enemy_scan_time: 0,
        }
    }

    fn find_area_victim(&self, owner: &Object) -> Option<Arc<RwLock<Object>>> {
        let polygon = self.base.get_machine_goal_polygon()?;
        let owner_id = owner.get_id();
        let attack_priority = resolve_attack_priority_info_for_object(owner_id);

        struct PolygonFilter {
            polygon: PolygonTrigger,
        }

        impl PartitionFilter for PolygonFilter {
            fn allow(&self, object: ObjectID) -> bool {
                let Some(target_arc) = TheGameLogic::find_object_by_id(object) else {
                    return false;
                };
                let Ok(target_guard) = target_arc.read() else {
                    return false;
                };
                let pos = target_guard.get_position();
                self.polygon.point_in_trigger(&Coord2D::new(pos.x, pos.y))
            }

            fn debug_get_name(&self) -> &str {
                "PartitionFilterPolygonTrigger"
            }
        }

        let filter = PolygonFilter {
            polygon: (*polygon).clone(),
        };

        let victim_id = THE_AI
            .read()
            .ok()?
            .find_closest_enemy(
                owner_id,
                9999.9,
                search_qualifiers::CAN_ATTACK,
                attack_priority.as_ref(),
                Some(&filter),
            )
            .ok()??;
        TheGameLogic::find_object_by_id(victim_id)
    }
}

impl StateImplementation for AIAttackAreaState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackAreaState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack area missing owner".to_string())?;
        let mut attack_machine = AIAttackThenIdleStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackThenIdleStateMachine",
        );

        let now = TheGameLogic::get_frame();
        let jitter = GameLogicRandomValue(0, ENEMY_SCAN_RATE as i32) as u32;
        self.next_enemy_scan_time = now + jitter;

        let result = attack_machine.init_default_state();
        self.attack_machine = Some(attack_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_enemy_scan_time {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack area missing owner".to_string())?;

            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                    return Ok(StateReturnType::Failure);
                }
            }

            self.next_enemy_scan_time = now + ENEMY_SCAN_RATE;
            if self.base.get_machine_goal_polygon().is_none() {
                return Ok(StateReturnType::Failure);
            }
            let victim = owner
                .read()
                .ok()
                .and_then(|owner_guard| self.find_area_victim(&owner_guard));

            if let Some(attack_machine) = self.attack_machine.as_mut() {
                attack_machine.set_goal_object(victim.as_ref());

                if attack_machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                    && victim.is_some()
                {
                    attack_machine.set_state(AIStateType::AttackObject);
                }
            }

            if victim.is_none() {
                return Ok(StateReturnType::Success);
            }
        }

        if let Some(attack_machine) = self.attack_machine.as_mut() {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(mut machine_guard) = machine.lock() {
                    machine_guard.lock();
                    let result = attack_machine.update();
                    machine_guard.unlock();
                    return Ok(match result {
                        StateReturnType::Sleep(_) => StateReturnType::Continue,
                        other => other,
                    });
                }
            }
            return Ok(match attack_machine.update() {
                StateReturnType::Sleep(_) => StateReturnType::Continue,
                other => other,
            });
        }

        Ok(StateReturnType::Failure)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

/// Guard state
#[derive(Debug)]
pub struct AIGuardState {
    base: State,
    guard_machine: Option<AIGuardMachine>,
}

impl AIGuardState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIGuard"),
            guard_machine: None,
        }
    }
}

impl StateImplementation for AIGuardState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIGuardState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "guard state missing machine owner".to_string())?;

        let mut guard_machine = AIGuardMachine::new(Arc::downgrade(&owner));

        if let Some(polygon) = self.base.get_machine_goal_polygon() {
            guard_machine.set_area_to_guard(Some(polygon.clone()));
            let center = polygon.get_center_point();
            guard_machine.set_target_position_to_guard(&center);
        } else if let Some(target) = self.base.get_machine_goal_object() {
            guard_machine.set_target_to_guard(Some(&target));
        } else if let Some(pos) = self.base.get_machine_goal_position() {
            guard_machine.set_target_position_to_guard(&pos);
        } else if let Ok(owner_guard) = owner.try_read() {
            guard_machine.set_target_position_to_guard(owner_guard.get_position());
        }

        let guard_mode = self
            .base
            .get_machine()
            .ok()
            .and_then(|machine| machine.lock().ok().map(|guard| guard.get_guard_mode_raw()))
            .map(GuardMode::from_i32)
            .unwrap_or(GuardMode::Normal);
        guard_machine.set_guard_mode(guard_mode);

        if guard_machine.init_default_state().is_failure() {
            return Ok(StateReturnType::Failure);
        }

        let result = guard_machine.set_state(GuardStateType::Return);
        self.guard_machine = Some(guard_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(guard_machine) = self.guard_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(machine) = self.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.lock();
                let result = guard_machine.update();
                machine_guard.unlock();
                return Ok(result);
            }
        }

        Ok(guard_machine.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_guard_idle(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_guard_idle_state())
            .unwrap_or(false)
    }

    fn classic_is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// Guard retaliate state
#[derive(Debug)]
pub struct AIGuardRetaliateState {
    base: State,
    guard_machine: Option<AIGuardRetaliateMachine>,
}

impl AIGuardRetaliateState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIGuardRetaliate"),
            guard_machine: None,
        }
    }
}

impl StateImplementation for AIGuardRetaliateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIGuardRetaliateState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "guard retaliate state missing machine owner".to_string())?;

        let mut guard_machine = AIGuardRetaliateMachine::new(Arc::downgrade(&owner));

        if let Some(pos) = self.base.get_machine_goal_position() {
            guard_machine.set_target_position_to_guard(&pos);
        } else if let Ok(owner_guard) = owner.try_read() {
            guard_machine.set_target_position_to_guard(owner_guard.get_position());
        }

        if let Some(goal) = self.base.get_machine_goal_object() {
            if let Ok(goal_guard) = goal.try_read() {
                guard_machine.set_nemesis_id(goal_guard.get_id());
            }
        }

        let result = guard_machine.init_default_state();
        self.guard_machine = Some(guard_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(guard_machine) = self.guard_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        Ok(guard_machine.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// Tunnel network guard state
#[derive(Debug)]
pub struct AITunnelNetworkGuardState {
    base: State,
    guard_machine: Option<AITNGuardMachine>,
}

impl AITunnelNetworkGuardState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AITunnelNetworkGuard"),
            guard_machine: None,
        }
    }
}

impl StateImplementation for AITunnelNetworkGuardState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AITunnelNetworkGuardState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "tunnel network guard state missing machine owner".to_string())?;

        let mut guard_machine = AITNGuardMachine::new(Arc::downgrade(&owner));

        if let Some(pos) = self.base.get_machine_goal_position() {
            guard_machine.set_target_position_to_guard(&pos);
        } else if let Ok(owner_guard) = owner.try_read() {
            guard_machine.set_target_position_to_guard(owner_guard.get_position());
        }

        guard_machine.set_guard_mode(GuardMode::Normal);

        if guard_machine.init_default_state().is_failure() {
            return Ok(StateReturnType::Failure);
        }

        let result = guard_machine.set_state(TNGuardStateType::Return);
        self.guard_machine = Some(guard_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(guard_machine) = self.guard_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(machine) = self.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.lock();
                let result = guard_machine.update();
                machine_guard.unlock();
                return Ok(result);
            }
        }

        Ok(guard_machine.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// Hunt state - seek and destroy
#[derive(Debug)]
pub struct AIHuntState {
    base: State,
    hunt_machine: Option<AIAttackThenIdleStateMachine>,
    next_enemy_scan_time: u32,
}

impl AIHuntState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIHunt"),
            hunt_machine: None,
            next_enemy_scan_time: 0,
        }
    }

    fn find_hunt_victim(&self, owner: &Object) -> Option<Arc<RwLock<Object>>> {
        let owner_id = owner.get_id();
        let attack_info = resolve_attack_priority_info_for_object(owner_id);

        let team_arc = owner.get_team();
        let mut attack_common_target = false;
        let mut team_victim: Option<Arc<RwLock<Object>>> = None;
        if let Some(team) = team_arc.as_ref() {
            if let Ok(team_guard) = team.read() {
                attack_common_target = team_guard.attack_common_target();
                if attack_common_target {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != INVALID_ID {
                        team_victim = get_legacy_object(team_target);
                    }
                }
            }
        }

        let mut victim = if team_victim.is_some() && attack_info.is_none() {
            team_victim.clone()
        } else {
            let enemy_id = THE_AI.read().ok().and_then(|ai| {
                ai.find_closest_enemy(
                    owner_id,
                    9999.9,
                    search_qualifiers::CAN_ATTACK,
                    attack_info.as_ref(),
                    None,
                )
                .ok()
                .flatten()
            });
            enemy_id.and_then(get_legacy_object)
        };

        if victim.is_none() {
            let units_should_hunt = owner
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|guard| guard.get_units_should_hunt())
                })
                .unwrap_or(false);
            if units_should_hunt {
                let fallback_id = THE_AI.read().ok().and_then(|ai| {
                    ai.find_closest_enemy(
                        owner_id,
                        9999.9,
                        search_qualifiers::CAN_ATTACK,
                        None,
                        None,
                    )
                    .ok()
                    .flatten()
                });
                victim = fallback_id.and_then(get_legacy_object);
            }
        }

        if attack_common_target {
            if let (Some(team_target), Some(info)) = (team_victim.as_ref(), attack_info.as_ref()) {
                if victim.is_none() {
                    victim = Some(team_target.clone());
                }
                let team_priority = team_target
                    .read()
                    .ok()
                    .map(|obj| info.get_priority(obj.get_template().get_name().as_str()))
                    .unwrap_or(0);
                let victim_priority = victim
                    .as_ref()
                    .and_then(|obj| {
                        obj.read().ok().map(|guard| {
                            info.get_priority(guard.get_template().get_name().as_str())
                        })
                    })
                    .unwrap_or(0);
                if team_priority >= victim_priority {
                    victim = Some(team_target.clone());
                }
            }

            if let Some(team) = team_arc.as_ref() {
                if let Ok(mut team_guard) = team.write() {
                    let victim_id = victim
                        .as_ref()
                        .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()))
                        .unwrap_or(INVALID_ID);
                    team_guard.set_team_target_object(victim_id);
                }
            }
        }

        victim
    }
}

impl StateImplementation for AIHuntState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIHuntState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "hunt state missing machine owner".to_string())?;
        let mut hunt_machine = AIAttackThenIdleStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackThenIdleStateMachine",
        );

        let now = TheGameLogic::get_frame();
        let jitter = GameLogicRandomValue(0, ENEMY_SCAN_RATE as i32) as u32;
        self.next_enemy_scan_time = now.saturating_add(jitter);

        let result = hunt_machine.init_default_state();
        self.hunt_machine = Some(hunt_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_enemy_scan_time {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "hunt state missing machine owner".to_string())?;
            let owner_guard = owner
                .read()
                .map_err(|_| "hunt state owner lock poisoned".to_string())?;

            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::Failure);
            }

            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                        if let Some(hunt_machine) = self.hunt_machine.as_mut() {
                            hunt_machine.set_goal_object(Some(&crate_obj));
                            let _ = hunt_machine.set_state(AIStateType::PickUpCrate);
                            return Ok(StateReturnType::Continue);
                        }
                    }
                }
            }

            self.next_enemy_scan_time = now.saturating_add(ENEMY_SCAN_RATE);

            let units_should_hunt = owner_guard
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|guard| guard.get_units_should_hunt())
                })
                .unwrap_or(false);
            let victim = self.find_hunt_victim(&owner_guard);
            drop(owner_guard);

            let Some(hunt_machine) = self.hunt_machine.as_mut() else {
                return Ok(StateReturnType::Failure);
            };
            hunt_machine.set_goal_object(victim.as_ref());

            if hunt_machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                && victim.is_some()
            {
                let _ = hunt_machine.set_state(AIStateType::AttackObject);
            }

            if !units_should_hunt
                && hunt_machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                && victim.is_none()
            {
                return Ok(StateReturnType::Success);
            }
        }

        let Some(hunt_machine) = self.hunt_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(machine) = self.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.lock();
                let result = hunt_machine.update();
                machine_guard.unlock();
                return Ok(match result {
                    StateReturnType::Sleep(_) => StateReturnType::Continue,
                    other => other,
                });
            }
        }

        let result = hunt_machine.update();
        Ok(match result {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            other => other,
        })
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.hunt_machine.take() {
            let _ = machine.halt();
        }
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.hunt_machine
            .as_ref()
            .map(|machine| machine.base.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// Dock state - dock with a goal object that supports docking
#[derive(Debug)]
pub struct AIDockState {
    base: State,
    dock_machine: Option<AIDockMachine>,
    using_precision_movement: bool,
}

impl AIDockState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIDock"),
            dock_machine: None,
            using_precision_movement: false,
        }
    }
}

impl StateImplementation for AIDockState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIDockState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "dock state missing machine owner".to_string())?;

        let Some(goal) = self.base.get_machine_goal_object() else {
            return Ok(StateReturnType::Failure);
        };

        let has_dock = goal
            .try_read()
            .ok()
            .and_then(|guard| guard.with_dock_update_interface(|_| true))
            .unwrap_or(false);
        if !has_dock {
            return Ok(StateReturnType::Failure);
        }

        if let Ok(owner_guard) = owner.try_read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.ignore_obstacle(Some(&goal));
                }
            }
        }

        let mut dock_machine = AIDockMachine::new(owner.clone())?;
        let init_result = if let Ok(mut machine) = dock_machine.state_machine.lock() {
            machine.set_goal_object(Some(Arc::downgrade(&goal)));
            Some(machine.init_default_state())
        } else {
            None
        };
        if let Some(result) = init_result {
            self.dock_machine = Some(dock_machine);
            return Ok(result);
        }

        Ok(StateReturnType::Failure)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(dock_machine) = self.dock_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_can_path_through_units(true);
                    }
                }
            }
        }

        let result = dock_machine
            .state_machine
            .lock()
            .map_err(|_| "dock state machine lock failed".to_string())?
            .update();

        Ok(match result {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            other => other,
        })
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.dock_machine.take() {
            let _ = machine.halt();
        }

        let owner = self.base.get_machine_owner();
        if let Some(owner) = owner {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.ignore_obstacle(None);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Move and evacuate state - move to a position then evacuate transport.
#[derive(Debug)]
pub struct AIMoveAndEvacuateState {
    base: AIMoveToState,
    origin: Coord3D,
}

impl AIMoveAndEvacuateState {
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = name.to_string();
        Self {
            base,
            origin: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIMoveAndEvacuateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIMoveAndEvacuateState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move+evacuate missing machine owner".to_string())?;
        if let Ok(owner_guard) = owner.read() {
            self.origin = *owner_guard.get_position();
        }

        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.lock();
            }
        }

        self.base.set_adjusts_destination(true);
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let status = self.base.classic_on_update()?;
        if status != StateReturnType::Continue {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "move+evacuate missing machine owner".to_string())?;
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_effectively_dead() {
                    return Ok(StateReturnType::Failure);
                }
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let params = AiCommandParams::new(
                            AiCommandType::Evacuate,
                            CommandSourceType::FromAi,
                        );
                        let _ = ai_guard.execute_command(&params);
                    }
                }
                if let Some(team) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team.write() {
                        team_guard.set_active();
                    }
                }
            };
        }
        Ok(status)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.unlock();
                machine_guard.set_goal_position(self.origin);
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Enter state - enter a transport or building
#[derive(Debug)]
pub struct AIEnterState {
    base: AIMoveToState,
    entry_to_clear: ObjectID,
    goal_position: Coord3D,
}

impl AIEnterState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIEnter".to_string();
        Self {
            base,
            entry_to_clear: INVALID_ID,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIEnterState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIEnterState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.entry_to_clear = INVALID_ID;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "enter state missing machine owner".to_string())?;
        let goal = self
            .base
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "enter state missing goal object".to_string())?;

        {
            let owner_guard = owner
                .lock()
                .map_err(|_| "enter state owner lock poisoned".to_string())?;
            let goal_guard = goal
                .lock()
                .map_err(|_| "enter state goal lock poisoned".to_string())?;

            let cmd_source = owner_guard
                .get_ai_update_interface()
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|ai_guard| ai_guard.get_last_command_source())
                })
                .unwrap_or(CommandSourceType::FromAi);
            if !TheActionManager::can_enter_object(
                &*owner_guard,
                &*goal_guard,
                cmd_source,
                CanEnterType::CheckCapacity,
            ) {
                return Ok(StateReturnType::Failure);
            }

            self.goal_position = *goal_guard.get_position();
            if let Some(contain) = goal_guard.get_contain() {
                contain.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToEnter);
                self.entry_to_clear = goal_guard.get_id();
            }

            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.ignore_obstacle(Some(&goal));
                    let _ = ai_guard.set_allow_invalid_position(true);
                }
            }
        }

        self.base.set_adjusts_destination(false);
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "enter state missing machine owner".to_string())?;
        let goal = self
            .base
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "enter state missing goal object".to_string())?;

        {
            let owner_guard = owner
                .lock()
                .map_err(|_| "enter state owner lock poisoned".to_string())?;
            let goal_guard = goal
                .lock()
                .map_err(|_| "enter state goal lock poisoned".to_string())?;

            if goal_guard.get_contained_by().is_some()
                && goal_guard.is_above_terrain()
                && !owner_guard.is_above_terrain()
            {
                return Ok(StateReturnType::Failure);
            }

            self.goal_position = *goal_guard.get_position();
            if let Ok(machine) = self.base.base.get_machine() {
                if let Ok(mut machine_guard) = machine.lock() {
                    machine_guard.set_goal_position(self.goal_position);
                }
            }

            let cmd_source = owner_guard
                .get_ai_update_interface()
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|ai_guard| ai_guard.get_last_command_source())
                })
                .unwrap_or(CommandSourceType::FromAi);
            if !TheActionManager::can_enter_object(
                &*owner_guard,
                &*goal_guard,
                cmd_source,
                CanEnterType::CheckCapacity,
            ) {
                if owner_guard.relationship_to(&goal_guard) == Relationship::Enemy {
                    let can_attack = owner_guard.get_able_to_attack_specific_object(
                        AbleToAttackType::CanAttackSpecific,
                        &goal_guard,
                        CommandSourceType::FromAi,
                    );
                    if matches!(
                        can_attack,
                        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                    ) {
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            ai.ai_attack_object(
                                &goal,
                                NO_MAX_SHOTS_LIMIT,
                                CommandSourceType::FromAi,
                            );
                        }
                        return Ok(StateReturnType::Continue);
                    }
                }
                return Ok(StateReturnType::Failure);
            }

            if owner_guard.is_disabled_by_type(DisabledType::Held) {
                return Ok(StateReturnType::Success);
            }
        }

        let mut code = self.base.classic_on_update()?;

        if code == StateReturnType::Success {
            let owner_guard = owner
                .lock()
                .map_err(|_| "enter state owner lock poisoned".to_string())?;
            let goal_guard = goal
                .lock()
                .map_err(|_| "enter state goal lock poisoned".to_string())?;

            if goal_guard.is_above_terrain() && !owner_guard.is_above_terrain() {
                return Ok(StateReturnType::Continue);
            }

            let owner_pos = owner_guard.get_position();
            let goal_pos = goal_guard.get_position();
            let dx = owner_pos.x - goal_pos.x;
            let dy = owner_pos.y - goal_pos.y;
            let mut radius = goal_guard.get_geometry_info().get_minor_radius();
            if goal_guard.get_template_geometry_type() != Some(GeometryType::Box) {
                radius = goal_guard.get_geometry_info().get_major_radius();
            }
            let close_enough = dx * dx + dy * dy < radius * radius;
            if close_enough {
                if let Some(contain) = goal_guard.get_contain() {
                    contain.add_to_contain(&*owner_guard);
                }
            }
        }

        Ok(code)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.ignore_obstacle(None);
                        let _ = ai_guard.set_allow_invalid_position(false);
                    }
                }

                if self.entry_to_clear != INVALID_ID {
                    if let Some(goal) = get_legacy_object(self.entry_to_clear) {
                        if let Ok(goal_guard) = goal.read() {
                            if let Some(contain) = goal_guard.get_contain() {
                                contain.on_object_wants_to_enter_or_exit(
                                    &*owner_guard,
                                    ContainWant::WantsNeither,
                                );
                            }
                        }
                    }
                }
            }
        }

        self.entry_to_clear = INVALID_ID;
        Ok(())
    }
}

/// Exit state - exit from transport or building
#[derive(Debug)]
pub struct AIExitState {
    base: State,
    entry_to_clear: ObjectID,
}

impl AIExitState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIExit"),
            entry_to_clear: INVALID_ID,
        }
    }
}

impl StateImplementation for AIExitState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIExitState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.entry_to_clear = INVALID_ID;

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "exit state missing machine owner".to_string())?;
        let goal = self
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "exit state missing goal object".to_string())?;

        let owner_guard = owner
            .read()
            .map_err(|_| "exit state owner lock poisoned".to_string())?;
        let goal_guard = goal
            .read()
            .map_err(|_| "exit state goal lock poisoned".to_string())?;

        if goal_guard.is_disabled_by_type(DisabledType::DisabledSubdued) {
            return Ok(StateReturnType::Failure);
        }

        if let Some(contain) = goal_guard.get_contain() {
            contain.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToExit);
            self.entry_to_clear = goal_guard.get_id();
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "exit state missing machine owner".to_string())?;
        let goal = self
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "exit state missing goal object".to_string())?;

        let owner_guard = owner
            .read()
            .map_err(|_| "exit state owner lock poisoned".to_string())?;
        let goal_guard = goal
            .read()
            .map_err(|_| "exit state goal lock poisoned".to_string())?;

        if let Some(goal_ai) = goal_guard.get_ai_update_interface() {
            if let Ok(goal_ai_guard) = goal_ai.lock() {
                if goal_ai_guard.get_ai_free_to_exit(&*owner_guard) == AIFreeToExitType::WaitToExit
                {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        let exit_interface = goal_guard
            .get_object_exit_interface()
            .ok_or_else(|| "exit state missing exit interface".to_string())?;
        let mut exit_guard = exit_interface
            .lock()
            .map_err(|_| "exit state exit interface lock poisoned".to_string())?;
        let exit_door = exit_guard.reserve_door_for_exit(Some(&*goal_guard), Some(&*owner_guard));
        if exit_door == ExitDoorType::NoneAvailable {
            return Ok(StateReturnType::Failure);
        }
        exit_guard
            .exit_object_via_door(&owner, exit_door)
            .map_err(|err| format!("exit state exit_object_via_door failed: {}", err))?;

        if let Ok(machine) = self.base.get_machine() {
            if let Ok(machine_guard) = machine.lock() {
                if machine_guard.get_current_state_id() != Some(self.base.get_id()) {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if self.entry_to_clear != INVALID_ID {
                    if let Some(goal) = get_legacy_object(self.entry_to_clear) {
                        if let Ok(goal_guard) = goal.read() {
                            if let Some(contain) = goal_guard.get_contain() {
                                contain.on_object_wants_to_enter_or_exit(
                                    &*owner_guard,
                                    ContainWant::WantsNeither,
                                );
                            }
                        }
                    }
                }
            }
        }

        self.entry_to_clear = INVALID_ID;
        Ok(())
    }
}

/// Exit instantly state - exit from transport or building immediately
#[derive(Debug)]
pub struct AIExitInstantlyState {
    base: State,
    entry_to_clear: ObjectID,
}

impl AIExitInstantlyState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIExitInstantly"),
            entry_to_clear: INVALID_ID,
        }
    }
}

impl StateImplementation for AIExitInstantlyState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIExitInstantlyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.entry_to_clear = INVALID_ID;

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "exit instantly state missing machine owner".to_string())?;
        let goal = self
            .base
            .get_machine_goal_object()
            .ok_or_else(|| "exit instantly state missing goal object".to_string())?;

        let owner_guard = owner
            .read()
            .map_err(|_| "exit instantly owner lock poisoned".to_string())?;
        let goal_guard = goal
            .read()
            .map_err(|_| "exit instantly goal lock poisoned".to_string())?;

        if goal_guard.is_disabled_by_type(DisabledType::DisabledSubdued) {
            return Ok(StateReturnType::Failure);
        }

        if let Some(contain) = goal_guard.get_contain() {
            contain.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToExit);
            self.entry_to_clear = goal_guard.get_id();
        }

        let exit_interface = goal_guard
            .get_object_exit_interface()
            .ok_or_else(|| "exit instantly missing exit interface".to_string())?;
        exit_interface
            .lock()
            .map_err(|_| "exit instantly exit interface lock poisoned".to_string())?
            .exit_object_via_door(&owner, ExitDoorType::Door1)
            .map_err(|err| format!("exit instantly exit_object_via_door failed: {}", err))?;

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Ok(machine) = self.base.get_machine() {
            if let Ok(machine_guard) = machine.lock() {
                if machine_guard.get_current_state_id() != Some(self.base.get_id()) {
                    return Ok(StateReturnType::Continue);
                }
            }
        }
        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if self.entry_to_clear != INVALID_ID {
                    if let Some(goal) = get_legacy_object(self.entry_to_clear) {
                        if let Ok(goal_guard) = goal.read() {
                            if let Some(contain) = goal_guard.get_contain() {
                                contain.on_object_wants_to_enter_or_exit(
                                    &*owner_guard,
                                    ContainWant::WantsNeither,
                                );
                            }
                        }
                    }
                }
            }
        }

        self.entry_to_clear = INVALID_ID;
        Ok(())
    }
}

/// Dead state - unit is dead
#[derive(Debug)]
pub struct AIDeadState {
    base: State,
}

impl AIDeadState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIDead"),
        }
    }
}

impl StateImplementation for AIDeadState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIDeadState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Handle death
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Stay dead indefinitely
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Should never exit from dead state
        Ok(())
    }
}

/// Attack state machine for more complex attack behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackSubStateId {
    AimAtTarget = 1,
    FireWeapon = 2,
    PursueTarget = 3,
    ApproachTarget = 4,
}

#[derive(Clone, Copy, Debug)]
struct AttackContinuationData {
    attack_type: AbleToAttackType,
    force_attacking: Bool,
}

fn out_of_weapon_range_object_state(base: &State) -> Result<bool, String> {
    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let target = base
        .get_machine_goal_object()
        .ok_or_else(|| "attack condition missing target".to_string())?;
    let owner_guard = owner
        .lock()
        .map_err(|_| "attack condition owner lock poisoned".to_string())?;
    let target_guard = target
        .lock()
        .map_err(|_| "attack condition target lock poisoned".to_string())?;
    let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
        return Ok(false);
    };
    if weapon.has_leech_range() {
        return Ok(false);
    }
    Ok(!weapon.is_within_attack_range(owner_guard.get_id(), Some(target_guard.get_id()), None))
}

fn out_of_weapon_range_position_state(base: &State) -> Result<bool, String> {
    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let owner_guard = owner
        .lock()
        .map_err(|_| "attack condition owner lock poisoned".to_string())?;
    let Some(pos) = base.get_machine_goal_position() else {
        return Ok(false);
    };
    let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
        return Ok(false);
    };
    Ok(!weapon.is_within_attack_range(owner_guard.get_id(), None, Some(&pos)))
}

fn want_to_squish_target_state(base: &State) -> Result<bool, String> {
    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let target = base
        .get_machine_goal_object()
        .ok_or_else(|| "attack condition missing target".to_string())?;
    let owner_guard = owner
        .lock()
        .map_err(|_| "attack condition owner lock poisoned".to_string())?;
    let target_guard = target
        .lock()
        .map_err(|_| "attack condition target lock poisoned".to_string())?;

    if target_guard.get_contained_by().is_some() {
        return Ok(false);
    }

    let turret = owner_guard
        .get_ai_update_interface()
        .map(|ai| ai.get_which_turret_for_cur_weapon())
        .unwrap_or(TurretType::Invalid);
    if turret == TurretType::Invalid {
        return Ok(false);
    }

    let is_computer = if let Some(player) = owner_guard.get_controlling_player() {
        if let Ok(player_guard) = player.read() {
            player_guard.get_player_type() == PlayerType::Computer
        } else {
            false
        }
    } else {
        false
    };
    if !is_computer {
        return Ok(false);
    }

    if owner_guard.get_crusher_level() == 0 {
        return Ok(false);
    }

    if !target_guard.is_kind_of(KindOf::Infantry) {
        return Ok(false);
    }

    Ok(true)
}

fn cannot_possibly_attack_object_state(
    base: &State,
    user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let target = base
        .get_machine_goal_object()
        .ok_or_else(|| "attack condition missing target".to_string())?;
    let owner_guard = owner
        .lock()
        .map_err(|_| "attack condition owner lock poisoned".to_string())?;
    let target_guard = target
        .lock()
        .map_err(|_| "attack condition target lock poisoned".to_string())?;

    if !owner_guard.is_able_to_attack() {
        return Ok(true);
    }

    let attack_type = user_data
        .data
        .as_ref()
        .and_then(|payload| payload.downcast_ref::<AttackContinuationData>())
        .map(|data| data.attack_type)
        .unwrap_or(AbleToAttackType::CanAttackSpecific);

    let cmd_source = owner_guard
        .get_ai_update_interface()
        .map(|ai| ai.get_last_command_source())
        .unwrap_or(CommandSourceType::FromAi);

    let result =
        owner_guard.get_able_to_attack_specific_object(attack_type, &target_guard, cmd_source);
    Ok(!matches!(
        result,
        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
    ))
}

fn out_of_weapon_range_object_aim(
    state: &AIAttackAimAtTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_object_state(state.base_state())
}

fn out_of_weapon_range_object_fire(
    state: &AIAttackFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_object_state(state.base_state())
}

fn out_of_weapon_range_position_aim(
    state: &AIAttackAimAtTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_position_state(state.base_state())
}

fn out_of_weapon_range_position_fire(
    state: &AIAttackFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_position_state(state.base_state())
}

fn want_to_squish_target_aim(
    state: &AIAttackAimAtTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    want_to_squish_target_state(state.base_state())
}

fn want_to_squish_target_fire(
    state: &AIAttackFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    want_to_squish_target_state(state.base_state())
}

fn cannot_possibly_attack_object_aim(
    state: &AIAttackAimAtTargetState,
    user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    cannot_possibly_attack_object_state(state.base_state(), user_data)
}

fn cannot_possibly_attack_object_fire(
    state: &AIAttackFireWeaponState,
    user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    cannot_possibly_attack_object_state(state.base_state(), user_data)
}

pub struct AttackStateMachine {
    base: StateMachine,
    exit_conditions: Option<Box<dyn AttackExitConditionsInterface>>,
    follow: Bool,
    attacking_object: Bool,
    force_attacking: Bool,
}

impl std::fmt::Debug for AttackStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttackStateMachine")
            .field("base", &self.base)
            .field("follow", &self.follow)
            .field("attacking_object", &self.attacking_object)
            .field("force_attacking", &self.force_attacking)
            .finish()
    }
}

/// Interface for attack exit conditions
pub trait AttackExitConditionsInterface: Send + Sync {
    fn should_exit(&self, machine: &StateMachine) -> bool;
}

impl AttackStateMachine {
    pub fn new(
        owner: Weak<RwLock<Object>>,
        name: &str,
        follow: Bool,
        attacking_object: Bool,
        force_attacking: Bool,
    ) -> Self {
        let mut base = StateMachine::new(Some(owner), name);
        let aim_state = AIAttackAimAtTargetState::new(&base, attacking_object, force_attacking);
        let fire_state = AIAttackFireWeaponState::new(&base, attacking_object);
        let pursue_state =
            AIAttackPursueTargetState::new(&base, follow, attacking_object, force_attacking);
        let approach_state =
            AIAttackApproachTargetState::new(&base, follow, attacking_object, force_attacking);

        let object_conditions_aim = if force_attacking {
            vec![
                legacy_transition::<AIAttackAimAtTargetState>(
                    out_of_weapon_range_object_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    cannot_possibly_attack_object_aim,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTargetForced,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    want_to_squish_target_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
            ]
        } else {
            vec![
                legacy_transition::<AIAttackAimAtTargetState>(
                    out_of_weapon_range_object_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    want_to_squish_target_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    cannot_possibly_attack_object_aim,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTarget,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
            ]
        };

        let object_conditions_fire = if force_attacking {
            vec![
                legacy_transition::<AIAttackFireWeaponState>(
                    out_of_weapon_range_object_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    cannot_possibly_attack_object_fire,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTargetForced,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    want_to_squish_target_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
            ]
        } else {
            vec![
                legacy_transition::<AIAttackFireWeaponState>(
                    out_of_weapon_range_object_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    want_to_squish_target_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    cannot_possibly_attack_object_fire,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTarget,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
            ]
        };

        let position_conditions_aim = vec![legacy_transition::<AIAttackAimAtTargetState>(
            out_of_weapon_range_position_aim,
            AttackSubStateId::PursueTarget as u32,
            StateTransitionUserData::new(),
            "out_of_weapon_range_position",
        )];

        let position_conditions_fire = vec![legacy_transition::<AIAttackFireWeaponState>(
            out_of_weapon_range_position_fire,
            AttackSubStateId::PursueTarget as u32,
            StateTransitionUserData::new(),
            "out_of_weapon_range_position",
        )];

        register_classic_state(
            &mut base,
            AttackSubStateId::AimAtTarget as u32,
            aim_state,
            Some(AttackSubStateId::FireWeapon as u32),
            Some(EXIT_MACHINE_WITH_FAILURE),
            if attacking_object {
                &object_conditions_aim
            } else {
                &position_conditions_aim
            },
        );

        register_classic_state(
            &mut base,
            AttackSubStateId::FireWeapon as u32,
            fire_state,
            Some(AttackSubStateId::AimAtTarget as u32),
            Some(AttackSubStateId::AimAtTarget as u32),
            if attacking_object {
                &object_conditions_fire
            } else {
                &position_conditions_fire
            },
        );

        register_classic_state(
            &mut base,
            AttackSubStateId::PursueTarget as u32,
            pursue_state,
            Some(AttackSubStateId::ApproachTarget as u32),
            Some(AttackSubStateId::ApproachTarget as u32),
            &[],
        );

        register_classic_state(
            &mut base,
            AttackSubStateId::ApproachTarget as u32,
            approach_state,
            Some(AttackSubStateId::AimAtTarget as u32),
            Some(EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        Self {
            base,
            exit_conditions: None,
            follow,
            attacking_object,
            force_attacking,
        }
    }

    /// Set exit conditions
    pub fn set_exit_conditions(&mut self, conditions: Box<dyn AttackExitConditionsInterface>) {
        self.exit_conditions = Some(conditions);
    }

    /// Check if should exit attack
    pub fn should_exit_attack(&self) -> bool {
        if let Some(ref conditions) = self.exit_conditions {
            conditions.should_exit(&self.base)
        } else {
            false
        }
    }

    pub fn set_goal_object(&mut self, obj: Option<&Arc<RwLock<Object>>>) {
        let weak = obj.map(|value| Arc::downgrade(&value));
        self.base.set_goal_object(weak);
    }

    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.set_goal_position(pos);
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: AttackSubStateId) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn update(&mut self) -> StateReturnType {
        if self.should_exit_attack() {
            return StateReturnType::Success;
        }
        self.base.update()
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer).map_err(|err| err.to_string())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process().map_err(|err| err.to_string())
    }
}

#[derive(Debug)]
pub struct AIAttackAimAtTargetState {
    base: State,
    attacking_object: Bool,
    force_attacking: Bool,
    can_turn_in_place: Bool,
    set_locomotor: Bool,
}

impl AIAttackAimAtTargetState {
    pub fn new(machine: &StateMachine, attacking_object: Bool, force_attacking: Bool) -> Self {
        Self {
            base: State::new(machine, "AIAttackAimAtTarget"),
            attacking_object,
            force_attacking,
            can_turn_in_place: false,
            set_locomotor: false,
        }
    }
}

impl StateImplementation for AIAttackAimAtTargetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackAimAtTargetState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack aim missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "attack aim owner lock poisoned".to_string())?;
        let weapon = owner_guard
            .get_current_weapon()
            .map(|(weapon, _slot)| weapon)
            .ok_or_else(|| "attack aim missing weapon".to_string())?;

        let target_pos = self.base.get_machine_goal_position();
        let mut in_range = false;
        let mut preventing = false;
        self.set_locomotor = false;

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if let Some(loco) = ai_guard.get_cur_locomotor() {
                    if let Ok(loco_guard) = loco.lock() {
                        self.can_turn_in_place = loco_guard.template.min_speed == 0.0;
                    }
                }
            }
        }

        let mut used_contain = false;
        if let Some(container_id) = owner_guard.get_contained_by() {
            if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container.read() {
                    if let Some(contain) = container_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            if contain_guard.is_enclosing_container_for(&*owner_guard) {
                                used_contain = true;
                                if self.attacking_object {
                                    if let Some(target) = self.base.get_machine_goal_object() {
                                        in_range = contain_guard.attempt_best_fire_point_position(
                                            owner.clone(),
                                            weapon,
                                            target.clone(),
                                        );
                                    }
                                } else if let Some(pos) = target_pos {
                                    in_range = contain_guard
                                        .attempt_best_fire_point_position_coord(
                                            owner.clone(),
                                            weapon,
                                            &pos,
                                        );
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.attacking_object {
            let target = self
                .base
                .get_machine_goal_object()
                .ok_or_else(|| "attack aim missing target".to_string())?;
            let target_guard = target
                .lock()
                .map_err(|_| "attack aim target lock poisoned".to_string())?;
            if !used_contain {
                in_range = weapon.is_within_attack_range(
                    owner_guard.get_id(),
                    Some(target_guard.get_id()),
                    None,
                );
            }
            if let Some(ai) = target_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.add_targeter(owner_guard.get_id(), true);
                    preventing = ai_guard.is_temporarily_preventing_aim_success();
                }
            }

            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.are_turrets_linked() {
                        for turret in [TurretType::Primary, TurretType::Secondary] {
                            ai_guard.set_turret_target_object(
                                turret,
                                Some(&target),
                                self.force_attacking,
                            );
                        }
                    } else {
                        let turret = ai_guard.get_which_turret_for_cur_weapon();
                        if turret != TurretType::Invalid {
                            ai_guard.set_turret_target_object(
                                turret,
                                Some(&target),
                                self.force_attacking,
                            );
                        } else if weapon.is_contact_weapon() && in_range && !preventing {
                            return Ok(StateReturnType::Success);
                        }
                    }
                }
            }
        } else if let Some(pos) = target_pos {
            if !used_contain {
                in_range = weapon.is_within_attack_range(owner_guard.get_id(), None, Some(&pos));
            }
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.are_turrets_linked() {
                        for turret in [TurretType::Primary, TurretType::Secondary] {
                            ai_guard.set_turret_target_position(turret, &pos);
                        }
                    } else {
                        let turret = ai_guard.get_which_turret_for_cur_weapon();
                        if turret != TurretType::Invalid {
                            ai_guard.set_turret_target_position(turret, &pos);
                        } else if weapon.is_contact_weapon() && in_range {
                            return Ok(StateReturnType::Success);
                        }
                    }
                }
            }
        } else {
            return Ok(StateReturnType::Failure);
        }

        owner_guard.set_status(ObjectStatusMaskType::IS_AIMING_WEAPON, true);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack aim missing owner".to_string())?;

        let owner_guard = owner
            .lock()
            .map_err(|_| "attack aim owner lock poisoned".to_string())?;

        if !owner_guard.has_any_weapon() {
            return Ok(StateReturnType::Failure);
        }

        let target_pos = if self.attacking_object {
            let target = self
                .base
                .get_machine_goal_object()
                .ok_or_else(|| "attack aim missing target".to_string())?;
            let target_guard = target
                .lock()
                .map_err(|_| "attack aim target lock poisoned".to_string())?;
            if target_guard.is_effectively_dead() {
                return Ok(StateReturnType::Failure);
            }
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        ai_guard.set_turret_target_object(
                            turret,
                            Some(&target),
                            self.force_attacking,
                        );
                        return Ok(StateReturnType::Continue);
                    }
                }
            }
            *target_guard.get_position()
        } else if let Some(pos) = self.base.get_machine_goal_position() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        ai_guard.set_turret_target_position(turret, &pos);
                        return Ok(StateReturnType::Continue);
                    }
                }
            }
            pos
        } else {
            return Ok(StateReturnType::Failure);
        };

        let owner_pos = *owner_guard.get_position();
        let owner_angle = owner_guard.get_orientation();
        let angle_to_target = (target_pos.y - owner_pos.y).atan2(target_pos.x - owner_pos.x);
        let rel_angle = normalize_angle(angle_to_target - owner_angle);

        let weapon = owner_guard
            .get_current_weapon()
            .map(|(weapon, _slot)| weapon)
            .ok_or_else(|| "attack aim missing weapon".to_string())?;
        const REL_THRESH: Real = 0.035;
        let mut aim_delta = weapon.get_template().aim_delta;
        if aim_delta < REL_THRESH {
            aim_delta = REL_THRESH;
        }

        if self.can_turn_in_place {
            if rel_angle.abs() > aim_delta {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let desired_angle = owner_angle + rel_angle;
                        ai_guard.set_locomotor_goal_orientation(desired_angle);
                        self.set_locomotor = true;
                    }
                }
            }
        } else if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.set_locomotor_goal_position_explicit(target_pos);
            }
        }

        if rel_angle.abs() < aim_delta {
            if self.attacking_object {
                if let Some(target) = self.base.get_machine_goal_object() {
                    if let Ok(target_guard) = target.lock() {
                        if let Some(ai) = target_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard.add_targeter(owner_guard.get_id(), true);
                                if ai_guard.is_temporarily_preventing_aim_success() {
                                    return Ok(StateReturnType::Continue);
                                }
                            }
                        }
                    }
                }
            }
            return Ok(StateReturnType::Success);
        }

        if owner_guard.is_disabled_by_type(DisabledType::Held) {
            let in_range = if self.attacking_object {
                if let Some(target) = self.base.get_machine_goal_object() {
                    if let Ok(target_guard) = target.read() {
                        weapon.is_within_attack_range(
                            owner_guard.get_id(),
                            Some(target_guard.get_id()),
                            None,
                        )
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                weapon.is_within_attack_range(owner_guard.get_id(), None, Some(&target_pos))
            };
            if !in_range {
                return Ok(StateReturnType::Failure);
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut guard) = owner.lock() {
                guard.set_status(ObjectStatusMaskType::IS_AIMING_WEAPON, false);
                if self.can_turn_in_place && self.set_locomotor {
                    if let Some(ai) = guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard.set_locomotor_goal_none();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackFireWeaponState {
    base: State,
    attacking_object: Bool,
}

impl AIAttackFireWeaponState {
    pub fn new(machine: &StateMachine, attacking_object: Bool) -> Self {
        Self {
            base: State::new(machine, "AIAttackFireWeapon"),
            attacking_object,
        }
    }
}

impl StateImplementation for AIAttackFireWeaponState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIAttackFireWeaponState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack fire missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "attack fire owner lock poisoned".to_string())?;

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let adjust = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Attack);
                if (adjust & mood_matrix_adjustment::ACTION_OK) == 0 {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        let victim_id = self
            .base
            .get_machine_goal_object()
            .and_then(|victim| victim.read().ok().map(|guard| guard.get_id()));
        owner_guard.set_status(
            ObjectStatusMaskType::from_status(ObjectStatusTypes::IsFiringWeapon),
            true,
        );
        owner_guard.pre_fire_current_weapon(victim_id);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack fire missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "attack fire owner lock poisoned".to_string())?;

        let victim = self.base.get_machine_goal_object();
        if self.attacking_object {
            if let Some(victim_obj) = victim.as_ref() {
                if let Ok(victim_guard) = victim_obj.read() {
                    if victim_guard.is_effectively_dead() {
                        return Ok(StateReturnType::Failure);
                    }
                }
            } else {
                return Ok(StateReturnType::Failure);
            }
        }

        let (slot, status, continue_range) = {
            let (weapon, slot) = owner_guard
                .get_current_weapon()
                .ok_or_else(|| "attack fire missing weapon".to_string())?;
            (
                slot,
                weapon.get_status(),
                weapon.get_continue_attack_range(),
            )
        };
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if !ai_guard.is_weapon_slot_ok_to_fire(slot) {
                    return Ok(StateReturnType::Failure);
                }
            }
        }
        if status == WeaponStatus::PreAttack {
            return Ok(StateReturnType::Continue);
        }
        if status != WeaponStatus::ReadyToFire {
            return Ok(StateReturnType::Failure);
        }

        owner_guard.set_firing_condition_for_current_weapon();

        if self.attacking_object {
            if let Some(target) = victim {
                let victim_id = target.read().ok().map(|g| g.get_id());
                if let Ok(target_guard) = target.read() {
                    let _ = owner_guard.fire_current_weapon_at_object(&*target_guard);
                }

                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(current_victim) = ai_guard.get_current_victim() {
                            if Some(current_victim) != victim_id {
                                if let Some(new_target) =
                                    crate::helpers::TheGameLogic::find_object_by_id(current_victim)
                                {
                                    self.base.set_goal_object(Some(Arc::downgrade(&new_target)));
                                    ai_guard.notify_new_victim_chosen(current_victim);
                                }
                            }
                        }
                    }
                }

                owner_guard.clear_status(ObjectStatusMaskType::from_status(
                    ObjectStatusTypes::IgnoringStealth,
                ));

                if continue_range > 0.0 {
                    let mut should_continue = false;
                    let mut victim_player = None;
                    let mut victim_pos = None;
                    if let Some(target) = self.base.get_machine_goal_object() {
                        if let Ok(target_guard) = target.read() {
                            victim_player = target_guard.get_controlling_player_id();
                            should_continue = target_guard.is_destroyed()
                                || target_guard.is_effectively_dead()
                                || (target_guard.is_kind_of(KindOf::Mine)
                                    && target_guard.test_status(ObjectStatusTypes::Masked));
                        }
                    }

                    if should_continue {
                        if let Some(pos) = owner_guard
                            .get_ai_update_interface()
                            .and_then(|ai| {
                                ai.lock()
                                    .ok()
                                    .and_then(|ai_guard| ai_guard.get_original_victim_pos())
                            })
                            .or_else(|| self.base.get_machine_goal_position())
                        {
                            victim_pos = Some(pos);
                        }

                        if let (Some(pos), Some(player_id)) = (victim_pos, victim_player) {
                            if let Some(partition) = ThePartitionManager::get() {
                                let same_map_status = owner_guard.is_off_map();
                                let last_cmd_source = owner_guard
                                    .get_ai_update_interface()
                                    .and_then(|ai| {
                                        ai.lock()
                                            .ok()
                                            .map(|ai_guard| ai_guard.get_last_command_source())
                                    })
                                    .unwrap_or(CommandSourceType::FromAi);
                                let closest =
                                    partition.get_closest_object(&pos, continue_range, |obj| {
                                        if obj.get_controlling_player_id() != Some(player_id) {
                                            return false;
                                        }
                                        if obj.is_destroyed() || obj.is_effectively_dead() {
                                            return false;
                                        }
                                        if obj.is_off_map() != same_map_status {
                                            return false;
                                        }
                                        match owner_guard.get_able_to_attack_specific_object(
                                            AbleToAttackType::CanAttackSpecific,
                                            obj,
                                            last_cmd_source,
                                        ) {
                                            CanAttackResult::Possible
                                            | CanAttackResult::PossibleAfterMoving => true,
                                            _ => false,
                                        }
                                    });
                                if let Some(new_id) = closest {
                                    if let Some(new_target) =
                                        crate::helpers::TheGameLogic::find_object_by_id(new_id)
                                    {
                                        self.base
                                            .set_goal_object(Some(Arc::downgrade(&new_target)));
                                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                                            if let Ok(mut ai_guard) = ai.lock() {
                                                ai_guard.notify_new_victim_chosen(new_id);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if let Some(pos) = self.base.get_machine_goal_position() {
            let mut fired_any = false;
            let linked = owner_guard
                .get_ai_update_interface()
                .and_then(|ai| ai.lock().ok().map(|ai_guard| ai_guard.are_turrets_linked()))
                .unwrap_or(false);

            if linked {
                for slot_index in 0..crate::common::WEAPONSLOT_COUNT {
                    let slot = match slot_index {
                        0 => WeaponSlotType::Primary,
                        1 => WeaponSlotType::Secondary,
                        _ => WeaponSlotType::Tertiary,
                    };
                    if owner_guard
                        .fire_weapon_in_slot_at_position(slot, &pos)
                        .is_ok()
                    {
                        owner_guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
                        fired_any = true;
                    }
                }
            } else if owner_guard.fire_current_weapon_at_position(&pos).is_ok() {
                fired_any = true;
            }

            if fired_any {
                owner_guard.clear_status(ObjectStatusMaskType::from_status(
                    ObjectStatusTypes::IgnoringStealth,
                ));
            }
        }

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.notify_fired();
            }
        }

        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack fire missing owner".to_string())?;
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "attack fire owner lock poisoned".to_string())?;

        owner_guard.clear_status(ObjectStatusMaskType::from_status(
            ObjectStatusTypes::IsFiringWeapon,
        ));
        owner_guard.clear_status(ObjectStatusMaskType::from_status(
            ObjectStatusTypes::IgnoringStealth,
        ));

        if let Some((weapon, _)) = owner_guard.get_current_weapon() {
            if weapon.get_status() == WeaponStatus::PreAttack {
                owner_guard.cancel_pre_attack_for_current_weapon();
            }
        }

        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

const ATTACK_MIN_RECOMPUTE_TIME: u32 = 10;

fn attack_view_blocked(source: &Object, victim: Option<&Object>, victim_pos: &Coord3D) -> bool {
    THE_AI
        .read()
        .ok()
        .and_then(|ai| {
            let pathfinder = ai.pathfinder()?;
            let guard = pathfinder.read().ok()?;
            Some(guard.is_attack_view_blocked_by_obstacle(
                source,
                source.get_position(),
                victim,
                victim_pos,
            ))
        })
        .unwrap_or(false)
}

fn attack_can_pursue(source: &Object, weapon: &Weapon, victim: &Object) -> bool {
    if victim.get_physics().is_none() {
        return false;
    }

    let Some(ai) = source.get_ai_update_interface() else {
        return false;
    };
    let Ok(ai_guard) = ai.lock() else {
        return false;
    };
    if ai_guard.get_which_turret_for_cur_weapon() == TurretType::Invalid {
        return false;
    }

    let ai_crushes_infantry = THE_AI
        .read()
        .ok()
        .and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|data| data.ai_crushes_infantry)
        })
        .unwrap_or(true);
    if ai_crushes_infantry {
        let is_computer = source
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|player_guard| player_guard.get_player_type() == PlayerType::Computer)
            })
            .unwrap_or(false);
        if is_computer && source.get_crusher_level() > 0 && victim.is_kind_of(KindOf::Infantry) {
            return true;
        }
    }

    if weapon.is_too_close(source.get_id(), Some(victim.get_id()), None) {
        return false;
    }

    let our_max_speed = ai_guard.get_cur_locomotor_speed();
    if our_max_speed <= 0.0 {
        return false;
    }

    let victim_speed = victim
        .get_physics()
        .and_then(|physics| {
            physics.lock().ok().map(|guard| {
                let velocity = guard.get_velocity();
                (velocity.x * velocity.x + velocity.y * velocity.y).sqrt()
            })
        })
        .unwrap_or(0.0);

    if victim_speed >= our_max_speed {
        return false;
    }
    if victim_speed < our_max_speed / 10.0 {
        return false;
    }

    let source_pos = source.get_position();
    let victim_pos = victim.get_position();
    let dx = victim_pos.x - source_pos.x;
    let dy = victim_pos.y - source_pos.y;
    let (victim_dir_x, victim_dir_y) = victim.get_unit_direction_vector_2d();
    if dx * victim_dir_x + dy * victim_dir_y < 0.0 {
        return false;
    }

    true
}

#[derive(Debug)]
pub struct AIAttackPursueTargetState {
    base: AIMoveToState,
    prev_victim_pos: Coord3D,
    approach_timestamp: UnsignedInt,
    follow: Bool,
    attacking_object: Bool,
    stop_if_in_range: Bool,
    is_initial_approach: Bool,
    force_attacking: Bool,
}

impl AIAttackPursueTargetState {
    pub fn new(
        machine: &StateMachine,
        follow: Bool,
        attacking_object: Bool,
        force_attacking: Bool,
    ) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackPursueTargetState".to_string();
        base.is_move_to = false;
        Self {
            base,
            prev_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            approach_timestamp: 0,
            follow,
            attacking_object,
            stop_if_in_range: false,
            is_initial_approach: true,
            force_attacking,
        }
    }

    fn compute_path(&mut self) -> Result<bool, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack pursue missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "attack pursue owner lock poisoned".to_string())?;
        if owner_guard.is_kind_of(KindOf::Immobile) {
            return Ok(false);
        }

        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "attack pursue missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack pursue AI lock poisoned".to_string())?;

        if ai_guard.is_blocked_and_stuck() {
            return Ok(false);
        }
        if self.base.waiting_for_path {
            return Ok(true);
        }

        let mut force_repath = false;
        if ai_guard.get_path().is_none() && !ai_guard.is_waiting_for_path() {
            force_repath = true;
        }
        if !force_repath
            && TheGameLogic::get_frame().saturating_sub(self.approach_timestamp)
                < ATTACK_MIN_RECOMPUTE_TIME
        {
            return Ok(true);
        }

        self.approach_timestamp = TheGameLogic::get_frame();

        let Some(victim) = self.base.base.get_machine_goal_object() else {
            return Ok(false);
        };
        let victim_guard = victim
            .read()
            .map_err(|_| "attack pursue victim lock poisoned".to_string())?;
        if !force_repath
            && self.base.is_same_position(
                owner_guard.get_position(),
                &self.prev_victim_pos,
                victim_guard.get_position(),
            )
        {
            return Ok(true);
        }

        let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
            return Ok(false);
        };
        if !attack_can_pursue(&owner_guard, weapon, &victim_guard) {
            return Ok(false);
        }

        self.prev_victim_pos = *victim_guard.get_position();
        self.base.set_adjusts_destination(true);
        self.base.goal_position = self.prev_victim_pos;
        self.base.waiting_for_path = true;
        ai_guard
            .request_path(&self.base.goal_position, false)
            .map_err(|err| format!("attack pursue request_path failed: {}", err))?;
        self.stop_if_in_range = false;

        Ok(true)
    }

    fn update_internal(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack pursue missing owner".to_string())?;

        if self
            .base
            .base
            .get_machine_goal_object()
            .and_then(|target| target.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(true)
        {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.notify_victim_is_dead();
                    }
                }
            }
            return Ok(StateReturnType::Failure);
        }

        self.stop_if_in_range = false;

        let Some(victim) = self.base.base.get_machine_goal_object() else {
            return Ok(StateReturnType::Failure);
        };
        {
            let victim_guard = victim
                .read()
                .map_err(|_| "attack pursue victim lock poisoned".to_string())?;
            if victim_guard.test_status(ObjectStatusTypes::Stealthed)
                && !victim_guard.test_status(ObjectStatusTypes::Detected)
                && !victim_guard.test_status(ObjectStatusTypes::Disguised)
            {
                return Ok(StateReturnType::Failure);
            }
        }

        if !self.compute_path()? {
            return Ok(StateReturnType::Failure);
        }

        let code = self.base.classic_on_update()?;
        if code != StateReturnType::Continue {
            return Ok(StateReturnType::Success);
        }

        let owner_guard = owner
            .lock()
            .map_err(|_| "attack pursue owner lock poisoned".to_string())?;
        let victim_guard = victim
            .read()
            .map_err(|_| "attack pursue victim lock poisoned".to_string())?;

        let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
            return Ok(StateReturnType::Failure);
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return Ok(StateReturnType::Failure);
        };
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack pursue AI lock poisoned".to_string())?;
        let turret = ai_guard.get_which_turret_for_cur_weapon();
        if turret == TurretType::Invalid {
            return Ok(StateReturnType::Success);
        }

        let view_blocked = attack_view_blocked(
            &owner_guard,
            Some(&victim_guard),
            victim_guard.get_position(),
        );
        if !view_blocked
            && weapon.is_within_attack_range(
                owner_guard.get_id(),
                Some(victim_guard.get_id()),
                None,
            )
        {
            ai_guard.set_turret_target_object(turret, Some(&victim), self.force_attacking);
            self.is_initial_approach = false;

            let mut desired_speed = victim_guard
                .get_physics()
                .and_then(|physics| {
                    physics.lock().ok().map(|guard| {
                        let velocity = guard.get_velocity();
                        (velocity.x * velocity.x + velocity.y * velocity.y).sqrt()
                    })
                })
                .unwrap_or(FAST_AS_POSSIBLE);
            desired_speed *= 0.95;
            ai_guard.set_desired_speed(desired_speed.max(0.0));
        } else {
            ai_guard.set_desired_speed(FAST_AS_POSSIBLE);
        }

        Ok(code)
    }
}

impl StateImplementation for AIAttackPursueTargetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackPursueTargetState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack pursue missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "attack pursue owner lock poisoned".to_string())?;

        if owner_guard.is_kind_of(KindOf::Projectile) {
            return Ok(StateReturnType::Success);
        }
        if self
            .base
            .base
            .get_machine_goal_object()
            .and_then(|target| target.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(true)
        {
            return Ok(StateReturnType::Success);
        }
        if !self.attacking_object {
            return Ok(StateReturnType::Success);
        }

        self.base.set_adjusts_destination(false);

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if ai_guard.get_current_state_id() != Some(AIStateType::GuardRetaliate as u32) {
                    let is_human = owner_guard
                        .get_controlling_player()
                        .and_then(|player| {
                            player.read().ok().map(|player_guard| {
                                player_guard.get_player_type() == PlayerType::Human
                            })
                        })
                        .unwrap_or(false);
                    if is_human && ai_guard.get_last_command_source() == CommandSourceType::FromAi {
                        return Ok(StateReturnType::Success);
                    }
                }
            }
        }

        self.prev_victim_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.approach_timestamp = 0u32.wrapping_sub(ATTACK_MIN_RECOMPUTE_TIME);

        let Some(victim) = self.base.base.get_machine_goal_object() else {
            return Ok(StateReturnType::Success);
        };
        let victim_guard = victim
            .read()
            .map_err(|_| "attack pursue victim lock poisoned".to_string())?;
        let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
            return Ok(StateReturnType::Failure);
        };
        if !attack_can_pursue(&owner_guard, weapon, &victim_guard) {
            return Ok(StateReturnType::Success);
        }
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let turret = ai_guard.get_which_turret_for_cur_weapon();
                if turret == TurretType::Invalid {
                    return Ok(StateReturnType::Success);
                }
                ai_guard.set_turret_target_object(turret, Some(&victim), self.force_attacking);
            }
        }
        drop(victim_guard);
        drop(owner_guard);

        if !self.compute_path()? {
            return Ok(StateReturnType::Success);
        }

        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let code = self.update_internal()?;

        if self.is_initial_approach {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack pursue missing owner".to_string())?;
            {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let turret = ai_guard.get_which_turret_for_cur_weapon();
                            if turret != TurretType::Invalid {
                                if let Some(temporary_target) =
                                    ai_guard.get_next_mood_target(true, false)
                                {
                                    ai_guard.set_turret_target_object(
                                        turret,
                                        Some(&temporary_target),
                                        self.force_attacking,
                                    );
                                }
                            }
                        }
                    }
                }
            };
        }

        Ok(code)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        self.is_initial_approach = false;
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackApproachTargetState {
    base: AIMoveToState,
    prev_victim_pos: Coord3D,
    approach_timestamp: UnsignedInt,
    follow: Bool,
    attacking_object: Bool,
    stop_if_in_range: Bool,
    is_initial_approach: Bool,
    force_attacking: Bool,
}

impl AIAttackApproachTargetState {
    pub fn new(
        machine: &StateMachine,
        follow: Bool,
        attacking_object: Bool,
        force_attacking: Bool,
    ) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackApproachTargetState".to_string();
        base.is_move_to = false;
        Self {
            base,
            prev_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            approach_timestamp: 0,
            follow,
            attacking_object,
            stop_if_in_range: false,
            is_initial_approach: true,
            force_attacking,
        }
    }

    fn compute_path(&mut self) -> Result<bool, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack approach missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "attack approach owner lock poisoned".to_string())?;
        if owner_guard.is_kind_of(KindOf::Immobile) {
            return Ok(false);
        }

        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "attack approach missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack approach AI lock poisoned".to_string())?;

        let mut force_repath = false;
        if ai_guard.is_blocked_and_stuck() {
            force_repath = true;
        }
        if self.base.waiting_for_path {
            return Ok(true);
        }
        if !force_repath && ai_guard.get_path().is_none() && !ai_guard.is_waiting_for_path() {
            force_repath = true;
        }
        if !force_repath
            && TheGameLogic::get_frame().saturating_sub(self.approach_timestamp)
                < ATTACK_MIN_RECOMPUTE_TIME
        {
            return Ok(true);
        }

        self.approach_timestamp = TheGameLogic::get_frame();

        if let Some(victim) = self.base.base.get_machine_goal_object() {
            let victim_guard = victim
                .read()
                .map_err(|_| "attack approach victim lock poisoned".to_string())?;
            if !force_repath
                && self.base.is_same_position(
                    owner_guard.get_position(),
                    &self.prev_victim_pos,
                    victim_guard.get_position(),
                )
            {
                return Ok(true);
            }

            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return Ok(false);
            };

            self.prev_victim_pos = *victim_guard.get_position();
            if attack_can_pursue(&owner_guard, weapon, &victim_guard) {
                return Ok(false);
            }

            self.base.set_adjusts_destination(true);
            if weapon.is_contact_weapon() {
                let _ = ai_guard.ignore_obstacle(Some(&victim));
                self.base.set_adjusts_destination(false);
                let _ = ai_guard.set_path_extra_distance(10.0 * PATHFIND_CELL_SIZE_F);
            }

            self.base.goal_position = self.prev_victim_pos;
            self.base.waiting_for_path = true;
            let victim_center = victim_guard
                .get_geometry_info()
                .get_center_position(victim_guard.get_position());
            ai_guard
                .request_attack_path(victim_guard.get_id(), &victim_center)
                .map_err(|err| format!("attack approach request_attack_path failed: {}", err))?;
            self.stop_if_in_range = false;
            return Ok(true);
        }

        self.base.set_adjusts_destination(true);
        self.stop_if_in_range = false;
        let Some(goal_position) = self.base.base.get_machine_goal_position() else {
            return Ok(false);
        };
        self.base.goal_position = goal_position;
        if !force_repath {
            return Ok(true);
        }
        self.base.waiting_for_path = true;
        ai_guard
            .request_attack_path(INVALID_ID, &self.base.goal_position)
            .map_err(|err| format!("attack approach request_attack_path failed: {}", err))?;
        Ok(true)
    }

    fn update_internal(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack approach missing owner".to_string())?;

        if self
            .base
            .base
            .get_machine_goal_object()
            .and_then(|target| target.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(false)
        {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.notify_victim_is_dead();
                    }
                }
            }
            return Ok(StateReturnType::Failure);
        }

        self.stop_if_in_range = false;

        if let Some(victim) = self.base.base.get_machine_goal_object() {
            {
                let owner_guard = owner
                    .lock()
                    .map_err(|_| "attack approach owner lock poisoned".to_string())?;
                let victim_guard = victim
                    .read()
                    .map_err(|_| "attack approach victim lock poisoned".to_string())?;
                if owner_guard
                    .get_controlling_player()
                    .and_then(|player| {
                        player.read().ok().map(|player_guard| {
                            player_guard.get_player_type() == PlayerType::Computer
                        })
                    })
                    .unwrap_or(false)
                {
                    let hunt = owner_guard
                        .get_ai_update_interface()
                        .and_then(|ai| {
                            ai.lock().ok().map(|ai_guard| {
                                ai_guard.get_current_state_id() == Some(AIStateType::Hunt as u32)
                            })
                        })
                        .unwrap_or(false);
                    if !hunt
                        && victim_guard.is_kind_of(KindOf::Aircraft)
                        && victim_guard.is_airborne_target()
                    {
                        return Ok(StateReturnType::Failure);
                    }
                }
                if victim_guard.test_status(ObjectStatusTypes::Stealthed)
                    && !victim_guard.test_status(ObjectStatusTypes::Detected)
                    && !victim_guard.test_status(ObjectStatusTypes::Disguised)
                {
                    return Ok(StateReturnType::Failure);
                }

                if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                    if weapon.is_contact_weapon()
                        && weapon.is_within_attack_range(
                            owner_guard.get_id(),
                            Some(victim_guard.get_id()),
                            None,
                        )
                    {
                        return Ok(StateReturnType::Success);
                    }
                    if self.stop_if_in_range
                        && weapon.is_within_attack_range(
                            owner_guard.get_id(),
                            Some(victim_guard.get_id()),
                            None,
                        )
                        && !attack_view_blocked(
                            &owner_guard,
                            Some(&victim_guard),
                            victim_guard.get_position(),
                        )
                    {
                        return Ok(StateReturnType::Success);
                    }
                }
            }

            if !self.compute_path()? {
                return Ok(StateReturnType::Success);
            }
            let code = self.base.classic_on_update()?;
            if code != StateReturnType::Continue {
                return Ok(StateReturnType::Success);
            }
            return Ok(code);
        }

        {
            let owner_guard = owner
                .lock()
                .map_err(|_| "attack approach owner lock poisoned".to_string())?;
            if self.stop_if_in_range {
                if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                    if weapon.is_within_attack_range(
                        owner_guard.get_id(),
                        None,
                        Some(&self.base.goal_position),
                    ) && !attack_view_blocked(&owner_guard, None, &self.base.goal_position)
                    {
                        return Ok(StateReturnType::Success);
                    }
                }
            }
        }

        if !self.compute_path()? {
            return Ok(StateReturnType::Failure);
        }
        self.base.classic_on_update()
    }
}

impl StateImplementation for AIAttackApproachTargetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackApproachTargetState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack approach missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "attack approach owner lock poisoned".to_string())?;

        if self
            .base
            .base
            .get_machine_goal_object()
            .and_then(|target| target.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(false)
        {
            return Ok(StateReturnType::Success);
        }

        self.prev_victim_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.approach_timestamp = 0u32.wrapping_sub(ATTACK_MIN_RECOMPUTE_TIME);

        if let Some(victim) = self.base.base.get_machine_goal_object() {
            let victim_guard = victim
                .read()
                .map_err(|_| "attack approach victim lock poisoned".to_string())?;
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return Ok(StateReturnType::Failure);
            };
            if weapon.is_within_attack_range(
                owner_guard.get_id(),
                Some(victim_guard.get_id()),
                None,
            ) && !attack_view_blocked(
                &owner_guard,
                Some(&victim_guard),
                victim_guard.get_position(),
            ) {
                return Ok(StateReturnType::Success);
            }

            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if ai_guard.get_current_state_id() != Some(AIStateType::GuardRetaliate as u32) {
                        let is_human = owner_guard
                            .get_controlling_player()
                            .and_then(|player| {
                                player.read().ok().map(|player_guard| {
                                    player_guard.get_player_type() == PlayerType::Human
                                })
                            })
                            .unwrap_or(false);
                        if is_human
                            && ai_guard.get_last_command_source() == CommandSourceType::FromAi
                            && !weapon.is_contact_weapon()
                        {
                            return Ok(StateReturnType::Failure);
                        }

                        let is_computer = owner_guard
                            .get_controlling_player()
                            .and_then(|player| {
                                player.read().ok().map(|player_guard| {
                                    player_guard.get_player_type() == PlayerType::Computer
                                })
                            })
                            .unwrap_or(false);
                        if is_computer
                            && ai_guard.get_current_state_id() != Some(AIStateType::Hunt as u32)
                            && victim_guard.is_kind_of(KindOf::Aircraft)
                            && victim_guard.is_airborne_target()
                        {
                            return Ok(StateReturnType::Failure);
                        }
                    }
                }
            }

            if attack_can_pursue(&owner_guard, weapon, &victim_guard) {
                return Ok(StateReturnType::Success);
            }
        } else if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let turret = ai_guard.get_which_turret_for_cur_weapon();
                if turret != TurretType::Invalid {
                    if self.attacking_object {
                        if let Some(victim) = self.base.base.get_machine_goal_object() {
                            ai_guard.set_turret_target_object(
                                turret,
                                Some(&victim),
                                self.force_attacking,
                            );
                        }
                    } else if let Some(goal_position) = self.base.base.get_machine_goal_position() {
                        ai_guard.set_turret_target_position(turret, &goal_position);
                    }
                }
            }
        }
        drop(owner_guard);

        if !self.compute_path()? {
            return Ok(StateReturnType::Failure);
        }

        self.base.set_adjusts_destination(false);
        let ret = self.base.classic_on_enter();
        self.base.set_adjusts_destination(true);
        ret
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let mut code = self.update_internal()?;

        if self.follow && self.attacking_object {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack approach missing owner".to_string())?;
            let keep_following = if let Ok(owner_guard) = owner.read() {
                if let Some(victim) = self.base.base.get_machine_goal_object() {
                    if let Ok(victim_guard) = victim.read() {
                        !owner_guard.is_kind_of(KindOf::Immobile)
                            && !victim_guard.is_kind_of(KindOf::Immobile)
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if keep_following {
                if code != StateReturnType::Continue {
                    self.is_initial_approach = false;
                }
                code = StateReturnType::Continue;
            }
        }

        if self.is_initial_approach {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack approach missing owner".to_string())?;
            {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let turret = ai_guard.get_which_turret_for_cur_weapon();
                            if turret != TurretType::Invalid {
                                if let Some(temporary_target) =
                                    ai_guard.get_next_mood_target(true, false)
                                {
                                    ai_guard.set_turret_target_object(
                                        turret,
                                        Some(&temporary_target),
                                        self.force_attacking,
                                    );
                                }
                            }
                        }
                    }
                }
            };
        }

        Ok(code)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;

        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.ignore_obstacle(None);
                        if ai_guard.is_doing_ground_movement() {
                            let dx = self.base.goal_position.x - owner_guard.get_position().x;
                            let dy = self.base.goal_position.y - owner_guard.get_position().y;
                            if dx * dx + dy * dy
                                < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.125
                            {
                                let _ = owner_guard.set_position(&self.base.goal_position);
                            }
                        }
                    }
                }
            }
        }

        self.is_initial_approach = false;
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackMoveStateMachine {
    base: StateMachine,
}

impl AIAttackMoveStateMachine {
    pub fn new(owner: Weak<RwLock<Object>>, name: &str) -> Self {
        let mut base = StateMachine::new(Some(owner), name);
        let idle_state = AIIdleState::new(&base, false);
        register_classic_state(
            &mut base,
            AIStateType::Idle as u32,
            idle_state,
            None,
            None,
            &[],
        );
        let pickup_state = AIPickUpCrateState::new(&base);
        register_classic_state(
            &mut base,
            AIStateType::PickUpCrate as u32,
            pickup_state,
            None,
            None,
            &[],
        );
        let attack_state = AIAttackObjectState::new(&base, false, true);
        register_classic_state(
            &mut base,
            AIStateType::AttackObject as u32,
            attack_state,
            None,
            None,
            &[],
        );
        Self { base }
    }

    pub fn clear(&mut self) {
        self.base.clear();
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: AIStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn set_goal_object(&mut self, obj: Option<Weak<RwLock<Object>>>) {
        self.base.set_goal_object(obj);
    }

    pub fn update(&mut self) -> StateReturnType {
        self.base.update()
    }

    pub fn is_in_idle_state(&self) -> bool {
        self.base.is_in_idle_state()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }
}

impl Snapshotable for AIMoveToState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveToState xfer version failed: {:?}", e))?;

        xfer.xfer_real(&mut self.goal_position.x)
            .map_err(|e| format!("AIMoveToState xfer goal_position.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.y)
            .map_err(|e| format!("AIMoveToState xfer goal_position.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.z)
            .map_err(|e| format!("AIMoveToState xfer goal_position.z failed: {:?}", e))?;
        xfer.xfer_u8(&mut self.goal_layer);
        xfer.xfer_bool(&mut self.waiting_for_path)
            .map_err(|e| format!("AIMoveToState xfer waiting_for_path failed: {:?}", e))?;
        xfer.xfer_real(&mut self.path_goal_position.x)
            .map_err(|e| format!("AIMoveToState xfer path_goal_position.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.path_goal_position.y)
            .map_err(|e| format!("AIMoveToState xfer path_goal_position.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.path_goal_position.z)
            .map_err(|e| format!("AIMoveToState xfer path_goal_position.z failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.path_timestamp)
            .map_err(|e| format!("AIMoveToState xfer path_timestamp failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.blocked_repath_timestamp)
            .map_err(|e| {
                format!(
                    "AIMoveToState xfer blocked_repath_timestamp failed: {:?}",
                    e
                )
            })?;
        xfer.xfer_bool(&mut self.adjust_destinations)
            .map_err(|e| format!("AIMoveToState xfer adjust_destinations failed: {:?}", e))?;

        if xfer.is_loading() {
            self.adjust_destinations_override = None;
            self.repath_limit = None;
            self.ambient_playing_handle = 0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                self.start_move_sound(&owner_guard);
            }
        }
        Ok(())
    }
}

impl Snapshotable for AIWanderInPlaceState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIWanderInPlaceState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_real(&mut self.origin.x)
            .map_err(|e| format!("AIWanderInPlaceState xfer origin.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.y)
            .map_err(|e| format!("AIWanderInPlaceState xfer origin.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.z)
            .map_err(|e| format!("AIWanderInPlaceState xfer origin.z failed: {:?}", e))?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("AIWanderInPlaceState xfer wait_frames failed: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("AIWanderInPlaceState xfer timer failed: {:?}", e))?;

        if xfer.is_loading() {
            self.goal_position = self.base.goal_position;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for AIMoveAndTightenState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndTightenState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_int(&mut self.ok_to_repath_times).map_err(|e| {
            format!(
                "AIMoveAndTightenState xfer ok_to_repath_times failed: {:?}",
                e
            )
        })?;
        xfer.xfer_bool(&mut self.check_for_path)
            .map_err(|e| format!("AIMoveAndTightenState xfer check_for_path failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for AIMoveAndDeleteState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndDeleteState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for AIMoveAndEvacuateState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_real(&mut self.origin.x)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer origin.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.y)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer origin.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.z)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer origin.z failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for AIAttackObjectState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack object has_machine: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.original_victim_pos);

        if xfer.is_loading() && has_machine && self.attack_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack object state missing machine owner".to_string())?;
            let mut machine = AttackStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
                self.follow_target,
                true,
                self.force_attack,
            );
            if let Some(target) = self.target.as_ref() {
                machine.set_goal_object(Some(target));
            }
            self.attack_machine = Some(machine);
        }

        if let Some(machine) = self.attack_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackPositionState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack position has_machine: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.target_position);

        if xfer.is_loading() && has_machine && self.attack_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack position state missing machine owner".to_string())?;
            let mut machine = AttackStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
                false,
                false,
                false,
            );
            machine.set_goal_position(self.target_position);
            self.attack_machine = Some(machine);
        }

        if let Some(machine) = self.attack_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackMoveToState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;

        let mut has_machine = self.attack_move_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack move has_machine: {:?}", e))?;
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.frame_to_sleep_until)
                .map_err(|e| format!("Failed to xfer frame_to_sleep_until: {:?}", e))?;
            xfer.xfer_int(&mut self.retry_count)
                .map_err(|e| format!("Failed to xfer retry_count: {:?}", e))?;
        }

        if xfer.is_loading() && has_machine && self.attack_move_machine.is_none() {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack move-to missing machine owner".to_string())?;
            self.attack_move_machine = Some(AIAttackMoveStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMoveMachine",
            ));
        }

        if let Some(machine) = self.attack_move_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_move_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackPursueTargetState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_coord3d(&mut self.prev_victim_pos);
        xfer.xfer_unsigned_int(&mut self.approach_timestamp)
            .map_err(|e| format!("Failed to xfer pursue approach_timestamp: {:?}", e))?;
        xfer.xfer_bool(&mut self.follow)
            .map_err(|e| format!("Failed to xfer pursue follow: {:?}", e))?;
        xfer.xfer_bool(&mut self.attacking_object)
            .map_err(|e| format!("Failed to xfer pursue attacking_object: {:?}", e))?;
        xfer.xfer_bool(&mut self.stop_if_in_range)
            .map_err(|e| format!("Failed to xfer pursue stop_if_in_range: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_initial_approach)
            .map_err(|e| format!("Failed to xfer pursue is_initial_approach: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIAttackApproachTargetState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_coord3d(&mut self.prev_victim_pos);
        xfer.xfer_unsigned_int(&mut self.approach_timestamp)
            .map_err(|e| format!("Failed to xfer approach approach_timestamp: {:?}", e))?;
        xfer.xfer_bool(&mut self.follow)
            .map_err(|e| format!("Failed to xfer approach follow: {:?}", e))?;
        xfer.xfer_bool(&mut self.attacking_object)
            .map_err(|e| format!("Failed to xfer approach attacking_object: {:?}", e))?;
        xfer.xfer_bool(&mut self.stop_if_in_range)
            .map_err(|e| format!("Failed to xfer approach stop_if_in_range: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_initial_approach)
            .map_err(|e| format!("Failed to xfer approach is_initial_approach: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIAttackAimAtTargetState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_bool(&mut self.can_turn_in_place)
            .map_err(|e| format!("Failed to xfer can_turn_in_place: {:?}", e))?;
        xfer.xfer_bool(&mut self.set_locomotor)
            .map_err(|e| format!("Failed to xfer set_locomotor: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsTeamState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsIndividualsState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsTeamExactState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut id: WaypointId = self
            .last_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("Failed to xfer team waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.last_waypoint = if id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(id)
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsIndividualsExactState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut id: WaypointId = self
            .last_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("Failed to xfer individual waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.last_waypoint = if id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(id)
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIAttackFollowWaypointPathAsTeamState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.base.xfer(xfer)?;

        let mut has_machine = self.attack_follow_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack-follow-team has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.attack_follow_machine.is_none() {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
            self.attack_follow_machine = Some(AIAttackMoveStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackFollowMachine",
            ));
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackFollowWaypointPathAsIndividualsState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.base.xfer(xfer)?;

        let mut has_machine = self.attack_follow_machine.is_some();
        xfer.xfer_bool(&mut has_machine).map_err(|e| {
            format!(
                "Failed to xfer attack-follow-individuals has_machine: {:?}",
                e
            )
        })?;

        if xfer.is_loading() && has_machine && self.attack_follow_machine.is_none() {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
            self.attack_follow_machine = Some(AIAttackMoveStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackFollowMachine",
            ));
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackMoveStateMachine {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        self.base
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer attack move machine: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("Failed to load post process attack move machine: {:?}", e))
    }
}

impl Snapshotable for AIAttackThenIdleStateMachine {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        self.base
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer attack-then-idle machine: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process().map_err(|e| {
            format!(
                "Failed to load post process attack-then-idle machine: {:?}",
                e
            )
        })
    }
}

impl Snapshotable for AIAttackSquadState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_squad_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack-squad has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.attack_squad_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack squad missing owner".to_string())?;
            self.attack_squad_machine = Some(AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
            ));
        }

        if let Some(machine) = self.attack_squad_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_squad_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackAreaState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack-area has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.attack_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack area missing owner".to_string())?;
            self.attack_machine = Some(AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackThenIdleStateMachine",
            ));
        }

        if let Some(machine) = self.attack_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| format!("Failed to xfer next_enemy_scan_time: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIGuardState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer guard has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.guard_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "guard state missing machine owner".to_string())?;
            self.guard_machine = Some(AIGuardMachine::new(Arc::downgrade(&owner)));
        }

        if let Some(machine) = self.guard_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.guard_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIGuardRetaliateState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer guard retaliate has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.guard_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "guard retaliate state missing machine owner".to_string())?;
            self.guard_machine = Some(AIGuardRetaliateMachine::new(Arc::downgrade(&owner)));
        }

        if let Some(machine) = self.guard_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.guard_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AITunnelNetworkGuardState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer tunnel-guard has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.guard_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "tunnel network guard state missing machine owner".to_string())?;
            self.guard_machine = Some(AITNGuardMachine::new(Arc::downgrade(&owner)));
        }

        if let Some(machine) = self.guard_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.guard_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIIdleState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_unsigned_short(&mut self.initial_sleep_offset)
            .map_err(|e| format!("Failed to xfer initial_sleep_offset: {:?}", e))?;
        xfer.xfer_bool(&mut self.should_look_for_targets)
            .map_err(|e| format!("Failed to xfer should_look_for_targets: {:?}", e))?;
        xfer.xfer_bool(&mut self.inited)
            .map_err(|e| format!("Failed to xfer inited: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIWanderState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("Failed to xfer wait_frames: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("Failed to xfer timer: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIPanicState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("Failed to xfer wait_frames: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("Failed to xfer timer: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIHuntState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.hunt_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer hunt has_machine: {:?}", e))?;

        if xfer.is_loading() && !has_machine {
            self.hunt_machine = None;
        } else if xfer.is_loading() && has_machine && self.hunt_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "hunt state missing machine owner".to_string())?;
            self.hunt_machine = Some(AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackThenIdleStateMachine",
            ));
        }

        if let Some(machine) = self.hunt_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| format!("Failed to xfer next_enemy_scan_time: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.hunt_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIExitState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_object_id(&mut self.entry_to_clear)
            .map_err(|e| format!("Failed to xfer entry_to_clear: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIExitInstantlyState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_object_id(&mut self.entry_to_clear)
            .map_err(|e| format!("Failed to xfer entry_to_clear: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIDockState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.dock_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer dock has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.dock_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "dock state missing machine owner".to_string())?;
            self.dock_machine = Some(AIDockMachine::new(owner)?);
        }

        if let Some(machine) = self.dock_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        xfer.xfer_bool(&mut self.using_precision_movement)
            .map_err(|e| format!("Failed to xfer precision movement: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.dock_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIEnterState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        if version >= 2 {
            self.base.xfer(xfer)?;
        }

        xfer.xfer_object_id(&mut self.entry_to_clear)
            .map_err(|e| format!("Failed to xfer entry_to_clear: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.goal_position);
        if xfer.is_loading() {
            self.base.goal_position = self.goal_position;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for AIFollowPathState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut index = self.index as i32;
        xfer.xfer_int(&mut index)
            .map_err(|e| format!("Failed to xfer follow path index: {:?}", e))?;
        if xfer.is_loading() {
            self.index = index.max(0) as usize;
        }

        xfer.xfer_bool(&mut self.adjust_final)
            .map_err(|e| format!("Failed to xfer follow path adjust_final: {:?}", e))?;
        xfer.xfer_bool(&mut self.adjust_final_override)
            .map_err(|e| format!("Failed to xfer follow path adjust_final_override: {:?}", e))?;
        xfer.xfer_int(&mut self.retry_count)
            .map_err(|e| format!("Failed to xfer follow path retry_count: {:?}", e))?;

        let mut path_len = self.path.len() as i32;
        xfer.xfer_int(&mut path_len)
            .map_err(|e| format!("Failed to xfer follow path length: {:?}", e))?;
        if xfer.is_loading() {
            self.path.clear();
        }
        for idx in 0..path_len.max(0) {
            let mut pos = if xfer.is_loading() {
                Coord3D::new(0.0, 0.0, 0.0)
            } else {
                self.path
                    .get(idx as usize)
                    .copied()
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0))
            };
            xfer.xfer_coord3d(&mut pos);
            if xfer.is_loading() {
                self.path.push(pos);
            }
        }

        if version >= 2 {
            let mut has_ignore_object = self.ignore_object_id.is_some();
            xfer.xfer_bool(&mut has_ignore_object)
                .map_err(|e| format!("Failed to xfer follow path has_ignore_object: {:?}", e))?;
            let mut ignore_object_id = self.ignore_object_id.unwrap_or(crate::common::INVALID_ID);
            xfer.xfer_object_id(&mut ignore_object_id)
                .map_err(|e| format!("Failed to xfer follow path ignore_object_id: {:?}", e))?;
            if xfer.is_loading() {
                self.ignore_object_id =
                    if has_ignore_object && ignore_object_id != crate::common::INVALID_ID {
                        Some(ignore_object_id)
                    } else {
                        None
                    };
            }
        } else if xfer.is_loading() {
            self.ignore_object_id = None;
        }

        if xfer.is_loading() && self.index > self.path.len() {
            self.index = self.path.len();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIPickUpCrateState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_int(&mut self.delay_counter)
            .map_err(|e| format!("Failed to xfer pick up crate delay_counter: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.goal_position);

        if xfer.is_loading() {
            self.base.goal_position = self.goal_position;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIRappelIntoState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_bool(&mut self.issued_command)
            .map_err(|e| format!("Failed to xfer rappel issued_command: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AICombatDropState {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_bool(&mut self.issued_command)
            .map_err(|e| format!("Failed to xfer combat drop issued_command: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod ai_state_machine_parity_tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
    use std::sync::{Mutex as StdMutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static TEST_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
        TEST_LOCK
            .get_or_init(|| StdMutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn set_frame(frame: u64) {
        let mut logic = crate::system::game_logic::get_game_logic()
            .lock()
            .expect("game logic lock poisoned");
        logic.set_current_frame(frame);
    }

    fn unique_missing_waypoint_name() -> String {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        format!("__codex_missing_waypoint_{id}__")
    }

    fn unique_polygon_trigger_id() -> i32 {
        static NEXT_ID: AtomicI32 = AtomicI32::new(1_000_000);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    #[test]
    fn add_to_goal_path_deduplicates_terminal_point() {
        let _guard = test_guard();
        let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-path");
        let p0 = Coord3D::new(1.0, 2.0, 3.0);
        let p1 = Coord3D::new(4.0, 5.0, 6.0);

        machine.add_to_goal_path(&p0);
        machine.add_to_goal_path(&p0);
        machine.add_to_goal_path(&p1);

        assert_eq!(machine.get_goal_path_size(), 2);
        assert_eq!(machine.get_goal_path_position(0), Some(&p0));
        assert_eq!(machine.get_goal_path_position(1), Some(&p1));
    }

    #[test]
    fn set_state_returns_base_state_machine_result() {
        let _guard = test_guard();
        let mut expected_machine =
            AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-state-expected");
        let mut actual_machine =
            AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-state-actual");

        let expected = expected_machine
            .base
            .set_current_state(MACHINE_DONE_STATE_ID);
        let actual = actual_machine.set_state(MACHINE_DONE_STATE_ID);

        assert_eq!(actual, expected);
    }

    #[test]
    fn clear_uses_base_clear_semantics() {
        let _guard = test_guard();
        let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-clear");
        assert!(machine.get_current_state_id().is_some());

        machine.clear();

        assert_eq!(machine.get_current_state_id(), None);
        assert_eq!(machine.get_goal_path_size(), 0);
    }

    #[test]
    fn set_goal_squad_copies_instead_of_aliasing() {
        let _guard = test_guard();
        let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-squad");
        let source = Arc::new(Mutex::new(Squad::new()));

        machine.set_goal_squad(Some(source.clone()));

        let stored = machine
            .get_goal_squad()
            .expect("goal squad should be set")
            .clone();
        assert!(!Arc::ptr_eq(&stored, &source));
    }

    #[test]
    fn waypoint_load_with_missing_name_clears_existing_goal_waypoint() {
        let _guard = test_guard();
        let missing_name = unique_missing_waypoint_name();

        let mut source = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-waypoint-source");
        source.set_goal_waypoint(Some(Arc::new(Waypoint::new(
            777_001,
            Coord3D::new(10.0, 20.0, 30.0),
            missing_name.clone(),
        ))));

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer(&mut saver)
                .expect("source state machine should serialize");
        }

        let mut loaded = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-waypoint-loaded");
        loaded.set_goal_waypoint(Some(Arc::new(Waypoint::new(
            777_002,
            Coord3D::new(-1.0, -2.0, -3.0),
            "stale-waypoint".to_string(),
        ))));
        assert!(loaded.get_goal_waypoint().is_some());

        let bytes = save_cursor.into_inner();
        let mut loader = XferLoad::new(Cursor::new(bytes), 1);
        loaded
            .xfer(&mut loader)
            .expect("loaded state machine should deserialize");

        assert!(loaded.get_goal_waypoint().is_none());
        assert_eq!(loaded.base.get_goal_waypoint(), None);
    }

    #[test]
    fn ai_do_command_polygon_updates_machine_goal_polygon() {
        let _guard = test_guard();
        let trigger_id = unique_polygon_trigger_id();
        let trigger_name = format!("__codex_test_trigger_{trigger_id}__");
        let trigger = PolygonTrigger::new(
            trigger_id,
            AsciiString::from(trigger_name.as_str()),
            vec![
                ICoord3D::new(0, 0, 0),
                ICoord3D::new(20, 0, 0),
                ICoord3D::new(20, 20, 0),
            ],
        );
        {
            let mut terrain = get_terrain_logic()
                .write()
                .expect("terrain logic write lock poisoned");
            terrain.add_trigger_area(trigger);
        }

        let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-polygon");
        let mut params =
            AiCommandParams::new(AiCommandType::GuardArea, CommandSourceType::FromScript);
        params.polygon = Some(trigger_id);

        machine
            .ai_do_command(&params)
            .expect("ai_do_command should succeed");

        assert_eq!(
            machine
                .goal_polygon
                .as_ref()
                .map(|polygon| polygon.get_id()),
            Some(trigger_id)
        );
        assert_eq!(
            machine
                .base
                .get_goal_polygon()
                .as_ref()
                .map(|polygon| polygon.get_id()),
            Some(trigger_id)
        );
    }

    #[test]
    fn xfer_roundtrip_preserves_path_squad_temp_and_waypoint_lookup_rules() {
        let _guard = test_guard();
        set_frame(1_000);

        let missing_name = unique_missing_waypoint_name();
        let mut source = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-roundtrip-source");
        let path = vec![Coord3D::new(1.0, 2.0, 3.0), Coord3D::new(4.0, 5.0, 6.0)];
        source.set_goal_path(&path);
        source.set_goal_squad(Some(Arc::new(Mutex::new(Squad::new()))));
        source.set_goal_waypoint(Some(Arc::new(Waypoint::new(
            777_010,
            Coord3D::new(30.0, 40.0, 50.0),
            missing_name,
        ))));

        let temp_ret = source.set_temporary_state(AIStateType::Idle as u32, 45);
        assert_eq!(temp_ret, StateReturnType::Continue);
        let expected_temp_end = source.temporary_state_frame_end;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer(&mut saver)
                .expect("source state machine should serialize");
        }

        let mut loaded = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-roundtrip-loaded");
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer(&mut loader)
            .expect("loaded state machine should deserialize");

        assert_eq!(loaded.get_goal_path_size(), path.len());
        assert_eq!(loaded.get_goal_path_position(0), Some(&path[0]));
        assert_eq!(loaded.get_goal_path_position(1), Some(&path[1]));
        assert!(loaded.get_goal_squad().is_some());
        assert!(loaded.base.get_goal_squad().is_some());
        assert_eq!(loaded.get_temporary_state(), Some(AIStateType::Idle as u32));
        assert_eq!(loaded.temporary_state_frame_end, expected_temp_end);
        assert!(loaded.get_goal_waypoint().is_none());
        assert_eq!(loaded.base.get_goal_waypoint(), None);
    }

    #[test]
    fn follow_path_state_snapshot_roundtrip_preserves_runtime_fields() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "follow-path-source");
        let mut source = AIFollowPathState::new(&source_machine, false);
        source.path = vec![
            Coord3D::new(2.0, 3.0, 4.0),
            Coord3D::new(5.0, 6.0, 7.0),
            Coord3D::new(8.0, 9.0, 10.0),
        ];
        source.index = 2;
        source.adjust_final = false;
        source.adjust_final_override = false;
        source.retry_count = 3;
        source.ignore_object_id = Some(4242);

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("follow path state should serialize");
        }

        let load_machine = StateMachine::new(None, "follow-path-loaded");
        let mut loaded = AIFollowPathState::new(&load_machine, false);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("follow path state should deserialize");

        assert_eq!(loaded.path.len(), 3);
        assert_eq!(loaded.path[0], Coord3D::new(2.0, 3.0, 4.0));
        assert_eq!(loaded.path[2], Coord3D::new(8.0, 9.0, 10.0));
        assert_eq!(loaded.index, 2);
        assert!(!loaded.adjust_final);
        assert!(!loaded.adjust_final_override);
        assert_eq!(loaded.retry_count, 3);
        assert_eq!(loaded.ignore_object_id, Some(4242));
    }

    #[test]
    fn follow_exit_production_path_snapshot_delegates_to_base_state() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "follow-exit-source");
        let mut source = AIFollowExitProductionPathState::new(&source_machine);
        source.base.path = vec![
            Coord3D::new(11.0, 12.0, 13.0),
            Coord3D::new(14.0, 15.0, 16.0),
        ];
        source.base.index = 1;
        source.base.ignore_object_id = Some(99);

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("follow exit production state should serialize");
        }

        let load_machine = StateMachine::new(None, "follow-exit-loaded");
        let mut loaded = AIFollowExitProductionPathState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("follow exit production state should deserialize");

        assert_eq!(loaded.base.path.len(), 2);
        assert_eq!(loaded.base.path[1], Coord3D::new(14.0, 15.0, 16.0));
        assert_eq!(loaded.base.index, 1);
        assert_eq!(loaded.base.ignore_object_id, Some(99));
    }

    #[test]
    fn pick_up_crate_state_snapshot_roundtrip_preserves_delay_and_goal() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "pickup-crate-source");
        let mut source = AIPickUpCrateState::new(&source_machine);
        source.delay_counter = 2;
        source.goal_position = Coord3D::new(21.0, 22.0, 23.0);
        source.base.goal_position = Coord3D::new(1.0, 1.0, 1.0);

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("pick up crate state should serialize");
        }

        let load_machine = StateMachine::new(None, "pickup-crate-loaded");
        let mut loaded = AIPickUpCrateState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("pick up crate state should deserialize");

        assert_eq!(loaded.delay_counter, 2);
        assert_eq!(loaded.goal_position, Coord3D::new(21.0, 22.0, 23.0));
        assert_eq!(loaded.base.goal_position, Coord3D::new(21.0, 22.0, 23.0));
    }

    #[test]
    fn attack_pursue_state_snapshot_roundtrip_preserves_base_and_runtime_fields() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "attack-pursue-source");
        let mut source = AIAttackPursueTargetState::new(&source_machine, true, true, true);
        source.base.goal_position = Coord3D::new(30.0, 31.0, 32.0);
        source.base.path_goal_position = Coord3D::new(33.0, 34.0, 35.0);
        source.base.path_timestamp = 123;
        source.base.blocked_repath_timestamp = 456;
        source.base.adjust_destinations = false;
        source.base.waiting_for_path = true;
        source.base.goal_layer = 2;
        source.prev_victim_pos = Coord3D::new(40.0, 41.0, 42.0);
        source.approach_timestamp = 789;
        source.follow = false;
        source.attacking_object = false;
        source.stop_if_in_range = true;
        source.is_initial_approach = false;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("attack pursue state should serialize");
        }

        let load_machine = StateMachine::new(None, "attack-pursue-loaded");
        let mut loaded = AIAttackPursueTargetState::new(&load_machine, true, true, false);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("attack pursue state should deserialize");

        assert_eq!(loaded.base.goal_position, Coord3D::new(30.0, 31.0, 32.0));
        assert_eq!(
            loaded.base.path_goal_position,
            Coord3D::new(33.0, 34.0, 35.0)
        );
        assert_eq!(loaded.base.path_timestamp, 123);
        assert_eq!(loaded.base.blocked_repath_timestamp, 456);
        assert!(!loaded.base.adjust_destinations);
        assert!(loaded.base.waiting_for_path);
        assert_eq!(loaded.base.goal_layer, 2);
        assert_eq!(loaded.prev_victim_pos, Coord3D::new(40.0, 41.0, 42.0));
        assert_eq!(loaded.approach_timestamp, 789);
        assert!(!loaded.follow);
        assert!(!loaded.attacking_object);
        assert!(loaded.stop_if_in_range);
        assert!(!loaded.is_initial_approach);
        assert!(!loaded.force_attacking);
    }

    #[test]
    fn attack_approach_state_snapshot_roundtrip_preserves_base_and_runtime_fields() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "attack-approach-source");
        let mut source = AIAttackApproachTargetState::new(&source_machine, true, true, true);
        source.base.goal_position = Coord3D::new(50.0, 51.0, 52.0);
        source.base.path_goal_position = Coord3D::new(53.0, 54.0, 55.0);
        source.base.path_timestamp = 223;
        source.base.blocked_repath_timestamp = 556;
        source.base.adjust_destinations = false;
        source.base.waiting_for_path = true;
        source.base.goal_layer = 1;
        source.prev_victim_pos = Coord3D::new(60.0, 61.0, 62.0);
        source.approach_timestamp = 889;
        source.follow = false;
        source.attacking_object = false;
        source.stop_if_in_range = true;
        source.is_initial_approach = false;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("attack approach state should serialize");
        }

        let load_machine = StateMachine::new(None, "attack-approach-loaded");
        let mut loaded = AIAttackApproachTargetState::new(&load_machine, true, true, false);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("attack approach state should deserialize");

        assert_eq!(loaded.base.goal_position, Coord3D::new(50.0, 51.0, 52.0));
        assert_eq!(
            loaded.base.path_goal_position,
            Coord3D::new(53.0, 54.0, 55.0)
        );
        assert_eq!(loaded.base.path_timestamp, 223);
        assert_eq!(loaded.base.blocked_repath_timestamp, 556);
        assert!(!loaded.base.adjust_destinations);
        assert!(loaded.base.waiting_for_path);
        assert_eq!(loaded.base.goal_layer, 1);
        assert_eq!(loaded.prev_victim_pos, Coord3D::new(60.0, 61.0, 62.0));
        assert_eq!(loaded.approach_timestamp, 889);
        assert!(!loaded.follow);
        assert!(!loaded.attacking_object);
        assert!(loaded.stop_if_in_range);
        assert!(!loaded.is_initial_approach);
        assert!(!loaded.force_attacking);
    }

    #[test]
    fn attack_aim_state_snapshot_roundtrip_preserves_runtime_flags() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "attack-aim-source");
        let mut source = AIAttackAimAtTargetState::new(&source_machine, true, true);
        source.can_turn_in_place = true;
        source.set_locomotor = true;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("attack aim state should serialize");
        }

        let load_machine = StateMachine::new(None, "attack-aim-loaded");
        let mut loaded = AIAttackAimAtTargetState::new(&load_machine, true, false);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("attack aim state should deserialize");

        assert!(loaded.can_turn_in_place);
        assert!(loaded.set_locomotor);
        assert!(!loaded.force_attacking);
        assert!(loaded.attacking_object);
    }

    #[test]
    fn idle_state_snapshot_roundtrip_preserves_runtime_flags() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "idle-source");
        let mut source = AIIdleState::new(&source_machine, false);
        source.initial_sleep_offset = 17;
        source.should_look_for_targets = true;
        source.inited = true;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("idle state should serialize");
        }

        let load_machine = StateMachine::new(None, "idle-loaded");
        let mut loaded = AIIdleState::new(&load_machine, false);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("idle state should deserialize");

        assert_eq!(loaded.initial_sleep_offset, 17);
        assert!(loaded.should_look_for_targets);
        assert!(loaded.inited);
    }

    #[test]
    fn wander_and_panic_state_snapshot_roundtrip_preserves_runtime_fields() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "wander-source");

        let mut wander = AIWanderState::new(&source_machine);
        wander.wait_frames = 23;
        wander.timer = -4;
        wander.core.group_offset = Coord2D::new(7.0, 8.0);
        wander.core.angle = 1.5;
        wander.core.frames_sleeping = 9;
        wander.core.append_goal_position = true;
        wander.core.goal_position = Coord3D::new(10.0, 11.0, 12.0);

        let mut wander_save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut wander_save_cursor, 1);
            wander
                .xfer_snapshot(&mut saver)
                .expect("wander state should serialize");
        }

        let load_machine = StateMachine::new(None, "wander-loaded");
        let mut wander_loaded = AIWanderState::new(&load_machine);
        let mut wander_loader = XferLoad::new(Cursor::new(wander_save_cursor.into_inner()), 1);
        wander_loaded
            .xfer_snapshot(&mut wander_loader)
            .expect("wander state should deserialize");

        assert_eq!(wander_loaded.wait_frames, 23);
        assert_eq!(wander_loaded.timer, -4);
        assert_eq!(wander_loaded.core.group_offset, Coord2D::new(7.0, 8.0));
        assert_eq!(wander_loaded.core.angle, 1.5);
        assert_eq!(wander_loaded.core.frames_sleeping, 9);
        assert!(wander_loaded.core.append_goal_position);
        assert_eq!(
            wander_loaded.core.goal_position,
            Coord3D::new(10.0, 11.0, 12.0)
        );

        let mut panic_state = AIPanicState::new(&source_machine);
        panic_state.wait_frames = 12;
        panic_state.timer = 6;
        panic_state.core.group_offset = Coord2D::new(-3.0, 4.0);
        panic_state.core.frames_sleeping = 2;

        let mut panic_save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut panic_save_cursor, 1);
            panic_state
                .xfer_snapshot(&mut saver)
                .expect("panic state should serialize");
        }

        let mut panic_loaded = AIPanicState::new(&load_machine);
        let mut panic_loader = XferLoad::new(Cursor::new(panic_save_cursor.into_inner()), 1);
        panic_loaded
            .xfer_snapshot(&mut panic_loader)
            .expect("panic state should deserialize");

        assert_eq!(panic_loaded.wait_frames, 12);
        assert_eq!(panic_loaded.timer, 6);
        assert_eq!(panic_loaded.core.group_offset, Coord2D::new(-3.0, 4.0));
        assert_eq!(panic_loaded.core.frames_sleeping, 2);
    }

    #[test]
    fn exit_states_snapshot_roundtrip_preserves_entry_to_clear() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "exit-source");

        let mut exit_state = AIExitState::new(&source_machine);
        exit_state.entry_to_clear = 1337;
        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            exit_state
                .xfer_snapshot(&mut saver)
                .expect("exit state should serialize");
        }
        let load_machine = StateMachine::new(None, "exit-loaded");
        let mut loaded = AIExitState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("exit state should deserialize");
        assert_eq!(loaded.entry_to_clear, 1337);

        let mut exit_instantly_state = AIExitInstantlyState::new(&source_machine);
        exit_instantly_state.entry_to_clear = 7331;
        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            exit_instantly_state
                .xfer_snapshot(&mut saver)
                .expect("exit instantly state should serialize");
        }
        let mut loaded = AIExitInstantlyState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("exit instantly state should deserialize");
        assert_eq!(loaded.entry_to_clear, 7331);
    }

    #[test]
    fn hunt_state_snapshot_roundtrip_preserves_scan_time_without_machine() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "hunt-source");
        let mut source = AIHuntState::new(&source_machine);
        source.next_enemy_scan_time = 54321;
        source.hunt_machine = None;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("hunt state should serialize");
        }

        let load_machine = StateMachine::new(None, "hunt-loaded");
        let mut loaded = AIHuntState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("hunt state should deserialize");

        assert_eq!(loaded.next_enemy_scan_time, 54321);
        assert!(loaded.hunt_machine.is_none());
    }

    #[test]
    fn enter_state_snapshot_roundtrip_preserves_base_and_entry_runtime_fields() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "enter-source");
        let mut source = AIEnterState::new(&source_machine);
        source.base.goal_position = Coord3D::new(70.0, 71.0, 72.0);
        source.base.path_goal_position = Coord3D::new(73.0, 74.0, 75.0);
        source.base.path_timestamp = 612;
        source.base.blocked_repath_timestamp = 913;
        source.base.waiting_for_path = true;
        source.base.adjust_destinations = false;
        source.base.goal_layer = 1;
        source.entry_to_clear = 17;
        source.goal_position = Coord3D::new(81.0, 82.0, 83.0);

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("enter state should serialize");
        }

        let load_machine = StateMachine::new(None, "enter-loaded");
        let mut loaded = AIEnterState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("enter state should deserialize");

        assert_eq!(loaded.base.goal_position, Coord3D::new(81.0, 82.0, 83.0));
        assert_eq!(
            loaded.base.path_goal_position,
            Coord3D::new(73.0, 74.0, 75.0)
        );
        assert_eq!(loaded.base.path_timestamp, 612);
        assert_eq!(loaded.base.blocked_repath_timestamp, 913);
        assert!(loaded.base.waiting_for_path);
        assert!(!loaded.base.adjust_destinations);
        assert_eq!(loaded.base.goal_layer, 1);
        assert_eq!(loaded.entry_to_clear, 17);
        assert_eq!(loaded.goal_position, Coord3D::new(81.0, 82.0, 83.0));
    }

    #[test]
    fn rappel_into_state_snapshot_roundtrip_preserves_issued_command() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "rappel-source");
        let mut source = AIRappelIntoState::new(&source_machine);
        source.issued_command = true;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("rappel state should serialize");
        }

        let load_machine = StateMachine::new(None, "rappel-loaded");
        let mut loaded = AIRappelIntoState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("rappel state should deserialize");

        assert!(loaded.issued_command);
    }

    #[test]
    fn combat_drop_state_snapshot_roundtrip_preserves_issued_command() {
        let _guard = test_guard();
        let source_machine = StateMachine::new(None, "combat-drop-source");
        let mut source = AICombatDropState::new(&source_machine);
        source.issued_command = true;

        let mut save_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut saver = XferSave::new(&mut save_cursor, 1);
            source
                .xfer_snapshot(&mut saver)
                .expect("combat drop state should serialize");
        }

        let load_machine = StateMachine::new(None, "combat-drop-loaded");
        let mut loaded = AICombatDropState::new(&load_machine);
        let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
        loaded
            .xfer_snapshot(&mut loader)
            .expect("combat drop state should deserialize");

        assert!(loaded.issued_command);
    }

    #[test]
    fn temporary_state_frame_end_uses_saturating_add() {
        let _guard = test_guard();
        set_frame((u32::MAX as u64).saturating_sub(10));

        let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-temp");
        let ret = machine.set_temporary_state(AIStateType::Idle as u32, u32::MAX);

        assert_eq!(ret, StateReturnType::Continue);
        assert_eq!(machine.temporary_state_frame_end, u32::MAX);
    }
}
