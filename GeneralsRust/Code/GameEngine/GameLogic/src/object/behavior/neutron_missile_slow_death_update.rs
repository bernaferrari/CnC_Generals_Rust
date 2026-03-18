//! NeutronMissileSlowDeathUpdate - Neutron missile superweapon slow death behavior
//! Port of C++ NeutronMissileSlowDeathBehavior (NeutronMissileSlowDeathUpdate.cpp)

use crate::common::{Bool, Coord3D, KindOf, ModuleData, Real, UnsignedInt};
use crate::damage::DamageInfo;
use crate::damage::{DamageType, DeathType};
use crate::effects::FXList;
use crate::helpers::{TheGameClient, TheGameLogic, ThePartitionManager, TheTerrainLogic};
use crate::modules::{
    BehaviorModuleInterface, SlowDeathBehaviorInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::behavior::slow_death_behavior::SlowDeathPhaseType;
use crate::object::behavior::topple_update::{
    ToppleUpdate, TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, Mutex, RwLock, Weak};

const MAX_NEUTRON_BLASTS: usize = 9;
const SCORCH_1: i32 = 1;

#[derive(Clone, Copy, Debug)]
pub struct BlastInfo {
    pub enabled: Bool,
    pub delay: Real,
    pub scorch_delay: Real,
    pub inner_radius: Real,
    pub outer_radius: Real,
    pub max_damage: Real,
    pub min_damage: Real,
    pub topple_speed: Real,
    pub push_force_mag: Real,
}

impl Default for BlastInfo {
    fn default() -> Self {
        Self {
            enabled: false,
            delay: 0.0,
            scorch_delay: 0.0,
            inner_radius: 0.0,
            outer_radius: 0.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.0,
            push_force_mag: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NeutronMissileSlowDeathUpdateModuleData {
    pub base: BehaviorModuleData,
    pub probability_modifier: i32,
    pub scorch_size: Real,
    pub fx_list: Option<Arc<FXList>>,
    pub blast_info: [BlastInfo; MAX_NEUTRON_BLASTS],
}

impl Default for NeutronMissileSlowDeathUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            probability_modifier: 1,
            scorch_size: 0.0,
            fx_list: None,
            blast_info: std::array::from_fn(|_| BlastInfo::default()),
        }
    }
}

crate::impl_behavior_module_data_via_base!(NeutronMissileSlowDeathUpdateModuleData, base);

pub struct NeutronMissileSlowDeathUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<NeutronMissileSlowDeathUpdateModuleData>,
    activated: Bool,
    scorch_placed: Bool,
    activation_frame: UnsignedInt,
    completed_blasts: [Bool; MAX_NEUTRON_BLASTS],
    completed_scorch_blasts: [Bool; MAX_NEUTRON_BLASTS],
}

impl NeutronMissileSlowDeathUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<NeutronMissileSlowDeathUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            activated: false,
            scorch_placed: false,
            activation_frame: 0,
            completed_blasts: [false; MAX_NEUTRON_BLASTS],
            completed_scorch_blasts: [false; MAX_NEUTRON_BLASTS],
        })
    }

    fn ensure_activation(&mut self, obj: &GameObject) {
        if self.activation_frame != 0 {
            return;
        }

        let mut pos = *obj.get_position();
        pos.z = TheTerrainLogic::get()
            .map(|terrain| terrain.get_ground_height(pos.x, pos.y, None))
            .unwrap_or(0.0);
        self.activation_frame = TheGameLogic::get_frame();

        if let Some(fx) = &self.module_data.fx_list {
            let _ = fx.do_fx_at_position(&pos);
        }
    }

    fn do_blast(&mut self, blast_info: &BlastInfo, obj: &GameObject) {
        if blast_info.outer_radius <= 0.0 {
            return;
        }

        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        let missile_pos = *obj.get_position();
        let mut damage_info = DamageInfo::with_simple(
            blast_info.min_damage,
            obj.get_id(),
            DamageType::Explosion,
            DeathType::Exploded,
        );

        let candidates = partition.get_objects_in_range(&missile_pos, blast_info.outer_radius);
        for id in candidates {
            let Some(other_arc) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(mut other) = other_arc.write() else {
                continue;
            };

            let other_pos = *other.get_position();
            let force_vector = other_pos - missile_pos;

            if let Some(module) = other.find_update_module("ToppleUpdate") {
                let _ = module.with_module_downcast::<
                    crate::object::behavior::topple_update::ToppleUpdateModule,
                    _,
                    _,
                >(|module| {
                    module.behavior_mut().apply_toppling_force(
                        &force_vector,
                        blast_info.topple_speed,
                        TOPPLE_OPTIONS_NO_BOUNCE | TOPPLE_OPTIONS_NO_FX,
                    );
                });
            }

            let dist = force_vector.length();
            let amount = if dist <= blast_info.inner_radius {
                blast_info.max_damage
            } else {
                let denom = (blast_info.outer_radius - blast_info.inner_radius + 0.01).max(0.01);
                let percent = (1.0 - ((dist - blast_info.inner_radius) / denom)).clamp(0.0, 1.0);
                let mut scaled = blast_info.max_damage * percent;
                if scaled < blast_info.min_damage {
                    scaled = blast_info.min_damage;
                }
                scaled
            };

            if amount > 0.0 {
                damage_info.input.amount = amount;
                damage_info.sync_from_input();
                let _ = other.attempt_damage(&mut damage_info);

                if !self.scorch_placed {
                    if let Some(client) = TheGameClient::get() {
                        client.add_scorch(&missile_pos, self.module_data.scorch_size, SCORCH_1);
                        self.scorch_placed = true;
                    }
                }
            }
        }
    }

    fn do_scorch_blast(&mut self, blast_info: &BlastInfo, obj: &GameObject) {
        if blast_info.outer_radius <= 0.0 {
            return;
        }

        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        let missile_pos = *obj.get_position();
        let candidates = partition.get_objects_in_range(&missile_pos, blast_info.outer_radius);
        for id in candidates {
            let Some(other_arc) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(mut other) = other_arc.write() else {
                continue;
            };

            other.set_model_condition_state(crate::common::ModelConditionFlags::BURNED);

            if other.is_kind_of(KindOf::Shrubbery) {
                if let Some(drawable) = other.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.set_shadows_enabled(false);
                    }
                }
            }
        }
    }
}

