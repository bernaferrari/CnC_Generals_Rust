//! Win32 Local File Module
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/Win32Device/Common/Win32LocalFile.cpp
//! 
//! Modern async file implementation with memory mapping support and streaming capabilities.
//! Uses tokio for async operations and memmap2 for efficient large file handling.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
    fmt,
};

use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt, AsyncBufReadExt, BufReader, SeekFrom},
    sync::{RwLock, Mutex},
};

use memmap2::{Mmap, MmapOptions, MmapMut};
use thiserror::Error;
use uuid::Uuid;
use tracing::{debug, error, info, warn, instrument};

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
    pub const APPEND: u32 = 0x20;
    pub const TEXT: u32 = 0x40;
    
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
    
    pub fn has_append(&self) -> bool {
        (self.bits & Self::APPEND) != 0
    }
    
    pub fn has_text(&self) -> bool {
        (self.bits & Self::TEXT) != 0
    }
}

/// File seek modes matching C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSeekMode {
    /// Seek from current position
    Current = 0,
    /// Seek from beginning of file
    Start = 1,
    /// Seek from end of file
    End = 2,
}

impl From<FileSeekMode> for SeekFrom {
    fn from(mode: FileSeekMode) -> Self {
        match mode {
            FileSeekMode::Current => SeekFrom::Current(0),
            FileSeekMode::Start => SeekFrom::Start(0),
            FileSeekMode::End => SeekFrom::End(0),
        }
    }
}

/// File operation modes for different access patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperationMode {
    /// Standard file I/O
    Standard,
    /// Memory-mapped file for large files
    MemoryMapped,
    /// Streaming mode for sequential access
    Streaming,
}

/// File handle wrapper for different operation modes
#[derive(Debug)]
enum FileHandle {
    Standard(File),
    MemoryMapped {
        _file: File,
        mmap: Mmap,
        position: usize,
    },
    MemoryMappedMut {
        _file: File,
        mmap: MmapMut,
        position: usize,
    },
}

/// Win32 Local File errors
#[derive(Error, Debug)]
pub enum Win32FileError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Access denied: {path}")]
    AccessDenied { path: String },
    
    #[error("Invalid file mode for operation")]
    InvalidMode,
    
    #[error("File not open")]
    FileNotOpen,
    
    #[error("Invalid seek position: {position}")]
    InvalidSeekPosition { position: i64 },
    
    #[error("Memory mapping failed: {source}")]
    MemoryMappingFailed {
        #[from]
        source: std::io::Error,
    },
    
    #[error("UTF-8 conversion error: {source}")]
    Utf8Error {
        #[from]
        source: std::str::Utf8Error,
    },
    
    #[error("Parse error: {message}")]
    ParseError { message: String },
}

type Result<T> = std::result::Result<T, Win32FileError>;

/// Win32 Local File implementation with modern async capabilities
#[derive(Debug)]
pub struct Win32LocalFile {
    /// Unique identifier for this file instance
    id: Uuid,
    /// File path
    path: PathBuf,
    /// File handle
    handle: Option<FileHandle>,
    /// Current file position
    position: u64,
    /// File size
    size: u64,
    /// Access mode used to open the file
    access_mode: FileAccess,
    /// Operation mode
    operation_mode: FileOperationMode,
    /// Whether file should be deleted when closed
    delete_on_close: bool,
    /// File opened timestamp
    opened_at: Option<SystemTime>,
    /// Buffer for line reading
    line_buffer: Vec<u8>,
}

