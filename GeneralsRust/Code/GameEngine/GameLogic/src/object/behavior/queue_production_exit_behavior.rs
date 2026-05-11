//! Queue Production Exit Behavior Module
//!
//! Complete C++ port of QueueProductionExitUpdate.cpp from GeneralsMD
//! This module handles unit exit behavior where units must wait in a queue
//! before exiting the production building.
//!
//! # C++ Source Reference
//! File: /GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp
//! Lines: 1-330
//!
//! # Key Features
//! - Sequential unit exit (one at a time)
//! - Exit delay between units
//! - Burst mode (rapid initial exit)
//! - Rally point support
//! - Natural rally point calculation
//! - Exit path generation
//! - Airborne creation support

use crate::ai::THE_AI;
use crate::common::*;
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, ExitDoorType as ModuleExitDoorType,
    ExitInterface as ModuleExitInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, Thing as ModuleThing};
use std::any::Any;
use std::sync::{Arc, RwLock};

/// Exit door type
/// Matches C++ ExitDoorType
pub type ExitDoorType = i32;

/// Door constants
pub const DOOR_1: ExitDoorType = 0;
pub const DOOR_NONE_AVAILABLE: ExitDoorType = -1;

/// Queue Production Exit Module Data (configuration)
/// Matches C++ QueueProductionExitUpdateModuleData
#[derive(Debug, Clone)]
pub struct QueueProductionExitModuleData {
    pub base: BehaviorModuleData,
    /// Unit creation point in model space (relative to building)
    pub unit_create_point: Coord3D,
    /// Natural rally point in model space
    pub natural_rally_point: Coord3D,
    /// Delay between unit exits (in frames)
    pub exit_delay: u32,
    /// Initial burst count (units that can exit rapidly at start)
    pub initial_burst: u32,
    /// Allow airborne creation (units spawn in air)
    pub allow_airborne_creation: bool,
}

impl QueueProductionExitModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            unit_create_point: Coord3D::new(0.0, 0.0, 0.0),
            natural_rally_point: Coord3D::new(0.0, 0.0, 0.0),
            exit_delay: 0,
            initial_burst: 0,
            allow_airborne_creation: false,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, QUEUE_PRODUCTION_EXIT_FIELDS)
    }
}

impl Default for QueueProductionExitModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(QueueProductionExitModuleData, base);

/// Queue Production Exit Behavior Module
/// Matches C++ QueueProductionExitUpdate class
#[derive(Debug)]
pub struct QueueProductionExitBehavior {
    /// Module configuration
    data: QueueProductionExitModuleData,
    /// UpdateModule scheduler state serialized by the C++ base class
    next_call_frame_and_phase: UnsignedInt,
    /// Current delay counter (frames until next unit can exit)
    /// Matches C++ m_currentDelay (line 23)
    current_delay: u32,
    /// Rally point location (world space)
    /// Matches C++ m_rallyPoint (line 28)
    rally_point: Coord3D,
    /// Whether a rally point has been set
    /// Matches C++ m_rallyPointExists (line 32)
    rally_point_exists: bool,
    /// Current burst count (remaining rapid exits)
    /// Matches C++ m_currentBurstCount (line 33)
    current_burst_count: u32,
    /// Creation clear distance (for spacing units)
    /// Matches C++ m_creationClearDistance (line 27)
    creation_clear_distance: f32,
    /// Owner object ID
    owner_id: ObjectID,
}

impl QueueProductionExitBehavior {
    /// Create new queue production exit behavior
    /// Matches C++ QueueProductionExitUpdate constructor (lines 21-39)
    pub fn new(data: QueueProductionExitModuleData, owner_id: ObjectID) -> Self {
        Self {
            current_burst_count: data.initial_burst,
            data,
            next_call_frame_and_phase: 0,
            current_delay: 0,
            rally_point: Coord3D::new(0.0, 0.0, 0.0),
            rally_point_exists: false,
            creation_clear_distance: 0.0,
            owner_id,
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<QueueProductionExitModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "QueueProductionExitUpdate requires an owning object".to_string())?;
        Ok(Self::new(
            module_data.as_ref().clone(),
            module_object.get_object_id(),
        ))
    }

