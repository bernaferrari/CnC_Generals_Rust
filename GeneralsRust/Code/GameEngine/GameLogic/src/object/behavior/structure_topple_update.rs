//! StructureToppleUpdate - Building topple and crushing behavior
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord2D, Coord3D, GameLogicRandomValue, GameLogicRandomValueReal, Matrix3D,
    ModelConditionFlags, ModuleData, Real, UnsignedInt, PLAYERMASK_ALL,
};
use crate::damage::{get_damage_type_flag, DamageInfo, DamageType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    game_client_random_value, TheFXListStore, TheGameLogic, TheObjectCreationListStore,
    TheTerrainLogic, TheWeaponStore,
};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::die::{
    parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens, DieMuxData, ObjectStatusMask,
};
use crate::object::DrawableArcExt;
use crate::object::Object as GameObject;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::scripting::engine::get_script_engine;
use crate::weapon::with_weapon_store;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::str::FromStr;
use std::sync::{Arc, RwLock, Weak};

const ST_PHASE_COUNT: usize = 3;
const MAX_IDX: usize = 32;
const TOPPLE_ACCELERATION_FACTOR: Real = 0.02;
const THETA_CEILING: Real = std::f32::consts::PI / 6.0;
const WEAPON_SPACING_PERPENDICULAR: Real = 25.0;
const WEAPON_SPACING_PARALLEL: Real = 25.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureTopplePhaseType {
    Initial = 0,
    Delay = 1,
    Final = 2,
}

impl StructureTopplePhaseType {
    pub const COUNT: usize = ST_PHASE_COUNT;

    fn idx(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureToppleStateType {
    Standing,
    WaitingForToppleStart,
    Toppling,
    WaitingForDone,
    Done,
}

#[derive(Clone, Debug)]
pub struct FXBoneInfo {
    pub bone_name: AsciiString,
    pub particle_system_template: Option<AsciiString>,
}

#[derive(Clone, Debug)]
pub struct AngleFXInfo {
    pub angle: Real,
    pub fx_list: Option<Arc<FXList>>,
}

#[derive(Clone, Debug)]
pub struct StructureToppleUpdateModuleData {
    pub base: BehaviorModuleData,
    pub die_mux_data: DieMuxData,
    pub min_topple_delay: UnsignedInt,
    pub max_topple_delay: UnsignedInt,
    pub structural_integrity: Real,
    pub structural_decay: Real,
    pub damage_fx_types: crate::damage::DamageTypeFlags,
    pub topple_start_fx_list: Option<Arc<FXList>>,
    pub topple_delay_fx_list: Option<Arc<FXList>>,
    pub topple_fx_list: Option<Arc<FXList>>,
    pub topple_done_fx_list: Option<Arc<FXList>>,
    pub crushing_fx_list: Option<Arc<FXList>>,
    pub crushing_weapon_name: AsciiString,
    pub min_topple_burst_delay: UnsignedInt,
    pub max_topple_burst_delay: UnsignedInt,
    pub ocls: [Vec<Option<Arc<ObjectCreationList>>>; StructureTopplePhaseType::COUNT],
    pub ocl_count: [UnsignedInt; StructureTopplePhaseType::COUNT],
    pub fx_bones: Vec<FXBoneInfo>,
    pub angle_fx: Vec<AngleFXInfo>,
}

impl Default for StructureToppleUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            die_mux_data: DieMuxData::default(),
            min_topple_delay: 0,
            max_topple_delay: 0,
            structural_integrity: 0.1,
            structural_decay: 0.0,
            damage_fx_types: crate::damage::DamageTypeFlags::all_flags(),
            topple_start_fx_list: None,
            topple_delay_fx_list: None,
            topple_fx_list: None,
            topple_done_fx_list: None,
            crushing_fx_list: None,
            crushing_weapon_name: AsciiString::new(),
            min_topple_burst_delay: 0,
            max_topple_burst_delay: 0,
            ocls: std::array::from_fn(|_| Vec::new()),
            ocl_count: [1; StructureTopplePhaseType::COUNT],
            fx_bones: Vec::new(),
            angle_fx: Vec::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(StructureToppleUpdateModuleData, base);

impl StructureToppleUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STRUCTURE_TOPPLE_UPDATE_FIELDS)
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> impl Iterator<Item = &'a str> + 'a {
    tokens.iter().copied().filter(|token| *token != "=")
}

fn parse_min_topple_delay(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_topple_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_max_topple_delay(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_topple_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_min_topple_burst_delay(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_topple_burst_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_max_topple_burst_delay(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_topple_burst_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_structural_integrity(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.structural_integrity = INI::parse_real(value)?;
    Ok(())
}

fn parse_structural_decay(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.structural_decay = INI::parse_real(value)?;
    Ok(())
}

fn parse_damage_fx_types(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if first_value_token(tokens).is_none() {
        return Err(INIError::InvalidData);
    }

    let mut flags = crate::damage::DamageTypeFlags::empty();
    for token in value_tokens(tokens) {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = crate::damage::DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = crate::damage::DamageTypeFlags::empty();
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Ok(damage_type) = DamageType::from_str(name) {
                let flag =
                    crate::damage::DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }

    data.damage_fx_types = flags;
    Ok(())
}

fn parse_fx_list(data_field: &mut Option<Arc<FXList>>, tokens: &[&str]) -> Result<(), INIError> {
    let token = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        *data_field = None;
        return Ok(());
    }
    *data_field = TheFXListStore::find_fx_list(token);
    Ok(())
}

fn parse_topple_fx(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.topple_fx_list, tokens)
}

fn parse_topple_delay_fx(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.topple_delay_fx_list, tokens)
}

fn parse_topple_start_fx(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.topple_start_fx_list, tokens)
}

fn parse_topple_done_fx(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.topple_done_fx_list, tokens)
}

fn parse_crushing_fx(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.crushing_fx_list, tokens)
}

