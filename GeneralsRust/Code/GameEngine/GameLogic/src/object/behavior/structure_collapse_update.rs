//! StructureCollapseUpdate - Building collapse death animation
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, GameLogicRandomValue, ModelConditionFlags, ModuleData, ObjectID, Real,
    UnsignedInt, PLAYERMASK_ALL,
};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    game_client_random_value_real, TheFXListStore, TheGameLogic, TheObjectCreationListStore,
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
use crate::physics::GRAVITY;
use game_engine::common::global_data;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::system::Xfer;
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use glam::Vec4;
use std::sync::{Arc, RwLock, Weak};

const SC_PHASE_COUNT: usize = 4;
const MAX_IDX: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureCollapsePhaseType {
    Initial = 0,
    Delay = 1,
    Burst = 2,
    Final = 3,
}

impl StructureCollapsePhaseType {
    pub const COUNT: usize = SC_PHASE_COUNT;

    fn idx(self) -> usize {
        self as usize
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StructureCollapseStateType {
    Standing,
    WaitingForCollapseStart,
    Collapsing,
    Done,
}

#[derive(Clone, Debug)]
pub struct StructureCollapseUpdateModuleData {
    pub base: BehaviorModuleData,
    pub die_mux_data: DieMuxData,
    pub min_collapse_delay: UnsignedInt,
    pub max_collapse_delay: UnsignedInt,
    pub min_burst_delay: UnsignedInt,
    pub max_burst_delay: UnsignedInt,
    pub collapse_damping: Real,
    pub max_shudder: Real,
    pub big_burst_frequency: i32,
    pub ocls: [Vec<Option<Arc<ObjectCreationList>>>; StructureCollapsePhaseType::COUNT],
    pub fxs: [Vec<Option<Arc<FXList>>>; StructureCollapsePhaseType::COUNT],
    pub ocl_count: [UnsignedInt; StructureCollapsePhaseType::COUNT],
    pub fx_count: [UnsignedInt; StructureCollapsePhaseType::COUNT],
}

impl Default for StructureCollapseUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            die_mux_data: DieMuxData::default(),
            min_collapse_delay: 0,
            max_collapse_delay: 0,
            min_burst_delay: 9999,
            max_burst_delay: 0,
            collapse_damping: 0.0,
            max_shudder: 0.0,
            big_burst_frequency: 0,
            ocls: std::array::from_fn(|_| Vec::new()),
            fxs: std::array::from_fn(|_| Vec::new()),
            ocl_count: [1; StructureCollapsePhaseType::COUNT],
            fx_count: [1; StructureCollapsePhaseType::COUNT],
        }
    }
}

crate::impl_behavior_module_data_via_base!(StructureCollapseUpdateModuleData, base);

impl StructureCollapseUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STRUCTURE_COLLAPSE_UPDATE_FIELDS)
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

