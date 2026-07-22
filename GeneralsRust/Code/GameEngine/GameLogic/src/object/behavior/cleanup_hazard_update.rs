//! CleanupHazardUpdate - Rust conversion of C++ CleanupHazardUpdate
//!
//! Handles independent targeting of hazards to cleanup (e.g., radiation, poison).
//! Author: Kris Morness, August 2002 (C++ version)
//! Rust conversion: 2025

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, KindOf, ModuleData, ObjectID, Real, UnsignedInt, FROM_CENTER_2D,
    INVALID_ID,
};
use crate::helpers::{game_logic_random_value, ThePartitionManager};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, CleanupHazardUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::{Object as GameObject, OBJECT_REGISTRY};
use crate::weapon::{WeaponLockType, WeaponSetType, WeaponSlotType, WeaponTemplate};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{
    CleanupHazardControlInterface, Module, ModuleData as EngineModuleData, NameKeyType,
};
use log::error;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct CleanupHazardUpdateModuleData {
    pub base: BehaviorModuleData,
    pub weapon_slot: WeaponSlotType,
    pub scan_frames: u32,
    pub scan_range: Real,
}

impl Default for CleanupHazardUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            weapon_slot: WeaponSlotType::Primary,
            scan_frames: 0,
            scan_range: 0.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(CleanupHazardUpdateModuleData, base);

impl CleanupHazardUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CLEANUP_HAZARD_UPDATE_FIELDS)
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_weapon_slot(token: &str) -> Result<WeaponSlotType, INIError> {
    match token.to_ascii_uppercase().as_str() {
        "PRIMARY_WEAPON" | "PRIMARY" => Ok(WeaponSlotType::Primary),
        "SECONDARY_WEAPON" | "SECONDARY" => Ok(WeaponSlotType::Secondary),
        "TERTIARY_WEAPON" | "TERTIARY" => Ok(WeaponSlotType::Tertiary),
        _ => Err(INIError::InvalidData),
    }
}

const CLEANUP_HAZARD_UPDATE_FIELDS: &[FieldParse<CleanupHazardUpdateModuleData>] = &[
    FieldParse {
        token: "WeaponSlot",
        parse: |_, data, tokens| {
            data.weapon_slot = parse_weapon_slot(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ScanRate",
        parse: |_, data, tokens| {
            data.scan_frames = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ScanRange",
        parse: |_, data, tokens| {
            data.scan_range = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
];

pub struct CleanupHazardUpdate {
    object_id: ObjectID,
    module_data: Arc<CleanupHazardUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    best_target_id: ObjectID,
    next_scan_frames: i32,
    next_shot_available_in_frames: i32,
    in_range: bool,
    weapon_template: Option<Arc<WeaponTemplate>>,
    pos: Coord3D,
    move_range: Real,
}

impl CleanupHazardUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<CleanupHazardUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            best_target_id: INVALID_ID,
            next_scan_frames: 0,
            next_shot_available_in_frames: 0,
            in_range: false,
            weapon_template: None,
            pos: Coord3D::default(),
            move_range: 0.0,
        })
    }

    pub fn scan_closest_target(&mut self) -> Option<ObjectID> {
        let me_arc = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })?;
        let me = me_arc.read().ok()?;

        let partition = ThePartitionManager::get()?;

        let target_pos = if self.move_range > 0.0 {
            &self.pos
        } else {
            me.get_position()
        };

        let radius = if self.move_range > 0.0 {
            self.module_data.scan_range + self.move_range
        } else {
            self.module_data.scan_range
        };

        // Filter for KINDOF_CLEANUP_HAZARD
        let best_target = partition.get_closest_object(target_pos, radius, |obj| {
            obj.is_kind_of(KindOf::CleanupHazard)
        });

        self.best_target_id = best_target.unwrap_or(INVALID_ID);
        best_target
    }

    pub fn fire_when_ready(&mut self) {
        let Some(me_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return;
        };
        let Ok(mut me) = me_arc.write() else {
            return;
        };

        let mut target_id = self.best_target_id;

        // Track target and check range if not cleaning an area
        if target_id != INVALID_ID && self.move_range == 0.0 {
            let fire_range = if let Some(ref template) = self.weapon_template {
                template.get_attack_range(&Default::default())
            } else {
                0.0
            };
            match OBJECT_REGISTRY.with_object(target_id, |target| {
                ThePartitionManager::get_distance_squared(&me, target, FROM_CENTER_2D)
            }) {
                Some(dist_sqr) => {
                    if dist_sqr < fire_range * fire_range {
                        self.in_range = true;
                    } else if self.in_range {
                        // Out of range, force new scan
                        self.next_scan_frames = game_logic_random_value(0, 3) as i32;
                        self.best_target_id = INVALID_ID;
                        if self.next_scan_frames == 0 {
                            self.scan_closest_target();
                            self.next_scan_frames = self.module_data.scan_frames as i32;
                            target_id = self.best_target_id;
                        } else {
                            target_id = INVALID_ID;
                        }
                    } else {
                        self.in_range = false;
                    }
                }
                None => {
                    self.best_target_id = INVALID_ID;
                    target_id = INVALID_ID;
                }
            }
        }

        if self.next_shot_available_in_frames > 0 {
            self.next_shot_available_in_frames -= 1;
            return;
        }

        if target_id != INVALID_ID {
            if OBJECT_REGISTRY.with_object(target_id, |_| ()).is_some() {
                if let Some(ai_arc) = me.get_ai() {
                    if let Ok(mut ai) = ai_arc.lock() {
                        if !(ai.is_idle() || ai.is_busy()) {
                            return;
                        }
                        me.set_weapon_lock(
                            self.module_data.weapon_slot,
                            WeaponLockType::LockedTemporarily,
                        );
                        let mut params = AiCommandParams::new(
                            AiCommandType::AttackObject,
                            CommandSourceType::FromAi,
                        );
                        params.obj = Some(target_id);
                        params.int_value = -1;
                        let _ = ai.execute_command(&params);
                    }
                }
            }
        }
    }
}

