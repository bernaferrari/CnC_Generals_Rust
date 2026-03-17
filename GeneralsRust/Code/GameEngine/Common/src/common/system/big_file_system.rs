////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! BIG File System Implementation
//!
//! This module provides a complete implementation of the BIG file archive format
//! used by Command & Conquer Generals to store game assets. It handles reading
//! BIG files, parsing their directory structures, and extracting individual files.
//!
//! BIG File Format:
//! - Header: "BIGF" magic bytes (4 bytes)
//! - Archive size: Total size of archive in bytes (4 bytes, little endian)
//! - File count: Number of files in archive (4 bytes, big endian)
//! - First file offset: Offset to first file data (4 bytes, big endian)
//! - Directory entries: File offset (4 bytes, big endian), file size (4 bytes, big endian), null-terminated filename
//! - File data: Raw file data at the specified offsets
//!
//! Bryan Cleveland, August 2002
//! Rust conversion: 2025

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use log::{debug, info, warn};
use regex::Regex;

use crate::common::ascii_string::AsciiString;
use crate::common::system::archive_file::{
    ArchiveFileTrait, ArchivedFileInfo, DetailedArchivedDirectoryInfo, FileInfo,
};
use crate::common::system::compression::{
    CompressionEngine, CompressionInterface, CompressionType,
};
use crate::common::system::file::{BaseFile, File as GameFile, FileAccess};
use crate::common::system::file_system::{
    FileInfo as SystemFileInfo, FileSystemBackend, FilenameList,
};

/// Perform case-insensitive wildcard matching supporting `*` and `?` tokens.
fn wildcard_match(candidate: &str, mask: &str) -> bool {
    let candidate_bytes = candidate.as_bytes();
    let mask_bytes = mask.as_bytes();

    let mut cand_idx = 0usize;
    let mut mask_idx = 0usize;
    let mut star_idx: Option<usize> = None;
    let mut match_idx = 0usize;

    while cand_idx < candidate_bytes.len() {
        let cand_ch = candidate_bytes[cand_idx];
        if mask_idx < mask_bytes.len()
            && (mask_bytes[mask_idx] == cand_ch || mask_bytes[mask_idx] == b'?')
        {
            cand_idx += 1;
            mask_idx += 1;
        } else if mask_idx < mask_bytes.len() && mask_bytes[mask_idx] == b'*' {
            star_idx = Some(mask_idx);
            match_idx = cand_idx;
            mask_idx += 1;
        } else if let Some(star) = star_idx {
            mask_idx = star + 1;
            match_idx += 1;
            cand_idx = match_idx;
        } else {
            return false;
        }
    }

    while mask_idx < mask_bytes.len() && mask_bytes[mask_idx] == b'*' {
        mask_idx += 1;
    }

    mask_idx == mask_bytes.len()
}

fn normalize_archive_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// BIG file format magic bytes
pub const BIG_FILE_IDENTIFIER: &[u8; 4] = b"BIGF";

/// BIG file header structure
#[derive(Debug, Clone)]
pub struct BigFileHeader {
    pub magic: [u8; 4],
    pub archive_size: u32,
    pub file_count: u32,
    pub first_file_offset: u32,
}

/// BIG file entry structure
#[derive(Debug, Clone)]
pub struct BigFileEntry {
    pub offset: u32,
    pub size: u32,
    pub filename: String,
    pub is_compressed: bool,
    pub compression_type: CompressionType,
}

/// BIG file implementation
pub struct BigFile {
    name: AsciiString,
    path: AsciiString,
    file: Option<File>,
    header: Option<BigFileHeader>,
    entries: Vec<BigFileEntry>,
    root_directory: DetailedArchivedDirectoryInfo,
    search_priority: i32,
    compression_engine: CompressionEngine,
}

impl Default for BigFile {
    fn default() -> Self {
        Self::new()
    }
}

impl BigFile {
    /// Create a new BIG file instance
    pub fn new() -> Self {
        Self {
            name: AsciiString::new(),
            path: AsciiString::new(),
            file: None,
            header: None,
            entries: Vec::new(),
            root_directory: DetailedArchivedDirectoryInfo::default(),
            search_priority: 0,
            compression_engine: CompressionEngine::new(),
        }
    }

    /// Return an iterator over the raw file entries contained in this BIG archive.
    pub fn entries(&self) -> &[BigFileEntry] {
        &self.entries
    }

    /// Return the archived metadata for the provided filename, if present.
    pub fn archived_file_info(&self, filename: &AsciiString) -> Option<ArchivedFileInfo> {
        let mut path = filename.clone();
        path.to_lower();

        let mut dir_info = &self.root_directory;
        let parts: Vec<AsciiString> = path
            .as_str()
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .map(AsciiString::from)
            .collect();

        if parts.is_empty() {
            return None;
        }

        for (idx, segment) in parts.iter().enumerate() {
            if idx == parts.len() - 1 {
                return dir_info.files.get(segment).cloned();
            }

            if let Some(next) = dir_info.directories.get(segment) {
                dir_info = next;
            } else {
                return None;
            }
        }

        None
    }

    /// Sort internal entry list using case-insensitive ordering for deterministic iteration.
    pub fn sort_entries(&mut self) {
        self.entries
            .sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
        self.build_directory_tree();
    }

