//! Dock update modules
//!
//! Provides docking infrastructure for units to enter buildings for various
//! purposes: repair, supply collection, transport, etc.

use crate::common::xfer::XferExt;
use crate::common::*;
use crate::helpers::{FindPositionOptions, ThePartitionManager};
use crate::modules::{BehaviorModule, BehaviorModuleInterface, DockUpdateInterface};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::drawable::DrawableArcExt;
use crate::object::{Object, ObjectLockExt};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData};
use std::sync::{Arc, RwLock};

const DEFAULT_APPROACH_VECTOR_SIZE: usize = 10;
const DYNAMIC_APPROACH_VECTOR_FLAG: i32 = -1;
const SINGLE_DOCK_BONE_START_INDEX: usize = 0;
const APPROACH_BONE_START_INDEX: usize = 1;

/// Base dock update module data.
#[derive(Debug, Clone)]
pub struct DockUpdateModuleData {
    pub base: BehaviorModuleData,
    /// A positive number is an absolute, DYNAMIC_APPROACH_VECTOR_FLAG means dynamic vector.
    pub number_approach_positions_data: Int,
    pub is_allow_passthrough: Bool,
}

impl DockUpdateModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            number_approach_positions_data: 0,
            is_allow_passthrough: true,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DOCK_UPDATE_FIELDS)
    }
}

impl Default for DockUpdateModuleData {
    fn default() -> Self {
        Self::new()
    }
}

pub type DockUpdateData = DockUpdateModuleData;

crate::impl_behavior_module_data_via_base!(DockUpdateModuleData, base);

fn parse_number_approach_positions(
    _ini: &mut INI,
    data: &mut DockUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.number_approach_positions_data = INI::parse_int(token)?;
    Ok(())
}

fn parse_allows_passthrough(
    _ini: &mut INI,
    data: &mut DockUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.is_allow_passthrough = INI::parse_bool(token)?;
    Ok(())
}

const DOCK_UPDATE_FIELDS: &[FieldParse<DockUpdateModuleData>] = &[
    FieldParse {
        token: "NumberApproachPositions",
        parse: parse_number_approach_positions,
    },
    FieldParse {
        token: "AllowsPassthrough",
        parse: parse_allows_passthrough,
    },
];

/// Main dock update module
#[derive(Debug)]
pub struct DockUpdate {
    /// Module configuration
    data: DockUpdateModuleData,
    /// Owning object ID
    owner_id: ObjectID,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    /// Dock positions loaded from drawable bones.
    enter_position: Coord3D,
    dock_position: Coord3D,
    exit_position: Coord3D,
    number_approach_positions: Int,
    number_approach_position_bones: Int,
    positions_loaded: Bool,
    approach_positions: Vec<Coord3D>,
    approach_position_owners: Vec<ObjectID>,
    approach_position_reached: Vec<Bool>,
    active_docker: ObjectID,
    docker_inside: Bool,
    dock_crippled: Bool,
    dock_open: Bool,
}

impl DockUpdate {
    /// Create a new dock update module
    pub fn new(data: DockUpdateModuleData, owner_id: ObjectID, _owner_position: &Coord3D) -> Self {
        let number_approach_positions = data.number_approach_positions_data;
        let initial_len = if number_approach_positions != DYNAMIC_APPROACH_VECTOR_FLAG {
            number_approach_positions.max(0) as usize
        } else {
            DEFAULT_APPROACH_VECTOR_SIZE
        };

        let mut approach_positions = Vec::with_capacity(initial_len);
        let mut approach_position_owners = Vec::with_capacity(initial_len);
        let mut approach_position_reached = Vec::with_capacity(initial_len);

        for _ in 0..initial_len {
            approach_positions.push(Coord3D::ZERO);
            approach_position_owners.push(INVALID_ID);
            approach_position_reached.push(false);
        }

        Self {
            data,
            owner_id,
            next_call_frame_and_phase: 0,
            enter_position: Coord3D::ZERO,
            dock_position: Coord3D::ZERO,
            exit_position: Coord3D::ZERO,
            number_approach_positions,
            number_approach_position_bones: -1,
            positions_loaded: false,
            approach_positions,
            approach_position_owners,
            approach_position_reached,
            active_docker: INVALID_ID,
            docker_inside: false,
            dock_crippled: false,
            dock_open: true,
        }
    }

