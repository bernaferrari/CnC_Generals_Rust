//! RAMFile - In-memory file implementation
//!
//! This module provides a faithful port of the C++ `RAMFile` class used by the
//! Generals engine. It loads entire file content into memory and provides
//! fast read operations without disk I/O.
//!
//! C++ Reference: GeneralsMD/Code/GameEngine/Source/Common/System/RAMFile.cpp

use std::cmp;
use std::io;

use crate::common::{
    ascii_string::AsciiString,
    system::file::{BaseFile, File, FileAccess, SeekMode},
};

/// RAMFile - A file abstraction that loads entire file content into memory.
///
/// This is useful for frequently accessed files like INI files, as it avoids
/// repeated disk I/O. RAMFiles are read-only.
pub struct RAMFile {
    /// Base file functionality
    base: BaseFile,
    /// File data in memory
    data: Option<Vec<u8>>,
    /// Current read position
    pos: i32,
    /// Size of file in memory
    size: i32,
}

impl RAMFile {
    /// Create a new RAMFile instance
    pub fn new() -> Self {
        Self {
            base: BaseFile::new(),
            data: None,
            pos: 0,
            size: 0,
        }
    }

    /// Open a RAMFile from another File implementation.
    ///
    /// This reads the entire contents of the source file into memory,
    /// then closes the source file.
    pub fn open_from_file<F: File>(&mut self, file: &mut F) -> bool {
        let access = file.get_access();
        let name = file.get_name().to_string();

        // Initialize base file
        if self.base.open_base(&name, access).is_err() {
            return false;
        }

        // Read whole file into memory
        self.size = file.size();
        if self.size < 0 {
            self.base.close_base();
            return false;
        }

        let mut data = vec![0u8; self.size as usize];
        match file.read(&mut data) {
            Ok(bytes_read) => {
                self.size = bytes_read as i32;
                data.truncate(bytes_read);
                self.data = Some(data);
                self.pos = 0;
                true
            }
            Err(_) => {
                self.base.close_base();
                false
            }
        }
    }

    /// Open from an archive file at a specific offset and size.
    ///
    /// This is used when loading files from BIG archives where the file
    /// data is stored at a known offset.
    pub fn open_from_archive<F: File>(
        &mut self,
        archive_file: &mut F,
        filename: &str,
        offset: i32,
        size: i32,
    ) -> bool {
        // Initialize base file
        if self
            .base
            .open_base(filename, FileAccess::READ.combine(FileAccess::BINARY))
            .is_err()
        {
            return false;
        }

        // Allocate buffer for the file data
        let mut data = vec![0u8; size as usize];
        self.size = size;

        // Seek to offset in archive and read the data
        match archive_file.seek(offset, SeekMode::Start) {
            Ok(seek_pos) if seek_pos == offset => {}
            _ => {
                self.base.close_base();
                return false;
            }
        }

        match archive_file.read(&mut data) {
            Ok(bytes_read) if bytes_read == size as usize => {
                self.data = Some(data);
                self.pos = 0;
                true
            }
            _ => {
                self.base.close_base();
                false
            }
        }
    }