fn parse_crushing_weapon_name(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.crushing_weapon_name = AsciiString::from(token);
    Ok(())
}

fn parse_topple_phase(token: &str) -> Option<StructureTopplePhaseType> {
    match token.to_ascii_uppercase().as_str() {
        "INITIAL" => Some(StructureTopplePhaseType::Initial),
        "DELAY" => Some(StructureTopplePhaseType::Delay),
        "FINAL" => Some(StructureTopplePhaseType::Final),
        _ => None,
    }
}

fn parse_phase_ocl(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut values = value_tokens(tokens);
    let phase_token = values.next().ok_or(INIError::InvalidData)?;
    let Some(phase) = parse_topple_phase(phase_token) else {
        return Err(INIError::InvalidData);
    };
    for token in values.map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let ocl = TheObjectCreationListStore::find_object_creation_list(token);
        data.ocls[phase.idx()].push(ocl);
    }
    Ok(())
}

fn parse_angle_fx(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut values = value_tokens(tokens);
    let angle_token = values.next().ok_or(INIError::InvalidData)?;
    let fx_token = values.next().ok_or(INIError::InvalidData)?;
    let angle_degrees = INI::parse_real(angle_token)?;
    let angle_radians = angle_degrees * std::f32::consts::PI / 180.0;
    let fx_list = if fx_token.eq_ignore_ascii_case("NONE") {
        None
    } else {
        TheFXListStore::find_fx_list(fx_token)
    };
    data.angle_fx.push(AngleFXInfo {
        angle: angle_radians,
        fx_list,
    });
    Ok(())
}

fn parse_death_types(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.death_types = parse_death_type_flags_tokens(&values)?;
    Ok(())
}

fn parse_veterancy_levels(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(&values)?;
    Ok(())
}

fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.exempt_status = parse_object_status_mask_tokens(&values)?;
    Ok(())
}

fn parse_required_status(
    _ini: &mut INI,
    data: &mut StructureToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.required_status = parse_object_status_mask_tokens(&values)?;
    Ok(())
}

