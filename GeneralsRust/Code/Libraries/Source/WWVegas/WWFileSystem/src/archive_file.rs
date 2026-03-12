use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::archived_file_info::ArchivedFileInfo;
use crate::directory_info::DetailedArchivedDirectoryInfo;
use crate::file_info::FileInfo;
use crate::search_string::search_string_matches;

/// Case-insensitive string set for filenames
pub type FilenameList = BTreeSet<String>;

/// Trait for archive file operations
/// Matches the C++ ArchiveFile interface
pub trait ArchiveFileTrait {
    fn get_file_info(&self, filename: &str) -> Option<FileInfo>;
    fn open_file(&mut self, filename: &str, access: i32) -> io::Result<Vec<u8>>;
    fn close_all_files(&mut self);
    fn get_name(&self) -> &str;
    fn get_path(&self) -> &str;
    fn set_search_priority(&mut self, new_priority: i32);
    fn close(&mut self);
    fn get_file_list_in_directory(
        &self,
        current_directory: &str,
        original_directory: &str,
        search_name: &str,
        search_subdirectories: bool,
    ) -> FilenameList;
}

/// ArchiveFile implementation matching the C++ ArchiveFile class
pub struct ArchiveFile {
    file: Option<File>,
    root_directory: DetailedArchivedDirectoryInfo,
    name: String,
    path: String,
}

impl ArchiveFile {
    pub fn new() -> Self {
        Self {
            file: None,
            root_directory: DetailedArchivedDirectoryInfo::new(),
            name: String::new(),
            path: String::new(),
        }
    }

    /// Attach a file handle to this archive
    pub fn attach_file(&mut self, file: File, name: String, path: String) {
        self.file = Some(file);
        self.name = name;
        self.path = path;
    }

    /// Add a file to the directory tree
    /// Matches the C++ ArchiveFile::addFile implementation
    pub fn add_file(&mut self, path: &str, file_info: &ArchivedFileInfo) {
        let mut temp = path.to_lowercase();
        let mut dir_info = &mut self.root_directory;

        // Tokenize the path by backslash or forward slash
        loop {
            if let Some(pos) = temp.find(|c| c == '\\' || c == '/') {
                let token = temp[..pos].to_string();
                temp = temp[(pos + 1)..].to_string();

                if token.is_empty() {
                    continue;
                }

                // Create directory entry if it doesn't exist
                dir_info
                    .directories
                    .entry(token.clone())
                    .or_insert_with(|| {
                        let mut new_dir = DetailedArchivedDirectoryInfo::new();
                        new_dir.directory_name = token.clone();
                        new_dir
                    });

                dir_info = dir_info.directories.get_mut(&token).unwrap();
            } else {
                break;
            }
        }

        // Add the file to the final directory
        dir_info
            .files
            .insert(file_info.filename.clone(), file_info.clone());
    }

    /// Get archived file info from the directory tree
    /// Matches the C++ ArchiveFile::getArchivedFileInfo implementation
    pub fn get_archived_file_info(&self, filename: &str) -> Option<&ArchivedFileInfo> {
        let mut path = filename.to_lowercase();
        let mut dir_info = &self.root_directory;

        // Navigate through directories
        loop {
            if let Some(pos) = path.find(|c| c == '\\' || c == '/') {
                let token = path[..pos].to_string();
                path = path[(pos + 1)..].to_string();

                // Check if this is a filename (has a dot and no more path separators)
                let has_dot = token.contains('.');
                let path_has_dot = path.contains('.');

                if has_dot && !path_has_dot {
                    // This is the filename
                    return dir_info.files.get(&token);
                }

                // Navigate to subdirectory
                dir_info = dir_info.directories.get(&token)?;
            } else {
                // Last token is the filename
                return dir_info.files.get(&path);
            }
        }
    }

    /// Get file list in a directory
    /// Matches the C++ ArchiveFile::getFileListInDirectory implementation
    pub fn get_file_list_in_directory_internal(
        dir_info: &DetailedArchivedDirectoryInfo,
        current_directory: &str,
        search_name: &str,
        search_subdirectories: bool,
    ) -> FilenameList {
        let mut filename_list = FilenameList::new();

        // Recursively search subdirectories if requested
        if search_subdirectories {
            for (_, subdir) in &dir_info.directories {
                let mut temp_dirname = current_directory.to_string();
                if !temp_dirname.is_empty() && !temp_dirname.ends_with('\\') {
                    temp_dirname.push('\\');
                }
                temp_dirname.push_str(&subdir.directory_name);

                let subdir_files = Self::get_file_list_in_directory_internal(
                    subdir,
                    &temp_dirname,
                    search_name,
                    search_subdirectories,
                );
                filename_list.extend(subdir_files);
            }
        }

        // Add matching files from current directory
        for (_, file_info) in &dir_info.files {
            if search_string_matches(&file_info.filename, search_name) {
                let mut temp_filename = current_directory.to_string();
                if !temp_filename.is_empty() && !temp_filename.ends_with('\\') {
                    temp_filename.push('\\');
                }
                temp_filename.push_str(&file_info.filename);
                filename_list.insert(temp_filename);
            }
        }

        filename_list
    }