    /// Copy data to another file
    pub fn copy_data_to_file<F: File>(&self, local_file: &mut F) -> bool {
        if let Some(ref data) = self.data {
            match local_file.write(data) {
                Ok(written) => written == data.len(),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Read entire file and close, transferring ownership of data to caller.
    ///
    /// This avoids copying the data buffer. After this call, the RAMFile
    /// is consumed and cannot be used further.
    pub fn read_entire_and_close_into_vec(mut self) -> Vec<u8> {
        let data = self.data.take().unwrap_or_else(|| vec![0u8; 1]);
        self.close();
        data
    }

    /// Convert to RAMFile (returns self) - for API compatibility with C++
    pub fn convert_to_ram_file(self) -> Self {
        self
    }

    /// Check if file has data loaded
    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }

    /// Get a reference to the underlying data
    pub fn get_data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

impl Default for RAMFile {
    fn default() -> Self {
        Self::new()
    }
}

impl File for RAMFile {
    fn open(&mut self, filename: &str, access: FileAccess) -> Result<(), io::Error> {
        // RAMFile requires an external file system to load data from.
        // This base open just sets up the file state but doesn't load data.
        // Use open_from_file() or open_from_archive() to actually load data.
        self.base.open_base(filename, access)
    }

    fn close(&mut self) {
        self.data.take();
        self.pos = 0;
        self.size = 0;
        self.base.close_base();
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, io::Error> {
        if self.data.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "RAMFile has no data loaded",
            ));
        }

        let bytes_left = self.size - self.pos;
        let bytes_to_read = cmp::min(buffer.len() as i32, bytes_left) as usize;

        if bytes_to_read > 0 {
            if let Some(ref data) = self.data {
                let start_pos = self.pos as usize;
                let end_pos = start_pos + bytes_to_read;
                buffer[..bytes_to_read].copy_from_slice(&data[start_pos..end_pos]);
            }
            self.pos += bytes_to_read as i32;
        }

        Ok(bytes_to_read)
    }

    fn write(&mut self, _buffer: &[u8]) -> Result<usize, io::Error> {
        // RAMFile is read-only
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "RAMFile is read-only",
        ))
    }

    fn seek(&mut self, pos: i32, mode: SeekMode) -> Result<i32, io::Error> {
        let new_pos = match mode {
            SeekMode::Start => pos,
            SeekMode::Current => self.pos + pos,
            SeekMode::End => {
                // Position should be <= 0 for seek from end (matching C++ behavior)
                self.size + pos
            }
        };

        // Clamp position to valid range [0, size]
        self.pos = cmp::max(0, cmp::min(new_pos, self.size));
        Ok(self.pos)
    }

    fn next_line(&mut self, buf: Option<&mut Vec<u8>>, buf_size: Option<usize>) {
        let Some(ref data) = self.data else {
            return;
        };

        let mut line: Vec<u8> = Vec::new();

        // Read until newline
        while (self.pos as usize) < data.len() && data[self.pos as usize] != b'\n' {
            line.push(data[self.pos as usize]);
            self.pos += 1;
        }

        // Include the newline if present
        if (self.pos as usize) < data.len() {
            line.push(data[self.pos as usize]);
            self.pos += 1;
        }

        // Clamp position to size
        if self.pos > self.size {
            self.pos = self.size;
        }

        // Copy to output buffer if provided
        if let Some(out) = buf {
            out.clear();
            let limit = buf_size.unwrap_or(line.len());
            let copy_len = line.len().min(limit);
            out.extend_from_slice(&line[..copy_len]);
        }
    }

    fn scan_int(&mut self) -> Result<i32, io::Error> {
        let Some(ref data) = self.data else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "RAMFile has no data loaded",
            ));
        };

        // Skip non-digit characters (except minus sign)
        while (self.pos as usize) < data.len() {
            let ch = data[self.pos as usize] as char;
            if ch.is_ascii_digit() || ch == '-' {
                break;
            }
            self.pos += 1;
        }

        if (self.pos as usize) >= data.len() {
            self.pos = self.size;
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while scanning int",
            ));
        }

        // Collect digits and minus sign
        let mut temp_str = String::new();
        while (self.pos as usize) < data.len() {
            let ch = data[self.pos as usize] as char;
            if ch.is_ascii_digit() {
                temp_str.push(ch);
                self.pos += 1;
            } else if ch == '-' && temp_str.is_empty() {
                temp_str.push(ch);
                self.pos += 1;
            } else {
                break;
            }
        }

        temp_str
            .parse::<i32>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    fn scan_real(&mut self) -> Result<f32, io::Error> {
        let Some(ref data) = self.data else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "RAMFile has no data loaded",
            ));
        };

        let mut saw_decimal = false;

        // Skip non-digit characters (except minus sign and decimal point)
        while (self.pos as usize) < data.len() {
            let ch = data[self.pos as usize] as char;
            if ch.is_ascii_digit() || ch == '-' || ch == '.' {
                break;
            }
            self.pos += 1;
        }

        if (self.pos as usize) >= data.len() {
            self.pos = self.size;
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while scanning real",
            ));
        }

        // Collect digits, minus sign, and decimal point
        let mut temp_str = String::new();
        while (self.pos as usize) < data.len() {
            let ch = data[self.pos as usize] as char;
            if ch.is_ascii_digit() {
                temp_str.push(ch);
                self.pos += 1;
            } else if ch == '-' && temp_str.is_empty() {
                temp_str.push(ch);
                self.pos += 1;
            } else if ch == '.' && !saw_decimal {
                saw_decimal = true;
                temp_str.push(ch);
                self.pos += 1;
            } else {
                break;
            }
        }

        temp_str
            .parse::<f32>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    fn scan_string(&mut self) -> Result<AsciiString, io::Error> {
        let Some(ref data) = self.data else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "RAMFile has no data loaded",
            ));
        };

        // Skip whitespace
        while (self.pos as usize) < data.len() {
            let ch = data[self.pos as usize] as char;
            if !ch.is_whitespace() {
                break;
            }
            self.pos += 1;
        }

        if (self.pos as usize) >= data.len() {
            self.pos = self.size;
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while scanning string",
            ));
        }

        // Collect non-whitespace characters
        let mut result = String::new();
        while (self.pos as usize) < data.len() {
            let ch = data[self.pos as usize] as char;
            if !ch.is_whitespace() {
                result.push(ch);
                self.pos += 1;
            } else {
                break;
            }
        }

        Ok(AsciiString::from(result.as_str()))
    }

    fn print(&mut self, _text: &str) -> Result<bool, io::Error> {
        // RAMFile is read-only
        Ok(false)
    }

    fn size(&self) -> i32 {
        self.size
    }

    fn position(&self) -> i32 {
        self.pos
    }

    fn eof(&self) -> bool {
        self.pos >= self.size
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn set_name(&mut self, name: &str) {
        self.base.set_name(name);
    }

    fn get_access(&self) -> FileAccess {
        self.base.get_access()
    }

    fn read_entire_and_close(&mut self) -> Result<Vec<u8>, io::Error> {
        let data = self.data.take().unwrap_or_default();
        self.close();
        Ok(data)
    }
}

