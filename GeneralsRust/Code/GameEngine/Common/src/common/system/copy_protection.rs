////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Copy Protection System Implementation
//!
//! Mirrors the original launcher handshake used by Generals. The C++ game
//! validates a message supplied by the launcher through shared memory; debug and
//! internal builds skip that requirement.
//!
//! Rust conversion: 2025

use once_cell::sync::OnceCell;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::ascii_string::AsciiString;

pub const EXPECTED_LAUNCHER_MESSAGE: &str =
    "Play the \"Command & Conquer: Generals\" Multiplayer Test.";

/// Copy protection status codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProtectionStatus {
    Valid,
    InvalidDisc,
    NoDisc,
    CorruptedData,
    NetworkError,
    Unknown,
}

/// License validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub status: ProtectionStatus,
    pub error_message: AsciiString,
    pub validation_time: u64,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            status: ProtectionStatus::Unknown,
            error_message: AsciiString::new(),
            validation_time: 0,
        }
    }
}

/// Copy protection interface trait
pub trait CopyProtectionInterface {
    /// Initialize the copy protection system
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    /// Validate the current license/installation
    fn validate_license(&self) -> ValidationResult;

    /// Check if the required disc is present
    fn check_disc_presence(&self) -> bool;

    /// Validate disc authenticity
    fn validate_disc(&self) -> ProtectionStatus;

    /// Get protection system status
    fn get_status(&self) -> ProtectionStatus;

    /// Perform periodic validation check
    fn periodic_check(&mut self) -> ValidationResult;
}

/// C++ CopyProtect state.
pub struct CopyProtection {
    is_initialized: bool,
    last_check_time: u64,
    current_status: ProtectionStatus,
    check_interval: u64, // seconds
    protected_message: Option<AsciiString>,
}

impl Default for CopyProtection {
    fn default() -> Self {
        Self::new()
    }
}

impl CopyProtection {
    /// Create a new copy protection instance
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            last_check_time: 0,
            current_status: ProtectionStatus::Unknown,
            check_interval: 300, // 5 minutes
            protected_message: None,
        }
    }

    /// Get current system time in seconds since Unix epoch
    fn get_current_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn accept_launcher_message<S: Into<AsciiString>>(&mut self, message: S) {
        self.protected_message = Some(message.into());
        self.current_status = self.validate_launcher_message();
    }

    pub fn clear_launcher_message(&mut self) {
        self.protected_message = None;
        self.current_status = if self.is_initialized {
            self.validate_launcher_message()
        } else {
            ProtectionStatus::Unknown
        };
    }

    fn should_skip_launcher_validation() -> bool {
        cfg!(any(debug_assertions, feature = "internal"))
    }

    fn validate_launcher_message(&self) -> ProtectionStatus {
        if Self::should_skip_launcher_validation() {
            return ProtectionStatus::Valid;
        }

        let Some(message) = &self.protected_message else {
            return ProtectionStatus::NoDisc;
        };

        if message.as_str() == EXPECTED_LAUNCHER_MESSAGE {
            ProtectionStatus::Valid
        } else {
            ProtectionStatus::CorruptedData
        }
    }

    fn make_validation_result(&self, status: ProtectionStatus) -> ValidationResult {
        let error_message = match status {
            ProtectionStatus::Valid => AsciiString::new(),
            ProtectionStatus::Unknown => AsciiString::from("Protection system not initialized"),
            ProtectionStatus::NoDisc => AsciiString::from("Launcher message not received"),
            ProtectionStatus::CorruptedData => AsciiString::from("Launcher message mismatch"),
            ProtectionStatus::InvalidDisc => AsciiString::from("Launcher validation failed"),
            ProtectionStatus::NetworkError => AsciiString::from("Launcher communication failed"),
        };

        ValidationResult {
            status,
            error_message,
            validation_time: Self::get_current_time(),
        }
    }

    fn validate_disc_internal(&self) -> ProtectionStatus {
        // C++ CopyProtect validates the launcher-provided message, not optical
        // media sectors. Keep this method as a compatibility alias for existing
        // CD-check call sites.
        self.validate_launcher_message()
    }

    fn validate_license_internal(&self) -> ValidationResult {
        self.make_validation_result(self.validate_launcher_message())
    }

    pub fn is_launcher_running(&self) -> bool {
        if Self::should_skip_launcher_validation() {
            return true;
        }

        self.protected_message.is_some()
    }

    pub fn notify_launcher(&self) -> bool {
        // The original implementation signaled a Windows event and waited for a
        // 0xBEEF message. Rust does not own that launcher process; this reports
        // whether the expected data has already been supplied.
        self.is_launcher_running()
    }

    pub fn shutdown(&mut self) {
        self.protected_message = None;
        self.current_status = ProtectionStatus::Unknown;
        self.is_initialized = false;
    }

    pub fn validate(&self) -> bool {
        self.validate_launcher_message() == ProtectionStatus::Valid
    }
}