    /// Exit a unit via door
    /// Matches C++ exitObjectViaDoor (lines 47-143)
    pub fn exit_object_via_door(
        &mut self,
        _new_obj_id: ObjectID,
        exit_door: ExitDoorType,
        building_transform: &Matrix3D,
        building_orientation: f32,
        terrain_height_fn: impl Fn(f32, f32) -> f32,
    ) -> Result<ExitResult, String> {
        // Only support DOOR_1 for queue exit
        if exit_door != DOOR_1 {
            return Err("QueueProductionExit only supports single door".to_string());
        }

        // Calculate exit position
        // Matches C++ lines 56-82
        let mut exit_pos = self.transform_point(&self.data.unit_create_point, building_transform);

        let ground_z = terrain_height_fn(exit_pos.x, exit_pos.y);
        let creation_in_air = exit_pos.z != ground_z;
        if creation_in_air && !self.data.allow_airborne_creation {
            exit_pos.z = ground_z;
        }

        // Calculate natural rally point
        // Matches C++ lines 112-117
        let natural_rally = self.get_natural_rally_point(building_transform, false);

        // Determine final rally point
        let final_rally = if self.rally_point_exists {
            self.rally_point
        } else {
            natural_rally
        };

        // Set exit delay for next unit
        // Matches C++ line 136
        self.current_delay = self.data.exit_delay;

        // Decrement burst count
        // Matches C++ lines 138-139
        if self.current_burst_count > 0 {
            self.current_burst_count -= 1;
        }

        Ok(ExitResult {
            exit_position: exit_pos,
            exit_orientation: building_orientation,
            rally_point: final_rally,
            creation_in_air,
        })
    }

    /// Exit object by budding (for units that spawn at same location)
    /// Matches C++ exitObjectByBudding (lines 210-242)
    pub fn exit_object_by_budding(
        &mut self,
        _new_obj_id: ObjectID,
        bud_host_position: Option<Coord3D>,
        bud_host_orientation: Option<f32>,
        owner_position: Option<Coord3D>,
        owner_orientation: Option<f32>,
    ) -> Result<ExitResult, String> {
        let (position, orientation) =
            if let (Some(pos), Some(orient)) = (bud_host_position, bud_host_orientation) {
                // Use host position
                (pos, orient)
            } else if let (Some(pos), Some(orient)) = (owner_position, owner_orientation) {
                // C++ fallback: use producer object's own location/orientation.
                (pos, orient)
            } else {
                return Err("No bud host or owner position provided".to_string());
            };

        // Set delay
        // Matches C++ line 237
        self.current_delay = self.data.exit_delay;

        // Decrement burst
        // Matches C++ lines 239-240
        if self.current_burst_count > 0 {
            self.current_burst_count -= 1;
        }

        Ok(ExitResult {
            exit_position: position,
            exit_orientation: orientation,
            rally_point: position, // Units just move away from spawn
            creation_in_air: false,
        })
    }

    /// Get exit position
    /// Matches C++ getExitPosition (lines 146-166)
    pub fn get_exit_position(&self, building_transform: &Matrix3D) -> Coord3D {
        self.transform_point(&self.data.unit_create_point, building_transform)
    }

    /// Reserve door for exit
    /// Matches C++ reserveDoorForExit (lines 170-173)
    pub fn reserve_door_for_exit(&self) -> ExitDoorType {
        if self.is_free_to_exit() {
            DOOR_1
        } else {
            DOOR_NONE_AVAILABLE
        }
    }

    /// Unreserve door (no-op for queue exit)
    /// Matches C++ unreserveDoorForExit (lines 176-179)
    pub fn unreserve_door_for_exit(&mut self, _exit_door: ExitDoorType) {
        // Nothing to do
    }