    /// Clone the underlying file handle for streaming access.
    pub fn clone_underlying_file(&self) -> io::Result<File> {
        match &self.file {
            Some(file) => file.try_clone(),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "BIG file not open",
            )),
        }
    }

    /// Open and parse a BIG file
    pub fn open<P: AsRef<Path>>(&mut self, filename: P) -> Result<(), io::Error> {
        let path_buf = filename.as_ref().to_path_buf();
        self.path = AsciiString::from(path_buf.to_string_lossy().as_ref());
        self.name = AsciiString::from(
            path_buf
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );

        // Open file for reading
        let mut file = OpenOptions::new().read(true).open(&path_buf)?;

        // Parse BIG file header
        let header = self.parse_header(&mut file)?;
        self.header = Some(header.clone());

        // Parse directory entries
        self.parse_entries(&mut file, &header)?;

        // Build directory tree
        self.build_directory_tree();

        // Store file handle
        self.file = Some(file);

        debug!(
            "BigFile::open - opened BIG file {} with {} files",
            self.name.as_str(),
            self.entries.len()
        );

        Ok(())
    }

    /// Parse BIG file header
    fn parse_header(&mut self, file: &mut File) -> Result<BigFileHeader, io::Error> {
        // Read magic bytes
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;

        // Verify magic
        if magic != *BIG_FILE_IDENTIFIER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid BIG file magic: {:?}", magic),
            ));
        }

        // Read archive size (little endian)
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let archive_size = u32::from_le_bytes(size_bytes);

        // Read file count (big endian)
        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes)?;
        let file_count = u32::from_be_bytes(count_bytes);

        // Read first file offset (big endian)
        let mut offset_bytes = [0u8; 4];
        file.read_exact(&mut offset_bytes)?;
        let first_file_offset = u32::from_be_bytes(offset_bytes);

        debug!(
            "BigFile::parse_header - archive size: {}, file count: {}, first offset: {}",
            archive_size, file_count, first_file_offset
        );

        Ok(BigFileHeader {
            magic,
            archive_size,
            file_count,
            first_file_offset,
        })
    }

    /// Parse file entries from BIG file directory
    fn parse_entries(&mut self, file: &mut File, header: &BigFileHeader) -> Result<(), io::Error> {
        // Seek to directory start (after header)
        file.seek(SeekFrom::Start(0x10))?;

        self.entries.clear();

        for i in 0..header.file_count {
            // Read file offset (big endian)
            let mut offset_bytes = [0u8; 4];
            file.read_exact(&mut offset_bytes)?;
            let file_offset = u32::from_be_bytes(offset_bytes);

            // Read file size (big endian)
            let mut size_bytes = [0u8; 4];
            file.read_exact(&mut size_bytes)?;
            let file_size = u32::from_be_bytes(size_bytes);

            // Read null-terminated filename
            let mut filename_bytes = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                file.read_exact(&mut byte)?;
                if byte[0] == 0 {
                    break;
                }
                filename_bytes.push(byte[0]);
            }

            let filename = String::from_utf8_lossy(&filename_bytes).to_string();

            // Detect compression based on file extension or header inspection
            let is_compressed = self.detect_compression(&filename);
            let compression_type = if is_compressed {
                self.detect_compression_type(&filename)
            } else {
                CompressionType::None
            };

            let entry = BigFileEntry {
                offset: file_offset,
                size: file_size,
                filename: filename.clone(),
                is_compressed,
                compression_type,
            };

            debug!(
                "BigFile::parse_entries - file {}: {} (offset: {}, size: {})",
                i, filename, file_offset, file_size
            );

            self.entries.push(entry);
        }

        Ok(())
    }

    /// Build internal directory tree for fast lookups
    fn build_directory_tree(&mut self) {
        self.root_directory.clear();

        // Collect the directory info first to avoid borrowing conflicts
        let mut directory_entries = Vec::new();

        for entry in &self.entries {
            // Extract filename from full path
            let path_str = entry.filename.replace('/', "\\");
            let path_parts: Vec<&str> = path_str.split('\\').collect();

            if path_parts.is_empty() {
                continue;
            }

            let filename_only = path_parts.last().unwrap();
            let dir_path = if path_parts.len() > 1 {
                path_parts[..path_parts.len() - 1].join("\\")
            } else {
                String::new()
            };

            let archived_file_info = ArchivedFileInfo {
                filename: AsciiString::from(filename_only.to_lowercase().as_str()),
                offset: entry.offset as u64,
                size: entry.size as u64,
                compressed_size: None,
                is_compressed: entry.is_compressed,
            };

            directory_entries.push((AsciiString::from(&dir_path), archived_file_info));
        }

        // Now add all entries to the directory tree
        for (path, file_info) in directory_entries {
            self.add_file_to_directory(&path, &file_info);
        }
    }

    /// Add a file to the directory tree
    fn add_file_to_directory(&mut self, path: &AsciiString, file_info: &ArchivedFileInfo) {
        let mut temp = path.clone();
        temp.to_lower();

        let mut dir_info = &mut self.root_directory;
        let mut debug_path = AsciiString::new();

        // Parse path tokens
        let path_parts: Vec<AsciiString> = temp
            .as_str()
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .map(|s| AsciiString::from(s))
            .collect();

        // Navigate/create directory structure
        for token in &path_parts {
            if !dir_info.directories.contains_key(token) {
                let mut new_dir = DetailedArchivedDirectoryInfo::default();
                new_dir.directory_name = token.clone();
                dir_info.directories.insert(token.clone(), new_dir);
            }

            debug_path.push_str(&token.as_str());
            debug_path.push('\\');

            dir_info = dir_info.directories.get_mut(token).unwrap();
        }

        // Add the file to the final directory
        dir_info
            .files
            .insert(file_info.filename.clone(), file_info.clone());
    }

    /// Detect if a file is compressed.
    ///
    /// Generals BIG entries do not expose a per-entry compression flag in the table of contents.
    /// Older parity code guessed compression from extensions, which caused valid raw assets
    /// (DDS/TGA/WAV/MP3) to be incorrectly decompressed and corrupted at load time.
    ///
    /// Keep archive entries uncompressed by default and let higher-level format parsers consume
    /// the raw payload exactly as stored in the BIG.
    fn detect_compression(&self, _filename: &str) -> bool {
        false
    }

    /// Detect compression type for a file entry.
    fn detect_compression_type(&self, _filename: &str) -> CompressionType {
        CompressionType::None
    }

    /// Extract file data from BIG archive
    pub fn extract_file_data(&mut self, entry: &BigFileEntry) -> Result<Vec<u8>, io::Error> {
        if let Some(ref mut file) = self.file {
            // Seek to file data
            file.seek(SeekFrom::Start(entry.offset as u64))?;

            // Read file data
            let mut data = vec![0u8; entry.size as usize];
            file.read_exact(&mut data)?;

            // Decompress if necessary
            if entry.is_compressed && entry.compression_type != CompressionType::None {
                match self
                    .compression_engine
                    .decompress(&data, entry.compression_type, None)
                {
                    Ok(decompressed) => Ok(decompressed),
                    Err(e) => {
                        warn!("Failed to decompress {}: {}", entry.filename, e);
                        // Return raw data if decompression fails
                        Ok(data)
                    }
                }
            } else {
                Ok(data)
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "BIG file not open",
            ))
        }
    }

    /// Find file entry by filename
    fn find_entry(&self, filename: &str) -> Option<&BigFileEntry> {
        let search_name = filename.replace('\\', "/").to_lowercase();
        self.entries
            .iter()
            .find(|entry| entry.filename.replace('\\', "/").to_lowercase() == search_name)
    }

    /// Get archived file info for a specific file
    pub fn get_archived_file_info(&self, filename: &AsciiString) -> Option<&ArchivedFileInfo> {
        let mut path = filename.clone();
        path.to_lower();

        let mut dir_info = &self.root_directory;

        // Parse path to navigate to the file
        let path_parts: Vec<AsciiString> = path
            .as_str()
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .map(|s| AsciiString::from(s))
            .collect();

        if path_parts.is_empty() {
            return None;
        }

        // Navigate through directories
        for (i, token) in path_parts.iter().enumerate() {
            if i == path_parts.len() - 1 {
                // This is the filename, look it up
                return dir_info.files.get(token);
            }

            // Check if token has a dot (might be a filename)
            if token.contains(".") && !path_parts[i..].iter().any(|t| !t.contains(".")) {
                // This looks like a filename
                return dir_info.files.get(token);
            }

            // Navigate to subdirectory
            if let Some(sub_dir) = dir_info.directories.get(token) {
                dir_info = sub_dir;
            } else {
                return None;
            }
        }

        None
    }
}

