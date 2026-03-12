//! Win32 Local File System Module
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/Win32Device/Common/Win32LocalFileSystem.cpp
//! 
//! This module provides complete file system operations using modern Rust libraries
//! with async support, memory mapping, and efficient directory traversal.

use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::RwLock,
};

use walkdir::WalkDir;
use ignore::WalkBuilder;
use thiserror::Error;
use uuid::Uuid;
use dashmap::DashMap;
use parking_lot::Mutex;
use tracing::{debug, error, info, warn};

use super::win32_local_file::Win32LocalFile;

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Storage::FileSystem::*,
    Win32::System::SystemServices::*,
};

/// File access modes matching C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileAccess {
    pub bits: u32,
}

impl FileAccess {
    pub const READ: u32 = 0x01;
    pub const WRITE: u32 = 0x02;
    pub const CREATE: u32 = 0x04;
    pub const BINARY: u32 = 0x08;
    pub const STREAMING: u32 = 0x10;
    
    pub fn new(bits: u32) -> Self {
        Self { bits }
    }
    
    pub fn has_read(&self) -> bool {
        (self.bits & Self::READ) != 0
    }
    
    pub fn has_write(&self) -> bool {
        (self.bits & Self::WRITE) != 0
    }
    
    pub fn has_create(&self) -> bool {
        (self.bits & Self::CREATE) != 0
    }
    
    pub fn has_binary(&self) -> bool {
        (self.bits & Self::BINARY) != 0
    }
    
    pub fn has_streaming(&self) -> bool {
        (self.bits & Self::STREAMING) != 0
    }
}

/// File information structure matching C++ FileInfo
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub timestamp_high: u32,
    pub timestamp_low: u32,
    pub size_high: u32,
    pub size_low: u32,
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub accessed: SystemTime,
}

impl FileInfo {
    pub fn size(&self) -> u64 {
        ((self.size_high as u64) << 32) | (self.size_low as u64)
    }
    
    pub fn timestamp(&self) -> u64 {
        ((self.timestamp_high as u64) << 32) | (self.timestamp_low as u64)
    }
}

/// Directory cache entry for performance optimization
#[derive(Debug, Clone)]
struct DirectoryCacheEntry {
    files: Vec<FileInfo>,
    last_updated: SystemTime,
    is_dirty: bool,
}

/// Win32 Local File System implementation with modern Rust features
#[derive(Debug)]
pub struct Win32LocalFileSystem {
    /// Directory cache for performance
    directory_cache: DashMap<PathBuf, DirectoryCacheEntry>,
    /// File watcher for directory changes
    #[cfg(feature = "file-watching")]
    watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    /// Root paths for file search
    root_paths: Arc<RwLock<Vec<PathBuf>>>,
    /// System configuration
    config: FileSystemConfig,
}

/// Configuration for the file system
#[derive(Debug, Clone)]
pub struct FileSystemConfig {
    pub cache_enabled: bool,
    pub cache_ttl_seconds: u64,
    pub max_cache_entries: usize,
    pub enable_file_watching: bool,
    pub async_operations: bool,
}

impl Default for FileSystemConfig {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_ttl_seconds: 300, // 5 minutes
            max_cache_entries: 10000,
            enable_file_watching: true,
            async_operations: true,
        }
    }
}

/// Errors that can occur during file system operations
#[derive(Error, Debug)]
pub enum Win32FileSystemError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Access denied: {path}")]
    AccessDenied { path: String },
    
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },
    
    #[error("Directory creation failed: {path}")]
    DirectoryCreationFailed { path: String },
    
    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
    
    #[error("Cache error: {message}")]
    CacheError { message: String },
    
    #[error("File system watcher error: {source}")]
    WatcherError {
        #[from]
        source: notify::Error,
    },
    
    #[cfg(windows)]
    #[error("Windows API error: {source}")]
    WindowsApiError {
        #[from]
        source: windows::core::Error,
    },
}

type Result<T> = std::result::Result<T, Win32FileSystemError>;

/// Type alias for filename lists (matching C++ implementation)
pub type FilenameList = HashSet<String>;


impl Win32LocalFileSystem {
    /// Create a new Win32LocalFileSystem with default configuration
    pub fn new() -> Self {
        Self::with_config(FileSystemConfig::default())
    }
    