fn parse_min_collapse_delay(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_collapse_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_max_collapse_delay(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_collapse_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_min_burst_delay(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_burst_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_max_burst_delay(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_burst_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_collapse_damping(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.collapse_damping = INI::parse_real(value)?;
    Ok(())
}

fn parse_max_shudder(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.max_shudder = INI::parse_real(value)?;
    Ok(())
}

fn parse_big_burst_frequency(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.big_burst_frequency = value.parse().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_collapse_phase(token: &str) -> Option<StructureCollapsePhaseType> {
    match token.to_ascii_uppercase().as_str() {
        "INITIAL" => Some(StructureCollapsePhaseType::Initial),
        "DELAY" => Some(StructureCollapsePhaseType::Delay),
        "BURST" => Some(StructureCollapsePhaseType::Burst),
        "FINAL" => Some(StructureCollapsePhaseType::Final),
        _ => None,
    }
}

fn parse_phase_fx(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut values = value_tokens(tokens);
    let phase_token = values.next().ok_or(INIError::InvalidData)?;
    let Some(phase) = parse_collapse_phase(phase_token) else {
        return Err(INIError::InvalidData);
    };
    for name in values.map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let fx = TheFXListStore::find_fx_list(name);
        data.fxs[phase.idx()].push(fx);
    }
    Ok(())
}

fn parse_phase_ocl(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut values = value_tokens(tokens);
    let phase_token = values.next().ok_or(INIError::InvalidData)?;
    let Some(phase) = parse_collapse_phase(phase_token) else {
        return Err(INIError::InvalidData);
    };
    for name in values.map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let ocl = TheObjectCreationListStore::find_object_creation_list(name);
        data.ocls[phase.idx()].push(ocl);
    }
    Ok(())
}

fn parse_death_types(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.death_types = parse_death_type_flags_tokens(&values)?;
    Ok(())
}

fn parse_veterancy_levels(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(&values)?;
    Ok(())
}

fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.exempt_status = parse_object_status_mask_tokens(&values)?;
    Ok(())
}

fn parse_required_status(
    _ini: &mut INI,
    data: &mut StructureCollapseUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<_> = value_tokens(tokens).collect();
    data.die_mux_data.required_status = parse_object_status_mask_tokens(&values)?;
    Ok(())
}

const STRUCTURE_COLLAPSE_UPDATE_FIELDS: &[FieldParse<StructureCollapseUpdateModuleData>] = &[
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
        token: "MinCollapseDelay",
        parse: parse_min_collapse_delay,
    },
    FieldParse {
        token: "MaxCollapseDelay",
        parse: parse_max_collapse_delay,
    },
    FieldParse {
        token: "MinBurstDelay",
        parse: parse_min_burst_delay,
    },
    FieldParse {
        token: "MaxBurstDelay",
        parse: parse_max_burst_delay,
    },
    FieldParse {
        token: "CollapseDamping",
        parse: parse_collapse_damping,
    },
    FieldParse {
        token: "MaxShudder",
        parse: parse_max_shudder,
    },
    FieldParse {
        token: "BigBurstFrequency",
        parse: parse_big_burst_frequency,
    },
    FieldParse {
        token: "OCL",
        parse: parse_phase_ocl,
    },
    FieldParse {
        token: "FXList",
        parse: parse_phase_fx,
    },
];

pub struct StructureCollapseUpdate {
    object_id: ObjectID,
    module_data: Arc<StructureCollapseUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    collapse_frame: UnsignedInt,
    burst_frame: UnsignedInt,
    collapse_state: StructureCollapseStateType,
    collapse_velocity: Real,
    current_height: Real,
}

impl StructureCollapseUpdate {
    pub fn new_with_data(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<StructureCollapseUpdateModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(obj) = object.read() {
            TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
        }

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data,
            next_call_frame_and_phase: 0,
            collapse_frame: 0,
            burst_frame: 0,
            collapse_state: StructureCollapseStateType::Standing,
            collapse_velocity: 0.0,
            current_height: 0.0,
        })
    }

    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<StructureCollapseUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Self::new_with_data(object, Arc::new(specific_data.clone()))
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

    fn do_phase_stuff(&self, phase: StructureCollapsePhaseType, target: &Coord3D) {
        let phase_idx = phase.idx();
        let list_size = self.module_data.fxs[phase_idx].len();
        if list_size > 0 {
            let count = self.module_data.fx_count[phase_idx] as usize;
            debug_assert!(
                count <= list_size && count <= MAX_IDX,
                "StructureCollapseUpdate FX count exceeds list size or MAX_IDX"
            );
            for idx in Self::build_non_dup_indices(list_size, count) {
                if let Some(Some(fx)) = self.module_data.fxs[phase_idx].get(idx) {
                    let _ = fx.do_fx_at_position(target);
                }
            }
        }

        let list_size = self.module_data.ocls[phase_idx].len();
        if list_size > 0 {
            let count = self.module_data.ocl_count[phase_idx] as usize;
            debug_assert!(
                count <= list_size && count <= MAX_IDX,
                "StructureCollapseUpdate OCL count exceeds list size or MAX_IDX"
            );
            let ctx = crate::object_creation_list::live_creation_context();
            let Some(owner_arc) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) else {
                return;
            };
            let orientation = {
                let Ok(owner_guard) = owner_arc.read() else {
                    return;
                };
                owner_guard.get_orientation()
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
                        orientation,
                        0,
                    );
                }
            }
        }
    }

    fn do_collapse_done_stuff(&self) {
        let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return;
        };
        let Ok(obj) = object_arc.read() else {
            return;
        };
        if let Some(module) = obj.find_update_module("BoneFXUpdate") {
            module.with_module(|module| {
                if let Some(bone_fx) = module.get_bone_fx_control_interface() {
                    bone_fx.stop_all_bone_fx();
                }
            });
        } else if let Some(behavior) = obj.find_update_behavior("BoneFXUpdate") {
            if let Ok(mut behavior) = behavior.lock() {
                if let Some(bone_fx) = behavior.get_bone_fx_control_interface() {
                    bone_fx.stop_all_bone_fx();
                }
            }
        }
    }

    /// Begin the structure collapse animation
    /// C++ Reference: StructureCollapseUpdate.cpp - beginStructureCollapse()
    pub fn begin_collapse(&mut self, _damage_info: &crate::damage::DamageInfo) {
        let current_frame = TheGameLogic::get_frame();

        // Calculate random delay within range
        // C++ uses GameLogicRandomValue for this
        let random_delay = GameLogicRandomValue(
            self.module_data.min_collapse_delay as i32,
            self.module_data.max_collapse_delay as i32,
        ) as UnsignedInt;
        self.collapse_frame = current_frame.wrapping_add(random_delay);

        if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(obj) = object_arc.read() {
                TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::None);
                let pos = *obj.get_position();
                self.do_phase_stuff(StructureCollapsePhaseType::Initial, &pos);
            }
        }

        self.collapse_state = StructureCollapseStateType::WaitingForCollapseStart;
        self.current_height = 0.0;
    }
}

