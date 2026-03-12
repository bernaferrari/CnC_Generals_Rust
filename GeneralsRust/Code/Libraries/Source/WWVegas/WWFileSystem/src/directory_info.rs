use crate::archived_file_info::ArchivedFileInfo;
use std::collections::BTreeMap;

/// ArchivedDirectoryInfo structure matching the C++ implementation
/// Represents a directory in the archive file system with files and subdirectories
#[derive(Debug, Clone)]
pub struct ArchivedDirectoryInfo {
    pub directory_name: String,
    pub directories: BTreeMap<String, ArchivedDirectoryInfo>,
    pub files: BTreeMap<String, String>, // filename -> archive filename
}

impl ArchivedDirectoryInfo {
    pub fn new() -> Self {
        Self {
            directory_name: String::new(),
            directories: BTreeMap::new(),
            files: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.directory_name.clear();
        self.directories.clear();
        self.files.clear();
    }
}

impl Default for ArchivedDirectoryInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// DetailedArchivedDirectoryInfo structure matching the C++ implementation
/// Stores detailed information about directories including full file metadata
#[derive(Debug, Clone)]
pub struct DetailedArchivedDirectoryInfo {
    pub directory_name: String,
    pub directories: BTreeMap<String, DetailedArchivedDirectoryInfo>,
    pub files: BTreeMap<String, ArchivedFileInfo>,
}

impl DetailedArchivedDirectoryInfo {
    pub fn new() -> Self {
        Self {
            directory_name: String::new(),
            directories: BTreeMap::new(),
            files: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.directory_name.clear();
        self.directories.clear();
        self.files.clear();
    }
}

impl Default for DetailedArchivedDirectoryInfo {
    fn default() -> Self {
        Self::new()
    }
}