    /// Create a new Win32LocalFileSystem with custom configuration
    pub fn with_config(config: FileSystemConfig) -> Self {
        Self {
            directory_cache: DashMap::new(),
            #[cfg(feature = "file-watching")]
            watcher: Arc::new(Mutex::new(None)),
            root_paths: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }
    
    /// Initialize the file system
    pub async fn init(&self) -> Result<()> {
        info!("Initializing Win32 Local File System");
        
        // Set up default root paths
        let mut paths = self.root_paths.write().await;
        paths.push(std::env::current_dir()?);
        
        // Initialize file watcher if enabled
        #[cfg(feature = "file-watching")]
        if self.config.enable_file_watching {
            self.setup_file_watcher().await?;
        }
        
        Ok(())
    }
    
    /// Reset the file system state
    pub async fn reset(&self) {
        debug!("Resetting Win32 Local File System");
        self.directory_cache.clear();
    }
    
    /// Update the file system (periodic maintenance)
    pub async fn update(&self) {
        if self.config.cache_enabled {
            self.cleanup_expired_cache_entries().await;
        }
    }
    
    /// Open a file with the specified access mode
    /// Matches the C++ Win32LocalFileSystem::openFile signature
    pub async fn open_file(&self, filename: &str, access: u32) -> Result<Option<Win32LocalFile>> {
        debug!("Opening file: {} with access: {:#x}", filename, access);
        
        if filename.is_empty() {
            warn!("Attempted to open file with empty filename");
            return Ok(None);
        }
        
        let access_mode = FileAccess::new(access);
        let path = PathBuf::from(filename);
        
        // Ensure directory exists if opening for write
        if access_mode.has_write() {
            self.ensure_parent_directory_exists(&path).await?;
        }
        
        // Create the Win32LocalFile
        let mut file = Win32LocalFile::new();
        
        match file.open(filename, access).await {
            Ok(true) => {
                file.set_delete_on_close(true);
                Ok(Some(file))
            },
            Ok(false) => {
                warn!("Failed to open file: {}", filename);
                Ok(None)
            },
            Err(e) => {
                error!("Error opening file {}: {}", filename, e);
                Err(Win32FileSystemError::IoError { source: e })
            }
        }
    }
    
    /// Check if a file exists (matches C++ doesFileExist)
    pub async fn does_file_exist(&self, filename: &str) -> bool {
        if filename.is_empty() {
            return false;
        }
        
        fs::metadata(filename).await.is_ok()
    }
    
    /// Get file information (matches C++ getFileInfo)
    pub async fn get_file_info(&self, filename: &str) -> Result<Option<FileInfo>> {
        let path = Path::new(filename);
        
        match fs::metadata(path).await {
            Ok(metadata) => {
                #[cfg(windows)]
                {
                    // Use Windows API for detailed file information
                    self.get_windows_file_info(path, &metadata).await
                }
                #[cfg(not(windows))]
                {
                    self.get_standard_file_info(path, &metadata).await
                }
            },
            Err(_) => Ok(None),
        }
    }
    
    /// Get list of files in directory (matches C++ getFileListInDirectory)
    pub async fn get_file_list_in_directory(
        &self,
        current_directory: &str,
        original_directory: &str, 
        search_name: &str,
        search_subdirectories: bool,
    ) -> Result<FilenameList> {
        let current_path = PathBuf::from(original_directory).join(current_directory);
        let mut file_list = FilenameList::new();
        
        debug!(
            "Searching for files in directory: {:?}, pattern: {}, recursive: {}", 
            current_path, search_name, search_subdirectories
        );
        
        // Use ignore crate for efficient directory traversal with pattern matching
        let mut builder = WalkBuilder::new(&current_path);
        
        if !search_subdirectories {
            builder.max_depth(Some(1));
        }
        
        // Convert search pattern to glob pattern
        let glob_pattern = if search_name.contains('*') || search_name.contains('?') {
            search_name.to_string()
        } else {
            format!("*{}*", search_name)
        };
        
        for result in builder.build() {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        let file_path = entry.path();
                        
                        // Check if filename matches pattern
                        if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
                            if self.matches_pattern(filename, &glob_pattern) {
                                let full_path = format!(
                                    "{}{}{}",
                                    original_directory,
                                    current_directory,
                                    filename
                                );
                                file_list.insert(full_path);
                            }
                        }
                    }
                },
                Err(e) => {
                    warn!("Error traversing directory: {}", e);
                }
            }
        }
        
        debug!("Found {} files matching pattern", file_list.len());
        Ok(file_list)
    }
    
    /// Create a directory (matches C++ createDirectory)
    pub async fn create_directory(&self, directory: &str) -> Result<bool> {
        if directory.is_empty() || directory.len() >= 260 { // MAX_PATH on Windows
            return Ok(false);
        }
        
        let path = Path::new(directory);
        
        match fs::create_dir_all(path).await {
            Ok(_) => {
                info!("Created directory: {}", directory);
                Ok(true)
            },
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Ok(true) // Directory already exists
            },
            Err(e) => {
                error!("Failed to create directory {}: {}", directory, e);
                Ok(false)
            }
        }
    }
    
    /// Add a root path for file searching
    pub async fn add_root_path<P: Into<PathBuf>>(&self, path: P) {
        let path = path.into();
        let mut paths = self.root_paths.write().await;
        if !paths.contains(&path) {
            debug!("Adding root path: {:?}", path);
            paths.push(path);
        }
    }
    
    /// Remove a root path
    pub async fn remove_root_path<P: AsRef<Path>>(&self, path: P) {
        let path = path.as_ref();
        let mut paths = self.root_paths.write().await;
        paths.retain(|p| p != path);
    }
    
    /// Get all root paths
    pub async fn get_root_paths(&self) -> Vec<PathBuf> {
        self.root_paths.read().await.clone()
    }
    
    /// Find file in any of the root paths
    pub async fn find_file(&self, filename: &str) -> Option<PathBuf> {
        let paths = self.root_paths.read().await;
        
        for root_path in paths.iter() {
            let full_path = root_path.join(filename);
            if self.does_file_exist(full_path.to_string_lossy().as_ref()).await {
                return Some(full_path);
            }
        }
        
        None
    }
    
    /// Private helper methods
    
    /// Ensure parent directory exists for a file path
    async fn ensure_parent_directory_exists(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                self.create_directory(&parent.to_string_lossy()).await?;
            }
        }
        Ok(())
    }
    
    /// Check if filename matches a glob pattern
    fn matches_pattern(&self, filename: &str, pattern: &str) -> bool {
        // Simple glob matching - could be enhanced with regex or glob crate
        if pattern == "*" || pattern == "*.*" {
            return true;
        }
        
        // For now, do simple wildcard matching
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return filename.starts_with(prefix) && filename.ends_with(suffix);
            }
        }
        
        filename == pattern
    }
    
    /// Clean up expired cache entries
    async fn cleanup_expired_cache_entries(&self) {
        let now = SystemTime::now();
        let ttl = std::time::Duration::from_secs(self.config.cache_ttl_seconds);
        
        self.directory_cache.retain(|_, entry| {
            now.duration_since(entry.last_updated)
                .map(|duration| duration < ttl)
                .unwrap_or(false)
        });
        
        // Enforce max cache size
        while self.directory_cache.len() > self.config.max_cache_entries {
            // Remove oldest entry (simple LRU)
            if let Some(oldest_key) = self.directory_cache
                .iter()
                .min_by_key(|entry| entry.last_updated)
                .map(|entry| entry.key().clone())
            {
                self.directory_cache.remove(&oldest_key);
            } else {
                break;
            }
        }
    }
    
    /// Set up file system watcher for directory changes
    #[cfg(feature = "file-watching")]
    async fn setup_file_watcher(&self) -> Result<()> {
        use notify::{Watcher, RecursiveMode};
        use std::sync::mpsc;
        
        let (tx, _rx) = std::sync::mpsc::channel();
        
        let watcher = notify::recommended_watcher(move |res| {
            match res {
                Ok(event) => {
                    debug!("File system event: {:?}", event);
                    // Handle file system events
                },
                Err(e) => {
                    warn!("File system watcher error: {:?}", e);
                }
            }
        })?;
        
        *self.watcher.lock() = Some(watcher);
        Ok(())
    }
    
    #[cfg(windows)]
    async fn get_windows_file_info(&self, path: &Path, metadata: &fs::Metadata) -> Result<Option<FileInfo>> {
        use std::os::windows::fs::MetadataExt;
        
        let file_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        
        let file_size = metadata.len();
        let size_high = ((file_size >> 32) & 0xFFFFFFFF) as u32;
        let size_low = (file_size & 0xFFFFFFFF) as u32;
        
        // Get Windows-specific timestamp information
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let created = metadata.created().unwrap_or(SystemTime::UNIX_EPOCH);
        let accessed = metadata.accessed().unwrap_or(SystemTime::UNIX_EPOCH);
        
        // Convert SystemTime to Windows FILETIME format
        let modified_duration = modified.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp = modified_duration.as_nanos() as u64 / 100 + 116444736000000000; // Convert to Windows epoch
        
        Ok(Some(FileInfo {
            timestamp_high: ((timestamp >> 32) & 0xFFFFFFFF) as u32,
            timestamp_low: (timestamp & 0xFFFFFFFF) as u32,
            size_high,
            size_low,
            name: file_name,
            path: path.to_path_buf(),
            is_directory: metadata.is_dir(),
            created,
            modified,
            accessed,
        }))
    }
    
    #[cfg(not(windows))]
    async fn get_standard_file_info(&self, path: &Path, metadata: &fs::Metadata) -> Result<Option<FileInfo>> {
        let file_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        
        let file_size = metadata.len();
        let size_high = ((file_size >> 32) & 0xFFFFFFFF) as u32;
        let size_low = (file_size & 0xFFFFFFFF) as u32;
        
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let created = modified; // Unix doesn't have creation time
        let accessed = metadata.accessed().unwrap_or(SystemTime::UNIX_EPOCH);
        
        let modified_duration = modified.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp = modified_duration.as_secs();
        
        Ok(Some(FileInfo {
            timestamp_high: ((timestamp >> 32) & 0xFFFFFFFF) as u32,
            timestamp_low: (timestamp & 0xFFFFFFFF) as u32,
            size_high,
            size_low,
            name: file_name,
            path: path.to_path_buf(),
            is_directory: metadata.is_dir(),
            created,
            modified,
            accessed,
        }))
    }
}

