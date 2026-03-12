//! DestroyDie - Default die module that removes the object
//!
//! Original C++ location: GameLogic/Module/DestroyDie.h/.cpp
//! Original C++ Author: Colin Day, November 2001
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::object::Object;
use std::sync::{Arc, RwLock};

/// DestroyDie - Removes the object from the game world
///
/// This is the default die module. It simply destroys the object,
/// removing it from the game completely without leaving any wreckage.
/// (Matches C++ DestroyDie)
#[derive(Debug)]
pub struct DestroyDie {
    base: DieModule<DieModuleData>,
}

impl DestroyDie {
    /// Create a new DestroyDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<DieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "DestroyDie"
    }
}

impl DieModuleInterface for DestroyDie {
    /// Called when the object dies - destroys the object
    /// (Matches C++ DestroyDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(object, damage_info, &self.base.module_data.die_mux_data) {
            return;
        }

        // Destroy the object (matches C++ DestroyDie.cpp: TheGameLogic->destroyObject(obj))
        let _ = TheGameLogic::destroy_object(object);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage::{DamageInfo, DamageInfoInput, DamageInfoOutput, DamageType, DeathType};

    #[test]
    fn test_destroy_die_creation() {
        assert_eq!(DestroyDie::get_module_name(), "DestroyDie");
    }

    #[test]
    fn test_destroy_die_module_name() {
        assert_eq!(DestroyDie::get_module_name(), "DestroyDie");
    }
}
