//! FileSystem management module
//!
//! This module provides file system abstraction and management capabilities,
//! including support for multiple file system backends (local files, archives, etc.).

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use crate::common::{
    ascii_string::AsciiString,
    name_key_generator::{NameKeyGenerator, NameKeyType},
    system::{
        file::{File, FileAccess},
        subsystem_interface::{SubsystemInterface, SubsystemResult, SubsystemState},
    },
};

/// File information structure
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub size_high: i32,
    pub size_low: i32,
    pub timestamp_high: i32,
    pub timestamp_low: i32,
}

impl Default for FileInfo {
    fn default() -> Self {
        Self {
            size_high: 0,
            size_low: 0,
            timestamp_high: 0,
            timestamp_low: 0,
        }
    }
}

/// Collection of filenames preserving deterministic, case-insensitive ordering.
#[derive(Debug, Clone, Default)]
pub struct FilenameList {
    entries: BTreeMap<String, AsciiString>,
}

impl FilenameList {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn insert(&mut self, filename: AsciiString) -> bool {
        let key = filename.as_str().replace('\\', "/").to_lowercase();
        self.entries.insert(key, filename).is_none()
    }

    pub fn contains(&self, filename: &AsciiString) -> bool {
        let key = filename.as_str().replace('\\', "/").to_lowercase();
        self.entries.contains_key(&key)
    }

    pub fn iter(&self) -> std::collections::btree_map::Values<'_, String, AsciiString> {
        self.entries.values()
    }
}

impl<'a> IntoIterator for &'a FilenameList {
    type Item = &'a AsciiString;
    type IntoIter = std::collections::btree_map::Values<'a, String, AsciiString>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.values()
    }
}

impl IntoIterator for FilenameList {
    type Item = AsciiString;
    type IntoIter = std::vec::IntoIter<AsciiString>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_values().collect::<Vec<_>>().into_iter()
    }
}

/// FileSystem trait for different file system implementations
pub trait FileSystemBackend: Send + Sync + Any {
    /// Stable identifier for the backend (used to avoid duplicate registrations)
    fn identifier(&self) -> &'static str;

    /// Initialize the file system backend
    fn init(&mut self);

    /// Reset the file system backend
    fn reset(&mut self);

    /// Update the file system backend
    fn update(&mut self);

    /// Open a file through this backend
    fn open_file(&mut self, filename: &str, access: FileAccess) -> Option<Box<dyn File>>;

    /// Check if a file exists in this backend
    fn does_file_exist(&self, filename: &str) -> bool;

    /// Get list of files in a directory
    fn get_file_list_in_directory(
        &self,
        base_path: &AsciiString,
        directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    );

    /// Get file information
    fn get_file_info(&self, filename: &AsciiString) -> Option<FileInfo>;

    /// Create a directory (if supported by this backend)
    fn create_directory(&mut self, _directory: AsciiString) -> bool {
        false // Default implementation for read-only backends
    }

    /// Downcast support for backend management
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Main file system manager
///
/// This corresponds to the C++ FileSystem class and manages multiple
/// file system backends in priority order.
pub struct FileSystem {
    /// Subsystem name
    name: String,

    /// Subsystem state
    state: SubsystemState,

    /// Cache for file existence checks
    file_exist_cache: Arc<RwLock<HashMap<NameKeyType, bool>>>,

    /// Registered file system backends in priority order
    backends: Vec<Box<dyn FileSystemBackend>>,
}

impl FileSystem {
    fn downcast_backend_mut<T: 'static>(backend: &mut dyn FileSystemBackend) -> Option<&mut T> {
        (backend as &mut dyn Any).downcast_mut::<T>()
    }

    /// Create a new FileSystem instance
    pub fn new() -> Self {
        Self {
            name: "FileSystem".to_string(),
            state: SubsystemState::Uninitialized,
            file_exist_cache: Arc::new(RwLock::new(HashMap::new())),
            backends: Vec::new(),
        }
    }

    /// Register a file system backend
    ///
    /// Backends are searched in the order they are registered.
    pub fn register_backend(&mut self, backend: Box<dyn FileSystemBackend>) {
        self.backends.push(backend);
    }

