//! POWTruckBehavior - Rust conversion of C++ POWTruckBehavior class.
//!
//! This behavior wraps OpenContain and auto-loads surrendered infantry on collision,
//! delegating prisoner handling to POWTruckAIUpdate.

use std::any::Any;
use std::borrow::Cow;
use std::sync::{Arc, RwLock, Weak};

use game_engine::common::ini::{INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{GameError, INVALID_ID, LegacyModuleData, ObjectID};
use crate::helpers::TheGameLogic;
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, ContainModuleInterface, ContainWant,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::Object;
use crate::object::collide::{COLLISION_MANAGER, Coord3D as CollideCoord3D, LegacyCollideAdapter};
use crate::object::contain::{OpenContain, OpenContainModuleData};
use log::warn;

/// Wave 366: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

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
        object_id: ObjectID,
        module_data: Arc<POWTruckBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let contain = OpenContain::new(Arc::downgrade(&object), &module_data.base)?;
        Ok(Self {
            object_id: object_id,
            contain,
        })
    }

    fn get_object_id(&self) -> crate::common::ObjectID {
        self.object_id
    }

    fn with_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        // Wave 366: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(id, f)
    }

    fn get_object(&self) -> Option<ObjectID> {
        // Wave 366: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    fn load_surrendered_prisoner(
        &mut self,
        prisoner_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::load_prisoner_via_object(self.get_object_id(), prisoner_id)
    }

    /// C++ POWTruckBehavior::loadSurrenderedPrisoner resolved through the
    /// owning object's AI update. Owner-id based so the collision adapter can
    /// run it without borrowing this behavior.
    fn load_prisoner_via_object(
        owner_id: ObjectID,
        prisoner_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 366: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() || owner_id == crate::common::INVALID_ID {
            return Ok(());
        }

        crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |guard| {
            if let Some(ai) = guard.get_ai_update_interface_mut() {
                if let Some(pow_ai) = ai.get_pow_truck_ai_update_interface() {
                    pow_ai.load_prisoner(prisoner_id);
                }
            }
        });
        Ok(())
    }

    /// C++ POWTruckBehavior::onCollision body, owner-resolved so the collision
    /// adapter needs no borrow of this behavior.
    fn on_collision_for_owner(object_id: ObjectID, other_id: ObjectID) {
        // Wave 366: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

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
            .map(|ai| ai.is_surrendered())
            .unwrap_or(false);

        if surrendered {
            let _ = Self::load_prisoner_via_object(object_id, other_id);
        }
    }

    /// C++ OpenContain::onDelete via POWTruckBehavior contain.
    pub fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.on_delete().map_err(|e| e.into())
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
        Self::on_collision_for_owner(object_id, other_id);
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

    fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> {
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
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 366: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.contain.on_containing(obj_id, was_selected)
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 366: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.contain.on_removing(obj_id)
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

    fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        POWTruckBehavior::on_delete(self)
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
}

#[cfg(feature = "allow_surrender")]
impl POWTruckCollideAdapter {
    fn new(owner_id: ObjectID) -> Self {
        Self { owner_id }
    }
}

#[cfg(feature = "allow_surrender")]
impl LegacyCollideAdapter for POWTruckCollideAdapter {
    fn legacy_on_collide(
        &mut self,
        other_id: crate::common::ObjectID,
        _loc: &CollideCoord3D,
        _normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let other_id = other_id;
        POWTruckBehavior::on_collision_for_owner(self.owner_id, other_id);
        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other_id: crate::common::ObjectID,
    ) -> Result<bool, GameError> {
        // Wave 366: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let Some(other) = crate::helpers::TheGameLogic::find_object_by_id(other_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(other_id))
        else {
            return Ok(false);
        };
        let surrendered = other
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .map(|ai| ai.is_surrendered())
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
    /// Owned contain behavior. `contain_handle()` hands it to the Object's
    /// contain slot during construction (C++ Object::setContain binding);
    /// the module entry keeps nothing afterwards.
    behavior: Option<Box<POWTruckBehavior>>,
    owner_id: ObjectID,
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
        let owner_id = behavior.object_id;
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior: Some(Box::new(behavior)),
            owner_id,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> Option<&mut POWTruckBehavior> {
        self.behavior.as_deref_mut()
    }

    /// C++ Object.cpp contain binding (Object::setContain for the POW truck):
    /// the Object's contain slot owns the behavior instance after this
    /// handover. Take semantics — invoked once during construction.
    pub fn contain_handle(&mut self) -> Option<Box<dyn ContainModuleInterface>> {
        self.behavior
            .take()
            .map(|behavior| behavior as Box<dyn ContainModuleInterface>)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for POWTruckBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        match self.behavior.as_deref() {
            Some(behavior) => Snapshotable::crc(&behavior.contain, xfer),
            None => Ok(()),
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        match self.behavior.as_deref_mut() {
            Some(behavior) => Snapshotable::xfer(&mut behavior.contain, xfer),
            None => Ok(()),
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        match self.behavior.as_deref_mut() {
            Some(behavior) => Snapshotable::load_post_process(&mut behavior.contain),
            None => Ok(()),
        }
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
        let object_id = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |guard| guard.get_id())
            .unwrap_or(INVALID_ID);

        if object_id == INVALID_ID {
            return;
        }

        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            object_id,
            Box::new(POWTruckCollideAdapter::new(object_id)),
        ) {
            warn!("POWTruckBehavior collision registration failed: {err}");
        }
    }

    fn on_delete(&mut self) {
        if let Some(behavior) = self.behavior.as_deref_mut() {
            let _ = behavior.on_delete();
        }
        let object_id = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |guard| guard.get_id())
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

#[cfg(all(test, feature = "allow_surrender"))]
mod tests {
    use super::*;

    #[test]
    fn contain_handle_hands_over_open_contain_riders() {
        // C++ POWTruckAIUpdate.cpp:766 iterateContained(putPrisonersInPrison)
        // walks the truck OpenContain list; the handed-over box owns that list.
        let contain = OpenContain::new(
            std::sync::Weak::<RwLock<Object>>::new(),
            &OpenContainModuleData::default(),
        )
        .expect("open contain");
        let behavior = POWTruckBehavior {
            object_id: crate::common::INVALID_ID,
            contain,
        };
        let mut module = POWTruckBehaviorModule::new(
            behavior,
            &AsciiString::from("POWTruckBehavior"),
            Arc::new(POWTruckBehaviorModuleData::default()),
        );
        module
            .behavior_mut()
            .expect("behavior")
            .contain
            .add_to_contain_list_id(77001, false)
            .expect("add rider");

        let contain = module.contain_handle().expect("contain handle");
        let riders = contain.get_contained_objects();

        // Object.contain now owns the rider list; the module entry keeps nothing.
        assert_eq!(riders.as_ref(), &[77001]);
        assert!(module.contain_handle().is_none());
    }
}
