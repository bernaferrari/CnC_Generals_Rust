//! PointDefenseLaserUpdate - Anti-missile point defense system
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, KindOf, KindOfMaskType, ModuleData, ObjectStatusTypes, Real, UnsignedInt,
    XferVersion, ALL_KIND_OF,
};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::Object as GameObject;
use crate::weapon::{WeaponAntiMask, WeaponBonus, WeaponSlotType};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct PointDefenseLaserUpdateModuleData {
    pub base: BehaviorModuleData,
    pub weapon_template: String,
    pub primary_target_kind_of: KindOfMaskType,
    pub secondary_target_kind_of: KindOfMaskType,
    pub scan_rate: UnsignedInt,
    pub scan_range: Real,
    pub velocity_factor: Real,
}

impl Default for PointDefenseLaserUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            weapon_template: String::new(),
            primary_target_kind_of: 0,
            secondary_target_kind_of: 0,
            scan_rate: 0,
            scan_range: 0.0,
            velocity_factor: 0.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(PointDefenseLaserUpdateModuleData, base);

impl PointDefenseLaserUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, POINT_DEFENSE_LASER_UPDATE_FIELDS)
    }
}

pub struct PointDefenseLaserUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<PointDefenseLaserUpdateModuleData>,
    enabled: Bool,
    next_call_frame_and_phase: UnsignedInt,
    best_target_id: crate::common::ObjectID,
    next_scan_frames: i32,
    next_shot_available_in_frames: i32,
    in_range: Bool,
}

fn parse_weapon_template(
    _ini: &mut INI,
    data: &mut PointDefenseLaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.weapon_template = token.to_string();
    Ok(())
}

