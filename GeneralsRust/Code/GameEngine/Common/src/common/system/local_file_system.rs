//! Local file system backend implementation
//!
//! This module provides a file system backend that works with the local file system,
//! implementing the FileSystemBackend trait for standard file operations.

use std::any::Any;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::common::ascii_string::AsciiString;
use crate::common::system::{
    file::{File, FileAccess},
    file_system::{FileInfo, FileSystemBackend, FilenameList},
    local_file::LocalFile,
    subsystem_interface::{SubsystemInterface, SubsystemResult, SubsystemState},
};

fn wildcard_match_case_insensitive(candidate: &str, mask: &str) -> bool {
    let candidate = candidate.to_ascii_lowercase();
    let mask = mask.to_ascii_lowercase();
    let candidate_bytes = candidate.as_bytes();
    let mask_bytes = mask.as_bytes();

    let mut candidate_index = 0usize;
    let mut mask_index = 0usize;
    let mut star_index: Option<usize> = None;
    let mut match_index = 0usize;

    while candidate_index < candidate_bytes.len() {
        if mask_index < mask_bytes.len()
            && (mask_bytes[mask_index] == candidate_bytes[candidate_index]
                || mask_bytes[mask_index] == b'?')
        {
            candidate_index += 1;
            mask_index += 1;
        } else if mask_index < mask_bytes.len() && mask_bytes[mask_index] == b'*' {
            star_index = Some(mask_index);
            match_index = candidate_index;
            mask_index += 1;
        } else if let Some(star) = star_index {
            mask_index = star + 1;
            match_index += 1;
            candidate_index = match_index;
        } else {
            return false;
        }
    }

    while mask_index < mask_bytes.len() && mask_bytes[mask_index] == b'*' {
        mask_index += 1;
    }

    mask_index == mask_bytes.len()
}

/// Local file system backend implementation
///
/// This backend provides access to files on the local file system using
/// standard filesystem operations.
pub struct LocalFileSystem {
    /// Subsystem name
    _name: String,
    /// Subsystem state
    state: SubsystemState,
    /// Base paths to search for files
    search_paths: Vec<PathBuf>,
    /// Whether the file system has been initialized
    initialized: bool,
}

impl LocalFileSystem {
    fn resolve_existing_path_case_insensitive(path: &Path) -> Option<PathBuf> {
        if path.exists() {
            return Some(path.to_path_buf());
        }

        let mut resolved = PathBuf::new();

        for component in path.components() {
            match component {
                Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
                Component::RootDir => resolved.push(component.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !resolved.pop() {
                        return None;
                    }
                }
                Component::Normal(part) => {
                    let exact = resolved.join(part);
                    if exact.exists() {
                        resolved = exact;
                        continue;
                    }

                    let search_dir = if resolved.as_os_str().is_empty() {
                        Path::new(".")
                    } else {
                        resolved.as_path()
                    };
                    let part = part.to_string_lossy();
                    let entries = fs::read_dir(search_dir).ok()?;
                    let matched = entries.filter_map(Result::ok).find_map(|entry| {
                        let name = entry.file_name();
                        if name.to_string_lossy().eq_ignore_ascii_case(&part) {
                            Some(entry.path())
                        } else {
                            None
                        }
                    })?;
                    resolved = matched;
                }
            }
        }

        resolved.exists().then_some(resolved)
    }

    /// Create a new local file system backend
    pub fn new() -> Self {
        Self {
            _name: "LocalFileSystem".to_string(),
            state: SubsystemState::Uninitialized,
            search_paths: Vec::new(),
            initialized: false,
        }
    }

    /// Add a search path for file lookups
    ///
    /// Files will be searched in the order paths were added.
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        let incoming = path.as_ref();
        let path_buf = if let Ok(canonical) = fs::canonicalize(incoming) {
            canonical
        } else {
            incoming.to_path_buf()
        };

