//! Leftover DockUpdate — C++ `DockUpdate.cpp` approach / bone machine.
//!
//! C++ DockUpdate is the shared dock machine (everything except `action()`):
//! approach-slot reservation, `m_activeDocker` / `isClearToEnter`, pristine
//! DockStart / DockAction / DockEnd / DockWaiting bones, `MODELCONDITION_DOCKING*`,
//! and xfer v1. This leftover previously invented a service-time VecDeque.

use crate::common::types::ModelConditionFlags;
use crate::common::xfer::XferExt;
use crate::common::{
    Bool, Coord3D, INVALID_ID, Int, KindOf, MODELCONDITION_DOCKING, MODELCONDITION_DOCKING_ACTIVE,
    MODELCONDITION_DOCKING_BEGINNING, MODELCONDITION_DOCKING_ENDING, ObjectID, UnsignedInt,
};
use crate::helpers::{FindPositionOptions, TheGameLogic, ThePartitionManager};
use crate::object::Object;
use crate::object::drawable::DrawableArcExt;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use serde::{Deserialize, Serialize};

const DEFAULT_APPROACH_VECTOR_SIZE: usize = 10;
const DYNAMIC_APPROACH_VECTOR_FLAG: i32 = -1;
const SINGLE_DOCK_BONE_START_INDEX: usize = 0;
const APPROACH_BONE_START_INDEX: usize = 1;

/// C++ `DockUpdateModuleData` — `NumberApproachPositions` / `AllowsPassthrough`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockUpdateModuleData {
    /// A positive number is an absolute; `DYNAMIC_APPROACH_VECTOR_FLAG` (-1) is dynamic.
    pub number_approach_positions_data: Int,
    pub is_allow_passthrough: Bool,
}

impl Default for DockUpdateModuleData {
    fn default() -> Self {
        Self {
            number_approach_positions_data: 0,
            is_allow_passthrough: true,
        }
    }
}

/// C++ `DockUpdate` approach / bone machine.
#[derive(Debug)]
pub struct DockUpdate {
    data: DockUpdateModuleData,
    owner_id: ObjectID,
    next_call_frame_and_phase: UnsignedInt,
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

fn resolve_dock_object(id: ObjectID) -> Option<std::sync::Arc<std::sync::RwLock<Object>>> {
    if id == INVALID_ID {
        return None;
    }
    TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
}

impl DockUpdate {
    pub fn new(data: DockUpdateModuleData, owner_id: ObjectID) -> Self {
        let number_approach_positions = data.number_approach_positions_data;
        let initial_len = if number_approach_positions != DYNAMIC_APPROACH_VECTOR_FLAG {
            number_approach_positions.max(0) as usize
        } else {
            DEFAULT_APPROACH_VECTOR_SIZE
        };

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
            approach_positions: vec![Coord3D::ZERO; initial_len],
            approach_position_owners: vec![INVALID_ID; initial_len],
            approach_position_reached: vec![false; initial_len],
            active_docker: INVALID_ID,
            docker_inside: false,
            dock_crippled: false,
            dock_open: true,
        }
    }

    /// Compatibility constructor used by leftover tests that pass a building pose.
    pub fn new_at(
        data: DockUpdateModuleData,
        _building_pos: Coord3D,
        _building_angle: crate::common::Real,
    ) -> Self {
        Self::new(data, INVALID_ID)
    }

    pub fn owner_id(&self) -> ObjectID {
        self.owner_id
    }

    pub fn is_dock_open(&self) -> Bool {
        self.dock_open
    }

    pub fn set_dock_open(&mut self, open: Bool) {
        self.dock_open = open;
    }

    pub fn is_allow_passthrough_type(&self) -> Bool {
        self.data.is_allow_passthrough
    }

    /// C++ `DockUpdate::isRallyPointAfterDockType` defaults FALSE.
    pub fn is_rally_point_after_dock_type(&self) -> Bool {
        false
    }

    pub fn set_dock_crippled(&mut self, crippled: Bool) {
        self.dock_crippled = crippled;
    }

    pub fn active_docker_id(&self) -> ObjectID {
        self.active_docker
    }

    pub fn docker_inside(&self) -> Bool {
        self.docker_inside
    }

    pub fn approach_positions_len(&self) -> usize {
        self.approach_positions.len()
    }

    pub fn all_approaches_unoccupied(&self) -> bool {
        self.approach_position_owners
            .iter()
            .all(|id| *id == INVALID_ID)
    }

