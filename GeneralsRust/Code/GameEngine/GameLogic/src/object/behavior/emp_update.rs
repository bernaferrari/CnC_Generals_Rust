//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/EMPUpdate.cpp`.
//!
//! EMPUpdate - Rust conversion of C++ EMPUpdate
//!
//! Handles EMP (Electromagnetic Pulse) effects on objects.
//! Author: EA Pacific (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Color, Coord3D, GameLogicRandomValueReal, Int, KindOf, KindOfMaskType,
    Matrix3D, ModuleData, Real, Relationship, UnsignedInt, XferVersion, PI,
};
use crate::helpers::{TheGameLogic, TheParticleSystemManager, ThePartitionManager};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::auto_heal_behavior::parse_kind_of_mask;
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::{
    registry::OBJECT_REGISTRY, DrawableArcExt, Object as GameObject,
    INVALID_ID as OBJECT_INVALID_ID,
};
use crate::weapon::WeaponAffectsMask;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

const WEAPON_AFFECTS_MASK_NAMES: &[&str] = &[
    "SELF",
    "ALLIES",
    "ENEMIES",
    "NEUTRALS",
    "SUICIDE",
    "NOT_SIMILAR",
    "NOT_AIRBORNE",
];

#[derive(Clone, Debug)]
pub struct EMPUpdateModuleData {
    pub base: BehaviorModuleData,
    pub life_frames: UnsignedInt,
    pub start_fade_frame: UnsignedInt,
    pub disabled_duration: UnsignedInt,
    pub start_scale: Real,
    pub target_scale_min: Real,
    pub target_scale_max: Real,
    pub start_color: Color,
    pub end_color: Color,
    pub disable_fx_particle_system: Option<AsciiString>,
    pub sparks_per_cubic_foot: Real,
    pub effect_radius: Real,
    pub reject_mask: Int,
    pub victim_kind_of: KindOfMaskType,
    pub victim_kind_of_not: KindOfMaskType,
    pub does_not_affect_my_own_buildings: Bool,
}

impl Default for EMPUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            life_frames: 1,
            start_fade_frame: 0,
            disabled_duration: 0,
            start_scale: 1.0,
            target_scale_min: 1.0,
            target_scale_max: 1.0,
            start_color: Color::white(),
            end_color: Color::black(),
            disable_fx_particle_system: None,
            sparks_per_cubic_foot: 0.001,
            effect_radius: 200.0,
            reject_mask: 0,
            victim_kind_of: 0,
            victim_kind_of_not: 0,
            does_not_affect_my_own_buildings: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(EMPUpdateModuleData, base);

impl EMPUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, EMP_UPDATE_FIELDS)
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_real(value)?);
    Ok(())
}

fn parse_duration_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(value)?);
    Ok(())
}

fn parse_bool_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Bool),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(value)?);
    Ok(())
}

fn parse_start_color(
    _ini: &mut INI,
    data: &mut EMPUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let filtered: Vec<&str> = tokens.iter().copied().filter(|t| *t != "=").collect();
    let (r, g, b) = INI::parse_rgb_color(&filtered)?;
    data.start_color = Color::rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    Ok(())
}

fn parse_end_color(
    _ini: &mut INI,
    data: &mut EMPUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let filtered: Vec<&str> = tokens.iter().copied().filter(|t| *t != "=").collect();
    let (r, g, b) = INI::parse_rgb_color(&filtered)?;
    data.end_color = Color::rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    Ok(())
}

fn parse_disable_fx_particle_system(
    _ini: &mut INI,
    data: &mut EMPUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("NONE") {
        data.disable_fx_particle_system = None;
    } else {
        data.disable_fx_particle_system = Some(AsciiString::from(value));
    }
    Ok(())
}

fn parse_reject_mask(
    _ini: &mut INI,
    data: &mut EMPUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let filtered: Vec<&str> = tokens.iter().copied().filter(|t| *t != "=").collect();
    data.reject_mask = INI::parse_bit_string_32(&filtered, WEAPON_AFFECTS_MASK_NAMES)? as Int;
    Ok(())
}

