////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! CD Manager Implementation
//!
//! Manages CD-ROM drives and disk detection for the game engine.
//! Provides interface to detect and manage CD drives for copy protection
//! and media validation purposes.
//!
//! Created: 11/26/01 TR
//! Rust conversion: 2025

use once_cell::sync::OnceCell;
use std::sync::{Mutex, MutexGuard};

use crate::common::ascii_string::AsciiString;

/// CD disk enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Disk {
    UnknownDisk = -3,
    NoDisk = -2,
    AnyDisk = -1,
    Disk1 = 0,
}

impl Disk {
    pub const NUM_DISKS: i32 = 1;
}

/// Trait for CD drive interface
pub trait CDDriveInterface {
    /// Update drive information
    fn refresh_info(&mut self);

    /// Get the disk name/volume label
    fn get_disk_name(&self) -> AsciiString;

    /// Get the drive path
    fn get_path(&self) -> AsciiString;

    /// Get the current disk ID
    fn get_disk(&self) -> Disk;
}

/// CD Drive implementation
#[derive(Debug, Clone)]
pub struct CDDrive {
    disk_name: AsciiString,
    drive_path: AsciiString,
    disk: Disk,
}

impl Default for CDDrive {
    fn default() -> Self {
        Self::new()
    }
}

impl CDDrive {
    /// Create a new CD drive
    pub fn new() -> Self {
        Self {
            disk_name: AsciiString::new(),
            drive_path: AsciiString::new(),
            disk: Disk::NoDisk,
        }
    }

    /// Set the drive path
    pub fn set_path(&mut self, path: &str) {
        self.drive_path = AsciiString::from(path);
        self.refresh_info();
    }
}

impl CDDriveInterface for CDDrive {
    fn refresh_info(&mut self) {
        // Mock implementation - in real system this would:
        // 1. Check if drive exists
        // 2. Read volume label
        // 3. Determine disk type
        // 4. Update disk status

        if !self.drive_path.is_empty() {
            // Simulate checking for disk presence
            if std::path::Path::new(self.drive_path.as_str()).exists() {
                self.disk = Disk::Disk1;
                self.disk_name = AsciiString::from("Game Disk 1");
            } else {
                self.disk = Disk::NoDisk;
                self.disk_name.clear();
            }
        } else {
            self.disk = Disk::UnknownDisk;
            self.disk_name.clear();
        }
    }

    fn get_disk_name(&self) -> AsciiString {
        self.disk_name.clone()
    }

    fn get_path(&self) -> AsciiString {
        self.drive_path.clone()
    }

    fn get_disk(&self) -> Disk {
        self.disk
    }
}

/// Trait for CD Manager interface
pub trait CDManagerInterface {
    /// Initialize the CD manager
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    /// Update the CD manager
    fn update(&mut self);

    /// Reset the CD manager
    fn reset(&mut self);

    /// Post-process load operations
    fn post_process_load(&mut self);

    /// Get number of CD drives detected
    fn drive_count(&self) -> usize;

    /// Get the specified drive
    fn get_drive(&self, index: usize) -> Option<&(dyn CDDriveInterface + Send + Sync)>;

    /// Get mutable reference to the specified drive
    fn get_drive_mut(&mut self, index: usize) -> Option<&mut (dyn CDDriveInterface + Send + Sync)>;

    /// Add a new drive with specified path
    fn new_drive(&mut self, path: &str) -> Result<usize, Box<dyn std::error::Error>>;

    /// Refresh all drive information
    fn refresh_drives(&mut self);

    /// Destroy all drives
    fn destroy_all_drives(&mut self);
}

/// CD Manager implementation
pub struct CDManager {
    drives: Vec<Box<dyn CDDriveInterface + Send + Sync>>,
}

impl Default for CDManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CDManager {
    /// Create a new CD Manager
    pub fn new() -> Self {
        Self { drives: Vec::new() }
    }

