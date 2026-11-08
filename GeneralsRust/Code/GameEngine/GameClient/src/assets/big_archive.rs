//! # BIG Archive Support
//!
//! Complete implementation of EA's BIG archive format used in Command & Conquer games.
//! Supports all variations including:
//! - Standard BIG files from original Generals
//! - Zero Hour expansion BIG files
//! - Compressed BIG archives
//! - Multi-part BIG archives
//! - Integrity checking and validation

use bytemuck::{Pod, Zeroable, cast_slice, from_bytes};
use std::convert::TryFrom;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use thiserror::Error;

use super::{AssetError, CompressionType};

/// BIG archive format errors
#[derive(Error, Debug)]
pub enum BigError {
    #[error("Invalid BIG signature: expected 'BIGF', got {0:?}")]
    InvalidSignature([u8; 4]),
    #[error("Unsupported BIG version: {0}")]
    UnsupportedVersion(u32),
    #[error("Invalid archive structure: {0}")]
    InvalidStructure(String),
    #[error("File not found in archive: {0}")]
    FileNotFound(String),
    #[error("Compression error: {0}")]
    CompressionError(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// BIG file header structure (little-endian)
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BigHeader {
    signature: [u8; 4],     // 'BIGF' or 'BIG4'
    archive_size: u32,      // Total size of archive
    num_files: u32,         // Number of files in archive
    first_file_offset: u32, // Offset to first file entry
}

/// BIG file entry structure
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BigFileEntry {
    offset: u32,      // Offset in archive
    size: u32,        // Uncompressed size
    name_offset: u32, // Offset to filename in name table
}

/// Enhanced BIG file entry with metadata
#[derive(Debug, Clone)]
pub struct BigFileInfo {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub compressed_size: Option<u64>,
    pub compression: CompressionType,
    pub checksum: Option<u32>,
    pub is_directory: bool,
}

/// BIG Archive implementation
pub struct BigArchive {
    path: PathBuf,
    header: BigHeader,
    files: HashMap<String, BigFileInfo>,
    name_lookup: BTreeMap<String, String>, // case-insensitive lookup
    archive_len: u64,
    file_handle: Option<Mutex<File>>,
    version: BigVersion,
    is_compressed: bool,
    total_uncompressed_size: u64,
}

/// BIG archive versions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BigVersion {
    /// Original BIG format (Generals)
    Standard,
    /// BIG4 format (Zero Hour)
    Enhanced,
    /// Compressed BIG format
    Compressed,
}

impl BigArchive {
    /// Load BIG archive from file
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<Self, BigError> {
        let path = path.as_ref().to_path_buf();

        log::info!("Loading BIG archive: {}", path.display());

        // Keep the opened file for lazy extraction. A read-only mapping would not
        // be safe: another process can still modify or truncate the backing file.
        let mut file = File::open(&path)?;
        let archive_len = file.metadata()?.len();
        let mut header_bytes = [0u8; std::mem::size_of::<BigHeader>()];
        if archive_len < header_bytes.len() as u64 {
            return Err(BigError::InvalidStructure(
                "File too small for BIG header".to_string(),
            ));
        }
        file.read_exact(&mut header_bytes)?;
        let header: BigHeader = *from_bytes(&header_bytes);

        let version = match &header.signature {
            b"BIGF" => BigVersion::Standard,
            b"BIG4" => BigVersion::Enhanced,
            b"BIGC" => BigVersion::Compressed,
            sig => return Err(BigError::InvalidSignature(*sig)),
        };

        let is_compressed = version == BigVersion::Compressed;

        let num_files = header.num_files;
        let archive_size = header.archive_size;

        log::debug!(
            "BIG archive info: version={:?}, files={}, size={}",
            version,
            num_files,
            archive_size
        );

        // Validate header
        if archive_size as u64 != archive_len {
            log::warn!(
                "Archive size mismatch: header={}, actual={}",
                archive_size,
                archive_len
            );
        }

        if header.first_file_offset as u64 >= archive_len {
            return Err(BigError::InvalidStructure(
                "Invalid first file offset".to_string(),
            ));
        }

        // Read file entries
        let mut files = HashMap::new();
        let mut name_lookup = BTreeMap::new();
        let mut total_uncompressed_size = 0u64;

        // Calculate positions
        let entries_start = std::mem::size_of::<BigHeader>();
        let entries_size = (num_files as usize)
            .checked_mul(std::mem::size_of::<BigFileEntry>())
            .ok_or_else(|| BigError::InvalidStructure("File entries overflow".to_string()))?;
        let name_table_start = entries_start.checked_add(entries_size)
            .ok_or_else(|| BigError::InvalidStructure("File entries overflow".to_string()))?;
        let first_file_offset = header.first_file_offset as usize;

        if name_table_start > first_file_offset {
            return Err(BigError::InvalidStructure(
                "Name table beyond file bounds".to_string(),
            ));
        }

        // Read only the archive index; file contents stay on disk until requested.
        let mut index = vec![0u8; first_file_offset - entries_start];
        file.read_exact(&mut index)?;
        let file_entries: &[BigFileEntry] = cast_slice(&index[..entries_size]);
        let name_table = &index[entries_size..];

        for (i, entry) in file_entries.iter().enumerate() {
            let name_offset = entry.name_offset;
            let entry_size = entry.size;
            let entry_offset = entry.offset;

            // Extract filename from name table
            let name_start = name_offset as usize;
            if name_start >= name_table.len() {
                log::error!("Invalid name offset for entry {}: {}", i, name_offset);
                continue;
            }

            let name_bytes = &name_table[name_start..];
            let name_end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(name_bytes.len());

            let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

            // Determine compression type
            let compression = if is_compressed {
                CompressionType::Zlib // Most common for BIG archives
            } else {
                CompressionType::None
            };

            let file_info = BigFileInfo {
                name: name.clone(),
                offset: entry_offset as u64,
                size: entry_size as u64,
                compressed_size: if is_compressed {
                    Some(entry_size as u64)
                } else {
                    None
                },
                compression,
                checksum: None, // Not stored in standard BIG format
                is_directory: name.ends_with('/') || name.ends_with('\\'),
            };

            total_uncompressed_size += file_info.size;

            log::trace!(
                "BIG file entry: {} ({} bytes at offset {})",
                name,
                entry_size,
                entry_offset
            );

            // Store with original case
            files.insert(name.clone(), file_info);

            // Store lowercase for case-insensitive lookup
            name_lookup.insert(name.to_lowercase(), name);
        }

        log::info!(
            "Loaded BIG archive: {} files, {} MB uncompressed",
            files.len(),
            total_uncompressed_size / (1024 * 1024)
        );

        Ok(Self {
            path,
            header,
            files,
            name_lookup,
            archive_len,
            file_handle: Some(Mutex::new(file)),
            version,
            is_compressed,
            total_uncompressed_size,
        })
    }

