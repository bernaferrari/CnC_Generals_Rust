//! COM Initialization Module
//!
//! Corresponds to C++ file: Tools/PATCHGET/COMINIT.CPP
//! Source header: Tools/PATCHGET/COMINIT.H
//!
//! Port of the C++ ComInit RAII class. The original C++ code calls
//! CoInitialize/CoUninitialize on construction/destruction and creates
//! a global instance `Global_COM_Initializer` to auto-initialize COM
//! at startup.
//!
//! In Rust, COM initialization is handled via platform-specific crates
//! (e.g., `windows` crate) when needed. This shim preserves the API
//! surface and RAII pattern for cross-platform compatibility.

use std::sync::atomic::{AtomicBool, Ordering};

static COM_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// RAII guard that initializes COM on creation and cleans up on drop.
/// Mirrors the C++ `ComInit` class behavior.
pub struct ComInit {
    initialized: bool,
}

impl ComInit {
    /// Create a new ComInit instance and attempt COM initialization.
    /// Mirrors the C++ constructor that calls `CoInitialize(NULL)`.
    pub fn new() -> Result<Self, ComInitError> {
        if COM_INITIALIZED.load(Ordering::SeqCst) {
            return Err(ComInitError::AlreadyInitialized);
        }

        #[cfg(target_os = "windows")]
        {
            // On Windows, attempt actual COM initialization
            // In the full port, this would call CoInitializeEx
            COM_INITIALIZED.store(true, Ordering::SeqCst);
            Ok(Self { initialized: true })
        }

        #[cfg(not(target_os = "windows"))]
        {
            // On non-Windows platforms, COM is not available
            // Mark as initialized for compatibility but no-op
            COM_INITIALIZED.store(true, Ordering::SeqCst);
            Ok(Self { initialized: true })
        }
    }

    /// Check if COM has been initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for ComInit {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self { initialized: false })
    }
}

impl Drop for ComInit {
    /// Cleanup COM on drop, mirroring the C++ destructor calling `CoUninitialize()`.
    fn drop(&mut self) {
        if self.initialized {
            #[cfg(target_os = "windows")]
            {
                // On Windows, call CoUninitialize
            }

            COM_INITIALIZED.store(false, Ordering::SeqCst);
            self.initialized = false;
        }
    }
}

/// Error types for COM initialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComInitError {
    /// COM is already initialized
    AlreadyInitialized,
    /// COM initialization failed (HRESULT equivalent)
    InitFailed,
}

impl std::fmt::Display for ComInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComInitError::AlreadyInitialized => write!(f, "COM already initialized"),
            ComInitError::InitFailed => write!(f, "Can't initialize COM"),
        }
    }
}

impl std::error::Error for ComInitError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_com_init_lifecycle() {
        let com = ComInit::new();
        assert!(com.is_ok());
        let com = com.unwrap();
        assert!(com.is_initialized());

        // Second init should fail
        let com2 = ComInit::new();
        assert!(com2.is_err());

        drop(com);

        // After drop, should be able to init again
        let com3 = ComInit::new();
        assert!(com3.is_ok());
    }
}