    /// Navigate to a directory in the tree
    fn navigate_to_directory(&self, directory: &str) -> Option<&DetailedArchivedDirectoryInfo> {
        if directory.is_empty() {
            return Some(&self.root_directory);
        }

        let mut search_dir = directory.to_lowercase();
        let mut dir_info = &self.root_directory;

        loop {
            if let Some(pos) = search_dir.find(|c| c == '\\' || c == '/') {
                let token = search_dir[..pos].to_string();
                search_dir = search_dir[(pos + 1)..].to_string();

                if token.is_empty() {
                    continue;
                }

                dir_info = dir_info.directories.get(&token)?;
            } else {
                if !search_dir.is_empty() {
                    dir_info = dir_info.directories.get(&search_dir)?;
                }
                break;
            }
        }

        Some(dir_info)
    }

    /// Read file data from archive
    pub fn read_file_data(&mut self, file_info: &ArchivedFileInfo) -> io::Result<Vec<u8>> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Archive file not attached"))?;

        file.seek(SeekFrom::Start(file_info.offset as u64))?;

        let mut buffer = vec![0u8; file_info.size as usize];
        file.read_exact(&mut buffer)?;

        Ok(buffer)
    }
}

impl Default for ArchiveFile {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveFileTrait for ArchiveFile {
    fn get_file_info(&self, filename: &str) -> Option<FileInfo> {
        let file_info = self.get_archived_file_info(filename)?;

        Some(FileInfo {
            size_high: 0,
            size_low: file_info.size as i32,
            timestamp_high: 0,
            timestamp_low: 0,
        })
    }

    fn open_file(&mut self, filename: &str, _access: i32) -> io::Result<Vec<u8>> {
        let file_info = self
            .get_archived_file_info(filename)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("File '{}' not found in archive", filename),
                )
            })?
            .clone();

        self.read_file_data(&file_info)
    }

    fn close_all_files(&mut self) {
        // In the C++ version this was a no-op
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_path(&self) -> &str {
        &self.path
    }

    fn set_search_priority(&mut self, _new_priority: i32) {
        // In the C++ version this was a no-op
    }

    fn close(&mut self) {
        self.file = None;
    }

    fn get_file_list_in_directory(
        &self,
        _current_directory: &str,
        original_directory: &str,
        search_name: &str,
        search_subdirectories: bool,
    ) -> FilenameList {
        if let Some(dir_info) = self.navigate_to_directory(original_directory) {
            Self::get_file_list_in_directory_internal(
                dir_info,
                original_directory,
                search_name,
                search_subdirectories,
            )
        } else {
            FilenameList::new()
        }
    }
}

/// Open a BIG archive file from disk
/// Matches the C++ Win32BIGFileSystem::openArchiveFile implementation
pub fn open_big_archive<P: AsRef<Path>>(path: P) -> io::Result<ArchiveFile> {
    use crate::big_file_parser::BigFileParser;

    let path_ref = path.as_ref();
    let mut file = File::open(path_ref)?;

    let archive_filename = path_ref.to_str().unwrap_or("unknown").to_lowercase();

    let file_infos = BigFileParser::parse_big_file(&mut file, &archive_filename)?;

    let mut archive = ArchiveFile::new();

    for (dir_path, file_info) in file_infos {
        archive.add_file(&dir_path, &file_info);
    }

    let name = path_ref
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    archive.attach_file(file, name, archive_filename);

    Ok(archive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_file_and_retrieve() {
        let mut archive = ArchiveFile::new();

        let file_info = ArchivedFileInfo {
            filename: "test.txt".to_string(),
            archive_filename: "test.big".to_string(),
            offset: 0x1000,
            size: 0x500,
        };

        archive.add_file("art\\textures\\", &file_info);

        let retrieved = archive.get_archived_file_info("art/textures/test.txt");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename, "test.txt");
    }

    #[test]
    fn test_navigate_to_directory() {
        let mut archive = ArchiveFile::new();

        let file_info = ArchivedFileInfo {
            filename: "test.txt".to_string(),
            archive_filename: "test.big".to_string(),
            offset: 0x1000,
            size: 0x500,
        };

        archive.add_file("art\\textures\\", &file_info);

        let dir = archive.navigate_to_directory("art\\textures");
        assert!(dir.is_some());
        assert!(dir.unwrap().files.contains_key("test.txt"));
    }
}
