use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::archive_file::{open_big_archive, ArchiveFile, ArchiveFileTrait, FilenameList};
use crate::directory_info::ArchivedDirectoryInfo;
use crate::file_info::FileInfo;

/// ArchiveFileSystem manages multiple archive files
/// Matches the C++ ArchiveFileSystem class
pub struct ArchiveFileSystem {
    archive_file_map: BTreeMap<String, ArchiveFile>,
    root_directory: ArchivedDirectoryInfo,
}

impl ArchiveFileSystem {
    pub fn new() -> Self {
        Self {
            archive_file_map: BTreeMap::new(),
            root_directory: ArchivedDirectoryInfo::new(),
        }
    }

    /// Initialize the archive file system
    /// Matches the C++ Win32BIGFileSystem::init implementation
    pub fn init(&mut self) -> io::Result<()> {
        // Load BIG files from current directory
        self.load_big_files_from_directory("", "*.big", false)?;
        Ok(())
    }

    /// Update the file system (called each frame)
    pub fn update(&mut self) {
        // No-op in the C++ implementation
    }

    /// Reset the file system
    pub fn reset(&mut self) {
        // No-op in the C++ implementation
    }

    /// Open an archive file by path
    /// Matches the C++ Win32BIGFileSystem::openArchiveFile implementation
    pub fn open_archive_file<P: AsRef<Path>>(&mut self, filename: P) -> io::Result<()> {
        let path = filename.as_ref();
        let archive_filename = path
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid path"))?
            .to_lowercase();

        // Check if already opened
        if self.archive_file_map.contains_key(&archive_filename) {
            return Ok(());
        }

        let archive = open_big_archive(path)?;

        // Load into directory tree
        self.load_into_directory_tree(&archive, &archive_filename, false);

        self.archive_file_map.insert(archive_filename, archive);

        Ok(())
    }

    /// Close a specific archive file
    /// Matches the C++ Win32BIGFileSystem::closeArchiveFile implementation
    pub fn close_archive_file(&mut self, filename: &str) {
        let filename_lower = filename.to_lowercase();
        self.archive_file_map.remove(&filename_lower);
    }

    /// Close all archive files
    pub fn close_all_archive_files(&mut self) {
        self.archive_file_map.clear();
        self.root_directory.clear();
    }

    /// Close all open files within archives
    pub fn close_all_files(&mut self) {
        for (_, archive) in &mut self.archive_file_map {
            archive.close_all_files();
        }
    }

    /// Check if a file exists in any archive
    /// Matches the C++ ArchiveFileSystem::doesFileExist implementation
    pub fn does_file_exist(&self, filename: &str) -> bool {
        let mut path = filename.to_lowercase();
        let mut dir_info = &self.root_directory;

        // Navigate through directories
        loop {
            if let Some(pos) = path.find(|c| c == '\\' || c == '/') {
                let token = path[..pos].to_string();
                path = path[(pos + 1)..].to_string();

                if token.is_empty() {
                    continue;
                }

                // Check if this token has a dot and there's no more path
                let has_dot = token.contains('.');
                let path_has_dot = path.contains('.');

                if has_dot && !path_has_dot {
                    // This is the filename
                    return dir_info.files.contains_key(&token);
                }

                // Navigate to next directory
                match dir_info.directories.get(&token) {
                    Some(next_dir) => dir_info = next_dir,
                    None => return false,
                }
            } else {
                // Last token is the filename
                return dir_info.files.contains_key(&path);
            }
        }
    }

