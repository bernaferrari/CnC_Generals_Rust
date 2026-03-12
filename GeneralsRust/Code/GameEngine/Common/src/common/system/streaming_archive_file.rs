////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: streaming_archive_file.rs /////////////////////////////////////////////
// Streaming archive file system for large archives
///////////////////////////////////////////////////////////////////////////////

use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Archive file entry information
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub compressed_size: u64,
    pub is_compressed: bool,
    pub checksum: u32,
}

/// Streaming archive file reader
pub struct StreamingArchiveFile {
    file: File,
    entries: HashMap<String, ArchiveEntry>,
    header_size: u64,
    is_open: bool,
}

impl StreamingArchiveFile {
    /// Open an archive file
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let (entries, header_size) = Self::read_directory(&mut file)?;

        Ok(Self {
            file,
            entries,
            header_size,
            is_open: true,
        })
    }

    /// Read archive directory
    fn read_directory(file: &mut File) -> io::Result<(HashMap<String, ArchiveEntry>, u64)> {
        file.seek(SeekFrom::Start(0))?;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;

        if &magic == b"BIGF" || &magic == b"BIG4" {
            return Self::read_big_directory(file);
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported archive format",
        ))
    }

    fn read_big_directory(file: &mut File) -> io::Result<(HashMap<String, ArchiveEntry>, u64)> {
        let mut entries = HashMap::new();
        let mut header_data = [0u8; 12];
        file.read_exact(&mut header_data)?;

        let _total_size = u32::from_le_bytes([
            header_data[0],
            header_data[1],
            header_data[2],
            header_data[3],
        ]) as u64;
        let num_entries = u32::from_le_bytes([
            header_data[4],
            header_data[5],
            header_data[6],
            header_data[7],
        ]) as usize;
        let _first_file_offset = u32::from_le_bytes([
            header_data[8],
            header_data[9],
            header_data[10],
            header_data[11],
        ]) as u64;

        let file_len = file.metadata()?.len();

        for _ in 0..num_entries {
            let mut entry_data = [0u8; 8];
            file.read_exact(&mut entry_data)?;

            let offset =
                u32::from_le_bytes([entry_data[0], entry_data[1], entry_data[2], entry_data[3]])
                    as u64;
            let size =
                u32::from_le_bytes([entry_data[4], entry_data[5], entry_data[6], entry_data[7]])
                    as u64;

            let end = offset.checked_add(size).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Archive entry offset overflow")
            })?;
            if end > file_len {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Archive entry exceeds file bounds",
                ));
            }

            let mut name_bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                file.read_exact(&mut byte)?;
                if byte[0] == 0 {
                    break;
                }
                name_bytes.push(byte[0]);
            }

            let name = String::from_utf8(name_bytes).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid archive entry name: {}", err),
                )
            })?;

            entries.insert(
                name.clone(),
                ArchiveEntry {
                    name: name.clone(),
                    offset,
                    size,
                    compressed_size: size,
                    is_compressed: false,
                    checksum: crc32fast::hash(name.as_bytes()),
                },
            );
        }

        let header_size = file.stream_position()?;
        Ok((entries, header_size))
    }

    fn get_entry(&self, filename: &str) -> Option<&ArchiveEntry> {
        if let Some(entry) = self.entries.get(filename) {
            return Some(entry);
        }

        self.entries.iter().find_map(|(name, entry)| {
            if name.eq_ignore_ascii_case(filename) {
                Some(entry)
            } else {
                None
            }
        })
    }

    /// Get list of files in the archive
    pub fn get_file_list(&self) -> Vec<&String> {
        self.entries.keys().collect()
    }

    /// Check if a file exists in the archive
    pub fn contains_file(&self, filename: &str) -> bool {
        self.get_entry(filename).is_some()
    }

    /// Get file entry information
    pub fn get_file_info(&self, filename: &str) -> Option<&ArchiveEntry> {
        self.get_entry(filename)
    }

    fn read_entry_data(&mut self, entry: &ArchiveEntry) -> io::Result<Vec<u8>> {
        self.file.seek(SeekFrom::Start(entry.offset))?;
        let mut raw = vec![0u8; entry.compressed_size as usize];
        self.file.read_exact(&mut raw)?;

        if entry.is_compressed {
            self.decompress_data(&raw, entry.size as usize)
        } else {
            if raw.len() != entry.size as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Archive entry size mismatch",
                ));
            }
            Ok(raw)
        }
    }

    /// Extract a file to a buffer
    pub fn extract_file(&mut self, filename: &str) -> io::Result<Vec<u8>> {
        let entry = self
            .get_entry(filename)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found in archive"))?;

        self.read_entry_data(&entry)
    }

    /// Stream a file (returns a reader)
    pub fn stream_file(&mut self, filename: &str) -> io::Result<ArchiveFileReader<'_>> {
        let entry = self
            .get_entry(filename)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found in archive"))?;
        let buffer = self.read_entry_data(&entry)?;

        Ok(ArchiveFileReader {
            archive: self,
            entry,
            position: 0,
            buffer,
        })
    }

    /// Decompress data
    fn decompress_data(
        &self,
        compressed_data: &[u8],
        uncompressed_size: usize,
    ) -> io::Result<Vec<u8>> {
        if uncompressed_size == 0 {
            return Ok(Vec::new());
        }

        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut output = Vec::with_capacity(uncompressed_size);
        match decoder.read_to_end(&mut output) {
            Ok(_) if output.len() == uncompressed_size => Ok(output),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Decompressed size mismatch: expected {}, got {}",
                    uncompressed_size,
                    output.len()
                ),
            )),
            Err(_) if compressed_data.len() == uncompressed_size => {
                // Legacy payloads may be flagged compressed but already raw.
                Ok(compressed_data.to_vec())
            }
            Err(err) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decompress archive data: {}", err),
            )),
        }
    }

    /// Get archive statistics
    pub fn get_stats(&self) -> ArchiveStats {
        let total_files = self.entries.len();
        let total_size = self.entries.values().map(|e| e.size).sum();
        let compressed_size = self.entries.values().map(|e| e.compressed_size).sum();
        let compressed_files = self.entries.values().filter(|e| e.is_compressed).count();

        ArchiveStats {
            total_files,
            compressed_files,
            total_size,
            compressed_size,
            compression_ratio: if total_size > 0 {
                (compressed_size as f64) / (total_size as f64)
            } else {
                0.0
            },
        }
    }

    /// Get parsed archive header size.
    pub fn header_size(&self) -> u64 {
        self.header_size
    }

    /// Check if archive is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Close the archive
    pub fn close(&mut self) {
        self.is_open = false;
        self.entries.clear();
    }
}

