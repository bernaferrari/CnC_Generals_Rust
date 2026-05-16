//! NeutronMissileSlowDeathUpdate - Neutron missile superweapon slow death behavior
//! Port of C++ NeutronMissileSlowDeathBehavior (NeutronMissileSlowDeathUpdate.cpp)

use crate::common::{Bool, Coord3D, KindOf, ModuleData, Real, UnsignedByte, UnsignedInt};
use crate::damage::DamageInfo;
use crate::damage::{DamageType, DeathType};
use crate::effects::FXList;
use crate::helpers::{
    TheFXListStore, TheGameClient, TheGameLogic, ThePartitionManager, TheTerrainLogic,
};
use crate::modules::{
    BehaviorModuleInterface, SlowDeathBehaviorInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::behavior::slow_death_behavior::SlowDeathPhaseType;
use crate::object::behavior::topple_update::{TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, Mutex, RwLock, Weak};

const MAX_NEUTRON_BLASTS: usize = 9;
const SCORCH_1: i32 = 1;
const SLOW_DEATH_ACTIVATED: UnsignedInt = 1 << 0;

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

impl NeutronMissileSlowDeathUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, NEUTRON_MISSILE_SLOW_DEATH_FIELDS)
    }
}

fn parse_real_token(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_duration_real_token(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_real(token)
}

fn parse_fx_list(
    _ini: &mut INI,
    data: &mut NeutronMissileSlowDeathUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.fx_list = None;
    } else {
        data.fx_list = TheFXListStore::find_fx_list(token);
    }
    Ok(())
}

fn parse_blast_field(
    data: &mut NeutronMissileSlowDeathUpdateModuleData,
    index: usize,
    tokens: &[&str],
    setter: impl FnOnce(&mut BlastInfo, &[&str]) -> Result<(), INIError>,
) -> Result<(), INIError> {
    let blast = data
        .blast_info
        .get_mut(index)
        .ok_or(INIError::InvalidData)?;
    setter(blast, tokens)
}

macro_rules! blast_field {
    ($index:expr, $token:literal, enabled) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    let token = tokens.first().ok_or(INIError::InvalidData)?;
                    blast.enabled = INI::parse_bool(token)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, delay) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.delay = parse_duration_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, scorch_delay) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.scorch_delay = parse_duration_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, inner_radius) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.inner_radius = parse_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, outer_radius) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.outer_radius = parse_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, max_damage) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.max_damage = parse_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, min_damage) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.min_damage = parse_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, topple_speed) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.topple_speed = parse_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
    ($index:expr, $token:literal, push_force) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                parse_blast_field(data, $index, tokens, |blast, tokens| {
                    blast.push_force_mag = parse_real_token(tokens)?;
                    Ok(())
                })
            },
        }
    };
}

