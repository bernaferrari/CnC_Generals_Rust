//! Destroy Modules - Rust conversion of C++ DestroyModule classes
//!
//! This module contains all destroy modules that handle object destruction behaviors.
//! Destroy modules control what happens when an object is explicitly destroyed - whether it
//! remains in the world, how it's removed, and what cleanup actions occur.
//!
//! Original C++ location: GameLogic/Module/DestroyModule.h and Object/Destroy/
//! Original C++ Author: Colin Day, September 2001
//! Rust conversion: 2025

pub mod destroy_module;
pub mod keep_object_die;

// Re-export all destroy modules for convenience
pub use destroy_module::*;
pub use keep_object_die::{KeepObjectDie, KeepObjectDieModuleData};

use crate::common::ModuleData as LegacyModuleData;
use crate::modules::DestroyModuleInterface;
use crate::object::Object;
use std::any::Any;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

/// Base trait for destroy module data
/// (matches C++ ModuleData base class for DestroyModule)
pub trait DestroyModuleData: LegacyModuleData + Debug {
    /// Downcast to Any for type inspection
    fn as_any(&self) -> &dyn Any;
}

/// Destroy module base trait
/// (matches C++ DestroyModule base class)
pub trait DestroyModule: Send + Sync + Debug {
    /// Called when object is being destroyed
    /// (matches C++ DestroyModuleInterface::onDestroy)
    fn on_destroy(&mut self, object: &mut Object);

    /// Get the destroy interface for this module
    fn get_destroy_interface(&self) -> Option<&dyn DestroyModuleInterface>;

    /// Get the destroy interface as mutable
    fn get_destroy_interface_mut(&mut self) -> Option<&mut dyn DestroyModuleInterface>;
}

/// Represents the result of a destroy operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestroyResult {
    /// Object successfully destroyed
    Success,
    /// Object destruction failed
    Failed,
    /// Object destruction deferred
    Deferred,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_destroy_result_values() {
        assert_eq!(DestroyResult::Success, DestroyResult::Success);
        assert_ne!(DestroyResult::Success, DestroyResult::Failed);
        assert_ne!(DestroyResult::Success, DestroyResult::Deferred);
    }
}
