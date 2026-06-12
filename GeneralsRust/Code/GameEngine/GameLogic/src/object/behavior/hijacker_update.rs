//! HijackerUpdate - Rust conversion of C++ HijackerUpdate
//!
//! Allows hijacker to stay with hijacked vehicle until it dies.
//! Author: Mark Lorenzen, July 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    Bool, CommandSourceType, Coord3D, ModuleData, ObjectID, ObjectStatusMaskType, UnsignedInt,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::HijackerControlInterface;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct HijackerUpdateModuleData {
    pub base: BehaviorModuleData,
    pub attach_to_bone: String,
    pub parachute_name: String,
}

impl Default for HijackerUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            attach_to_bone: String::new(),
            parachute_name: String::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(HijackerUpdateModuleData, base);

impl HijackerUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HIJACKER_UPDATE_FIELDS)
    }
}

fn parse_attach_to_target_bone(
    _ini: &mut INI,
    data: &mut HijackerUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.attach_to_bone = token.to_string();
    Ok(())
}

fn parse_parachute_name(
    _ini: &mut INI,
    data: &mut HijackerUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.parachute_name = token.to_string();
    Ok(())
}

const HIJACKER_UPDATE_FIELDS: &[FieldParse<HijackerUpdateModuleData>] = &[
    FieldParse {
        token: "AttachToTargetBone",
        parse: parse_attach_to_target_bone,
    },
    FieldParse {
        token: "ParachuteName",
        parse: parse_parachute_name,
    },
];

pub struct HijackerUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<HijackerUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    target_id: ObjectID,
    eject_pos: Coord3D,
    update: Bool,
    is_in_vehicle: Bool,
    was_target_airborne: Bool,
}

impl HijackerUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<HijackerUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            target_id: OBJECT_INVALID_ID,
            eject_pos: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            update: false,
            is_in_vehicle: false,
            was_target_airborne: false,
        })
    }

    pub fn set_target_object(&mut self, target_id: ObjectID) {
        self.target_id = target_id;
    }

    pub fn get_target_object(&self) -> ObjectID {
        self.target_id
    }

    pub fn set_update(&mut self, update: Bool) {
        self.update = update;
    }

    pub fn set_is_in_vehicle(&mut self, is_in_vehicle: Bool) {
        self.is_in_vehicle = is_in_vehicle;
    }
}

