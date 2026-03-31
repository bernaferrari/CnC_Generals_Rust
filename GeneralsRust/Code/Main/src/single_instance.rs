////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: single_instance.rs
//
// Single instance protection system
// Prevents multiple instances of the game from running simultaneously
// Cross-platform implementation using file locks
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::{Context, Result};
use log::{error, info, warn};
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

/// Single instance guard that prevents multiple game instances
pub struct SingleInstanceGuard {
    lock_file_path: PathBuf,
    #[allow(dead_code)] // Kept alive to hold file lock for single-instance enforcement
    lock_file: File,
    process_id: u32,
}

static GENERALS_MUTEX_GUARD: OnceLock<Mutex<Option<SingleInstanceGuard>>> = OnceLock::new();

fn generals_mutex_guard_slot() -> &'static Mutex<Option<SingleInstanceGuard>> {
    GENERALS_MUTEX_GUARD.get_or_init(|| Mutex::new(None))
}

impl SingleInstanceGuard {
    /// Create a new single instance guard
    /// Returns Ok(guard) if this is the only instance, Err if another instance exists
    pub fn new(application_name: &str) -> Result<Self> {
        let lock_file_path = Self::get_lock_file_path(application_name)?;

        // Check if lock file already exists and is active
        if lock_file_path.exists() {
            if let Err(e) = Self::check_existing_instance(&lock_file_path) {
                // If we can't verify the existing instance, remove stale lock file
                info!("Removing stale lock file: {}", e);
                let _ = std::fs::remove_file(&lock_file_path);
            } else {
                return Err(anyhow::anyhow!(
                    "Another instance of {} is already running",
                    application_name
                ));
            }
        }

        // Create and lock the file
        let lock_file = Self::create_lock_file(&lock_file_path)?;
        let process_id = Self::get_current_process_id();

        // Write process information to lock file
        Self::write_lock_info(&lock_file_path, process_id)?;

        info!("Single instance lock acquired: {:?}", lock_file_path);
        info!("Process ID: {}", process_id);

        Ok(Self {
            lock_file_path,
            lock_file,
            process_id,
        })
    }

    /// Get the path for the lock file
    fn get_lock_file_path(application_name: &str) -> Result<PathBuf> {
        let mut path = if cfg!(target_os = "windows") {
            // On Windows, use temp directory
            std::env::temp_dir()
        } else {
            // On Unix-like systems, use /tmp or similar
            PathBuf::from("/tmp")
        };

        path.push(format!("{}.lock", application_name));
        Ok(path)
    }

    /// Create and lock the lock file
    fn create_lock_file(path: &PathBuf) -> Result<File> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .context("Failed to create lock file")?;

        // Platform-specific file locking
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();