impl UpdateModuleInterface for StructureCollapseUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let now = TheGameLogic::get_frame();
        let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return UpdateSleepTime::Forever;
        };

        loop {
            match self.collapse_state {
                StructureCollapseStateType::Standing => return UpdateSleepTime::Forever,
                StructureCollapseStateType::WaitingForCollapseStart => {
                    if let Ok(obj) = object_arc.read() {
                        if let Some(drawable) = obj.get_drawable() {
                            let shudder = Coord3D::new(
                                game_client_random_value_real(
                                    -self.module_data.max_shudder,
                                    self.module_data.max_shudder,
                                ),
                                game_client_random_value_real(
                                    -self.module_data.max_shudder,
                                    self.module_data.max_shudder,
                                ),
                                0.0,
                            );
                            let mut inst = drawable.get_instance_matrix();
                            inst.w_axis = Vec4::new(shudder.x, shudder.y, shudder.z, inst.w_axis.w);
                            drawable.set_instance_matrix(Some(&inst));
                        }

                        if now >= self.collapse_frame {
                            self.collapse_state = StructureCollapseStateType::Collapsing;
                            let pos = *obj.get_position();
                            self.do_phase_stuff(StructureCollapsePhaseType::Burst, &pos);
                            let burst_delay = GameLogicRandomValue(
                                self.module_data.min_burst_delay as i32,
                                self.module_data.max_burst_delay as i32,
                            ) as UnsignedInt;
                            self.burst_frame = now.wrapping_add(burst_delay);
                            continue;
                        }
                    }
                    return UpdateSleepTime::None;
                }
                StructureCollapseStateType::Collapsing => {
                    if let Ok(mut obj) = object_arc.write() {
                        self.current_height -= self.collapse_velocity;
                        let gravity = global_data::read_safe()
                            .map(|data| data.gravity)
                            .unwrap_or(GRAVITY);
                        self.collapse_velocity -=
                            gravity * (1.0 - self.module_data.collapse_damping);

                        if let Some(drawable) = obj.get_drawable() {
                            let shudder = Coord3D::new(
                                game_client_random_value_real(
                                    -self.module_data.max_shudder,
                                    self.module_data.max_shudder,
                                ),
                                game_client_random_value_real(
                                    -self.module_data.max_shudder,
                                    self.module_data.max_shudder,
                                ),
                                self.current_height,
                            );
                            let mut inst = drawable.get_instance_matrix();
                            inst.w_axis = Vec4::new(shudder.x, shudder.y, shudder.z, inst.w_axis.w);
                            drawable.set_instance_matrix(Some(&inst));
                        }

                        if now >= self.burst_frame {
                            let pos = *obj.get_position();
                            if GameLogicRandomValue(1, self.module_data.big_burst_frequency) == 1 {
                                self.do_phase_stuff(StructureCollapsePhaseType::Burst, &pos);
                            } else {
                                self.do_phase_stuff(StructureCollapsePhaseType::Delay, &pos);
                            }
                            let burst_delay = GameLogicRandomValue(
                                self.module_data.min_burst_delay as i32,
                                self.module_data.max_burst_delay as i32,
                            ) as UnsignedInt;
                            self.burst_frame = self.burst_frame.wrapping_add(burst_delay);
                        }

                        let template_geo = obj.get_template().get_template_geometry_info();
                        if self.current_height + template_geo.get_max_height_above_position() <= 0.0
                        {
                            self.collapse_state = StructureCollapseStateType::Done;
                            let pos = *obj.get_position();
                            self.do_phase_stuff(StructureCollapsePhaseType::Final, &pos);
                            self.do_collapse_done_stuff();

                            if let Some(drawable) = obj.get_drawable() {
                                drawable.clear_model_condition_state(ModelConditionFlags::RUBBLE);
                                drawable
                                    .set_model_condition_state(ModelConditionFlags::POST_COLLAPSE);
                            }

                            let orientation = obj.get_orientation();
                            let _ = obj.set_orientation(orientation);

                            if let Some(body) = obj.get_body_module() {
                                if let Ok(mut body_guard) = body.lock() {
                                    let _ = body_guard.update_body_particle_systems();
                                }
                            }

                            if let Some(drawable) = obj.get_drawable() {
                                let mut inst = drawable.get_instance_matrix();
                                inst.w_axis = Vec4::new(0.0, 0.0, 0.0, inst.w_axis.w);
                                drawable.set_instance_matrix(Some(&inst));
                            }
                            return UpdateSleepTime::Forever;
                        }
                    }
                    return UpdateSleepTime::None;
                }
                StructureCollapseStateType::Done => return UpdateSleepTime::Forever,
            }
        }
    }
}