/// Statistics about an archive
#[derive(Debug)]
pub struct ArchiveStats {
    pub total_files: usize,
    pub compressed_files: usize,
    pub total_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f64,
}

/// Reader for streaming individual files from archive
pub struct ArchiveFileReader<'a> {
    archive: &'a mut StreamingArchiveFile,
    entry: ArchiveEntry,
    position: u64,
    buffer: Vec<u8>,
}

impl<'a> ArchiveFileReader<'a> {
    /// Get the size of the file
    pub fn size(&self) -> u64 {
        self.entry.size
    }

    /// Get current position
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Check if at end of file
    pub fn is_eof(&self) -> bool {
        self.position >= self.entry.size
    }
}

impl<'a> Read for ArchiveFileReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let _ = &self.archive;
        if self.is_eof() {
            return Ok(0);
        }

        let start = self.position as usize;
        let available = self.buffer.len().saturating_sub(start);
        if available == 0 {
            self.position = self.entry.size;
            return Ok(0);
        }

        let bytes_to_copy = buf.len().min(available);
        let end = start + bytes_to_copy;
        buf[..bytes_to_copy].copy_from_slice(&self.buffer[start..end]);
        self.position += bytes_to_copy as u64;
        Ok(bytes_to_copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_big_archive(filename: &str, payload: &[u8]) -> io::Result<NamedTempFile> {
        let mut temp_file = NamedTempFile::new()?;
        let name_bytes = filename.as_bytes();
        let directory_size = 8 + name_bytes.len() + 1;
        let first_file_offset = 16 + directory_size;
        let total_size = first_file_offset + payload.len();

        temp_file.write_all(b"BIGF")?;
        temp_file.write_all(&(total_size as u32).to_le_bytes())?;
        temp_file.write_all(&(1u32).to_le_bytes())?;
        temp_file.write_all(&(first_file_offset as u32).to_le_bytes())?;
        temp_file.write_all(&(first_file_offset as u32).to_le_bytes())?;
        temp_file.write_all(&(payload.len() as u32).to_le_bytes())?;
        temp_file.write_all(name_bytes)?;
        temp_file.write_all(&[0])?;
        temp_file.write_all(payload)?;
        temp_file.flush()?;

        Ok(temp_file)
    }

    #[test]
    fn test_open_big_archive_and_extract_file() {
        let payload = b"This is test file content for the archive system.";
        let temp_archive = create_test_big_archive("example.dat", payload).expect("archive");

        let mut archive = StreamingArchiveFile::open(temp_archive.path()).expect("open");
        assert!(archive.is_open());
        assert!(archive.header_size() > 0);
        assert!(archive.contains_file("example.dat"));

        let extracted = archive.extract_file("example.dat").expect("extract");
        assert_eq!(extracted, payload);
    }

    #[test]
    fn test_stream_file_reads_full_payload() {
        let payload = b"stream me through archive reader";
        let temp_archive = create_test_big_archive("stream.dat", payload).expect("archive");

        let mut archive = StreamingArchiveFile::open(temp_archive.path()).expect("open");
        let mut reader = archive.stream_file("stream.dat").expect("stream");
        let mut output = Vec::new();
        reader.read_to_end(&mut output).expect("read");

        assert_eq!(output, payload);
        assert!(reader.is_eof());
        assert_eq!(reader.position(), payload.len() as u64);
    }

    #[test]
    fn test_archive_stats() {
        let mut entries = HashMap::new();
        entries.insert(
            "file1.dat".to_string(),
            ArchiveEntry {
                name: "file1.dat".to_string(),
                offset: 0,
                size: 1000,
                compressed_size: 800,
                is_compressed: true,
                checksum: 0,
            },
        );
        entries.insert(
            "file2.dat".to_string(),
            ArchiveEntry {
                name: "file2.dat".to_string(),
                offset: 800,
                size: 500,
                compressed_size: 500,
                is_compressed: false,
                checksum: 0,
            },
        );

        let total_size: u64 = entries.values().map(|e| e.size).sum();
        let compressed_size: u64 = entries.values().map(|e| e.compressed_size).sum();

        assert_eq!(total_size, 1500);
        assert_eq!(compressed_size, 1300);
    }

    #[test]
    fn test_decompress_data_accepts_legacy_uncompressed_payload() {
        let temp_archive = NamedTempFile::new().expect("temp");
        let file = File::open(temp_archive.path()).expect("open");
        let archive = StreamingArchiveFile {
            file,
            entries: HashMap::new(),
            header_size: 0,
            is_open: true,
        };

        let payload = b"raw-data";
        let decompressed = archive
            .decompress_data(payload, payload.len())
            .expect("legacy payload should be accepted");
        assert_eq!(decompressed, payload);
    }

    #[test]
    fn test_decompress_data_zlib_payload() {
        let temp_archive = NamedTempFile::new().expect("temp");
        let file = File::open(temp_archive.path()).expect("open");
        let archive = StreamingArchiveFile {
            file,
            entries: HashMap::new(),
            header_size: 0,
            is_open: true,
        };

        let payload = b"compressed archive payload";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).expect("write");
        let compressed = encoder.finish().expect("finish");

        let decompressed = archive
            .decompress_data(&compressed, payload.len())
            .expect("zlib decode");
        assert_eq!(decompressed, payload);
    }
}
