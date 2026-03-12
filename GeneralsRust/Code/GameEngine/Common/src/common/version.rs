////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: version.rs //////////////////////////////////////////////////////
// Generals version number class
// Author: Matthew D. Campbell, November 2001

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::OnceLock;

/// The Version class formats the version number into integer and string
/// values for different parts of the game.
#[derive(Debug, Clone)]
pub struct Version {
    major: i32,
    minor: i32,
    build_num: i32,
    local_build_num: i32,
    build_location: String,
    build_user: String,
    build_time: String,
    build_date: String,
    show_full_version: bool,
}

impl Default for Version {
    fn default() -> Self {
        Self::new()
    }
}

impl Version {
    /// Create a new Version instance
    pub fn new() -> Self {
        Self {
            major: 1,
            minor: 0,
            build_num: 0,
            local_build_num: 0,
            build_user: "somebody".to_string(),
            build_location: "somewhere".to_string(),
            build_time: String::new(),
            build_date: String::new(),
            show_full_version: cfg!(any(feature = "debug", feature = "internal")),
        }
    }

    /// Set version information
    pub fn set_version(
        &mut self,
        major: i32,
        minor: i32,
        build_num: i32,
        local_build_num: i32,
        user: String,
        location: String,
        build_time: String,
        build_date: String,
    ) {
        self.major = major;
        self.minor = minor;
        self.build_num = build_num;
        self.local_build_num = local_build_num;
        self.build_user = user;
        self.build_location = location;
        self.build_time = build_time;
        self.build_date = build_date;
    }

    /// Return a 4-byte integer suitable for network protocols
    pub fn get_version_number(&self) -> u32 {
        ((self.major as u32) << 16) | (self.minor as u32)
    }

    /// Return a human-readable version number as ASCII string
    pub fn get_ascii_version(&self) -> String {
        #[cfg(any(feature = "debug", feature = "internal"))]
        {
            if self.local_build_num != 0 {
                format!(
                    "{}.{}.{}.{}{}{}",
                    self.major,
                    self.minor,
                    self.build_num,
                    self.local_build_num,
                    self.build_user.chars().nth(0).unwrap_or(' '),
                    self.build_user.chars().nth(1).unwrap_or(' ')
                )
            } else {
                format!("{}.{}.{}", self.major, self.minor, self.build_num)
            }
        }
        #[cfg(not(any(feature = "debug", feature = "internal")))]
        {
            format!("{}.{}", self.major, self.minor)
        }
    }

    /// Return a human-readable version number as Unicode string
    pub fn get_unicode_version(&self) -> String {
        #[allow(unused_mut)]
        let mut version = self.get_base_unicode_version();

        #[cfg(feature = "debug")]
        {
            version.push_str(" Debug");
        }

        #[cfg(feature = "internal")]
        {
            version.push_str(" Internal");
        }

        version
    }

    /// Return the full Unicode version (always shows full details)
    pub fn get_full_unicode_version(&self) -> String {
        #[allow(unused_mut)]
        let mut version = if self.local_build_num == 0 {
            format!("{}.{}.{}", self.major, self.minor, self.build_num)
        } else {
            format!(
                "{}.{}.{}.{}{}{}",
                self.major,
                self.minor,
                self.build_num,
                self.local_build_num,
                self.build_user.chars().nth(0).unwrap_or(' '),
                self.build_user.chars().nth(1).unwrap_or(' ')
            )
        };

        #[cfg(feature = "debug")]
        {
            version.push_str(" Debug");
        }

        #[cfg(feature = "internal")]
        {
            version.push_str(" Internal");
        }

        version
    }

    /// Get base Unicode version (without debug/internal suffix)
    fn get_base_unicode_version(&self) -> String {
        #[cfg(any(feature = "debug", feature = "internal"))]
        {
            if self.local_build_num == 0 {
                // Would use TheGameText->fetch("Version:Format3") in real implementation
                format!("{}.{}.{}", self.major, self.minor, self.build_num)
            } else {
                // Would use TheGameText->fetch("Version:Format4") in real implementation
                format!(
                    "{}.{}.{}.{}{}{}",
                    self.major,
                    self.minor,
                    self.build_num,
                    self.local_build_num,
                    self.build_user.chars().nth(0).unwrap_or(' '),
                    self.build_user.chars().nth(1).unwrap_or(' ')
                )
            }
        }
        #[cfg(not(any(feature = "debug", feature = "internal")))]
        {
            // Would use TheGameText->fetch("Version:Format2") in real implementation
            format!("{}.{}", self.major, self.minor)
        }
    }

    /// Return a formatted date/time string for build time
    pub fn get_ascii_build_time(&self) -> String {
        format!("{} {}", self.build_date, self.build_time)
    }

