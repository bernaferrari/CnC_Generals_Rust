//! WWFILE - File I/O utilities for WWLib
//!
//! This module provides Rust implementations of the WWFILE utilities from the
//! Command & Conquer Generals WWLib library. It includes file system operations,
//! date/time handling, and formatted writing capabilities.

use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default buffer size for printf operations
const PRINTF_BUFFER_SIZE: usize = 1024;

/// File access rights
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRights {
    Read = 1,
    Write = 2,
    ReadWrite = 3,
}

/// Seek direction enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekDirection {
    Start = 0,
    Current = 1,
    End = 2,
}

impl From<SeekDirection> for SeekFrom {
    fn from(dir: SeekDirection) -> Self {
        match dir {
            SeekDirection::Start => SeekFrom::Start(0),
            SeekDirection::Current => SeekFrom::Current(0),
            SeekDirection::End => SeekFrom::End(0),
        }
    }
}

/// Date/time extraction utilities for DOS-style packed date/time format
pub mod datetime {
    /// Extract year from DOS datetime (1980-based)
    pub fn year(dt: u32) -> u16 {
        (((dt & 0xFE00_0000) >> (9 + 16)) + 1980) as u16
    }

    /// Extract month from DOS datetime (1-12)
    pub fn month(dt: u32) -> u8 {
        ((dt & 0x01E0_0000) >> (5 + 16)) as u8
    }

    /// Extract day from DOS datetime (1-31)
    pub fn day(dt: u32) -> u8 {
        ((dt & 0x001F_0000) >> (0 + 16)) as u8
    }

    /// Extract hour from DOS datetime (0-23)
    pub fn hour(dt: u32) -> u8 {
        ((dt & 0x0000_F800) >> 11) as u8
    }

    /// Extract minute from DOS datetime (0-59)
    pub fn minute(dt: u32) -> u8 {
        ((dt & 0x0000_07E0) >> 5) as u8
    }

    /// Extract second from DOS datetime (0-59, 2-second precision)
    pub fn second(dt: u32) -> u8 {
        ((dt & 0x0000_001F) << 1) as u8
    }
}

/// Error types for file operations
#[derive(Debug)]
pub enum FileError {
    Io(io::Error),
    NotFound { path: String },
    AccessDenied { path: String },
    InvalidHandle,
    Format(std::fmt::Error),
    Utf8(std::str::Utf8Error),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::Io(err) => write!(f, "IO error: {}", err),
            FileError::NotFound { path } => write!(f, "File not found: {}", path),
            FileError::AccessDenied { path } => write!(f, "Access denied: {}", path),
            FileError::InvalidHandle => write!(f, "Invalid file handle"),
            FileError::Format(err) => write!(f, "Format error: {}", err),
            FileError::Utf8(err) => write!(f, "UTF-8 error: {}", err),
        }
    }
}

