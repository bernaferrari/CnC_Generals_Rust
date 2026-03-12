//! File I/O abstraction module
//!
//! This module provides a cross-platform file I/O abstraction layer based on the
//! C&C Generals game engine's File system. It defines traits and basic functionality
//! for file operations that can be implemented by different file system backends.

use std::fmt;
use std::io;

use crate::common::ascii_string::AsciiString;

/// File access flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileAccess(u32);

impl FileAccess {
    pub const NONE: FileAccess = FileAccess(0x00000000);
    pub const READ: FileAccess = FileAccess(0x00000001);
    pub const WRITE: FileAccess = FileAccess(0x00000002);
    pub const APPEND: FileAccess = FileAccess(0x00000004);
    pub const CREATE: FileAccess = FileAccess(0x00000008);
    pub const TRUNCATE: FileAccess = FileAccess(0x00000010);
    pub const TEXT: FileAccess = FileAccess(0x00000020);
    pub const BINARY: FileAccess = FileAccess(0x00000040);
    pub const READWRITE: FileAccess = FileAccess(0x00000003); // READ | WRITE
    pub const ONLY_NEW: FileAccess = FileAccess(0x00000080);
    pub const STREAMING: FileAccess = FileAccess(0x00000100);

    /// Check if access mode contains specific flag
    pub fn contains(&self, other: FileAccess) -> bool {
        (self.0 & other.0) != 0
    }

    /// Combine access flags
    pub fn combine(&self, other: FileAccess) -> FileAccess {
        FileAccess(self.0 | other.0)
    }
}

/// File seek modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekMode {
    /// Seek from start of file
    Start,
    /// Seek from current position
    Current,
    /// Seek from end of file
    End,
}

/// Abstract file interface
///
/// This trait provides the core file operations that all file implementations must support.
/// It mirrors the C++ File class interface while being idiomatic Rust.
pub trait File {
    /// Open a file with the specified access mode
    fn open(&mut self, filename: &str, access: FileAccess) -> Result<(), io::Error>;

    /// Close the file
    fn close(&mut self);

    /// Read data from file into buffer
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, io::Error>;

    /// Write data from buffer to file
    fn write(&mut self, buffer: &[u8]) -> Result<usize, io::Error>;

    /// Seek to a position in the file
    fn seek(&mut self, pos: i32, mode: SeekMode) -> Result<i32, io::Error>;

    /// Read the next line from file
    fn next_line(&mut self, buf: Option<&mut Vec<u8>>, buf_size: Option<usize>);

    /// Scan an integer from the file
    fn scan_int(&mut self) -> Result<i32, io::Error>;

    /// Scan a real number from the file
    fn scan_real(&mut self) -> Result<f32, io::Error>;

    /// Scan a string from the file
    fn scan_string(&mut self) -> Result<AsciiString, io::Error>;

    /// Print formatted text to file
    fn print(&mut self, text: &str) -> Result<bool, io::Error>;

    /// Get the size of the file
    fn size(&self) -> i32;

    /// Get current position in file
    fn position(&self) -> i32;

    /// Check if at end of file
    fn eof(&self) -> bool;

    /// Get the filename
    fn get_name(&self) -> &str;

    /// Set the filename
    fn set_name(&mut self, name: &str);

    /// Get access flags
    fn get_access(&self) -> FileAccess;

    /// Read entire file contents and close
    fn read_entire_and_close(&mut self) -> Result<Vec<u8>, io::Error>;
}

/// Basic file implementation
///
/// This struct provides the common functionality for file implementations,
/// corresponding to the C++ File base class.
pub struct BaseFile {
    name: AsciiString,
    access: FileAccess,
    is_open: bool,
    delete_on_close: bool,
}

impl BaseFile {
    /// Create a new BaseFile instance
    pub fn new() -> Self {
        Self {
            name: AsciiString::from("<no file>"),
            access: FileAccess::NONE,
            is_open: false,
            delete_on_close: false,
        }
    }