        if !self
            .search_paths
            .iter()
            .any(|existing| existing == &path_buf)
        {
            self.search_paths.push(path_buf);
        }
    }

    /// Remove a search path
    pub fn remove_search_path<P: AsRef<Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        self.search_paths.retain(|p| p != &path_buf);
    }

    /// Get all search paths
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Find a file in the search paths
    fn find_file_path(&self, filename: &str) -> Option<PathBuf> {
        // First try the filename as-is (absolute path or relative to current directory)
        let direct_path = Path::new(filename);
        if let Some(path) = Self::resolve_existing_path_case_insensitive(direct_path) {
            return Some(path);
        }

        // Then search in all configured search paths
        for search_path in &self.search_paths {
            let full_path = search_path.join(filename);
            if let Some(path) = Self::resolve_existing_path_case_insensitive(&full_path) {
                return Some(path);
            }
        }

        None
    }

    /// Convert filesystem metadata to FileInfo
    fn metadata_to_file_info(metadata: &fs::Metadata) -> FileInfo {
        let size = metadata.len();
        let timestamp = metadata
            .modified()
            .or_else(|_| metadata.created())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
            .unwrap_or(0) as u64;

        FileInfo {
            size_high: ((size >> 32) & 0xFFFFFFFF) as i32,
            size_low: (size & 0xFFFFFFFF) as i32,
            timestamp_high: ((timestamp >> 32) & 0xFFFFFFFF) as i32,
            timestamp_low: (timestamp & 0xFFFFFFFF) as i32,
        }
    }

    /// Check if a path matches a search pattern
    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        wildcard_match_case_insensitive(filename, pattern)
    }

    /// Recursively search directory for matching files
    fn search_directory(
        &self,
        dir_path: &Path,
        search_pattern: &str,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
        base_path: &Path,
    ) {
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        if Self::matches_pattern(filename, search_pattern) {
                            // Create relative path from base path
                            let relative_path = path
                                .strip_prefix(base_path)
                                .unwrap_or(&path)
                                .to_string_lossy();
                            filename_list.insert(AsciiString::from(relative_path.as_ref()));
                        }
                    }
                } else if path.is_dir() && search_subdirectories {
                    self.search_directory(&path, search_pattern, filename_list, true, base_path);
                }
            }
        }
    }
}

