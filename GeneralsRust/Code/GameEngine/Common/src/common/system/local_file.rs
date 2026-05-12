//! Local file system implementation
//!
//! This module provides a faithful, safe port of the C++ `LocalFile` class used by the
//! Generals engine.  The previous async stub mixed Tokio primitives with a synchronous
//! trait, which neither compiled nor matched the behaviour of the original engine.  The
//! new implementation relies on the standard library and keeps explicit track of seek
//! position so users obtain deterministic offsets just like the legacy code.

use std::fs::{File as StdFile, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};

use crate::common::{
    ascii_string::AsciiString,
    system::file::{BaseFile, File, FileAccess, SeekMode},
};

/// Local file implementation.  Mirrors the behaviour of the C++ `LocalFile` by
/// performing synchronous I/O on the host file system.
pub struct LocalFile {
    base: BaseFile,
    handle: Option<StdFile>,
    current_pos: u64,
}

impl LocalFile {
    /// Create a new closed file
    pub fn new() -> Self {
        Self {
            base: BaseFile::new(),
            handle: None,
            current_pos: 0,
        }
    }

    fn access_to_open_options(access: FileAccess) -> OpenOptions {
        let mut options = OpenOptions::new();

        if access.contains(FileAccess::READ) {
            options.read(true);
        }
        if access.contains(FileAccess::WRITE) {
            options.write(true);
        }
        if access.contains(FileAccess::APPEND) {
            options.append(true);
        }
        if access.contains(FileAccess::TRUNCATE) {
            options.truncate(true);
        }
        if access.contains(FileAccess::ONLY_NEW) {
            options.create_new(true);
        } else if access.contains(FileAccess::CREATE) || access.contains(FileAccess::WRITE) {
            options.create(true);
        }

        options
    }

    fn ensure_open(&self) -> io::Result<()> {
        if self.base.is_open() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "File not open"))
        }
    }

    fn read_byte(&mut self) -> io::Result<Option<u8>> {
        let mut byte = [0u8; 1];
        let Some(handle) = self.handle.as_mut() else {
            return Ok(None);
        };
        match handle.read(&mut byte)? {
            0 => Ok(None),
            1 => {
                self.current_pos += 1;
                Ok(Some(byte[0]))
            }
            _ => unreachable!(),
        }
    }

    fn unread_byte(&mut self) -> io::Result<()> {
        let Some(handle) = self.handle.as_mut() else {
            return Ok(());
        };
        handle.seek(SeekFrom::Current(-1))?;
        self.current_pos = self.current_pos.saturating_sub(1);
        Ok(())
    }
}

impl Default for LocalFile {
    fn default() -> Self {
        Self::new()
    }
}

impl File for LocalFile {
    fn open(&mut self, filename: &str, access: FileAccess) -> Result<(), io::Error> {
        // Run shared validation logic; this marks the file as open so any error after this
        // should roll back to a clean state.
        if let Err(err) = self.base.open_base(filename, access) {
            return Err(err);
        }

        let final_access = self.base.get_access();

        match Self::access_to_open_options(final_access).open(filename) {
            Ok(file) => {
                let mut file = file;

                if final_access.contains(FileAccess::APPEND) {
                    self.current_pos = file.seek(SeekFrom::End(0))?;
                } else {
                    self.current_pos = file.seek(SeekFrom::Start(0))?;
                }

                self.handle = Some(file);
                Ok(())
            }
            Err(err) => {
                self.base.close_base();
                Err(err)
            }
        }
    }

