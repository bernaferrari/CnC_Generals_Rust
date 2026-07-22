//! Railed Transport Dock Update Module
//!
//! Handles docking for railed transport (tunnel) systems where units are pulled
//! inside and later pushed out.
//!
//! Original C++ location: GameLogic/Module/RailedTransportDockUpdate.h/.cpp
//! Original C++ Author: Colin Day, August 2002
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::*;
use crate::helpers::TheTerrainLogic;
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModule, BehaviorModuleInterface, DockUpdateInterface,
    RailedTransportDockUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use std::sync::{Arc, RwLock};

const UNLOAD_ALL: Int = -1;
const CLOSE_ENOUGH_PULL: Real = 6.0;
const CLOSE_ENOUGH_PUSH: Real = 3.0;
const MIN_NORMALIZE_LENGTH_SQUARED: Real = 0.0001;

fn wrap_err(message: String) -> Box<dyn std::error::Error + Send + Sync> {
    std::io::Error::new(std::io::ErrorKind::Other, message).into()
}

fn safe_normalized(v: Coord3D) -> Coord3D {
    if v.length_squared() <= MIN_NORMALIZE_LENGTH_SQUARED {
        Coord3D::ZERO
    } else {
        v.normalize()
    }
}

/// Railed transport dock configuration data
#[derive(Debug, Clone)]
pub struct RailedTransportDockUpdateData {
    /// Base dock data
    pub base: super::DockUpdateData,
    /// Duration to pull object inside (in frames)
    pub pull_inside_duration_frames: UnsignedInt,
    /// Duration to push object outside (in frames)
    pub push_outside_duration_frames: UnsignedInt,
    /// Tolerance distance for docking cheat
    pub tolerance_distance: Real,
}

impl Default for RailedTransportDockUpdateData {
    fn default() -> Self {
        Self {
            base: super::DockUpdateData::default(),
            pull_inside_duration_frames: 0,
            push_outside_duration_frames: 0,
            tolerance_distance: 50.0,
        }
    }
}

impl RailedTransportDockUpdateData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RAILED_TRANSPORT_DOCK_UPDATE_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(RailedTransportDockUpdateData, base);