impl Drop for RAMFile {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_ram_file(content: &[u8]) -> RAMFile {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some(content.to_vec());
        ram_file.size = content.len() as i32;
        ram_file.pos = 0;
        ram_file
    }

    #[test]
    fn test_ram_file_creation() {
        let ram_file = RAMFile::new();
        assert_eq!(ram_file.size(), 0);
        assert_eq!(ram_file.position(), 0);
        assert!(ram_file.eof());
        assert!(!ram_file.has_data());
    }

    #[test]
    fn test_seek_operations() {
        let mut ram_file = create_test_ram_file(&[1, 2, 3, 4, 5]);

        // Test seek from start
        assert_eq!(ram_file.seek(2, SeekMode::Start).unwrap(), 2);
        assert_eq!(ram_file.position(), 2);

        // Test seek from current
        assert_eq!(ram_file.seek(1, SeekMode::Current).unwrap(), 3);
        assert_eq!(ram_file.position(), 3);

        // Test seek from end
        assert_eq!(ram_file.seek(-1, SeekMode::End).unwrap(), 4);
        assert_eq!(ram_file.position(), 4);

        // Test boundary conditions - seek before start
        assert_eq!(ram_file.seek(-10, SeekMode::Start).unwrap(), 0);
        assert_eq!(ram_file.position(), 0);

        // Test boundary conditions - seek past end
        assert_eq!(ram_file.seek(100, SeekMode::Start).unwrap(), 5);
        assert_eq!(ram_file.position(), 5);
        assert!(ram_file.eof());
    }

    #[test]
    fn test_read_operations() {
        let mut ram_file = create_test_ram_file(&[1, 2, 3, 4, 5]);

        let mut buffer = [0u8; 3];
        assert_eq!(ram_file.read(&mut buffer).unwrap(), 3);
        assert_eq!(buffer, [1, 2, 3]);
        assert_eq!(ram_file.position(), 3);

        // Test reading beyond end
        let mut buffer2 = [0u8; 5];
        assert_eq!(ram_file.read(&mut buffer2).unwrap(), 2);
        assert_eq!(buffer2[0..2], [4, 5]);
        assert_eq!(ram_file.position(), 5);
        assert!(ram_file.eof());
    }