impl Win32LocalFile {
    /// Create a new Win32LocalFile instance
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            path: PathBuf::new(),
            handle: None,
            position: 0,
            size: 0,
            access_mode: FileAccess::new(0),
            operation_mode: FileOperationMode::Standard,
            delete_on_close: false,
            opened_at: None,
            line_buffer: Vec::new(),
        }
    }
    
    /// Open a file with specified access mode (matches C++ signature)
    #[instrument(skip(self), fields(file_id = %self.id))]
    pub async fn open(&mut self, filename: &str, access: u32) -> Result<bool> {
        debug!("Opening file: {} with access: {:#x}", filename, access);
        
        if filename.is_empty() {
            warn!("Attempted to open file with empty filename");
            return Ok(false);
        }
        
        self.path = PathBuf::from(filename);
        self.access_mode = FileAccess::new(access);
        self.opened_at = Some(SystemTime::now());
        
        // Determine operation mode based on file size and access pattern
        self.operation_mode = self.determine_operation_mode().await?;
        
        // Open file with appropriate options
        let file_result = self.create_file_handle().await;
        
        match file_result {
            Ok(handle) => {
                self.handle = Some(handle);
                self.position = 0;
                self.size = self.get_file_size().await?;
                
                info!(
                    "Successfully opened file: {} (size: {} bytes, mode: {:?})", 
                    filename, self.size, self.operation_mode
                );
                Ok(true)
            }
            Err(e) => {
                error!("Failed to open file {}: {}", filename, e);
                Ok(false)
            }
        }
    }
    
    /// Close the file
    #[instrument(skip(self), fields(file_id = %self.id))]
    pub async fn close(&mut self) {
        debug!("Closing file: {:?}", self.path);
        
        if let Some(_handle) = self.handle.take() {
            // Handle cleanup is automatic via Drop
            if self.delete_on_close {
                if let Err(e) = tokio::fs::remove_file(&self.path).await {
                    warn!("Failed to delete file on close: {}", e);
                }
            }
        }
        
        self.position = 0;
        self.size = 0;
        self.opened_at = None;
        self.line_buffer.clear();
        
        info!("File closed: {:?}", self.path);
    }
    
    /// Read data from file (matches C++ signature)
    pub async fn read(&mut self, buffer: &mut [u8]) -> Result<i32> {
        match &mut self.handle {
            Some(FileHandle::Standard(file)) => {
                match file.read(buffer).await {
                    Ok(bytes_read) => {
                        self.position += bytes_read as u64;
                        Ok(bytes_read as i32)
                    }
                    Err(e) => Err(Win32FileError::MemoryMappingFailed { source: e }),
                }
            }
            Some(FileHandle::MemoryMapped { mmap, position, .. }) => {
                let bytes_to_read = std::cmp::min(buffer.len(), mmap.len() - *position);
                if bytes_to_read > 0 {
                    buffer[..bytes_to_read].copy_from_slice(&mmap[*position..*position + bytes_to_read]);
                    *position += bytes_to_read;
                    self.position += bytes_to_read as u64;
                }
                Ok(bytes_to_read as i32)
            }
            Some(FileHandle::MemoryMappedMut { mmap, position, .. }) => {
                let bytes_to_read = std::cmp::min(buffer.len(), mmap.len() - *position);
                if bytes_to_read > 0 {
                    buffer[..bytes_to_read].copy_from_slice(&mmap[*position..*position + bytes_to_read]);
                    *position += bytes_to_read;
                    self.position += bytes_to_read as u64;
                }
                Ok(bytes_to_read as i32)
            }
            None => Err(Win32FileError::FileNotOpen),
        }
    }
    
    /// Write data to file (matches C++ signature)
    pub async fn write(&mut self, buffer: &[u8]) -> Result<i32> {
        if !self.access_mode.has_write() {
            return Err(Win32FileError::InvalidMode);
        }
        
        match &mut self.handle {
            Some(FileHandle::Standard(file)) => {
                match file.write(buffer).await {
                    Ok(bytes_written) => {
                        self.position += bytes_written as u64;
                        self.size = std::cmp::max(self.size, self.position);
                        Ok(bytes_written as i32)
                    }
                    Err(e) => Err(Win32FileError::MemoryMappingFailed { source: e }),
                }
            }
            Some(FileHandle::MemoryMappedMut { mmap, position, .. }) => {
                let bytes_to_write = std::cmp::min(buffer.len(), mmap.len() - *position);
                if bytes_to_write > 0 {
                    mmap[*position..*position + bytes_to_write].copy_from_slice(&buffer[..bytes_to_write]);
                    *position += bytes_to_write;
                    self.position += bytes_to_write as u64;
                    self.size = std::cmp::max(self.size, self.position);
                }
                Ok(bytes_to_write as i32)
            }
            Some(FileHandle::MemoryMapped { .. }) => {
                Err(Win32FileError::InvalidMode)
            }
            None => Err(Win32FileError::FileNotOpen),
        }
    }
    
    /// Seek to position in file (matches C++ signature)
    pub async fn seek(&mut self, new_pos: i32, mode: FileSeekMode) -> Result<i32> {
        match &mut self.handle {
            Some(FileHandle::Standard(file)) => {
                let seek_from = match mode {
                    FileSeekMode::Current => SeekFrom::Current(new_pos as i64),
                    FileSeekMode::Start => SeekFrom::Start(new_pos as u64),
                    FileSeekMode::End => SeekFrom::End(new_pos as i64),
                };
                
                match file.seek(seek_from).await {
                    Ok(pos) => {
                        self.position = pos;
                        Ok(pos as i32)
                    }
                    Err(e) => Err(Win32FileError::MemoryMappingFailed { source: e }),
                }
            }
            Some(FileHandle::MemoryMapped { mmap, position, .. }) | 
            Some(FileHandle::MemoryMappedMut { mmap, position, .. }) => {
                let new_position = match mode {
                    FileSeekMode::Current => (*position as i64 + new_pos as i64) as usize,
                    FileSeekMode::Start => new_pos as usize,
                    FileSeekMode::End => (mmap.len() as i64 + new_pos as i64) as usize,
                };
                
                if new_position <= mmap.len() {
                    *position = new_position;
                    self.position = new_position as u64;
                    Ok(new_position as i32)
                } else {
                    Err(Win32FileError::InvalidSeekPosition { position: new_position as i64 })
                }
            }
            None => Err(Win32FileError::FileNotOpen),
        }
    }

    
    /// Read next line from file (matches C++ signature)
    pub async fn next_line(&mut self, max_length: Option<usize>) -> Result<Option<String>> {
        match &mut self.handle {
            Some(FileHandle::Standard(file)) => {
                let mut reader = BufReader::new(file);
                let mut line = String::new();
                
                match reader.read_line(&mut line).await {
                    Ok(0) => Ok(None), // EOF
                    Ok(_) => {
                        // Remove trailing newline
                        if line.ends_with('\n') {
                            line.pop();
                            if line.ends_with('\r') {
                                line.pop();
                            }
                        }
                        
                        // Truncate if max_length specified
                        if let Some(max_len) = max_length {
                            line.truncate(max_len);
                        }
                        
                        Ok(Some(line))
                    }
                    Err(e) => Err(Win32FileError::MemoryMappingFailed { source: e }),
                }
            }
            Some(FileHandle::MemoryMapped { mmap, position, .. }) | 
            Some(FileHandle::MemoryMappedMut { mmap, position, .. }) => {
                if *position >= mmap.len() {
                    return Ok(None);
                }
                
                // Find next newline
                let start = *position;
                let mut end = start;
                
                while end < mmap.len() && mmap[end] != b'\n' {
                    end += 1;
                }
                
                if end < mmap.len() {
                    // Found newline
                    let mut line_bytes = &mmap[start..end];
                    
                    // Remove trailing \r if present
                    if line_bytes.ends_with(&[b'\r']) {
                        line_bytes = &line_bytes[..line_bytes.len() - 1];
                    }
                    
                    *position = end + 1; // Move past the newline
                    self.position = *position as u64;
                    
                    let line = std::str::from_utf8(line_bytes)?.to_string();
                    
                    // Truncate if max_length specified
                    if let Some(max_len) = max_length {
                        if line.len() > max_len {
                            return Ok(Some(line[..max_len].to_string()));
                        }
                    }
                    
                    Ok(Some(line))
                } else if start < mmap.len() {
                    // Last line without newline
                    let line_bytes = &mmap[start..end];
                    *position = mmap.len();
                    self.position = *position as u64;
                    
                    let line = std::str::from_utf8(line_bytes)?.to_string();
                    Ok(Some(line))
                } else {
                    Ok(None)
                }
            }
            None => Err(Win32FileError::FileNotOpen),
        }
    }
    
    /// Scan for next integer in file
    pub async fn scan_int(&mut self) -> Result<Option<i32>> {
        // Read characters until we find a number
        let mut number_str = String::new();
        let mut found_digit = false;
        let mut buffer = [0u8; 1];
        
        loop {
            match self.read(&mut buffer).await? {
                0 => break, // EOF
                _ => {
                    let ch = buffer[0] as char;
                    
                    if ch.is_ascii_digit() || (ch == '-' && !found_digit) {
                        number_str.push(ch);
                        found_digit = true;
                    } else if found_digit {
                        // End of number, seek back one character
                        let _ = self.seek(-1, FileSeekMode::Current).await;
                        break;
                    }
                    // Skip whitespace and other characters if no number started
                }
            }
        }
        
        if number_str.is_empty() {
            Ok(None)
        } else {
            number_str.parse::<i32>()
                .map(Some)
                .map_err(|e| Win32FileError::ParseError { message: format!("Invalid integer: {}", e) })
        }
    }
    
    /// Scan for next float in file
    pub async fn scan_real(&mut self) -> Result<Option<f32>> {
        let mut number_str = String::new();
        let mut found_digit = false;
        let mut found_dot = false;
        let mut buffer = [0u8; 1];
        
        loop {
            match self.read(&mut buffer).await? {
                0 => break, // EOF
                _ => {
                    let ch = buffer[0] as char;
                    
                    if ch.is_ascii_digit() || 
                       (ch == '-' && !found_digit) || 
                       (ch == '.' && !found_dot) {
                        number_str.push(ch);
                        if ch.is_ascii_digit() {
                            found_digit = true;
                        }
                        if ch == '.' {
                            found_dot = true;
                        }
                    } else if found_digit {
                        // End of number, seek back one character
                        let _ = self.seek(-1, FileSeekMode::Current).await;
                        break;
                    }
                }
            }
        }
        
        if number_str.is_empty() {
            Ok(None)
        } else {
            number_str.parse::<f32>()
                .map(Some)
                .map_err(|e| Win32FileError::ParseError { message: format!("Invalid float: {}", e) })
        }
    }
    
    /// Scan for next string (word) in file
    pub async fn scan_string(&mut self) -> Result<Option<String>> {
        let mut word = String::new();
        let mut buffer = [0u8; 1];
        let mut in_word = false;
        
        loop {
            match self.read(&mut buffer).await? {
                0 => break, // EOF
                _ => {
                    let ch = buffer[0] as char;
                    
                    if ch.is_whitespace() {
                        if in_word {
                            break; // End of word
                        }
                        // Skip leading whitespace
                    } else {
                        word.push(ch);
                        in_word = true;
                    }
                }
            }
        }
        
        if word.is_empty() {
            Ok(None)
        } else {
            Ok(Some(word))
        }
    }
    
    /// Read entire file and close (matches C++ signature)
    pub async fn read_entire_and_close(&mut self) -> Result<Option<Vec<u8>>> {
        match &mut self.handle {
            Some(FileHandle::Standard(file)) => {
                let mut buffer = Vec::new();
                match file.read_to_end(&mut buffer).await {
                    Ok(_) => {
                        self.close().await;
                        Ok(Some(buffer))
                    }
                    Err(e) => Err(Win32FileError::MemoryMappingFailed { source: e }),
                }
            }
            Some(FileHandle::MemoryMapped { mmap, .. }) => {
                let buffer = mmap.to_vec();
                self.close().await;
                Ok(Some(buffer))
            }
            Some(FileHandle::MemoryMappedMut { mmap, .. }) => {
                let buffer = mmap.to_vec();
                self.close().await;
                Ok(Some(buffer))
            }
            None => Err(Win32FileError::FileNotOpen),
        }
    }
    
    /// Get memory-mapped view of file (if using memory mapping)
    pub fn get_memory_map(&self) -> Option<&[u8]> {
        match &self.handle {
            Some(FileHandle::MemoryMapped { mmap, .. }) => Some(mmap),
            Some(FileHandle::MemoryMappedMut { mmap, .. }) => Some(mmap),
            _ => None,
        }
    }
    
    /// Set delete on close flag
    pub fn set_delete_on_close(&mut self, delete: bool) {
        self.delete_on_close = delete;
    }
    
    /// Get file size
    pub fn get_size(&self) -> u64 {
        self.size
    }
    
    /// Get current position
    pub fn get_position(&self) -> u64 {
        self.position
    }
    
    /// Get file path
    pub fn get_path(&self) -> &Path {
        &self.path
    }
    
    /// Check if file is open
    pub fn is_open(&self) -> bool {
        self.handle.is_some()
    }
    
    /// Get access mode
    pub fn get_access_mode(&self) -> FileAccess {
        self.access_mode
    }
    
    /// Get operation mode
    pub fn get_operation_mode(&self) -> FileOperationMode {
        self.operation_mode
    }
    
    /// Private helper methods
    
    /// Determine the best operation mode based on file characteristics
    async fn determine_operation_mode(&self) -> Result<FileOperationMode> {
        // Check file size to determine if memory mapping would be beneficial
        if let Ok(metadata) = tokio::fs::metadata(&self.path).await {
            let file_size = metadata.len();
            
            // Use memory mapping for large files (>1MB) in read mode
            if file_size > 1024 * 1024 && self.access_mode.has_read() && !self.access_mode.has_write() {
                return Ok(FileOperationMode::MemoryMapped);
            }
            
            // Use streaming for sequential access patterns
            if self.access_mode.has_streaming() {
                return Ok(FileOperationMode::Streaming);
            }
        }
        
        Ok(FileOperationMode::Standard)
    }
    
    /// Create appropriate file handle based on operation mode
    async fn create_file_handle(&self) -> Result<FileHandle> {
        let mut options = OpenOptions::new();
        
        // Configure options based on access mode
        if self.access_mode.has_read() {
            options.read(true);
        }
        if self.access_mode.has_write() {
            options.write(true);
        }
        if self.access_mode.has_create() {
            options.create(true);
        }
        if self.access_mode.has_append() {
            options.append(true);
        }
        
        let file = options.open(&self.path).await?;
        
        match self.operation_mode {
            FileOperationMode::Standard | FileOperationMode::Streaming => {
                Ok(FileHandle::Standard(file))
            }
            FileOperationMode::MemoryMapped => {
                if self.access_mode.has_write() {
                    // Create mutable memory map
                    let mmap = unsafe {
                        MmapOptions::new()
                            .map_mut(&file)?
                    };
                    Ok(FileHandle::MemoryMappedMut {
                        _file: file,
                        mmap,
                        position: 0,
                    })
                } else {
                    // Create read-only memory map
                    let mmap = unsafe {
                        MmapOptions::new()
                            .map(&file)?
                    };
                    Ok(FileHandle::MemoryMapped {
                        _file: file,
                        mmap,
                        position: 0,
                    })
                }
            }
        }
    }
    
    /// Get file size from handle
    async fn get_file_size(&self) -> Result<u64> {
        match &self.handle {
            Some(FileHandle::Standard(file)) => {
                Ok(file.metadata().await?.len())
            }
            Some(FileHandle::MemoryMapped { mmap, .. }) => {
                Ok(mmap.len() as u64)
            }
            Some(FileHandle::MemoryMappedMut { mmap, .. }) => {
                Ok(mmap.len() as u64)
            }
            None => Err(Win32FileError::FileNotOpen),
        }
    }
}

