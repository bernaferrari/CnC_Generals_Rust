//! KeepObjectDie - Die module that leaves the object as wreckage
//!
//! Original C++ location: GameLogic/Module/KeepObjectDie.h/.cpp
//! Original C++ Author: Kris Morness, November 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::damage::DamageInfo;
use crate::object::Object;
use std::sync::{Arc, RwLock};

/// KeepObjectDie - Keeps the object in the world as a corpse/wreckage
///
/// This die module does not remove the object from the game world.
/// Instead, it leaves it in place as wreckage or a corpse. This is used
/// for civilian buildings that don't have garrison contains, allowing them
/// to leave rubble without the DestroyDie module removing them entirely.
/// (Matches C++ KeepObjectDie)
#[derive(Debug)]
pub struct KeepObjectDie {
    base: DieModule<DieModuleData>,
}

impl KeepObjectDie {
    /// Create a new KeepObjectDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<DieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "KeepObjectDie"
    }
}

impl DieModuleInterface for KeepObjectDie {
    /// Called when the object dies - keeps the object in place
    /// (Matches C++ KeepObjectDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(object, damage_info, &self.base.module_data.die_mux_data) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keep_object_die_module_name() {
        assert_eq!(KeepObjectDie::get_module_name(), "KeepObjectDie");
    }
}