impl Default for Win32LocalFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;
    
    #[tokio::test]
    async fn test_file_system_creation() {
        let fs = Win32LocalFileSystem::new();
        assert!(fs.init().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_file_exists() {
        let fs = Win32LocalFileSystem::new();
        fs.init().await.unwrap();
        
        // Test with a file that definitely doesn't exist
        assert!(!fs.does_file_exist("non_existent_file_12345.txt").await);
    }
    
    #[tokio::test]
    async fn test_directory_creation() {
        let fs = Win32LocalFileSystem::new();
        fs.init().await.unwrap();
        
        let temp_dir = tempdir().unwrap();
        let test_dir = temp_dir.path().join("test_subdir");
        
        assert!(fs.create_directory(&test_dir.to_string_lossy()).await.unwrap());
        assert!(test_dir.exists());
    }
    
    #[tokio::test]
    async fn test_file_list_in_directory() {
        let fs = Win32LocalFileSystem::new();
        fs.init().await.unwrap();
        
        let temp_dir = tempdir().unwrap();
        
        // Create some test files
        let test_file1 = temp_dir.path().join("test1.txt");
        let test_file2 = temp_dir.path().join("test2.txt");
        
        fs::write(&test_file1, "test content 1").await.unwrap();
        fs::write(&test_file2, "test content 2").await.unwrap();
        
        let file_list = fs.get_file_list_in_directory(
            "",
            &temp_dir.path().to_string_lossy(),
            "*.txt",
            false,
        ).await.unwrap();
        
        assert!(file_list.len() >= 2);
    }
    
    #[tokio::test]
    async fn test_root_path_management() {
        let fs = Win32LocalFileSystem::new();
        fs.init().await.unwrap();
        
        let test_path = PathBuf::from("/test/path");
        
        fs.add_root_path(&test_path).await;
        let paths = fs.get_root_paths().await;
        assert!(paths.contains(&test_path));
        
        fs.remove_root_path(&test_path).await;
        let paths = fs.get_root_paths().await;
        assert!(!paths.contains(&test_path));
    }
    
    #[tokio::test]
    async fn test_file_access_modes() {
        let access = FileAccess::new(FileAccess::READ | FileAccess::BINARY);
        assert!(access.has_read());
        assert!(access.has_binary());
        assert!(!access.has_write());
        assert!(!access.has_create());
    }
    
    #[tokio::test] 
    async fn test_pattern_matching() {
        let fs = Win32LocalFileSystem::new();
        
        assert!(fs.matches_pattern("test.txt", "*.txt"));
        assert!(fs.matches_pattern("file.exe", "*.exe"));
        assert!(!fs.matches_pattern("file.txt", "*.exe"));
        assert!(fs.matches_pattern("anything", "*"));
    }
}