const STRUCTURE_TOPPLE_UPDATE_FIELDS: &[FieldParse<StructureToppleUpdateModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_required_status,
    },
    FieldParse {
        token: "MinToppleDelay",
        parse: parse_min_topple_delay,
    },
    FieldParse {
        token: "MaxToppleDelay",
        parse: parse_max_topple_delay,
    },
    FieldParse {
        token: "MinToppleBurstDelay",
        parse: parse_min_topple_burst_delay,
    },
    FieldParse {
        token: "MaxToppleBurstDelay",
        parse: parse_max_topple_burst_delay,
    },
    FieldParse {
        token: "StructuralIntegrity",
        parse: parse_structural_integrity,
    },
    FieldParse {
        token: "StructuralDecay",
        parse: parse_structural_decay,
    },
    FieldParse {
        token: "DamageFXTypes",
        parse: parse_damage_fx_types,
    },
    FieldParse {
        token: "TopplingFX",
        parse: parse_topple_fx,
    },
    FieldParse {
        token: "ToppleDelayFX",
        parse: parse_topple_delay_fx,
    },
    FieldParse {
        token: "ToppleStartFX",
        parse: parse_topple_start_fx,
    },
    FieldParse {
        token: "ToppleDoneFX",
        parse: parse_topple_done_fx,
    },
    FieldParse {
        token: "CrushingFX",
        parse: parse_crushing_fx,
    },
    FieldParse {
        token: "CrushingWeaponName",
        parse: parse_crushing_weapon_name,
    },
    FieldParse {
        token: "OCL",
        parse: parse_phase_ocl,
    },
    FieldParse {
        token: "AngleFX",
        parse: parse_angle_fx,
    },
];

pub struct StructureToppleUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<StructureToppleUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    topple_frame: UnsignedInt,
    topple_direction: Coord2D,
    topple_state: StructureToppleStateType,
    topple_velocity: Real,
    accumulated_angle: Real,
    structural_integrity: Real,
    last_crushed_location: Real,
    next_burst_frame: i32,
    delay_burst_location: Coord3D,
    building_height: Real,
}