    /// Open a file from any archive
    /// Matches the C++ ArchiveFileSystem::openFile implementation
    pub fn open_file(&mut self, filename: &str, access: i32) -> io::Result<Vec<u8>> {
        let archive_filename = self.get_archive_filename_for_file(filename);

        if archive_filename.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File '{}' not found in any archive", filename),
            ));
        }

        let archive = self
            .archive_file_map
            .get_mut(&archive_filename)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Archive '{}' not found", archive_filename),
                )
            })?;

        archive.open_file(filename, access)
    }

    /// Get file info from archives
    /// Matches the C++ ArchiveFileSystem::getFileInfo implementation
    pub fn get_file_info(&self, filename: &str) -> Option<FileInfo> {
        let archive_filename = self.get_archive_filename_for_file(filename);

        if archive_filename.is_empty() {
            return None;
        }

        let archive = self.archive_file_map.get(&archive_filename)?;
        archive.get_file_info(filename)
    }

    /// Get the archive filename that contains the specified file
    /// Matches the C++ ArchiveFileSystem::getArchiveFilenameForFile implementation
    pub fn get_archive_filename_for_file(&self, filename: &str) -> String {
        let mut path = filename.to_lowercase();
        let mut dir_info = &self.root_directory;

        loop {
            if let Some(pos) = path.find(|c| c == '\\' || c == '/') {
                let token = path[..pos].to_string();
                path = path[(pos + 1)..].to_string();

                if token.is_empty() {
                    continue;
                }

                let has_dot = token.contains('.');
                let path_has_dot = path.contains('.');

                if has_dot && !path_has_dot {
                    // This is the filename
                    return dir_info.files.get(&token).cloned().unwrap_or_default();
                }

                match dir_info.directories.get(&token) {
                    Some(next_dir) => dir_info = next_dir,
                    None => return String::new(),
                }
            } else {
                return dir_info.files.get(&path).cloned().unwrap_or_default();
            }
        }
    }

    /// Get list of files in a directory across all archives
    /// Matches the C++ ArchiveFileSystem::getFileListInDirectory implementation
    pub fn get_file_list_in_directory(
        &self,
        current_directory: &str,
        original_directory: &str,
        search_name: &str,
        search_subdirectories: bool,
    ) -> FilenameList {
        let mut filename_list = FilenameList::new();

        for (_, archive) in &self.archive_file_map {
            let files = archive.get_file_list_in_directory(
                current_directory,
                original_directory,
                search_name,
                search_subdirectories,
            );
            filename_list.extend(files);
        }

        filename_list
    }

    /// Load BIG files from a directory
    /// Matches the C++ Win32BIGFileSystem::loadBigFilesFromDirectory implementation
    pub fn load_big_files_from_directory(
        &mut self,
        dir: &str,
        file_mask: &str,
        overwrite: bool,
    ) -> io::Result<bool> {
        let search_dir = if dir.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(dir)
        };

        if !search_dir.exists() || !search_dir.is_dir() {
            return Ok(false);
        }

        let mut actually_added = false;

        // Convert file mask to extension filter
        let extension = if file_mask.starts_with("*.") {
            Some(&file_mask[2..])
        } else {
            None
        };

        // Recursively search for .big files
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file() {
                    let should_load = if let Some(ext) = extension {
                        path.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case(ext))
                            .unwrap_or(false)
                    } else {
                        true
                    };

                    if should_load {
                        match open_big_archive(&path) {
                            Ok(archive) => {
                                let archive_filename =
                                    path.to_str().unwrap_or("unknown").to_lowercase();

                                self.load_into_directory_tree(
                                    &archive,
                                    &archive_filename,
                                    overwrite,
                                );
                                self.archive_file_map.insert(archive_filename, archive);
                                actually_added = true;
                            }
                            Err(_e) => {
                                // Skip files that can't be loaded
                            }
                        }
                    }
                } else if path.is_dir() {
                    // Recursively search subdirectories
                    if let Some(subdir) = path.to_str() {
                        let _ = self.load_big_files_from_directory(subdir, file_mask, overwrite);
                    }
                }
            }
        }

        Ok(actually_added)
    }

    /// Load an archive file into the global directory tree
    /// Matches the C++ ArchiveFileSystem::loadIntoDirectoryTree implementation
    fn load_into_directory_tree(
        &mut self,
        archive_file: &ArchiveFile,
        archive_filename: &str,
        overwrite: bool,
    ) {
        let filename_list = archive_file.get_file_list_in_directory("", "", "*", true);

        for filename in &filename_list {
            let mut path = filename.to_lowercase();
            let mut dir_info = &mut self.root_directory;

            // Navigate/create directory structure
            loop {
                if let Some(pos) = path.find(|c| c == '\\' || c == '/') {
                    let token = path[..pos].to_string();
                    path = path[(pos + 1)..].to_string();

                    if token.is_empty() {
                        continue;
                    }

                    let has_dot = token.contains('.');
                    let path_has_dot = path.contains('.');

                    if has_dot && !path_has_dot {
                        // This is the filename
                        if !dir_info.files.contains_key(&token) || overwrite {
                            dir_info.files.insert(token, archive_filename.to_string());
                        }
                        break;
                    }

                    // Create or navigate to subdirectory
                    dir_info
                        .directories
                        .entry(token.clone())
                        .or_insert_with(|| {
                            let mut new_dir = ArchivedDirectoryInfo::new();
                            new_dir.directory_name = token.clone();
                            new_dir
                        });

                    dir_info = dir_info.directories.get_mut(&token).unwrap();
                } else {
                    // Last token is the filename
                    if !dir_info.files.contains_key(&path) || overwrite {
                        dir_info
                            .files
                            .insert(path.clone(), archive_filename.to_string());
                    }
                    break;
                }
            }
        }
    }
}

impl Default for ArchiveFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_file_system_creation() {
        let afs = ArchiveFileSystem::new();
        assert_eq!(afs.archive_file_map.len(), 0);
    }

    #[test]
    fn test_does_file_exist_empty() {
        let afs = ArchiveFileSystem::new();
        assert!(!afs.does_file_exist("test.txt"));
    }
}
