////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Archive File System Implementation
//!
//! This module provides the core archive file functionality for the game engine.
//! It handles reading and managing files within archive containers.
//!
//! Bryan Cleveland, August 2002
//! Rust conversion: 2025

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};

use crate::common::ascii_string::AsciiString;

/// File information for files within archives
#[derive(Debug, Clone)]
pub struct ArchivedFileInfo {
    pub filename: AsciiString,
    pub offset: u64,
    pub size: u64,
    pub compressed_size: Option<u64>,
    pub is_compressed: bool,
}

/// Detailed directory information for archives
#[derive(Debug, Clone)]
pub struct DetailedArchivedDirectoryInfo {
    pub directory_name: AsciiString,
    pub directories: HashMap<AsciiString, DetailedArchivedDirectoryInfo>,
    pub files: HashMap<AsciiString, ArchivedFileInfo>,
}

impl Default for DetailedArchivedDirectoryInfo {
    fn default() -> Self {
        Self {
            directory_name: AsciiString::new(),
            directories: HashMap::new(),
            files: HashMap::new(),
        }
    }
}

impl DetailedArchivedDirectoryInfo {
    pub fn clear(&mut self) {
        self.directory_name.clear();
        self.directories.clear();
        self.files.clear();
    }
}

use crate::common::system::file_system::FilenameList;

/// Map of archived file information
pub type ArchivedFileInfoMap = HashMap<AsciiString, ArchivedFileInfo>;

/// Map of directory information
pub type DetailedArchivedDirectoryInfoMap = HashMap<AsciiString, DetailedArchivedDirectoryInfo>;

/// FileInfo structure for basic file information
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub size: u64,
    pub modified_time: u64,
    pub is_directory: bool,
}

/// Checks if a string matches a search pattern with wildcards
/// * matches any number of characters
/// ? matches a single character
fn search_string_matches(text: &AsciiString, pattern: &AsciiString) -> bool {
    if text.is_empty() {
        return pattern.is_empty();
    }
    if pattern.is_empty() {
        return false;
    }

    let text_chars: Vec<char> = text.as_str().chars().collect();
    let pattern_chars: Vec<char> = pattern.as_str().chars().collect();

    search_string_matches_recursive(&text_chars, &pattern_chars, 0, 0)
}

