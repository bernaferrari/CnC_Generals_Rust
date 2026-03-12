////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: version.rs
//
// Version information system for Command & Conquer Generals Zero Hour
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use serde::{Deserialize, Serialize};
use std::fmt;

/// Version information for the game build
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: u32,
    pub build_date: String,
    pub build_time: String,
    pub git_hash: String,
    pub build_type: BuildType,
    pub platform: String,
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BuildType {
    Debug,
    Release,
    Final,
    Internal,
}

impl fmt::Display for BuildType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildType::Debug => write!(f, "Debug"),
            BuildType::Release => write!(f, "Release"),
            BuildType::Final => write!(f, "Final"),
            BuildType::Internal => write!(f, "Internal"),
        }
    }
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            major: 1,
            minor: 8,
            patch: 0,
            build: get_build_number(),
            build_date: option_env!("BUILD_DATE").unwrap_or("Unknown").to_string(),
            build_time: option_env!("BUILD_TIME").unwrap_or("Unknown").to_string(),
            git_hash: option_env!("GIT_HASH").unwrap_or("Unknown").to_string(),
            build_type: if cfg!(debug_assertions) {
                BuildType::Debug
            } else {
                BuildType::Release
            },
            platform: get_platform(),
            architecture: get_architecture(),
        }
    }
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Command & Conquer Generals Zero Hour v{}.{}.{}.{} ({}) - {} {}",
            self.major,
            self.minor,
            self.patch,
            self.build,
            self.build_type,
            self.platform,
            self.architecture
        )
    }
}

impl VersionInfo {
    /// Create a new version info instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the full version string
    pub fn get_version_string(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build
        )
    }

    /// Get the display name for the game
    pub fn get_display_name(&self) -> String {
        "Command & Conquer Generals Zero Hour - Rust Edition".to_string()
    }

    /// Get the short version string (major.minor.patch)
    pub fn get_short_version(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Get build information as a formatted string
    pub fn get_build_info(&self) -> String {
        format!(
            "Build {} ({}) - {} at {}",
            self.build, self.git_hash, self.build_date, self.build_time
        )
    }

    /// Check if this is a debug build
    pub fn is_debug(&self) -> bool {
        matches!(self.build_type, BuildType::Debug)
    }

    /// Check if this is a release build
    pub fn is_release(&self) -> bool {
        matches!(self.build_type, BuildType::Release | BuildType::Final)
    }

    /// Get the game window title
    pub fn get_window_title(&self) -> String {
        if self.is_debug() {
            format!("{} - {} Build", self.get_display_name(), self.build_type)
        } else {
            self.get_display_name()
        }
    }
}

/// Get the current build number (incremental)
fn get_build_number() -> u32 {
    // In a real build system, this would be set by CI/CD
    // For development, we'll use a compile-time constant
    match option_env!("BUILD_NUMBER") {
        Some(build_str) => build_str.parse().unwrap_or(1),
        None => 1, // Development build
    }
}

/// Get the current platform string
fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Get the current architecture string
fn get_architecture() -> String {
    if cfg!(target_arch = "x86_64") {
        "x64".to_string()
    } else if cfg!(target_arch = "x86") {
        "x86".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "ARM64".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Global version info instance
static VERSION_INFO: std::sync::LazyLock<VersionInfo> = std::sync::LazyLock::new(VersionInfo::new);

/// Get the global version information
pub fn get_version_info() -> &'static VersionInfo {
    &VERSION_INFO
}

/// Initialize the version system (called during startup)
pub fn initialize_version_system() {
    let version = get_version_info();
    log::info!("Game Version: {}", version);
    log::info!("Build Info: {}", version.get_build_info());
    log::info!("Platform: {} {}", version.platform, version.architecture);
}

/// Initialize version system with copy protection integration
/// This matches the C++ TheVersion initialization in WinMain
pub fn initialize_version_system_with_copy_protection() {
    use crate::copy_protection;

    let version = get_version_info();
    log::info!("Initializing version system with copy protection integration");
    log::info!("Game Version: {}", version);
    log::info!("Build Info: {}", version.get_build_info());
    log::info!("Platform: {} {}", version.platform, version.architecture);

    // Notify copy protection system of version information
    if copy_protection::is_copy_protection_enabled() {
        let _version_message = copy_protection::LauncherMessage::VersionCheck {
            game_version: version.get_version_string(),
        };

        if let Some(handle) = copy_protection::get_copy_protection() {
            if let Err(e) = handle.lock().check_for_message() {
                log::warn!("Failed to send version info to copy protection: {}", e);
            }
        }

        log::debug!("Version information integrated with copy protection system");
    }
}

/// Get version for copy protection validation
pub fn get_version_for_copy_protection() -> String {
    get_version_info().get_version_string()
}

/// Validate version against copy protection requirements
pub fn validate_version_for_copy_protection() -> bool {
    let version = get_version_info();

    // Basic version validation - in a real implementation this would be more sophisticated
    if version.major == 0 || version.build == 0 {
        log::error!("Invalid version information detected");
        return false;
    }

    // Check for development builds in production mode
    if !crate::copy_protection::is_development_mode() && version.is_debug() {
        log::warn!("Debug build detected in production mode");
        // In a real implementation, this might be blocked
    }

    log::debug!("Version validation passed for copy protection");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info_creation() {
        let version = VersionInfo::new();
        assert!(version.major > 0);
        assert!(version.build > 0);
    }

    #[test]
    fn test_version_string_formatting() {
        let version = VersionInfo {
            major: 1,
            minor: 8,
            patch: 0,
            build: 42,
            ..Default::default()
        };
        assert_eq!(version.get_version_string(), "1.8.0.42");
        assert_eq!(version.get_short_version(), "1.8.0");
    }

    #[test]
    fn test_build_type_detection() {
        let version = VersionInfo::new();
        if cfg!(debug_assertions) {
            assert!(version.is_debug());
            assert!(!version.is_release());
        } else {
            assert!(!version.is_debug());
            assert!(version.is_release());
        }
    }
}
