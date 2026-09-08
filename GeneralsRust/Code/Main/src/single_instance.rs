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
use log::info;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
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

impl SingleInstanceGuard {
    /// Create a new single instance guard
    /// Returns Ok(guard) if this is the only instance, Err if another instance exists
    pub fn new(application_name: &str) -> Result<Self> {
        let lock_file_path = Self::get_lock_file_path(application_name)?;

        // The OS lock is the authority, as with C++ CreateMutex. Never unlink
        // a supposedly stale file: another process may already lock its inode.
        let mut lock_file = Self::create_lock_file(&lock_file_path)?;
        let process_id = Self::get_current_process_id();

        // Write process information to lock file
        Self::write_lock_info(&mut lock_file, process_id)?;

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
        let mut options = OpenOptions::new();
        options.create(true).write(true).truncate(false);
        #[cfg(windows)]
        options.share_mode(0);
        let file = options.open(path).context("Failed to create lock file")?;

        // Platform-specific file locking
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();

            // Try to acquire an exclusive lock
            // SAFETY: fd is a valid open file descriptor borrowed from
            // `file`, which outlives this call.
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };

            if result != 0 {
                return Err(anyhow::anyhow!("Failed to acquire file lock"));
            }
        }

        #[cfg(windows)]
        {
            // share_mode(0) above holds exclusive access until this file closes.
        }

        Ok(file)
    }

    /// Write process information to the lock file
    fn write_lock_info(lock_file: &mut File, process_id: u32) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = format!(
            "Command & Conquer Generals Zero Hour\nPID: {}\nStarted: {}\n",
            process_id, timestamp
        );

        // Mutate only after acquiring exclusion, through the locked descriptor.
        lock_file.set_len(0)?;
        lock_file.write_all(info.as_bytes())?;

        Ok(())
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
        // Closing lock_file releases exclusion, including after process death.
        // Keep the inode: unlinking allows a competing opener to lock a new
        // file while another process still holds the previous one.
        info!("Single instance lock released: {:?}", self.lock_file_path);
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
    let guard = initialize_single_instance_protection()?;
    if crate::copy_protection::is_copy_protection_enabled() {
        crate::copy_protection::notify_launcher()?;
    }
    Ok(guard)
}

/// Acquire the original unconditional single-instance exclusion. The entry
/// point owns the returned guard through game execution and cleanup.
pub fn create_generals_mutex() -> Result<SingleInstanceGuard> {
    initialize_single_instance_protection()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn locked_file_with_incomplete_metadata_still_excludes_another_instance() {
        let name = format!("generals_lock_metadata_{}", std::process::id());
        let guard = SingleInstanceGuard::new(&name).unwrap();
        // A second launcher can observe the first between flock and metadata
        // publication. PID text is not the authority for a held OS lock.
        guard.lock_file.set_len(0).unwrap();
        assert!(SingleInstanceGuard::new(&name).is_err());
    }

    #[test]
    fn lock_holder_subprocess() {
        let Ok(name) = std::env::var("GENERALS_LOCK_TEST_NAME") else {
            return;
        };
        let _guard = SingleInstanceGuard::new(&name).unwrap();
        println!("lock-acquired");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
    }

    #[test]
    fn another_process_is_excluded_and_process_death_releases_the_lock() {
        use std::io::{BufRead, BufReader};
        use std::process::{Child, Command, Stdio};
        struct Holder(Child);
        impl Drop for Holder {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let name = format!("generals_process_lock_{}", std::process::id());
        let mut holder = Holder(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "single_instance::tests::lock_holder_subprocess",
                    "--nocapture",
                ])
                .env("GENERALS_LOCK_TEST_NAME", &name)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut output = BufReader::new(holder.0.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "holder exited before acquiring lock"
            );
            if line.contains("lock-acquired") {
                break;
            }
        }
        assert!(SingleInstanceGuard::new(&name).is_err());
        // Force termination rather than running the Rust guard destructor.
        holder.0.kill().unwrap();
        holder.0.wait().unwrap();
        let _replacement = SingleInstanceGuard::new(&name).unwrap();
    }

    #[test]
    fn test_guard_cleanup() {
        let lock_path = {
            let guard = SingleInstanceGuard::new("test_app_cleanup").unwrap();
            guard.get_instance_lock_file_path().clone()
        }; // Guard is dropped here

        // Metadata may remain, but the OS lock must be released immediately.
        assert!(lock_path.exists());
        let _next = SingleInstanceGuard::new("test_app_cleanup").unwrap();
    }

    #[test]
    fn caller_owned_guard_releases_exclusion_on_drop() {
        let name = format!("generals_scoped_lock_{}", std::process::id());
        let guard = acquire_single_instance_lock(&name).unwrap();
        assert!(acquire_single_instance_lock(&name).is_err());
        drop(guard);
        let _next = acquire_single_instance_lock(&name).unwrap();
    }
}
