//! FirestormDynamicGeometryInfoUpdate - Rust conversion of C++ FirestormDynamicGeometryInfoUpdate
//!
//! Update module that adds firestorm effects (particle systems, scorch, damage)
//! to the dynamic geometry transition.
//! Author: Graham Smallwood, April 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::EmissionVolumeType;
use crate::common::{Coord3D, ModuleData, Real};
use crate::damage::{DamageInfo, DamageInfoInput, DamageType, DeathType};
use crate::helpers::{
    TheFXList, TheGameClient, TheGameLogic, TheParticleSystemManager, ThePartitionManager,
    TheTerrainLogic,
};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::dynamic_geometry_info_update::{
    DynamicGeometryInfoUpdateLogic, DynamicGeometryInfoUpdateModuleData,
};
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

const MAX_FIRESTORM_SYSTEMS: usize = 16;
const INVALID_PARTICLE_SYSTEM_ID: u32 = 0;

/// INI-configurable data for FirestormDynamicGeometryInfoUpdate
#[derive(Clone, Debug)]
pub struct FirestormDynamicGeometryInfoUpdateModuleData {
    pub base: DynamicGeometryInfoUpdateModuleData,
    pub fx_list: Option<String>,
    pub particle_systems: [Option<String>; MAX_FIRESTORM_SYSTEMS],
    pub particle_offset_z: Real,
    pub scorch_size: Real,
    pub delay_between_damage_frames: u32,
    pub damage_amount: Real,
    pub max_height_for_damage: Real,
}

impl Default for FirestormDynamicGeometryInfoUpdateModuleData {
    fn default() -> Self {
        Self {
            base: DynamicGeometryInfoUpdateModuleData::default(),
            fx_list: None,
            particle_systems: Default::default(),
            particle_offset_z: 0.0,
            scorch_size: 0.0,
            delay_between_damage_frames: 0,
            damage_amount: 0.0,
            max_height_for_damage: 20.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(FirestormDynamicGeometryInfoUpdateModuleData, base);

/// FirestormDynamicGeometryInfoUpdate - firestorm effects during geometry transition
pub struct FirestormDynamicGeometryInfoUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<FirestormDynamicGeometryInfoUpdateModuleData>,
    pub logic: DynamicGeometryInfoUpdateLogic,

    particle_system_ids: [u32; MAX_FIRESTORM_SYSTEMS],
    effects_fired: bool,
    scorch_placed: bool,
    last_damage_frame: u32,
}

impl FirestormDynamicGeometryInfoUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
        .downcast_ref::<FirestormDynamicGeometryInfoUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            logic: DynamicGeometryInfoUpdateLogic::new(&data.base),
            module_data: Arc::new(data.clone()),
            particle_system_ids: [INVALID_PARTICLE_SYSTEM_ID; MAX_FIRESTORM_SYSTEMS],
            effects_fired: false,
            scorch_placed: false,
            last_damage_frame: 0,
        })
    }

    fn do_damage_scan(&mut self, object: &GameObject) {
        let pos = *object.get_position();
        let radius = object.get_geometry_info().get_bounding_circle_radius();

        if radius <= 0.0 {
            return;
        }

        let damage_info = DamageInfo {
            input: DamageInfoInput {
                amount: self.module_data.damage_amount,
                damage_type: DamageType::Flame,
                death_type: DeathType::Burned,
                source_id: object.get_id(),
                ..Default::default()
            },
            ..Default::default()
        };

        if let Some(partition) = ThePartitionManager::get() {
            for id in partition.get_objects_in_range_boundary_2d(&pos, radius) {
                let Some(target_arc) = TheGameLogic::find_object_by_id(id) else {
                    continue;
                };
                let Ok(mut target) = target_arc.write() else {
                    continue;
                };

                if target.get_position().z > pos.z + self.module_data.max_height_for_damage {
                    continue;
                }

                let mut dmg = damage_info.clone();
                dmg.sync_from_input();
                let _ = target.attempt_damage(&mut dmg);
            }
        }
    }
}