impl UpdateModuleInterface for NeutronMissileSlowDeathUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.activated {
            return UpdateSleepTime::None;
        }

        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(obj) = object_arc.read() else {
            return UpdateSleepTime::None;
        };

        self.ensure_activation(&obj);

        let curr_frame = TheGameLogic::get_frame();
        let elapsed = (curr_frame - self.activation_frame) as Real;

        for i in 0..MAX_NEUTRON_BLASTS {
            let blast = self.module_data.blast_info[i];
            if !blast.enabled {
                continue;
            }

            if !self.completed_blasts[i] && elapsed > blast.delay {
                self.do_blast(&blast, &obj);
                self.completed_blasts[i] = true;
            }

            if !self.completed_scorch_blasts[i] && elapsed > blast.scorch_delay {
                self.do_scorch_blast(&blast, &obj);
                self.completed_scorch_blasts[i] = true;
            }
        }

        UpdateSleepTime::None
    }
}

impl SlowDeathBehaviorInterface for NeutronMissileSlowDeathUpdate {
    fn is_slow_death_active(&self) -> bool {
        self.activated
    }

    fn begin_slow_death(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.activated = true;
        self.activation_frame = 0;
        Ok(())
    }

    fn get_probability_modifier(&self, _damage_info: &DamageInfo) -> i32 {
        self.module_data.probability_modifier.max(1)
    }

    fn is_die_applicable(&self, _damage_info: &DamageInfo) -> bool {
        true
    }

    fn get_slow_death_phase(&self) -> u32 {
        if !self.activated {
            return SlowDeathPhaseType::Initial as u32;
        }

        let mut all_done = true;
        for i in 0..MAX_NEUTRON_BLASTS {
            if !self.module_data.blast_info[i].enabled {
                continue;
            }

            if !self.completed_blasts[i] || !self.completed_scorch_blasts[i] {
                all_done = false;
                break;
            }
        }

        if all_done {
            SlowDeathPhaseType::Final as u32
        } else {
            SlowDeathPhaseType::Midpoint as u32
        }
    }
}

impl BehaviorModuleInterface for NeutronMissileSlowDeathUpdate {
    fn get_module_name(&self) -> &'static str {
        "NeutronMissileSlowDeathBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_slow_death_behavior_interface(&mut self) -> Option<&mut dyn SlowDeathBehaviorInterface> {
        Some(self)
    }
}

impl Snapshotable for NeutronMissileSlowDeathUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_bool(&mut self.activated)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer activated: {:?}", e))?;
        xfer.xfer_bool(&mut self.scorch_placed)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer scorch_placed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.activation_frame)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer activation_frame: {:?}", e))?;
        for completed in &mut self.completed_blasts {
            xfer.xfer_bool(completed)
                .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer completed_blast: {:?}", e))?;
        }
        for completed in &mut self.completed_scorch_blasts {
            xfer.xfer_bool(completed)
                .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer completed_scorch: {:?}", e))?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct NeutronMissileSlowDeathUpdateFactory;
impl NeutronMissileSlowDeathUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(NeutronMissileSlowDeathUpdate::new(
            thing,
            module_data,
        )?))
    }
}