fn parse_victim_kind_of(
    _ini: &mut INI,
    data: &mut EMPUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.victim_kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_victim_kind_of_not(
    _ini: &mut INI,
    data: &mut EMPUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.victim_kind_of_not = parse_kind_of_mask(tokens);
    Ok(())
}

const EMP_UPDATE_FIELDS: &[FieldParse<EMPUpdateModuleData>] = &[
    FieldParse {
        token: "Lifetime",
        parse: |ini, data, tokens| parse_duration_field(ini, &mut |v| data.life_frames = v, tokens),
    },
    FieldParse {
        token: "StartFadeTime",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.start_fade_frame = v, tokens)
        },
    },
    FieldParse {
        token: "StartScale",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.start_scale = v, tokens),
    },
    FieldParse {
        token: "DisabledDuration",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.disabled_duration = v, tokens)
        },
    },
    FieldParse {
        token: "TargetScaleMax",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.target_scale_max = v, tokens)
        },
    },
    FieldParse {
        token: "TargetScaleMin",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.target_scale_min = v, tokens)
        },
    },
    FieldParse {
        token: "StartColor",
        parse: parse_start_color,
    },
    FieldParse {
        token: "EndColor",
        parse: parse_end_color,
    },
    FieldParse {
        token: "DisableFXParticleSystem",
        parse: parse_disable_fx_particle_system,
    },
    FieldParse {
        token: "SparksPerCubicFoot",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.sparks_per_cubic_foot = v, tokens)
        },
    },
    FieldParse {
        token: "EffectRadius",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.effect_radius = v, tokens),
    },
    FieldParse {
        token: "DoesNotAffect",
        parse: parse_reject_mask,
    },
    FieldParse {
        token: "DoesNotAffectMyOwnBuildings",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |v| data.does_not_affect_my_own_buildings = v,
                tokens,
            )
        },
    },
    FieldParse {
        token: "VictimRequiredKindOf",
        parse: parse_victim_kind_of,
    },
    FieldParse {
        token: "VictimForbiddenKindOf",
        parse: parse_victim_kind_of_not,
    },
];

pub struct EMPUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<EMPUpdateModuleData>,
    die_frame: UnsignedInt,
    tint_env_fade_frames: UnsignedInt,
    tint_env_play_frame: UnsignedInt,
    target_scale: Real,
    current_scale: Real,
}

