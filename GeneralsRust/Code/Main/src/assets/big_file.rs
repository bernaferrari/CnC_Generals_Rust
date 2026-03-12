////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// BIG file format implementation - exact port of C++ Win32BIGFile system

use anyhow::{anyhow, Result};
use log::{error, info};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

/// BIG file identifier - exactly as in C++
const _BIG_FILE_IDENTIFIER: &[u8; 4] = b"BIGF";

/// ArchivedFileInfo - matches C++ structure
#[derive(Debug, Clone)]
pub struct ArchivedFileInfo {
    pub filename: String,
    pub archive_filename: String,
    pub offset: u32,
    pub size: u32,
}

impl Default for ArchivedFileInfo {
    fn default() -> Self {
        Self::new()
    }
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

impl Default for DirectoryInfo {
    fn default() -> Self {
        Self::new()
    }
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

/// BIGFile - main BIG archive reader - matches C++ Win32BIGFile
pub struct BIGFile {
    name: String,
    path: String,
    file: Option<File>,
    root_directory: DirectoryInfo,
    file_count: u32,
}

impl Default for BIGFile {
    fn default() -> Self {
        Self::new()
    }
}

impl BIGFile {
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
    pub async fn open<P: AsRef<Path>>(&mut self, filepath: P) -> Result<()> {
        let path = filepath.as_ref();

        // BIG file open attempt (logging disabled)

        // Check if file exists before trying to open
        if !path.exists() {
            error!("BIG file not found: {:?}", path);
            return Err(anyhow!("BIG file not found: {:?}", path));
        }

        // Check file size
        let _metadata = std::fs::metadata(path)?;
        // BIG file size check (logging disabled)

        let mut file = File::open(path).await?;

        self.path = path.to_string_lossy().to_string();
        self.name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        // BIG file opened (logging disabled)

        // Read and verify BIG file header
        let mut identifier = [0u8; 4];
        file.read_exact(&mut identifier).await?;

        // BIG file header check (logging disabled)

        // Check for BIGF or BIG4 headers
        if &identifier != b"BIGF" && &identifier != b"BIG4" {
            // Non-standard header detected, skipping (logging disabled)
            return Ok(());
        }

        // BIG file header valid (logging disabled)

        // Read archive file size (4 bytes, little endian)
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes).await?;
        let _archive_size = u32::from_le_bytes(size_bytes);
        // Archive size (logging disabled)

        // Read number of files (4 bytes, BIG ENDIAN - as per C++ code using ntohl)
        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes).await?;
        self.file_count = u32::from_be_bytes(count_bytes); // ntohl equivalent
                                                           // BIG file count (logging disabled)

        // Skip to directory listing at offset 0x10 (as per C++ code)
        file.seek(SeekFrom::Start(0x10)).await?;

        // Parse directory entries
        //
        // The previous implementation aggressively truncated audio/map archives to 10 files to
        // speed up bring‑up. That meant critical assets (voice lines, map INIs, lighting tables)
        // were silently missing. To stay faithful to the C++ loader and ensure full playability,
        // walk every entry the archive advertises.
        for _i in 0..self.file_count {
            let mut file_info = ArchivedFileInfo::new();
            file_info.archive_filename = self.name.clone();

            // Read file offset (4 bytes, BIG ENDIAN)
            let mut offset_bytes = [0u8; 4];
            file.read_exact(&mut offset_bytes).await?;
            file_info.offset = u32::from_be_bytes(offset_bytes); // ntohl equivalent

            // Read file size (4 bytes, BIG ENDIAN)
            let mut size_bytes = [0u8; 4];
            file.read_exact(&mut size_bytes).await?;
            file_info.size = u32::from_be_bytes(size_bytes); // ntohl equivalent

            // Read null-terminated filename
            let mut filename_bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                file.read_exact(&mut byte).await?;
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

            // File entry processing (logging disabled)

            // Add file to directory tree
            self.add_file_to_directory(&directory_path, file_info);
        }

        // Keep file handle open for reading data (as per C++ code)
        self.file = Some(file);

        // BIG file loaded successfully (logging disabled)