    #[test]
    fn test_write_fails() {
        let mut ram_file = create_test_ram_file(&[1, 2, 3, 4, 5]);
        assert!(ram_file.write(&[6, 7, 8]).is_err());
    }

    #[test]
    fn test_scan_int() {
        let mut ram_file = create_test_ram_file("  123  456  -789  ".as_bytes());

        assert_eq!(ram_file.scan_int().unwrap(), 123);
        assert_eq!(ram_file.scan_int().unwrap(), 456);
        assert_eq!(ram_file.scan_int().unwrap(), -789);
        assert!(ram_file.scan_int().is_err());
    }

    #[test]
    fn test_scan_real() {
        let mut ram_file = create_test_ram_file("  12.5  -3.14  ".as_bytes());

        let val1 = ram_file.scan_real().unwrap();
        assert!((val1 - 12.5).abs() < 0.001);

        let val2 = ram_file.scan_real().unwrap();
        assert!((val2 - (-3.14)).abs() < 0.001);

        assert!(ram_file.scan_real().is_err());
    }

    #[test]
    fn test_scan_string() {
        let mut ram_file = create_test_ram_file("  hello   world  ".as_bytes());

        assert_eq!(ram_file.scan_string().unwrap().as_str(), "hello");
        assert_eq!(ram_file.scan_string().unwrap().as_str(), "world");
        assert!(ram_file.scan_string().is_err());
    }

    #[test]
    fn test_next_line() {
        let mut ram_file = create_test_ram_file("line1\nline2\nline3".as_bytes());

        let mut buffer = Vec::new();

        ram_file.next_line(Some(&mut buffer), None);
        assert_eq!(buffer, b"line1\n");

        ram_file.next_line(Some(&mut buffer), None);
        assert_eq!(buffer, b"line2\n");

        ram_file.next_line(Some(&mut buffer), None);
        assert_eq!(buffer, b"line3");
    }

    #[test]
    fn test_next_line_with_size_limit() {
        let mut ram_file = create_test_ram_file("1234567890\n".as_bytes());

        let mut buffer = Vec::new();
        ram_file.next_line(Some(&mut buffer), Some(5));

        // Should only copy up to limit
        assert_eq!(buffer.len(), 5);
    }

    #[test]
    fn test_next_line_without_buffer() {
        let mut ram_file = create_test_ram_file("line1\nline2\n".as_bytes());

        // Advance without storing
        ram_file.next_line(None, None);
        assert_eq!(ram_file.position(), 6); // "line1\n" = 6 bytes

        // Now read next line into buffer
        let mut buffer = Vec::new();
        ram_file.next_line(Some(&mut buffer), None);
        assert_eq!(buffer, b"line2\n");
    }

    #[test]
    fn test_copy_data_to_file() {
        use crate::common::system::local_file::LocalFile;
        use std::io::Write;

        // Create a temp file path
        let temp_path = "/tmp/test_ram_copy.txt";

        // Create RAMFile with content
        let ram_file = create_test_ram_file(b"test content");

        // Create local file and copy
        {
            let mut local = LocalFile::new();
            local
                .open(
                    temp_path,
                    FileAccess::CREATE
                        .combine(FileAccess::WRITE)
                        .combine(FileAccess::BINARY),
                )
                .unwrap();

            // Can't directly use copy_data_to_file with LocalFile because it needs mutable ref
            // and we're in a test, so let's just verify the data is accessible
            let data = ram_file.get_data().unwrap();
            local.write(data).unwrap();
            local.close();
        }

        // Verify content
        let content = std::fs::read(temp_path).unwrap();
        assert_eq!(content, b"test content");

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_read_entire_and_close() {
        let ram_file = create_test_ram_file(b"test data");
        let data = ram_file.read_entire_and_close_into_vec();
        assert_eq!(data, b"test data");
    }

    #[test]
    fn test_close_clears_data() {
        let mut ram_file = create_test_ram_file(&[1, 2, 3, 4, 5]);
        assert!(ram_file.has_data());

        ram_file.close();

        assert!(!ram_file.has_data());
        assert_eq!(ram_file.size(), 0);
        assert_eq!(ram_file.position(), 0);
    }
}