    pub fn owner_id(&self) -> ObjectID {
        self.owner_id
    }

    fn load_dock_positions(&mut self) {
        let Some(owner) = crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(drawable) = owner_guard.get_drawable() else {
            return;
        };
        let Ok(drawable_guard) = drawable.read() else {
            return;
        };

        if !owner_guard.is_kind_of(KindOf::IgnoreDockingBones) {
            if let Some(pos) = drawable_guard
                .get_pristine_bone_positions("DockStart", SINGLE_DOCK_BONE_START_INDEX, 1)
                .first()
            {
                self.enter_position = *pos;
            }
            if let Some(pos) = drawable_guard
                .get_pristine_bone_positions("DockAction", SINGLE_DOCK_BONE_START_INDEX, 1)
                .first()
            {
                self.dock_position = *pos;
            }
            if let Some(pos) = drawable_guard
                .get_pristine_bone_positions("DockEnd", SINGLE_DOCK_BONE_START_INDEX, 1)
                .first()
            {
                self.exit_position = *pos;
            }

            if self.number_approach_positions != DYNAMIC_APPROACH_VECTOR_FLAG {
                let count = self.approach_positions.len();
                let positions = drawable_guard.get_pristine_bone_positions(
                    "DockWaiting",
                    APPROACH_BONE_START_INDEX,
                    count,
                );
                self.number_approach_position_bones = positions.len() as Int;
                if count == positions.len() {
                    for (slot, pos) in self.approach_positions.iter_mut().zip(positions.iter()) {
                        *slot = *pos;
                    }
                }
            } else {
                self.number_approach_position_bones = 0;
            }
        } else {
            self.number_approach_position_bones = 0;
        }

        self.positions_loaded = true;
    }

    fn compute_approach_position(&mut self, position_index: usize, docker: &Object) -> Coord3D {
        if !self.positions_loaded {
            self.load_dock_positions();
        }

        let Some(owner) = crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id) else {
            return Coord3D::ZERO;
        };
        let Ok(owner_guard) = owner.read() else {
            return Coord3D::ZERO;
        };

        let mut working_position = if position_index < self.approach_positions.len() {
            owner_guard
                .convert_bone_pos_to_world_pos(Some(&self.approach_positions[position_index]), None)
                .transform_point3(Coord3D::ZERO)
        } else {
            *owner_guard.get_position()
        };

        if self.number_approach_position_bones == 0 {
            let our_position = owner_guard.get_position();
            let their_position = docker.get_position();
            let mut offset = *their_position - *our_position;
            if offset.length_squared() > 0.0001 {
                offset = offset.normalize();
                offset *= owner_guard.get_geometry_info().get_major_radius() * 0.5;
            }
            working_position += offset;
        }

        if let Some(partition) = ThePartitionManager::get() {
            let mut best_position = working_position;
            let mut options = FindPositionOptions::default();
            options.min_radius = 0.0;
            options.max_radius = 100.0;
            options.source_to_path_to_dest_id = Some(docker.get_id());
            if docker.is_using_airborne_locomotor() {
                options.ignore_object_id = Some(owner_guard.get_id());
            }

            if partition.find_position_around_with_options(
                &working_position,
                &options,
                &mut best_position,
            ) {
                return best_position;
            }
        }

        working_position
    }

    #[allow(dead_code)]
    pub(crate) fn approach_positions_len(&self) -> usize {
        self.approach_positions.len()
    }

    #[allow(dead_code)]
    pub(crate) fn all_approaches_unoccupied(&self) -> bool {
        self.approach_position_owners
            .iter()
            .all(|id| *id == INVALID_ID)
    }

    pub(crate) fn active_docker_id(&self) -> ObjectID {
        self.active_docker
    }

    pub(crate) fn docker_inside(&self) -> Bool {
        self.docker_inside
    }
}