impl std::error::Error for FileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FileError::Io(err) => Some(err),
            FileError::Format(err) => Some(err),
            FileError::Utf8(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for FileError {
    fn from(err: io::Error) -> Self {
        FileError::Io(err)
    }
}

impl From<std::fmt::Error> for FileError {
    fn from(err: std::fmt::Error) -> Self {
        FileError::Format(err)
    }
}

impl From<std::str::Utf8Error> for FileError {
    fn from(err: std::str::Utf8Error) -> Self {
        FileError::Utf8(err)
    }
}

/// Result type for file operations
pub type FileResult<T> = Result<T, FileError>;

/// Trait defining the core file operations interface
///
/// This trait mirrors the original C++ FileClass interface, adapted for Rust
/// with proper error handling and memory safety.
pub trait FileInterface {
    /// Get the file name
    fn file_name(&self) -> Option<&str>;

    /// Set the file name
    fn set_name(&mut self, filename: &str) -> FileResult<String>;

    /// Create the file
    fn create(&mut self) -> FileResult<()>;

    /// Delete the file
    fn delete(&mut self) -> FileResult<()>;

    /// Check if file is available for access
    fn is_available(&self, forced: bool) -> bool;

    /// Check if file is currently open
    fn is_open(&self) -> bool;

    /// Open file with specified name and rights
    fn open_with_name(&mut self, filename: &str, rights: FileRights) -> FileResult<()>;

    /// Open file with current name and specified rights
    fn open(&mut self, rights: FileRights) -> FileResult<()>;

    /// Read data from file
    fn read(&mut self, buffer: &mut [u8]) -> FileResult<usize>;

    /// Seek to position in file
    fn seek(&mut self, pos: i64, dir: SeekDirection) -> FileResult<u64>;

    /// Get current position in file
    fn tell(&mut self) -> FileResult<u64> {
        self.seek(0, SeekDirection::Current)
    }

    /// Get file size
    fn size(&self) -> FileResult<u64>;

    /// Write data to file
    fn write(&mut self, buffer: &[u8]) -> FileResult<usize>;

    /// Close the file
    fn close(&mut self) -> FileResult<()>;

    /// Get file date/time
    fn get_date_time(&self) -> FileResult<u32>;

    /// Set file date/time
    fn set_date_time(&mut self, datetime: u32) -> FileResult<()>;

    /// Write formatted string to file
    fn write_fmt(&mut self, args: std::fmt::Arguments) -> FileResult<usize>;

    /// Write formatted string with indentation
    fn write_fmt_indented(&mut self, depth: usize, args: std::fmt::Arguments) -> FileResult<usize>;
}

/// Standard file implementation
///
/// This struct provides a concrete implementation of the FileInterface trait
/// using standard library file operations.
#[derive(Debug)]
pub struct WWFile {
    path: Option<PathBuf>,
    file: Option<File>,
    rights: Option<FileRights>,
}

impl Default for WWFile {
    fn default() -> Self {
        Self::new()
    }
}

impl WWFile {
    /// Create a new WWFile instance
    pub fn new() -> Self {
        Self {
            path: None,
            file: None,
            rights: None,
        }
    }

    /// Create a new WWFile instance with a path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            file: None,
            rights: None,
        }
    }

    /// Get file metadata if file exists
    pub fn metadata(&self) -> FileResult<Option<Metadata>> {
        if let Some(path) = &self.path {
            match std::fs::metadata(path) {
                Ok(metadata) => Ok(Some(metadata)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(FileError::Io(e)),
            }
        } else {
            Ok(None)
        }
    }

    /// Convert system time to DOS datetime format
    fn system_time_to_dos_datetime(time: SystemTime) -> u32 {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = duration.as_secs();

        // Convert Unix timestamp to DOS format
        // This is a simplified conversion - in practice, you'd want proper date handling
        let years_since_1980 = ((secs / (365 * 24 * 3600)) as u32).min(127);
        let remaining_secs = secs % (365 * 24 * 3600);
        let days = (remaining_secs / (24 * 3600)) as u32;
        let month = (days / 30).min(12).max(1); // Simplified month calculation
        let day = (days % 30).min(31).max(1);
        let hour = ((remaining_secs % (24 * 3600)) / 3600) as u32;
        let minute = ((remaining_secs % 3600) / 60) as u32;
        let second = ((remaining_secs % 60) / 2) as u32; // DOS format uses 2-second precision

        (years_since_1980 << (9 + 16))
            | (month << (5 + 16))
            | (day << 16)
            | (hour << 11)
            | (minute << 5)
            | second
    }
}

impl FileInterface for WWFile {
    fn file_name(&self) -> Option<&str> {
        self.path.as_ref()?.file_name()?.to_str()
    }

    fn set_name(&mut self, filename: &str) -> FileResult<String> {
        self.path = Some(PathBuf::from(filename));
        Ok(self.file_name().unwrap_or(filename).to_string())
    }

    fn create(&mut self) -> FileResult<()> {
        let path = self.path.as_ref().ok_or(FileError::InvalidHandle)?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        self.file = Some(file);
        self.rights = Some(FileRights::Write);
        Ok(())
    }

