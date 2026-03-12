////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//----------------------------------------------------------------------------
//
//                       Westwood Studios Pacific.
//
//                       Confidential Information
//                Copyright(C) 2001 - All Rights Reserved
//
//----------------------------------------------------------------------------
//
// Project:   WSYS Library
//
// Module:    IO
//
// File name: ram_file.rs
//
// Created:   11/08/01
//
//----------------------------------------------------------------------------

use std::cmp;

/// Seek mode enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeekMode {
    Start,
    Current,
    End,
}

/// File access flags
pub mod access {
    pub const READ: u32 = 0x01;
    pub const WRITE: u32 = 0x02;
    pub const BINARY: u32 = 0x04;
}

/// Trait for file operations - base class equivalent
pub trait File {
    fn open(&mut self, filename: &str, access_flags: u32) -> bool;
    fn close(&mut self);
    fn read(&mut self, buffer: &mut [u8]) -> i32;
    fn write(&mut self, buffer: &[u8]) -> i32;
    fn seek(&mut self, pos: i32, mode: SeekMode) -> i32;
    fn size(&self) -> i32;
    fn get_name(&self) -> &str;
    fn get_access(&self) -> u32;
}

/// RAMFile - A file abstraction that loads entire file content into memory
pub struct RAMFile {
    /// File data in memory
    data: Option<Vec<u8>>,
    /// Current read position
    pos: i32,
    /// Size of file in memory
    size: i32,
    /// File name
    name: String,
    /// Access flags
    access_flags: u32,
    /// Whether file is open
    is_open: bool,
}

impl RAMFile {
    /// Create a new RAMFile instance
    pub fn new() -> Self {
        Self {
            data: None,
            pos: 0,
            size: 0,
            name: String::new(),
            access_flags: 0,
            is_open: false,
        }
    }

    /// Open a file from another File implementation
    pub fn open_from_file<F: File>(&mut self, file: &mut F) -> bool {
        if !self.base_open(file.get_name(), file.get_access()) {
            return false;
        }

        // Read whole file into memory
        self.size = file.size();
        let mut data = vec![0u8; self.size as usize];

        let bytes_read = file.read(&mut data);
        if bytes_read < 0 {
            return false;
        }

        self.size = bytes_read;
        data.truncate(bytes_read as usize);
        self.data = Some(data);
        self.pos = 0;

        true
    }

    /// Open from an archive file at a specific offset and size
    pub fn open_from_archive<F: File>(
        &mut self,
        archive_file: &mut F,
        filename: &str,
        offset: i32,
        size: i32,
    ) -> bool {
        if !self.base_open(filename, access::READ | access::BINARY) {
            return false;
        }

        // Allocate buffer for the file data
        let mut data = vec![0u8; size as usize];
        self.size = size;

        // Seek to offset in archive and read the data
        if archive_file.seek(offset, SeekMode::Start) != offset {
            return false;
        }

        let bytes_read = archive_file.read(&mut data);
        if bytes_read != size {
            return false;
        }

        self.data = Some(data);
        self.name = filename.to_string();
        self.pos = 0;

        true
    }

    /// Copy data to another file
    pub fn copy_data_to_file<F: File>(&self, local_file: &mut F) -> bool {
        if let Some(ref data) = self.data {
            local_file.write(data) == self.size
        } else {
            false
        }
    }

    /// Read entire file and close, transferring ownership of data to caller
    pub fn read_entire_and_close(mut self) -> Vec<u8> {
        let data = self.data.take().unwrap_or_else(|| vec![0u8; 1]);
        self.close();
        data
    }

    /// Convert to RAMFile (returns self)
    pub fn convert_to_ram_file(self) -> Self {
        self
    }