    /// Scan for available CD drives on the system
    fn scan_drives(&mut self) {
        // Mock implementation - in real system this would:
        // 1. Enumerate system drives
        // 2. Check which ones are CD-ROM drives
        // 3. Add them to the drives list

        // For now, we'll just add a mock drive
        #[cfg(target_os = "windows")]
        {
            for letter in b'D'..=b'Z' {
                let drive_path = format!("{}:\\", letter as char);
                if std::path::Path::new(&drive_path).exists() {
                    let mut drive = CDDrive::new();
                    drive.set_path(&drive_path);
                    self.drives.push(Box::new(drive));
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Unix-like systems
            let common_mount_points = ["/media", "/mnt", "/cdrom"];
            for mount_point in &common_mount_points {
                if std::path::Path::new(mount_point).exists() {
                    let mut drive = CDDrive::new();
                    drive.set_path(mount_point);
                    self.drives.push(Box::new(drive));
                }
            }
        }
    }
}

impl CDManagerInterface for CDManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_drives();
        Ok(())
    }

    fn update(&mut self) {
        // Periodic update if needed
    }

    fn reset(&mut self) {
        self.destroy_all_drives();
    }

    fn post_process_load(&mut self) {
        // Post-processing after loading
    }

    fn drive_count(&self) -> usize {
        self.drives.len()
    }

    fn get_drive(&self, index: usize) -> Option<&(dyn CDDriveInterface + Send + Sync)> {
        self.drives.get(index).map(|d| d.as_ref())
    }

    fn get_drive_mut(&mut self, index: usize) -> Option<&mut (dyn CDDriveInterface + Send + Sync)> {
        if let Some(drive) = self.drives.get_mut(index) {
            Some(drive.as_mut())
        } else {
            None
        }
    }

    fn new_drive(&mut self, path: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let mut drive = CDDrive::new();
        drive.set_path(path);
        let index = self.drives.len();
        self.drives.push(Box::new(drive));
        Ok(index)
    }

    fn refresh_drives(&mut self) {
        for drive in &mut self.drives {
            drive.refresh_info();
        }
    }

    fn destroy_all_drives(&mut self) {
        self.drives.clear();
    }
}

/// Global CD manager instance
static CD_MANAGER: OnceCell<Mutex<Box<dyn CDManagerInterface + Send + Sync>>> = OnceCell::new();

/// Initialize the global CD manager
pub fn init_cd_manager() {
    let mut manager = CDManager::new();
    let _ = manager.init();

    if CD_MANAGER.get().is_none() {
        let _ = CD_MANAGER.set(Mutex::new(Box::new(manager)));
    } else if let Some(cell) = CD_MANAGER.get() {
        if let Ok(mut guard) = cell.lock() {
            *guard = Box::new(manager);
        }
    }
}

/// Get reference to the global CD manager
pub fn get_cd_manager() -> Option<MutexGuard<'static, Box<dyn CDManagerInterface + Send + Sync>>> {
    CD_MANAGER
        .get()
        .map(|cell| cell.lock().expect("CDManager mutex poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cd_drive_creation() {
        let drive = CDDrive::new();
        assert_eq!(drive.get_disk(), Disk::NoDisk);
        assert!(drive.get_path().is_empty());
        assert!(drive.get_disk_name().is_empty());
    }

    #[test]
    fn test_cd_drive_path_setting() {
        let mut drive = CDDrive::new();
        drive.set_path("/test/path");
        assert_eq!(drive.get_path().as_str(), "/test/path");
    }

    #[test]
    fn test_cd_manager_creation() {
        let manager = CDManager::new();
        assert_eq!(manager.drive_count(), 0);
    }

    #[test]
    fn test_cd_manager_add_drive() {
        let mut manager = CDManager::new();
        let result = manager.new_drive("/test/drive");
        assert!(result.is_ok());
        assert_eq!(manager.drive_count(), 1);
    }

    #[test]
    fn test_disk_enum() {
        assert_eq!(Disk::UnknownDisk as i32, -3);
        assert_eq!(Disk::NoDisk as i32, -2);
        assert_eq!(Disk::AnyDisk as i32, -1);
        assert_eq!(Disk::Disk1 as i32, 0);
    }
}