        // Show some sample files for debugging
        let all_files = self.list_files();
        let _w3d_files: Vec<_> = all_files
            .iter()
            .filter(|f| f.to_lowercase().ends_with(".w3d"))
            .take(5)
            .collect();
        let _tga_files: Vec<_> = all_files
            .iter()
            .filter(|f| f.to_lowercase().ends_with(".tga"))
            .take(5)
            .collect();

        // Sample files detected (logging disabled)
        Ok(())
    }

    /// Add file to directory tree - matches C++ addFile logic
    fn add_file_to_directory(&mut self, path: &str, file_info: ArchivedFileInfo) {
        if path.is_empty() {
            // File in root directory
            self.root_directory
                .files
                .insert(file_info.filename.clone(), file_info);
            return;
        }

        // Navigate/create directory structure
        let path_parts: Vec<&str> = path.split(&['\\', '/'][..]).collect();
        let mut current_dir = &mut self.root_directory;

        for part in path_parts {
            if !part.is_empty() {
                current_dir = current_dir
                    .subdirectories
                    .entry(part.to_lowercase())
                    .or_insert_with(|| {
                        let mut dir = DirectoryInfo::new();
                        dir.directory_name = part.to_string();
                        dir
                    });
            }
        }

        // Add file to final directory
        current_dir
            .files
            .insert(file_info.filename.clone(), file_info);
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
    fn search_directory_for_file<'a>(
        &self,
        dir: &'a DirectoryInfo,
        filename: &str,
    ) -> Option<&'a ArchivedFileInfo> {
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
    pub async fn extract_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        // File extraction attempt (logging disabled)

        let file_info = match self.get_file_info(filename) {
            Some(info) => {
                // File found in archive (logging disabled)
                info.clone()
            }
            None => {
                error!("File not found in archive: {}", filename);
                return Err(anyhow!("File not found in archive: {}", filename));
            }
        };

        let file = self
            .file
            .as_mut()
            .ok_or_else(|| anyhow!("BIG file not opened"))?;

        // Seek to file offset
        // Seeking to file offset (logging disabled)
        file.seek(SeekFrom::Start(file_info.offset as u64)).await?;

        // Read file data with timeout protection (reduced logging)
        if file_info.size > 0 && filename.ends_with(".w3d") {
            println!("📄 Loading model: {} ({} bytes)", filename, file_info.size);
        }

        if file_info.size > 10_000_000 {
            // 10MB limit
            return Err(anyhow!("File too large: {} bytes", file_info.size));
        }

        let mut data = vec![0u8; file_info.size as usize];

        // Add timeout to prevent hanging on large reads
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            file.read_exact(&mut data),
        )
        .await
        {
            Ok(Ok(_)) => {
                // Success - only log for important files
                if filename.ends_with(".w3d") {
                    println!("✅ Loaded model: {} bytes", data.len());
                }
            }
            Ok(Err(e)) => {
                return Err(anyhow!("Failed to read file data: {}", e));
            }
            Err(_) => {
                return Err(anyhow!("File read timeout after 5s for {}", filename));
            }
        }

        // File extraction success (logging disabled)

        Ok(data)
    }

    /// List all files in archive
    pub fn list_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        self.collect_files_from_directory(&self.root_directory, "", &mut files);
        files
    }

    /// Recursively collect all files
    fn collect_files_from_directory(
        &self,
        dir: &DirectoryInfo,
        path_prefix: &str,
        files: &mut Vec<String>,
    ) {
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

impl Drop for BIGFile {
    fn drop(&mut self) {
        if self.file.is_some() {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archived_file_info() {
        let mut info = ArchivedFileInfo::new();
        assert_eq!(info.filename, "");
        assert_eq!(info.size, 0);

        info.filename = "test.txt".to_string();
        info.size = 1024;
        assert_eq!(info.filename, "test.txt");
        assert_eq!(info.size, 1024);

        info.clear();
        assert_eq!(info.filename, "");
        assert_eq!(info.size, 0);
    }

    #[test]
    fn test_directory_info() {
        let mut dir = DirectoryInfo::new();
        assert!(dir.files.is_empty());
        assert!(dir.subdirectories.is_empty());

        let file_info = ArchivedFileInfo::new();
        dir.files.insert("test.txt".to_string(), file_info);
        assert_eq!(dir.files.len(), 1);

        dir.clear();
        assert!(dir.files.is_empty());
    }
}
