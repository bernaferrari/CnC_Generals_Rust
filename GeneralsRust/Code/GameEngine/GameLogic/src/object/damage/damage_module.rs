//! DamageModule - Base class for damage processing modules
//!
//! Original C++ location: GameLogic/Module/DamageModule.h/.cpp
//! Original C++ Author: Colin Day, September 2002
//! Rust conversion: 2025
//!
//! The C++ DamageModule base class only extends BehaviorModule and adds
//! version-tracked xfer.  No damage-specific virtuals exist at this level;
//! derived modules (like SlowDeathBehaviorModule) override xfer/loadPostProcess.

use crate::common::{ModuleData, NameKeyType};
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Base data for all damage modules (matches C++ DamageModuleData).
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
    /// CRC for damage module data (C++ DamageModule::crc).
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Serialize/deserialize damage module data (C++ DamageModule::xfer, version 1).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);
        Ok(())
    }

    /// Post-process after loading (C++ DamageModule::loadPostProcess).
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(DamageModuleData, module_tag_name_key);

/// Base struct for damage modules with common functionality (matches C++ DamageModule).
#[derive(Debug)]
pub struct DamageModule<T: ModuleData> {
    pub module_data: Arc<T>,
    pub object: Arc<RwLock<Object>>,
}

impl<T: ModuleData> DamageModule<T> {
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<T>) -> Self {
        Self {
            module_data,
            object,
        }
    }

    pub fn get_module_data(&self) -> &T {
        &self.module_data
    }

    pub fn get_object(&self) -> Arc<RwLock<Object>> {
        Arc::clone(&self.object)
    }
}
