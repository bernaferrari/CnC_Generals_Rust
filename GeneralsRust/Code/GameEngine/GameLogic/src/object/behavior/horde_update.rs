//! HordeUpdate - Rust conversion of C++ HordeUpdate
//!
//! Horde mechanics for GLA units.
//! Author: Steven Johnson, Feb 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Bool, Int, KindOf, KindOfMaskType, ModuleData, Real, UnsignedInt,
    WeaponBonusConditionFlags, KIND_OF_MASK_NONE,
};
use crate::common::{GameLogicRandomValue, FROM_CENTER_2D, LOGICFRAMES_PER_SECOND};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::draw::draw_module::TerrainDecalType;
use crate::object::drawable::DrawableArcExt;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HordeActionType {
    Horde = 0,
}

#[derive(Clone, Debug)]
pub struct HordeUpdateModuleData {
    pub base: BehaviorModuleData,
    pub update_rate: UnsignedInt,
    pub kindof: KindOfMaskType,
    pub min_count: Int,
    pub min_dist: Real,
    pub allies_only: Bool,
    pub exact_match: Bool,
    pub rub_off_radius: Real,
    pub action: HordeActionType,
    pub allowed_nationalism: Bool,
    pub flag_sub_obj_names: Vec<String>,
}

impl Default for HordeUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            update_rate: LOGICFRAMES_PER_SECOND as UnsignedInt,
            kindof: 0,
            min_count: 0,
            min_dist: 0.0,
            allies_only: true,
            exact_match: false,
            rub_off_radius: 20.0,
            action: HordeActionType::Horde,
            allowed_nationalism: true,
            flag_sub_obj_names: Vec::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(HordeUpdateModuleData, base);

impl HordeUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HORDE_UPDATE_FIELDS)
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_int(tokens: &[&str]) -> Result<Int, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    token.parse::<Int>().map_err(|_| INIError::InvalidData)
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_bool(tokens: &[&str]) -> Result<Bool, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_bool(token)
}