    /// Check if free to exit (no delay active)
    /// Matches C++ isFreeToExit (lines 182-193)
    pub fn is_free_to_exit(&self) -> bool {
        let still_bursting = self.current_burst_count > 0;
        let still_delaying = self.current_delay != 0;

        if still_bursting {
            return true;
        }

        !still_delaying
    }

    /// Get natural rally point (in world space)
    /// Matches C++ getNaturalRallyPoint (lines 248-274)
    pub fn get_natural_rally_point(&self, building_transform: &Matrix3D, offset: bool) -> Coord3D {
        let mut point = self.data.natural_rally_point;

        // Apply offset if requested
        // Matches C++ lines 261-267
        if offset {
            // Normalize and offset by 2 pathfind cells
            let length = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
            if length > 0.0 {
                let pathfind_cell_size = crate::path::PATHFIND_CELL_SIZE_F;
                let offset_distance = 2.0 * pathfind_cell_size;
                point.x += (point.x / length) * offset_distance;
                point.y += (point.y / length) * offset_distance;
                point.z += (point.z / length) * offset_distance;
            }
        }

        // Transform to world space
        // Matches C++ lines 270-272
        self.transform_point(&point, building_transform)
    }

    /// Set rally point
    pub fn set_rally_point(&mut self, point: Coord3D) {
        self.rally_point = point;
        self.rally_point_exists = true;
    }

    /// Clear rally point
    pub fn clear_rally_point(&mut self) {
        self.rally_point_exists = false;
    }

    /// Update the module
    /// Matches C++ update (lines 196-207)
    pub fn update(&mut self) -> UpdateResult {
        // Match C++: always run with UPDATE_SLEEP_NONE even when no delay is active.
        if self.is_free_to_exit() {
            self.current_delay = 0;
            return UpdateResult::Continue;
        }

        // Decrement delay counter
        if self.current_delay > 0 {
            self.current_delay -= 1;
        }

        UpdateResult::Continue
    }

    /// Transform a point from model space to world space
    fn transform_point(&self, point: &Coord3D, transform: &Matrix3D) -> Coord3D {
        transform.transform_point3(*point)
    }
}

/// Exit result data
#[derive(Debug, Clone)]
pub struct ExitResult {
    /// Position to spawn unit
    pub exit_position: Coord3D,
    /// Orientation for spawned unit
    pub exit_orientation: f32,
    /// Rally point for unit to move to
    pub rally_point: Coord3D,
    /// Whether unit was created above terrain height.
    pub creation_in_air: bool,
}

/// Update result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// Continue updating
    Continue,
}

impl UpdateModuleInterface for QueueProductionExitBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let _ = QueueProductionExitBehavior::update(self);
        Ok(UpdateSleepTime::None)
    }
}

impl BehaviorModuleInterface for QueueProductionExitBehavior {
    fn get_module_name(&self) -> &'static str {
        "QueueProductionExitUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ModuleExitInterface> {
        Some(self)
    }
}

impl ModuleExitInterface for QueueProductionExitBehavior {
    fn can_exit(&self, _object_id: ObjectID) -> bool {
        self.is_free_to_exit()
    }

    fn exit(&mut self, _object_id: ObjectID) -> bool {
        self.is_free_to_exit()
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        if self.rally_point_exists {
            Ok(Some(self.rally_point))
        } else {
            Ok(None)
        }
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&crate::object::Object>,
        _spawn: Option<&crate::object::Object>,
    ) -> ModuleExitDoorType {
        if self.is_free_to_exit() {
            ModuleExitDoorType::Primary
        } else {
            ModuleExitDoorType::NoneAvailable
        }
    }

    fn unreserve_door_for_exit(&mut self, _door: ModuleExitDoorType) {}

    fn exit_object_via_door(
        &mut self,
        obj: &Arc<RwLock<crate::object::Object>>,
        door: ModuleExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let exit_door = match door {
            ModuleExitDoorType::Primary
            | ModuleExitDoorType::Secondary
            | ModuleExitDoorType::Emergency
            | ModuleExitDoorType::Door1
            | ModuleExitDoorType::Door2
            | ModuleExitDoorType::Door3
            | ModuleExitDoorType::Door4 => DOOR_1,
            ModuleExitDoorType::None | ModuleExitDoorType::NoneAvailable => DOOR_NONE_AVAILABLE,
        };

        if exit_door == DOOR_NONE_AVAILABLE {
            return Ok(());
        }

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return Ok(());
        };

