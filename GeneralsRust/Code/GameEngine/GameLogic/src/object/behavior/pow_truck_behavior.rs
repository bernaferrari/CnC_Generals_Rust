//! POWTruckBehavior - Rust conversion of C++ POWTruckBehavior class.
//!
//! This behavior wraps OpenContain and auto-loads surrendered infantry on collision,
//! delegating prisoner handling to POWTruckAIUpdate.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock, Weak};

use game_engine::common::ini::{INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{GameError, LegacyModuleData, ObjectID, INVALID_ID};
use crate::helpers::TheGameLogic;
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, ContainModuleInterface, ContainWant,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::collide::{Coord3D as CollideCoord3D, LegacyCollideAdapter, COLLISION_MANAGER};
use crate::object::contain::{OpenContain, OpenContainModuleData};
use crate::object::Object;
use log::warn;

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
pub struct POWTruckBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    pub base: OpenContainModuleData,
}

#[cfg(feature = "allow_surrender")]
impl Default for POWTruckBehaviorModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: OpenContainModuleData::default(),
        }
    }
}

#[cfg(feature = "allow_surrender")]
impl POWTruckBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for POWTruckBehaviorModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.base.crc(xfer)?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
crate::impl_legacy_module_data_with_key_field!(POWTruckBehaviorModuleData, module_tag_name_key);

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct POWTruckBehavior {
    object_id: ObjectID,
    contain: OpenContain,
}

#[cfg(feature = "allow_surrender")]
impl POWTruckBehavior {
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<POWTruckBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let contain = OpenContain::new(Arc::downgrade(&object), &module_data.base)?;
        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            contain,
        })
    }

    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })
    }

    fn load_surrendered_prisoner(
        &mut self,
        prisoner_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(owner) = self.get_object() else {
            return Ok(());
        };

        let Some(ai_handle) = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        else {
            return Ok(());
        };

        let mut ai_guard = ai_handle
            .lock()
            .map_err(|_| "POWTruckBehavior AI lock poisoned")?;
        let Some(pow_ai) = ai_guard.get_pow_truck_ai_update_interface() else {
            return Ok(());
        };

        pow_ai.load_prisoner(prisoner_id);
        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
impl UpdateModuleInterface for POWTruckBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.contain.update()
    }
}

#[cfg(feature = "allow_surrender")]
impl CollideModuleInterface for POWTruckBehavior {
    fn on_collision(&mut self, object_id: ObjectID, other_id: ObjectID) {
        if object_id == other_id {
            return;
        }

        let Some(other) = TheGameLogic::find_object_by_id(other_id) else {
            return;
        };

        let surrendered = other
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .and_then(|ai| ai.lock().ok().map(|ai| ai.is_surrendered()))
            .unwrap_or(false);

        if surrendered {
            let _ = self.load_surrendered_prisoner(other_id);
        }
    }
}

#[cfg(feature = "allow_surrender")]
impl ContainModuleInterface for POWTruckBehavior {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        self.contain.can_contain(object_id)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.contain.contain_object(object_id)
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.contain.release_object(object_id)
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        self.contain.get_contained_objects()
    }

    fn get_contained_count(&self) -> usize {
        self.contain.get_contained_count()
    }

    fn get_max_capacity(&self) -> usize {
        self.contain.get_max_capacity()
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        self.contain.is_enclosing_container_for(obj)
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.contain.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain
            .contain_object(obj.get_id())
            .map_err(|err| err.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
    }

    fn is_garrisonable(&self) -> bool {
        self.contain.is_garrisonable()
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        self.contain.is_passenger_allowed_to_fire(id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.contain.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.contain.set_passenger_allowed_to_fire(allowed);
    }

    fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.on_containing(obj, was_selected)
    }

    fn on_removing(
        &mut self,
        obj: Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.on_removing(obj)
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.remove_all_contained(expose_stealth)
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        self.contain.is_displayed_on_control_bar()
    }

    fn is_kick_out_on_capture(&self) -> bool {
        self.contain.is_kick_out_on_capture()
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.client_visible_contained_flash_as_selected()
    }

    fn get_contain_count(&self) -> u32 {
        self.contain.get_contain_count()
    }

    fn get_contain_max(&self) -> i32 {
        self.contain.get_contain_max()
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        self.contain.friend_get_rider()
    }
}