impl EMPUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<EMPUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let now = TheGameLogic::get_frame();
        let die_frame = now.saturating_add(specific_data.life_frames);
        let tint_env_play_frame = now.saturating_add(specific_data.start_fade_frame);
        let tint_env_fade_frames = die_frame.saturating_sub(tint_env_play_frame);
        let target_scale = GameLogicRandomValueReal(
            specific_data.target_scale_min,
            specific_data.target_scale_max,
        );

        if let Ok(mut guard) = object.write() {
            let _ = guard.set_orientation(GameLogicRandomValueReal(-PI, PI));
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            die_frame,
            tint_env_fade_frames,
            tint_env_play_frame,
            target_scale,
            current_scale: specific_data.start_scale,
        })
    }

    fn do_disable_attack(&self, source: &Arc<RwLock<GameObject>>) {
        let data = &self.module_data;
        let source_guard = match source.read() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let radius = data.effect_radius;
        if radius <= 0.0 {
            return;
        }

        let source_id = source_guard.get_id();
        let source_pos = *source_guard.get_position();
        let source_player_id = source_guard.get_controlling_player_id();
        let producer_id = source_guard.get_producer_id();

        let mut intended_victim_id = None;
        let mut only_effect_airborne = false;

        if producer_id != OBJECT_INVALID_ID {
            if let Some(producer_arc) = OBJECT_REGISTRY.get_object(producer_id) {
                if let Ok(producer_guard) = producer_arc.read() {
                    if let Some(ai) = producer_guard.get_ai() {
                        if let Some(victim_id) = ai.get_current_victim() {
                            intended_victim_id = Some(victim_id);
                            if let Some(victim_arc) = OBJECT_REGISTRY.get_object(victim_id) {
                                if let Ok(victim_guard) = victim_arc.read() {
                                    if victim_guard.is_airborne_target() {
                                        only_effect_airborne = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut intended_victim_processed = false;
        let candidates = ThePartitionManager::get()
            .map(|partition| partition.get_objects_in_range_boundary_3d(&source_pos, radius))
            .unwrap_or_default();

        for id in candidates {
            if id == source_id {
                continue;
            }

            let Some(victim_arc) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(mut victim) = victim_arc.write() else {
                continue;
            };

            if only_effect_airborne && !victim.is_airborne_target() {
                continue;
            }

            if data.does_not_affect_my_own_buildings && victim.is_kind_of(KindOf::Structure) {
                if let (Some(source_player), Some(victim_player)) =
                    (source_player_id, victim.get_controlling_player_id())
                {
                    if source_player == victim_player {
                        continue;
                    }
                }
            }

            if !victim.is_kind_of(KindOf::Vehicle)
                && !victim.is_kind_of(KindOf::Structure)
                && !victim.is_kind_of(KindOf::SpawnsAreTheWeapons)
            {
                continue;
            }

            if victim.is_kind_of(KindOf::Aircraft) && victim.is_airborne_target() {
                if victim.is_kind_of(KindOf::EmpHardened) {
                    continue;
                }
                victim.kill(None, None);
                continue;
            }

            if data.reject_mask & WeaponAffectsMask::ALLIES as Int != 0 {
                let relationship = source_guard.relationship_to(&victim);
                if matches!(relationship, Relationship::Ally | Relationship::Allies) {
                    continue;
                }
            }

            let disable_frame = TheGameLogic::get_frame().saturating_add(data.disabled_duration);
            victim.set_disabled_until(crate::common::DisabledType::DisabledEmp, disable_frame);

            if intended_victim_id == Some(victim.get_id()) {
                intended_victim_processed = true;
            }

            if let Some(particle_name) = data.disable_fx_particle_system.as_deref() {
                if let Some(manager) = TheParticleSystemManager::get() {
                    let geometry = victim.get_geometry_info();
                    let victim_height = geometry.get_max_height_above_position();
                    let footprint_area = (geometry.bounds.max.x - geometry.bounds.min.x).abs()
                        * (geometry.bounds.max.y - geometry.bounds.min.y).abs();
                    let victim_volume = footprint_area * victim_height.min(10.0);
                    let emitter_count =
                        ((data.sparks_per_cubic_foot * victim_volume).ceil() as i32).max(15);

                    for _ in 0..emitter_count {
                        let Some(system_id) =
                            manager.create_particle_system(Some(particle_name.as_ref()))
                        else {
                            continue;
                        };

                        let mut offset = Coord3D::new(
                            GameLogicRandomValueReal(geometry.bounds.min.x, geometry.bounds.max.x),
                            GameLogicRandomValueReal(geometry.bounds.min.y, geometry.bounds.max.y),
                            0.0,
                        );
                        offset.z = GameLogicRandomValueReal(3.0, victim_height);

                        let length = offset.length();
                        if length > victim_height && length > 0.0 {
                            let restore_x = offset.x;
                            let restore_y = offset.y;
                            let normalized = offset / length;
                            offset.z = normalized.z * victim_height;
                            offset.x = restore_x;
                            offset.y = restore_y;
                        }

                        manager.attach_particle_system_to_object(system_id, victim.get_id());
                        manager.set_particle_system_position(system_id, &offset);
                    }
                }
            }
        }

        if let Some(victim_id) = intended_victim_id {
            if !intended_victim_processed {
                if let Some(victim_arc) = OBJECT_REGISTRY.get_object(victim_id) {
                    if let Ok(mut victim) = victim_arc.write() {
                        if victim.is_kind_of(KindOf::Aircraft)
                            && !victim.is_kind_of(KindOf::EmpHardened)
                        {
                            let offset = *victim.get_position() - source_pos;
                            let dist_sqr = offset.length_squared();
                            if dist_sqr <= radius * 2.0 || dist_sqr <= 40.0 * 40.0 {
                                let disable_frame = TheGameLogic::get_frame()
                                    .saturating_add(data.disabled_duration);
                                victim.set_disabled_until(
                                    crate::common::DisabledType::DisabledEmp,
                                    disable_frame,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

impl UpdateModuleInterface for EMPUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(obj_arc) = self.object.upgrade() else {
            return UpdateSleepTime::None;
        };

        let now = TheGameLogic::get_frame();

        if let Ok(obj) = obj_arc.read() {
            self.current_scale += (self.target_scale - self.current_scale) * 0.05;
            if let Some(drawable) = obj.get_drawable() {
                let scale = Matrix3D::from_scale(Coord3D::splat(self.current_scale));
                drawable.set_instance_matrix(Some(&scale));
            }
        }

        if now == self.tint_env_play_frame {
            self.do_disable_attack(&obj_arc);
        }

        if now >= self.die_frame {
            if let Ok(mut obj) = obj_arc.write() {
                obj.kill(None, None);
            }
        }

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for EMPUpdate {
    fn get_module_name(&self) -> &'static str {
        "EMPUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for EMPUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes EMPUpdate through the common Module trait.
pub struct EMPUpdateModule {
    behavior: EMPUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<EMPUpdateModuleData>,
}

impl EMPUpdateModule {
    pub fn new(
        behavior: EMPUpdate,
        module_name: &AsciiString,
        module_data: Arc<EMPUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut EMPUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for EMPUpdateModule {
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

impl Module for EMPUpdateModule {
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

pub struct EMPUpdateFactory;
impl EMPUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(EMPUpdate::new(thing, module_data)?))
    }
}