fn parse_update_rate(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.update_rate = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_kindof(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.kindof = crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_min_count(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_count = parse_int(tokens)?;
    Ok(())
}

fn parse_min_dist(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_dist = parse_real(tokens)?;
    Ok(())
}

fn parse_rub_off_radius(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.rub_off_radius = parse_real(tokens)?;
    Ok(())
}

fn parse_allies_only(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.allies_only = parse_bool(tokens)?;
    Ok(())
}

fn parse_exact_match(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.exact_match = parse_bool(tokens)?;
    Ok(())
}

fn parse_allowed_nationalism(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.allowed_nationalism = parse_bool(tokens)?;
    Ok(())
}

fn parse_action(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    let idx =
        INI::parse_index_list(token, HORDE_ACTION_NAMES).map_err(|_| INIError::InvalidData)?;
    data.action = HORDE_ACTION_TYPES
        .get(idx)
        .copied()
        .unwrap_or(HordeActionType::Horde);
    Ok(())
}

fn parse_flag_sub_obj_names(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.flag_sub_obj_names = tokens
        .iter()
        .copied()
        .filter(|t| *t != "=")
        .map(|t| t.to_string())
        .collect();
    Ok(())
}

const HORDE_ACTION_TYPES: &[HordeActionType] = &[HordeActionType::Horde];
const HORDE_ACTION_NAMES: &[&str] = &["HORDE"];

const HORDE_UPDATE_FIELDS: &[FieldParse<HordeUpdateModuleData>] = &[
    FieldParse {
        token: "UpdateRate",
        parse: parse_update_rate,
    },
    FieldParse {
        token: "KindOf",
        parse: parse_kindof,
    },
    FieldParse {
        token: "Count",
        parse: parse_min_count,
    },
    FieldParse {
        token: "Radius",
        parse: parse_min_dist,
    },
    FieldParse {
        token: "RubOffRadius",
        parse: parse_rub_off_radius,
    },
    FieldParse {
        token: "AlliesOnly",
        parse: parse_allies_only,
    },
    FieldParse {
        token: "ExactMatch",
        parse: parse_exact_match,
    },
    FieldParse {
        token: "Action",
        parse: parse_action,
    },
    FieldParse {
        token: "FlagSubObjectNames",
        parse: parse_flag_sub_obj_names,
    },
    FieldParse {
        token: "AllowedNationalism",
        parse: parse_allowed_nationalism,
    },
];

pub struct HordeUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<HordeUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    last_horde_refresh_frame: UnsignedInt,
    in_horde: Bool,
    true_horde_member: Bool,
    has_flag: Bool,
}

impl HordeUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<HordeUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let instance = Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            last_horde_refresh_frame: TheGameLogic::get_frame(),
            in_horde: false,
            true_horde_member: false,
            has_flag: false,
        };

        if let Ok(obj) = object.read() {
            let delay = instance.module_data.update_rate;
            if delay > 0 {
                let wake = GameLogicRandomValue(1, delay as i32) as u32;
                TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::from_u32(wake));
            }
        }

        Ok(instance)
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<HordeUpdateModuleData>,
    ) -> Self {
        let instance = Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            last_horde_refresh_frame: TheGameLogic::get_frame(),
            in_horde: false,
            true_horde_member: false,
            has_flag: false,
        };

        if let Ok(obj) = object.read() {
            let delay = instance.module_data.update_rate;
            if delay > 0 {
                let wake = GameLogicRandomValue(1, delay as i32) as u32;
                TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::from_u32(wake));
            }
        }

        instance
    }

    pub fn is_in_horde(&self) -> Bool {
        self.in_horde
    }

    pub fn is_true_horde_member(&self) -> Bool {
        self.true_horde_member && self.in_horde
    }

    pub fn has_flag(&self) -> Bool {
        self.has_flag
    }

    pub fn is_allowed_nationalism(&self) -> Bool {
        self.module_data.allowed_nationalism
    }

    #[allow(dead_code)]
    fn show_hide_flag(&mut self, show: Bool) {
        self.has_flag = show;
        if self.module_data.flag_sub_obj_names.is_empty() {
            return;
        }
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj) = object_arc.read() else {
            return;
        };
        if let Some(drawable) = obj.get_drawable() {
            if let Ok(mut draw_guard) = drawable.write() {
                for name in &self.module_data.flag_sub_obj_names {
                    draw_guard.show_sub_object(name, show);
                }
                draw_guard.update_sub_objects();
            }
        }
    }

    fn check_horde_status(&mut self) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj) = object_arc.read() else {
            return;
        };
        let owner_id = obj.get_id();
        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        let mut horde_candidates = Vec::new();
        for id in
            partition.get_objects_in_range_boundary_3d_from_object(&obj, self.module_data.min_dist)
        {
            if id == owner_id {
                continue;
            }
            let Some(other_arc) = TheGameLogic::find_object_by_id(id) else {
                continue;
            };
            let Ok(other) = other_arc.read() else {
                continue;
            };

            if self.module_data.exact_match
                && obj.get_template().get_name() != other.get_template().get_name()
            {
                continue;
            }

            let mut has_horde = false;
            other.with_horde_update_interface(|_| {
                has_horde = true;
            });
            if !has_horde {
                continue;
            }

            if !other.is_kind_of_multi(self.module_data.kindof, KIND_OF_MASK_NONE) {
                continue;
            }

            if self.module_data.allies_only {
                let relationship = obj.relationship_to(&other);
                if !matches!(relationship, crate::common::Relationship::Allies) {
                    continue;
                }
            }

            if obj.is_off_map() != other.is_off_map() {
                continue;
            }

            horde_candidates.push(id);
        }

        let required = self.module_data.min_count - 1;
        if required <= 0 || horde_candidates.len() as Int >= required {
            self.in_horde = true;
            self.true_horde_member = true;
            return;
        }

        self.in_horde = false;
        self.true_horde_member = false;

        let rub_off_radius_sq = self.module_data.rub_off_radius * self.module_data.rub_off_radius;
        for id in &horde_candidates {
            let Some(other_arc) = TheGameLogic::find_object_by_id(*id) else {
                continue;
            };
            let Ok(other) = other_arc.read() else {
                continue;
            };
            let mut is_true = false;
            other.with_horde_update_interface(|hui| {
                if hui.is_true_horde_member() {
                    is_true = true;
                }
            });
            if !is_true {
                continue;
            }

            let dist_sq = ThePartitionManager::get_distance_squared(&obj, &other, FROM_CENTER_2D);
            if dist_sq <= rub_off_radius_sq {
                self.in_horde = true;
                break;
            }
        }
    }
}