        let building_transform = owner_guard.get_transform_matrix();
        let building_orientation = owner_guard.get_orientation();
        let owner_layer = owner_guard.get_layer();
        let owner_velocity = owner_guard
            .get_physics()
            .and_then(|physics| physics.lock().ok().map(|p| p.get_velocity()));
        drop(owner_guard);

        let new_obj_id = obj.read().map(|guard| guard.get_id()).unwrap_or(INVALID_ID);
        let exit_result = QueueProductionExitBehavior::exit_object_via_door(
            self,
            new_obj_id,
            exit_door,
            &building_transform,
            building_orientation,
            |x, y| {
                TheTerrainLogic::get()
                    .map(|terrain| terrain.get_ground_height(x, y, None))
                    .unwrap_or(0.0)
            },
        );

        if let Ok(result) = exit_result {
            if let Ok(mut guard) = obj.write() {
                let _ = guard.set_position(&result.exit_position);
                let _ = guard.set_orientation(result.exit_orientation);
                guard.set_layer(owner_layer);
            }

            if result.creation_in_air {
                if let Some(owner_velocity) = owner_velocity {
                    if let Ok(obj_guard) = obj.read() {
                        if let Some(physics) = obj_guard.get_physics() {
                            if let Ok(mut phys_guard) = physics.lock() {
                                let mut starting_force = owner_velocity;
                                starting_force *= phys_guard.get_mass();
                                phys_guard.apply_motive_force(&starting_force);
                            }
                        }
                    }
                }
            }

            if let Ok(ai_guard) = THE_AI.read() {
                if let Some(pathfinder) = ai_guard.pathfinder() {
                    if let Ok(mut pf) = pathfinder.write() {
                        pf.add_object_to_map(new_obj_id, &[result.exit_position], false);
                    }
                }
            }

            let natural_rally = self.get_natural_rally_point(&building_transform, false);
            let mut exit_path = vec![natural_rally];
            if let Ok(new_obj_guard) = obj.read() {
                if let Some(ai) = new_obj_guard.get_ai_update_interface() {
                    if self.rally_point_exists {
                        if let Ok(mut ai_guard) = ai.lock() {
                            if ai_guard.is_doing_ground_movement() {
                                let mut rally = result.rally_point;
                                if ai_guard.adjust_destination(&mut rally) {
                                    exit_path.push(rally);
                                }
                            }
                        }
                    } else {
                        // Match C++ "double destination" to prevent stacking.
                        exit_path.push(natural_rally);
                    }
                    ai.ai_follow_exit_production_path(
                        &exit_path,
                        Some(self.owner_id),
                        CommandSourceType::FromAi,
                    );
                }
            }
        }

        Ok(())
    }

    fn exit_object_by_budding(
        &mut self,
        obj: &Arc<RwLock<crate::object::Object>>,
        host: Option<&Arc<RwLock<crate::object::Object>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host_info = host.and_then(|arc| {
            arc.read().ok().map(|guard| {
                (
                    *guard.get_position(),
                    guard.get_orientation(),
                    guard.get_layer(),
                )
            })
        });
        let owner_info = TheGameLogic::find_object_by_id(self.owner_id).and_then(|arc| {
            arc.read()
                .ok()
                .map(|guard| (*guard.get_position(), guard.get_orientation()))
        });

        let (host_pos, host_orient, host_layer) = if let Some((pos, orient, layer)) = host_info {
            (Some(pos), Some(orient), Some(layer))
        } else {
            (None, None, None)
        };

        let new_obj_id = obj.read().map(|guard| guard.get_id()).unwrap_or(INVALID_ID);
        if let Ok(result) = QueueProductionExitBehavior::exit_object_by_budding(
            self,
            new_obj_id,
            host_pos,
            host_orient,
            owner_info.as_ref().map(|(pos, _)| *pos),
            owner_info.as_ref().map(|(_, orient)| *orient),
        ) {
            if let Ok(mut guard) = obj.write() {
                let _ = guard.set_position(&result.exit_position);
                let _ = guard.set_orientation(result.exit_orientation);
                if let Some(layer) = host_layer {
                    guard.set_layer(layer);
                }
            }

            if let Ok(new_obj_guard) = obj.read() {
                if let Some(ai) = new_obj_guard.get_ai_update_interface() {
                    ai.ai_move_to_position(&result.exit_position, false, CommandSourceType::FromAi);
                }
            }
        }

        Ok(())
    }
}