    /// C++ `loadDockPositions` — only marks loaded when a drawable exists.
    pub fn load_dock_positions(&mut self) {
        let Some((ignore_bones, drawable)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(self.owner_id, |owner_guard| {
                (
                    owner_guard.is_kind_of(KindOf::IgnoreDockingBones),
                    owner_guard.get_drawable(),
                )
            })
        else {
            return;
        };
        let Some(drawable) = drawable else {
            return;
        };
        let Ok(drawable_guard) = drawable.read() else {
            return;
        };

        if !ignore_bones {
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

        let approach = if position_index < self.approach_positions.len() {
            Some(self.approach_positions[position_index])
        } else {
            None
        };
        let their_position = *docker.get_position();
        let Some(mut working_position) =
            crate::object::registry::OBJECT_REGISTRY.with_object(self.owner_id, |owner_guard| {
                let mut working_position = if let Some(approach_pos) = approach {
                    owner_guard
                        .convert_bone_pos_to_world_pos(Some(&approach_pos), None)
                        .transform_point3(Coord3D::ZERO)
                } else {
                    *owner_guard.get_position()
                };

                if self.number_approach_position_bones == 0 {
                    let our_position = owner_guard.get_position();
                    let mut offset = their_position - *our_position;
                    if offset.length_squared() > 0.0001 {
                        offset = offset.normalize();
                        offset *= owner_guard.get_geometry_info().get_major_radius() * 0.5;
                    }
                    working_position += offset;
                }
                working_position
            })
        else {
            return Coord3D::ZERO;
        };

        if let Some(partition) = ThePartitionManager::get() {
            let mut best_position = working_position;
            let mut options = FindPositionOptions::default();
            options.min_radius = 0.0;
            options.max_radius = 100.0;
            options.source_to_path_to_dest_id = Some(docker.get_id());
            if docker.is_using_airborne_locomotor() {
                options.ignore_object_id = Some(self.owner_id);
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

    /// C++ `isClearToApproach`.
    pub fn is_clear_to_approach(&self, obj_id: ObjectID) -> Bool {
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            return true;
        }
        self.approach_position_owners
            .iter()
            .any(|owner| *owner == INVALID_ID || *owner == obj_id)
    }

    /// C++ `reserveApproachPosition`.
    pub fn reserve_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Bool {
        if !self.positions_loaded {
            self.load_dock_positions();
        }
        let Some(obj) = resolve_dock_object(obj_id) else {
            return false;
        };
        let obj_guard = obj.write().unwrap();

        for (position_index, owner) in self.approach_position_owners.iter().enumerate() {
            if *owner == obj_id {
                *goal_pos = self.compute_approach_position(position_index, &obj_guard);
                *approach_pos = position_index as i32;
                return true;
            }
            if *owner == INVALID_ID {
                self.approach_position_owners[position_index] = obj_id;
                *goal_pos = self.compute_approach_position(position_index, &obj_guard);
                *approach_pos = position_index as i32;
                return true;
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
            return true;
        }

        false
    }

    /// C++ `advanceApproachPosition`.
    pub fn advance_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Bool {
        if !self.positions_loaded {
            self.load_dock_positions();
        }
        let Some(obj) = resolve_dock_object(obj_id) else {
            return false;
        };
        let obj_guard = obj.write().unwrap();
        if *approach_pos <= 0 {
            return false;
        }
        let current_pos = *approach_pos as usize;
        if current_pos == 0
            || self.approach_position_owners.get(current_pos - 1) != Some(&INVALID_ID)
        {
            return false;
        }

        self.approach_position_owners[current_pos - 1] = obj_id;
        self.approach_position_reached[current_pos - 1] = false;
        self.approach_position_owners[current_pos] = INVALID_ID;
        self.approach_position_reached[current_pos] = false;
        *goal_pos = self.compute_approach_position(current_pos - 1, &obj_guard);
        *approach_pos = (current_pos - 1) as i32;
        true
    }

    pub fn is_clear_to_advance(&self, obj_id: ObjectID, approach_position: i32) -> Bool {
        if approach_position < 0 {
            return false;
        }
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
        correct_request && approach_reached && next_spot_free
    }

    pub fn on_approach_reached(&mut self, obj_id: ObjectID) {
        for (index, owner) in self.approach_position_owners.iter().enumerate() {
            if *owner == obj_id {
                if let Some(reached) = self.approach_position_reached.get_mut(index) {
                    *reached = true;
                }
                break;
            }
        }
    }

    /// C++ `isClearToEnter` — only the promoted `m_activeDocker`.
    pub fn is_clear_to_enter(&self, obj_id: ObjectID) -> Bool {
        obj_id == self.active_docker
    }

    pub fn get_enter_position(&mut self, obj_id: ObjectID, goal_pos: &mut Coord3D) {
        if !self.positions_loaded {
            self.load_dock_positions();
        }
        let zero = Coord3D::ZERO;
        if self.enter_position == zero {
            if let Some(obj) = resolve_dock_object(obj_id) {
                if let Ok(docker_guard) = obj.read() {
                    if docker_guard.is_using_airborne_locomotor() {
                        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                            if let Ok(owner_guard) = owner.read() {
                                *goal_pos = *owner_guard.get_position();
                                return;
                            }
                        }
                    }
                    *goal_pos = *docker_guard.get_position();
                }
            }
            return;
        }
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let world =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&self.enter_position), None);
                *goal_pos = world.transform_point3(Coord3D::ZERO);
            }
        }
    }

