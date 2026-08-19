//! WthreeDDisplayStringManager Module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DDisplayStringManager.h
//!
//! This module provides resource management.

use std::{
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    ptr,
};

/// WthreeDDisplayStringManager for managing resources
pub struct WthreeDDisplayStringManager {
    /// Internal state
    initialized: bool,
    /// Managed resources
    resources: HashMap<String, *mut c_void>,
}

impl WthreeDDisplayStringManager {
    /// Create a new WthreeDDisplayStringManager
    pub fn new() -> Self {
        Self {
            initialized: false,
            resources: HashMap::new(),
        }
    }

    pub fn initialize(&mut self) -> Result<(), WthreeDDisplayStringManagerError> {
        if self.initialized {
            return Ok(());
        }
        self.post_process_load();
        self.initialized = true;
        Ok(())
    }

    /// C++ `W3DDisplayStringManager::postProcessLoad`.
    pub fn post_process_load(&mut self) {
        for numeral in 0..10 {
            let key = format!("NUMBER:{numeral}");
            self.resources
                .insert(key, std::ptr::null_mut());
        }
        self.resources
            .insert("LABEL:FORMATION".to_string(), std::ptr::null_mut());
        let _ = game_client_rust::display_string_manager::get_group_numeral_string(0);
        let _ = game_client_rust::display_string_manager::get_formation_letter_string();
    }

    /// Shutdown the manager
    pub fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }
        // PARITY_NOTE: C++ W3DDisplayStringManager::~W3DDisplayStringManager frees all
        // DisplayString objects. Each DisplayString holds font, text, color, position data.
        // Full port requires: DisplayString deallocation, font resource cleanup.
        self.resources.clear();
        self.initialized = false;
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for WthreeDDisplayStringManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WthreeDDisplayStringManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Error types for WthreeDDisplayStringManager
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WthreeDDisplayStringManagerError {
    /// Not initialized
    NotInitialized,
    /// Resource not found
    ResourceNotFound,
    /// Unknown error
    Unknown,
}

impl std::fmt::Display for WthreeDDisplayStringManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WthreeDDisplayStringManagerError::NotInitialized => {
                write!(f, "Manager not initialized")
            }
            WthreeDDisplayStringManagerError::ResourceNotFound => write!(f, "Resource not found"),
            WthreeDDisplayStringManagerError::Unknown => write!(f, "Unknown manager error"),
        }
    }
}

impl std::error::Error for WthreeDDisplayStringManagerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_display_string_manager_basic() {
        let mut mgr = WthreeDDisplayStringManager::new();
        assert!(!mgr.is_initialized());
        mgr.initialize().unwrap();
        assert!(mgr.is_initialized());
        mgr.shutdown();
        assert!(!mgr.is_initialized());
    }
}