    fn delete(&mut self) -> FileResult<()> {
        let path = self.path.as_ref().ok_or(FileError::InvalidHandle)?.clone();
        self.close()?;
        std::fs::remove_file(&path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => FileError::NotFound {
                path: path.display().to_string(),
            },
            io::ErrorKind::PermissionDenied => FileError::AccessDenied {
                path: path.display().to_string(),
            },
            _ => FileError::Io(e),
        })
    }

    fn is_available(&self, _forced: bool) -> bool {
        if let Some(path) = &self.path {
            path.exists()
        } else {
            false
        }
    }

    fn is_open(&self) -> bool {
        self.file.is_some()
    }

    fn open_with_name(&mut self, filename: &str, rights: FileRights) -> FileResult<()> {
        self.set_name(filename)?;
        self.open(rights)
    }

    fn open(&mut self, rights: FileRights) -> FileResult<()> {
        let path = self.path.as_ref().ok_or(FileError::InvalidHandle)?;

        let file = match rights {
            FileRights::Read => File::open(path)?,
            FileRights::Write => OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(path)?,
            FileRights::ReadWrite => OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path)?,
        };

        self.file = Some(file);
        self.rights = Some(rights);
        Ok(())
    }

    fn read(&mut self, buffer: &mut [u8]) -> FileResult<usize> {
        let file = self.file.as_mut().ok_or(FileError::InvalidHandle)?;
        Ok(file.read(buffer)?)
    }

    fn seek(&mut self, pos: i64, dir: SeekDirection) -> FileResult<u64> {
        let file = self.file.as_mut().ok_or(FileError::InvalidHandle)?;
        let seek_pos = match dir {
            SeekDirection::Start => SeekFrom::Start(pos as u64),
            SeekDirection::Current => SeekFrom::Current(pos),
            SeekDirection::End => SeekFrom::End(pos),
        };
        Ok(file.seek(seek_pos)?)
    }

    fn size(&self) -> FileResult<u64> {
        if let Some(file) = &self.file {
            Ok(file.metadata()?.len())
        } else if let Some(path) = &self.path {
            Ok(std::fs::metadata(path)?.len())
        } else {
            Err(FileError::InvalidHandle)
        }
    }

    fn write(&mut self, buffer: &[u8]) -> FileResult<usize> {
        let file = self.file.as_mut().ok_or(FileError::InvalidHandle)?;
        Ok(file.write(buffer)?)
    }

    fn close(&mut self) -> FileResult<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        self.rights = None;
        Ok(())
    }

    fn get_date_time(&self) -> FileResult<u32> {
        if let Some(metadata) = self.metadata()? {
            let modified = metadata.modified()?;
            Ok(Self::system_time_to_dos_datetime(modified))
        } else {
            Err(FileError::NotFound {
                path: self
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            })
        }
    }

    fn set_date_time(&mut self, _datetime: u32) -> FileResult<()> {
        // Note: Setting file times requires platform-specific code
        // This is a simplified implementation that would need to be extended
        // for full compatibility with the original API
        Err(FileError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "Setting file date/time not implemented",
        )))
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments) -> FileResult<usize> {
        let formatted = format!("{}", args);
        self.write(formatted.as_bytes())
    }

    fn write_fmt_indented(&mut self, depth: usize, args: std::fmt::Arguments) -> FileResult<usize> {
        let limited_depth = depth.min(PRINTF_BUFFER_SIZE);
        let tabs = "\t".repeat(limited_depth);
        let formatted = format!("{}{}", tabs, args);
        self.write(formatted.as_bytes())
    }
}

/// Convenience macro for writing formatted strings to files
///
/// Usage: `file_printf!(file, "Hello {}", name)`
#[macro_export]
macro_rules! file_printf {
    ($file:expr, $($arg:tt)*) => {
        $file.write_fmt(format_args!($($arg)*))
    };
}

/// Convenience macro for writing indented formatted strings to files
///
/// Usage: `file_printf_indented!(file, 2, "Indented: {}", value)`
#[macro_export]
macro_rules! file_printf_indented {
    ($file:expr, $depth:expr, $($arg:tt)*) => {
        $file.write_fmt_indented($depth, format_args!($($arg)*))
    };
}

/// File utility functions
pub mod utils {
    use super::*;

