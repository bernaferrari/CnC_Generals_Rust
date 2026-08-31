//! # Linux-specific device implementation
//!
//! Provides Linux-specific device operations and system integration.

use super::{MemoryUsage, PlatformError, Result, ThreadPriority};

/// Linux-specific device implementation  
pub struct LinuxDevice {
    /// Process ID
    pid: u32,
}

#[cfg(target_os = "linux")]
impl LinuxDevice {
    /// Create a new Linux device
    pub async fn new() -> Result<Self> {
        Ok(Self {
            pid: std::process::id(),
        })
    }

    /// Set thread priority
    pub async fn set_thread_priority(&self, priority: ThreadPriority) -> Result<()> {
        let nice_value = match priority {
            ThreadPriority::Lowest => 19,
            ThreadPriority::BelowNormal => 10,
            ThreadPriority::Normal => 0,
            ThreadPriority::AboveNormal => -10,
            ThreadPriority::Highest => -19,
            ThreadPriority::RealTime => -20,
        };

        // Use libc to set process priority
        SAFETY: setpriority(PRIO_PROCESS, 0, nice) targets the calling process only;
        // SAFETY: by-value scalar arguments, no pointer dereference.
        unsafe {
            // SAFETY: setpriority(PRIO_PROCESS, 0, nice) targets the calling process only;
            // SAFETY: by-value scalar arguments, no pointer dereference.
            if libc::setpriority(libc::PRIO_PROCESS, 0, nice_value) != 0 {
                return Err(PlatformError::SystemCallFailed(
                    "setpriority failed".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Get CPU usage
    pub async fn get_cpu_usage(&self) -> Result<f32> {
        // Read from /proc/stat for CPU usage
        // Simplified implementation - real version would parse /proc/stat
        Ok(0.0)
    }

    /// Get memory usage
    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        // Read from /proc/meminfo
        match std::fs::read_to_string("/proc/meminfo") {
            Ok(contents) => {
                let mut total = 0u64;
                let mut available = 0u64;

                for line in contents.lines() {
                    if line.starts_with("MemTotal:") {
                        total = Self::parse_meminfo_value(line);
                    } else if line.starts_with("MemAvailable:") {
                        available = Self::parse_meminfo_value(line);
                    }
                }

                // Convert from KB to bytes
                total *= 1024;
                available *= 1024;
                let used = total - available;

                Ok(MemoryUsage {
                    physical_used: used,
                    physical_total: total,
                    virtual_used: used,       // Simplified
                    virtual_total: total * 2, // Simplified
                })
            }
            Err(_) => Err(PlatformError::SystemCallFailed(
                "Failed to read /proc/meminfo".to_string(),
            )),
        }
    }

    /// Parse memory value from /proc/meminfo line
    fn parse_meminfo_value(line: &str) -> u64 {
        line.split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
}

// Stub implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
impl LinuxDevice {
    pub async fn new() -> Result<Self> {
        Err(PlatformError::DeviceNotSupported(
            "LinuxDevice only supported on Linux".to_string(),
        ))
    }

    pub async fn set_thread_priority(&self, _priority: ThreadPriority) -> Result<()> {
        Err(PlatformError::DeviceNotSupported(
            "LinuxDevice only supported on Linux".to_string(),
        ))
    }

    pub async fn get_cpu_usage(&self) -> Result<f32> {
        Err(PlatformError::DeviceNotSupported(
            "LinuxDevice only supported on Linux".to_string(),
        ))
    }

    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        Err(PlatformError::DeviceNotSupported(
            "LinuxDevice only supported on Linux".to_string(),
        ))
    }
}

// For non-Linux platforms, we need libc stub
#[cfg(not(target_os = "linux"))]
mod libc {
    pub const PRIO_PROCESS: i32 = 0;
    // SAFETY: Non-native-platform stub mirroring the libc symbol signature; it
    // SAFETY: discards its arguments and returns a fixed failure value, so no
    // SAFETY: operation here can violate memory safety. The unsafe qualifier exists
    // SAFETY: only to match the real libc ABI for drop-in use.
    pub unsafe fn setpriority(_which: i32, _who: u32, _prio: i32) -> i32 {
        -1
    }
}