impl BehaviorModuleInterface for DockUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.active_docker == INVALID_ID && !self.dock_crippled {
            for (index, reached) in self.approach_position_reached.iter().enumerate() {
                if *reached {
                    self.active_docker = self.approach_position_owners[index];
                    break;
                }
            }
        } else if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_kind_of(KindOf::SupplySource) {
                    if let Some(docker) =
                        crate::helpers::TheGameLogic::find_object_by_id(self.active_docker)
                    {
                        if let Ok(mut docker_guard) = docker.write() {
                            if docker_guard.is_kind_of(KindOf::Dozer)
                                && docker_guard.is_kind_of(KindOf::Harvester)
                            {
                                if let Some(drawable) = docker_guard.get_drawable() {
                                    let flags = drawable.get_model_condition_flags();
                                    if flags.contains(MODELCONDITION_DOCKING_BEGINNING) {
                                        let _ = docker_guard.clear_model_condition_flags(
                                            ModelConditionFlags::MOVING,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn get_module_name(&self) -> &str {
        "DockUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000004 // DOCK_UPDATE interface
    }

    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        Some(self)
    }
}

impl BehaviorModule for DockUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("DockUpdate module initialized for object {}", self.owner_id);
        Ok(())
    }

    fn on_destroy(&mut self) {
        log::info!("DockUpdate module destroyed for object {}", self.owner_id);
        self.approach_positions.clear();
        self.approach_position_owners.clear();
        self.approach_position_reached.clear();
    }
}

impl DockUpdateInterface for DockUpdate {
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.dock_open)
    }

    fn set_dock_open(&mut self, open: Bool) {
        self.dock_open = open;
    }

    fn is_clear_to_approach(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            return Ok(true);
        }

        let obj_guard = obj.read().unwrap();
        let obj_id = obj_guard.get_id();

        for owner in self.approach_position_owners.iter() {
            if *owner == INVALID_ID || *owner == obj_id {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn cancel_dock(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();

        for (owner, reached) in self
            .approach_position_owners
            .iter_mut()
            .zip(self.approach_position_reached.iter_mut())
        {
            if *owner == obj_id {
                *owner = INVALID_ID;
                *reached = false;
            }
        }

        if self.active_docker == obj_id {
            self.active_docker = INVALID_ID;
            self.docker_inside = false;
            let clear = MODELCONDITION_DOCKING_ENDING
                | MODELCONDITION_DOCKING_BEGINNING
                | MODELCONDITION_DOCKING_ACTIVE
                | MODELCONDITION_DOCKING;
            if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut owner_guard) = owner.write() {
                    let _ = owner_guard.clear_model_condition_flags(clear);
                }
            }
            let _ = obj_guard.clear_model_condition_flags(clear).ok();
        }

        Ok(())
    }

    fn reserve_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if !self.positions_loaded {
            self.load_dock_positions();
        }

        let obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();

        for (position_index, owner) in self.approach_position_owners.iter().enumerate() {
            if *owner == obj_id {
                *goal_pos = self.compute_approach_position(position_index, &obj_guard);
                *approach_pos = position_index as i32;
                return Ok(true);
            }
            if *owner == INVALID_ID {
                self.approach_position_owners[position_index] = obj_id;
                *goal_pos = self.compute_approach_position(position_index, &obj_guard);
                *approach_pos = position_index as i32;
                return Ok(true);
            }
        }

        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            self.approach_positions.push(Coord3D::ZERO);
            self.approach_position_owners.push(INVALID_ID);
            self.approach_position_reached.push(false);

            self.load_dock_positions();

            let position_index = self.approach_position_owners.len() - 1;
            self.approach_position_owners[position_index] = obj_id;
            *goal_pos = self.compute_approach_position(position_index, &obj_guard);
            *approach_pos = position_index as i32;
            return Ok(true);
        }

        Ok(false)
    }

    fn advance_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if !self.positions_loaded {
            self.load_dock_positions();
        }

        let obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();

        if *approach_pos <= 0 {
            return Ok(false);
        }
        let current_pos = *approach_pos as usize;
        if current_pos == 0 {
            return Ok(false);
        }
        if self.approach_position_owners[current_pos - 1] != INVALID_ID {
            return Ok(false);
        }

        self.approach_position_owners[current_pos - 1] = obj_id;
        self.approach_position_reached[current_pos - 1] = false;
        self.approach_position_owners[current_pos] = INVALID_ID;
        self.approach_position_reached[current_pos] = false;

        *goal_pos = self.compute_approach_position(current_pos - 1, &obj_guard);
        *approach_pos = (current_pos - 1) as i32;
        Ok(true)
    }

    fn is_clear_to_advance(
        &self,
        obj: &Arc<RwLock<Object>>,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if approach_position < 0 {
            return Ok(false);
        }
        let obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();

        let position_index = approach_position as usize;
        let correct_request = self
            .approach_position_owners
            .get(position_index)
            .copied()
            .unwrap_or(INVALID_ID)
            == obj_id;
        let approach_reached = self
            .approach_position_reached
            .get(position_index)
            .copied()
            .unwrap_or(false);
        let next_spot_free = position_index > 0
            && self
                .approach_position_owners
                .get(position_index - 1)
                .copied()
                .unwrap_or(INVALID_ID)
                == INVALID_ID;

        Ok(correct_request && approach_reached && next_spot_free)
    }

    fn on_approach_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();
        for (index, owner) in self.approach_position_owners.iter().enumerate() {
            if *owner == obj_id {
                if let Some(reached) = self.approach_position_reached.get_mut(index) {
                    *reached = true;
                }
                break;
            }
        }
        Ok(())
    }

    fn is_clear_to_enter(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();
        Ok(obj_id == self.active_docker)
    }

    fn get_enter_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let zero = Coord3D::ZERO;
        if self.enter_position == zero {
            if let Ok(docker_guard) = obj.read() {
                if docker_guard.is_using_airborne_locomotor() {
                    if let Some(owner) =
                        crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
                    {
                        if let Ok(owner_guard) = owner.read() {
                            *goal_pos = *owner_guard.get_position();
                            return Ok(());
                        }
                    }
                }
                *goal_pos = *docker_guard.get_position();
            }
            return Ok(());
        }

        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let world =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&self.enter_position), None);
                *goal_pos = world.transform_point3(Coord3D::ZERO);
            }
        }
        Ok(())
    }

    fn on_enter_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();

        let clear = MODELCONDITION_DOCKING_ENDING;
        let set = MODELCONDITION_DOCKING_BEGINNING | MODELCONDITION_DOCKING;
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);
            }
        }
        let _ = obj_guard.clear_and_set_model_condition_flags(clear, set);

        self.docker_inside = true;

        for (index, owner) in self.approach_position_owners.iter().enumerate() {
            if *owner == obj_id {
                self.approach_position_owners[index] = INVALID_ID;
                self.approach_position_reached[index] = false;
                break;
            }
        }
        Ok(())
    }

    fn get_dock_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let zero = Coord3D::ZERO;
        if self.enter_position == zero {
            if let Ok(docker_guard) = obj.read() {
                *goal_pos = *docker_guard.get_position();
            }
            return Ok(());
        }

        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let world =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&self.dock_position), None);
                *goal_pos = world.transform_point3(Coord3D::ZERO);
            }
        }
        Ok(())
    }

    fn on_dock_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut obj_guard = obj.write().unwrap();

        let clear = MODELCONDITION_DOCKING_BEGINNING;
        let set = MODELCONDITION_DOCKING_ACTIVE;
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);
            }
        }
        let _ = obj_guard.clear_and_set_model_condition_flags(clear, set);

        Ok(())
    }

    fn action(
        &mut self,
        _obj: &Arc<RwLock<Object>>,
        _drone: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }

    fn get_exit_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let zero = Coord3D::ZERO;
        if self.enter_position == zero {
            if let Ok(docker_guard) = obj.read() {
                *goal_pos = *docker_guard.get_position();
            }
            return Ok(());
        }

        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let world =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&self.exit_position), None);
                *goal_pos = world.transform_point3(Coord3D::ZERO);
            }
        }
        Ok(())
    }

    fn on_exit_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut obj_guard = obj.write().unwrap();
        let obj_id = obj_guard.get_id();

        let clear = MODELCONDITION_DOCKING_ACTIVE | MODELCONDITION_DOCKING;
        let set = MODELCONDITION_DOCKING_ENDING;
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);
            }
        }
        let _ = obj_guard.clear_and_set_model_condition_flags(clear, set);

        self.docker_inside = false;
        if self.active_docker == obj_id {
            self.active_docker = INVALID_ID;
        } else if self.dock_open {
            log::warn!(
                "DockUpdate {}: exit reached by unexpected docker {}",
                self.owner_id,
                obj_id
            );
        }

        Ok(())
    }

    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.data.is_allow_passthrough)
    }

    fn is_rally_point_after_dock_type(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }

    fn set_dock_crippled(
        &mut self,
        crippled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.dock_crippled = crippled;
        Ok(())
    }
}