impl CopyProtectionInterface for CopyProtection {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_initialized = true;
        self.last_check_time = Self::get_current_time();
        self.current_status = self.validate_launcher_message();
        Ok(())
    }

    fn validate_license(&self) -> ValidationResult {
        if !self.is_initialized {
            return self.make_validation_result(ProtectionStatus::Unknown);
        }

        self.validate_license_internal()
    }

    fn check_disc_presence(&self) -> bool {
        // Compatibility for callers named after the old CD check. The C++ path
        // checked launcher state/message data, not a physical disc.
        self.is_launcher_running()
    }

    fn validate_disc(&self) -> ProtectionStatus {
        if !self.is_initialized {
            return ProtectionStatus::Unknown;
        }

        self.validate_disc_internal()
    }

    fn get_status(&self) -> ProtectionStatus {
        self.current_status
    }

    fn periodic_check(&mut self) -> ValidationResult {
        let current_time = Self::get_current_time();

        if current_time - self.last_check_time < self.check_interval {
            return self.make_validation_result(self.current_status);
        }

        self.last_check_time = current_time;
        let result = self.validate_license_internal();
        self.current_status = result.status;
        result
    }
}

/// Copy protection configuration
#[derive(Debug, Clone)]
pub struct ProtectionConfig {
    pub enable_disc_check: bool,
    pub enable_periodic_check: bool,
    pub check_interval_seconds: u64,
    pub max_validation_attempts: u32,
}

impl Default for ProtectionConfig {
    fn default() -> Self {
        Self {
            enable_disc_check: false,
            enable_periodic_check: true,
            check_interval_seconds: 300, // 5 minutes
            max_validation_attempts: 3,
        }
    }
}

/// Advanced copy protection manager
pub struct ProtectionManager {
    protection: Box<dyn CopyProtectionInterface + Send + Sync>,
    config: ProtectionConfig,
    validation_attempts: u32,
}

impl ProtectionManager {
    /// Create a new protection manager
    pub fn new(config: ProtectionConfig) -> Self {
        Self {
            protection: Box::new(CopyProtection::new()),
            config,
            validation_attempts: 0,
        }
    }

    /// Initialize the protection manager
    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.protection.init()
    }

    /// Perform a comprehensive validation
    pub fn comprehensive_validation(&mut self) -> ValidationResult {
        self.validation_attempts += 1;

        // Check disc if enabled
        if self.config.enable_disc_check {
            let disc_status = self.protection.validate_disc();
            if disc_status != ProtectionStatus::Valid {
                return ValidationResult {
                    status: disc_status,
                    error_message: AsciiString::from("Disc validation failed"),
                    validation_time: CopyProtection::get_current_time(),
                };
            }
        }

        // Validate license
        let result = self.protection.validate_license();

        // Reset attempts counter on successful validation
        if result.status == ProtectionStatus::Valid {
            self.validation_attempts = 0;
        }

        result
    }

    /// Check if maximum validation attempts exceeded
    pub fn is_max_attempts_exceeded(&self) -> bool {
        self.validation_attempts >= self.config.max_validation_attempts
    }

    /// Get current protection status
    pub fn get_status(&self) -> ProtectionStatus {
        self.protection.get_status()
    }
}