    /// Scan for an integer in the current position
    pub fn scan_int(&mut self) -> Option<i32> {
        if let Some(ref data) = self.data {
            let mut temp_str = String::new();

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
                return None;
            }

            // Collect digits and minus sign
            while (self.pos as usize) < data.len() {
                let ch = data[self.pos as usize] as char;
                if ch.is_ascii_digit() || (ch == '-' && temp_str.is_empty()) {
                    temp_str.push(ch);
                    self.pos += 1;
                } else {
                    break;
                }
            }

            temp_str.parse().ok()
        } else {
            None
        }
    }

    /// Scan for a real (float) in the current position
    pub fn scan_real(&mut self) -> Option<f32> {
        if let Some(ref data) = self.data {
            let mut temp_str = String::new();
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
                return None;
            }

            // Collect digits, minus sign, and decimal point
            while (self.pos as usize) < data.len() {
                let ch = data[self.pos as usize] as char;
                if ch.is_ascii_digit()
                    || (ch == '-' && temp_str.is_empty())
                    || (ch == '.' && !saw_decimal)
                {
                    if ch == '.' {
                        saw_decimal = true;
                    }
                    temp_str.push(ch);
                    self.pos += 1;
                } else {
                    break;
                }
            }

            temp_str.parse().ok()
        } else {
            None
        }
    }

    /// Scan for a string (whitespace-delimited) in the current position
    pub fn scan_string(&mut self) -> Option<String> {
        if let Some(ref data) = self.data {
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
                return None;
            }

            let mut result = String::new();

            // Collect non-whitespace characters
            while (self.pos as usize) < data.len() {
                let ch = data[self.pos as usize] as char;
                if !ch.is_whitespace() {
                    result.push(ch);
                    self.pos += 1;
                } else {
                    break;
                }
            }

            Some(result)
        } else {
            None
        }
    }

    /// Read next line into buffer, or just advance position if buffer is None
    pub fn next_line(&mut self, buf: Option<&mut String>) {
        if let Some(ref data) = self.data {
            if let Some(buffer) = buf {
                buffer.clear();

                // Read until newline
                while (self.pos as usize) < data.len() && data[self.pos as usize] != b'\n' {
                    buffer.push(data[self.pos as usize] as char);
                    self.pos += 1;
                }

                // Include the newline if present
                if (self.pos as usize) < data.len() {
                    buffer.push(data[self.pos as usize] as char);
                    self.pos += 1;
                }
            } else {
                // Just advance position to after next newline
                while (self.pos as usize) < data.len() && data[self.pos as usize] != b'\n' {
                    self.pos += 1;
                }

                // Skip the newline
                if (self.pos as usize) < data.len() {
                    self.pos += 1;
                }
            }

            if (self.pos as usize) >= data.len() {
                self.pos = self.size;
            }
        }
    }

    /// Base implementation of open functionality
    fn base_open(&mut self, filename: &str, access_flags: u32) -> bool {
        self.name = filename.to_string();
        self.access_flags = access_flags;
        self.is_open = true;
        true
    }
}

impl File for RAMFile {
    fn open(&mut self, filename: &str, access_flags: u32) -> bool {
        // This would need to interface with a file system
        // For now, we'll just set up the basic state
        self.base_open(filename, access_flags)
    }

    fn close(&mut self) {
        if let Some(_) = self.data.take() {
            // Data is dropped here
        }
        self.is_open = false;
        self.pos = 0;
        self.size = 0;
        self.name.clear();
        self.access_flags = 0;
    }

    fn read(&mut self, buffer: &mut [u8]) -> i32 {
        if let Some(ref data) = self.data {
            let bytes_left = self.size - self.pos;
            let bytes_to_read = cmp::min(buffer.len() as i32, bytes_left);

            if bytes_to_read > 0 {
                let start_pos = self.pos as usize;
                let end_pos = start_pos + bytes_to_read as usize;
                buffer[..bytes_to_read as usize].copy_from_slice(&data[start_pos..end_pos]);

                self.pos += bytes_to_read;
            }

            bytes_to_read
        } else {
            -1
        }
    }

    fn write(&mut self, _buffer: &[u8]) -> i32 {
        // RAMFile is read-only
        -1
    }