    /// Ensure a backend of type `T` exists, returning a mutable reference to it.
    pub fn ensure_backend<T, F>(&mut self, constructor: F) -> &mut T
    where
        T: FileSystemBackend + 'static,
        F: FnOnce() -> T,
    {
        if let Some(index) = self
            .backends
            .iter()
            .position(|backend| backend.as_any().is::<T>())
        {
            let backend = self.backends[index].as_mut();
            return Self::downcast_backend_mut::<T>(backend)
                .expect("backend type mismatch during downcast");
        }

        self.backends.push(Box::new(constructor()));
        let len = self.backends.len();
        let backend = self.backends[len - 1].as_mut();
        Self::downcast_backend_mut::<T>(backend).expect("backend should downcast after insertion")
    }

    /// Open a file using the registered backends
    ///
    /// This searches through all backends in order until a file is found.
    pub fn open_file(&mut self, filename: &str, access: FileAccess) -> Option<Box<dyn File>> {
        for backend in &mut self.backends {
            if let Some(file) = backend.open_file(filename, access) {
                return Some(file);
            }
        }
        None
    }

    /// Check if a file exists in any backend
    ///
    /// This method implements caching to avoid repeated file system queries.
    pub fn does_file_exist(&self, filename: &str) -> bool {
        // Generate a simple hash key for the filename
        let key = self.filename_to_key(filename);

        // Check cache first
        {
            let cache = self.file_exist_cache.read().unwrap();
            if let Some(&exists) = cache.get(&key) {
                return exists;
            }
        }

        // Check all backends
        let exists = self
            .backends
            .iter()
            .any(|backend| backend.does_file_exist(filename));

        // Update cache
        {
            let mut cache = self.file_exist_cache.write().unwrap();
            cache.insert(key, exists);
        }

        exists
    }

    /// Get list of files in a directory from all backends
    pub fn get_file_list_in_directory(
        &self,
        directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        for backend in &self.backends {
            backend.get_file_list_in_directory(
                &AsciiString::from(""),
                directory,
                search_name,
                filename_list,
                search_subdirectories,
            );
        }
    }

    /// Get file information from backends
    pub fn get_file_info(&self, filename: &AsciiString) -> Option<FileInfo> {
        for backend in &self.backends {
            if let Some(info) = backend.get_file_info(filename) {
                return Some(info);
            }
        }
        None
    }

    /// Create a directory using the first writable backend
    pub fn create_directory(&mut self, directory: AsciiString) -> bool {
        for backend in &mut self.backends {
            if backend.create_directory(directory.clone()) {
                return true;
            }
        }
        false
    }

    /// Simple hash function for filename caching
    ///
    /// This converts a filename to a simple hash key for caching purposes.
    fn filename_to_key(&self, filename: &str) -> NameKeyType {
        NameKeyGenerator::name_to_key_lowercase(filename)
    }

    /// Clear the file existence cache
    pub fn clear_cache(&self) {
        let mut cache = self.file_exist_cache.write().unwrap();
        cache.clear();
    }

    /// Get the number of registered backends
    pub fn backend_count(&self) -> usize {
        self.backends.len()
    }

    /// Attempt to retrieve a mutable backend of the requested type.
    pub fn get_backend_mut<T: 'static>(&mut self) -> Option<&mut T> {
        for backend in &mut self.backends {
            if let Some(existing) = Self::downcast_backend_mut::<T>(backend.as_mut()) {
                return Some(existing);
            }
        }
        None
    }
}

impl SubsystemInterface for FileSystem {
    fn name(&self) -> &str {
        &self.name
    }

    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;

        // Initialize all backends
        for backend in &mut self.backends {
            backend.init();
        }

        self.state = SubsystemState::Running;
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        // Reset all backends
        for backend in &mut self.backends {
            backend.reset();
        }

        // Clear cache
        self.clear_cache();

        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
        // Update all backends
        for backend in &mut self.backends {
            backend.update();
        }
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::ShuttingDown;

        // Clear cache
        self.clear_cache();

        // Reset all backends
        for backend in &mut self.backends {
            backend.reset();
        }

        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        self.state
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

impl Default for FileSystem {
    fn default() -> Self {
        Self::new()
    }
}

// Global file system instance (matches TheFileSystem singleton in C++)
lazy_static::lazy_static! {
    pub static ref THE_FILE_SYSTEM: Arc<Mutex<FileSystem>> = Arc::new(Mutex::new(FileSystem::new()));
}

/// Convenience function to access the global file system
pub fn get_file_system() -> Arc<Mutex<FileSystem>> {
    THE_FILE_SYSTEM.clone()
}

/// Directory path constants
///
/// These correspond to the directory defines in the C++ FileSystem.h
pub mod paths {
    pub const W3D_DIR_PATH: &str = "Art/W3D/";
    pub const TGA_DIR_PATH: &str = "Art/Textures/";
    pub const TERRAIN_TGA_DIR_PATH: &str = "Art/Terrain/";
    pub const MAP_PREVIEW_DIR_PATH: &str = "%sMapPreviews/";
    pub const USER_W3D_DIR_PATH: &str = "%sW3D/";
    pub const USER_TGA_DIR_PATH: &str = "%sTextures/";