            // Try to acquire an exclusive lock
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };

            if result != 0 {
                return Err(anyhow::anyhow!("Failed to acquire file lock"));
            }
        }

        #[cfg(windows)]
        {
            // On Windows, the file being opened exclusively should be sufficient
            // In a full implementation, you might use LockFile() API
        }

        Ok(file)
    }

    /// Write process information to the lock file
    fn write_lock_info(lock_file_path: &PathBuf, process_id: u32) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = format!(
            "Command & Conquer Generals Zero Hour\nPID: {}\nStarted: {}\n",
            process_id, timestamp
        );

        std::fs::write(lock_file_path, info)?;

        Ok(())
    }

    /// Check if an existing instance is actually running
    fn check_existing_instance(lock_file_path: &PathBuf) -> Result<()> {
        let content =
            std::fs::read_to_string(lock_file_path).context("Failed to read lock file")?;

        // Parse PID from lock file
        let process_id = Self::parse_pid_from_lock_file(&content)?;

        // Check if process is actually running
        if Self::is_process_running(process_id) {
            Ok(()) // Process is running
        } else {
            Err(anyhow::anyhow!("Process {} is not running", process_id))
        }
    }

    /// Parse process ID from lock file content
    fn parse_pid_from_lock_file(content: &str) -> Result<u32> {
        for line in content.lines() {
            if line.starts_with("PID: ") {
                let pid_str = line.strip_prefix("PID: ").unwrap_or("");
                return pid_str.parse().context("Invalid PID in lock file");
            }
        }
        Err(anyhow::anyhow!("No PID found in lock file"))
    }

    /// Check if a process with the given ID is running
    fn is_process_running(process_id: u32) -> bool {
        #[cfg(unix)]
        {
            // On Unix, use kill with signal 0 to test if process exists
            let result = unsafe { libc::kill(process_id as i32, 0) };
            if result == 0 {
                return true;
            }

            // EPERM means the process exists but we do not have permission to signal it.
            matches!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EPERM)
            )
        }

        #[cfg(windows)]
        {
            // On Windows, try to open the process handle
            use std::os::windows::io::AsRawHandle;
            use std::ptr;

            unsafe {
                let handle = winapi::um::processthreadsapi::OpenProcess(
                    winapi::um::winnt::PROCESS_QUERY_INFORMATION,
                    0, // Don't inherit handle
                    process_id,
                );

                if handle != ptr::null_mut() {
                    winapi::um::handleapi::CloseHandle(handle);
                    true
                } else {
                    false
                }
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback: assume process is running to be safe
            warn!("Process check not implemented for this platform");
            true
        }
    }

    /// Get the current process ID
    fn get_current_process_id() -> u32 {
        std::process::id()
    }

    /// Get the process ID protected by this guard
    pub fn get_process_id(&self) -> u32 {
        self.process_id
    }

    /// Get the lock file path for this instance
    pub fn get_instance_lock_file_path(&self) -> &PathBuf {
        &self.lock_file_path
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // Remove lock file when guard is dropped
        if let Err(e) = std::fs::remove_file(&self.lock_file_path) {
            error!(
                "Failed to remove lock file {:?}: {}",
                self.lock_file_path, e
            );
        } else {
            info!("Single instance lock released: {:?}", self.lock_file_path);
        }
    }
}

/// Check if another instance of the application is running
/// Returns true if another instance exists, false otherwise
pub fn check_for_existing_instance(application_name: &str) -> bool {
    match SingleInstanceGuard::new(application_name) {
        Ok(_guard) => {
            // We successfully acquired the lock, so no other instance is running
            // The guard will be dropped immediately, releasing the lock
            false
        }
        Err(_) => {
            // Failed to acquire lock, another instance is running
            true
        }
    }
}

/// Create a single instance guard for the application
/// This should be called early in main() and the guard should be kept alive
pub fn acquire_single_instance_lock(application_name: &str) -> Result<SingleInstanceGuard> {
    SingleInstanceGuard::new(application_name)
}

/// Show a message to the user about another instance running
pub fn show_multiple_instance_message() {
    eprintln!("Command & Conquer Generals Zero Hour is already running!");
    eprintln!("Only one instance of the game can run at a time.");
    eprintln!("Please close the existing instance before starting a new one.");

    // On desktop platforms, you might want to show a GUI dialog here
    #[cfg(feature = "native-dialog")]
    {
        let _ = native_dialog::MessageDialog::new()
            .set_type(native_dialog::MessageType::Warning)
            .set_title("Game Already Running")
            .set_text("Command & Conquer Generals Zero Hour is already running!\n\nOnly one instance can run at a time.")
            .show_alert();
    }
}

/// Initialize single instance protection
/// Returns a guard that must be kept alive for the duration of the program
pub fn initialize_single_instance_protection() -> Result<SingleInstanceGuard> {
    const APP_NAME: &str = "CnCGeneralsZeroHour";

    match acquire_single_instance_lock(APP_NAME) {
        Ok(guard) => {
            info!("Single instance protection initialized");
            Ok(guard)
        }
        Err(e) => {
            show_multiple_instance_message();
            Err(e)
        }
    }
}