impl Default for Win32LocalFile {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Win32LocalFile {
    fn drop(&mut self) {
        // Ensure cleanup happens even if close() wasn't called
        if self.handle.is_some() {
            // Note: Can't call async close() in Drop, but cleanup is mostly automatic
            if self.delete_on_close {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

impl fmt::Display for Win32LocalFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Win32LocalFile {{ path: {:?}, size: {}, position: {}, mode: {:?} }}",
            self.path, self.size, self.position, self.operation_mode
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;
    
    #[tokio::test]
    async fn test_file_creation() {
        let mut file = Win32LocalFile::new();
        assert!(!file.is_open());
        assert_eq!(file.get_size(), 0);
        assert_eq!(file.get_position(), 0);
    }
    
    #[tokio::test]
    async fn test_file_open_read() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_read.txt");
        
        // Create test file
        fs::write(&test_file, "Hello, World!").await.unwrap();
        
        let mut file = Win32LocalFile::new();
        let result = file.open(&test_file.to_string_lossy(), FileAccess::READ).await.unwrap();
        
        assert!(result);
        assert!(file.is_open());
        assert_eq!(file.get_size(), 13);
        
        file.close().await;
        assert!(!file.is_open());
    }
    
    #[tokio::test]
    async fn test_file_read_write() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_write.txt");
        
        let mut file = Win32LocalFile::new();
        let result = file.open(
            &test_file.to_string_lossy(), 
            FileAccess::READ | FileAccess::WRITE | FileAccess::CREATE
        ).await.unwrap();
        
        assert!(result);
        
        // Write data
        let write_data = b"Test data for writing";
        let bytes_written = file.write(write_data).await.unwrap();
        assert_eq!(bytes_written, write_data.len() as i32);
        
        // Seek to beginning
        let pos = file.seek(0, FileSeekMode::Start).await.unwrap();
        assert_eq!(pos, 0);
        
        // Read data back
        let mut read_buffer = vec![0u8; write_data.len()];
        let bytes_read = file.read(&mut read_buffer).await.unwrap();
        assert_eq!(bytes_read, write_data.len() as i32);
        assert_eq!(read_buffer, write_data);
        
        file.close().await;
    }
    
    #[tokio::test]
    async fn test_line_reading() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_lines.txt");
        
        // Create test file with multiple lines
        let test_content = "Line 1\nLine 2\nLine 3";
        fs::write(&test_file, test_content).await.unwrap();
        
        let mut file = Win32LocalFile::new();
        file.open(&test_file.to_string_lossy(), FileAccess::READ).await.unwrap();
        
        // Read lines
        let line1 = file.next_line(None).await.unwrap();
        assert_eq!(line1, Some("Line 1".to_string()));
        
        let line2 = file.next_line(None).await.unwrap();
        assert_eq!(line2, Some("Line 2".to_string()));
        
        let line3 = file.next_line(None).await.unwrap();
        assert_eq!(line3, Some("Line 3".to_string()));
        
        let eof = file.next_line(None).await.unwrap();
        assert_eq!(eof, None);
        
        file.close().await;
    }
    
