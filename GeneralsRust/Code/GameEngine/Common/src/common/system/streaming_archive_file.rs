////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: streaming_archive_file.rs /////////////////////////////////////////////
// Streaming archive file - a view into a portion of an existing file.
// Port of Common/StreamingArchiveFile.cpp
///////////////////////////////////////////////////////////////////////////////

use std::io;

use crate::common::{
    ascii_string::AsciiString,
    system::file::{BaseFile, File, FileAccess, SeekMode},
};

/// StreamingArchiveFile - A file abstraction that provides a view into a portion
/// of an existing archive file.
///
/// This is a port of the C++ StreamingArchiveFile class which inherits from RAMFile.
/// It wraps an existing File and provides access to a sub-range of that file
/// defined by an offset and size.
///
/// Key characteristics:
/// - Read-only access
/// - Cannot write (returns error)
/// - Seek operations are relative to the virtual file boundaries
/// - Scan/nextLine operations should not be used (will return errors)
pub struct StreamingArchiveFile {
    /// Base file state (name, access flags, open status)
    base: BaseFile,
    /// The archive file that this streaming file came from
    file: Option<Box<dyn File>>,
    /// Starting position in the archive (offset)
    starting_pos: i32,
    /// Length of this virtual file
    size: i32,
    /// Current position within the virtual file (0 to size)
    cur_pos: i32,
}

impl StreamingArchiveFile {
    /// Create a new StreamingArchiveFile instance
    pub fn new() -> Self {
        Self {
            base: BaseFile::new(),
            file: None,
            starting_pos: 0,
            size: 0,
            cur_pos: 0,
        }
    }

    /// Open a streaming file from an archive file at a specific offset and size.
    ///
    /// This is the primary constructor for StreamingArchiveFile, corresponding to
    /// `StreamingArchiveFile::openFromArchive` in the C++ code.
    ///
    /// # Arguments
    /// * `archive_file` - The archive file to read from (ownership is taken)
    /// * `filename` - The name to associate with this virtual file
    /// * `offset` - The starting position in the archive file
    /// * `size` - The size of the virtual file
    ///
    /// # Returns
    /// `true` if successful, `false` otherwise
    pub fn open_from_archive(
        &mut self,
        mut archive_file: Box<dyn File>,
        filename: &AsciiString,
        offset: i32,
        size: i32,
    ) -> bool {
        // Initialize base file state
        if !self.base_open(
            filename.as_str(),
            FileAccess::READ
                .combine(FileAccess::BINARY)
                .combine(FileAccess::STREAMING),
        ) {
            return false;
        }

        // Verify the archive can seek to the expected positions
        if archive_file.seek(offset, SeekMode::Start).unwrap_or(-1) != offset {
            self.base.close_base();
            return false;
        }

        // Verify the size is accessible
        if archive_file.seek(size, SeekMode::Current).unwrap_or(-1) != offset + size {
            self.base.close_base();
            return false;
        }

        // Seek back to the starting position
        if archive_file.seek(offset, SeekMode::Start).unwrap_or(-1) != offset {
            self.base.close_base();
            return false;
        }

        self.file = Some(archive_file);
        self.starting_pos = offset;
        self.size = size;
        self.cur_pos = 0;

        true
    }