/// Read-only file implementation backed by data extracted from a BIG archive.
enum BigArchiveSource {
    Memory(Cursor<Vec<u8>>),
    Stream {
        file: File,
        offset: u64,
        size: u64,
        position: u64,
    },
}

struct BigArchiveFile {
    base: BaseFile,
    source: BigArchiveSource,
}

impl BigArchiveFile {
    fn from_data(name: &str, data: Vec<u8>) -> io::Result<Self> {
        let mut base = BaseFile::new();
        base.open_base(name, FileAccess::READ)?;
        Ok(Self {
            base,
            source: BigArchiveSource::Memory(Cursor::new(data)),
        })
    }

    fn from_stream(name: &str, file: File, offset: u64, size: u64) -> io::Result<Self> {
        let mut base = BaseFile::new();
        base.open_base(name, FileAccess::READ)?;
        Ok(Self {
            base,
            source: BigArchiveSource::Stream {
                file,
                offset,
                size,
                position: 0,
            },
        })
    }

    fn ensure_memory(&mut self) -> io::Result<()> {
        if matches!(self.source, BigArchiveSource::Memory(_)) {
            return Ok(());
        }

        if let BigArchiveSource::Stream {
            mut file,
            offset,
            size,
            position,
        } = std::mem::replace(
            &mut self.source,
            BigArchiveSource::Memory(Cursor::new(Vec::new())),
        ) {
            file.seek(SeekFrom::Start(offset))?;
            let mut data = vec![0u8; size as usize];
            file.read_exact(&mut data)?;
            let mut cursor = Cursor::new(data);
            cursor.set_position(position.min(size) as u64);
            self.source = BigArchiveSource::Memory(cursor);
        }

        Ok(())
    }

    fn with_cursor_mut<F, R>(&mut self, f: F) -> io::Result<R>
    where
        F: FnOnce(&mut Cursor<Vec<u8>>) -> io::Result<R>,
    {
        self.ensure_memory()?;
        if let BigArchiveSource::Memory(cursor) = &mut self.source {
            f(cursor)
        } else {
            unreachable!("BigArchiveFile source must be memory after ensure_memory")
        }
    }
}