/// Initialize single instance protection with copy protection integration
/// This version integrates with the copy protection system
pub fn initialize_single_instance_protection_with_copy_protection() -> Result<SingleInstanceGuard> {
    use crate::copy_protection;

    const APP_NAME: &str = "CnCGeneralsZeroHour";

    // First check if copy protection allows multiple instances
    if copy_protection::is_development_mode() {
        info!("Development mode: Allowing multiple instances");
    }

    match acquire_single_instance_lock(APP_NAME) {
        Ok(guard) => {
            info!("Single instance protection initialized with copy protection integration");

            // Notify copy protection system that we have acquired single instance lock
            if copy_protection::is_copy_protection_enabled() {
                if let Err(e) = copy_protection::notify_launcher() {
                    warn!(
                        "Failed to notify copy protection of single instance lock: {}",
                        e
                    );
                }
            }

            Ok(guard)
        }
        Err(e) => {
            // In development mode, we might want to allow override
            if copy_protection::is_development_mode() {
                info!("Development mode: Multiple instance detected; proceeding with dev lock");
                info!("Original lock error: {}", e);

                // Try to create a development instance with different name
                let dev_app_name = format!("{}_dev_{}", APP_NAME, std::process::id());
                match acquire_single_instance_lock(&dev_app_name) {
                    Ok(guard) => {
                        info!("Created development single instance lock: {}", dev_app_name);
                        return Ok(guard);
                    }
                    Err(dev_e) => {
                        info!("Failed to create development instance lock: {}", dev_e);
                    }
                }
            }

            show_multiple_instance_message();
            Err(e)
        }
    }
}

/// Create Generals mutex (matching C++ GeneralsMutex creation)
/// This is the function called from win_main.rs to replace create_generals_mutex
pub fn create_generals_mutex() -> bool {
    let mut guard_slot = match generals_mutex_guard_slot().lock() {
        Ok(slot) => slot,
        Err(_) => {
            error!("Failed to acquire single-instance mutex state lock");
            return false;
        }
    };

    if guard_slot.is_some() {
        info!("Generals mutex already active for this process");
        return true;
    }

    match initialize_single_instance_protection_with_copy_protection() {
        Ok(guard) => {
            *guard_slot = Some(guard);
            info!("Generals mutex created successfully");
            true
        }
        Err(e) => {
            error!("Failed to create Generals mutex: {}", e);
            false
        }
    }
}

/// Release the global Generals mutex guard.
///
/// Primarily used by controlled shutdown paths and tests.
pub fn release_generals_mutex() {
    if let Ok(mut slot) = generals_mutex_guard_slot().lock() {
        if slot.take().is_some() {
            info!("Generals mutex released");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_single_instance_guard_creation() {
        let guard = SingleInstanceGuard::new("test_app").unwrap();
        assert!(guard.get_process_id() > 0);
    }

    #[test]
    fn test_multiple_instance_prevention() {
        let _guard1 = SingleInstanceGuard::new("test_app_multi").unwrap();

        // This should fail because guard1 is still active
        let result = SingleInstanceGuard::new("test_app_multi");
        assert!(result.is_err());
    }

    #[test]
    fn test_guard_cleanup() {
        let lock_path = {
            let guard = SingleInstanceGuard::new("test_app_cleanup").unwrap();
            guard.get_instance_lock_file_path().clone()
        }; // Guard is dropped here

        // Small delay to ensure file system operations complete
        std::thread::sleep(Duration::from_millis(10));

        // Lock file should be cleaned up
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_process_id_parsing() {
        let content = "Command & Conquer Generals Zero Hour\nPID: 12345\nStarted: 1234567890\n";
        let pid = SingleInstanceGuard::parse_pid_from_lock_file(content).unwrap();
        assert_eq!(pid, 12345);
    }

    #[test]
    fn test_create_generals_mutex_retains_guard() {
        release_generals_mutex();

        assert!(
            create_generals_mutex(),
            "first create_generals_mutex call should acquire and retain guard"
        );
        assert!(
            create_generals_mutex(),
            "second create_generals_mutex call should detect existing retained guard"
        );

        release_generals_mutex();
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_running_detects_current_pid() {
        assert!(SingleInstanceGuard::is_process_running(std::process::id()));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_running_rejects_impossible_pid() {
        let current = std::process::id();
        let candidate = current
            .saturating_add(10_000_000)
            .min((i32::MAX - 1) as u32);
        assert!(candidate > 0);
        assert!(!SingleInstanceGuard::is_process_running(candidate));
    }
}