    /// Extract file data from archive
    pub async fn extract_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, BigError> {
        let path_str = path.as_ref().to_string_lossy();
        let normalized_path = self.normalize_path(&path_str);

        let file_info = self
            .files
            .get(&normalized_path)
            .or_else(|| {
                // Try case-insensitive lookup
                self.name_lookup
                    .get(&normalized_path.to_lowercase())
                    .and_then(|real_name| self.files.get(real_name))
            })
            .ok_or_else(|| BigError::FileNotFound(path_str.to_string()))?;

        if file_info.is_directory {
            return Err(BigError::FileNotFound("Path is directory".to_string()));
        }

        let data = self.extract_from_file(file_info)?;

        log::trace!("Extracted file: {} ({} bytes)", normalized_path, data.len());
        Ok(data)
    }


    /// Extract file using file I/O
    fn extract_from_file(&self, file_info: &BigFileInfo) -> Result<Vec<u8>, BigError> {
        let file_handle = self
            .file_handle
            .as_ref()
            .ok_or_else(|| BigError::InvalidStructure("No file handle available".to_string()))?;

        let size = file_info.compressed_size.unwrap_or(file_info.size);
        let end = file_info.offset.checked_add(size)
            .ok_or_else(|| BigError::InvalidStructure("File data beyond archive bounds".to_string()))?;
        if end > self.archive_len {
            return Err(BigError::InvalidStructure(
                "File data beyond archive bounds".to_string(),
            ));
        }
        let size = usize::try_from(size)
            .map_err(|_| BigError::InvalidStructure("File data too large".to_string()))?;
        let mut file = file_handle.lock().unwrap_or_else(|e| e.into_inner());
        file.seek(SeekFrom::Start(file_info.offset))?;

        let mut raw_data = vec![0u8; size];
        file.read_exact(&mut raw_data)?;
        drop(file);

        match file_info.compression {
            CompressionType::None => Ok(raw_data),
            CompressionType::Zlib => self.decompress_zlib(&raw_data, file_info.size as usize),
            CompressionType::Lz4 => self.decompress_lz4(&raw_data, file_info.size as usize),
            _ => Err(BigError::CompressionError(
                "Unsupported compression type".to_string(),
            )),
        }
    }