fn search_string_matches_recursive(
    text: &[char],
    pattern: &[char],
    text_idx: usize,
    pattern_idx: usize,
) -> bool {
    if pattern_idx >= pattern.len() {
        return text_idx >= text.len();
    }

    if text_idx >= text.len() {
        // Check if remaining pattern is all '*'
        for i in pattern_idx..pattern.len() {
            if pattern[i] != '*' {
                return false;
            }
        }
        return true;
    }

    match pattern[pattern_idx] {
        '*' => {
            // Try matching zero or more characters
            if pattern_idx + 1 >= pattern.len() {
                return true; // '*' at end matches everything
            }

            // Try matching zero characters
            if search_string_matches_recursive(text, pattern, text_idx, pattern_idx + 1) {
                return true;
            }

            // Try matching one or more characters
            for i in text_idx..text.len() {
                if search_string_matches_recursive(text, pattern, i + 1, pattern_idx + 1) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // Match any single character
            search_string_matches_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
        }
        c if c == text[text_idx] => {
            // Exact character match
            search_string_matches_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
        }
        _ => false,
    }
}

/// Archive file trait defining the interface for archive files
pub trait ArchiveFileTrait {
    /// Fill in the file_info struct with info about the requested file
    fn get_file_info(
        &self,
        filename: &AsciiString,
        file_info: &mut FileInfo,
    ) -> Result<bool, io::Error>;

    /// Open the specified file within the archive file
    fn open_file(&mut self, filename: &str, access: i32)
        -> Result<Box<dyn Read + Send>, io::Error>;

    /// Close all files opened in this archive file
    fn close_all_files(&mut self);

    /// Returns the name of the archive file
    fn get_name(&self) -> AsciiString;

    /// Returns full path and name of archive file
    fn get_path(&self) -> AsciiString;

    /// Set this archive file's search priority
    fn set_search_priority(&mut self, new_priority: i32);

    /// Close this archive file
    fn close(&mut self);
}

/// Main archive file implementation
pub struct ArchiveFile {
    file: Option<File>,
    root_directory: DetailedArchivedDirectoryInfo,
}

impl Default for ArchiveFile {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveFile {
    /// Create a new archive file instance
    pub fn new() -> Self {
        Self {
            file: None,
            root_directory: DetailedArchivedDirectoryInfo::default(),
        }
    }

    /// Add a file to the archive directory tree
    pub fn add_file(&mut self, path: &AsciiString, file_info: &ArchivedFileInfo) {
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
        for (i, token) in path_parts.iter().enumerate() {
            if i == path_parts.len() - 1 {
                // This is the filename, don't create a directory for it
                break;
            }

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

    /// Get list of files in a directory
    pub fn get_file_list_in_directory(
        &self,
        _current_directory: &AsciiString,
        original_directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        let mut search_dir = original_directory.clone();
        search_dir.to_lower();

        let mut dir_info = &self.root_directory;

        // Navigate to the target directory
        let path_parts: Vec<AsciiString> = search_dir
            .as_str()
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .map(|s| AsciiString::from(s))
            .collect();

        for token in &path_parts {
            if let Some(next_dir) = dir_info.directories.get(token) {
                dir_info = next_dir;
            } else {
                // Directory doesn't exist, no files to find
                return;
            }
        }

        self.get_file_list_in_directory_recursive(
            dir_info,
            original_directory,
            search_name,
            filename_list,
            search_subdirectories,
        );
    }

    /// Recursive helper for getting file lists
    fn get_file_list_in_directory_recursive(
        &self,
        dir_info: &DetailedArchivedDirectoryInfo,
        current_directory: &AsciiString,
        search_name: &AsciiString,
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        // Search subdirectories if requested
        if search_subdirectories {
            for (_, sub_dir_info) in &dir_info.directories {
                let mut temp_dir_name = current_directory.clone();
                if !temp_dir_name.is_empty() && !temp_dir_name.ends_with("\\") {
                    temp_dir_name.push('\\');
                }
                temp_dir_name.push_str(&sub_dir_info.directory_name.as_str());

                self.get_file_list_in_directory_recursive(
                    sub_dir_info,
                    &temp_dir_name,
                    search_name,
                    filename_list,
                    search_subdirectories,
                );
            }
        }

        // Search files in current directory
        for (_, file_info) in &dir_info.files {
            if search_string_matches(&file_info.filename, search_name) {
                let mut temp_filename = current_directory.clone();
                if !temp_filename.is_empty() && !temp_filename.ends_with("\\") {
                    temp_filename.push('\\');
                }
                temp_filename.push_str(&file_info.filename.as_str());

                filename_list.insert(temp_filename);
            }
        }
    }

    /// Attach a file to this archive
    pub fn attach_file(&mut self, file: File) {
        self.file = Some(file);
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

impl Drop for ArchiveFile {
    fn drop(&mut self) {
        if let Some(_file) = self.file.take() {
            // File will be automatically closed when dropped
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_string_matches() {
        let text = AsciiString::from("test.txt");
        let pattern1 = AsciiString::from("*.txt");
        let pattern2 = AsciiString::from("test.*");
        let pattern3 = AsciiString::from("t?st.txt");
        let pattern4 = AsciiString::from("*.exe");

        assert!(search_string_matches(&text, &pattern1));
        assert!(search_string_matches(&text, &pattern2));
        assert!(search_string_matches(&text, &pattern3));
        assert!(!search_string_matches(&text, &pattern4));
    }

    #[test]
    fn test_archive_file_creation() {
        let archive = ArchiveFile::new();
        assert!(archive.file.is_none());
    }

    #[test]
    fn test_add_file() {
        let mut archive = ArchiveFile::new();
        let file_info = ArchivedFileInfo {
            filename: AsciiString::from("test.txt"),
            offset: 0,
            size: 100,
            compressed_size: None,
            is_compressed: false,
        };

        let path = AsciiString::from("folder/test.txt");
        archive.add_file(&path, &file_info);

        let retrieved = archive.get_archived_file_info(&AsciiString::from("folder/test.txt"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename.as_str(), "test.txt");
    }
}