    #[cfg(feature = "legacy-files")]
    pub mod legacy {
        pub const LEGACY_W3D_DIR_PATH: &str = "../LegacyArt/W3D/";
        pub const LEGACY_TGA_DIR_PATH: &str = "../LegacyArt/Textures/";
    }

    #[cfg(feature = "test-assets")]
    pub mod test {
        pub const ROAD_DIRECTORY: &str = "../TestArt/TestRoad/";
        pub const TEST_STRING: &str = "***TESTING";
        pub const TEST_W3D_DIR_PATH: &str = "../TestArt/";
        pub const TEST_TGA_DIR_PATH: &str = "../TestArt/";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Mock file system backend for testing
    struct MockFileSystemBackend {
        files: HashSet<String>,
    }

    impl MockFileSystemBackend {
        fn new() -> Self {
            let mut files = HashSet::new();
            files.insert("test.txt".to_string());
            files.insert("data/config.ini".to_string());

            Self { files }
        }
    }

    impl FileSystemBackend for MockFileSystemBackend {
        fn identifier(&self) -> &'static str {
            "mock"
        }

        fn init(&mut self) {}
        fn reset(&mut self) {}
        fn update(&mut self) {}

        fn open_file(&mut self, filename: &str, _access: FileAccess) -> Option<Box<dyn File>> {
            if self.files.contains(filename) {
                // Return a mock file implementation
                None // For this test, we'll just return None
            } else {
                None
            }
        }

        fn does_file_exist(&self, filename: &str) -> bool {
            self.files.contains(filename)
        }

        fn get_file_list_in_directory(
            &self,
            _base_path: &AsciiString,
            _directory: &AsciiString,
            _search_name: &AsciiString,
            _filename_list: &mut FilenameList,
            _search_subdirectories: bool,
        ) {
            // Mock implementation
        }

        fn get_file_info(&self, _filename: &AsciiString) -> Option<FileInfo> {
            None
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_file_system_creation() {
        let fs = FileSystem::new();
        assert_eq!(fs.backend_count(), 0);
    }

    #[test]
    fn test_file_system_backend_registration() {
        let mut fs = FileSystem::new();
        let backend = Box::new(MockFileSystemBackend::new());

        fs.register_backend(backend);
        assert_eq!(fs.backend_count(), 1);
    }

    #[test]
    fn test_file_existence_check() {
        let mut fs = FileSystem::new();
        let backend = Box::new(MockFileSystemBackend::new());

        fs.register_backend(backend);

        assert!(fs.does_file_exist("test.txt"));
        assert!(!fs.does_file_exist("nonexistent.txt"));

        // Test caching - second call should use cache
        assert!(fs.does_file_exist("test.txt"));
        assert!(!fs.does_file_exist("nonexistent.txt"));
    }

    #[test]
    fn test_ensure_backend() {
        let mut fs = FileSystem::new();

        {
            let backend: &mut MockFileSystemBackend = fs.ensure_backend(MockFileSystemBackend::new);
            backend.files.insert("extra.txt".to_string());
        }

        assert_eq!(fs.backend_count(), 1);
        assert!(fs.does_file_exist("extra.txt"));

        // Calling ensure_backend again should return the existing backend
        let backend_again: &mut MockFileSystemBackend =
            fs.ensure_backend(MockFileSystemBackend::new);
        assert!(backend_again.files.contains("extra.txt"));
    }

    #[test]
    fn test_filename_to_key() {
        NameKeyGenerator::reset();
        let fs = FileSystem::new();

        let key1 = fs.filename_to_key("test.txt");
        let key2 = fs.filename_to_key("TEST.TXT");
        let key3 = fs.filename_to_key("different.txt");

        // Same filename (case insensitive) should produce same key
        assert_eq!(key1, key2);

        // Different filename should produce different key
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_file_info_default() {
        let info = FileInfo::default();
        assert_eq!(info.size_high, 0);
        assert_eq!(info.size_low, 0);
        assert_eq!(info.timestamp_high, 0);
        assert_eq!(info.timestamp_low, 0);
    }
}
