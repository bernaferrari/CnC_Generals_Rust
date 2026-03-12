// Windows-specific platform implementation

use anyhow::Result;

/// Initialize Windows-specific subsystems
pub fn initialize() -> Result<()> {
    log::info!("Initializing Windows platform");

    // Set up Windows-specific features
    #[cfg(target_os = "windows")]
    {
        use winapi::um::timeapi;
        unsafe {
            // Request 1ms timer resolution for better frame timing
            timeapi::timeBeginPeriod(1);
        }
    }

    Ok(())
}

/// Shutdown Windows-specific subsystems
pub fn shutdown() {
    log::info!("Shutting down Windows platform");

    #[cfg(target_os = "windows")]
    {
        use winapi::um::timeapi;
        unsafe {
            // Restore default timer resolution
            timeapi::timeEndPeriod(1);
        }
    }
}

/// Get Windows-specific system information
pub fn get_system_info() -> String {
    "Windows".to_string()
}