impl Snapshotable for DockUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("DockUpdate::xfer version failed: {err}"))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_coord3d(&mut self.enter_position);
        xfer.xfer_coord3d(&mut self.dock_position);
        xfer.xfer_coord3d(&mut self.exit_position);
        xfer.xfer_int(&mut self.number_approach_positions)
            .map_err(|err| format!("DockUpdate::xfer number_approach_positions failed: {err}"))?;
        xfer.xfer_bool(&mut self.positions_loaded)
            .map_err(|err| format!("DockUpdate::xfer positions_loaded failed: {err}"))?;

        let mut vector_size = self.approach_positions.len() as Int;
        xfer.xfer_int(&mut vector_size)
            .map_err(|err| format!("DockUpdate::xfer approach_positions size failed: {err}"))?;
        self.approach_positions
            .resize(vector_size.max(0) as usize, Coord3D::ZERO);
        for position in &mut self.approach_positions {
            xfer.xfer_coord3d(position);
        }

        let mut vector_size = self.approach_position_owners.len() as Int;
        xfer.xfer_int(&mut vector_size).map_err(|err| {
            format!("DockUpdate::xfer approach_position_owners size failed: {err}")
        })?;
        self.approach_position_owners
            .resize(vector_size.max(0) as usize, INVALID_ID);
        for owner in &mut self.approach_position_owners {
            xfer.xfer_object_id(owner)
                .map_err(|err| format!("DockUpdate::xfer approach_position_owner failed: {err}"))?;
        }

        let mut vector_size = self.approach_position_reached.len() as Int;
        xfer.xfer_int(&mut vector_size).map_err(|err| {
            format!("DockUpdate::xfer approach_position_reached size failed: {err}")
        })?;
        self.approach_position_reached
            .resize(vector_size.max(0) as usize, false);
        for reached in &mut self.approach_position_reached {
            xfer.xfer_bool(reached).map_err(|err| {
                format!("DockUpdate::xfer approach_position_reached failed: {err}")
            })?;
        }

        xfer.xfer_object_id(&mut self.active_docker)
            .map_err(|err| format!("DockUpdate::xfer active_docker failed: {err}"))?;
        xfer.xfer_bool(&mut self.docker_inside)
            .map_err(|err| format!("DockUpdate::xfer docker_inside failed: {err}"))?;
        xfer.xfer_bool(&mut self.dock_crippled)
            .map_err(|err| format!("DockUpdate::xfer dock_crippled failed: {err}"))?;
        xfer.xfer_bool(&mut self.dock_open)
            .map_err(|err| format!("DockUpdate::xfer dock_open failed: {err}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// ============================================================================
// SPECIALIZED DOCK TYPES
// ============================================================================

/// Repair dock for vehicle repair
#[derive(Debug, Clone)]
pub struct RepairDockUpdateData {
    /// Base dock data
    pub base: DockUpdateModuleData,
    /// Frames required for full heal
    pub frames_for_full_heal: Real,
}

impl RepairDockUpdateData {
    pub fn new() -> Self {
        Self {
            base: DockUpdateModuleData::default(),
            frames_for_full_heal: 1.0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, REPAIR_DOCK_UPDATE_FIELDS)
    }
}

impl Default for RepairDockUpdateData {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for RepairDockUpdateData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(RepairDockUpdateData, base);

fn parse_time_for_full_heal(
    _ini: &mut INI,
    data: &mut RepairDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.frames_for_full_heal = INI::parse_duration_real(token)?;
    Ok(())
}

const REPAIR_DOCK_UPDATE_FIELDS: &[FieldParse<RepairDockUpdateData>] = &[FieldParse {
    token: "TimeForFullHeal",
    parse: parse_time_for_full_heal,
}];

/// Repair dock module
#[derive(Debug)]
pub struct RepairDockUpdate {
    /// Base dock
    base: DockUpdate,
    /// Repair configuration
    data: RepairDockUpdateData,
    last_repair: ObjectID,
    health_to_add_per_frame: Real,
}

impl RepairDockUpdate {
    pub fn new(data: RepairDockUpdateData, owner_id: ObjectID, owner_position: &Coord3D) -> Self {
        Self {
            base: DockUpdate::new(data.base.clone(), owner_id, owner_position),
            data,
            last_repair: INVALID_ID,
            health_to_add_per_frame: 0.0,
        }
    }

    fn repair_unit(&mut self, unit: &Arc<RwLock<Object>>) -> Result<bool, String> {
        let mut unit_guard = unit.write().unwrap();
        let current_health = unit_guard.get_health();
        let max_health = unit_guard.get_max_health();

        if self.last_repair == INVALID_ID {
            self.last_repair = unit_guard.get_id();
            let frames = self.data.frames_for_full_heal.max(1.0);
            self.health_to_add_per_frame = (max_health - current_health) / frames;
        }

        if current_health >= max_health {
            self.last_repair = INVALID_ID;
            return Ok(false);
        }

        let _ = unit_guard.heal(self.health_to_add_per_frame);
        Ok(true)
    }
}

// Delegate to base implementation
impl BehaviorModuleInterface for RepairDockUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.update()
    }

    fn get_module_name(&self) -> &str {
        "RepairDockUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000004
    }

    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        Some(self)
    }
}

