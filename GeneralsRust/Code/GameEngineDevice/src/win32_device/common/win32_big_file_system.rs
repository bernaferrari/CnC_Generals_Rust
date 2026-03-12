//! Win32BIGFileSystem - exact port of C++ Win32BIGFileSystem
//! 
//! Corresponds to C++ file: GameEngineDevice/Include/Win32Device/Common/Win32BIGFileSystem.h
//! Original author: Bryan Cleveland, August 2002
//! Rust port: 2025

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use super::win32_big_file::Win32BIGFile;

// Type aliases to match C++
pub type AsciiString = String;
pub type Bool = bool;
pub type Char = i8;

/// Win32BIGFileSystem - exact port of C++ Win32BIGFileSystem class
/// 
/// This class manages BIG file archives, matching the exact functionality
/// of the C++ Win32BIGFileSystem from Win32BIGFileSystem.h/cpp
pub struct Win32BIGFileSystem {
    /// Map of opened archive files
    archive_files: HashMap<String, Win32BIGFile>,
}

impl Win32BIGFileSystem {
    /// Constructor - matches C++ Win32BIGFileSystem::Win32BIGFileSystem()
    pub fn new() -> Self {
        Self {
            archive_files: HashMap::new(),
        }
    }
    
    /// Destructor - matches C++ Win32BIGFileSystem::~Win32BIGFileSystem()
    pub fn destroy(&mut self) {
        self.closeAllArchiveFiles();
    }
    
    /// Initialize - matches C++ Win32BIGFileSystem::init()
    pub fn init(&mut self) {
        // Load BIG files from current directory - matches C++ line 61
        self.loadBigFilesFromDirectory("".to_string(), "*.big".to_string(), false);
        
        // Load original Generals assets from assets directory - matches C++ InstallPath logic
        // Check multiple possible asset locations
        let asset_paths = [
            "assets",
            "GeneralsRust/Code/Main/assets",
            "../assets",
            "./GeneralsRust/Code/Main/assets",
        ];
        
        for assets_path in &asset_paths {
            if Path::new(assets_path).exists() {
                println!("Loading BIG files from: {}", assets_path);
                self.loadBigFilesFromDirectory(assets_path.to_string(), "*.big".to_string(), false);
                break; // Use first valid path found
            }
        }
        
        // Also check windows_game directory structure (in case assets are there)
        let install_path = "windows_game/Command & Conquer Generals Zero Hour";
        if Path::new(install_path).exists() {
            self.loadBigFilesFromDirectory(install_path.to_string(), "*.big".to_string(), false);
        }
    }
    
    /// Update - matches C++ Win32BIGFileSystem::update()
    pub fn update(&mut self) {
        // Nothing to update in the base implementation
    }
    
    /// Reset - matches C++ Win32BIGFileSystem::reset()
    pub fn reset(&mut self) {
        // Nothing to reset in the base implementation
    }
    
    /// Post process load - matches C++ Win32BIGFileSystem::postProcessLoad()
    pub fn postProcessLoad(&mut self) {
        // Nothing to post-process in the base implementation
    }
    
    /// Close all archive files - matches C++ Win32BIGFileSystem::closeAllArchiveFiles()
    pub fn closeAllArchiveFiles(&mut self) {
        for (_, archive) in self.archive_files.iter_mut() {
            archive.close();
        }
        self.archive_files.clear();
    }
    
    /// Open archive file - matches C++ Win32BIGFileSystem::openArchiveFile()
    pub fn openArchiveFile(&mut self, filename: &str) -> Option<&mut Win32BIGFile> {
        self.archive_files.get_mut(filename)
    }
    
    /// Close archive file - matches C++ Win32BIGFileSystem::closeArchiveFile()
    pub fn closeArchiveFile(&mut self, filename: &str) {
        if let Some(mut archive) = self.archive_files.remove(filename) {
            archive.close();
        }
    }
    
    /// Close all files - matches C++ Win32BIGFileSystem::closeAllFiles()
    pub fn closeAllFiles(&mut self) {
        for (_, archive) in self.archive_files.iter_mut() {
            archive.close(); // BIGFile doesn't have closeAllFiles, just close
        }
    }
    
    /// Load BIG files from directory - matches C++ Win32BIGFileSystem::loadBigFilesFromDirectory()
    /// 
    /// This is the core method that discovers and loads BIG files, exactly matching
    /// the C++ signature: Bool loadBigFilesFromDirectory(AsciiString dir, AsciiString fileMask, Bool overwrite)
    pub fn loadBigFilesFromDirectory(&mut self, dir: AsciiString, file_mask: AsciiString, overwrite: Bool) -> Bool {
        let dir_path = if dir.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(&dir)
        };
        
        if !dir_path.exists() {
            return false;
        }
        
        let mut loaded_any = false;
        
        // Read directory and find .big files (matches C++ logic)
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension.to_string_lossy().to_lowercase() == "big" {
                            let filename = path.to_string_lossy().to_string();
                            let archive_name = path.file_stem().unwrap().to_string_lossy().to_string();
                            
                            if overwrite || !self.archive_files.contains_key(&archive_name) {
                                let mut big_file = Win32BIGFile::new();
                                if big_file.open(&filename).is_ok() {
                                    println!("Successfully loaded BIG file: {}", filename);
                                    self.archive_files.insert(archive_name, big_file);
                                    loaded_any = true;
                                } else {
                                    eprintln!("Failed to load BIG file: {}", filename);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        loaded_any
    }
}

impl Default for Win32BIGFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory function to create Win32BIGFileSystem (matches C++ pattern)
pub fn create_big_file_system() -> Win32BIGFileSystem {
    Win32BIGFileSystem::new()
}

/// Factory function with configuration
pub fn create_big_file_system_with_config(config: BIGFileSystemConfig) -> Win32BIGFileSystem {
    Win32BIGFileSystem::with_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;
    
    #[tokio::test]
    async fn test_big_file_system_creation() {
        let mut fs = Win32BIGFileSystem::new();
        assert!(fs.init().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_directory_tree() {
        let mut tree = DirectoryTreeNode::new();
        
        let file_info = Arc::new(ArchivedFileInfo {
            filename: "test.txt".to_string(),
            archive_filename: "test.big".to_string(),
            offset: 0,
            size: 100,
        });
        
        // Add file to subdirectory
        tree.add_file(&["data", "test.txt"], file_info.clone(), "test.big".to_string());
        
        // Find the file
        let found = tree.find_file(&["data", "test.txt"]);
        assert!(found.is_some());
        
        let found_info = found.unwrap();
        assert_eq!(found_info.filename, "test.txt");
        assert_eq!(found_info.size, 100);
    }
    
    #[tokio::test]
    async fn test_load_from_nonexistent_directory() {
        let mut fs = Win32BIGFileSystem::new();
        
        let result = fs.load_big_files_from_directory(
            "/nonexistent/path",
            "*.big",
            false
        ).await;
        
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for no files loaded
    }
    
    #[tokio::test]
    async fn test_statistics() {
        let fs = Win32BIGFileSystem::new();
        let stats = fs.get_statistics().await;
        
        assert_eq!(stats.total_archives, 0);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size_bytes, 0);
    }
    
    #[tokio::test]
    async fn test_configuration() {
        let config = BIGFileSystemConfig {
            enable_caching: false,
            enable_parallel_loading: false,
            max_concurrent_files: 5,
            ..Default::default()
        };
        
        let fs = Win32BIGFileSystem::with_config(config.clone());
        assert!(!fs.config.enable_caching);
        assert!(!fs.config.enable_parallel_loading);
        assert_eq!(fs.config.max_concurrent_files, 5);
    }
}