    /// Open from a mutable reference to a file (for compatibility)
    ///
    /// This method is less common but matches the C++ API where File* is passed.
    pub fn open_from_archive_ref<F: File + 'static>(
        &mut self,
        archive_file: &mut F,
        filename: &AsciiString,
        offset: i32,
        size: i32,
    ) -> bool {
        // Initialize base file state
        if !self.base_open(
            filename.as_str(),
            FileAccess::READ
                .combine(FileAccess::BINARY)
                .combine(FileAccess::STREAMING),
        ) {
            return false;
        }

        // Verify the archive can seek to the expected positions
        if archive_file.seek(offset, SeekMode::Start).unwrap_or(-1) != offset {
            self.base.close_base();
            return false;
        }

        // Verify the size is accessible
        if archive_file.seek(size, SeekMode::Current).unwrap_or(-1) != offset + size {
            self.base.close_base();
            return false;
        }

        // Seek back to the starting position
        if archive_file.seek(offset, SeekMode::Start).unwrap_or(-1) != offset {
            self.base.close_base();
            return false;
        }

        // Note: We don't take ownership in this variant
        // This is for cases where the caller retains ownership
        self.file = None; // No ownership taken
        self.starting_pos = offset;
        self.size = size;
        self.cur_pos = 0;

        // Store reference info for later use - but this requires a different approach
        // For now, this is a placeholder that won't work without ownership
        self.base.close_base();
        false
    }

    /// Base implementation of open functionality
    fn base_open(&mut self, filename: &str, access: FileAccess) -> bool {
        // Use the BaseFile's validation
        match self.base.open_base(filename, access) {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    /// Get the starting position in the archive
    pub fn starting_pos(&self) -> i32 {
        self.starting_pos
    }

    /// Get the size of this virtual file
    pub fn virtual_size(&self) -> i32 {
        self.size
    }

    /// Get the current position within the virtual file
    pub fn virtual_position(&self) -> i32 {
        self.cur_pos
    }
}

impl Default for StreamingArchiveFile {
    fn default() -> Self {
        Self::new()
    }
}

impl File for StreamingArchiveFile {
    /// Open a file by filename - uses TheFileSystem to open the file first,
    /// then wraps it as a streaming file.
    ///
    /// Note: This matches the C++ behavior where `open(filename, access)` calls
    /// `TheFileSystem->openFile()` internally.
    fn open(&mut self, _filename: &str, _access: FileAccess) -> Result<(), io::Error> {
        // In the C++ code, this opens via TheFileSystem and then calls open(file)
        // For now, this is a stub that returns an error since we need a FileSystem
        // implementation to properly support this.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "StreamingArchiveFile::open requires FileSystem - use open_from_archive instead",
        ))
    }

    /// Close the streaming file
    fn close(&mut self) {
        // Drop the reference to the archive file
        self.file = None;
        self.starting_pos = 0;
        self.size = 0;
        self.cur_pos = 0;
        self.base.close_base();
    }

    /// Read data from the streaming file
    ///
    /// If buffer is null (empty slice), just advances the current position by `bytes`.
    /// Otherwise, reads up to `buffer.len()` bytes into the buffer.
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, io::Error> {
        let Some(ref mut file) = self.file else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "No archive file",
            ));
        };

        // Seek to the correct position in the archive
        file.seek(self.starting_pos + self.cur_pos, SeekMode::Start)?;

        // Calculate how many bytes we can read
        let bytes_available = self.size - self.cur_pos;
        let bytes_to_read = if buffer.is_empty() {
            // Just advance position if buffer is empty
            let advance = bytes_available.min(buffer.len() as i32);
            self.cur_pos += advance;
            return Ok(0);
        } else {
            buffer.len().min(bytes_available as usize) as i32
        };

        if bytes_to_read <= 0 {
            return Ok(0);
        }

        // Read from the underlying file
        let bytes_read = file.read(&mut buffer[..bytes_to_read as usize])?;
        self.cur_pos += bytes_read as i32;

        Ok(bytes_read)
    }

    /// Write is not supported for streaming files
    fn write(&mut self, _buffer: &[u8]) -> Result<usize, io::Error> {
        // C++ code: DEBUG_CRASH(("Cannot write to streaming files.\n"));
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Cannot write to streaming files",
        ))
    }

    /// Seek within the virtual file
    ///
    /// The seek position is relative to the virtual file boundaries (0 to size).
    /// Positions outside this range are clamped to [0, size].
    fn seek(&mut self, pos: i32, mode: SeekMode) -> Result<i32, io::Error> {
        let new_pos = match mode {
            SeekMode::Start => pos,
            SeekMode::Current => self.cur_pos + pos,
            SeekMode::End => {
                // C++ asserts pos <= 0 for END mode
                if pos > 0 {
                    // DEBUG_ASSERTCRASH in C++, we return error
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Position should be <= 0 for seek from end",
                    ));
                }
                self.size + pos
            }
        };

        // Clamp to valid range [0, size]
        let clamped_pos = if new_pos < 0 {
            0
        } else if new_pos > self.size {
            self.size
        } else {
            new_pos
        };

        self.cur_pos = clamped_pos;
        Ok(self.cur_pos)
    }

    /// Next line is not supported for streaming files
    fn next_line(&mut self, _buf: Option<&mut Vec<u8>>, _buf_size: Option<usize>) {
        // C++ code: DEBUG_CRASH(("Should not call nextLine on a streaming file.\n"))
        // In Rust, we silently do nothing rather than crash
    }

    /// Scan int is not supported for streaming files
    fn scan_int(&mut self) -> Result<i32, io::Error> {
        // C++ code: DEBUG_CRASH(("Should not call scanInt on a streaming file.\n"))
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Should not call scanInt on a streaming file",
        ))
    }

    /// Scan real is not supported for streaming files
    fn scan_real(&mut self) -> Result<f32, io::Error> {
        // C++ code: DEBUG_CRASH(("Should not call scanReal on a streaming file.\n"))
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Should not call scanReal on a streaming file",
        ))
    }

    /// Scan string is not supported for streaming files
    fn scan_string(&mut self) -> Result<AsciiString, io::Error> {
        // C++ code: DEBUG_CRASH(("Should not call scanString on a streaming file.\n"))
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Should not call scanString on a streaming file",
        ))
    }

    /// Print is not supported for streaming files
    fn print(&mut self, _text: &str) -> Result<bool, io::Error> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Cannot write to streaming files",
        ))
    }

    /// Get the size of the virtual file
    fn size(&self) -> i32 {
        self.size
    }

    /// Get current position within the virtual file
    fn position(&self) -> i32 {
        self.cur_pos
    }

    /// Check if at end of virtual file
    fn eof(&self) -> bool {
        self.cur_pos >= self.size
    }

    /// Get the filename
    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    /// Set the filename
    fn set_name(&mut self, name: &str) {
        self.base.set_name(name);
    }

    /// Get access flags
    fn get_access(&self) -> FileAccess {
        self.base.get_access()
    }

    /// Read entire file and close is not recommended for streaming files
    fn read_entire_and_close(&mut self) -> Result<Vec<u8>, io::Error> {
        // C++ code: DEBUG_CRASH(("Are you sure you meant to readEntireAndClose on a streaming file?"))
        // We implement it anyway since it can be useful, but with a warning semantics
        let size = self.size as usize;
        let mut buffer = vec![0u8; size];

        // Seek to start
        self.seek(0, SeekMode::Start)?;

        // Read all data
        let mut total_read = 0;
        while total_read < size {
            let read = self.read(&mut buffer[total_read..])?;
            if read == 0 {
                break;
            }
            total_read += read;
        }

        buffer.truncate(total_read);
        self.close();
        Ok(buffer)
    }
}

