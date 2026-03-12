//! Win32BIGFile - exact port of C++ Win32BIGFile
//! 
//! Corresponds to C++ file: GameEngineDevice/Include/Win32Device/Common/Win32BIGFile.h
//! Original author: Bryan Cleveland, August 2002
//! Rust port: 2025

use anyhow::{anyhow, Result};
use log::{info, debug, error};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// BIG file identifier - exactly as in C++
const BIG_FILE_IDENTIFIER: &[u8; 4] = b"BIGF";

/// ArchivedFileInfo - matches C++ structure
#[derive(Debug, Clone)]
pub struct ArchivedFileInfo {
    pub filename: String,
    pub archive_filename: String,
    pub offset: u32,
    pub size: u32,
}

impl ArchivedFileInfo {
    pub fn new() -> Self {
        Self {
            filename: String::new(),
            archive_filename: String::new(),
            offset: 0,
            size: 0,
        }
    }

    pub fn clear(&mut self) {
        self.filename.clear();
        self.archive_filename.clear();
        self.offset = 0;
        self.size = 0;
    }
}

/// Directory info structure - matches C++
#[derive(Debug)]
pub struct DirectoryInfo {
    pub directory_name: String,
    pub files: HashMap<String, ArchivedFileInfo>,
    pub subdirectories: HashMap<String, DirectoryInfo>,
}