impl StructureToppleUpdate {
    pub fn new_with_data(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<StructureToppleUpdateModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(building) = object.read() {
            TheGameLogic::set_wake_frame(building.get_id(), UpdateSleepTime::Forever);
        }

        let building_height = object
            .read()
            .map(|obj| obj.get_geometry_info().get_max_height_above_position())
            .unwrap_or(0.0);

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            topple_frame: 0,
            topple_direction: Coord2D::ZERO,
            topple_state: StructureToppleStateType::Standing,
            topple_velocity: 0.0,
            accumulated_angle: 0.001,
            structural_integrity: 0.0,
            last_crushed_location: 0.0,
            next_burst_frame: -1,
            delay_burst_location: Coord3D::ZERO,
            building_height,
        })
    }

    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<StructureToppleUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Self::new_with_data(object, Arc::new(data.clone()))
    }

    fn build_non_dup_indices(range: usize, count: usize) -> Vec<usize> {
        let mut indices = Vec::with_capacity(count);
        while indices.len() < count {
            let idx = GameLogicRandomValue(0, range as i32 - 1) as usize;
            if !indices.contains(&idx) {
                indices.push(idx);
            }
        }
        indices
    }

    fn do_phase_stuff(&self, phase: StructureTopplePhaseType, target: &Coord3D) {
        let phase_idx = phase.idx();
        let list_size = self.module_data.ocls[phase_idx].len();
        if list_size == 0 {
            return;
        }
        let count = self.module_data.ocl_count[phase_idx] as usize;
        debug_assert!(
            count <= list_size && count <= MAX_IDX,
            "StructureToppleUpdate OCL count exceeds list size or MAX_IDX"
        );
        let ctx = crate::object_creation_list::live_creation_context();
        let Some(owner_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return;
        };
        let primary = *target;
        let secondary = *target;
        for idx in Self::build_non_dup_indices(list_size, count) {
            if let Some(Some(ocl)) = self.module_data.ocls[phase_idx].get(idx) {
                let _ = ocl.create_with_angle(
                    &ctx,
                    Some(&*owner_guard),
                    &primary,
                    &secondary,
                    INVALID_ANGLE,
                    0,
                );
            }
        }
    }

    fn should_play_damage_fx(
        last_damage_type: Option<DamageType>,
        mask: crate::damage::DamageTypeFlags,
    ) -> bool {
        last_damage_type
            .map(|dmg| get_damage_type_flag(mask, dmg))
            .unwrap_or(true)
    }

    fn begin_structure_topple(&mut self, damage_info: &DamageInfo) {
        let now = TheGameLogic::get_frame();
        let delay = GameLogicRandomValue(
            self.module_data.min_topple_delay as i32,
            self.module_data.max_topple_delay as i32,
        ) as UnsignedInt;
        self.topple_frame = now.wrapping_add(delay);

        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(building) = object_arc.read() else {
            return;
        };

        let attacker = TheGameLogic::find_object_by_id(damage_info.input.source_id);
        let topple_angle = if let Some(attacker_arc) = attacker {
            if let Ok(attacker_guard) = attacker_arc.read() {
                let attacker_pos = attacker_guard.get_position();
                let building_pos = building.get_position();
                let dir = Coord2D::new(
                    building_pos.x - attacker_pos.x,
                    building_pos.y - attacker_pos.y,
                );
                let base_angle = dir.y.atan2(dir.x);
                base_angle
                    + GameLogicRandomValueReal(
                        -std::f32::consts::PI / 8.0,
                        std::f32::consts::PI / 8.0,
                    )
            } else {
                GameLogicRandomValueReal(0.0, 2.0 * std::f32::consts::PI)
            }
        } else {
            GameLogicRandomValueReal(0.0, 2.0 * std::f32::consts::PI)
        };

        self.topple_direction = Coord2D::new(topple_angle.cos(), topple_angle.sin());
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                let mut direction =
                    Coord3D::new(self.topple_direction.x, self.topple_direction.y, 0.0);
                engine.adjust_topple_direction(&building, &mut direction);
                self.topple_direction = Coord2D::new(direction.x, direction.y);
            }
        }

        let geo = building.get_geometry_info();
        let average_radius = (geo.get_major_radius() + geo.get_minor_radius()) * 0.5;
        let explosion_radius = average_radius * 0.90;

        self.delay_burst_location.x =
            building.get_position().x + explosion_radius * topple_angle.cos();
        self.delay_burst_location.y =
            building.get_position().y + explosion_radius * topple_angle.sin();
        self.delay_burst_location.z = TheTerrainLogic::get()
            .map(|terrain| {
                terrain.get_ground_height(
                    self.delay_burst_location.x,
                    self.delay_burst_location.y,
                    None,
                )
            })
            .unwrap_or(0.0);

        self.do_topple_start_fx(&building);
        let burst_delay = game_client_random_value(
            self.module_data.min_topple_burst_delay as i32,
            self.module_data.max_topple_burst_delay as i32,
        );
        self.next_burst_frame = (now as i32).wrapping_add(burst_delay);
        self.topple_state = StructureToppleStateType::WaitingForToppleStart;
        TheGameLogic::set_wake_frame(building.get_id(), UpdateSleepTime::None);
    }

    fn do_topple_start_fx(&self, building: &GameObject) {
        let last_damage_type = building.get_body_module().and_then(|body| {
            body.lock()
                .ok()
                .and_then(|b| b.get_last_damage_info().map(|info| info.damage_type))
        });

        if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
            if let Some(fx) = &self.module_data.topple_start_fx_list {
                let _ = fx.do_fx_at_position(building.get_position());
            }
        }
        self.do_phase_stuff(StructureTopplePhaseType::Initial, building.get_position());
    }

    fn do_topple_delay_burst_fx(&mut self) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(building) = object_arc.read() else {
            return;
        };
        let last_damage_type = building.get_body_module().and_then(|body| {
            body.lock()
                .ok()
                .and_then(|b| b.get_last_damage_info().map(|info| info.damage_type))
        });

        if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
            if let Some(fx) = &self.module_data.topple_delay_fx_list {
                let _ = fx.do_fx_at_position(&self.delay_burst_location);
            }
        }

        if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
            if let Some(drawable) = building.get_drawable() {
                if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
                    for bone in &self.module_data.fx_bones {
                        let Some(system_id) = ps_manager.create_particle_system(
                            bone.particle_system_template.as_ref().map(|s| s.as_str()),
                        ) else {
                            continue;
                        };

                        let mut bone_positions = [Coord3D::ZERO; 1];
                        let mut bone_transforms = [Matrix3D::IDENTITY; 1];
                        let mut found = false;

                        for module_handle in drawable.get_draw_modules() {
                            if let Some(count) =
                                module_handle.with_object_draw_interface(|draw_module| {
                                    draw_module.get_pristine_bone_positions(
                                        &ModelConditionFlags::PRISTINE,
                                        bone.bone_name.as_str(),
                                        0,
                                        &mut bone_positions,
                                        &mut bone_transforms,
                                        1,
                                    )
                                })
                            {
                                if count == 1 {
                                    found = true;
                                    break;
                                }
                            }
                        }

                        if found {
                            ps_manager.set_particle_system_position(system_id, &bone_positions[0]);
                            ps_manager
                                .attach_particle_system_to_drawable(system_id, drawable.get_id());
                        }
                    }
                }
            }
        }

        self.do_phase_stuff(StructureTopplePhaseType::Delay, &self.delay_burst_location);
    }

    fn do_topple_done_stuff(&self) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(building) = object_arc.read() else {
            return;
        };

        if let Some(module) = building.find_update_module("BoneFXUpdate") {
            module.with_module(|module| {
                if let Some(bone_fx) = module.get_bone_fx_control_interface() {
                    bone_fx.stop_all_bone_fx();
                }
            });
        } else if let Some(behavior) = building.find_update_behavior("BoneFXUpdate") {
            if let Ok(mut behavior) = behavior.lock() {
                if let Some(bone_fx) = behavior.get_bone_fx_control_interface() {
                    bone_fx.stop_all_bone_fx();
                }
            }
        }

        let orig_angle = building.get_orientation();
        let topple_angle = self.topple_direction.y.atan2(self.topple_direction.x);
        drop(building);

        {
            let Ok(mut building) = object_arc.write() else {
                return;
            };
            let _ = building.set_orientation(orig_angle);
            let mut xfrm = building.get_transform_matrix();
            let rot = Matrix3D::from_rotation_z(topple_angle - orig_angle);
            xfrm = rot * xfrm;
            building.set_transform_matrix(&xfrm);
        }
    }

    fn do_angle_fx(&self, cur_angle: Real, new_angle: Real) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(building) = object_arc.read() else {
            return;
        };
        let last_damage_type = building.get_body_module().and_then(|body| {
            body.lock()
                .ok()
                .and_then(|b| b.get_last_damage_info().map(|info| info.damage_type))
        });

        for info in &self.module_data.angle_fx {
            if info.angle > cur_angle && info.angle <= new_angle {
                if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
                    if let Some(fx) = &info.fx_list {
                        let _ = fx.do_fx_obj(&object_arc, None);
                    }
                }
            }
        }
    }

    fn apply_crushing_damage(&mut self, theta: Real) {
        if theta > THETA_CEILING {
            return;
        }

        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(building) = object_arc.read() else {
            return;
        };

        let orientation_angle = building.get_orientation();
        let topple_angle = self.topple_direction.y.atan2(self.topple_direction.x);
        let angle = orientation_angle - topple_angle;

        let geo = building.get_geometry_info();
        let major_radius = geo.get_major_radius();
        let minor_radius = geo.get_minor_radius();

        let minor_component = minor_radius * angle.cos();
        let major_component = major_radius * angle.sin();
        let facing_width = (Coord3D::new(major_component, minor_component, 0.0)).length() * 0.5;

        let weapon_template = with_weapon_store(|store| {
            store
                .find_weapon_template(self.module_data.crushing_weapon_name.as_str())
                .cloned()
        })
        .ok()
        .flatten();

        let Some(wt) = weapon_template else {
            return;
        };

        let max_distance = self.building_height * (1.0 - theta.sin());

        let mut j = self.last_crushed_location;
        while j < max_distance {
            let jcos = j * topple_angle.cos();
            let jsin = j * topple_angle.sin();
            self.do_damage_line(&building, &wt, jcos, jsin, facing_width, topple_angle);
            j += WEAPON_SPACING_PERPENDICULAR;
        }

        let jcos = max_distance * topple_angle.cos();
        let jsin = max_distance * topple_angle.sin();
        self.do_damage_line(&building, &wt, jcos, jsin, facing_width, topple_angle);
        self.last_crushed_location = j;
    }

    fn do_damage_line(
        &self,
        building: &GameObject,
        weapon: &Arc<crate::weapon::WeaponTemplate>,
        jcos: Real,
        jsin: Real,
        facing_width: Real,
        topple_angle: Real,
    ) {
        let last_damage_type = building.get_body_module().and_then(|body| {
            body.lock()
                .ok()
                .and_then(|b| b.get_last_damage_info().map(|info| info.damage_type))
        });

        let Some(terrain) = TheTerrainLogic::get() else {
            return;
        };

        let source_id = building.get_id();

        let mut i = -facing_width;
        while i < facing_width {
            let mut target = Coord3D::new(
                building.get_position().x + jcos + (i * topple_angle.sin()),
                building.get_position().y + jsin + (i * topple_angle.cos()),
                0.0,
            );
            target.z = terrain.get_ground_height(target.x, target.y, None);

            if let Some(store) = TheWeaponStore::get() {
                let _ = store.create_and_fire_temp_weapon_at_pos(weapon, source_id, &target);
            }

            if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
                if let Some(fx) = &self.module_data.crushing_fx_list {
                    let _ = fx.do_fx_at_position(&target);
                }
            }
            i += WEAPON_SPACING_PARALLEL;
        }

        let mut edge_target = Coord3D::new(
            building.get_position().x + jcos + (facing_width * topple_angle.sin()),
            building.get_position().y + jsin + (facing_width * topple_angle.cos()),
            0.0,
        );
        edge_target.z = terrain.get_ground_height(edge_target.x, edge_target.y, None);

        if let Some(store) = TheWeaponStore::get() {
            let _ = store.create_and_fire_temp_weapon_at_pos(weapon, source_id, &edge_target);
        }

        if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
            if let Some(fx) = &self.module_data.crushing_fx_list {
                let _ = fx.do_fx_at_position(&edge_target);
            }
        }

        let mut debris_target = Coord3D::new(
            building.get_position().x + jcos,
            building.get_position().y + jsin,
            0.0,
        );
        debris_target.z = terrain.get_ground_height(debris_target.x, debris_target.y, None);
        self.do_phase_stuff(StructureTopplePhaseType::Final, &debris_target);
    }
}