impl UpdateModuleInterface for HijackerUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.update {
            return UpdateSleepTime::None;
        }

        if self.is_in_vehicle {
            if let Some(target_arc) = TheGameLogic::find_object_by_id(self.target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    let target_pos = *target_guard.get_position();
                    let target_tracker = target_guard.get_experience_tracker();
                    let target_level = target_guard.get_veterancy_level();
                    self.was_target_airborne = target_guard.is_significantly_above_terrain();
                    self.eject_pos = target_pos;

                    if let Some(hijacker_arc) = self.object.upgrade() {
                        if let Ok(mut hijacker_guard) = hijacker_arc.write() {
                            let hijacker_tracker = hijacker_guard.get_experience_tracker();
                            let hijacker_level = hijacker_guard.get_veterancy_level();
                            let _ = hijacker_guard.set_position(&target_pos);
                            drop(hijacker_guard);

                            if let (Some(target_tracker), Some(hijacker_tracker)) =
                                (target_tracker, hijacker_tracker)
                            {
                                let highest_level = target_level.max(hijacker_level);
                                if Arc::ptr_eq(&target_tracker, &hijacker_tracker) {
                                    if let Ok(mut tracker_guard) = target_tracker.lock() {
                                        let _ = tracker_guard.set_veterancy_level(highest_level);
                                    }
                                } else {
                                    if let Ok(mut tracker_guard) = hijacker_tracker.lock() {
                                        let _ = tracker_guard.set_veterancy_level(highest_level);
                                    }
                                    if let Ok(mut tracker_guard) = target_tracker.lock() {
                                        let _ = tracker_guard.set_veterancy_level(highest_level);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                if let Some(hijacker_arc) = self.object.upgrade() {
                    if let Ok(hijacker_guard) = hijacker_arc.read() {
                        if let Some(container_arc) = hijacker_guard.get_container() {
                            if let Ok(container_guard) = container_arc.read() {
                                if let Some(contain_arc) = container_guard.get_contain() {
                                    if let Ok(mut contain_guard) = contain_arc.lock() {
                                        let _ =
                                            contain_guard.release_object(hijacker_guard.get_id());
                                    }
                                }
                            }
                        }
                    }
                    if let Ok(mut hijacker_guard) = hijacker_arc.write() {
                        let _ = hijacker_guard.set_position(&self.eject_pos);
                        if let Some(drawable) = hijacker_guard.get_drawable() {
                            if let Ok(mut drawable_guard) = drawable.write() {
                                let _ = drawable_guard.set_drawable_hidden(false);
                            }
                        }
                        hijacker_guard.set_status(
                            ObjectStatusMaskType::NO_COLLISIONS
                                | ObjectStatusMaskType::MASKED
                                | ObjectStatusMaskType::UNSELECTABLE,
                            false,
                        );
                        hijacker_guard.handle_partition_cell_maintenance();
                        let ai = hijacker_guard.get_ai();
                        drop(hijacker_guard);

                        if let Some(ai) = ai {
                            ai.ai_idle(CommandSourceType::FromAi);
                        }
                    }
                }

                self.target_id = OBJECT_INVALID_ID;
                self.is_in_vehicle = false;
                self.update = false;
                self.was_target_airborne = false;
                return UpdateSleepTime::None;
            }

            return UpdateSleepTime::None;
        }

        self.was_target_airborne = false;
        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for HijackerUpdate {
    fn get_module_name(&self) -> &'static str {
        "HijackerUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_hijacker_control_interface(&mut self) -> Option<&mut dyn HijackerControlInterface> {
        Some(self)
    }
}

impl HijackerControlInterface for HijackerUpdate {
    fn configure_hijacked_vehicle(&mut self, target_id: ObjectID) {
        self.set_target_object(target_id);
        self.set_update(true);
        self.set_is_in_vehicle(true);
    }
}

impl Snapshotable for HijackerUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("HijackerUpdate xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_object_id(&mut self.target_id)
            .map_err(|e| format!("HijackerUpdate xfer target_id: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.eject_pos);
        xfer.xfer_bool(&mut self.update)
            .map_err(|e| format!("HijackerUpdate xfer update: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_in_vehicle)
            .map_err(|e| format!("HijackerUpdate xfer is_in_vehicle: {:?}", e))?;
        xfer.xfer_bool(&mut self.was_target_airborne)
            .map_err(|e| format!("HijackerUpdate xfer was_target_airborne: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct HijackerUpdateFactory;
impl HijackerUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HijackerUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ObjectStatusTypes, VeterancyLevel};
    use crate::experience::ExperienceTracker;
    use crate::object::registry::OBJECT_REGISTRY;
    use std::sync::Mutex;

    fn module_data() -> Arc<dyn ModuleData> {
        Arc::new(HijackerUpdateModuleData::default())
    }

    #[test]
    fn inactive_hijacker_update_runs_again_next_frame_like_cpp() {
        let hijacker = Arc::new(RwLock::new(GameObject::new_test(9301, 100.0)));
        let mut update = HijackerUpdate::new(Arc::clone(&hijacker), module_data()).unwrap();

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
    }

    #[test]
    fn hijacker_in_vehicle_tracks_target_position_each_frame() {
        let hijacker = Arc::new(RwLock::new(GameObject::new_test(9302, 100.0)));
        let target = Arc::new(RwLock::new(GameObject::new_test(9303, 100.0)));
        let target_pos = Coord3D {
            x: 35.0,
            y: -12.0,
            z: 4.0,
        };
        target.write().unwrap().set_position(&target_pos).unwrap();
        OBJECT_REGISTRY.register_object(9302, &hijacker);
        OBJECT_REGISTRY.register_object(9303, &target);

        let mut update = HijackerUpdate::new(Arc::clone(&hijacker), module_data()).unwrap();
        update.configure_hijacked_vehicle(9303);

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
        assert_eq!(*hijacker.read().unwrap().get_position(), target_pos);
        assert_eq!(update.eject_pos, target_pos);
        assert_eq!(update.target_id, 9303);
        assert!(update.update);
        assert!(update.is_in_vehicle);

        OBJECT_REGISTRY.unregister_object(9302);
        OBJECT_REGISTRY.unregister_object(9303);
    }

    #[test]
    fn hijacker_in_vehicle_keeps_highest_veterancy_with_target() {
        let hijacker = Arc::new(RwLock::new(GameObject::new_test(9304, 100.0)));
        let target = Arc::new(RwLock::new(GameObject::new_test(9305, 100.0)));
        let hijacker_tracker = Arc::new(Mutex::new(ExperienceTracker::new(9304)));
        let target_tracker = Arc::new(Mutex::new(ExperienceTracker::new(9305)));
        hijacker_tracker
            .lock()
            .unwrap()
            .set_veterancy_level(VeterancyLevel::Veteran);
        target_tracker
            .lock()
            .unwrap()
            .set_veterancy_level(VeterancyLevel::Elite);
        hijacker.write().unwrap().experience_tracker = Some(Arc::clone(&hijacker_tracker));
        target.write().unwrap().experience_tracker = Some(Arc::clone(&target_tracker));
        OBJECT_REGISTRY.register_object(9304, &hijacker);
        OBJECT_REGISTRY.register_object(9305, &target);

        let mut update = HijackerUpdate::new(Arc::clone(&hijacker), module_data()).unwrap();
        update.configure_hijacked_vehicle(9305);

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
        assert_eq!(
            hijacker_tracker.lock().unwrap().get_veterancy_level(),
            VeterancyLevel::Elite
        );
        assert_eq!(
            target_tracker.lock().unwrap().get_veterancy_level(),
            VeterancyLevel::Elite
        );

        OBJECT_REGISTRY.unregister_object(9304);
        OBJECT_REGISTRY.unregister_object(9305);
    }

    #[test]
    fn missing_target_restores_hijacker_object_state() {
        let hijacker = Arc::new(RwLock::new(GameObject::new_test(9306, 100.0)));
        let eject_pos = Coord3D {
            x: 8.0,
            y: 9.0,
            z: 10.0,
        };
        {
            let mut hijacker_guard = hijacker.write().unwrap();
            hijacker_guard
                .set_position(&Coord3D {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                })
                .unwrap();
            hijacker_guard.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
            hijacker_guard.set_status(ObjectStatusMaskType::MASKED, true);
            hijacker_guard.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
        }
        OBJECT_REGISTRY.register_object(9306, &hijacker);

        let mut update = HijackerUpdate::new(Arc::clone(&hijacker), module_data()).unwrap();
        update.configure_hijacked_vehicle(99_999);
        update.eject_pos = eject_pos;
        update.was_target_airborne = true;

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
        assert_eq!(update.target_id, OBJECT_INVALID_ID);
        assert!(!update.update);
        assert!(!update.is_in_vehicle);
        assert!(!update.was_target_airborne);

        let hijacker_guard = hijacker.read().unwrap();
        assert_eq!(*hijacker_guard.get_position(), eject_pos);
        assert!(!hijacker_guard.test_status(ObjectStatusTypes::NoCollisions));
        assert!(!hijacker_guard.test_status(ObjectStatusTypes::Masked));
        assert!(!hijacker_guard.test_status(ObjectStatusTypes::Unselectable));

        OBJECT_REGISTRY.unregister_object(9306);
    }
}