const NEUTRON_MISSILE_SLOW_DEATH_FIELDS: &[FieldParse<NeutronMissileSlowDeathUpdateModuleData>] = &[
    FieldParse {
        token: "ProbabilityModifier",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.probability_modifier = INI::parse_int(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ScorchMarkSize",
        parse: |_, data, tokens| {
            data.scorch_size = parse_real_token(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FXList",
        parse: parse_fx_list,
    },
    blast_field!(0, "Blast1Enabled", enabled),
    blast_field!(0, "Blast1Delay", delay),
    blast_field!(0, "Blast1ScorchDelay", scorch_delay),
    blast_field!(0, "Blast1InnerRadius", inner_radius),
    blast_field!(0, "Blast1OuterRadius", outer_radius),
    blast_field!(0, "Blast1MaxDamage", max_damage),
    blast_field!(0, "Blast1MinDamage", min_damage),
    blast_field!(0, "Blast1ToppleSpeed", topple_speed),
    blast_field!(0, "Blast1PushForce", push_force),
    blast_field!(1, "Blast2Enabled", enabled),
    blast_field!(1, "Blast2Delay", delay),
    blast_field!(1, "Blast2ScorchDelay", scorch_delay),
    blast_field!(1, "Blast2InnerRadius", inner_radius),
    blast_field!(1, "Blast2OuterRadius", outer_radius),
    blast_field!(1, "Blast2MaxDamage", max_damage),
    blast_field!(1, "Blast2MinDamage", min_damage),
    blast_field!(1, "Blast2ToppleSpeed", topple_speed),
    blast_field!(1, "Blast2PushForce", push_force),
    blast_field!(2, "Blast3Enabled", enabled),
    blast_field!(2, "Blast3Delay", delay),
    blast_field!(2, "Blast3ScorchDelay", scorch_delay),
    blast_field!(2, "Blast3InnerRadius", inner_radius),
    blast_field!(2, "Blast3OuterRadius", outer_radius),
    blast_field!(2, "Blast3MaxDamage", max_damage),
    blast_field!(2, "Blast3MinDamage", min_damage),
    blast_field!(2, "Blast3ToppleSpeed", topple_speed),
    blast_field!(2, "Blast3PushForce", push_force),
    blast_field!(3, "Blast4Enabled", enabled),
    blast_field!(3, "Blast4Delay", delay),
    blast_field!(3, "Blast4ScorchDelay", scorch_delay),
    blast_field!(3, "Blast4InnerRadius", inner_radius),
    blast_field!(3, "Blast4OuterRadius", outer_radius),
    blast_field!(3, "Blast4MaxDamage", max_damage),
    blast_field!(3, "Blast4MinDamage", min_damage),
    blast_field!(3, "Blast4ToppleSpeed", topple_speed),
    blast_field!(3, "Blast4PushForce", push_force),
    blast_field!(4, "Blast5Enabled", enabled),
    blast_field!(4, "Blast5Delay", delay),
    blast_field!(4, "Blast5ScorchDelay", scorch_delay),
    blast_field!(4, "Blast5InnerRadius", inner_radius),
    blast_field!(4, "Blast5OuterRadius", outer_radius),
    blast_field!(4, "Blast5MaxDamage", max_damage),
    blast_field!(4, "Blast5MinDamage", min_damage),
    blast_field!(4, "Blast5ToppleSpeed", topple_speed),
    blast_field!(4, "Blast5PushForce", push_force),
    blast_field!(5, "Blast6Enabled", enabled),
    blast_field!(5, "Blast6Delay", delay),
    blast_field!(5, "Blast6ScorchDelay", scorch_delay),
    blast_field!(5, "Blast6InnerRadius", inner_radius),
    blast_field!(5, "Blast6OuterRadius", outer_radius),
    blast_field!(5, "Blast6MaxDamage", max_damage),
    blast_field!(5, "Blast6MinDamage", min_damage),
    blast_field!(5, "Blast6ToppleSpeed", topple_speed),
    blast_field!(5, "Blast6PushForce", push_force),
    blast_field!(6, "Blast7Enabled", enabled),
    blast_field!(6, "Blast7Delay", delay),
    blast_field!(6, "Blast7ScorchDelay", scorch_delay),
    blast_field!(6, "Blast7InnerRadius", inner_radius),
    blast_field!(6, "Blast7OuterRadius", outer_radius),
    blast_field!(6, "Blast7MaxDamage", max_damage),
    blast_field!(6, "Blast7MinDamage", min_damage),
    blast_field!(6, "Blast7ToppleSpeed", topple_speed),
    blast_field!(6, "Blast7PushForce", push_force),
    blast_field!(7, "Blast8Enabled", enabled),
    blast_field!(7, "Blast8Delay", delay),
    blast_field!(7, "Blast8ScorchDelay", scorch_delay),
    blast_field!(7, "Blast8InnerRadius", inner_radius),
    blast_field!(7, "Blast8OuterRadius", outer_radius),
    blast_field!(7, "Blast8MaxDamage", max_damage),
    blast_field!(7, "Blast8MinDamage", min_damage),
    blast_field!(7, "Blast8ToppleSpeed", topple_speed),
    blast_field!(7, "Blast8PushForce", push_force),
    blast_field!(8, "Blast9Enabled", enabled),
    blast_field!(8, "Blast9Delay", delay),
    blast_field!(8, "Blast9ScorchDelay", scorch_delay),
    blast_field!(8, "Blast9InnerRadius", inner_radius),
    blast_field!(8, "Blast9OuterRadius", outer_radius),
    blast_field!(8, "Blast9MaxDamage", max_damage),
    blast_field!(8, "Blast9MinDamage", min_damage),
    blast_field!(8, "Blast9ToppleSpeed", topple_speed),
    blast_field!(8, "Blast9PushForce", push_force),
];

pub struct NeutronMissileSlowDeathUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<NeutronMissileSlowDeathUpdateModuleData>,
    activated: Bool,
    next_call_frame_and_phase: UnsignedInt,
    slow_death_sink_frame: UnsignedInt,
    slow_death_midpoint_frame: UnsignedInt,
    slow_death_destruction_frame: UnsignedInt,
    slow_death_accelerated_time_scale: Real,
    slow_death_flags: UnsignedInt,
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
            .downcast_ref::<NeutronMissileSlowDeathUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            activated: false,
            next_call_frame_and_phase: 0,
            slow_death_sink_frame: 0,
            slow_death_midpoint_frame: 0,
            slow_death_destruction_frame: 0,
            slow_death_accelerated_time_scale: 1.0,
            slow_death_flags: 0,
            scorch_placed: false,
            activation_frame: 0,
            completed_blasts: [false; MAX_NEUTRON_BLASTS],
            completed_scorch_blasts: [false; MAX_NEUTRON_BLASTS],
        })
    }

    fn is_slow_death_activated(&self) -> bool {
        self.activated || (self.slow_death_flags & SLOW_DEATH_ACTIVATED) != 0
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
                module.with_module(|module| {
                    if let Some(topple) = module.get_topple_control_interface() {
                        topple.apply_toppling_force(
                            force_vector.x,
                            force_vector.y,
                            force_vector.z,
                            blast_info.topple_speed,
                            TOPPLE_OPTIONS_NO_BOUNCE | TOPPLE_OPTIONS_NO_FX,
                        );
                    }
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
        if !self.is_slow_death_activated() {
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
        self.is_slow_death_activated()
    }

    fn begin_slow_death(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.activated = true;
        self.slow_death_flags |= SLOW_DEATH_ACTIVATED;
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
        if !self.is_slow_death_activated() {
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer version: {:?}", e))?;

        let mut slow_death_version: XferVersion = 1;
        xfer.xfer_version(&mut slow_death_version, 1).map_err(|e| {
            format!(
                "NeutronMissileSlowDeathUpdate xfer slow death version: {:?}",
                e
            )
        })?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.slow_death_sink_frame)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer sink frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.slow_death_midpoint_frame)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer midpoint frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.slow_death_destruction_frame)
            .map_err(|e| {
                format!(
                    "NeutronMissileSlowDeathUpdate xfer destruction frame: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.slow_death_accelerated_time_scale)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer time scale: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.slow_death_flags)
            .map_err(|e| {
                format!(
                    "NeutronMissileSlowDeathUpdate xfer slow death flags: {:?}",
                    e
                )
            })?;

        xfer.xfer_unsigned_int(&mut self.activation_frame)
            .map_err(|e| {
                format!(
                    "NeutronMissileSlowDeathUpdate xfer activation_frame: {:?}",
                    e
                )
            })?;

        let mut max_neutron_blasts: UnsignedByte = MAX_NEUTRON_BLASTS as UnsignedByte;
        xfer.xfer_unsigned_byte(&mut max_neutron_blasts)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer blast count: {:?}", e))?;
        if max_neutron_blasts as usize != MAX_NEUTRON_BLASTS {
            return Err(format!(
                "NeutronMissileSlowDeathUpdate invalid blast count: {}",
                max_neutron_blasts
            ));
        }

        for completed in &mut self.completed_blasts {
            xfer.xfer_bool(completed).map_err(|e| {
                format!(
                    "NeutronMissileSlowDeathUpdate xfer completed_blast: {:?}",
                    e
                )
            })?;
        }
        for completed in &mut self.completed_scorch_blasts {
            xfer.xfer_bool(completed).map_err(|e| {
                format!(
                    "NeutronMissileSlowDeathUpdate xfer completed_scorch: {:?}",
                    e
                )
            })?;
        }
        xfer.xfer_bool(&mut self.scorch_placed)
            .map_err(|e| format!("NeutronMissileSlowDeathUpdate xfer scorch_placed: {:?}", e))?;

        self.activated = (self.slow_death_flags & SLOW_DEATH_ACTIVATED) != 0;
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

pub struct NeutronMissileSlowDeathUpdateModule {
    module_name_key: NameKeyType,
    module_data: Arc<NeutronMissileSlowDeathUpdateModuleData>,
    behavior: NeutronMissileSlowDeathUpdate,
}

impl NeutronMissileSlowDeathUpdateModule {
    fn new(
        module_name: &str,
        module_data: Arc<NeutronMissileSlowDeathUpdateModuleData>,
        behavior: NeutronMissileSlowDeathUpdate,
    ) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            module_data,
            behavior,
        }
    }
}

impl Snapshotable for NeutronMissileSlowDeathUpdateModule {
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

impl EngineModule for NeutronMissileSlowDeathUpdateModule {
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

pub fn neutron_missile_slow_death_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = NeutronMissileSlowDeathUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse NeutronMissileSlowDeathBehavior data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn neutron_missile_slow_death_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn EngineModule> {
    let typed = module_data
        .as_any()
        .downcast_ref::<NeutronMissileSlowDeathUpdateModuleData>()
        .expect("NeutronMissileSlowDeathUpdateModuleData expected");

    let object_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(crate::common::INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("NeutronMissileSlowDeathBehavior requires an owning object");
    let shared_data = Arc::new(typed.clone());
    let behavior =
        NeutronMissileSlowDeathUpdate::new(object, Arc::clone(&shared_data) as Arc<dyn ModuleData>)
            .expect("NeutronMissileSlowDeathBehavior failed to initialize");

    Box::new(NeutronMissileSlowDeathUpdateModule::new(
        "NeutronMissileSlowDeathBehavior",
        shared_data,
        behavior,
    ))
}
