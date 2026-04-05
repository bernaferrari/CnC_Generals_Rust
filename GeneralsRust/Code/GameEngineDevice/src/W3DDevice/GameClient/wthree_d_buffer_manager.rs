//! WthreeDBufferManager Module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DBufferManager.h
//!
//! This module provides resource management.

use std::{
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    ptr,
};

/// WthreeDBufferManager for managing resources
pub struct WthreeDBufferManager {
    /// Internal state
    initialized: bool,
    /// Managed resources
    resources: HashMap<String, *mut c_void>,
}

impl WthreeDBufferManager {
    /// Create a new WthreeDBufferManager
    pub fn new() -> Self {
        Self {
            initialized: false,
            resources: HashMap::new(),
        }
    }

    /// Initialize the manager
    pub fn initialize(&mut self) -> Result<(), WthreeDBufferManagerError> {
        if self.initialized {
            return Ok(());
        }
        // PARITY_NOTE: C++ W3DBufferManager (GameClient level) manages GPU vertex/index
        // buffer allocation for shadow geometry. init() is not explicitly defined in C++ -
        // the constructor zeros out slot/buffer arrays. Actual DX8 buffer creation happens
        // in allocateSlotStorage() on demand. See Shadow/wthree_d_buffer_manager.rs for
        // the shadow-specific buffer manager with detailed C++ reference.
        self.initialized = true;
        Ok(())
    }

    /// Shutdown the manager
    pub fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }
        // PARITY_NOTE: C++ destructor calls freeAllSlots() then freeAllBuffers().
        // See Shadow/wthree_d_buffer_manager.rs for detailed C++ reference.
        self.resources.clear();
        self.initialized = false;
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for WthreeDBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WthreeDBufferManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Error types for WthreeDBufferManager
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WthreeDBufferManagerError {
    /// Not initialized
    NotInitialized,
    /// Resource not found
    ResourceNotFound,
    /// Unknown error
    Unknown,
}

impl std::fmt::Display for WthreeDBufferManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WthreeDBufferManagerError::NotInitialized => write!(f, "Manager not initialized"),
            WthreeDBufferManagerError::ResourceNotFound => write!(f, "Resource not found"),
            WthreeDBufferManagerError::Unknown => write!(f, "Unknown manager error"),
        }
    }
}

impl std::error::Error for WthreeDBufferManagerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_buffer_manager_basic() {
        let mut mgr = WthreeDBufferManager::new();
        assert!(!mgr.is_initialized());
        mgr.initialize().unwrap();
        assert!(mgr.is_initialized());
        mgr.shutdown();
        assert!(!mgr.is_initialized());
    }
}