impl UpdateModuleInterface for HordeUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(obj) = object_arc.read() else {
            return UpdateSleepTime::Forever;
        };

        let current_frame = TheGameLogic::get_frame();
        let is_infantry = obj.is_kind_of(crate::common::KindOf::Infantry);
        let was_in_horde = self.in_horde;

        if is_infantry
            || current_frame > self.last_horde_refresh_frame + self.module_data.update_rate
        {
            self.last_horde_refresh_frame = current_frame;
            self.check_horde_status();
            if let Some(ai) = obj.get_ai_update_interface() {
                let _ = ai.lock().map(|mut ai| ai.evaluate_morale_bonus());
            }
        }

        if let Some(drawable) = obj.get_drawable() {
            if !obj.is_effectively_dead() {
                let draw_icon_ui = TheGameLogic::get_draw_icon_ui();
                let is_portable_structure = obj.is_kind_of(KindOf::PortableStructure);
                let bonus_flags = obj.get_weapon_bonus_condition();
                let has_nationalism = bonus_flags.contains(WeaponBonusConditionFlags::NATIONALISM);
                let has_fanaticism = bonus_flags.contains(WeaponBonusConditionFlags::FANATICISM);

                if draw_icon_ui {
                    if self.in_horde && !is_portable_structure {
                        let decal_type = if is_infantry {
                            if has_fanaticism {
                                TerrainDecalType::HordeWithFanaticismUpgrade
                            } else if has_nationalism {
                                TerrainDecalType::HordeWithNationalismUpgrade
                            } else {
                                TerrainDecalType::Horde
                            }
                        } else {
                            let size = 3.5 * obj.get_geometry_info().get_major_radius();
                            drawable.set_terrain_decal_size(size, size);
                            if has_fanaticism {
                                TerrainDecalType::HordeWithFanaticismUpgrade
                            } else if has_nationalism {
                                TerrainDecalType::HordeWithNationalismUpgradeVehicle
                            } else {
                                TerrainDecalType::HordeVehicle
                            }
                        };

                        drawable.set_terrain_decal(decal_type);
                    }
                } else {
                    drawable.set_terrain_decal(TerrainDecalType::None);
                }

                if !was_in_horde && self.in_horde {
                    drawable.set_terrain_decal_fade_target(1.0, 0.03);
                } else if was_in_horde && !self.in_horde {
                    drawable.set_terrain_decal_fade_target(0.0, -0.03);
                }
            }
        }

        if is_infantry {
            UpdateSleepTime::from_u32(self.module_data.update_rate)
        } else {
            UpdateSleepTime::None
        }
    }
}

impl Snapshotable for HordeUpdate {
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
        xfer.xfer_bool(&mut self.in_horde)
            .map_err(|e| format!("Failed to xfer in_horde: {:?}", e))?;
        xfer.xfer_bool(&mut self.has_flag)
            .map_err(|e| format!("Failed to xfer has_flag: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
impl BehaviorModuleInterface for HordeUpdate {
    fn get_module_name(&self) -> &'static str {
        "HordeUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_horde_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::HordeUpdateInterface> {
        Some(self)
    }
}

impl crate::modules::HordeUpdateInterface for HordeUpdate {
    fn is_true_horde_member(&self) -> bool {
        self.is_true_horde_member()
    }

    fn is_in_horde(&self) -> bool {
        self.is_in_horde()
    }

    fn is_allowed_nationalism(&self) -> bool {
        self.is_allowed_nationalism()
    }
}

/// Glue that exposes HordeUpdate through the common Module trait.
pub struct HordeUpdateModule {
    behavior: HordeUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<HordeUpdateModuleData>,
}

impl HordeUpdateModule {
    pub fn new(
        behavior: HordeUpdate,
        module_name: &AsciiString,
        module_data: Arc<HordeUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut HordeUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for HordeUpdateModule {
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

impl Module for HordeUpdateModule {
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

pub struct HordeUpdateFactory;
impl HordeUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HordeUpdate::new(thing, module_data)?))
    }
}