impl GameFile for BigArchiveFile {
    fn open(&mut self, _filename: &str, _access: FileAccess) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "BigArchiveFile instances are immutable",
        ))
    }

    fn close(&mut self) {
        match &mut self.source {
            BigArchiveSource::Memory(cursor) => cursor.set_position(0),
            BigArchiveSource::Stream { position, .. } => *position = 0,
        }
        self.base.close_base();
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match &mut self.source {
            BigArchiveSource::Memory(cursor) => cursor.read(buffer),
            BigArchiveSource::Stream {
                file,
                offset,
                size,
                position,
            } => {
                if *position >= *size {
                    return Ok(0);
                }

                let max_len = std::cmp::min(buffer.len() as u64, *size - *position) as usize;
                file.seek(SeekFrom::Start(*offset + *position))?;
                let read = file.read(&mut buffer[..max_len])?;
                *position += read as u64;
                Ok(read)
            }
        }
    }

    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Archive files are read-only",
        ))
    }

    fn seek(&mut self, pos: i32, mode: crate::common::system::file::SeekMode) -> io::Result<i32> {
        match &mut self.source {
            BigArchiveSource::Memory(cursor) => {
                let target = match mode {
                    crate::common::system::file::SeekMode::Start => {
                        SeekFrom::Start(pos.max(0) as u64)
                    }
                    crate::common::system::file::SeekMode::Current => SeekFrom::Current(pos as i64),
                    crate::common::system::file::SeekMode::End => SeekFrom::End(pos as i64),
                };
                let new_pos = cursor.seek(target)?;
                Ok(new_pos as i32)
            }
            BigArchiveSource::Stream { size, position, .. } => {
                let new_pos = match mode {
                    crate::common::system::file::SeekMode::Start => pos.max(0) as u64,
                    crate::common::system::file::SeekMode::Current => {
                        let current = (*position as i64).saturating_add(pos as i64);
                        if current < 0 {
                            0
                        } else {
                            current as u64
                        }
                    }
                    crate::common::system::file::SeekMode::End => {
                        let end = (*size as i64).saturating_add(pos as i64);
                        if end < 0 {
                            0
                        } else {
                            end as u64
                        }
                    }
                };
                *position = new_pos.min(*size);
                Ok(*position as i32)
            }
        }
    }

    fn next_line(&mut self, buf: Option<&mut Vec<u8>>, buf_size: Option<usize>) {
        if self.ensure_memory().is_err() {
            if let Some(buffer) = buf {
                buffer.clear();
            }
            return;
        }

        if self
            .with_cursor_mut(|cursor| {
                let data = cursor.get_ref();
                let position = cursor.position() as usize;

                if position >= data.len() {
                    if let Some(buffer) = buf {
                        buffer.clear();
                    }
                    return Ok(());
                }

                let mut end = position;
                while end < data.len() && data[end] != b'\n' {
                    end += 1;
                }

                if let Some(buffer) = buf {
                    buffer.clear();
                    let mut slice = &data[position..end];
                    if let Some(last) = slice.last() {
                        if *last == b'\r' {
                            slice = &slice[..slice.len() - 1];
                        }
                    }
                    let max_len = buf_size.unwrap_or(slice.len());
                    buffer.extend_from_slice(&slice[..slice.len().min(max_len)]);
                }

                let new_pos = if end < data.len() { end + 1 } else { end };
                cursor.set_position(new_pos as u64);
                Ok(())
            })
            .is_err()
        {}
    }

    fn scan_int(&mut self) -> io::Result<i32> {
        self.with_cursor_mut(|cursor| {
            let (value, new_pos) = {
                let data = cursor.get_ref();
                let mut idx = cursor.position() as usize;
                while idx < data.len() {
                    let ch = data[idx] as char;
                    if ch.is_ascii_digit() || ch == '-' {
                        break;
                    }
                    idx += 1;
                }

                if idx >= data.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "EOF while scanning int",
                    ));
                }

                let start = idx;
                idx += 1;
                while idx < data.len() && (data[idx] as char).is_ascii_digit() {
                    idx += 1;
                }

                let slice = &data[start..idx];
                let number = std::str::from_utf8(slice)
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Invalid UTF-8 while parsing int",
                        )
                    })?
                    .parse::<i32>()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid integer format")
                    })?;

                (number, idx as u64)
            };

            cursor.set_position(new_pos);
            Ok(value)
        })
    }

    fn scan_real(&mut self) -> io::Result<f32> {
        self.with_cursor_mut(|cursor| {
            let (value, new_pos) = {
                let data = cursor.get_ref();
                let mut idx = cursor.position() as usize;
                while idx < data.len() {
                    let ch = data[idx] as char;
                    if ch.is_ascii_digit() || ch == '-' || ch == '.' {
                        break;
                    }
                    idx += 1;
                }

                if idx >= data.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "EOF while scanning real",
                    ));
                }

                let start = idx;
                idx += 1;
                while idx < data.len() {
                    let ch = data[idx] as char;
                    if !ch.is_ascii_digit() && ch != '.' {
                        break;
                    }
                    idx += 1;
                }

                let slice = &data[start..idx];
                let number = std::str::from_utf8(slice)
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Invalid UTF-8 while parsing real",
                        )
                    })?
                    .parse::<f32>()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid real format")
                    })?;

                (number, idx as u64)
            };

            cursor.set_position(new_pos);
            Ok(value)
        })
    }

    fn scan_string(&mut self) -> io::Result<AsciiString> {
        self.with_cursor_mut(|cursor| {
            let (value, new_pos) = {
                let data = cursor.get_ref();
                let mut idx = cursor.position() as usize;

                while idx < data.len() && data[idx].is_ascii_whitespace() {
                    idx += 1;
                }

                if idx >= data.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "EOF while scanning string",
                    ));
                }

                let start = idx;
                idx += 1;
                while idx < data.len() && !data[idx].is_ascii_whitespace() {
                    idx += 1;
                }

                let slice = &data[start..idx];
                let string = AsciiString::from(std::str::from_utf8(slice).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in string")
                })?);

                (string, idx as u64)
            };

            cursor.set_position(new_pos);
            Ok(value)
        })
    }

    fn print(&mut self, _text: &str) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Archive files are read-only",
        ))
    }

    fn size(&self) -> i32 {
        match &self.source {
            BigArchiveSource::Memory(cursor) => cursor.get_ref().len() as i32,
            BigArchiveSource::Stream { size, .. } => *size as i32,
        }
    }

    fn position(&self) -> i32 {
        match &self.source {
            BigArchiveSource::Memory(cursor) => cursor.position() as i32,
            BigArchiveSource::Stream { position, .. } => *position as i32,
        }
    }

    fn eof(&self) -> bool {
        match &self.source {
            BigArchiveSource::Memory(cursor) => {
                cursor.position() as usize >= cursor.get_ref().len()
            }
            BigArchiveSource::Stream { position, size, .. } => position >= size,
        }
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
        let data = self.with_cursor_mut(|cursor| {
            cursor.set_position(0);
            let mut buffer = Vec::new();
            cursor.read_to_end(&mut buffer)?;
            Ok(buffer)
        })?;
        self.close();
        Ok(data)
    }
}

