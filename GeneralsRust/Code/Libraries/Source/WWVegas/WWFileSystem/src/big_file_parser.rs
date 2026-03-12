use crate::archived_file_info::ArchivedFileInfo;
use std::io::{self, Read, Seek, SeekFrom};

/// BIG file format identifier
const BIG_FILE_IDENTIFIER: &[u8; 4] = b"BIGF";

/// BIG file header offset for the file table
const BIG_FILE_TABLE_OFFSET: u64 = 0x10;

/// Represents the header of a BIG file
#[derive(Debug)]
pub struct BigFileHeader {
    pub identifier: [u8; 4],
    pub archive_size: u32,
    pub num_files: u32,
}

/// Entry in the BIG file directory table
#[derive(Debug, Clone)]
pub struct BigFileEntry {
    pub offset: u32,
    pub size: u32,
    pub name: String,
}

/// Parser for BIG archive files
/// Matches the C++ Win32BIGFileSystem::openArchiveFile implementation
pub struct BigFileParser;

impl BigFileParser {
    /// Parse a BIG file header from a reader
    pub fn parse_header<R: Read>(reader: &mut R) -> io::Result<BigFileHeader> {
        let mut identifier = [0u8; 4];
        reader.read_exact(&mut identifier)?;

        if &identifier != BIG_FILE_IDENTIFIER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid BIG file identifier. Expected 'BIGF', got '{}'",
                    String::from_utf8_lossy(&identifier)
                ),
            ));
        }

        let mut archive_size_buf = [0u8; 4];
        reader.read_exact(&mut archive_size_buf)?;
        let archive_size = u32::from_le_bytes(archive_size_buf);

        let mut num_files_buf = [0u8; 4];
        reader.read_exact(&mut num_files_buf)?;
        // BIG files store the number of files in network byte order (big-endian)
        let num_files = u32::from_be_bytes(num_files_buf);

        Ok(BigFileHeader {
            identifier,
            archive_size,
            num_files,
        })
    }

    /// Parse all file entries from a BIG file
    pub fn parse_entries<R: Read + Seek>(
        reader: &mut R,
        num_files: u32,
    ) -> io::Result<Vec<BigFileEntry>> {
        // Seek to the beginning of the directory listing
        reader.seek(SeekFrom::Start(BIG_FILE_TABLE_OFFSET))?;

        let mut entries = Vec::with_capacity(num_files as usize);

        for _ in 0..num_files {
            // Read file offset (big-endian)
            let mut offset_buf = [0u8; 4];
            reader.read_exact(&mut offset_buf)?;
            let offset = u32::from_be_bytes(offset_buf);

            // Read file size (big-endian)
            let mut size_buf = [0u8; 4];
            reader.read_exact(&mut size_buf)?;
            let size = u32::from_be_bytes(size_buf);

            // Read null-terminated path name
            let mut name_bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                reader.read_exact(&mut byte)?;
                if byte[0] == 0 {
                    break;
                }
                name_bytes.push(byte[0]);
            }

            let name = String::from_utf8_lossy(&name_bytes).to_string();

            entries.push(BigFileEntry { offset, size, name });
        }

        Ok(entries)
    }

    /// Convert a BigFileEntry to an ArchivedFileInfo
    /// Extracts the filename from the path and stores the full path
    pub fn entry_to_archived_file_info(
        entry: &BigFileEntry,
        archive_filename: &str,
    ) -> (String, ArchivedFileInfo) {
        let path = entry.name.to_lowercase();

        // Find the filename by searching backwards for path separators
        let filename = if let Some(pos) = path.rfind(|c| c == '\\' || c == '/') {
            path[(pos + 1)..].to_string()
        } else {
            path.clone()
        };

        // Extract the directory path (everything before the filename)
        let dir_path = if let Some(pos) = path.rfind(|c| c == '\\' || c == '/') {
            path[..=pos].to_string()
        } else {
            String::new()
        };

        let file_info = ArchivedFileInfo {
            filename: filename.clone(),
            archive_filename: archive_filename.to_string(),
            offset: entry.offset,
            size: entry.size,
        };

        (dir_path, file_info)
    }

    /// Parse a complete BIG file and return all entries as ArchivedFileInfo structures
    pub fn parse_big_file<R: Read + Seek>(
        reader: &mut R,
        archive_filename: &str,
    ) -> io::Result<Vec<(String, ArchivedFileInfo)>> {
        let header = Self::parse_header(reader)?;
        let entries = Self::parse_entries(reader, header.num_files)?;

        let mut file_infos = Vec::with_capacity(entries.len());
        for entry in &entries {
            let info = Self::entry_to_archived_file_info(entry, archive_filename);
            file_infos.push(info);
        }

        Ok(file_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_header_valid() {
        let data = vec![
            b'B', b'I', b'G', b'F', // Identifier
            0x00, 0x01, 0x00, 0x00, // Archive size (little-endian)
            0x00, 0x00, 0x00, 0x02, // Number of files (big-endian = 2)
        ];
        let mut cursor = Cursor::new(data);
        let header = BigFileParser::parse_header(&mut cursor).unwrap();

        assert_eq!(&header.identifier, b"BIGF");
        assert_eq!(header.archive_size, 256);
        assert_eq!(header.num_files, 2);
    }

    #[test]
    fn test_parse_header_invalid_identifier() {
        let data = vec![
            b'X', b'X', b'X', b'X', // Invalid identifier
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        ];
        let mut cursor = Cursor::new(data);
        let result = BigFileParser::parse_header(&mut cursor);

        assert!(result.is_err());
    }

    #[test]
    fn test_entry_to_archived_file_info() {
        let entry = BigFileEntry {
            offset: 0x1000,
            size: 0x500,
            name: "Art\\Textures\\test.tga".to_string(),
        };

        let (dir_path, file_info) = BigFileParser::entry_to_archived_file_info(&entry, "test.big");

        assert_eq!(dir_path, "art\\textures\\");
        assert_eq!(file_info.filename, "test.tga");
        assert_eq!(file_info.archive_filename, "test.big");
        assert_eq!(file_info.offset, 0x1000);
        assert_eq!(file_info.size, 0x500);
    }
}