impl CleanupHazardUpdateInterface for CleanupHazardUpdate {
    fn set_cleanup_area_parameters(&mut self, pos: &Coord3D, range: Real) {
        self.move_range = range;
        self.pos = *pos;

        let Some(me_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return;
        };
        let Ok(me) = me_arc.read() else {
            return;
        };

        if let Some(ai_arc) = me.get_ai() {
            ai_arc.ai_move_to_position(pos, false, CommandSourceType::FromAi);
        }
    }
}

impl UpdateModuleInterface for CleanupHazardUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(me_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return UPDATE_SLEEP_NONE;
        };

        // Handle busy status for area cleanup
        if self.move_range > 0.0 {
            let Some(ai_arc) = me_arc.read().ok().and_then(|me| me.get_ai()) else {
                return UPDATE_SLEEP_NONE;
            };
            if ai_arc.is_idle() {
                ai_arc.ai_busy(CommandSourceType::FromAi);
            } else if ai_arc.get_last_command_source() != CommandSourceType::FromAi {
                // Canceled by user/script (abandon the cleanup)
                self.move_range = 0.0;
                return UPDATE_SLEEP_NONE;
            }
        }

        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            self.fire_when_ready();
            return UPDATE_SLEEP_NONE;
        }
        self.next_scan_frames = self.module_data.scan_frames as i32;

        if self.scan_closest_target().is_some() {
            self.fire_when_ready();
        } else if self.move_range > 0.0 {
            if let Some(ai_arc) = me_arc.read().ok().and_then(|me| me.get_ai()) {
                if ai_arc.is_idle() || ai_arc.is_busy() {
                    if let Ok(me) = me_arc.read() {
                        let dist_sqr = ThePartitionManager::get_distance_squared_to_pos(
                            &me,
                            &self.pos,
                            FROM_CENTER_2D,
                        );
                        if dist_sqr < 25.0 * 25.0 {
                            self.move_range = 0.0;
                        } else {
                            ai_arc.ai_move_to_position(&self.pos, false, CommandSourceType::FromAi);
                        }
                    }
                }
            }
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for CleanupHazardUpdate {
    fn get_module_name(&self) -> &'static str {
        "CleanupHazardUpdate"
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let me_arc = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })
        .ok_or("Object lost")?;
        let mut me = me_arc.write().unwrap();

        me.set_weapon_set_flag(WeaponSetType::Veteran);
        if let Some(weapon_arc) = me.get_weapon_in_slot(self.module_data.weapon_slot) {
            self.weapon_template = Some(Arc::clone(weapon_arc.get_template()));
        } else {
            return Err(format!(
                "CleanupHazardUpdate for {} doesn't have a valid weapon template",
                me.get_template().get_name()
            )
            .into());
        }

        // Validate scan range vs attack range
        if let Some(ref template) = self.weapon_template {
            let attack_range = template.get_attack_range(&Default::default());
            if self.module_data.scan_range <= attack_range {
                error!("CleanupHazardUpdate for {} requires the scan range ({:.1}) being larger than the firing range ({:.1})",
                    me.get_template().get_name(), self.module_data.scan_range, attack_range);
            }
        }

        Ok(())
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_cleanup_hazard_update_interface(
        &mut self,
    ) -> Option<&mut dyn CleanupHazardUpdateInterface> {
        Some(self)
    }
}

