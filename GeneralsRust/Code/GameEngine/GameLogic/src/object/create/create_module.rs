////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

// FILE: create_module.rs /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2001 (Converted to Rust)
// Desc: Object Create Module base classes and traits
///////////////////////////////////////////////////////////////////////////////////////////////////

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, ModuleData, Thing as ThingTrait,
};

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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