#[cfg(feature = "allow_surrender")]
impl BehaviorModuleInterface for POWTruckBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_collide(&mut self) -> Option<&mut dyn CollideModuleInterface> {
        Some(self)
    }

    fn get_contain(&mut self) -> Option<&mut dyn ContainModuleInterface> {
        Some(self)
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
struct POWTruckCollideAdapter {
    owner_id: ObjectID,
    behavior: Arc<Mutex<POWTruckBehavior>>,
}

#[cfg(feature = "allow_surrender")]
impl POWTruckCollideAdapter {
    fn new(owner_id: ObjectID, behavior: Arc<Mutex<POWTruckBehavior>>) -> Self {
        Self { owner_id, behavior }
    }
}

#[cfg(feature = "allow_surrender")]
impl LegacyCollideAdapter for POWTruckCollideAdapter {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        _loc: &CollideCoord3D,
        _normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let other_id = other.read().map(|guard| guard.get_id()).unwrap_or_default();
        if let Ok(mut guard) = self.behavior.lock() {
            guard.on_collision(self.owner_id, other_id);
        }
        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        let surrendered = other
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .and_then(|ai| ai.lock().ok().map(|ai| ai.is_surrendered()))
            .unwrap_or(false);

        if !surrendered {
            return Ok(false);
        }

        Ok(surrendered)
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct POWTruckBehaviorModule {
    behavior: Arc<Mutex<POWTruckBehavior>>,
    module_name_key: NameKeyType,
    module_data: Arc<POWTruckBehaviorModuleData>,
}

#[cfg(feature = "allow_surrender")]
impl POWTruckBehaviorModule {
    pub fn new(
        behavior: POWTruckBehavior,
        module_name: &AsciiString,
        module_data: Arc<POWTruckBehaviorModuleData>,
    ) -> Self {
        let behavior_arc = Arc::new(Mutex::new(behavior));
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior: behavior_arc,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> Option<std::sync::MutexGuard<'_, POWTruckBehavior>> {
        self.behavior.lock().ok()
    }

    pub fn contain_handle(&self) -> Arc<Mutex<dyn ContainModuleInterface>> {
        Arc::new(Mutex::new(POWTruckBehaviorContainHandle {
            behavior: Arc::clone(&self.behavior),
        }))
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
struct POWTruckBehaviorContainHandle {
    behavior: Arc<Mutex<POWTruckBehavior>>,
}

#[cfg(feature = "allow_surrender")]
impl ContainModuleInterface for POWTruckBehaviorContainHandle {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        self.behavior
            .lock()
            .map(|guard| guard.can_contain(object_id))
            .unwrap_or(false)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.behavior
            .lock()
            .map_err(|_| "POWTruckBehaviorContainHandle lock poisoned".to_string())?
            .contain_object(object_id)
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.behavior
            .lock()
            .map_err(|_| "POWTruckBehaviorContainHandle lock poisoned".to_string())?
            .release_object(object_id)
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        &[]
    }

    fn get_contained_count(&self) -> usize {
        self.behavior
            .lock()
            .map(|guard| guard.get_contained_count())
            .unwrap_or(0)
    }

    fn get_max_capacity(&self) -> usize {
        self.behavior
            .lock()
            .map(|guard| guard.get_max_capacity())
            .unwrap_or(0)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for POWTruckBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let guard = self
            .behavior
            .lock()
            .map_err(|_| "POWTruckBehaviorModule lock poisoned".to_string())?;
        Snapshotable::crc(&guard.contain, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut guard = self
            .behavior
            .lock()
            .map_err(|_| "POWTruckBehaviorModule lock poisoned".to_string())?;
        Snapshotable::xfer(&mut guard.contain, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut behavior = self
            .behavior
            .lock()
            .map_err(|_| "POWTruckBehaviorModule lock poisoned".to_string())?;
        Snapshotable::load_post_process(&mut behavior.contain)
    }
}

#[cfg(feature = "allow_surrender")]
impl Module for POWTruckBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        ModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        let object_id = self
            .behavior
            .lock()
            .ok()
            .and_then(|behavior| behavior.get_object())
            .and_then(|object| object.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(INVALID_ID);

        if object_id == INVALID_ID {
            return;
        }

        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            object_id,
            Box::new(POWTruckCollideAdapter::new(
                object_id,
                Arc::clone(&self.behavior),
            )),
        ) {
            warn!("POWTruckBehavior collision registration failed: {err}");
        }
    }

    fn on_delete(&mut self) {
        let object_id = self
            .behavior
            .lock()
            .ok()
            .and_then(|behavior| behavior.get_object())
            .and_then(|object| object.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(INVALID_ID);

        if object_id != INVALID_ID {
            let _ = COLLISION_MANAGER.unregister_object(object_id);
        }
    }
}

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default, Clone)]
pub struct POWTruckBehaviorModuleData;

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default)]
pub struct POWTruckBehavior;

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default)]
pub struct POWTruckBehaviorModule;