impl BehaviorModule for RepairDockUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.init()
    }

    fn on_destroy(&mut self) {
        crate::resource::remove_supply_center(self.base.owner_id());
        self.base.on_destroy()
    }
}

impl DockUpdateInterface for RepairDockUpdate {
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_dock_open()
    }

    fn set_dock_open(&mut self, open: Bool) {
        self.base.set_dock_open(open);
    }

    fn is_clear_to_approach(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_approach(obj)
    }

    fn cancel_dock(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.cancel_dock(obj)
    }

    fn reserve_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .reserve_approach_position(obj, goal_pos, approach_pos)
    }

    fn advance_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .advance_approach_position(obj, goal_pos, approach_pos)
    }

    fn is_clear_to_advance(
        &self,
        obj: &Arc<RwLock<Object>>,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_advance(obj, approach_position)
    }

    fn on_approach_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_approach_reached(obj)
    }

    fn is_clear_to_enter(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_enter(obj)
    }

    fn get_enter_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_enter_position(obj, goal_pos)
    }

    fn on_enter_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_enter_reached(obj)
    }

    fn get_dock_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_dock_position(obj, goal_pos)
    }

    fn on_dock_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_dock_reached(obj)
    }

    fn action(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        drone: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let keep_docked = self.repair_unit(obj)?;
        if keep_docked {
            if let Some(drone) = drone {
                if let Ok(mut drone_guard) = drone.write() {
                    let _ = drone_guard.heal_completely();
                }
            }
        }
        Ok(keep_docked)
    }

    fn get_exit_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_exit_position(obj, goal_pos)
    }

    fn on_exit_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_exit_reached(obj)
    }

    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_allow_passthrough_type()
    }

    fn is_rally_point_after_dock_type(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // C++ RepairDockUpdate overrides DockUpdate::isRallyPointAfterDockType() to TRUE.
        Ok(true)
    }

    fn set_dock_crippled(
        &mut self,
        crippled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.set_dock_crippled(crippled)
    }
}

