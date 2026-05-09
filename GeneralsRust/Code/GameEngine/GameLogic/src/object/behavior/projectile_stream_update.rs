//! ProjectileStreamUpdate - Continuous projectile stream weapon
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, CoordOrigin, ModuleData, ObjectID, Real, UnsignedInt, XferVersion,
};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct ProjectileStreamUpdateModuleData {
    pub base: BehaviorModuleData,
}

impl Default for ProjectileStreamUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(ProjectileStreamUpdateModuleData, base);

impl ProjectileStreamUpdateModuleData {
    pub fn parse_from_ini(&mut self, _ini: &mut INI) -> Result<(), INIError> {
        Ok(())
    }
}

pub struct ProjectileStreamUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<ProjectileStreamUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    projectile_ids: [ObjectID; MAX_PROJECTILE_STREAM],
    next_free_index: i32,
    first_valid_index: i32,
    owning_object: ObjectID,
    target_object: ObjectID,
    target_position: crate::common::Coord3D,
}

impl ProjectileStreamUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<ProjectileStreamUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            projectile_ids: [OBJECT_INVALID_ID; MAX_PROJECTILE_STREAM],
            next_free_index: 0,
            first_valid_index: 0,
            owning_object: OBJECT_INVALID_ID,
            target_object: OBJECT_INVALID_ID,
            target_position: crate::common::Coord3D::origin(),
        })
    }

    pub fn add_projectile(
        &mut self,
        source_id: ObjectID,
        new_id: ObjectID,
        victim_id: ObjectID,
        victim_pos: Option<&crate::common::Coord3D>,
    ) {
        if self.owning_object == OBJECT_INVALID_ID {
            self.owning_object = source_id;
        } else {
            debug_assert!(
                self.owning_object == source_id,
                "Two objects are trying to use the same Projectile stream."
            );
        }

        if victim_id != OBJECT_INVALID_ID {
            if victim_id != self.target_object {
                self.projectile_ids[self.next_free_index as usize] = OBJECT_INVALID_ID;
                self.next_free_index = (self.next_free_index + 1) % MAX_PROJECTILE_STREAM as i32;
                self.target_object = victim_id;
            }
            self.target_position = crate::common::Coord3D::origin();
        } else if let Some(pos) = victim_pos {
            if self.target_position != *pos {
                self.projectile_ids[self.next_free_index as usize] = OBJECT_INVALID_ID;
                self.next_free_index = (self.next_free_index + 1) % MAX_PROJECTILE_STREAM as i32;
                self.target_position = *pos;
            }
            self.target_object = OBJECT_INVALID_ID;
        } else {
            debug_assert!(
                false,
                "Projectile stream fired at neither object nor position."
            );
        }

        self.projectile_ids[self.next_free_index as usize] = new_id;
        self.next_free_index = (self.next_free_index + 1) % MAX_PROJECTILE_STREAM as i32;
        debug_assert!(
            self.next_free_index != self.first_valid_index,
            "Need to increase the allowed number of simultaneous particles in ProjectileStreamUpdate."
        );
    }

    pub fn get_all_points(&self) -> Vec<crate::common::Coord3D> {
        let mut points = Vec::with_capacity(MAX_PROJECTILE_STREAM);
        let mut point_index = self.first_valid_index;

        let owning_object = if self.owning_object != OBJECT_INVALID_ID {
            TheGameLogic::find_object_by_id(self.owning_object)
        } else {
            None
        };

        while point_index != self.next_free_index {
            let projectile_id = self.projectile_ids[point_index as usize];
            if let Some(projectile) = TheGameLogic::find_object_by_id(projectile_id) {
                if let Ok(projectile_guard) = projectile.read() {
                    let mut point = *projectile_guard.get_position();

                    if let Some(owner) = owning_object.as_ref().and_then(|obj| obj.read().ok()) {
                        if owner.is_kind_of(crate::common::KindOf::Vehicle) {
                            let pos = owner.get_position();
                            let my_top = owner.get_geometry_info().get_max_height_above_position()
                                + pos.z
                                + 0.5;
                            let delta =
                                crate::common::Coord3D::new(pos.x - point.x, pos.y - point.y, 0.0);
                            if delta.length() <= owner.get_geometry_info().get_major_radius() * 1.5
                            {
                                point.z = point.z.max(my_top);
                            }
                        }
                    }

                    points.push(point);
                } else {
                    points.push(crate::common::Coord3D::origin());
                }
            } else {
                let fallback_pos = crate::weapon::with_projectile_manager(|manager| {
                    manager
                        .get_projectile(projectile_id)
                        .map(|projectile| projectile.physics.position)
                });
                if let Some(pos) = fallback_pos {
                    points.push(pos);
                } else {
                    points.push(crate::common::Coord3D::origin());
                }
            }

            point_index = (point_index + 1) % MAX_PROJECTILE_STREAM as i32;
        }

        points
    }

    pub fn set_position(&mut self, new_position: &crate::common::Coord3D) {
        if let Some(object) = self.object.upgrade() {
            if let Ok(mut guard) = object.write() {
                if let Err(err) = guard.set_position(new_position) {
                    log::debug!("ProjectileStreamUpdate::set_position failed: {err}");
                }
            }
        }
    }

    fn cull_front_of_list(&mut self) {
        while self.first_valid_index != self.next_free_index {
            let id = self.projectile_ids[self.first_valid_index as usize];
            if TheGameLogic::find_object_by_id(id).is_some() {
                break;
            }
            self.first_valid_index = (self.first_valid_index + 1) % MAX_PROJECTILE_STREAM as i32;
        }
    }

    fn consider_dying(&self) -> bool {
        if self.first_valid_index == self.next_free_index && self.owning_object != OBJECT_INVALID_ID
        {
            return TheGameLogic::find_object_by_id(self.owning_object).is_none();
        }
        false
    }
}

impl UpdateModuleInterface for ProjectileStreamUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.cull_front_of_list();

        if self.consider_dying() {
            if let Some(obj) = self.object.upgrade() {
                if let Ok(obj_guard) = obj.read() {
                    if let Err(err) = TheGameLogic::destroy_object(&obj_guard) {
                        log::debug!("ProjectileStreamUpdate::destroy_object failed: {err}");
                    }
                }
            }
        }

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for ProjectileStreamUpdate {
    fn get_module_name(&self) -> &'static str {
        "ProjectileStreamUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for ProjectileStreamUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        for id in &mut self.projectile_ids {
            xfer.xfer_object_id(id).map_err(|e| e.to_string())?;
        }
        xfer.xfer_i32(&mut self.next_free_index)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.first_valid_index)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.owning_object)
            .map_err(|e| e.to_string())?;
        if version >= 2 {
            xfer.xfer_object_id(&mut self.target_object)
                .map_err(|e| e.to_string())?;
            xfer.xfer_coord3d(&mut self.target_position);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes ProjectileStreamUpdate through the common Module trait.
pub struct ProjectileStreamUpdateModule {
    behavior: ProjectileStreamUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<ProjectileStreamUpdateModuleData>,
}

impl ProjectileStreamUpdateModule {
    pub fn new(
        behavior: ProjectileStreamUpdate,
        module_name: &AsciiString,
        module_data: Arc<ProjectileStreamUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut ProjectileStreamUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for ProjectileStreamUpdateModule {
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

impl Module for ProjectileStreamUpdateModule {
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
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

pub struct ProjectileStreamUpdateFactory;
impl ProjectileStreamUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(ProjectileStreamUpdate::new(thing, module_data)?))
    }
}

pub const MAX_PROJECTILE_STREAM: usize = 20;