impl UpdateModuleInterface for StructureToppleUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };

        if matches!(self.topple_state, StructureToppleStateType::Standing) {
            return UpdateSleepTime::Forever;
        }

        let last_damage_type = object_arc.read().ok().and_then(|building| {
            building.get_body_module().and_then(|body| {
                body.lock()
                    .ok()
                    .and_then(|b| b.get_last_damage_info().map(|info| info.damage_type))
            })
        });

        let now = TheGameLogic::get_frame();
        if matches!(
            self.topple_state,
            StructureToppleStateType::WaitingForToppleStart
        ) {
            if now as i32 >= self.next_burst_frame {
                self.do_topple_delay_burst_fx();
                let burst_delay = game_client_random_value(
                    self.module_data.min_topple_burst_delay as i32,
                    self.module_data.max_topple_burst_delay as i32,
                );
                self.next_burst_frame = (now as i32).wrapping_add(burst_delay);
            }
            if now >= self.topple_frame {
                self.topple_state = StructureToppleStateType::Toppling;
                self.structural_integrity = self.module_data.structural_integrity;
            }
        }

        if matches!(self.topple_state, StructureToppleStateType::Toppling) {
            let topple_accel = TOPPLE_ACCELERATION_FACTOR
                * (self.accumulated_angle.sin() * (1.0 - self.structural_integrity));
            self.topple_velocity += topple_accel;

            if self.structural_integrity > 0.0 {
                self.structural_integrity *= self.module_data.structural_decay;
                if self.structural_integrity < 0.0 {
                    self.structural_integrity = 0.0;
                }
            }

            self.do_angle_fx(
                self.accumulated_angle,
                self.accumulated_angle + self.topple_velocity,
            );
            self.accumulated_angle += self.topple_velocity;
            self.apply_crushing_damage(std::f32::consts::PI / 2.0 - self.accumulated_angle);

            if self.accumulated_angle >= std::f32::consts::PI / 2.0 {
                self.topple_velocity -= self.accumulated_angle - std::f32::consts::PI / 2.0;
                self.accumulated_angle = std::f32::consts::PI / 2.0;
                self.topple_state = StructureToppleStateType::WaitingForDone;

                self.apply_crushing_damage(0.0);
                let pos = object_arc
                    .read()
                    .ok()
                    .map(|b| *b.get_position())
                    .unwrap_or(Coord3D::ZERO);
                self.do_phase_stuff(StructureTopplePhaseType::Final, &pos);

                if Self::should_play_damage_fx(last_damage_type, self.module_data.damage_fx_types) {
                    if let Some(fx) = &self.module_data.topple_done_fx_list {
                        let _ = fx.do_fx_obj(&object_arc, None);
                    }
                }
                self.topple_frame = now;
            }

            if now as i32 >= self.next_burst_frame {
                self.do_topple_delay_burst_fx();
                let burst_delay = game_client_random_value(
                    self.module_data.min_topple_burst_delay as i32,
                    self.module_data.max_topple_burst_delay as i32,
                );
                self.next_burst_frame = (now as i32).wrapping_add(burst_delay);
            }

            let mut xfrm = object_arc
                .read()
                .ok()
                .map(|b| b.get_transform_matrix())
                .unwrap_or(Matrix3D::IDENTITY);
            let rot_x = Matrix3D::from_rotation_x(-self.topple_velocity * self.topple_direction.y);
            let rot_y = Matrix3D::from_rotation_y(self.topple_velocity * self.topple_direction.x);
            xfrm = rot_y * rot_x * xfrm;
            if let Ok(mut building) = object_arc.write() {
                building.set_transform_matrix(&xfrm);
            }
        }

        if matches!(self.topple_state, StructureToppleStateType::WaitingForDone) {
            if self.topple_frame <= TheGameLogic::get_frame() {
                if let Ok(building) = object_arc.read() {
                    if let Some(drawable) = building.get_drawable() {
                        drawable.clear_model_condition_state(ModelConditionFlags::RUBBLE);
                        drawable.set_model_condition_state(ModelConditionFlags::POST_COLLAPSE);
                    }

                    if let Some(body) = building.get_body_module() {
                        if let Ok(mut body_guard) = body.lock() {
                            let _ = body_guard.update_body_particle_systems();
                        }
                    }
                }

                self.do_topple_done_stuff();
                self.topple_state = StructureToppleStateType::Done;
                return UpdateSleepTime::Forever;
            }
        }

        UpdateSleepTime::None
    }
}