    fn seek(&mut self, pos: i32, mode: SeekMode) -> i32 {
        let new_pos = match mode {
            SeekMode::Start => pos,
            SeekMode::Current => self.pos + pos,
            SeekMode::End => {
                assert!(pos <= 0, "Position should be <= 0 for seek from end");
                self.size + pos
            }
        };

        self.pos = cmp::max(0, cmp::min(new_pos, self.size));
        self.pos
    }

    fn size(&self) -> i32 {
        self.size
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_access(&self) -> u32 {
        self.access_flags
    }
}

impl Default for RAMFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_file_creation() {
        let ram_file = RAMFile::new();
        assert_eq!(ram_file.size(), 0);
        assert_eq!(ram_file.get_name(), "");
    }

    #[test]
    fn test_seek_operations() {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some(vec![1, 2, 3, 4, 5]);
        ram_file.size = 5;

        // Test seek from start
        assert_eq!(ram_file.seek(2, SeekMode::Start), 2);
        assert_eq!(ram_file.pos, 2);

        // Test seek from current
        assert_eq!(ram_file.seek(1, SeekMode::Current), 3);
        assert_eq!(ram_file.pos, 3);

        // Test seek from end
        assert_eq!(ram_file.seek(-1, SeekMode::End), 4);
        assert_eq!(ram_file.pos, 4);

        // Test boundary conditions
        assert_eq!(ram_file.seek(-10, SeekMode::Start), 0);
        assert_eq!(ram_file.seek(100, SeekMode::Start), 5);
    }

    #[test]
    fn test_read_operations() {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some(vec![1, 2, 3, 4, 5]);
        ram_file.size = 5;
        ram_file.pos = 0;

        let mut buffer = [0u8; 3];
        assert_eq!(ram_file.read(&mut buffer), 3);
        assert_eq!(buffer, [1, 2, 3]);
        assert_eq!(ram_file.pos, 3);

        // Test reading beyond end
        let mut buffer2 = [0u8; 5];
        assert_eq!(ram_file.read(&mut buffer2), 2);
        assert_eq!(buffer2[0..2], [4, 5]);
    }

    #[test]
    fn test_scan_int() {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some("  123  456  -789  ".as_bytes().to_vec());
        ram_file.size = ram_file.data.as_ref().unwrap().len() as i32;
        ram_file.pos = 0;

        assert_eq!(ram_file.scan_int(), Some(123));
        assert_eq!(ram_file.scan_int(), Some(456));
        assert_eq!(ram_file.scan_int(), Some(-789));
        assert_eq!(ram_file.scan_int(), None);
    }

    #[test]
    fn test_scan_real() {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some("  12.5  -3.14  ".as_bytes().to_vec());
        ram_file.size = ram_file.data.as_ref().unwrap().len() as i32;
        ram_file.pos = 0;

        assert_eq!(ram_file.scan_real(), Some(12.5));
        assert_eq!(ram_file.scan_real(), Some(-3.14));
        assert_eq!(ram_file.scan_real(), None);
    }

    #[test]
    fn test_scan_string() {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some("  hello   world  ".as_bytes().to_vec());
        ram_file.size = ram_file.data.as_ref().unwrap().len() as i32;
        ram_file.pos = 0;

        assert_eq!(ram_file.scan_string(), Some("hello".to_string()));
        assert_eq!(ram_file.scan_string(), Some("world".to_string()));
        assert_eq!(ram_file.scan_string(), None);
    }

    #[test]
    fn test_next_line() {
        let mut ram_file = RAMFile::new();
        ram_file.data = Some("line1\nline2\nline3".as_bytes().to_vec());
        ram_file.size = ram_file.data.as_ref().unwrap().len() as i32;
        ram_file.pos = 0;

        let mut buffer = String::new();

        ram_file.next_line(Some(&mut buffer));
        assert_eq!(buffer, "line1\n");

        ram_file.next_line(Some(&mut buffer));
        assert_eq!(buffer, "line2\n");

        ram_file.next_line(Some(&mut buffer));
        assert_eq!(buffer, "line3");
    }
}
