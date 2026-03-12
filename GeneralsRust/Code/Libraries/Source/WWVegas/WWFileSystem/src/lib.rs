/*!
# WWFileSystem - BIG Archive File System

This library provides a faithful Rust port of the C++ BigFile/Archive system from Command & Conquer: Generals Zero Hour.

## Features

- **BIG File Format Parsing**: Complete implementation of the BIGF archive format
- **File Lookup by Name**: Fast case-insensitive file lookup with directory tree navigation
- **Archive Mounting/Unmounting**: Dynamic loading and unloading of archive files
- **File Extraction/Streaming**: Read files directly from archives without full extraction
- **Virtual File System Integration**: Unified interface for local and archived files

## Architecture

The system is organized into several key components:

### Core Structures

- `FileInfo`: Metadata about files (size, timestamp)
- `ArchivedFileInfo`: Information about files within archives (offset, size, archive name)
- `ArchivedDirectoryInfo`: Directory tree structure for quick lookups
- `DetailedArchivedDirectoryInfo`: Extended directory information with full file metadata

### File System Layers

1. **BigFileParser**: Low-level BIG format parser
2. **ArchiveFile**: Individual archive file management
3. **ArchiveFileSystem**: Multiple archive coordination
4. **FileSystem**: Unified local + archive interface

## Usage

```rust,ignore
use ww_file_system::FileSystem;

fn main() -> std::io::Result<()> {
    // Create and initialize the file system
    let mut fs = FileSystem::new();
    fs.init()?;

    // Check if a file exists
    if fs.does_file_exist("Art/Textures/test.tga") {
        // Read file data
        let data = fs.open_file("Art/Textures/test.tga", 0)?;
        // Process data...
    }

    // List files in a directory
    let files = fs.get_file_list_in_directory("Art/W3D", "*.w3d", true);
    for filename in files {
        println!("Found: {}", filename);
    }

    Ok(())
}
```

## BIG File Format

The BIG file format used by Command & Conquer games has the following structure:

```text
Offset  Size  Description
------  ----  -----------
0x00    4     Identifier "BIGF"
0x04    4     Archive size (little-endian)
0x08    4     Number of files (big-endian)
0x0C    4     First file table offset (big-endian)
0x10    ...   File table entries

Each file entry:
0x00    4     File offset (big-endian)
0x04    4     File size (big-endian)
0x08    ...   Null-terminated file path
```

## Implementation Notes

This implementation faithfully reproduces the C++ logic:

- Case-insensitive file lookups (all paths converted to lowercase)
- Backslash and forward slash both accepted as path separators
- Wildcard matching with `*` and `?` characters
- Network byte order (big-endian) for file table entries
- Little-endian for archive size field
- Directory tree structure for O(log n) file lookups

*/

// Module declarations
pub mod archive_file;
pub mod archive_file_system;
pub mod archived_file_info;
pub mod big_file_parser;
pub mod directory_info;
pub mod file_info;
pub mod file_system;
pub mod search_string;

// Re-export main types for convenience
pub use archive_file::{open_big_archive, ArchiveFile, ArchiveFileTrait, FilenameList};
pub use archive_file_system::ArchiveFileSystem;
pub use archived_file_info::ArchivedFileInfo;
pub use big_file_parser::{BigFileEntry, BigFileHeader, BigFileParser};
pub use directory_info::{ArchivedDirectoryInfo, DetailedArchivedDirectoryInfo};
pub use file_info::FileInfo;
pub use file_system::FileSystem;
pub use search_string::search_string_matches;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_file_system_integration() {
        let mut fs = FileSystem::new();
        assert!(fs.init().is_ok());
    }

    #[test]
    fn test_archive_file_system_integration() {
        let mut afs = ArchiveFileSystem::new();
        assert!(afs.init().is_ok());
    }
}