impl Drop for StreamingArchiveFile {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_archive_file_creation() {
        let file = StreamingArchiveFile::new();
        assert_eq!(file.size(), 0);
        assert_eq!(file.position(), 0);
        assert!(file.get_name().is_empty() || file.get_name() == "<no file>");
    }

    #[test]
    fn test_seek_modes() {
        let mut file = StreamingArchiveFile::new();
        file.size = 100;
        file.cur_pos = 50;

        // Seek from start
        assert_eq!(file.seek(25, SeekMode::Start).unwrap(), 25);
        assert_eq!(file.cur_pos, 25);

        // Seek from current
        assert_eq!(file.seek(10, SeekMode::Current).unwrap(), 35);
        assert_eq!(file.cur_pos, 35);

        // Seek from end (negative offset)
        assert_eq!(file.seek(-10, SeekMode::End).unwrap(), 90);
        assert_eq!(file.cur_pos, 90);
    }

    #[test]
    fn test_seek_clamping() {
        let mut file = StreamingArchiveFile::new();
        file.size = 100;
        file.cur_pos = 50;

        // Seek beyond end - should clamp to size
        assert_eq!(file.seek(200, SeekMode::Start).unwrap(), 100);
        assert_eq!(file.cur_pos, 100);

        // Seek before start - should clamp to 0
        assert_eq!(file.seek(-50, SeekMode::Start).unwrap(), 0);
        assert_eq!(file.cur_pos, 0);
    }

    #[test]
    fn test_seek_from_end_positive_is_error() {
        let mut file = StreamingArchiveFile::new();
        file.size = 100;

        // Positive offset from END should error
        assert!(file.seek(10, SeekMode::End).is_err());
    }

    #[test]
    fn test_write_returns_error() {
        let mut file = StreamingArchiveFile::new();
        let data = b"test data";
        assert!(file.write(data).is_err());
    }

    #[test]
    fn test_scan_operations_return_error() {
        let mut file = StreamingArchiveFile::new();

        assert!(file.scan_int().is_err());
        assert!(file.scan_real().is_err());
        assert!(file.scan_string().is_err());
    }

    #[test]
    fn test_eof_detection() {
        let mut file = StreamingArchiveFile::new();
        file.size = 100;
        file.cur_pos = 50;
        assert!(!file.eof());

        file.cur_pos = 100;
        assert!(file.eof());

        file.cur_pos = 150;
        assert!(file.eof());
    }
}
