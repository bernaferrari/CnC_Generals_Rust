use std::io;
use std::path::Path;

use crate::archive_file::FilenameList;
use crate::archive_file_system::ArchiveFileSystem;
use crate::file_info::FileInfo;

/// FileSystem integrates local and archive file systems
/// Matches the C++ FileSystem class implementation
pub struct FileSystem {
    archive_file_system: ArchiveFileSystem,
}

impl FileSystem {
    pub fn new() -> Self {
        Self {
            archive_file_system: ArchiveFileSystem::new(),
        }
    }

    /// Initialize the file system
    /// Matches the C++ FileSystem::init implementation
    pub fn init(&mut self) -> io::Result<()> {
        self.archive_file_system.init()
    }

    /// Update the file system (called each frame)
    /// Matches the C++ FileSystem::update implementation
    pub fn update(&mut self) {
        self.archive_file_system.update();
    }

    /// Reset the file system
    /// Matches the C++ FileSystem::reset implementation
    pub fn reset(&mut self) {
        self.archive_file_system.reset();
    }

    /// Open a file from the file system
    /// Tries local file system first, then archive file system
    /// Matches the C++ FileSystem::openFile implementation
    pub fn open_file(&mut self, filename: &str, access: i32) -> io::Result<Vec<u8>> {
        // Try to open from local file system first
        if let Ok(data) = std::fs::read(filename) {
            return Ok(data);
        }

        // Try archive file system
        self.archive_file_system.open_file(filename, access)
    }

    /// Check if a file exists in the file system
    /// Matches the C++ FileSystem::doesFileExist implementation
    pub fn does_file_exist(&self, filename: &str) -> bool {
        // Check local file system first
        if Path::new(filename).exists() {
            return true;
        }

        // Check archive file system
        self.archive_file_system.does_file_exist(filename)
    }

    /// Get file list in a directory
    /// Matches the C++ FileSystem::getFileListInDirectory implementation
    pub fn get_file_list_in_directory(
        &self,
        directory: &str,
        search_name: &str,
        search_subdirectories: bool,
    ) -> FilenameList {
        let mut filename_list = FilenameList::new();

        // Get files from archive file system
        let archive_files = self.archive_file_system.get_file_list_in_directory(
            "",
            directory,
            search_name,
            search_subdirectories,
        );
        filename_list.extend(archive_files);

        // Could also scan local file system here if needed

        filename_list
    }

    /// Get file information
    /// Matches the C++ FileSystem::getFileInfo implementation
    pub fn get_file_info(&self, filename: &str) -> Option<FileInfo> {
        // Try local file system first
        if let Ok(metadata) = std::fs::metadata(filename) {
            return Some(FileInfo {
                size_high: 0,
                size_low: metadata.len() as i32,
                timestamp_high: 0,
                timestamp_low: 0,
            });
        }

        // Try archive file system
        self.archive_file_system.get_file_info(filename)
    }

    /// Get access to the archive file system
    pub fn archive_file_system(&self) -> &ArchiveFileSystem {
        &self.archive_file_system
    }

    /// Get mutable access to the archive file system
    pub fn archive_file_system_mut(&mut self) -> &mut ArchiveFileSystem {
        &mut self.archive_file_system
    }
}

impl Default for FileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_system_creation() {
        let fs = FileSystem::new();
        assert!(!fs.does_file_exist("nonexistent.txt"));
    }

    #[test]
    fn test_file_system_init() {
        let mut fs = FileSystem::new();
        // Init should succeed even if no BIG files are found
        let result = fs.init();
        assert!(result.is_ok());
    }
}