impl DieModuleInterface for StructureCollapseUpdate {
    fn on_die(
        &mut self,
        damage_info: &crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
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
        self.begin_collapse(damage_info);

        Ok(())
    }
}

impl BehaviorModuleInterface for StructureCollapseUpdate {
    fn get_module_name(&self) -> &'static str {
        "StructureCollapseUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for StructureCollapseUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_unsigned_int(&mut self.collapse_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.burst_frame)
            .map_err(|e| e.to_string())?;

        let mut collapse_state = self.collapse_state as i32;
        xfer.xfer_i32(&mut collapse_state)
            .map_err(|e| e.to_string())?;
        self.collapse_state = match collapse_state {
            1 => StructureCollapseStateType::WaitingForCollapseStart,
            2 => StructureCollapseStateType::Collapsing,
            3 => StructureCollapseStateType::Done,
            _ => StructureCollapseStateType::Standing,
        };

        xfer.xfer_real(&mut self.collapse_velocity)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.current_height)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes StructureCollapseUpdate through the common Module trait.
pub struct StructureCollapseUpdateModule {
    behavior: StructureCollapseUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<StructureCollapseUpdateModuleData>,
}

impl StructureCollapseUpdateModule {
    pub fn new(
        behavior: StructureCollapseUpdate,
        module_name: &AsciiString,
        module_data: Arc<StructureCollapseUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut StructureCollapseUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for StructureCollapseUpdateModule {
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

impl Module for StructureCollapseUpdateModule {
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

pub struct StructureCollapseUpdateFactory;
impl StructureCollapseUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(StructureCollapseUpdate::new(thing, module_data)?))
    }
}
