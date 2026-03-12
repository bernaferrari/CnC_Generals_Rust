// Linux-specific platform implementation

use anyhow::Result;

/// Initialize Linux-specific subsystems
pub fn initialize() -> Result<()> {
    log::info!("Initializing Linux platform");

    // Set up Linux-specific features
    #[cfg(target_os = "linux")]
    {
        // Linux automatically provides high-resolution timers via clock_gettime
    }

    Ok(())
}

/// Shutdown Linux-specific subsystems
pub fn shutdown() {
    log::info!("Shutting down Linux platform");

    #[cfg(target_os = "linux")]
    {
        // Clean up Linux-specific resources
    }
}

/// Get Linux-specific system information
pub fn get_system_info() -> String {
    #[cfg(target_os = "linux")]
    {
        // Read /etc/os-release for distribution info
        if let Ok(contents) = std::fs::read_to_string("/etc/os-release") {
            for line in contents.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    return line
                        .trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
        "Linux".to_string()
    }

    #[cfg(not(target_os = "linux"))]
    {
        "Unknown".to_string()
    }
}