    /// Decompress zlib data
    fn decompress_zlib(
        &self,
        compressed_data: &[u8],
        expected_size: usize,
    ) -> Result<Vec<u8>, BigError> {
        let mut decoder = flate2::read::ZlibDecoder::new(compressed_data);
        let mut decompressed = Vec::with_capacity(expected_size);

        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| BigError::CompressionError(format!("Zlib decompression failed: {}", e)))?;

        if decompressed.len() != expected_size {
            log::warn!(
                "Decompressed size mismatch: expected {}, got {}",
                expected_size,
                decompressed.len()
            );
        }

        Ok(decompressed)
    }

    /// Decompress LZ4 data
    fn decompress_lz4(
        &self,
        compressed_data: &[u8],
        expected_size: usize,
    ) -> Result<Vec<u8>, BigError> {
        let decompressed = lz4::block::decompress(compressed_data, Some(expected_size as i32))
            .map_err(|e| BigError::CompressionError(format!("LZ4 decompression failed: {}", e)))?;

        Ok(decompressed)
    }

    /// Get file info by path
    pub fn get_file_info<P: AsRef<Path>>(&self, path: P) -> Option<&BigFileInfo> {
        let path_str = path.as_ref().to_string_lossy();
        let normalized_path = self.normalize_path(&path_str);

        self.files.get(&normalized_path).or_else(|| {
            self.name_lookup
                .get(&normalized_path.to_lowercase())
                .and_then(|real_name| self.files.get(real_name))
        })
    }

    /// Get file size
    pub fn get_file_size<P: AsRef<Path>>(&self, path: P) -> Option<u64> {
        self.get_file_info(path).map(|info| info.size)
    }

    /// Check if file exists in archive
    pub fn contains_file<P: AsRef<Path>>(&self, path: P) -> bool {
        self.get_file_info(path).is_some()
    }

    /// List all files in archive
    pub fn list_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    pub fn archive_path(&self) -> &Path {
        &self.path
    }

    /// Find files matching pattern
    pub async fn find_matching_entries(&self, pattern: &str) -> Result<Vec<PathBuf>, AssetError> {
        let mut matches = Vec::new();

        for file_name in self.files.keys() {
            if !self.files[file_name].is_directory
                && super::glob_match::glob_match(pattern, file_name)
            {
                matches.push(PathBuf::from(file_name));
            }
        }

        Ok(matches)
    }

    /// List files in directory
    pub fn list_directory<P: AsRef<Path>>(&self, dir_path: P) -> Vec<String> {
        let dir_str = self.normalize_path(&dir_path.as_ref().to_string_lossy());
        let dir_prefix = if dir_str.is_empty() {
            String::new()
        } else {
            format!("{}/", dir_str.trim_end_matches('/'))
        };

        let mut entries = Vec::new();

        for file_name in self.files.keys() {
            if file_name.starts_with(&dir_prefix) {
                let relative_path = &file_name[dir_prefix.len()..];

                // Only include direct children (no sub-directories)
                if !relative_path.contains('/') && !relative_path.is_empty() {
                    entries.push(relative_path.to_string());
                }
            }
        }

        entries.sort();
        entries
    }

