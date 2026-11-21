//! PowerPlantUpdate - Rust conversion of C++ PowerPlantUpdate
//!
//! Update module for power plant buildings that generate power for a player.
//! Author: Amit Kumar, August 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::types::ModelConditionFlags;
use crate::common::{AsciiString, Bool, ModuleData, ObjectID, UnsignedInt};
use crate::modules::{
    BehaviorModuleInterface, PowerPlantUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::Object as GameObject;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::system::game_logic::get_game_logic;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Thing as ModuleThing,
};
use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

/// Wave 392: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

#[allow(dead_code)]
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

/// Module data for PowerPlantUpdate
#[derive(Clone, Debug)]
pub struct PowerPlantUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub rods_extend_time: UnsignedInt,
}

impl Default for PowerPlantUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            rods_extend_time: 0,
        }
    }
}

impl PowerPlantUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, POWER_PLANT_UPDATE_FIELDS)
    }
}

const POWER_PLANT_UPDATE_FIELDS: &[FieldParse<PowerPlantUpdateModuleData>] = &[FieldParse {
    token: "RodsExtendTime",
    parse: |_, data, tokens| {
        let value = tokens.iter().copied().find(|token| *token != "=");
        data.rods_extend_time =
            INI::parse_duration_unsigned_int(value.ok_or(INIError::InvalidData)?)?;
        Ok(())
    },
}];

crate::impl_legacy_module_data_with_key_field!(PowerPlantUpdateModuleData, module_tag_name_key);

impl Snapshotable for PowerPlantUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.rods_extend_time)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// PowerPlantUpdate behavior module
pub struct PowerPlantUpdate {
    object_id: ObjectID,
    module_data: game_engine::common::thing::module::ModuleDataView<PowerPlantUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    extended: Bool,
    extend_done_frame: UnsignedInt,
}

impl PowerPlantUpdate {
    pub fn new_with_module_data(object: Arc<RwLock<GameObject>>, data: game_engine::common::thing::module::ModuleDataHandle) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !data.is::<PowerPlantUpdateModuleData>() { return Err("Invalid module data type for PowerPlantUpdate".into()); }
        Ok(Self {
            object_id: object.read().ok().map(|g| g.get_id()).unwrap_or(crate::common::INVALID_ID),
            module_data: game_engine::common::thing::module::ModuleDataView::new(data, |data| data.as_any().downcast_ref::<PowerPlantUpdateModuleData>().expect("PowerPlantUpdate module data type")),
            next_call_frame_and_phase: 0,
            extended: false,
            extend_done_frame: 0,
        })
    }


    /// Extend or retract the power plant rods
    pub fn extend_rods(&mut self, extend: Bool) {
        // Wave 392: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        // Matches C++ PowerPlantUpdate::extendRods (sets upgrading flag and wake frame)
        if extend {
            if !self.extended {
                if self.object_id != crate::common::INVALID_ID {
                    if let Some(object) = crate::helpers::TheGameLogic::find_object_by_id(
                        self.object_id,
                    )
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
                    {
                        if let Ok(mut obj) = object.write() {
                            obj.set_model_condition_state(
                                ModelConditionFlags::POWER_PLANT_UPGRADING,
                            );
                        }
                    }
                }
                self.extended = true;
                let now = crate::helpers::TheGameLogic::get_frame();
                self.next_call_frame_and_phase =
                    now.saturating_add(self.module_data.read().rods_extend_time);
                self.reschedule_self();
            }
        } else {
            self.extended = false;
            self.extend_done_frame = 0;
            self.next_call_frame_and_phase = UPDATE_SLEEP_FOREVER.to_u32();
            self.reschedule_self();

            if let Some(object) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) {
                if let Ok(mut obj) = object.write() {
                    // Clear both upgrading and upgraded visual flags immediately
                    obj.clear_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADING);
                    obj.clear_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADED);
                }
            }
        }
    }

    fn reschedule_self(&self) {
        if self.object_id == crate::common::INVALID_ID {
            return;
        }
        let Some(object) = crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        else {
            return;
        };
        if let Ok(guard) = object.read() {
            let module_tag = self.module_data.read().get_module_tag_name_key();
            if !guard.reschedule_tagged_update(module_tag, self.next_call_frame_and_phase) {
                guard.reschedule_named_update("PowerPlantUpdate", self.next_call_frame_and_phase);
            }
        }
    }

    /// Check if rods are extended
    pub fn is_extended(&self) -> Bool {
        self.extended
    }
}

impl UpdateModuleInterface for PowerPlantUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Wave 392: empty dual-world → Forever.
        if dual_world_registry_unavailable() {
            return UPDATE_SLEEP_FOREVER;
        }

        if let Some(object) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(mut obj) = object.write() {
                obj.clear_and_set_model_condition_flags(
                    ModelConditionFlags::POWER_PLANT_UPGRADING,
                    ModelConditionFlags::POWER_PLANT_UPGRADED,
                );
            }
        }
        self.extended = true;
        UPDATE_SLEEP_FOREVER
    }
}

