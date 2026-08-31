////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

// FILE: create_module.rs /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2001 (Converted to Rust)
// Desc: Object Create Module base classes and traits
///////////////////////////////////////////////////////////////////////////////////////////////////

use std::cell::Cell;
use std::ptr::NonNull;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::common::ObjectID;
use crate::helpers::TheGameLogic;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, ModuleData, Thing as ThingTrait,
};

thread_local! {
    static CREATE_OWNER: Cell<Option<(NonNull<Object>, ObjectID)>> = const { Cell::new(None) };
}

/// C++ `CreateModule::getObject()` is the same `Object*` already on the stack.
/// Live crate callers (`init_object` / `on_build_complete`) hold that write lock,
/// so modules must not `find_object_by_id` + `write()` again.
pub fn with_create_owner_object<R>(
    object: *mut Object,
    object_id: ObjectID,
    f: impl FnOnce() -> R,
) -> R {
    CREATE_OWNER.with(|cell| {
        let prev = cell.replace(NonNull::new(object).map(|ptr| (ptr, object_id)));
        let result = f();
        cell.set(prev);
        result
    })
}

/// Apply `f` to the object currently running create hooks, else try-write.
pub fn with_create_owner_mut(object_id: ObjectID, f: impl FnOnce(&mut Object)) {
    if object_id == 0 {
        return;
    }
    let owner = CREATE_OWNER.with(|cell| cell.get());
    if let Some((ptr, _)) = owner {
        // SAFETY: `with_create_owner_object` keeps `&mut Object` live on this thread.
        f(unsafe { &mut *ptr.as_ptr() });
        return;
    }
    let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
        return;
    };
    if let Ok(mut guard) = object_arc.try_write() {
        f(&mut guard);
    }
}

/// Id of the object currently running create hooks, when known.
///
/// The create caller already holds that object's write lock, so re-entrant
/// walks over player objects (C++ Player::onUpgradeCompleted fan-out) must
/// skip it; the init tail re-checks it (C++ Object::initObject tail).
pub fn create_owner_id() -> Option<ObjectID> {
    CREATE_OWNER.with(|cell| cell.get().map(|(_, id)| id))
}

/// Data structure for create modules
#[derive(Debug, Clone)]
pub struct CreateModuleData {
    pub base: BaseModuleData,
}

impl CreateModuleData {
    /// Create new create module data
    pub fn new() -> Self {
        Self {
            base: BaseModuleData::new(),
        }
    }
}

impl Default for CreateModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for CreateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for CreateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// Base create module implementation
#[derive(Debug)]
pub struct CreateModule {
    /// The thing this module is attached to
    thing: Arc<dyn ThingTrait>,

    /// Prevent multiple calling of on_build_complete
    need_to_run_on_build_complete: AtomicBool,
}

impl CreateModule {
    /// Create a new create module
    pub fn new(thing: Arc<dyn ThingTrait>) -> Self {
        Self {
            thing,
            need_to_run_on_build_complete: AtomicBool::new(true),
        }
    }

    /// Get reference to the associated thing
    pub fn get_thing(&self) -> &Arc<dyn ThingTrait> {
        &self.thing
    }

    /// Mark build-complete as handled (mirrors CreateModule::onBuildComplete)
    pub fn mark_build_complete(&self) {
        self.need_to_run_on_build_complete
            .store(false, Ordering::Release);
    }

    /// Whether should do on build complete (mirrors CreateModule::shouldDoOnBuildComplete)
    pub fn should_do_on_build_complete(&self) -> bool {
        self.need_to_run_on_build_complete.load(Ordering::Acquire)
    }
}

impl CreateInterface for CreateModule {
    /// Base implementation - should be overridden by subclasses
    fn on_create(&self) {}

    /// Called when build is complete
    fn on_build_complete(&self) {
        self.mark_build_complete();
    }

    /// Whether should do on build complete
    fn should_do_on_build_complete(&self) -> bool {
        self.should_do_on_build_complete()
    }
}

impl Snapshotable for CreateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::XferVersion = 1;
        let current_version: game_engine::common::system::XferVersion = 1;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|err| err.to_string())?;

        let mut need = self.need_to_run_on_build_complete.load(Ordering::Acquire);
        xfer.xfer_bool(&mut need).map_err(|err| err.to_string())?;
        self.need_to_run_on_build_complete
            .store(need, Ordering::Release);
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// Thread-safe wrapper for create modules (legacy tests)
pub type SafeCreateModule = Arc<dyn CreateInterface + Send + Sync>;

/// Create a thread-safe create module
pub fn create_safe_module(thing: Arc<dyn ThingTrait>) -> SafeCreateModule {
    Arc::new(CreateModule::new(thing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_module_creation() {
        // Test would need mock Thing implementation
        // let thing = Arc::new(Mutex::new(Thing::default()));
        // let module_data = CreateModuleData::new();
        // let module = CreateModule::new(thing);
        // assert!(module.should_do_on_build_complete());
    }

    #[test]
    fn test_build_complete_flag() {
        // Test would verify that the build complete flag works correctly
    }
}