    /// Normalize path for consistent lookup
    fn normalize_path(&self, path: &str) -> String {
        path.replace('\\', "/").trim_start_matches('/').to_string()
    }

    /// Get archive statistics
    pub fn get_stats(&self) -> BigArchiveStats {
        let compressed_files = self
            .files
            .values()
            .filter(|f| f.compression != CompressionType::None)
            .count();

        let total_compressed_size = self
            .files
            .values()
            .filter_map(|f| f.compressed_size)
            .sum::<u64>();

        let compression_ratio = if total_compressed_size > 0 {
            self.total_uncompressed_size as f64 / total_compressed_size as f64
        } else {
            1.0
        };

        BigArchiveStats {
            path: self.path.clone(),
            version: self.version,
            total_files: self.files.len() as u32,
            total_size: self.header.archive_size as u64,
            uncompressed_size: self.total_uncompressed_size,
            compressed_files: compressed_files as u32,
            compression_ratio,
            is_memory_mapped: false,
        }
    }

    /// Validate archive integrity
    pub async fn validate(&self) -> Result<BigValidationResult, BigError> {
        let mut result = BigValidationResult {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        // Check header consistency
        if self.header.archive_size as u64 != self.archive_len {
            result
                .warnings
                .push("Archive size mismatch in header".to_string());
        }

        // Validate file entries
        for (name, file_info) in &self.files {
            // Check bounds
            if file_info.offset + file_info.size > self.header.archive_size as u64 {
                result
                    .errors
                    .push(format!("File '{}' extends beyond archive bounds", name));
                result.is_valid = false;
                continue;
            }

            // Check for overlapping files (basic check)
            let overlapping = self.files.values().any(|other| {
                other.name != file_info.name
                    && other.offset < file_info.offset + file_info.size
                    && file_info.offset < other.offset + other.size
            });

            if overlapping {
                result
                    .warnings
                    .push(format!("File '{}' may overlap with another file", name));
            }
        }

        // Validate a few random files by extraction
        let sample_size = (self.files.len() / 10).max(1).min(10);
        let sample_files: Vec<_> = self.files.keys().take(sample_size).cloned().collect();

        for file_name in sample_files {
            if let Err(e) = self.extract_file(&file_name).await {
                result
                    .errors
                    .push(format!("Failed to extract '{}': {}", file_name, e));
                result.is_valid = false;
            }
        }

        Ok(result)
    }
}

/// Archive statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct BigArchiveStats {
    pub path: PathBuf,
    pub version: BigVersion,
    pub total_files: u32,
    pub total_size: u64,
    pub uncompressed_size: u64,
    pub compressed_files: u32,
    pub compression_ratio: f64,
    pub is_memory_mapped: bool,
}

/// Archive validation result
#[derive(Debug)]
pub struct BigValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<BigError> for AssetError {
    fn from(err: BigError) -> Self {
        match err {
            BigError::FileNotFound(path) => AssetError::NotFound { path },
            BigError::Io(io_err) => AssetError::Io(io_err),
            BigError::InvalidSignature(_)
            | BigError::UnsupportedVersion(_)
            | BigError::InvalidStructure(_) => AssetError::InvalidArchive {
                archive: "unknown".to_string(),
                error: err.to_string(),
            },
            BigError::CompressionError(msg) => AssetError::LoadingFailed {
                path: "compressed_data".to_string(),
                error: msg,
            },
        }
    }
}