fn parse_pull_inside_duration(
    _ini: &mut INI,
    data: &mut RailedTransportDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.pull_inside_duration_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_push_outside_duration(
    _ini: &mut INI,
    data: &mut RailedTransportDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.push_outside_duration_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_tolerance_distance(
    _ini: &mut INI,
    data: &mut RailedTransportDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.tolerance_distance = INI::parse_real(token)?;
    Ok(())
}

const RAILED_TRANSPORT_DOCK_UPDATE_FIELDS: &[FieldParse<RailedTransportDockUpdateData>] = &[
    FieldParse {
        token: "PullInsideDuration",
        parse: parse_pull_inside_duration,
    },
    FieldParse {
        token: "PushOutsideDuration",
        parse: parse_push_outside_duration,
    },
    FieldParse {
        token: "ToleranceDistance",
        parse: parse_tolerance_distance,
    },
];

/// Railed transport dock module
#[derive(Debug)]
pub struct RailedTransportDockUpdate {
    /// Base dock functionality
    base: super::DockUpdate,
    /// Transport configuration
    data: RailedTransportDockUpdateData,
    /// Object currently docking with us
    docking_object_id: ObjectID,
    /// Distance to move per frame when pulling inside
    pull_inside_distance_per_frame: Real,
    /// Object currently unloading
    unloading_object_id: ObjectID,
    /// Distance to move per frame when pushing outside
    push_outside_distance_per_frame: Real,
    /// Count of units to unload (UNLOAD_ALL = all, 0 = none)
    unload_count: Int,
}

impl RailedTransportDockUpdate {
    pub fn new(
        data: RailedTransportDockUpdateData,
        owner_id: ObjectID,
        owner_position: &Coord3D,
    ) -> Self {
        Self {
            base: super::DockUpdate::new(data.base.clone(), owner_id, owner_position),
            data,
            docking_object_id: INVALID_ID,
            pull_inside_distance_per_frame: 0.0,
            unloading_object_id: INVALID_ID,
            push_outside_distance_per_frame: 0.0,
            unload_count: UNLOAD_ALL,
        }
    }

    fn do_pull_in_docking(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.docking_object_id == INVALID_ID {
            return Ok(());
        }

        let Some(docker) = TheGameLogic::find_object_by_id(self.docking_object_id) else {
            self.docking_object_id = INVALID_ID;
            return Ok(());
        };
        let Some(us) = TheGameLogic::find_object_by_id(self.base.owner_id()) else {
            self.docking_object_id = INVALID_ID;
            return Ok(());
        };

        let dock_pos = {
            let us_guard = us.read().map_err(|_| "Failed to lock dock owner")?;
            *us_guard.get_position()
        };

        let reached;
        {
            let mut docker_guard = docker.write().map_err(|_| "Failed to lock docker")?;
            let docker_pos = *docker_guard.get_position();

            let mut v = Coord3D::new(
                dock_pos.x - docker_pos.x,
                dock_pos.y - docker_pos.y,
                dock_pos.z - docker_pos.z,
            );
            v = safe_normalized(v);
            v.x *= self.pull_inside_distance_per_frame;
            v.y *= self.pull_inside_distance_per_frame;
            v.x += docker_pos.x;
            v.y += docker_pos.y;
            v.z = docker_pos.z;

            docker_guard.set_position(&v).map_err(wrap_err)?;
            docker_guard.set_model_condition_state(ModelConditionFlags::MOVING);

            let dx = dock_pos.x - v.x;
            let dy = dock_pos.y - v.y;
            let dist_sq = dx * dx + dy * dy;
            reached = dist_sq <= (CLOSE_ENOUGH_PULL * CLOSE_ENOUGH_PULL);

            if reached {
                docker_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }

        if reached {
            self.base
                .cancel_dock(docker.read().map(|g| g.get_id()).unwrap_or(0))?;

            if let Ok(docker_guard) = docker.read() {
                if let Some(ai) = docker_guard.get_ai_update_interface() {
                    ai.ai_idle(CommandSourceType::FromAi);
                }
            }

            if let Ok(us_guard) = us.read() {
                if let Some(contain) = us_guard.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        if let Ok(docker_guard) = docker.read() {
                            let _ = contain_guard.add_to_contain(&*docker_guard);
                        }
                    }
                }
            }

            self.docking_object_id = INVALID_ID;
        }

        Ok(())
    }

    fn do_push_out_docking(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.unloading_object_id == INVALID_ID {
            return Ok(());
        }

        let Some(unloader) = TheGameLogic::find_object_by_id(self.unloading_object_id) else {
            self.unload_next()?;
            return Ok(());
        };

        let mut dest_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.base.get_exit_position(
            unloader.read().map(|g| g.get_id()).unwrap_or(0),
            &mut dest_pos,
        )?;
        if let Some(terrain) = TheTerrainLogic::get() {
            dest_pos.z = terrain.get_ground_height(dest_pos.x, dest_pos.y, None);
        }

        let reached;
        {
            let mut unloader_guard = unloader.write().map_err(|_| "Failed to lock unloader")?;
            let unloader_pos = *unloader_guard.get_position();

            let mut v = Coord3D::new(
                dest_pos.x - unloader_pos.x,
                dest_pos.y - unloader_pos.y,
                dest_pos.z - unloader_pos.z,
            );
            v = safe_normalized(v);
            v.x *= self.push_outside_distance_per_frame;
            v.y *= self.push_outside_distance_per_frame;
            v.x += unloader_pos.x;
            v.y += unloader_pos.y;
            v.z = dest_pos.z;

            unloader_guard.set_position(&v).map_err(wrap_err)?;
            unloader_guard.set_model_condition_state(ModelConditionFlags::MOVING);

            let dx = dest_pos.x - v.x;
            let dy = dest_pos.y - v.y;
            let dz = dest_pos.z - v.z;
            let dist_sq = dx * dx + dy * dy + dz * dz;
            reached = dist_sq <= (CLOSE_ENOUGH_PUSH * CLOSE_ENOUGH_PUSH);

            if reached {
                unloader_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }

        if reached {
            if let Ok(unloader_guard) = unloader.read() {
                if let Some(ai) = unloader_guard.get_ai_update_interface() {
                    ai.ai_idle(CommandSourceType::FromAi);
                }
            }

            if let Ok(mut unloader_guard) = unloader.write() {
                unloader_guard.clear_disabled(DisabledType::Held);
                unloader_guard.clear_status(ObjectStatusMaskType::UNSELECTABLE);
            }

            let us = TheGameLogic::find_object_by_id(self.base.owner_id());
            if let (Some(us), Ok(unloader_guard)) = (us, unloader.read()) {
                if let Some(ai) = unloader_guard.get_ai_update_interface() {
                    if let Ok(us_guard) = us.read() {
                        if let Some(drawable) = us_guard.get_drawable() {
                            if let Ok(drawable_guard) = drawable.read() {
                                if let Some(local_pos) = drawable_guard
                                    .get_pristine_bone_positions("DOCKWAITING07", 0, 1)
                                    .first()
                                    .copied()
                                {
                                    let world = us_guard
                                        .convert_bone_pos_to_world_pos(Some(&local_pos), None);
                                    let (_, _, translation) = world.to_scale_rotation_translation();
                                    ai.ai_move_to_position(
                                        &translation,
                                        false,
                                        CommandSourceType::FromAi,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            self.unload_next()?;
        }

        Ok(())
    }

    fn unload_next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.unloading_object_id = INVALID_ID;

        if self.unload_count == 0 {
            return Ok(());
        }

        let Some(us) = TheGameLogic::find_object_by_id(self.base.owner_id()) else {
            return Ok(());
        };

        let contain = {
            let us_guard = us.read().map_err(|_| "Failed to lock dock owner")?;
            us_guard.get_contain()
        };

        let Some(contain) = contain else {
            return Ok(());
        };

        let unloader_id = {
            let contain_guard = contain.lock().map_err(|_| "Failed to lock contain")?;
            contain_guard.get_contained_objects().first().copied()
        };

        let Some(unloader_id) = unloader_id else {
            return Ok(());
        };

        let Some(unloader) = TheGameLogic::find_object_by_id(unloader_id) else {
            return Ok(());
        };

        {
            let mut contain_guard = contain.lock().map_err(|_| "Failed to lock contain")?;
            let _ = contain_guard.release_object(unloader_id);
        }

        let us_pos = {
            let us_guard = us.read().map_err(|_| "Failed to lock dock owner")?;
            *us_guard.get_position()
        };

        {
            let mut unloader_guard = unloader.write().map_err(|_| "Failed to lock unloader")?;
            unloader_guard.set_position(&us_pos).map_err(wrap_err)?;
            if let Ok(us_guard) = us.read() {
                unloader_guard
                    .set_orientation(us_guard.get_orientation())
                    .map_err(wrap_err)?;
            }
            unloader_guard.set_disabled(DisabledType::Held);
        }

        let mut dock_position = Coord3D::new(0.0, 0.0, 0.0);
        self.base.get_exit_position(
            unloader.read().map(|g| g.get_id()).unwrap_or(0),
            &mut dock_position,
        )?;

        let unloader_pos = {
            let unloader_guard = unloader.read().map_err(|_| "Failed to lock unloader")?;
            *unloader_guard.get_position()
        };

        let v = Coord3D::new(
            dock_position.x - unloader_pos.x,
            dock_position.y - unloader_pos.y,
            dock_position.z - unloader_pos.z,
        );
        let mag = v.length();

        let duration = self.data.push_outside_duration_frames;
        if duration > 0 {
            self.push_outside_distance_per_frame = mag / duration as Real;
        } else {
            self.push_outside_distance_per_frame = mag;
        }

        self.unloading_object_id = unloader_id;

        if self.unload_count != UNLOAD_ALL {
            self.unload_count -= 1;
        }

        Ok(())
    }
}

impl BehaviorModuleInterface for RailedTransportDockUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = UpdateModuleInterface::update(self)?;
        Ok(())
    }

    fn get_module_name(&self) -> &str {
        "RailedTransportDockUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000004
    }

    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        Some(self)
    }

    fn get_railed_transport_dock_update_interface(
        &mut self,
    ) -> Option<&mut dyn RailedTransportDockUpdateInterface> {
        Some(self)
    }
}

impl UpdateModuleInterface for RailedTransportDockUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.base.update()?;
        self.do_pull_in_docking()?;
        self.do_push_out_docking()?;
        Ok(UpdateSleepTime::None)
    }
}

impl BehaviorModule for RailedTransportDockUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.init()
    }

    fn on_destroy(&mut self) {
        self.base.on_destroy();
    }
}

impl DockUpdateInterface for RailedTransportDockUpdate {
    fn action(
        &mut self,
        obj_id: ObjectID,
        _drone_id: Option<ObjectID>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(false);
        };
        let docker_id = {
            let docker_guard = obj.read().map_err(|_| "Failed to lock docker")?;
            docker_guard.get_id()
        };

        if self.docking_object_id != docker_id {
            let Some(us) = TheGameLogic::find_object_by_id(self.base.owner_id()) else {
                return Ok(false);
            };
            let dock_pos = {
                let us_guard = us.read().map_err(|_| "Failed to lock dock owner")?;
                *us_guard.get_position()
            };
            let docker_pos = {
                let docker_guard = obj.read().map_err(|_| "Failed to lock docker")?;
                *docker_guard.get_position()
            };

            let v = Coord3D::new(
                dock_pos.x - docker_pos.x,
                dock_pos.y - docker_pos.y,
                dock_pos.z - docker_pos.z,
            );
            let mag = v.length();

            if mag <= self.data.tolerance_distance {
                self.docking_object_id = docker_id;

                if let Ok(mut docker_guard) = obj.write() {
                    let _ = TheGameLogic::deselect_object(&*docker_guard, PLAYERMASK_ALL, true);
                    docker_guard.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
                    docker_guard.set_disabled(DisabledType::Held);

                    let angle = (dock_pos.y - docker_pos.y).atan2(dock_pos.x - docker_pos.x);
                    let _ = docker_guard.set_orientation(angle);
                }

                let duration = self.data.pull_inside_duration_frames;
                if duration > 0 {
                    self.pull_inside_distance_per_frame = mag / duration as Real;
                } else {
                    self.pull_inside_distance_per_frame = mag;
                }
            }
        }

        Ok(true)
    }

    fn is_clear_to_enter(
        &self,
        obj_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.is_clear_to_enter(obj_id)? {
            return Ok(false);
        }

        let Some(us) = TheGameLogic::find_object_by_id(self.base.owner_id()) else {
            return Ok(false);
        };

        let contain = {
            let us_guard = us.read().map_err(|_| "Failed to lock dock owner")?;
            us_guard.get_contain()
        };

        let Some(contain) = contain else {
            return Ok(true);
        };

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(false);
        };
        let obj_guard = obj.read().map_err(|_| "Failed to lock docker")?;
        let contain_guard = contain.lock().map_err(|_| "Failed to lock contain")?;
        Ok(contain_guard.is_valid_container_for(&*obj_guard, true))
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

    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_dock_open()
    }

    fn set_dock_open(&mut self, open: Bool) {
        self.base.set_dock_open(open);
    }

    fn cancel_dock(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.docking_object_id == obj_id {
            self.docking_object_id = INVALID_ID;
        }
        self.base.cancel_dock(obj_id)
    }

    fn reserve_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .reserve_approach_position(obj_id, goal_pos, approach_pos)
    }

    fn advance_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .advance_approach_position(obj_id, goal_pos, approach_pos)
    }

    fn is_clear_to_advance(
        &self,
        obj_id: ObjectID,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_advance(obj_id, approach_position)
    }

    fn on_approach_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_approach_reached(obj_id)
    }

    fn get_enter_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_enter_position(obj_id, goal_pos)
    }

    fn on_enter_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_enter_reached(obj_id)
    }

    fn get_dock_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_dock_position(obj_id, goal_pos)
    }

    fn on_dock_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_dock_reached(obj_id)
    }

    fn get_exit_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_exit_position(obj_id, goal_pos)
    }

    fn on_exit_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_exit_reached(obj_id)
    }
}