/// Supply center dock data
#[derive(Debug, Clone)]
pub struct SupplyCenterDockUpdateData {
    /// Base dock data
    pub base: DockUpdateModuleData,
    /// Temporary stealth grant frames
    pub grant_temporary_stealth_frames: UnsignedInt,
}

impl SupplyCenterDockUpdateData {
    pub fn new() -> Self {
        Self {
            base: DockUpdateModuleData::default(),
            grant_temporary_stealth_frames: 0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUPPLY_CENTER_DOCK_UPDATE_FIELDS)
    }
}

impl Default for SupplyCenterDockUpdateData {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for SupplyCenterDockUpdateData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(SupplyCenterDockUpdateData, base);

fn parse_grant_temporary_stealth(
    _ini: &mut INI,
    data: &mut SupplyCenterDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.grant_temporary_stealth_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const SUPPLY_CENTER_DOCK_UPDATE_FIELDS: &[FieldParse<SupplyCenterDockUpdateData>] = &[FieldParse {
    token: "GrantTemporaryStealth",
    parse: parse_grant_temporary_stealth,
}];

/// Supply center dock module
#[derive(Debug)]
pub struct SupplyCenterDockUpdate {
    /// Base dock
    base: DockUpdate,
    /// Supply configuration
    data: SupplyCenterDockUpdateData,
}

impl SupplyCenterDockUpdate {
    pub fn new(
        data: SupplyCenterDockUpdateData,
        owner_id: ObjectID,
        owner_position: &Coord3D,
    ) -> Self {
        Self {
            base: DockUpdate::new(data.base.clone(), owner_id, owner_position),
            data,
        }
    }
}

// Similar delegate pattern for SupplyCenterDockUpdate...
impl BehaviorModuleInterface for SupplyCenterDockUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.update()
    }