impl DieModuleInterface for StructureToppleUpdate {
    fn on_die(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(object_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let Ok(obj_read) = object_arc.read() else {
            return Ok(());
        };

        if !self
            .module_data
            .die_mux_data
            .is_die_applicable(&*obj_read, damage_info)
        {
            return Ok(());
        }

        if let Some(ai) = obj_read.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.mark_as_dead();
            }
        }

        TheGameLogic::deselect_object(&*obj_read, PLAYERMASK_ALL, true)?;

        drop(obj_read);
        self.begin_structure_topple(damage_info);
        Ok(())
    }
}

impl BehaviorModuleInterface for StructureToppleUpdate {
    fn get_module_name(&self) -> &'static str {
        "StructureToppleUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for StructureToppleUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.topple_frame)
            .map_err(|e| e.to_string())?;

        let mut topple_direction = self.topple_direction;
        xfer.xfer_coord2d(&mut topple_direction);
        self.topple_direction = topple_direction;

        let mut topple_state = self.topple_state as i32;
        xfer.xfer_i32(&mut topple_state)
            .map_err(|e| e.to_string())?;
        self.topple_state = match topple_state {
            1 => StructureToppleStateType::WaitingForToppleStart,
            2 => StructureToppleStateType::Toppling,
            3 => StructureToppleStateType::WaitingForDone,
            4 => StructureToppleStateType::Done,
            _ => StructureToppleStateType::Standing,
        };

        xfer.xfer_real(&mut self.topple_velocity)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.accumulated_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.structural_integrity)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.last_crushed_location)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.next_burst_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_coord3d(&mut self.delay_burst_location);
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes StructureToppleUpdate through the common Module trait.
pub struct StructureToppleUpdateModule {
    behavior: StructureToppleUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<StructureToppleUpdateModuleData>,
}

impl StructureToppleUpdateModule {
    pub fn new(
        behavior: StructureToppleUpdate,
        module_name: &AsciiString,
        module_data: Arc<StructureToppleUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut StructureToppleUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for StructureToppleUpdateModule {
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

impl Module for StructureToppleUpdateModule {
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

pub struct StructureToppleUpdateFactory;
impl StructureToppleUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(StructureToppleUpdate::new(thing, module_data)?))
    }
}