impl Snapshotable for CleanupHazardUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("CleanupHazardUpdate xfer version failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_object_id(&mut self.best_target_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.in_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.next_scan_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.next_shot_available_in_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_coord3d(&mut self.pos);
        xfer.xfer_real(&mut self.move_range)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes CleanupHazardUpdate through the common Module trait.
pub struct CleanupHazardUpdateModule {
    behavior: CleanupHazardUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<CleanupHazardUpdateModuleData>,
}

impl CleanupHazardUpdateModule {
    pub fn new(
        behavior: CleanupHazardUpdate,
        module_name: &AsciiString,
        module_data: Arc<CleanupHazardUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut CleanupHazardUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for CleanupHazardUpdateModule {
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

impl Module for CleanupHazardUpdateModule {
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

    fn get_cleanup_hazard_control_interface(
        &mut self,
    ) -> Option<&mut dyn CleanupHazardControlInterface> {
        Some(self)
    }
}

impl CleanupHazardControlInterface for CleanupHazardUpdateModule {
    fn set_cleanup_area_parameters(&mut self, x: f32, y: f32, z: f32, range: f32) {
        let pos = Coord3D::new(x, y, z);
        self.behavior.set_cleanup_area_parameters(&pos, range);
    }
}

pub struct CleanupHazardUpdateFactory;
impl CleanupHazardUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(CleanupHazardUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lost_owner_update() -> CleanupHazardUpdate {
        CleanupHazardUpdate {
            object_id: crate::common::INVALID_ID,
            module_data: Arc::new(CleanupHazardUpdateModuleData {
                scan_frames: 7,
                scan_range: 100.0,
                ..CleanupHazardUpdateModuleData::default()
            }),
            next_call_frame_and_phase: 0,
            best_target_id: INVALID_ID,
            next_scan_frames: 0,
            next_shot_available_in_frames: 0,
            in_range: false,
            weapon_template: None,
            pos: Coord3D::default(),
            move_range: 0.0,
        }
    }

    #[test]
    fn cleanup_hazard_lost_owner_paths_do_not_panic() {
        let mut update = lost_owner_update();

        assert_eq!(update.scan_closest_target(), None);
        update.fire_when_ready();
        update.set_cleanup_area_parameters(
            &Coord3D {
                x: 12.0,
                y: 34.0,
                z: 0.0,
            },
            50.0,
        );
        assert_eq!(update.update_simple(), UPDATE_SLEEP_NONE);
    }

    #[test]
    fn cleanup_hazard_set_area_records_parameters_before_owner_lookup() {
        let mut update = lost_owner_update();
        let pos = Coord3D {
            x: 4.0,
            y: 8.0,
            z: 1.0,
        };

        update.set_cleanup_area_parameters(&pos, 64.0);

        assert_eq!(update.pos, pos);
        assert_eq!(update.move_range, 64.0);
    }

    #[test]
    fn cleanup_hazard_module_exposes_typed_control_hook() {
        let data = Arc::new(CleanupHazardUpdateModuleData {
            scan_frames: 7,
            scan_range: 100.0,
            ..CleanupHazardUpdateModuleData::default()
        });
        let mut module = CleanupHazardUpdateModule::new(
            lost_owner_update(),
            &AsciiString::from("CleanupHazardUpdate"),
            data,
        );

        let control = module
            .get_cleanup_hazard_control_interface()
            .expect("CleanupHazardUpdate module should expose cleanup control");
        control.set_cleanup_area_parameters(10.0, 20.0, 3.0, 75.0);

        assert_eq!(module.behavior.pos, Coord3D::new(10.0, 20.0, 3.0));
        assert_eq!(module.behavior.move_range, 75.0);
    }
}