impl UpdateModuleInterface for FirestormDynamicGeometryInfoUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let obj_arc = match self.object.upgrade() {
            Some(arc) => arc,
            None => return UPDATE_SLEEP_NONE,
        };

        let obj = match obj_arc.read() {
            Ok(guard) => guard,
            Err(_) => return UPDATE_SLEEP_NONE,
        };

        // Call base transition logic
        let res = self.logic.update_step(&obj);

        // Don't do firestorm stuff if still in initial delay
        if !self.logic.started {
            return res;
        }

        // Fired effects for the first time
        if !self.effects_fired {
            let pos = *obj.get_position();
            let terrain_height = if let Some(terrain) = TheTerrainLogic::get() {
                terrain.get_height_at(pos.x, pos.y)
            } else {
                0.0
            };

            let effect_pos = Coord3D::new(
                pos.x,
                pos.y,
                terrain_height + self.module_data.particle_offset_z,
            );

            if let Some(mgr) = TheParticleSystemManager::get() {
                for i in 0..MAX_FIRESTORM_SYSTEMS {
                    if let Some(system_id) =
                        mgr.create_particle_system(self.module_data.particle_systems[i].as_deref())
                    {
                        self.particle_system_ids[i] = system_id;
                        mgr.set_particle_system_position(system_id, &effect_pos);
                    }
                }
            }

            if let Some(fx) = TheFXList::get() {
                if let Some(fx_name) = &self.module_data.fx_list {
                    fx.do_fx_at_position(fx_name, &effect_pos);
                }
            }

            self.effects_fired = true;
            // recordFirestormCreated would be here
        }

        // Update particle system radii
        if let Some(mgr) = TheParticleSystemManager::get() {
            let major_radius = obj.get_geometry_info().get_major_radius();
            for i in 0..MAX_FIRESTORM_SYSTEMS {
                if self.particle_system_ids[i] != INVALID_PARTICLE_SYSTEM_ID {
                    if mgr
                        .find_particle_system(self.particle_system_ids[i])
                        .is_some()
                    {
                        let emission_type = mgr
                            .get_particle_system_emission_volume_type(self.particle_system_ids[i])
                            .unwrap_or(EmissionVolumeType::Sphere);
                        match emission_type {
                            EmissionVolumeType::Sphere | EmissionVolumeType::None => {
                                mgr.set_particle_system_emission_volume_sphere_radius(
                                    self.particle_system_ids[i],
                                    major_radius,
                                );
                            }
                            EmissionVolumeType::Cylinder => {
                                mgr.set_particle_system_emission_volume_cylinder_radius(
                                    self.particle_system_ids[i],
                                    major_radius,
                                );
                            }
                        }
                    } else {
                        self.particle_system_ids[i] = INVALID_PARTICLE_SYSTEM_ID;
                    }
                }
            }
        }

        // Place scorch mark when reversed
        if self.logic.switched_directions && !self.scorch_placed {
            if let Some(client) = TheGameClient::get() {
                client.add_scorch(obj.get_position(), self.module_data.scorch_size, 0);
                // 0 = default scorch type
            }
            self.scorch_placed = true;
        }

        // Periodic damage scan
        let current_frame = TheGameLogic::get_frame();
        if current_frame - self.last_damage_frame >= self.module_data.delay_between_damage_frames {
            self.do_damage_scan(&obj);
            self.last_damage_frame = current_frame;
        }

        res
    }
}

impl BehaviorModuleInterface for FirestormDynamicGeometryInfoUpdate {
    fn get_module_name(&self) -> &'static str {
        "FirestormDynamicGeometryInfoUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FirestormDynamicGeometryInfoUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Transfer base geometry logic
        xfer.xfer_unsigned_int(&mut self.logic.starting_delay_countdown)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer starting_delay_countdown: {:?}",
                    e
                )
            })?;
        xfer.xfer_unsigned_int(&mut self.logic.time_active)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer time_active: {:?}",
                    e
                )
            })?;
        xfer.xfer_bool(&mut self.logic.started)
            .map_err(|e| format!("FirestormDynamicGeometryInfoUpdate xfer started: {:?}", e))?;
        xfer.xfer_bool(&mut self.logic.finished)
            .map_err(|e| format!("FirestormDynamicGeometryInfoUpdate xfer finished: {:?}", e))?;
        xfer.xfer_bool(&mut self.logic.switched_directions)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer switched_directions: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.logic.initial_height)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer initial_height: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.logic.initial_major_radius)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer initial_major_radius: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.logic.initial_minor_radius)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer initial_minor_radius: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.logic.final_height).map_err(|e| {
            format!(
                "FirestormDynamicGeometryInfoUpdate xfer final_height: {:?}",
                e
            )
        })?;
        xfer.xfer_real(&mut self.logic.final_major_radius)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer final_major_radius: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.logic.final_minor_radius)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer final_minor_radius: {:?}",
                    e
                )
            })?;
        xfer.xfer_unsigned_int(&mut self.logic.transition_time)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer transition_time: {:?}",
                    e
                )
            })?;

        // Transfer firestorm-specific state
        xfer.xfer_bool(&mut self.effects_fired).map_err(|e| {
            format!(
                "FirestormDynamicGeometryInfoUpdate xfer effects_fired: {:?}",
                e
            )
        })?;
        xfer.xfer_bool(&mut self.scorch_placed).map_err(|e| {
            format!(
                "FirestormDynamicGeometryInfoUpdate xfer scorch_placed: {:?}",
                e
            )
        })?;
        xfer.xfer_unsigned_int(&mut self.last_damage_frame)
            .map_err(|e| {
                format!(
                    "FirestormDynamicGeometryInfoUpdate xfer last_damage_frame: {:?}",
                    e
                )
            })?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct FirestormDynamicGeometryInfoUpdateFactory;
impl FirestormDynamicGeometryInfoUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(FirestormDynamicGeometryInfoUpdate::new(
            thing,
            module_data,
        )?))
    }
}