/// Global protection manager instance
static PROTECTION_MANAGER: OnceCell<Mutex<ProtectionManager>> = OnceCell::new();

/// Initialize the global protection manager
pub fn init_protection_manager(config: Option<ProtectionConfig>) {
    let config = config.unwrap_or_default();

    if PROTECTION_MANAGER.get().is_none() {
        let mut manager = ProtectionManager::new(config.clone());
        let _ = manager.init();
        let _ = PROTECTION_MANAGER.set(Mutex::new(manager));
    } else if let Some(cell) = PROTECTION_MANAGER.get() {
        if let Ok(mut guard) = cell.lock() {
            *guard = ProtectionManager::new(config);
            let _ = guard.init();
        }
    }
}

/// Get reference to the global protection manager
pub fn get_protection_manager() -> Option<MutexGuard<'static, ProtectionManager>> {
    PROTECTION_MANAGER
        .get()
        .map(|cell| cell.lock().expect("ProtectionManager mutex poisoned"))
}

/// Utility functions for copy protection
pub mod utils {
    /// Generate a simple checksum for data validation
    pub fn calculate_checksum(data: &[u8]) -> u32 {
        let mut checksum = 0u32;
        for &byte in data {
            checksum = checksum.wrapping_add(byte as u32);
        }
        checksum
    }

    /// Simple XOR obfuscation for sensitive data
    pub fn xor_obfuscate(data: &mut [u8], key: u8) {
        for byte in data {
            *byte ^= key;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_protection_creation() {
        let protection = CopyProtection::new();
        assert_eq!(protection.current_status, ProtectionStatus::Unknown);
        assert!(!protection.is_initialized);
    }

    #[test]
    fn test_copy_protection_init() {
        let mut protection = CopyProtection::new();
        assert!(protection.init().is_ok());
        assert!(protection.is_initialized);
        let expected = if CopyProtection::should_skip_launcher_validation() {
            ProtectionStatus::Valid
        } else {
            ProtectionStatus::NoDisc
        };
        assert_eq!(protection.get_status(), expected);
    }

    #[test]
    fn launcher_message_controls_validation_state() {
        let mut protection = CopyProtection::new();
        assert!(protection.init().is_ok());

        protection.accept_launcher_message(EXPECTED_LAUNCHER_MESSAGE);
        assert!(protection.validate());
        assert!(protection.notify_launcher());

        protection.clear_launcher_message();
        if CopyProtection::should_skip_launcher_validation() {
            assert!(protection.validate());
        } else {
            assert_eq!(protection.validate_disc(), ProtectionStatus::NoDisc);
            assert!(!protection.validate());
        }
    }

    #[test]
    fn test_validation_result() {
        let result = ValidationResult::default();
        assert_eq!(result.status, ProtectionStatus::Unknown);
        assert!(result.error_message.is_empty());
    }

    #[test]
    fn test_protection_manager() {
        let config = ProtectionConfig::default();
        let mut manager = ProtectionManager::new(config);
        assert!(manager.init().is_ok());

        let result = manager.comprehensive_validation();
        let expected = if CopyProtection::should_skip_launcher_validation() {
            ProtectionStatus::Valid
        } else {
            ProtectionStatus::NoDisc
        };
        assert_eq!(result.status, expected);
    }

    #[test]
    fn test_utils_checksum() {
        let data = b"Hello, World!";
        let checksum1 = utils::calculate_checksum(data);
        let checksum2 = utils::calculate_checksum(data);
        assert_eq!(checksum1, checksum2);

        let different_data = b"Hello, World?";
        let checksum3 = utils::calculate_checksum(different_data);
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_utils_xor_obfuscation() {
        let mut data = *b"Hello";
        let key = 0xAB;
        let original = data;

        utils::xor_obfuscate(&mut data, key);
        assert_ne!(data, original);

        utils::xor_obfuscate(&mut data, key); // XOR again to decrypt
        assert_eq!(data, original);
    }
}