    #[tokio::test]
    async fn test_memory_mapped_file() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_mmap.txt");
        
        // Create a large test file to trigger memory mapping
        let large_data = vec![b'A'; 2 * 1024 * 1024]; // 2MB
        fs::write(&test_file, &large_data).await.unwrap();
        
        let mut file = Win32LocalFile::new();
        file.open(&test_file.to_string_lossy(), FileAccess::READ).await.unwrap();
        
        // Should use memory mapping for large read-only files
        assert_eq!(file.get_operation_mode(), FileOperationMode::MemoryMapped);
        
        // Test memory map access
        let mmap = file.get_memory_map();
        assert!(mmap.is_some());
        assert_eq!(mmap.unwrap().len(), large_data.len());
        
        file.close().await;
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
    async fn test_scan_operations() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_scan.txt");
        
        // Create test file with numbers and text
        let test_content = "42 3.14 hello 123 world 2.71";
        fs::write(&test_file, test_content).await.unwrap();
        
        let mut file = Win32LocalFile::new();
        file.open(&test_file.to_string_lossy(), FileAccess::READ).await.unwrap();
        
        // Test integer scanning
        let int1 = file.scan_int().await.unwrap();
        assert_eq!(int1, Some(42));
        
        // Test float scanning
        let float1 = file.scan_real().await.unwrap();
        assert_eq!(float1, Some(3.14));
        
        // Test string scanning
        let string1 = file.scan_string().await.unwrap();
        assert_eq!(string1, Some("hello".to_string()));
        
        file.close().await;
    }
    
    #[tokio::test]
    async fn test_read_entire_and_close() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_entire.txt");
        
        let test_data = b"Complete file content to read at once";
        fs::write(&test_file, test_data).await.unwrap();
        
        let mut file = Win32LocalFile::new();
        file.open(&test_file.to_string_lossy(), FileAccess::READ).await.unwrap();
        
        let result = file.read_entire_and_close().await.unwrap();
        assert_eq!(result, Some(test_data.to_vec()));
        assert!(!file.is_open());
    }
}
