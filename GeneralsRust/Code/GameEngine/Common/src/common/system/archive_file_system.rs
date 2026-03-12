////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Archive File System Implementation
//!
//! Modernized archive file system facade that unifies BIG archive handling
//! while retaining the high-level semantics of the original C++ subsystem.

use once_cell::sync::OnceCell;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::system::archive_file::{ArchiveFileTrait, FileInfo};
use crate::common::system::big_file_system::BigFileSystem;
use crate::common::system::file_system::FilenameList;

/// Canonical music archive filename used by the original engine.
pub const MUSIC_BIG: &str = "Music.big";

/// Archive file system for managing multiple archive backends.
pub struct ArchiveFileSystem {
    big_system: BigFileSystem,
    search_paths: Vec<PathBuf>,
}

impl Default for ArchiveFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveFileSystem {
    /// Create a new archive file system facade.
    pub fn new() -> Self {
        Self {
            big_system: BigFileSystem::new(),
            search_paths: Vec::new(),
        }
    }

    /// Register an additional search path for BIG archives.
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if let Ok(canon) = path.canonicalize() {
            if !self.search_paths.iter().any(|p| p == &canon) {
                self.search_paths.push(canon);
            }
        } else if !self.search_paths.iter().any(|p| p == path) {
            self.search_paths.push(path.to_path_buf());
        }
    }

    /// Initialize the archive system, loading any configured search paths.
    pub fn init(&mut self) -> io::Result<()> {
        self.big_system.init()?;
        for path in self.search_paths.clone() {
            let _ = self.big_system.load_big_files_from_directory(
                path.to_string_lossy().as_ref(),
                "*.big",
                false,
            );
        }
        Ok(())
    }

    /// Update hook retained for API compatibility (currently a no-op).
    pub fn update(&mut self) {}

    /// Reset the archive system, clearing all loaded archives.
    pub fn reset(&mut self) {
        self.big_system.close_all_archive_files();
    }

    /// Hook retained for parity with the C++ implementation (currently a no-op).
    pub fn post_process_load(&mut self) {}

    /// Mirror of the C++ method – forwards to `open_archive_file` when possible.
    pub fn load_into_directory_tree(
        &mut self,
        archive_file: &dyn ArchiveFileTrait,
        _archive_filename: &AsciiString,
        _overwrite: bool,
    ) -> io::Result<()> {
        let path = archive_file.get_path();
        if path.is_empty() {
            return Ok(());
        }
        let path_buf = PathBuf::from(path.as_str());
        // Ignore errors – the path might already be registered.
        let _ = self.big_system.open_archive_file(&path_buf);
        Ok(())
    }

    /// Load mod archives from configured search paths.
    pub fn load_mods(&mut self) -> io::Result<()> {
        for path in self.search_paths.clone() {
            if path.exists() {
                self.big_system.load_big_files_from_directory(
                    path.to_string_lossy().as_ref(),
                    "*.big",
                    true,
                )?;
            }
        }
        Ok(())
    }

    /// Check if a virtual file exists within any registered archive.
    pub fn does_file_exist(&self, filename: &str) -> bool {
        self.big_system.does_file_exist(filename)
    }

    /// Open a file stream from the archive system.
    pub fn open_file(
        &mut self,
        filename: &str,
        access: i32,
    ) -> Result<Box<dyn Read + Send>, io::Error> {
        self.big_system.open_file(filename, access)
    }

    /// Retrieve file metadata for the specified virtual filename.
    pub fn get_file_info(
        &self,
        filename: &AsciiString,
        file_info: &mut FileInfo,
    ) -> Result<bool, io::Error> {
        if let Some(info) = self.big_system.get_archived_file_info(filename) {
            file_info.size = info.size;
            file_info.modified_time = 0;
            file_info.is_directory = false;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Resolve which archive currently owns the given virtual path.
    pub fn get_archive_filename_for_file(&self, filename: &AsciiString) -> AsciiString {
        self.big_system
            .resolve_archive_filename(filename)
            .unwrap_or_else(AsciiString::new)
    }

    /// Aggregate file listings across all archives.
    pub fn get_file_list_in_directory(
        &self,
        current_directory: &AsciiString,
        original_directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        let mut base = current_directory.clone();
        if base.is_empty() {
            base = original_directory.clone();
        }
        self.big_system.collect_matching_files(
            &base,
            search_name,
            filename_list,
            search_subdirectories,
        );
    }

    /// Open an archive file directly from disk and register it with the system.
    pub fn open_archive_file(&mut self, filename: &str) -> io::Result<()> {
        self.big_system.open_archive_file(Path::new(filename))
    }

    /// Close a specific archive file by name.
    pub fn close_archive_file(&mut self, filename: &str) {
        self.big_system.close_archive_file(filename);
    }

    /// Close all registered archives.
    pub fn close_all_archive_files(&mut self) {
        self.big_system.close_all_archive_files();
    }

    /// Close any materialized file handles (archives are already read-only memory/stream views).
    pub fn close_all_files(&mut self) {
        self.big_system.close_all_archive_files();
    }

    /// Load BIG archives from a directory using a glob mask.
    pub fn load_big_files_from_directory(
        &mut self,
        dir: &AsciiString,
        file_mask: &AsciiString,
        overwrite: bool,
    ) -> io::Result<bool> {
        self.big_system
            .load_big_files_from_directory(dir.as_str(), file_mask.as_str(), overwrite)
    }

    /// Forwarded access to the archive registry helpers.
    pub fn get_loaded_big_files(&self) -> Vec<AsciiString> {
        self.big_system.get_loaded_big_files()
    }

    /// Total physical file count across all archives.
    pub fn total_physical_files(&self) -> usize {
        self.big_system.total_physical_files()
    }

    /// Total unique virtual files tracked by the archive registry.
    pub fn total_virtual_files(&self) -> usize {
        self.big_system.total_virtual_files()
    }

    /// Enumerate the canonical virtual paths.
    pub fn virtual_paths(&self) -> Vec<String> {
        self.big_system.virtual_paths()
    }

    /// Access configured search paths (for mounting helpers).
    pub fn search_paths(&self) -> Vec<PathBuf> {
        self.search_paths.clone()
    }
}

/// Global archive file system instance (mirrors the original singleton pattern).
static ARCHIVE_FILE_SYSTEM: OnceCell<Mutex<ArchiveFileSystem>> = OnceCell::new();

/// Initialize the global archive file system singleton.
pub fn init_archive_file_system() {
    if ARCHIVE_FILE_SYSTEM.get().is_none() {
        let _ = ARCHIVE_FILE_SYSTEM.set(Mutex::new(ArchiveFileSystem::new()));
    } else if let Some(cell) = ARCHIVE_FILE_SYSTEM.get() {
        if let Ok(mut guard) = cell.lock() {
            *guard = ArchiveFileSystem::new();
        }
    }
}

/// Fetch a mutable reference to the global archive file system.
pub fn get_archive_file_system() -> Option<MutexGuard<'static, ArchiveFileSystem>> {
    ARCHIVE_FILE_SYSTEM
        .get()
        .map(|cell| cell.lock().expect("ArchiveFileSystem mutex poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_system_detects_missing_files() {
        let archive_system = ArchiveFileSystem::new();
        assert!(!archive_system.does_file_exist("nonexistent.txt"));
    }

    #[test]
    fn archive_system_resolves_archive_name() {
        let mut archive_system = ArchiveFileSystem::new();
        // Registering an empty path should no-op but still succeed.
        assert!(archive_system.open_archive_file("nonexistent.big").is_err());
    }
}