    fn get_module_name(&self) -> &str {
        "SupplyCenterDockUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000004
    }

    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        Some(self)
    }
}

impl BehaviorModule for SupplyCenterDockUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.init()
    }

    fn on_destroy(&mut self) {
        self.base.on_destroy()
    }
}

impl DockUpdateInterface for SupplyCenterDockUpdate {
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_dock_open()
    }

    fn set_dock_open(&mut self, open: Bool) {
        self.base.set_dock_open(open);
    }

    fn is_clear_to_approach(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_approach(obj)
    }

    fn cancel_dock(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.cancel_dock(obj)
    }

    fn reserve_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .reserve_approach_position(obj, goal_pos, approach_pos)
    }

    fn advance_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .advance_approach_position(obj, goal_pos, approach_pos)
    }

    fn is_clear_to_advance(
        &self,
        obj: &Arc<RwLock<Object>>,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_advance(obj, approach_position)
    }

    fn on_approach_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_approach_reached(obj)
    }

    fn is_clear_to_enter(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_enter(obj)
    }

    fn get_enter_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_enter_position(obj, goal_pos)
    }

    fn on_enter_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_enter_reached(obj)
    }

    fn get_dock_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_dock_position(obj, goal_pos)
    }

    fn on_dock_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_dock_reached(obj)
    }

    fn action(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        _drone: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let docker_guard = obj.write().unwrap();
        let Some(ai) = docker_guard.get_ai_update_interface() else {
            return Ok(false);
        };

        let Some(owner_player) =
            crate::helpers::TheGameLogic::find_object_by_id(self.base.owner_id)
                .and_then(|owner| owner.read().ok()?.get_controlling_player())
        else {
            return Ok(false);
        };
        let supply_box_value = owner_player
            .read()
            .map(|player| player.get_supply_box_value())
            .unwrap_or(0);

        let mut value: u32 = 0;
        if let Ok(mut ai_guard) = ai.lock() {
            if let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() {
                while truck.lose_one_box() {
                    value = value.saturating_add(supply_box_value);
                }
                value = value.saturating_add(truck.get_upgraded_supply_boost());
            } else {
                return Ok(false);
            }
        }

        if value > 0 {
            if let Ok(mut player_guard) = owner_player.write() {
                let _ = player_guard.get_money_mut().deposit(value);
            }

            if self.data.grant_temporary_stealth_frames > 0 {
                if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.base.owner_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if owner_guard.test_status(ObjectStatusTypes::Stealthed) {
                            if let Some(stealth) = docker_guard.get_stealth() {
                                if let Ok(mut stealth_guard) = stealth.lock() {
                                    let _ = stealth_guard.receive_grant(
                                        true,
                                        self.data.grant_temporary_stealth_frames,
                                        crate::helpers::TheGameLogic::get_frame(),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            let mut display_money = true;
            if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.base.owner_id)
            {
                if let Ok(owner_guard) = owner.read() {
                    if owner_guard.test_status(ObjectStatusTypes::Stealthed) {
                        if !owner_guard.is_locally_controlled()
                            && !owner_guard.test_status(ObjectStatusTypes::Detected)
                        {
                            display_money = false;
                        }
                    }
                }
            }

            if display_money {
                let pos = docker_guard.get_position();
                let text = format!("+${}", value);
                let color = if let Some(owner) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.base.owner_id)
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(player) = owner_guard.get_controlling_player() {
                            if let Ok(player_guard) = player.read() {
                                let base = player_guard.get_player_color();
                                Color::new(base.r, base.g, base.b, 230)
                            } else {
                                Color::white()
                            }
                        } else {
                            Color::white()
                        }
                    } else {
                        Color::white()
                    }
                } else {
                    Color::white()
                };

                let _ = crate::helpers::TheInGameUI::add_floating_text(&text, &pos, color);
            }
        }

        Ok(false)
    }

    fn get_exit_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_exit_position(obj, goal_pos)
    }

    fn on_exit_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_exit_reached(obj)
    }

    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_allow_passthrough_type()
    }

    fn is_rally_point_after_dock_type(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_rally_point_after_dock_type()
    }

    fn set_dock_crippled(
        &mut self,
        crippled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.set_dock_crippled(crippled)
    }
}