impl Snapshotable for QueueProductionExitBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("QueueProductionExitBehavior::xfer version failed: {err}"))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.current_delay)
            .map_err(|err| {
                format!("QueueProductionExitBehavior::xfer current_delay failed: {err}")
            })?;
        xfer.xfer_coord3d(&mut self.rally_point);
        xfer.xfer_bool(&mut self.rally_point_exists)
            .map_err(|err| {
                format!("QueueProductionExitBehavior::xfer rally_point_exists failed: {err}")
            })?;
        xfer.xfer_real(&mut self.creation_clear_distance)
            .map_err(|err| {
                format!("QueueProductionExitBehavior::xfer creation_clear_distance failed: {err}")
            })?;
        xfer.xfer_unsigned_int(&mut self.current_burst_count)
            .map_err(|err| {
                format!("QueueProductionExitBehavior::xfer current_burst_count failed: {err}")
            })?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.data.load_post_process()
    }
}

/// Glue that exposes QueueProductionExitBehavior through the common Module trait.
pub struct QueueProductionExitBehaviorModule {
    behavior: QueueProductionExitBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<QueueProductionExitModuleData>,
}

impl QueueProductionExitBehaviorModule {
    pub fn new(
        behavior: QueueProductionExitBehavior,
        module_name: &AsciiString,
        module_data: Arc<QueueProductionExitModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut QueueProductionExitBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for QueueProductionExitBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for QueueProductionExitBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
            self.module_data.as_ref(),
        )
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

fn parse_coord3d(tokens: &[&str]) -> Result<Coord3D, INIError> {
    let (x, y, z) = INI::parse_coord_3d(tokens)?;
    Ok(Coord3D::new(x, y, z))
}

fn parse_unit_create_point(
    _ini: &mut INI,
    data: &mut QueueProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.unit_create_point = parse_coord3d(tokens)?;
    Ok(())
}

fn parse_natural_rally_point(
    _ini: &mut INI,
    data: &mut QueueProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.natural_rally_point = parse_coord3d(tokens)?;
    Ok(())
}

fn parse_exit_delay(
    _ini: &mut INI,
    data: &mut QueueProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.exit_delay = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_initial_burst(
    _ini: &mut INI,
    data: &mut QueueProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.initial_burst = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_allow_airborne_creation(
    _ini: &mut INI,
    data: &mut QueueProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.allow_airborne_creation = INI::parse_bool(token)?;
    Ok(())
}

const QUEUE_PRODUCTION_EXIT_FIELDS: &[FieldParse<QueueProductionExitModuleData>] = &[
    FieldParse {
        token: "UnitCreatePoint",
        parse: parse_unit_create_point,
    },
    FieldParse {
        token: "NaturalRallyPoint",
        parse: parse_natural_rally_point,
    },
    FieldParse {
        token: "ExitDelay",
        parse: parse_exit_delay,
    },
    FieldParse {
        token: "InitialBurst",
        parse: parse_initial_burst,
    },
    FieldParse {
        token: "AllowAirborneCreation",
        parse: parse_allow_airborne_creation,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "actual {actual} expected {expected}"
        );
    }

    #[test]
    fn test_exit_creation() {
        let data = QueueProductionExitModuleData::default();
        let exit = QueueProductionExitBehavior::new(data, 1);

        assert_eq!(exit.current_delay, 0);
        assert!(!exit.rally_point_exists);
    }

    #[test]
    fn test_is_free_to_exit() {
        let mut data = QueueProductionExitModuleData::default();
        data.exit_delay = 30;
        data.initial_burst = 2;

        let mut exit = QueueProductionExitBehavior::new(data, 1);

        // Should be free during burst
        assert!(exit.is_free_to_exit());
        assert_eq!(exit.current_burst_count, 2);

        // After burst, need to check delay
        exit.current_burst_count = 0;
        assert!(exit.is_free_to_exit()); // No delay yet

        exit.current_delay = 30;
        assert!(!exit.is_free_to_exit()); // Delay active
    }

    #[test]
    fn test_delay_countdown() {
        let mut data = QueueProductionExitModuleData::default();
        data.exit_delay = 30;
        data.initial_burst = 0;

        let mut exit = QueueProductionExitBehavior::new(data, 1);

        // Set delay
        exit.current_delay = 10;

        // Update should decrement
        exit.update();
        assert_eq!(exit.current_delay, 9);

        // Update again
        exit.update();
        assert_eq!(exit.current_delay, 8);
    }

    #[test]
    fn test_reserve_door() {
        let data = QueueProductionExitModuleData::default();
        let mut exit = QueueProductionExitBehavior::new(data, 1);

        // Should be able to reserve when free
        assert_eq!(exit.reserve_door_for_exit(), DOOR_1);

        // Set delay
        exit.current_delay = 30;

        // Should not be able to reserve when delayed
        assert_eq!(exit.reserve_door_for_exit(), DOOR_NONE_AVAILABLE);
    }

    #[test]
    fn test_rally_point() {
        let data = QueueProductionExitModuleData::default();
        let mut exit = QueueProductionExitBehavior::new(data, 1);

        assert!(!exit.rally_point_exists);

        // Set rally point
        let rally = Coord3D::new(100.0, 200.0, 0.0);
        exit.set_rally_point(rally);

        assert!(exit.rally_point_exists);
        assert_eq!(exit.rally_point.x, 100.0);
        assert_eq!(exit.rally_point.y, 200.0);

        // Clear rally point
        exit.clear_rally_point();
        assert!(!exit.rally_point_exists);
    }

    #[test]
    fn natural_rally_offset_normalizes_full_3d_vector() {
        let mut data = QueueProductionExitModuleData::default();
        data.natural_rally_point = Coord3D::new(3.0, 4.0, 12.0);
        let exit = QueueProductionExitBehavior::new(data, 1);

        let point = exit.get_natural_rally_point(&Matrix3D::IDENTITY, true);
        let offset = 2.0 * crate::path::PATHFIND_CELL_SIZE_F / 13.0;

        assert_near(point.x, 3.0 + 3.0 * offset);
        assert_near(point.y, 4.0 + 4.0 * offset);
        assert_near(point.z, 12.0 + 12.0 * offset);
    }

    #[test]
    fn test_burst_mode() {
        let mut data = QueueProductionExitModuleData::default();
        data.initial_burst = 4;
        data.exit_delay = 30;

        let mut exit = QueueProductionExitBehavior::new(data, 1);

        // Should have 4 burst exits
        assert_eq!(exit.current_burst_count, 4);
        assert!(exit.is_free_to_exit());

        // Simulate exits during burst
        let transform = Matrix3D::IDENTITY;

        // First exit
        exit.exit_object_via_door(1, DOOR_1, &transform, 0.0, |_, _| 0.0)
            .unwrap();
        assert_eq!(exit.current_burst_count, 3);
        assert!(exit.is_free_to_exit());

        // Continue until burst exhausted
        for _ in 0..3 {
            exit.exit_object_via_door(1, DOOR_1, &transform, 0.0, |_, _| 0.0)
                .unwrap();
        }

        assert_eq!(exit.current_burst_count, 0);

        // Now delay should apply
        assert!(!exit.is_free_to_exit());
        assert_eq!(exit.current_delay, 30);
    }
}