impl Read for BigArchiveFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        GameFile::read(self, buf)
    }
}

impl ArchiveFileTrait for BigFile {
    fn get_file_info(
        &self,
        filename: &AsciiString,
        file_info: &mut FileInfo,
    ) -> Result<bool, io::Error> {
        if let Some(archived_info) = self.get_archived_file_info(filename) {
            file_info.size = archived_info.size;
            file_info.modified_time = 0; // BIG files don't store modification time
            file_info.is_directory = false;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn open_file(
        &mut self,
        filename: &str,
        _access: i32,
    ) -> Result<Box<dyn Read + Send>, io::Error> {
        let entry = self
            .find_entry(filename)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File entry not found"))?
            .clone();

        if entry.is_compressed && entry.compression_type != CompressionType::None {
            let data = self.extract_file_data(&entry)?;
            let archive_file = BigArchiveFile::from_data(filename, data)?;
            Ok(Box::new(archive_file))
        } else {
            let mut file_handle = self.clone_underlying_file()?;
            file_handle.seek(SeekFrom::Start(entry.offset as u64))?;
            let archive_file = BigArchiveFile::from_stream(
                filename,
                file_handle,
                entry.offset as u64,
                entry.size as u64,
            )?;
            Ok(Box::new(archive_file))
        }
    }

    fn close_all_files(&mut self) {
        // In this implementation, files are read into memory, so no cleanup needed
    }

    fn get_name(&self) -> AsciiString {
        self.name.clone()
    }

    fn get_path(&self) -> AsciiString {
        self.path.clone()
    }

    fn set_search_priority(&mut self, new_priority: i32) {
        self.search_priority = new_priority;
    }

    fn close(&mut self) {
        if let Some(_) = self.file.take() {
            // File will be automatically closed when dropped
        }
        self.entries.clear();
        self.root_directory.clear();
        self.header = None;
    }
}

impl Drop for BigFile {
    fn drop(&mut self) {
        self.close();
    }
}

/// Entry metadata maintained for each loaded BIG archive.
#[allow(dead_code)]
struct ArchiveEntry {
    name: AsciiString,
    path: PathBuf,
    big_file: BigFile,
    override_existing: bool,
    load_sequence: u64,
}

#[derive(Clone)]
struct FileLocator {
    archive_index: usize,
    entry_index: usize,
    normalized_path: AsciiString,
    original_path: AsciiString,
}

#[allow(dead_code)]
struct PreparedEntry {
    entry: BigFileEntry,
    normalized_path: AsciiString,
    original_path: AsciiString,
    data: Option<Vec<u8>>,
    stream: Option<File>,
}

/// BIG File System for managing multiple BIG files
pub struct BigFileSystem {
    archives: Vec<ArchiveEntry>,
    archive_lookup: HashMap<String, usize>,
    file_index: HashMap<String, FileLocator>,
    load_counter: u64,
}

impl Default for BigFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl BigFileSystem {
    /// Create a new BIG file system
    pub fn new() -> Self {
        Self {
            archives: Vec::new(),
            archive_lookup: HashMap::new(),
            file_index: HashMap::new(),
            load_counter: 0,
        }
    }

    /// Initialize BIG file system state.
    ///
    /// C++ parity note:
    /// Archive discovery is driven by `ArchiveFileSystem`/Win32BIGFileSystem init.
    /// Do not scan implicit directories here because that can alter mount order
    /// and shadow files differently than the original engine.
    pub fn init(&mut self) -> io::Result<()> {
        Ok(())
    }

    /// Load BIG files from a directory, optionally overwriting existing mappings.
    pub fn load_big_files_from_directory(
        &mut self,
        dir: &str,
        file_mask: &str,
        overwrite: bool,
    ) -> io::Result<bool> {
        use std::collections::VecDeque;
        use std::fs;

        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            return Ok(false);
        }

        let masks: Vec<String> = file_mask
            .split(';')
            .map(|mask| mask.trim().replace('\\', "/").to_lowercase())
            .filter(|mask| !mask.is_empty())
            .collect();

        let matches_mask = |entry_path: &Path| {
            if masks.is_empty() {
                return true;
            }

            let file_name = entry_path
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase());

            let relative = entry_path
                .strip_prefix(dir_path)
                .unwrap_or(entry_path)
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase();

            masks.iter().any(|mask| {
                if mask.contains('/') {
                    wildcard_match(&relative, mask)
                } else if let Some(ref file_name) = file_name {
                    wildcard_match(file_name, mask)
                } else {
                    false
                }
            })
        };

        // C++ parity: Win32BIGFileSystem accumulates recursive matches in a
        // case-insensitive set keyed by full path before mounting in-order.
        let mut candidates: BTreeMap<String, PathBuf> = BTreeMap::new();
        let mut pending_dirs = VecDeque::from([dir_path.to_path_buf()]);
        while let Some(current_dir) = pending_dirs.pop_front() {
            let entries = match fs::read_dir(&current_dir) {
                Ok(entries) => entries,
                Err(err) => {
                    warn!(
                        "Failed to enumerate BIG directory {}: {}",
                        current_dir.display(),
                        err
                    );
                    continue;
                }
            };

            for entry in entries {
                let entry_path = match entry {
                    Ok(entry) => entry.path(),
                    Err(_) => continue,
                };
                if entry_path.is_dir() {
                    pending_dirs.push_back(entry_path);
                    continue;
                }
                if entry_path.is_file() && matches_mask(&entry_path) {
                    let key = normalize_archive_path_key(&entry_path);
                    candidates.entry(key).or_insert(entry_path);
                }
            }
        }

        let mut loaded_any = false;

        for (_, candidate) in candidates {
            match self.register_archive(candidate.as_path(), overwrite) {
                Ok(true) => loaded_any = true,
                Ok(false) => {}
                Err(err) => {
                    warn!(
                        "Failed to load BIG archive {}: {}",
                        candidate.display(),
                        err
                    );
                }
            }
        }

        Ok(loaded_any)
    }