    pub fn get_dock_position(&mut self, obj_id: ObjectID, goal_pos: &mut Coord3D) {
        if !self.positions_loaded {
            self.load_dock_positions();
        }
        let zero = Coord3D::ZERO;
        if self.enter_position == zero {
            if let Some(obj) = resolve_dock_object(obj_id) {
                if let Ok(docker_guard) = obj.read() {
                    *goal_pos = *docker_guard.get_position();
                }
            }
            return;
        }
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let world =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&self.dock_position), None);
                *goal_pos = world.transform_point3(Coord3D::ZERO);
            }
        }
    }

    pub fn get_exit_position(&mut self, obj_id: ObjectID, goal_pos: &mut Coord3D) {
        if !self.positions_loaded {
            self.load_dock_positions();
        }
        let zero = Coord3D::ZERO;
        if self.enter_position == zero {
            if let Some(obj) = resolve_dock_object(obj_id) {
                if let Ok(docker_guard) = obj.read() {
                    *goal_pos = *docker_guard.get_position();
                }
            }
            return;
        }
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let world =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&self.exit_position), None);
                *goal_pos = world.transform_point3(Coord3D::ZERO);
            }
        }
    }

    pub fn on_enter_reached(&mut self, obj_id: ObjectID) {
        let Some(obj) = resolve_dock_object(obj_id) else {
            return;
        };
        let mut obj_guard = obj.write().unwrap();
        let clear = MODELCONDITION_DOCKING_ENDING;
        let set = MODELCONDITION_DOCKING_BEGINNING | MODELCONDITION_DOCKING;
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
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
    }

    pub fn on_dock_reached(&mut self, obj_id: ObjectID) {
        let Some(obj) = resolve_dock_object(obj_id) else {
            return;
        };
        let mut obj_guard = obj.write().unwrap();
        let clear = MODELCONDITION_DOCKING_BEGINNING;
        let set = MODELCONDITION_DOCKING_ACTIVE;
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);
            }
        }
        let _ = obj_guard.clear_and_set_model_condition_flags(clear, set);
    }

    pub fn on_exit_reached(&mut self, obj_id: ObjectID) {
        let Some(obj) = resolve_dock_object(obj_id) else {
            return;
        };
        let mut obj_guard = obj.write().unwrap();
        let clear = MODELCONDITION_DOCKING_ACTIVE | MODELCONDITION_DOCKING;
        let set = MODELCONDITION_DOCKING_ENDING;
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);
            }
        }
        let _ = obj_guard.clear_and_set_model_condition_flags(clear, set);
        self.docker_inside = false;
        if self.active_docker == obj_id {
            self.active_docker = INVALID_ID;
        }
    }

    pub fn cancel_dock(&mut self, obj_id: ObjectID) {
        let Some(obj) = resolve_dock_object(obj_id) else {
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
            }
            return;
        };
        let mut obj_guard = obj.write().unwrap();
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
            if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut owner_guard) = owner.write() {
                    let _ = owner_guard.clear_model_condition_flags(clear);
                }
            }
            let _ = obj_guard.clear_model_condition_flags(clear).ok();
        }
    }

    /// C++ `DockUpdate::update` — promote first reached approach owner to `m_activeDocker`.
    pub fn update(&mut self) {
        if self.active_docker == INVALID_ID && !self.dock_crippled {
            for (index, reached) in self.approach_position_reached.iter().enumerate() {
                if *reached {
                    self.active_docker = self.approach_position_owners[index];
                    break;
                }
            }
        } else if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_kind_of(KindOf::SupplySource) {
                    if let Some(docker) = TheGameLogic::find_object_by_id(self.active_docker) {
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
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| format!("DockUpdate::xfer version failed: {e}"))?;

        xfer.xfer_coord3d(&mut self.enter_position);
        xfer.xfer_coord3d(&mut self.dock_position);
        xfer.xfer_coord3d(&mut self.exit_position);
        xfer.xfer_int(&mut self.number_approach_positions)
            .map_err(|e| format!("DockUpdate::xfer number_approach_positions failed: {e}"))?;
        xfer.xfer_bool(&mut self.positions_loaded)
            .map_err(|e| format!("DockUpdate::xfer positions_loaded failed: {e}"))?;

        let mut vector_size = self.approach_positions.len() as Int;
        xfer.xfer_int(&mut vector_size)
            .map_err(|e| format!("DockUpdate::xfer approach_positions size failed: {e}"))?;
        self.approach_positions
            .resize(vector_size.max(0) as usize, Coord3D::ZERO);
        for position in &mut self.approach_positions {
            xfer.xfer_coord3d(position);
        }

        let mut vector_size = self.approach_position_owners.len() as Int;
        xfer.xfer_int(&mut vector_size)
            .map_err(|e| format!("DockUpdate::xfer approach_position_owners size failed: {e}"))?;
        self.approach_position_owners
            .resize(vector_size.max(0) as usize, INVALID_ID);
        for owner in &mut self.approach_position_owners {
            xfer.xfer_object_id(owner)
                .map_err(|e| format!("DockUpdate::xfer approach_position_owner failed: {e}"))?;
        }

        let mut vector_size = self.approach_position_reached.len() as Int;
        xfer.xfer_int(&mut vector_size)
            .map_err(|e| format!("DockUpdate::xfer approach_position_reached size failed: {e}"))?;
        self.approach_position_reached
            .resize(vector_size.max(0) as usize, false);
        for reached in &mut self.approach_position_reached {
            xfer.xfer_bool(reached)
                .map_err(|e| format!("DockUpdate::xfer approach_position_reached failed: {e}"))?;
        }

        xfer.xfer_object_id(&mut self.active_docker)
            .map_err(|e| format!("DockUpdate::xfer active_docker failed: {e}"))?;
        xfer.xfer_bool(&mut self.docker_inside)
            .map_err(|e| format!("DockUpdate::xfer docker_inside failed: {e}"))?;
        xfer.xfer_bool(&mut self.dock_crippled)
            .map_err(|e| format!("DockUpdate::xfer dock_crippled failed: {e}"))?;
        xfer.xfer_bool(&mut self.dock_open)
            .map_err(|e| format!("DockUpdate::xfer dock_open failed: {e}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_sizes_approach_vector_from_number_approach_positions() {
        let data = DockUpdateModuleData {
            number_approach_positions_data: 3,
            is_allow_passthrough: true,
        };
        let dock = DockUpdate::new(data, 1);
        assert_eq!(dock.approach_positions_len(), 3);
        assert!(dock.all_approaches_unoccupied());
        assert!(!dock.is_rally_point_after_dock_type());
        assert!(dock.is_allow_passthrough_type());
        assert!(dock.is_dock_open());
    }

    #[test]
    fn dynamic_approach_flag_uses_default_vector_size() {
        let data = DockUpdateModuleData {
            number_approach_positions_data: DYNAMIC_APPROACH_VECTOR_FLAG,
            is_allow_passthrough: true,
        };
        let dock = DockUpdate::new(data, 1);
        assert_eq!(dock.approach_positions_len(), DEFAULT_APPROACH_VECTOR_SIZE);
        assert!(dock.is_clear_to_approach(123));
    }

    #[test]
    fn update_promotes_first_reached_approach_owner() {
        let data = DockUpdateModuleData {
            number_approach_positions_data: 2,
            is_allow_passthrough: true,
        };
        let mut dock = DockUpdate::new(data, 1);
        dock.approach_position_owners[0] = 10;
        dock.approach_position_reached[0] = true;
        dock.update();
        assert_eq!(dock.active_docker_id(), 10);
        assert!(dock.is_clear_to_enter(10));
        assert!(!dock.is_clear_to_enter(11));
    }

    #[test]
    fn crippled_dock_never_promotes_active_docker() {
        let data = DockUpdateModuleData {
            number_approach_positions_data: 1,
            is_allow_passthrough: true,
        };
        let mut dock = DockUpdate::new(data, 1);
        dock.set_dock_crippled(true);
        dock.approach_position_owners[0] = 10;
        dock.approach_position_reached[0] = true;
        dock.update();
        assert_eq!(dock.active_docker_id(), INVALID_ID);
        assert!(!dock.is_clear_to_enter(10));
    }

    #[test]
    fn cancel_dock_clears_approach_slot_without_object() {
        let data = DockUpdateModuleData {
            number_approach_positions_data: 1,
            is_allow_passthrough: true,
        };
        let mut dock = DockUpdate::new(data, 1);
        dock.approach_position_owners[0] = 42;
        dock.approach_position_reached[0] = true;
        dock.active_docker = 42;
        dock.cancel_dock(42);
        assert_eq!(dock.approach_position_owners[0], INVALID_ID);
        assert!(!dock.approach_position_reached[0]);
        assert_eq!(dock.active_docker_id(), INVALID_ID);
    }

    #[test]
    fn get_enter_position_calls_load_when_unloaded() {
        let data = DockUpdateModuleData::default();
        let mut dock = DockUpdate::new(data, 1);
        assert!(!dock.positions_loaded);
        let mut goal = Coord3D::ZERO;
        dock.get_enter_position(99, &mut goal);
        // No drawable → C++ leaves positions_loaded false so the next call retries.
        assert!(!dock.positions_loaded);
    }
}