    /// Check if a path exists
    pub fn exists<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().exists()
    }

    /// Get file size without opening
    pub fn file_size<P: AsRef<Path>>(path: P) -> FileResult<u64> {
        Ok(std::fs::metadata(path)?.len())
    }

    /// Copy file from source to destination
    pub fn copy_file<P: AsRef<Path>>(from: P, to: P) -> FileResult<u64> {
        Ok(std::fs::copy(from, to)?)
    }

    /// Move/rename file
    pub fn move_file<P: AsRef<Path>>(from: P, to: P) -> FileResult<()> {
        std::fs::rename(from, to)?;
        Ok(())
    }

    /// Create directory and all parent directories
    pub fn create_dir_all<P: AsRef<Path>>(path: P) -> FileResult<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }

    /// Remove directory and all its contents
    pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> FileResult<()> {
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    /// List directory contents
    pub fn list_dir<P: AsRef<Path>>(path: P) -> FileResult<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            entries.push(entry?.path());
        }
        entries.sort();
        Ok(entries)
    }

    /// Find files matching a pattern in a directory
    pub fn find_files<P: AsRef<Path>>(
        dir: P,
        pattern: &str,
        recursive: bool,
    ) -> FileResult<Vec<PathBuf>> {
        let mut results = Vec::new();
        find_files_recursive(dir.as_ref(), pattern, recursive, &mut results)?;
        Ok(results)
    }

    fn find_files_recursive(
        dir: &Path,
        pattern: &str,
        recursive: bool,
        results: &mut Vec<PathBuf>,
    ) -> FileResult<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if matches_pattern(filename, pattern) {
                        results.push(path);
                    }
                }
            } else if recursive && path.is_dir() {
                find_files_recursive(&path, pattern, recursive, results)?;
            }
        }
        Ok(())
    }

    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        // Simple wildcard matching - could be enhanced with regex or glob patterns
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            // Very basic wildcard support
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return filename.starts_with(prefix) && filename.ends_with(suffix);
            }
        }

        filename == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_datetime_extraction() {
        let test_dt = 0x2A7F_1234; // Test date/time value

        assert!(datetime::year(test_dt) >= 1980);
        assert!(datetime::month(test_dt) >= 1 && datetime::month(test_dt) <= 12);
        assert!(datetime::day(test_dt) >= 1 && datetime::day(test_dt) <= 31);
        assert!(datetime::hour(test_dt) <= 23);
        assert!(datetime::minute(test_dt) <= 59);
        assert!(datetime::second(test_dt) <= 59);
    }

    #[test]
    fn test_file_creation_and_deletion() -> FileResult<()> {
        let temp_dir = env::temp_dir();
        let file_path = temp_dir.join("wwfile_test_file.txt");

        let mut file = WWFile::with_path(&file_path);

        // File should not exist initially
        assert!(!file.is_available(false));
        assert!(!file.is_open());

        // Create file
        file.create()?;
        assert!(file.is_open());
        assert!(file.is_available(false));

        // Write some data
        let test_data = b"Hello, World!";
        let written = file.write(test_data)?;
        assert_eq!(written, test_data.len());

        // Close and reopen for reading
        file.close()?;
        assert!(!file.is_open());

        file.open(FileRights::Read)?;
        let mut buffer = vec![0u8; test_data.len()];
        let read = file.read(&mut buffer)?;
        assert_eq!(read, test_data.len());
        assert_eq!(&buffer, test_data);

        file.close()?;

        // Delete file
        file.delete()?;
        assert!(!file.is_available(false));

        Ok(())
    }

    #[test]
    fn test_file_seeking() -> FileResult<()> {
        let temp_dir = env::temp_dir();
        let file_path = temp_dir.join("wwfile_seek_test.txt");

        let mut file = WWFile::with_path(&file_path);
        file.create()?;

        // Write test data
        let test_data = b"0123456789";
        file.write(test_data)?;

        // Test seeking
        let pos = file.seek(5, SeekDirection::Start)?;
        assert_eq!(pos, 5);

        let pos = file.seek(2, SeekDirection::Current)?;
        assert_eq!(pos, 7);

        let pos = file.seek(-2, SeekDirection::End)?;
        assert_eq!(pos, 8);

        file.close()?;
        file.delete()?;

        Ok(())
    }

    #[test]
    fn test_formatted_writing() -> FileResult<()> {
        let temp_dir = env::temp_dir();
        let file_path = temp_dir.join("wwfile_format_test.txt");

        let mut file = WWFile::with_path(&file_path);
        file.create()?;

        // Test formatted writing
        file_printf!(file, "Number: {}\n", 42)?;
        file_printf_indented!(file, 2, "Indented: {}\n", "test")?;

        file.close()?;

        // Verify contents
        let contents = std::fs::read_to_string(&file_path)?;
        assert!(contents.contains("Number: 42"));
        assert!(contents.contains("\t\tIndented: test"));

        std::fs::remove_file(file_path)?;

        Ok(())
    }

    #[test]
    fn test_file_utilities() -> FileResult<()> {
        let temp_dir = env::temp_dir();
        let test_file = temp_dir.join("wwfile_utility_test.txt");

        // Create test file
        std::fs::write(&test_file, "test content")?;

        // Test utility functions
        assert!(utils::exists(&test_file));
        assert_eq!(utils::file_size(&test_file)?, 12);

        // Test directory listing
        let entries = utils::list_dir(&temp_dir)?;
        // Note: temp directory may contain other files, so we just check our file exists
        assert!(entries.contains(&test_file));

        // Test file finding
        let found = utils::find_files(&temp_dir, "*utility_test.txt", false)?;
        assert!(found.len() >= 1);
        assert!(found.contains(&test_file));

        Ok(())
    }
}