    fn register_archive(&mut self, path: &Path, overwrite: bool) -> io::Result<bool> {
        let archive_key = normalize_archive_path_key(path);
        let name = AsciiString::from(path.to_string_lossy().as_ref());

        if !overwrite && self.archive_lookup.contains_key(&archive_key) {
            return Ok(false);
        }

        let mut big_file = BigFile::new();
        big_file.open(path)?;

        self.load_counter = self.load_counter.wrapping_add(1);

        let entry = ArchiveEntry {
            name: name.clone(),
            path: path.to_path_buf(),
            big_file,
            override_existing: overwrite,
            load_sequence: self.load_counter,
        };

        let was_existing = if let Some(index) = self.archive_lookup.get(&archive_key).copied() {
            self.archives[index] = entry;
            true
        } else {
            self.archive_lookup.insert(archive_key, self.archives.len());
            self.archives.push(entry);
            false
        };

        self.rebuild_file_index();
        info!("BigFileSystem loaded archive {}", path.display());
        Ok(!was_existing || overwrite)
    }

    fn rebuild_file_index(&mut self) {
        self.file_index.clear();

        let mut ordered: Vec<(usize, u64)> = self
            .archives
            .iter()
            .enumerate()
            .map(|(idx, archive)| (idx, archive.load_sequence))
            .collect();
        ordered.sort_by_key(|(_, seq)| *seq);

        for (archive_index, _) in ordered {
            let archive = &self.archives[archive_index];
            let override_existing = archive.override_existing;

            for (entry_index, entry) in archive.big_file.entries().iter().enumerate() {
                let normalized = entry.filename.replace('\\', "/");
                let canonical = normalized.to_lowercase();
                let locator = FileLocator {
                    archive_index,
                    entry_index,
                    normalized_path: AsciiString::from(normalized.as_str()),
                    original_path: AsciiString::from(entry.filename.as_str()),
                };

                if override_existing {
                    self.file_index.insert(canonical, locator);
                } else {
                    self.file_index.entry(canonical).or_insert(locator);
                }
            }
        }
    }

    fn resolve_locator(&self, filename: &str) -> Option<FileLocator> {
        let canonical = filename.replace('\\', "/").to_lowercase();
        self.file_index.get(&canonical).cloned()
    }

    fn prepare_entry(
        &mut self,
        filename: &str,
        streaming: bool,
    ) -> io::Result<Option<PreparedEntry>> {
        let locator = match self.resolve_locator(filename) {
            Some(locator) => locator,
            None => return Ok(None),
        };

        let archive = self
            .archives
            .get_mut(locator.archive_index)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Archive index out of range"))?;

        let entry = archive
            .big_file
            .entries()
            .get(locator.entry_index)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Entry index out of range"))?;

        if entry.is_compressed && entry.compression_type != CompressionType::None {
            let data = archive.big_file.extract_file_data(&entry)?;
            Ok(Some(PreparedEntry {
                entry,
                normalized_path: locator.normalized_path,
                original_path: locator.original_path,
                data: Some(data),
                stream: None,
            }))
        } else if streaming {
            let file = archive.big_file.clone_underlying_file()?;
            Ok(Some(PreparedEntry {
                entry,
                normalized_path: locator.normalized_path,
                original_path: locator.original_path,
                data: None,
                stream: Some(file),
            }))
        } else {
            let data = archive.big_file.extract_file_data(&entry)?;
            Ok(Some(PreparedEntry {
                entry,
                normalized_path: locator.normalized_path,
                original_path: locator.original_path,
                data: Some(data),
                stream: None,
            }))
        }
    }

    /// Open a specific archive file
    pub fn open_archive_file<P: AsRef<Path>>(&mut self, filename: P) -> Result<(), io::Error> {
        self.register_archive(filename.as_ref(), true).map(|_| ())
    }

    /// Close a specific archive file
    pub fn close_archive_file(&mut self, filename: &str) {
        let key = filename.replace('\\', "/").to_lowercase();
        if let Some(index) = self.archive_lookup.remove(&key) {
            self.archives.remove(index);

            // Rebuild lookup since indices shifted.
            self.archive_lookup.clear();
            for (idx, archive) in self.archives.iter().enumerate() {
                self.archive_lookup
                    .insert(normalize_archive_path_key(&archive.path), idx);
            }
            self.rebuild_file_index();
        }
    }

    /// Close all archive files
    pub fn close_all_archive_files(&mut self) {
        self.archives.clear();
        self.archive_lookup.clear();
        self.file_index.clear();
    }

    /// Get a mutable reference to a BIG file by archive name
    pub fn get_big_file(&mut self, filename: &str) -> Option<&mut BigFile> {
        let key = filename.replace('\\', "/").to_lowercase();
        self.archive_lookup
            .get(&key)
            .and_then(|&idx| self.archives.get_mut(idx))
            .map(|entry| &mut entry.big_file)
    }

    /// Retrieve archived file metadata for the given filename.
    pub fn get_archived_file_info(&self, filename: &AsciiString) -> Option<ArchivedFileInfo> {
        let canonical = filename.as_str().replace('\\', "/").to_lowercase();
        let locator = self.file_index.get(&canonical)?;
        let archive = self.archives.get(locator.archive_index)?;
        archive
            .big_file
            .get_archived_file_info(&locator.original_path)
            .cloned()
    }

    /// Resolve the archive filename that currently owns the provided virtual path.
    pub fn resolve_archive_filename(&self, filename: &AsciiString) -> Option<AsciiString> {
        let canonical = filename.as_str().replace('\\', "/").to_lowercase();
        let locator = self.file_index.get(&canonical)?;
        let archive = self.archives.get(locator.archive_index)?;
        Some(archive.name.clone())
    }

