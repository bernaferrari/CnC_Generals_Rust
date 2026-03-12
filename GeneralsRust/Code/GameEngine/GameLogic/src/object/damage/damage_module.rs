//! DamageModule - Base class for damage processing modules
//!
//! Original C++ location: GameLogic/Module/DamageModule.h/.cpp
//! Original C++ Author: Colin Day, September 2002
//! Rust conversion: 2025

use crate::common::{ModuleData, NameKeyType, XferExt};
use crate::damage::DamageInfo;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Base data for all damage modules
/// (Matches C++ DamageModuleData)
#[derive(Debug, Clone)]
pub struct DamageModuleData {
    pub module_tag_name_key: NameKeyType,
}

impl Default for DamageModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl Snapshotable for DamageModuleData {
    /// CRC calculation for damage module data
    /// (Matches C++ DamageModule::crc at line 14)
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Base implementation - extend in derived classes
        Ok(())
    }

    /// Serialize/deserialize damage module data
    /// (Matches C++ DamageModule::xfer at line 25)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version tracking
        let current_version: u32 = 1;

        // Xfer version
        if xfer.is_loading() {
            let version = xfer.xfer_version_read();
            if version > current_version {
                return Err(format!(
                    "DamageModule version {} > current version {}",
                    version, current_version
                ));
            }
        } else {
            xfer.xfer_version_write(current_version);
        }

        // No additional data in base class
        Ok(())
    }

    /// Post-process after loading
    /// (Matches C++ DamageModule::loadPostProcess at line 41)
    fn load_post_process(&mut self) -> Result<(), String> {
        // Base implementation - extend in derived classes
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(DamageModuleData, module_tag_name_key);

/// Base trait for all damage modules
/// (Matches C++ DamageModuleInterface)
pub trait DamageModuleInterface: Send + Sync + std::fmt::Debug {
    /// Called before damage is applied to the object
    /// Returns modified damage info or None to cancel damage
    fn on_damage_received(
        &mut self,
        object: &mut Object,
        damage_info: &mut DamageInfo,
    ) -> Option<DamageInfo>;

    /// Called after damage has been applied
    fn on_damage_applied(&mut self, object: &mut Object, damage_info: &DamageInfo);

    /// Get the module's priority for damage processing
    /// Lower numbers process first
    fn get_priority(&self) -> i32 {
        100 // Default priority
    }
}

/// Base struct for damage modules with common functionality
/// (Matches C++ DamageModule)
#[derive(Debug)]
pub struct DamageModule<T: ModuleData> {
    pub module_data: Arc<T>,
    pub object: Arc<RwLock<Object>>,
}

impl<T: ModuleData> DamageModule<T> {
    /// Create a new damage module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<T>) -> Self {
        Self {
            module_data,
            object,
        }
    }

    /// Get the module data
    pub fn get_module_data(&self) -> &T {
        &self.module_data
    }

    /// Get the object this module is attached to
    pub fn get_object(&self) -> Arc<RwLock<Object>> {
        Arc::clone(&self.object)
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