impl RailedTransportDockUpdateInterface for RailedTransportDockUpdate {
    fn is_loading_or_unloading(&self) -> bool {
        self.unloading_object_id != INVALID_ID || self.docking_object_id != INVALID_ID
    }

    fn unload_all(&mut self) {
        if self.unloading_object_id != INVALID_ID {
            return;
        }

        self.unload_count = UNLOAD_ALL;
        let _ = self.unload_next();
    }

    fn unload_single_object(&mut self, _obj: &Arc<RwLock<Object>>) {
        self.unload_count = 1;
        let _ = self.unload_next();
    }
}

/// Glue that exposes RailedTransportDockUpdate through the common Module trait.
pub struct RailedTransportDockUpdateModule {
    behavior: RailedTransportDockUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<RailedTransportDockUpdateData>,
}

impl RailedTransportDockUpdateModule {
    pub fn new(
        behavior: RailedTransportDockUpdate,
        module_name: &AsciiString,
        module_data: Arc<RailedTransportDockUpdateData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &RailedTransportDockUpdate {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut RailedTransportDockUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for RailedTransportDockUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(|err| {
            format!("RailedTransportDockUpdateModule::xfer version failed: {err}")
        })?;
        self.behavior.base.xfer(xfer)?;
        xfer.xfer_object_id(&mut self.behavior.docking_object_id)
            .map_err(|err| {
                format!("RailedTransportDockUpdateModule::xfer docking_object_id failed: {err}")
            })?;
        xfer.xfer_real(&mut self.behavior.pull_inside_distance_per_frame)
            .map_err(|err| {
                format!(
                    "RailedTransportDockUpdateModule::xfer pull_inside_distance_per_frame failed: {err}"
                )
            })?;
        xfer.xfer_object_id(&mut self.behavior.unloading_object_id)
            .map_err(|err| {
                format!("RailedTransportDockUpdateModule::xfer unloading_object_id failed: {err}")
            })?;
        xfer.xfer_real(&mut self.behavior.push_outside_distance_per_frame)
            .map_err(|err| {
                format!(
                    "RailedTransportDockUpdateModule::xfer push_outside_distance_per_frame failed: {err}"
                )
            })?;
        xfer.xfer_int(&mut self.behavior.unload_count)
            .map_err(|err| {
                format!("RailedTransportDockUpdateModule::xfer unload_count failed: {err}")
            })
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.base.load_post_process()
    }
}

impl Module for RailedTransportDockUpdateModule {
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

    #[test]
    fn railed_transport_defaults_match_cpp() {
        let data = RailedTransportDockUpdateData::default();
        assert_eq!(data.pull_inside_duration_frames, 0);
        assert_eq!(data.push_outside_duration_frames, 0);
        assert_eq!(data.tolerance_distance, 50.0);
    }

    #[test]
    fn safe_normalized_keeps_zero_length_vectors_finite() {
        assert_eq!(safe_normalized(Coord3D::ZERO), Coord3D::ZERO);

        let unit = safe_normalized(Coord3D::new(3.0, 4.0, 0.0));
        assert!(unit.x.is_finite());
        assert!(unit.y.is_finite());
        assert!((unit.length() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn railed_transport_loading_state() {
        let data = RailedTransportDockUpdateData::default();
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut dock = RailedTransportDockUpdate::new(data, 1, &pos);

        assert!(!dock.is_loading_or_unloading());
        dock.docking_object_id = 123;
        assert!(dock.is_loading_or_unloading());
        dock.docking_object_id = INVALID_ID;
        dock.unloading_object_id = 456;
        assert!(dock.is_loading_or_unloading());
        dock.unloading_object_id = INVALID_ID;
        assert!(!dock.is_loading_or_unloading());
    }
}