    /// Collect files matching the directory/pattern combination.
    pub fn collect_matching_files(
        &self,
        directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        recursive: bool,
    ) {
        let dir_norm = directory
            .as_str()
            .replace('\\', "/")
            .trim_matches('/')
            .to_lowercase();
        let mut dir_prefix = dir_norm.clone();
        if !dir_prefix.is_empty() {
            dir_prefix.push('/');
        }

        let pattern_source = if search_name.is_empty() {
            "*".to_string()
        } else {
            search_name.as_str().to_lowercase()
        };
        let regex_pattern = pattern_source
            .replace('.', r"\.")
            .replace('*', ".*")
            .replace('?', ".");
        let pattern = Regex::new(&format!("^{}$", regex_pattern)).ok();

        let mut canonical_paths: Vec<(&String, &FileLocator)> = self.file_index.iter().collect();
        canonical_paths.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (canonical_path, locator) in canonical_paths {
            let path_lower = canonical_path.as_str();

            if !dir_prefix.is_empty() {
                if !path_lower.starts_with(&dir_prefix) {
                    continue;
                }
                if !recursive {
                    let remainder = &path_lower[dir_prefix.len()..];
                    if remainder.contains('/') {
                        continue;
                    }
                }
            } else if !recursive && path_lower.contains('/') {
                continue;
            }

            let file_name_lower = path_lower
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(path_lower);

            if pattern
                .as_ref()
                .map(|regex| regex.is_match(file_name_lower))
                .unwrap_or(true)
            {
                filename_list.insert(locator.normalized_path.clone());
            }
        }
    }

    /// Check if file exists in any BIG file
    pub fn does_file_exist(&self, filename: &str) -> bool {
        self.resolve_locator(filename).is_some()
    }

    /// Open file from any BIG file
    pub fn open_file(
        &mut self,
        filename: &str,
        access: i32,
    ) -> Result<Box<dyn Read + Send>, io::Error> {
        const FILE_READ: i32 = 0x00000001;
        const FILE_WRITE: i32 = 0x00000002;
        const FILE_STREAMING: i32 = 0x00000100;

        if (access & FILE_WRITE) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "BIG archive entries are read-only",
            ));
        }
        if (access & FILE_READ) == 0 && access != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "BIG archive access requires read permission",
            ));
        }

        let streaming = (access & FILE_STREAMING) != 0;

        let prepared = match self.prepare_entry(filename, streaming)? {
            Some(entry) => entry,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "File not found in any BIG file",
                ))
            }
        };

        if let Some(data) = prepared.data {
            Ok(Box::new(Cursor::new(data)))
        } else if let Some(mut file) = prepared.stream {
            file.seek(SeekFrom::Start(prepared.entry.offset as u64))?;
            Ok(Box::new(file.take(prepared.entry.size as u64)))
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to prepare BIG file entry",
            ))
        }
    }

    /// Get list of all loaded BIG files
    pub fn get_loaded_big_files(&self) -> Vec<AsciiString> {
        let mut names: Vec<AsciiString> = self
            .archives
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        names.sort_by(|a, b| a.as_str().to_lowercase().cmp(&b.as_str().to_lowercase()));
        names
    }

    /// Total number of unique virtual files currently indexed across all archives.
    pub fn total_virtual_files(&self) -> usize {
        self.file_index.len()
    }

    /// Total number of physical files across every loaded archive (duplicates counted per archive).
    pub fn total_physical_files(&self) -> usize {
        self.archives
            .iter()
            .map(|entry| entry.big_file.entries().len())
            .sum()
    }

    /// Snapshot the canonical virtual paths tracked by the archive system.
    pub fn virtual_paths(&self) -> Vec<String> {
        let paths: Vec<String> = self.file_index.keys().cloned().collect();
        paths
    }
}

/// File-system backend that surfaces BIG archives through the general `FileSystem` API.
pub struct BigArchiveBackend {
    big_system: BigFileSystem,
    search_paths: Vec<PathBuf>,
    initialized: bool,
}

impl BigArchiveBackend {
    pub fn new() -> Self {
        Self {
            big_system: BigFileSystem::new(),
            search_paths: Vec::new(),
            initialized: false,
        }
    }

    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        let incoming = path.as_ref();
        if !self.search_paths.iter().any(|existing| existing == incoming) {
            self.search_paths.push(incoming.to_path_buf());
        }
    }

    fn load_archives_from_path(&mut self, path: &Path) -> io::Result<()> {
        let Some(path_str) = path.to_str() else {
            return Ok(());
        };

        match self
            .big_system
            .load_big_files_from_directory(path_str, "*.big", false)
        {
            Ok(true) => {
                info!("Discovered BIG archives in {}", path_str);
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string())),
        }
    }

    fn reload_archives(&mut self) -> io::Result<()> {
        self.big_system.close_all_archive_files();
        let paths: Vec<PathBuf> = self.search_paths.clone();
        for path in paths {
            if let Err(err) = self.load_archives_from_path(&path) {
                warn!("Failed to load BIG archives from {:?}: {}", path, err);
            }
        }
        Ok(())
    }

    pub fn virtual_paths(&self) -> Vec<String> {
        self.big_system.virtual_paths()
    }
}