impl DirectoryInfo {
    pub fn new() -> Self {
        Self {
            directory_name: String::new(),
            files: HashMap::new(),
            subdirectories: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.directory_name.clear();
        self.files.clear();
        self.subdirectories.clear();
    }
}

/// Win32BIGFile - main BIG archive reader - matches C++ Win32BIGFile
pub struct Win32BIGFile {
    name: String,
    path: String,
    file: Option<File>,
    root_directory: DirectoryInfo,
    file_count: u32,
}

impl Win32BIGFile {
    /// Constructor - matches C++ Win32BIGFile::Win32BIGFile()
    pub fn new() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            file: None,
            root_directory: DirectoryInfo::new(),
            file_count: 0,
        }
    }

    /// Open and parse BIG file - based on C++ Win32BIGFileSystem::openArchiveFile
    pub fn open<P: AsRef<Path>>(&mut self, filepath: P) -> Result<()> {
        let path = filepath.as_ref();
        let mut file = File::open(path)?;
        
        self.path = path.to_string_lossy().to_string();
        self.name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        info!("Opening BIG file: {}", self.path);

        // Read and verify BIG file header
        let mut identifier = [0u8; 4];
        file.read_exact(&mut identifier)?;
        
        if &identifier != BIG_FILE_IDENTIFIER {
            return Err(anyhow!(
                "Invalid BIG file identifier in {}: expected BIGF, got {:?}",
                self.path, 
                std::str::from_utf8(&identifier).unwrap_or("invalid")
            ));
        }

        // Read archive file size (4 bytes, little endian)
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let archive_size = u32::from_le_bytes(size_bytes);
        debug!("Archive size: {} bytes", archive_size);

        // Read number of files (4 bytes, BIG ENDIAN - as per C++ code using ntohl)
        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes)?;
        self.file_count = u32::from_be_bytes(count_bytes); // ntohl equivalent
        info!("Number of files in archive: {}", self.file_count);

        // Skip to directory listing at offset 0x10 (as per C++ code)
        file.seek(SeekFrom::Start(0x10))?;

        // Parse directory entries
        for i in 0..self.file_count {
            let mut file_info = ArchivedFileInfo::new();
            file_info.archive_filename = self.name.clone();

            // Read file offset (4 bytes, BIG ENDIAN)
            let mut offset_bytes = [0u8; 4];
            file.read_exact(&mut offset_bytes)?;
            file_info.offset = u32::from_be_bytes(offset_bytes); // ntohl equivalent

            // Read file size (4 bytes, BIG ENDIAN)
            let mut size_bytes = [0u8; 4];
            file.read_exact(&mut size_bytes)?;
            file_info.size = u32::from_be_bytes(size_bytes); // ntohl equivalent

            // Read null-terminated filename
            let mut filename_bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                file.read_exact(&mut byte)?;
                if byte[0] == 0 {
                    break;
                }
                filename_bytes.push(byte[0]);
            }

            let full_path = String::from_utf8_lossy(&filename_bytes).to_string();
            
            // Extract filename from path (same logic as C++)
            let mut filename_start = full_path.len();
            for (i, ch) in full_path.char_indices().rev() {
                if ch == '\\' || ch == '/' {
                    filename_start = i + 1;
                    break;
                }
            }
            
            file_info.filename = full_path[filename_start..].to_lowercase();
            
            // Extract directory path
            let directory_path = if filename_start > 0 {
                full_path[..filename_start - 1].to_string()
            } else {
                String::new()
            };

            debug!("File {}: {} ({} bytes at offset {})", 
                   i, file_info.filename, file_info.size, file_info.offset);

            // Add file to directory tree
            self.add_file_to_directory(&directory_path, file_info);
        }

        // Keep file handle open for reading data (as per C++ code)
        self.file = Some(file);
        
        info!("Successfully loaded BIG file {} with {} files", self.name, self.file_count);
        Ok(())
    }

    /// Add file to directory tree - matches C++ addFile logic
    fn add_file_to_directory(&mut self, path: &str, file_info: ArchivedFileInfo) {
        if path.is_empty() {
            // File in root directory
            self.root_directory.files.insert(file_info.filename.clone(), file_info);
            return;
        }

        // Navigate/create directory structure
        let path_parts: Vec<&str> = path.split(&['\\', '/'][..]).collect();
        let mut current_dir = &mut self.root_directory;

        for part in path_parts {
            if !part.is_empty() {
                current_dir = current_dir.subdirectories
                    .entry(part.to_lowercase())
                    .or_insert_with(|| {
                        let mut dir = DirectoryInfo::new();
                        dir.directory_name = part.to_string();
                        dir
                    });
            }
        }

        // Add file to final directory
        current_dir.files.insert(file_info.filename.clone(), file_info);
    }

    /// Get file info - matches C++ getArchivedFileInfo
    pub fn get_file_info(&self, filename: &str) -> Option<&ArchivedFileInfo> {
        let filename_lower = filename.to_lowercase();
        
        // Search in root directory first
        if let Some(file_info) = self.root_directory.files.get(&filename_lower) {
            return Some(file_info);
        }

        // Search in subdirectories recursively
        self.search_directory_for_file(&self.root_directory, &filename_lower)
    }

    /// Recursively search directories for file
    fn search_directory_for_file<'a>(&self, dir: &'a DirectoryInfo, filename: &str) -> Option<&'a ArchivedFileInfo> {
        // Check files in current directory
        if let Some(file_info) = dir.files.get(filename) {
            return Some(file_info);
        }

        // Search subdirectories
        for subdir in dir.subdirectories.values() {
            if let Some(file_info) = self.search_directory_for_file(subdir, filename) {
                return Some(file_info);
            }
        }

        None
    }

    /// Extract file data - matches C++ openFile logic
    pub fn extract_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        let file_info = self.get_file_info(filename)
            .ok_or_else(|| anyhow!("File not found in archive: {}", filename))?
            .clone(); // Clone to avoid borrow checker issues

        let file = self.file.as_mut()
            .ok_or_else(|| anyhow!("BIG file not opened"))?;

        // Seek to file offset
        file.seek(SeekFrom::Start(file_info.offset as u64))?;

        // Read file data
        let mut data = vec![0u8; file_info.size as usize];
        file.read_exact(&mut data)?;

        debug!("Extracted {} bytes from {} at offset {}", 
               data.len(), filename, file_info.offset);

        Ok(data)
    }

    /// List all files in archive
    pub fn list_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        self.collect_files_from_directory(&self.root_directory, "", &mut files);
        files
    }

    /// Recursively collect all files
    fn collect_files_from_directory(&self, dir: &DirectoryInfo, path_prefix: &str, files: &mut Vec<String>) {
        // Add files from current directory
        for filename in dir.files.keys() {
            if path_prefix.is_empty() {
                files.push(filename.clone());
            } else {
                files.push(format!("{}/{}", path_prefix, filename));
            }
        }

        // Recursively process subdirectories
        for (dir_name, subdir) in &dir.subdirectories {
            let new_prefix = if path_prefix.is_empty() {
                dir_name.clone()
            } else {
                format!("{}/{}", path_prefix, dir_name)
            };
            self.collect_files_from_directory(subdir, &new_prefix, files);
        }
    }

    /// Get archive name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get archive path
    pub fn get_path(&self) -> &str {
        &self.path
    }

    /// Get file count
    pub fn get_file_count(&self) -> u32 {
        self.file_count
    }

    /// Close the archive
    pub fn close(&mut self) {
        self.file = None;
        self.root_directory.clear();
        self.file_count = 0;
        info!("Closed BIG file: {}", self.name);
    }
}

impl Drop for Win32BIGFile {
    fn drop(&mut self) {
        if self.file.is_some() {
            self.close();
        }
    }
}