fn parse_primary_target_types(
    _ini: &mut INI,
    data: &mut PointDefenseLaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.primary_target_kind_of =
        crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_secondary_target_types(
    _ini: &mut INI,
    data: &mut PointDefenseLaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.secondary_target_kind_of =
        crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_scan_rate(
    _ini: &mut INI,
    data: &mut PointDefenseLaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scan_rate = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_scan_range(
    _ini: &mut INI,
    data: &mut PointDefenseLaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scan_range = INI::parse_real(token)?;
    Ok(())
}

fn parse_velocity_factor(
    _ini: &mut INI,
    data: &mut PointDefenseLaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.velocity_factor = INI::parse_real(token)?;
    Ok(())
}

const POINT_DEFENSE_LASER_UPDATE_FIELDS: &[FieldParse<PointDefenseLaserUpdateModuleData>] = &[
    FieldParse {
        token: "WeaponTemplate",
        parse: parse_weapon_template,
    },
    FieldParse {
        token: "PrimaryTargetTypes",
        parse: parse_primary_target_types,
    },
    FieldParse {
        token: "SecondaryTargetTypes",
        parse: parse_secondary_target_types,
    },
    FieldParse {
        token: "ScanRate",
        parse: parse_scan_rate,
    },
    FieldParse {
        token: "ScanRange",
        parse: parse_scan_range,
    },
    FieldParse {
        token: "PredictTargetVelocityFactor",
        parse: parse_velocity_factor,
    },
];

impl PointDefenseLaserUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<PointDefenseLaserUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            enabled: true,
            next_call_frame_and_phase: 0,
            best_target_id: crate::common::INVALID_ID,
            next_scan_frames: 0,
            next_shot_available_in_frames: 0,
            in_range: false,
        })
    }

    fn matches_kind_of_mask(obj: &GameObject, mask: KindOfMaskType) -> bool {
        if mask == 0 {
            return false;
        }
        for &kind in ALL_KIND_OF {
            let bit = 1u64 << (kind as u32);
            if (mask & bit) != 0 && obj.is_kind_of(kind) {
                return true;
            }
        }
        false
    }

    fn scan_closest_target(&mut self, owner_guard: &GameObject) -> Option<crate::common::ObjectID> {
        let object_ids = ThePartitionManager::get()
            .map(|mgr| {
                mgr.get_objects_in_range(owner_guard.get_position(), self.module_data.scan_range)
            })
            .unwrap_or_default();

        let mut best_in_range: [Option<crate::common::ObjectID>; 2] = [None, None];
        let mut best_out_range: [Option<crate::common::ObjectID>; 2] = [None, None];
        let mut closest_in = [Real::MAX; 2];
        let mut closest_out = [Real::MAX; 2];

        let template = crate::weapon::with_weapon_store(|store| {
            store
                .find_weapon_template(self.module_data.weapon_template.as_str())
                .cloned()
        })
        .ok()
        .flatten();

        let Some(template) = template else {
            return None;
        };
        let mut bonus = WeaponBonus::new();
        bonus.clear();
        let fire_range = template.get_attack_range(&bonus);

        for obj_id in object_ids {
            if obj_id == owner_guard.get_id() {
                continue;
            }

            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let index = if Self::matches_kind_of_mask(
                &obj_guard,
                self.module_data.primary_target_kind_of,
            ) {
                0
            } else if Self::matches_kind_of_mask(
                &obj_guard,
                self.module_data.secondary_target_kind_of,
            ) {
                1
            } else {
                continue;
            };

            if !obj_guard.is_airborne_target()
                && !template.anti_mask.contains(WeaponAntiMask::GROUND)
            {
                continue;
            }

            if owner_guard.get_relationship_to(&obj_guard) != ObjectRelationship::Enemy {
                continue;
            }

            if obj_guard.test_status(ObjectStatusTypes::Stealthed)
                && !obj_guard.test_status(ObjectStatusTypes::Detected)
                && !obj_guard.test_status(ObjectStatusTypes::Disguised)
            {
                continue;
            }

            let pos = obj_guard.get_position();
            let owner_pos = owner_guard.get_position();
            let dx = pos.x - owner_pos.x;
            let dy = pos.y - owner_pos.y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= fire_range {
                if dist < closest_in[index] {
                    closest_in[index] = dist;
                    best_in_range[index] = Some(obj_id);
                }
            } else if best_in_range[index].is_none() {
                let mut candidate_dist = dist;
                if self.module_data.velocity_factor != 0.0
                    && !obj_guard.is_kind_of(KindOf::Immobile)
                {
                    if let Some(physics_arc) = obj_guard.get_physics() {
                        if let Ok(physics_guard) = physics_arc.lock() {
                            let vel = physics_guard.get_velocity();
                            let predicted = *obj_guard.get_position()
                                + crate::common::Coord3D::new(
                                    vel.x * self.module_data.velocity_factor,
                                    vel.y * self.module_data.velocity_factor,
                                    vel.z * self.module_data.velocity_factor,
                                );
                            let dx = predicted.x - owner_pos.x;
                            let dy = predicted.y - owner_pos.y;
                            candidate_dist = (dx * dx + dy * dy).sqrt();
                        }
                    }
                }

                if candidate_dist < closest_out[index] {
                    closest_out[index] = candidate_dist;
                    best_out_range[index] = Some(obj_id);
                }
            }
        }

        best_in_range[0]
            .or(best_in_range[1])
            .or(best_out_range[0])
            .or(best_out_range[1])
    }

    fn fire_when_ready(&mut self, owner_guard: &GameObject) {
        if self.next_shot_available_in_frames > 0 {
            self.next_shot_available_in_frames -= 1;
            return;
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(self.best_target_id) else {
            return;
        };
        let Ok(target_guard) = target_arc.read() else {
            return;
        };

        if target_guard.is_destroyed() {
            self.best_target_id = crate::common::INVALID_ID;
            self.in_range = false;
            return;
        }

        let template = crate::weapon::with_weapon_store(|store| {
            store
                .find_weapon_template(self.module_data.weapon_template.as_str())
                .cloned()
        })
        .ok()
        .flatten();

        let Some(template) = template else {
            return;
        };

        let mut bonus = WeaponBonus::new();
        bonus.clear();
        let fire_range = template.get_attack_range(&bonus);

        let owner_pos = owner_guard.get_position();
        let target_pos = target_guard.get_position();
        let dx = target_pos.x - owner_pos.x;
        let dy = target_pos.y - owner_pos.y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist <= fire_range {
            self.in_range = true;
        } else {
            if self.in_range {
                self.next_scan_frames = crate::GameLogicRandomValue!(0, 3) as i32;
                self.best_target_id = crate::common::INVALID_ID;
                if self.next_scan_frames == 0 {
                    if let Some(target_id) = self.scan_closest_target(owner_guard) {
                        self.best_target_id = target_id;
                    }
                    self.next_scan_frames = self.module_data.scan_rate as i32;
                }
            }
            self.in_range = false;
            return;
        }

        let _ = crate::weapon::with_weapon_store(|store| {
            let mut weapon = store.allocate_new_weapon(&template, WeaponSlotType::Tertiary);
            let _ = weapon.load_ammo_now(owner_guard.get_id());
            weapon
                .fire_weapon_at_object(owner_guard.get_id(), target_guard.get_id())
                .map_err(|err| err.to_string())?;
            self.next_shot_available_in_frames = template.get_delay_between_shots(&bonus) as i32;
            Ok::<(), String>(())
        });

        if target_guard.is_destroyed() {
            self.next_scan_frames = crate::GameLogicRandomValue!(0, 3) as i32;
            self.best_target_id = crate::common::INVALID_ID;
            if self.next_scan_frames == 0 {
                if let Some(target_id) = self.scan_closest_target(owner_guard) {
                    self.best_target_id = target_id;
                }
                self.next_scan_frames = self.module_data.scan_rate as i32;
            }
        }
    }
}