impl FileSystemBackend for BigArchiveBackend {
    fn identifier(&self) -> &'static str {
        "big"
    }

    fn init(&mut self) {
        if !self.initialized {
            if let Err(err) = self.reload_archives() {
                warn!("Failed to initialize BIG archive backend: {}", err);
            }
            self.initialized = true;
        }
    }

    fn reset(&mut self) {
        self.big_system.close_all_archive_files();
        self.initialized = false;
    }

    fn update(&mut self) {}

    fn open_file(&mut self, filename: &str, access: FileAccess) -> Option<Box<dyn GameFile>> {
        if !access.contains(FileAccess::READ) || access.contains(FileAccess::WRITE) {
            return None;
        }

        let streaming = access.contains(FileAccess::STREAMING);

        let prepared = match self.big_system.prepare_entry(filename, streaming) {
            Ok(Some(entry)) => entry,
            Ok(None) => return None,
            Err(err) => {
                warn!("Failed to resolve archive entry {}: {}", filename, err);
                return None;
            }
        };

        if let Some(data) = prepared.data {
            match BigArchiveFile::from_data(filename, data) {
                Ok(file) => Some(Box::new(file)),
                Err(err) => {
                    warn!("Failed to materialize archive file {}: {}", filename, err);
                    None
                }
            }
        } else if let Some(mut file) = prepared.stream {
            if let Err(err) = file.seek(SeekFrom::Start(prepared.entry.offset as u64)) {
                warn!(
                    "Failed to seek archive entry {} at offset {}: {}",
                    filename, prepared.entry.offset, err
                );
                return None;
            }
            match BigArchiveFile::from_stream(
                filename,
                file,
                prepared.entry.offset as u64,
                prepared.entry.size as u64,
            ) {
                Ok(file) => Some(Box::new(file)),
                Err(err) => {
                    warn!("Failed to open archive stream {}: {}", filename, err);
                    None
                }
            }
        } else {
            None
        }
    }

    fn does_file_exist(&self, filename: &str) -> bool {
        self.big_system.does_file_exist(filename)
    }

    fn get_file_list_in_directory(
        &self,
        base_path: &AsciiString,
        directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        let mut combined = base_path.clone();
        if !directory.is_empty() {
            if !combined.is_empty() {
                combined.push('/');
            }
            combined.push_str(directory.as_str());
        }

        self.big_system.collect_matching_files(
            &combined,
            search_name,
            filename_list,
            search_subdirectories,
        );
    }

    fn get_file_info(&self, filename: &AsciiString) -> Option<SystemFileInfo> {
        self.big_system
            .get_archived_file_info(filename)
            .map(|info| SystemFileInfo {
                size_high: ((info.size >> 32) & 0xFFFFFFFF) as i32,
                size_low: (info.size & 0xFFFFFFFF) as i32,
                timestamp_high: 0,
                timestamp_low: 0,
            })
    }

    fn create_directory(&mut self, _directory: AsciiString) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn create_test_big_file(path: &str) -> Result<(), io::Error> {
        let mut file = fs::File::create(path)?;

        // Write BIG file header
        file.write_all(BIG_FILE_IDENTIFIER)?; // Magic
        file.write_all(&100u32.to_le_bytes())?; // Archive size
        file.write_all(&1u32.to_be_bytes())?; // File count
        file.write_all(&50u32.to_be_bytes())?; // First file offset

        // Write file entry
        file.write_all(&50u32.to_be_bytes())?; // File offset
        file.write_all(&11u32.to_be_bytes())?; // File size
        file.write_all(b"test.txt\0")?; // Filename

        // Pad to offset 50
        let current_pos = file.seek(SeekFrom::Current(0))? as usize;
        let padding = 50 - current_pos;
        file.write_all(&vec![0u8; padding])?;

        // Write file content
        file.write_all(b"Hello World")?;

        Ok(())
    }

    #[test]
    fn test_big_file_header_parsing() {
        let test_file = "test_header.big";
        create_test_big_file(test_file).unwrap();

        let mut big_file = BigFile::new();
        assert!(big_file.open(test_file).is_ok());

        assert!(big_file.header.is_some());
        let header = big_file.header.as_ref().unwrap();
        assert_eq!(header.magic, *BIG_FILE_IDENTIFIER);
        assert_eq!(header.file_count, 1);

        // Cleanup
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_big_file_extraction() {
        let test_file = "test_extract.big";
        create_test_big_file(test_file).unwrap();

        let mut big_file = BigFile::new();
        big_file.open(test_file).unwrap();

        let mut reader = big_file.open_file("test.txt", 0).unwrap();
        let mut content = String::new();
        reader.read_to_string(&mut content).unwrap();

        assert_eq!(content, "Hello World");

        // Cleanup
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_big_file_system() {
        let test_file = "test_system.big";
        create_test_big_file(test_file).unwrap();

        let mut big_system = BigFileSystem::new();
        big_system.open_archive_file(test_file).unwrap();

        assert!(big_system.does_file_exist("test.txt"));
        assert!(!big_system.does_file_exist("nonexistent.txt"));

        let loaded_files = big_system.get_loaded_big_files();
        assert_eq!(loaded_files.len(), 1);

        // Cleanup
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn wildcard_matching_covers_common_patterns() {
        assert!(super::wildcard_match("music.big", "music.big"));
        assert!(super::wildcard_match("music.big", "*.big"));
        assert!(super::wildcard_match("music.big", "m?sic.*"));
        assert!(super::wildcard_match("data/audio/mix.big", "data/*/*.big"));
        assert!(!super::wildcard_match("music.big", "*.mix"));
        assert!(!super::wildcard_match("voice.big", "music.*"));
    }

    #[test]
    fn directory_masks_respect_exact_matches() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let music_path = dir.path().join("Music.big");
        let support_path = dir.path().join("Support.big");
        create_test_big_file(music_path.to_str().unwrap()).unwrap();
        create_test_big_file(support_path.to_str().unwrap()).unwrap();

        let mut system = BigFileSystem::new();
        system
            .load_big_files_from_directory(dir.path().to_str().unwrap(), "Music.big", false)
            .unwrap();

        assert!(system.does_file_exist("test.txt"));
        assert_eq!(system.get_loaded_big_files().len(), 1);

        let mut system_case = BigFileSystem::new();
        system_case
            .load_big_files_from_directory(dir.path().to_str().unwrap(), "support.BIG", false)
            .unwrap();
        assert_eq!(system_case.get_loaded_big_files().len(), 1);
    }
}