/// Glue that exposes RepairDockUpdate through the common Module trait.
pub struct RepairDockUpdateModule {
    behavior: RepairDockUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<RepairDockUpdateData>,
}

impl RepairDockUpdateModule {
    pub fn new(
        behavior: RepairDockUpdate,
        module_name: &AsciiString,
        module_data: Arc<RepairDockUpdateData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &RepairDockUpdate {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut RepairDockUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for RepairDockUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("RepairDockUpdateModule::xfer version failed: {err}"))?;
        self.behavior.base.xfer(xfer)?;
        xfer.xfer_object_id(&mut self.behavior.last_repair)
            .map_err(|err| format!("RepairDockUpdateModule::xfer last_repair failed: {err}"))?;
        xfer.xfer_real(&mut self.behavior.health_to_add_per_frame)
            .map_err(|err| {
                format!("RepairDockUpdateModule::xfer health_to_add_per_frame failed: {err}")
            })
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.base.load_post_process()
    }
}

impl Module for RepairDockUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

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

/// Glue that exposes SupplyCenterDockUpdate through the common Module trait.
pub struct SupplyCenterDockUpdateModule {
    behavior: SupplyCenterDockUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SupplyCenterDockUpdateData>,
}

impl SupplyCenterDockUpdateModule {
    pub fn new(
        behavior: SupplyCenterDockUpdate,
        module_name: &AsciiString,
        module_data: Arc<SupplyCenterDockUpdateData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &SupplyCenterDockUpdate {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut SupplyCenterDockUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SupplyCenterDockUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("SupplyCenterDockUpdateModule::xfer version failed: {err}"))?;
        self.behavior.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.base.load_post_process()
    }
}

impl Module for SupplyCenterDockUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::body::active_body::{ActiveBody, ActiveBodyModuleData};
    use crate::object::body::body_module::BodyModuleInterface;
    use std::sync::Mutex;

    #[test]
    fn dock_bone_start_indices_match_cpp() {
        assert_eq!(SINGLE_DOCK_BONE_START_INDEX, 0);
        assert_eq!(APPROACH_BONE_START_INDEX, 1);
    }

    #[test]
    fn parse_time_for_full_heal_accepts_duration_suffixes() {
        let mut data = RepairDockUpdateData::default();
        let mut ini = INI::new();

        parse_time_for_full_heal(&mut ini, &mut data, &["1500ms"]).expect("duration");
        assert!((data.frames_for_full_heal - 45.0).abs() < f32::EPSILON);

        parse_time_for_full_heal(&mut ini, &mut data, &["1.5s"]).expect("duration");
        assert!((data.frames_for_full_heal - 45.0).abs() < f32::EPSILON);
    }

    fn test_object_with_health(id: ObjectID, health: f32, max_health: f32) -> Arc<RwLock<Object>> {
        let mut obj = Object::new_test(id, max_health);
        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = max_health;
        module_data.initial_health = health;
        let body: Arc<Mutex<dyn BodyModuleInterface>> = Arc::new(Mutex::new(
            ActiveBody::new_with_owner(module_data, obj.get_id()),
        ));
        obj.set_body_module(Some(body));
        Arc::new(RwLock::new(obj))
    }

    #[test]
    fn repair_dock_action_heals_drone_to_full_while_repair_continues() {
        let data = RepairDockUpdateData {
            frames_for_full_heal: 10.0,
            ..Default::default()
        };
        let mut dock = RepairDockUpdate::new(data, 1, &Coord3D::ZERO);
        let docker = test_object_with_health(2, 50.0, 100.0);
        let drone = test_object_with_health(3, 10.0, 25.0);

        assert!(dock.action(&docker, Some(&drone)).expect("repair action"));
        assert_eq!(docker.read().unwrap().get_health(), 55.0);
        assert_eq!(drone.read().unwrap().get_health(), 25.0);
    }

    #[test]
    fn repair_dock_action_leaves_drone_when_docker_repair_is_complete() {
        let mut dock = RepairDockUpdate::new(RepairDockUpdateData::default(), 1, &Coord3D::ZERO);
        let docker = test_object_with_health(2, 100.0, 100.0);
        let drone = test_object_with_health(3, 10.0, 25.0);

        assert!(!dock.action(&docker, Some(&drone)).expect("repair action"));
        assert_eq!(drone.read().unwrap().get_health(), 10.0);
    }
}