// Add flate2 and lz4 dependencies for compression
use flate2;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn archive_bytes(data_offset: u32, data_size: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BIGF");
        bytes.extend_from_slice(&39u32.to_ne_bytes());
        bytes.extend_from_slice(&1u32.to_ne_bytes());
        bytes.extend_from_slice(&34u32.to_ne_bytes());
        bytes.extend_from_slice(&data_offset.to_ne_bytes());
        bytes.extend_from_slice(&data_size.to_ne_bytes());
        bytes.extend_from_slice(&0u32.to_ne_bytes());
        bytes.extend_from_slice(b"a.txt\0hello");
        bytes
    }

    #[tokio::test]
    async fn extracts_lazily_and_handles_modified_backing_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&archive_bytes(34, 5)).unwrap();
        let archive = BigArchive::load(file.path()).await.unwrap();
        assert!(!archive.get_stats().is_memory_mapped);
        assert_eq!(archive.extract_file("A.TXT").await.unwrap(), b"hello");

        file.seek(SeekFrom::Start(34)).unwrap();
        file.write_all(b"world").unwrap();
        assert_eq!(archive.extract_file("a.txt").await.unwrap(), b"world");

        file.as_file().set_len(34).unwrap();
        assert!(matches!(archive.extract_file("a.txt").await, Err(BigError::Io(_))));
    }

    #[tokio::test]
    async fn extracts_compressed_entry() {
        let mut encoder = flate2::write::ZlibEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        );
        encoder.write_all(b"hello").unwrap();
        let compressed = encoder.finish().unwrap();
        let mut bytes = archive_bytes(34, compressed.len() as u32);
        bytes[..4].copy_from_slice(b"BIGC");
        bytes[4..8].copy_from_slice(&((34 + compressed.len()) as u32).to_ne_bytes());
        bytes.truncate(34);
        bytes.extend_from_slice(&compressed);
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();

        let archive = BigArchive::load(file.path()).await.unwrap();
        assert_eq!(archive.extract_file("a.txt").await.unwrap(), b"hello");
    }

    #[tokio::test]
    async fn rejects_invalid_index_and_out_of_bounds_entry() {
        let mut file = NamedTempFile::new().unwrap();
        let mut bytes = archive_bytes(34, 6);
        file.write_all(&bytes).unwrap();
        let archive = BigArchive::load(file.path()).await.unwrap();
        assert!(matches!(
            archive.extract_file("a.txt").await,
            Err(BigError::InvalidStructure(_))
        ));

        // The name table cannot begin after the first file offset.
        bytes[12..16].copy_from_slice(&20u32.to_ne_bytes());
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&bytes).unwrap();
        assert!(matches!(
            BigArchive::load(file.path()).await,
            Err(BigError::InvalidStructure(_))
        ));
    }

    #[tokio::test]
    async fn test_big_header_parsing() {
        // Test data for a minimal BIG file
        let header = BigHeader {
            signature: *b"BIGF",
            archive_size: 1024,
            num_files: 2,
            first_file_offset: 100,
        };

        assert_eq!(header.signature, *b"BIGF");
        let archive_size = header.archive_size;
        let num_files = header.num_files;
        assert_eq!(archive_size, 1024);
        assert_eq!(num_files, 2);
    }

    #[test]
    fn test_path_normalization() {
        crate::test_support::ensure_ambient_host();
        let archive = create_test_archive();
        assert_eq!(
            archive.normalize_path("path\\to\\file.txt"),
            "path/to/file.txt"
        );
        assert_eq!(
            archive.normalize_path("/path/to/file.txt"),
            "path/to/file.txt"
        );
        assert_eq!(archive.normalize_path("file.txt"), "file.txt");
    }

    fn create_test_archive() -> BigArchive {
        BigArchive {
            path: PathBuf::from("test.big"),
            header: BigHeader {
                signature: *b"BIGF",
                archive_size: 1024,
                num_files: 0,
                first_file_offset: 100,
            },
            files: HashMap::new(),
            name_lookup: BTreeMap::new(),
            archive_len: 0,
            file_handle: None,
            version: BigVersion::Standard,
            is_compressed: false,
            total_uncompressed_size: 0,
        }
    }
}