impl Default for LocalFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemBackend for LocalFileSystem {
    fn identifier(&self) -> &'static str {
        "local"
    }

    fn init(&mut self) {
        if !self.initialized {
            if self.search_paths.is_empty() {
                self.search_paths.push(PathBuf::from("."));
            }

            if let Ok(current_dir) = std::env::current_dir() {
                self.add_search_path(current_dir.join("Data"));
                self.add_search_path(current_dir.join("Art"));
                self.add_search_path(current_dir.join("Maps"));
            }

            self.initialized = true;
        }
        self.state = SubsystemState::Running;
    }

    fn reset(&mut self) {
        self.initialized = false;
        self.state = SubsystemState::Uninitialized;
    }

    fn update(&mut self) {
        // No ongoing updates needed for local file system
    }

    fn open_file(&mut self, filename: &str, access: FileAccess) -> Option<Box<dyn File>> {
        if let Some(file_path) = self.find_file_path(filename) {
            let mut local_file = LocalFile::new();

            if local_file
                .open(file_path.to_string_lossy().as_ref(), access)
                .is_ok()
            {
                return Some(Box::new(local_file));
            }
        }

        None
    }

    fn does_file_exist(&self, filename: &str) -> bool {
        self.find_file_path(filename).is_some()
    }

    fn get_file_list_in_directory(
        &self,
        base_path: &AsciiString,
        directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        let base = if base_path.is_empty() {
            Path::new(".")
        } else {
            Path::new(base_path.as_str())
        };

        let search_dir = if directory.is_empty() {
            base.to_path_buf()
        } else {
            base.join(directory.as_str())
        };

        // Also search in all configured search paths
        for search_path in &self.search_paths {
            let full_search_dir = if directory.is_empty() {
                search_path.clone()
            } else {
                search_path.join(directory.as_str())
            };

            if let Some(full_search_dir) =
                Self::resolve_existing_path_case_insensitive(&full_search_dir)
            {
                if full_search_dir.is_dir() {
                    self.search_directory(
                        &full_search_dir,
                        search_name.as_str(),
                        filename_list,
                        search_subdirectories,
                        search_path,
                    );
                }
            }
        }

        // Search in the specified directory
        if let Some(search_dir) = Self::resolve_existing_path_case_insensitive(&search_dir) {
            if search_dir.is_dir() {
                self.search_directory(
                    &search_dir,
                    search_name.as_str(),
                    filename_list,
                    search_subdirectories,
                    base,
                );
            }
        }
    }

    fn get_file_info(&self, filename: &AsciiString) -> Option<FileInfo> {
        if let Some(file_path) = self.find_file_path(filename.as_str()) {
            if let Ok(metadata) = fs::metadata(file_path) {
                return Some(Self::metadata_to_file_info(&metadata));
            }
        }

        None
    }

    fn create_directory(&mut self, directory: AsciiString) -> bool {
        let path = Path::new(directory.as_str());

        // Try to create in each search path
        for search_path in &self.search_paths {
            let full_path = search_path.join(&path);
            if fs::create_dir_all(&full_path).is_ok() {
                return true;
            }
        }

        // Try to create directory as-is
        fs::create_dir_all(path).is_ok()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl SubsystemInterface for LocalFileSystem {
    fn name(&self) -> &str {
        "LocalFileSystem"
    }

    fn init(&mut self) -> SubsystemResult<()> {
        FileSystemBackend::init(self);
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        FileSystemBackend::reset(self);
        Ok(())
    }

    fn update(&mut self, _delta_time: std::time::Duration) -> SubsystemResult<()> {
        FileSystemBackend::update(self);
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        FileSystemBackend::reset(self);
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn state(&self) -> crate::common::system::subsystem_interface::SubsystemState {
        if self.initialized {
            crate::common::system::subsystem_interface::SubsystemState::Running
        } else {
            crate::common::system::subsystem_interface::SubsystemState::Uninitialized
        }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

// Global local file system instance (mirrors TheLocalFileSystem singleton)
lazy_static::lazy_static! {
    pub static ref THE_LOCAL_FILE_SYSTEM: Arc<Mutex<LocalFileSystem>> =
        Arc::new(Mutex::new(LocalFileSystem::new()));
}

/// Convenience function to access the global local file system
pub fn get_local_file_system() -> Arc<Mutex<LocalFileSystem>> {
    THE_LOCAL_FILE_SYSTEM.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_local_file_system_creation() {
        let fs = LocalFileSystem::new();
        assert!(!fs.initialized);
        assert_eq!(fs.search_paths().len(), 0);
    }

    #[test]
    fn test_search_path_management() {
        let mut fs = LocalFileSystem::new();

        fs.add_search_path("/test/path1");
        fs.add_search_path("/test/path2");

        assert_eq!(fs.search_paths().len(), 2);

        // Adding the same path twice should not duplicate it
        fs.add_search_path("/test/path1");
        assert_eq!(fs.search_paths().len(), 2);

        fs.remove_search_path("/test/path1");
        assert_eq!(fs.search_paths().len(), 1);
    }

    #[test]
    fn test_pattern_matching() {
        assert!(LocalFileSystem::matches_pattern("test.txt", "*.txt"));
        assert!(LocalFileSystem::matches_pattern("file.ini", "*.ini"));
        assert!(LocalFileSystem::matches_pattern("file.ini", "*.INI"));
        assert!(LocalFileSystem::matches_pattern(
            "AmericaVehicle.INI",
            "america*.ini"
        ));
        assert!(LocalFileSystem::matches_pattern("foo.bar", "f?o.*"));
        assert!(LocalFileSystem::matches_pattern("anything", "*"));
        assert!(!LocalFileSystem::matches_pattern("test.txt", "*.ini"));
        assert!(LocalFileSystem::matches_pattern("test.txt", "test.txt"));
    }

    #[test]
    fn test_file_operations() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary test file
        let test_file = "test_local_fs.txt";
        let test_content = "Hello, LocalFileSystem!";

        {
            let mut file = fs::File::create(test_file)?;
            file.write_all(test_content.as_bytes())?;
        }

        let mut fs = LocalFileSystem::new();
        FileSystemBackend::init(&mut fs);

        // Test file existence
        assert!(fs.does_file_exist(test_file));
        assert!(!fs.does_file_exist("nonexistent.txt"));

        // Test file info
        let file_info = fs.get_file_info(&AsciiString::from(test_file));
        assert!(file_info.is_some());

        let info = file_info.unwrap();
        assert!(info.size_low > 0);

        // Test opening file
        let file_handle = fs.open_file(test_file, FileAccess::READ);
        assert!(file_handle.is_some());

        // Clean up
        fs::remove_file(test_file)?;

        Ok(())
    }

    #[test]
    fn test_directory_operations() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary test directory structure
        let test_dir = "test_dir";
        let test_subdir = "test_dir/subdir";

        fs::create_dir_all(test_subdir)?;

        // Create test files
        fs::File::create("test_dir/file1.txt")?;
        fs::File::create("test_dir/file2.ini")?;
        fs::File::create("test_dir/subdir/file3.txt")?;

        let fs = LocalFileSystem::new();
        let mut filename_list = FilenameList::new();

        // Test file listing without subdirectories
        fs.get_file_list_in_directory(
            &AsciiString::from(""),
            &AsciiString::from(test_dir),
            &AsciiString::from("*.txt"),
            &mut filename_list,
            false,
        );

        assert_eq!(filename_list.len(), 1);

        filename_list.clear();

        // Test file listing with subdirectories
        fs.get_file_list_in_directory(
            &AsciiString::from(""),
            &AsciiString::from(test_dir),
            &AsciiString::from("*.txt"),
            &mut filename_list,
            true,
        );

        assert_eq!(filename_list.len(), 2);

        // Clean up
        fs::remove_dir_all(test_dir)?;

        Ok(())
    }

    #[test]
    fn test_metadata_conversion() {
        // This test uses a file that should exist in most systems
        if let Ok(metadata) = fs::metadata(".") {
            let file_info = LocalFileSystem::metadata_to_file_info(&metadata);

            // Basic sanity checks
            assert!(file_info.size_high >= 0);
            assert!(file_info.size_low >= 0);
            assert!(file_info.timestamp_high >= 0);
            assert!(file_info.timestamp_low >= 0);
        }
    }

    #[test]
    fn test_directory_creation() {
        let mut fs = LocalFileSystem::new();
        FileSystemBackend::init(&mut fs);

        let test_dir = "test_create_dir";

        // Test creating a directory
        assert!(fs.create_directory(AsciiString::from(test_dir)));
        assert!(Path::new(test_dir).exists());

        // Clean up
        let _ = fs::remove_dir(test_dir);
    }

    #[test]
    fn test_initialization() {
        let mut fs = LocalFileSystem::new();
        assert!(!fs.initialized);

        FileSystemBackend::init(&mut fs);
        assert!(fs.initialized);
        assert!(!fs.search_paths().is_empty());

        let path_count = fs.search_paths().len();

        FileSystemBackend::reset(&mut fs);
        assert!(!fs.initialized);
        assert_eq!(fs.search_paths().len(), path_count);

        FileSystemBackend::init(&mut fs);
        assert!(fs.initialized);
    }

    #[test]
    fn test_find_file_path_matches_case_insensitive_search_path_entries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let test_root = PathBuf::from("test_case_insensitive_local_fs");
        let actual_path = test_root
            .join("Art")
            .join("Textures")
            .join("mainmenuruleruserinterface.tga");
        fs::create_dir_all(actual_path.parent().expect("asset parent"))?;
        fs::write(&actual_path, b"test")?;

        let mut fs_backend = LocalFileSystem::new();
        fs_backend.add_search_path(&test_root);

        let resolved = fs_backend
            .find_file_path("art/textures/MainMenuRuleruserinterface.tga")
            .expect("case-insensitive local lookup should resolve");

        assert_eq!(resolved, actual_path);

        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn test_directory_listing_matches_case_insensitive_directories(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let test_root = PathBuf::from("test_case_insensitive_listing");
        let actual_dir = test_root.join("Art").join("Textures");
        fs::create_dir_all(&actual_dir)?;
        fs::write(actual_dir.join("mainmenuruleruserinterface.tga"), b"test")?;

        let mut fs_backend = LocalFileSystem::new();
        fs_backend.add_search_path(&test_root);

        let mut filenames = FilenameList::new();
        fs_backend.get_file_list_in_directory(
            &AsciiString::from(""),
            &AsciiString::from("art/textures"),
            &AsciiString::from("*.tga"),
            &mut filenames,
            false,
        );

        assert_eq!(filenames.len(), 1);

        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn test_directory_listing_matches_case_insensitive_masks(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let test_root = PathBuf::from("test_case_insensitive_mask_listing");
        let actual_dir = test_root.join("Data").join("INI").join("Object");
        fs::create_dir_all(&actual_dir)?;
        fs::write(actual_dir.join("AmericaVehicle.ini"), b"test")?;
        fs::write(actual_dir.join("AmericaInfantry.INI"), b"test")?;
        fs::write(actual_dir.join("Readme.txt"), b"test")?;

        let mut fs_backend = LocalFileSystem::new();
        fs_backend.add_search_path(&test_root);

        let mut filenames = FilenameList::new();
        fs_backend.get_file_list_in_directory(
            &AsciiString::from(""),
            &AsciiString::from("data/ini/object"),
            &AsciiString::from("*.INI"),
            &mut filenames,
            false,
        );

        let listed = filenames
            .iter()
            .map(|name| name.as_str().replace('\\', "/").to_lowercase())
            .collect::<Vec<_>>();

        assert_eq!(
            listed,
            vec![
                "data/ini/object/americainfantry.ini".to_string(),
                "data/ini/object/americavehicle.ini".to_string()
            ]
        );

        fs::remove_dir_all(test_root)?;
        Ok(())
    }
}