    /// Return a formatted date/time string for build time (Unicode)
    pub fn get_unicode_build_time(&self) -> String {
        // Would use TheGameText->fetch("Version:BuildTime") in real implementation
        format!("Built: {} {}", self.build_date, self.build_time)
    }

    /// Return the build location
    pub fn get_ascii_build_location(&self) -> String {
        self.build_location.clone()
    }

    /// Return the build location (Unicode)
    pub fn get_unicode_build_location(&self) -> String {
        // Would use TheGameText->fetch("Version:BuildMachine") in real implementation
        format!("Machine: {}", self.build_location)
    }

    /// Return the build user
    pub fn get_ascii_build_user(&self) -> String {
        self.build_user.clone()
    }

    /// Return the build user (Unicode)
    pub fn get_unicode_build_user(&self) -> String {
        // Would use TheGameText->fetch("Version:BuildUser") in real implementation
        format!("User: {}", self.build_user)
    }

    /// Check if full version should be shown
    pub fn show_full_version(&self) -> bool {
        self.show_full_version
    }

    /// Set whether full version should be shown
    pub fn set_show_full_version(&mut self, val: bool) {
        self.show_full_version = val;
    }

    /// Get major version
    pub fn get_major(&self) -> i32 {
        self.major
    }

    /// Get minor version
    pub fn get_minor(&self) -> i32 {
        self.minor
    }

    /// Get build number
    pub fn get_build_num(&self) -> i32 {
        self.build_num
    }

    /// Get local build number
    pub fn get_local_build_num(&self) -> i32 {
        self.local_build_num
    }

    /// Get build user
    pub fn get_build_user(&self) -> &str {
        &self.build_user
    }

    /// Get build location
    pub fn get_build_location(&self) -> &str {
        &self.build_location
    }

    /// Get build time
    pub fn get_build_time(&self) -> &str {
        &self.build_time
    }

    /// Get build date
    pub fn get_build_date(&self) -> &str {
        &self.build_date
    }
}

/// Global version instance stored in a thread-safe container.
static VERSION_INSTANCE: OnceLock<RwLock<Version>> = OnceLock::new();

fn version_lock() -> &'static RwLock<Version> {
    VERSION_INSTANCE.get_or_init(|| RwLock::new(Version::new()))
}

/// Initialize the global version instance with default values.
pub fn initialize_version() {
    let mut version = version_lock().write();
    *version = Version::new();
}

/// Initialize the global version instance with specific version info.
pub fn initialize_version_with_info(
    major: i32,
    minor: i32,
    build_num: i32,
    local_build_num: i32,
    user: String,
    location: String,
    build_time: String,
    build_date: String,
) {
    let mut version = version_lock().write();
    version.set_version(
        major,
        minor,
        build_num,
        local_build_num,
        user,
        location,
        build_time,
        build_date,
    );
}

/// Get read access to the global version instance.
pub fn get_version() -> RwLockReadGuard<'static, Version> {
    version_lock().read()
}

/// Get mutable access to the global version instance.
pub fn get_version_mut() -> RwLockWriteGuard<'static, Version> {
    version_lock().write()
}

/// Check if version is initialized.
pub fn is_version_initialized() -> bool {
    VERSION_INSTANCE.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_creation() {
        let version = Version::new();
        assert_eq!(version.get_major(), 1);
        assert_eq!(version.get_minor(), 0);
        assert_eq!(version.get_build_num(), 0);
    }

    #[test]
    fn test_version_number() {
        let mut version = Version::new();
        version.set_version(
            2,
            5,
            100,
            0,
            "test".to_string(),
            "test".to_string(),
            "".to_string(),
            "".to_string(),
        );
        assert_eq!(version.get_version_number(), (2 << 16) | 5);
    }

    #[test]
    fn test_ascii_version() {
        let mut version = Version::new();
        version.set_version(
            1,
            2,
            100,
            0,
            "test".to_string(),
            "test".to_string(),
            "".to_string(),
            "".to_string(),
        );

        #[cfg(any(feature = "debug", feature = "internal"))]
        {
            assert_eq!(version.get_ascii_version(), "1.2.100");
        }
        #[cfg(not(any(feature = "debug", feature = "internal")))]
        {
            assert_eq!(version.get_ascii_version(), "1.2");
        }
    }

    #[test]
    fn test_build_info() {
        let mut version = Version::new();
        version.set_version(
            1,
            0,
            0,
            0,
            "testuser".to_string(),
            "testlocation".to_string(),
            "12:00:00".to_string(),
            "2023-01-01".to_string(),
        );

        assert_eq!(version.get_ascii_build_time(), "2023-01-01 12:00:00");
        assert_eq!(version.get_build_user(), "testuser");
        assert_eq!(version.get_build_location(), "testlocation");
    }
}
