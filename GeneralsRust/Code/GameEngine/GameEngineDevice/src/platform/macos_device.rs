//! # macOS-specific device implementation
//!
//! Provides macOS-specific device operations and system integration.

use super::device_interface::{MemoryUsage, ThreadPriority};
use super::{PlatformError, Result};
#[cfg(feature = "libc")]
use std::ffi::CString;
use std::process::Command;

#[cfg(all(target_os = "macos", feature = "libc"))]
use libc;

/// macOS-specific device implementation
pub struct MacOsDevice {
    /// Process ID
    pid: u32,
}

#[cfg(target_os = "macos")]
impl MacOsDevice {
    /// Create a new macOS device
    pub async fn new() -> Result<Self> {
        Ok(Self {
            pid: std::process::id(),
        })
    }

    /// Set thread priority
    pub async fn set_thread_priority(&self, priority: ThreadPriority) -> Result<()> {
        // Use BSD setpriority system call (if libc available)
        #[cfg(feature = "libc")]
        unsafe {
            let nice_value = match priority {
                ThreadPriority::Lowest => 19,
                ThreadPriority::BelowNormal => 10,
                ThreadPriority::Normal => 0,
                ThreadPriority::AboveNormal => -10,
                ThreadPriority::Highest => -19,
                ThreadPriority::RealTime => -20,
            };
            if libc::setpriority(libc::PRIO_PROCESS, 0, nice_value) != 0 {
                return Err(PlatformError::SystemCallFailed(
                    "setpriority failed".to_string(),
                ));
            }
        }

        #[cfg(not(feature = "libc"))]
        {
            // Fallback: log the priority change without system call
            log::warn!(
                "Thread priority change requested but libc not available: {:?}",
                priority
            );
        }

        Ok(())
    }

    /// Get CPU usage
    pub async fn get_cpu_usage(&self) -> Result<f32> {
        let pid = self.pid.to_string();
        let output = Command::new("ps")
            .args(["-p", &pid, "-o", "%cpu="])
            .output()
            .map_err(|e| PlatformError::SystemCallFailed(format!("ps failed: {e}")))?;
        if !output.status.success() {
            return Err(PlatformError::SystemCallFailed(format!(
                "ps exited with status {}",
                output.status
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let cpu_percent = parse_ps_cpu_percent(&stdout).ok_or_else(|| {
            PlatformError::SystemCallFailed("failed to parse ps %cpu output".to_string())
        })?;
        Ok((cpu_percent / 100.0).clamp(0.0, 1.0))
    }

    /// Get memory usage
    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        // Use sysctl-backed page counters when available.
        #[cfg(feature = "libc")]
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;

        #[cfg(not(feature = "libc"))]
        let page_size = 4096u64;

        let total_memory = Self::get_sysctl_value("hw.memsize").unwrap_or(8 * 1024 * 1024 * 1024);
        let free_pages = Self::get_sysctl_value("vm.page_free_count").unwrap_or(0);
        let speculative_pages = Self::get_sysctl_value("vm.page_speculative_count").unwrap_or(0);
        let free_memory = free_pages
            .saturating_add(speculative_pages)
            .saturating_mul(page_size.max(1));
        let used_memory = total_memory.saturating_sub(free_memory.min(total_memory));

        Ok(MemoryUsage {
            physical_used: used_memory,
            physical_total: total_memory,
            virtual_used: used_memory,
            virtual_total: total_memory * 2,
        })
    }

    /// Get sysctl integer value by key.
    fn get_sysctl_value(name: &str) -> Option<u64> {
        #[cfg(feature = "libc")]
        {
            let c_name = CString::new(name).ok()?;
            let mut size: libc::size_t = 0;
            let probe = unsafe {
                libc::sysctlbyname(
                    c_name.as_ptr(),
                    std::ptr::null_mut(),
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if probe != 0 || size == 0 {
                return None;
            }

            let mut buffer = vec![0u8; size];
            let read = unsafe {
                libc::sysctlbyname(
                    c_name.as_ptr(),
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if read != 0 {
                return None;
            }

            return match size as usize {
                1 => Some(buffer[0] as u64),
                2 => Some(u16::from_ne_bytes(buffer[0..2].try_into().ok()?) as u64),
                4 => Some(u32::from_ne_bytes(buffer[0..4].try_into().ok()?) as u64),
                8 => Some(u64::from_ne_bytes(buffer[0..8].try_into().ok()?)),
                _ => None,
            };
        }

        #[cfg(not(feature = "libc"))]
        match name {
            "hw.memsize" => Some(16 * 1024 * 1024 * 1024), // 16GB default
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
fn parse_ps_cpu_percent(output: &str) -> Option<f32> {
    let token = output.split_whitespace().next()?;
    let normalized = token.replace(',', ".");
    normalized.parse::<f32>().ok()
}

// Stub implementation for non-macOS platforms
#[cfg(not(target_os = "macos"))]
impl MacOsDevice {
    pub async fn new() -> Result<Self> {
        Err(PlatformError::DeviceNotSupported(
            "MacOsDevice only supported on macOS".to_string(),
        ))
    }

    pub async fn set_thread_priority(&self, _priority: ThreadPriority) -> Result<()> {
        Err(PlatformError::DeviceNotSupported(
            "MacOsDevice only supported on macOS".to_string(),
        ))
    }

    pub async fn get_cpu_usage(&self) -> Result<f32> {
        Err(PlatformError::DeviceNotSupported(
            "MacOsDevice only supported on macOS".to_string(),
        ))
    }

    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        Err(PlatformError::DeviceNotSupported(
            "MacOsDevice only supported on macOS".to_string(),
        ))
    }
}

// For non-macOS platforms, we need libc stubs
#[cfg(not(target_os = "macos"))]
mod libc {
    pub const PRIO_PROCESS: i32 = 0;
    pub const _SC_PAGESIZE: i32 = 30;
    pub unsafe fn setpriority(_which: i32, _who: u32, _prio: i32) -> i32 {
        -1
    }
    pub unsafe fn sysconf(_name: i32) -> i64 {
        4096
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::parse_ps_cpu_percent;

    #[test]
    fn parse_ps_cpu_percent_accepts_standard_output() {
        assert_eq!(parse_ps_cpu_percent("12.5\n"), Some(12.5));
    }

    #[test]
    fn parse_ps_cpu_percent_handles_padding_and_locale_decimal() {
        assert_eq!(parse_ps_cpu_percent("  3,25  \n"), Some(3.25));
    }
}
