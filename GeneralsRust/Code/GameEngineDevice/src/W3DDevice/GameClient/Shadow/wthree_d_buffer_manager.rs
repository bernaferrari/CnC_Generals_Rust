//! WthreeDBufferManager Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DBufferManager.cpp
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
        // PARITY_NOTE: C++ W3DBufferManager.cpp:33 W3DBufferManager::W3DBufferManager
        // Initializes slot/buffer counters to 0 and NULLs out all VB/IB arrays.
        // The actual DX8 vertex/index buffer creation is deferred to allocateSlotStorage()
        // when a slot is requested. No device resources are created at init time.
        self.initialized = true;
        Ok(())
    }

    /// Shutdown the manager
    pub fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }
        // PARITY_NOTE: C++ W3DBufferManager.cpp:51 ~W3DBufferManager
        // Calls freeAllSlots() then freeAllBuffers().
        // freeAllSlots: unlinks all W3DVertexBufferSlot/W3DIndexBufferSlot from their VB/IB lists
        // freeAllBuffers: iterates all VB/IB linked lists, releases DX8VertexBufferClass/DX8IndexBufferClass
        // via REF_PTR_RELEASE, asserts all slots are freed first.
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
        let mgr = WthreeDBufferManager::new();
        assert!(!mgr.is_initialized());
        mgr.initialize().unwrap();
        assert!(mgr.is_initialized());
        mgr.shutdown();
        assert!(!mgr.is_initialized());
    }
}