impl UpdateModuleInterface for PointDefenseLaserUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.enabled {
            return UpdateSleepTime::Forever;
        }

        let Some(owner_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return UpdateSleepTime::Forever;
        };
        if owner_guard.is_destroyed() {
            return UpdateSleepTime::Forever;
        }

        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            self.fire_when_ready(&owner_guard);
            return UpdateSleepTime::Frames(1);
        }

        self.next_scan_frames = self.module_data.scan_rate as i32;
        if let Some(target_id) = self.scan_closest_target(&owner_guard) {
            self.best_target_id = target_id;
            self.fire_when_ready(&owner_guard);
        } else {
            self.best_target_id = crate::common::INVALID_ID;
            self.in_range = false;
        }

        UpdateSleepTime::Frames(1)
    }
}

impl BehaviorModuleInterface for PointDefenseLaserUpdate {
    fn get_module_name(&self) -> &'static str {
        "PointDefenseLaserUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(template) = crate::weapon::with_weapon_store(|store| {
            store
                .find_weapon_template(self.module_data.weapon_template.as_str())
                .cloned()
        })
        .ok()
        .flatten() else {
            log::warn!(
                "PointDefenseLaserUpdate missing weapon template '{}'",
                self.module_data.weapon_template
            );
            return Ok(());
        };

        let mut bonus = WeaponBonus::new();
        bonus.clear();
        let attack_range = template.get_attack_range(&bonus);
        if self.module_data.scan_range <= attack_range {
            log::warn!(
                "PointDefenseLaserUpdate scan range {} should exceed attack range {}",
                self.module_data.scan_range,
                attack_range
            );
        }
        Ok(())
    }
}

impl Snapshotable for PointDefenseLaserUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_object_id(&mut self.best_target_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.in_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.next_scan_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.next_shot_available_in_frames)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes PointDefenseLaserUpdate through the common Module trait.
pub struct PointDefenseLaserUpdateModule {
    behavior: PointDefenseLaserUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<PointDefenseLaserUpdateModuleData>,
}

impl PointDefenseLaserUpdateModule {
    pub fn new(
        behavior: PointDefenseLaserUpdate,
        module_name: &AsciiString,
        module_data: Arc<PointDefenseLaserUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut PointDefenseLaserUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for PointDefenseLaserUpdateModule {
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

impl Module for PointDefenseLaserUpdateModule {
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

pub struct PointDefenseLaserUpdateFactory;
impl PointDefenseLaserUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(PointDefenseLaserUpdate::new(thing, module_data)?))
    }
}

pub fn point_defense_laser_update_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = PointDefenseLaserUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PointDefenseLaserUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn point_defense_laser_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<PointDefenseLaserUpdateModuleData>()
        .expect("PointDefenseLaserUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(crate::common::INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("PointDefenseLaserUpdate requires object");
    let behavior = PointDefenseLaserUpdate::new(object, module_data_arc.clone())
        .expect("PointDefenseLaserUpdate failed to initialize");
    let module_name = AsciiString::from("PointDefenseLaserUpdate");
    Box::new(PointDefenseLaserUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}