    fn close(&mut self) {
        if !self.base.is_open() {
            return;
        }

        let path = self.base.get_name().to_owned();
        let should_delete = self.base.take_delete_on_close();

        self.handle.take();
        self.current_pos = 0;
        self.base.close_base();

        if should_delete && !path.is_empty() && path != "<no file>" {
            let _ = std::fs::remove_file(path);
        }
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, io::Error> {
        self.ensure_open()?;
        if buffer.is_empty() {
            return Ok(0);
        }

        let Some(handle) = self.handle.as_mut() else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "File not open"));
        };

        let bytes = handle.read(buffer)?;
        self.current_pos += bytes as u64;
        Ok(bytes)
    }

    fn write(&mut self, buffer: &[u8]) -> Result<usize, io::Error> {
        self.ensure_open()?;
        let Some(handle) = self.handle.as_mut() else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "File not open"));
        };

        let bytes = handle.write(buffer)?;
        self.current_pos += bytes as u64;
        Ok(bytes)
    }

    fn seek(&mut self, offset: i32, mode: SeekMode) -> Result<i32, io::Error> {
        self.ensure_open()?;
        let Some(handle) = self.handle.as_mut() else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "File not open"));
        };

        let target = match mode {
            SeekMode::Start => SeekFrom::Start(offset.max(0) as u64),
            SeekMode::Current => SeekFrom::Current(offset as i64),
            SeekMode::End => SeekFrom::End(offset as i64),
        };

        let new_pos = handle.seek(target)?;
        self.current_pos = new_pos;
        Ok(new_pos as i32)
    }

    fn next_line(&mut self, buf: Option<&mut Vec<u8>>, buf_size: Option<usize>) {
        if self.ensure_open().is_err() {
            return;
        }

        let mut line: Vec<u8> = Vec::new();
        while let Ok(Some(byte)) = self.read_byte() {
            line.push(byte);
            if byte == b'\n' {
                break;
            }
        }

        if let Some(out) = buf {
            out.clear();
            let limit = buf_size.unwrap_or(line.len());
            out.extend_from_slice(&line[..line.len().min(limit)]);
        }
    }

    fn scan_int(&mut self) -> Result<i32, io::Error> {
        self.ensure_open()?;
        let mut digits = Vec::new();

        while let Some(byte) = self.read_byte()? {
            let ch = byte as char;
            if ch.is_ascii_digit() || ch == '-' {
                digits.push(byte);
                break;
            }
        }

        while let Some(byte) = self.read_byte()? {
            let ch = byte as char;
            if ch.is_ascii_digit() {
                digits.push(byte);
            } else {
                self.unread_byte()?;
                break;
            }
        }

        if digits.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while scanning int",
            ));
        }

        let text = String::from_utf8(digits)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        text.parse::<i32>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    fn scan_real(&mut self) -> Result<f32, io::Error> {
        self.ensure_open()?;
        let mut digits = Vec::new();
        let mut seen_decimal = false;

        while let Some(byte) = self.read_byte()? {
            let ch = byte as char;
            if ch.is_ascii_digit() || (!seen_decimal && ch == '.') || ch == '-' {
                if ch == '.' {
                    seen_decimal = true;
                }
                digits.push(byte);
                break;
            }
        }

        while let Some(byte) = self.read_byte()? {
            let ch = byte as char;
            if ch.is_ascii_digit() {
                digits.push(byte);
            } else if ch == '.' && !seen_decimal {
                seen_decimal = true;
                digits.push(byte);
            } else {
                self.unread_byte()?;
                break;
            }
        }

        if digits.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while scanning real",
            ));
        }

        let text = String::from_utf8(digits)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        text.parse::<f32>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    fn scan_string(&mut self) -> Result<AsciiString, io::Error> {
        self.ensure_open()?;

        // Skip leading whitespace
        while let Some(byte) = self.read_byte()? {
            if !(byte as char).is_whitespace() {
                self.unread_byte()?;
                break;
            }
        }

        let mut bytes = Vec::new();
        while let Some(byte) = self.read_byte()? {
            if (byte as char).is_whitespace() {
                self.unread_byte()?;
                break;
            }
            bytes.push(byte);
        }

        if bytes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while scanning string",
            ));
        }

        let text = String::from_utf8(bytes)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(AsciiString::from(text.as_str()))
    }

    fn print(&mut self, text: &str) -> Result<bool, io::Error> {
        if !self.base.get_access().contains(FileAccess::TEXT) {
            return Ok(false);
        }
        let bytes = self.write(text.as_bytes())?;
        Ok(bytes == text.len())
    }

    fn size(&self) -> i32 {
        self.handle
            .as_ref()
            .and_then(|file| file.metadata().ok().map(|m| m.len() as i64))
            .map(|len| len.min(i32::MAX as i64) as i32)
            .unwrap_or(0)
    }

    fn position(&self) -> i32 {
        self.current_pos.min(i32::MAX as u64) as i32
    }

    fn eof(&self) -> bool {
        if let Some(handle) = self.handle.as_ref() {
            if let Ok(len) = handle.metadata().map(|m| m.len()) {
                return self.current_pos >= len;
            }
        }
        false
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
        self.seek(0, SeekMode::Start)?;
        let size = self.size().max(0) as usize;
        let mut buffer = vec![0u8; size];
        let read = self.read(&mut buffer)?;
        buffer.truncate(read);
        self.close();
        Ok(buffer)
    }
}

impl Drop for LocalFile {
    fn drop(&mut self) {
        self.close();
    }
}

impl LocalFile {
    /// Convert the file contents into memory, matching the RAMFile helper in the C++ code.
    pub fn convert_to_ram_file(mut self) -> Result<Vec<u8>, io::Error> {
        self.read_entire_and_close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_file_round_trip() {
        let path = "temp_local_file_test.txt";
        {
            let mut file = LocalFile::new();
            file.open(
                path,
                FileAccess::CREATE
                    .combine(FileAccess::WRITE)
                    .combine(FileAccess::TEXT),
            )
            .expect("open for write");
            file.print("42").expect("print succeeds");
            file.close();
        }

        {
            let mut file = LocalFile::new();
            file.open(path, FileAccess::READ.combine(FileAccess::TEXT))
                .expect("open for read");
            let value = file.scan_int().expect("scan int");
            assert_eq!(value, 42);
            file.close();
        }

        let _ = std::fs::remove_file(path);
    }
}