impl BehaviorModuleInterface for PowerPlantUpdate {
    fn get_module_name(&self) -> &'static str {
        "PowerPlantUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_power_plant_update_interface(&mut self) -> Option<&mut dyn PowerPlantUpdateInterface> {
        Some(self)
    }
}

impl PowerPlantUpdateInterface for PowerPlantUpdate {
    fn extend_rods(&mut self, extend: Bool) {
        self.extend_rods(extend);
    }
}

impl Snapshotable for PowerPlantUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_bool(&mut self.extended)
            .map_err(|e| format!("Failed to xfer extended: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes PowerPlantUpdate through the common Module trait.
pub struct PowerPlantUpdateModule {
    behavior: PowerPlantUpdate,
    module_name_key: NameKeyType,
    module_data: game_engine::common::thing::module::ModuleDataHandle,
}

impl PowerPlantUpdateModule {
    pub fn new(
        behavior: PowerPlantUpdate,
        module_name: &AsciiString,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut PowerPlantUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for PowerPlantUpdateModule {
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

impl Module for PowerPlantUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.module_data)
    }

    /// C++ `UpdateModule : Module` interface query
    /// (`Module::DynamicInterfaceCast(ModuleInterfaceType::UPDATE)`): forwards
    /// to the wrapped behavior's per-frame hooks. Replaces the former
    /// `as_any().downcast_mut::<Self>()` branch in `ModuleUpdateProxy`.
    fn get_update_module_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        Some(self.behavior_mut())
    }
}

// Factory for creating PowerPlantUpdate instances
pub struct PowerPlantUpdateFactory;

impl PowerPlantUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let behavior = PowerPlantUpdate::new_with_module_data(thing, module_data)?;
        Ok(Box::new(behavior))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    // A real canonical object, rather than an INVALID_ID module literal or
    // another test's leftover object. This fixture does not reset any world.
    struct RodOwner {
        logic: crate::system::game_logic::GameLogic,
        object: Arc<RwLock<GameObject>>,
        id: ObjectID,
    }

    impl RodOwner {
        fn new(id: ObjectID) -> Self {
            let object = Arc::new(RwLock::new(GameObject::new_test(id, 100.0)));
            let mut logic = crate::system::game_logic::GameLogic::new();
            logic.register_object(object.clone()).unwrap();
            Self { logic, object, id }
        }
    }

    impl Drop for RodOwner {
        fn drop(&mut self) {
            let unwinding = std::thread::panicking();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert!(Arc::ptr_eq(
                    &self.logic.find_object_by_id(self.id).unwrap(),
                    &self.object
                ));
                self.logic.destroy_object(self.id);
                self.logic.process_destroy_list().unwrap();
                assert!(self.logic.find_object_by_id(self.id).is_none());
                assert_eq!(
                    self.object.read().unwrap().get_id(),
                    crate::common::INVALID_ID
                );
            }));
            if let Err(error) = result {
                if unwinding {
                    eprintln!("power plant fixture retirement failed during unwind");
                } else {
                    std::panic::resume_unwind(error);
                }
            }
        }
    }

    #[test]
    fn test_power_plant_creation() {
        let data = PowerPlantUpdateModuleData::default();
        assert_eq!(data.rods_extend_time, 0);
    }

    #[test]
    fn power_plant_retract_cancels_pending_extension_before_completion() {
        let _lock = crate::test_sync::lock();
        let owner = RodOwner::new(0xD158);
        let data = game_engine::common::thing::module::ModuleDataHandle::new(PowerPlantUpdateModuleData::default());
        let mut update = PowerPlantUpdate::new_with_module_data(owner.object.clone(), data).unwrap();
        update.extend_done_frame = 123;
        update.extend_rods(false);

        assert!(!update.extended);
        assert_eq!(update.extend_done_frame, 0);
    }

    #[test]
    fn power_plant_extension_marks_extended_immediately_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = RodOwner::new(0xD159);
        let data = game_engine::common::thing::module::ModuleDataHandle::new(PowerPlantUpdateModuleData {
            rods_extend_time: 25,
            ..PowerPlantUpdateModuleData::default()
        });
        let mut update = PowerPlantUpdate::new_with_module_data(owner.object.clone(), data).unwrap();
        update.extend_rods(true);

        assert!(update.extended);
    }

    #[test]
    fn power_plant_update_xfer_preserves_cpp_runtime_fields_only() {
        let module_data = game_engine::common::thing::module::ModuleDataView::new(game_engine::common::thing::module::ModuleDataHandle::new(PowerPlantUpdateModuleData::default()), |data| data.as_any().downcast_ref::<PowerPlantUpdateModuleData>().unwrap());
        let mut saved = PowerPlantUpdate {
            object_id: crate::common::INVALID_ID,
            module_data: module_data.clone(),
            next_call_frame_and_phase: 0x1234,
            extended: true,
            extend_done_frame: 1234,
        };

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("power_plant_update").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = PowerPlantUpdate {
            object_id: crate::common::INVALID_ID,
            module_data,
            next_call_frame_and_phase: 0,
            extended: false,
            extend_done_frame: 77,
        };
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("power_plant_update").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        assert!(loaded.extended);
        assert_eq!(loaded.next_call_frame_and_phase, 0x1234);
        assert_eq!(loaded.extend_done_frame, 77);
    }
    /// C++ `UpdateModule::setWakeFrame` wakes `this` only. Two differently
    /// tagged PowerPlantUpdate instances on one Object must schedule
    /// independently through `reschedule_tagged_update`.
    #[test]
    fn tagged_power_plant_wake_hits_only_the_matching_installed_entry() {
        use crate::modules::UpdateModulePtr;
        use crate::object::ModuleUpdateProxy;

        fn install(
            owner: &Arc<RwLock<GameObject>>,
            register: &mut dyn FnMut(ObjectID, UpdateModulePtr, UnsignedInt),
            tag: NameKeyType,
        ) -> UpdateModulePtr {
            let data = game_engine::common::thing::module::ModuleDataHandle::new(
                PowerPlantUpdateModuleData {
                    module_tag_name_key: tag,
                    ..Default::default()
                },
            );
            let behavior =
                PowerPlantUpdate::new_with_module_data(owner.clone(), data.clone()).unwrap();
            let module = PowerPlantUpdateModule::new(
                behavior,
                &AsciiString::from("PowerPlantUpdate"),
                data.clone(),
            );
            owner.write().unwrap().install_update_module(
                "PowerPlantUpdate",
                Box::new(module),
                data,
            );
            let (id, entry) = {
                let object = owner.read().unwrap();
                (object.get_id(), object.modules.last().unwrap().clone())
            };
            let proxy: UpdateModulePtr =
                Arc::new(RwLock::new(ModuleUpdateProxy::new(entry.clone(), id)));
            owner
                .write()
                .unwrap()
                .attach_update_module_registration(proxy.clone(), Some(&entry))
                .unwrap();
            register(id, proxy.clone(), 40);
            proxy
        }

        let _serial = crate::test_sync::lock();
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let stores = crate::system::engine_stores::new_for_world();
            crate::system::engine_stores::with_active_stores(&stores, || {
                let id = 0xD160;
                let owner = Arc::new(RwLock::new(GameObject::new_test(id, 100.0)));
                let ambient = crate::system::game_logic::get_game_logic();
                ambient.lock().unwrap().admit_scheduling_owner(id, owner.clone());
                let mut register = |id, proxy, wake| {
                    ambient
                        .lock()
                        .unwrap()
                        .register_sleepy_update_module(id, proxy, wake);
                };
                let first = install(&owner, &mut register, 1);
                let second = install(&owner, &mut register, 2);

                // Install order is preserved and each entry keeps its own tag.
                {
                    let object = owner.read().unwrap();
                    let tags: Vec<NameKeyType> = object
                        .modules
                        .iter()
                        .map(|entry| entry.module_tag_key())
                        .collect();
                    assert_eq!(tags, vec![1, 2]);
                    assert!(!object.reschedule_tagged_update(0, 99));
                    assert!(!object.reschedule_tagged_update(77, 99));
                }

                {
                    let object = owner.read().unwrap();
                    assert!(object.reschedule_tagged_update(1, 17));
                }
                {
                    let scheduler = ambient.lock().unwrap();
                    let (first_id, first_wake) = scheduler.sleepy_entry_for(&first).unwrap();
                    let (second_id, second_wake) = scheduler.sleepy_entry_for(&second).unwrap();
                    assert_eq!((first_id, first_wake), (id, 17));
                    assert_eq!(
                        (second_id, second_wake),
                        (id, 40),
                        "sibling registration keeps its object id and wake frame"
                    );
                }

                {
                    let object = owner.read().unwrap();
                    assert!(object.reschedule_tagged_update(2, 55));
                }
                {
                    let mut scheduler = ambient.lock().unwrap();
                    assert_eq!(scheduler.sleepy_entry_for(&first).unwrap().1, 17);
                    assert_eq!(scheduler.sleepy_entry_for(&second).unwrap().1, 55);
                    scheduler.unregister_update_module(id, first.clone());
                    scheduler.unregister_update_module(id, second.clone());
                }
            });
        });
    }
}