    /// Open file with access validation
    ///
    /// This method performs the common validation logic from the C++ implementation
    /// before delegating to the specific file type's open implementation.
    pub fn open_base(&mut self, filename: &str, access: FileAccess) -> Result<(), io::Error> {
        if self.is_open {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "File already open",
            ));
        }

        self.set_name(filename);

        // Validate access flags (converted from C++ validation logic)
        if access.contains(FileAccess::STREAMING) && access.contains(FileAccess::WRITE) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Illegal access: streaming with write",
            ));
        }

        if access.contains(FileAccess::TEXT) && access.contains(FileAccess::BINARY) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Illegal access: text and binary",
            ));
        }

        let mut final_access = access;

        // Apply default access modes (from C++ logic)
        if !access.contains(FileAccess::READ) && !access.contains(FileAccess::WRITE) {
            final_access = final_access.combine(FileAccess::READ);
        }

        if !access.contains(FileAccess::READ) && !access.contains(FileAccess::APPEND) {
            final_access = final_access.combine(FileAccess::TRUNCATE);
        }

        if !access.contains(FileAccess::TEXT) && !access.contains(FileAccess::BINARY) {
            final_access = final_access.combine(FileAccess::BINARY);
        }

        self.access = final_access;
        self.is_open = true;

        Ok(())
    }

    /// Close the file
    pub fn close_base(&mut self) {
        if self.is_open {
            self.set_name("<no file>");
            self.is_open = false;
        }
    }

    /// Calculate file size using seek operations
    ///
    /// This provides a default implementation that can be overridden by specific file types.
    pub fn size_default<T: File>(&self, file: &mut T) -> i32 {
        let pos = file.seek(0, SeekMode::Current).unwrap_or(0);
        let size = file.seek(0, SeekMode::End).unwrap_or(0);
        let _ = file.seek(pos, SeekMode::Start);

        if size < 0 {
            0
        } else {
            size
        }
    }

    /// Get current position in file
    pub fn position_default<T: File>(&self, file: &mut T) -> i32 {
        file.seek(0, SeekMode::Current).unwrap_or(0)
    }

    /// Print formatted text to file
    pub fn print_default<T: File>(&self, file: &mut T, text: &str) -> Result<bool, io::Error> {
        if !self.access.contains(FileAccess::TEXT) {
            return Ok(false);
        }

        let bytes = text.as_bytes();
        let written = file.write(bytes)?;
        Ok(written == bytes.len())
    }

    /// Check if at end of file
    pub fn eof_default<T: File>(&self, file: &T) -> bool {
        file.position() == file.size()
    }

    /// Set delete on close flag
    pub fn delete_on_close(&mut self) {
        self.delete_on_close = true;
    }

    /// Return whether the file is flagged for deletion without resetting the flag.
    pub fn should_delete_on_close(&self) -> bool {
        self.delete_on_close
    }

    /// Consume the delete-on-close flag, resetting it to `false`.
    pub fn take_delete_on_close(&mut self) -> bool {
        let delete = self.delete_on_close;
        self.delete_on_close = false;
        delete
    }

    /// Check if file is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Get the filename
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    /// Set the filename
    pub fn set_name(&mut self, name: &str) {
        self.name = AsciiString::from(name);
    }

    /// Get access flags
    pub fn get_access(&self) -> FileAccess {
        self.access
    }
}

impl Default for BaseFile {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BaseFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "File(name: {}, open: {}, access: {:?})",
            self.name.as_str(),
            self.is_open,
            self.access
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_access_flags() {
        assert!(FileAccess::READ.contains(FileAccess::READ));
        assert!(!FileAccess::READ.contains(FileAccess::WRITE));

        let combined = FileAccess::READ.combine(FileAccess::WRITE);
        assert!(combined.contains(FileAccess::READ));
        assert!(combined.contains(FileAccess::WRITE));
        assert_eq!(combined, FileAccess::READWRITE);
    }

    #[test]
    fn test_base_file_creation() {
        let file = BaseFile::new();
        assert_eq!(file.get_name(), "<no file>");
        assert!(!file.is_open());
        assert_eq!(file.get_access(), FileAccess::NONE);
    }

    #[test]
    fn test_base_file_validation() {
        let mut file = BaseFile::new();

        // Test invalid access combinations
        assert!(file
            .open_base("test.txt", FileAccess::STREAMING.combine(FileAccess::WRITE))
            .is_err());
        assert!(file
            .open_base("test.txt", FileAccess::TEXT.combine(FileAccess::BINARY))
            .is_err());
    }
}
