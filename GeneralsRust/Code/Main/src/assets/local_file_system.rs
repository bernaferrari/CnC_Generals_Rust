use anyhow::{anyhow, Result};
use log::info;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Local file system for accessing files on disk
pub struct LocalFileSystem {
    base_path: PathBuf,
    case_sensitive: bool,
}

impl LocalFileSystem {
    /// Create new local file system
    pub fn new() -> Self {
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            base_path,
            case_sensitive: cfg!(unix), // Unix systems are typically case-sensitive
        }
    }

    /// Set base path for file operations
    pub fn set_base_path<P: AsRef<Path>>(&mut self, path: P) {
        self.base_path = path.as_ref().to_path_buf();
        info!("LocalFileSystem base path set to: {:?}", self.base_path);
    }

    /// Get base path
    pub fn get_base_path(&self) -> &Path {
        &self.base_path
    }

    /// Check if file exists
    pub fn does_file_exist<P: AsRef<Path>>(&self, path: P) -> bool {
        let full_path = self.resolve_path(path.as_ref());
        full_path.exists() && full_path.is_file()
    }

    /// Check if directory exists
    pub fn does_directory_exist<P: AsRef<Path>>(&self, path: P) -> bool {
        let full_path = self.resolve_path(path.as_ref());
        full_path.exists() && full_path.is_dir()
    }

    /// Open file for reading
    pub fn open_file_read<P: AsRef<Path>>(&self, path: P) -> Result<File> {
        let full_path = self.resolve_path(path.as_ref());

        File::open(&full_path).map_err(|e| anyhow!("Failed to open file {:?}: {}", full_path, e))
    }

    /// Open file for writing
    pub fn open_file_write<P: AsRef<Path>>(
        &self,
        path: P,
        create: bool,
        truncate: bool,
    ) -> Result<File> {
        let full_path = self.resolve_path(path.as_ref());

        // Create parent directories if necessary
        if let Some(parent) = full_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    anyhow!("Failed to create directories for {:?}: {}", full_path, e)
                })?;
            }
        }

        let mut options = OpenOptions::new();
        options.write(true);

        if create {
            options.create(true);
        }
        if truncate {
            options.truncate(true);
        }

        options
            .open(&full_path)
            .map_err(|e| anyhow!("Failed to open file for write {:?}: {}", full_path, e))
    }

    /// Read entire file to bytes
    pub fn read_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>> {
        let mut file = self.open_file_read(path)?;
        let mut buffer = Vec::new();

        file.read_to_end(&mut buffer)
            .map_err(|e| anyhow!("Failed to read file: {}", e))?;

        Ok(buffer)
    }

    /// Read entire file to string
    pub fn read_file_text<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let bytes = self.read_file(path)?;
        String::from_utf8(bytes).map_err(|e| anyhow!("File is not valid UTF-8: {}", e))
    }

    /// Write bytes to file
    pub fn write_file<P: AsRef<Path>>(&self, path: P, data: &[u8]) -> Result<()> {
        let mut file = self.open_file_write(path, true, true)?;
        file.write_all(data)
            .map_err(|e| anyhow!("Failed to write file: {}", e))
    }

    /// Write string to file
    pub fn write_file_text<P: AsRef<Path>>(&self, path: P, text: &str) -> Result<()> {
        self.write_file(path, text.as_bytes())
    }

    /// Get file size
    pub fn get_file_size<P: AsRef<Path>>(&self, path: P) -> Result<u64> {
        let full_path = self.resolve_path(path.as_ref());
        let metadata = std::fs::metadata(&full_path)
            .map_err(|e| anyhow!("Failed to get file metadata for {:?}: {}", full_path, e))?;
        Ok(metadata.len())
    }

    /// Get file modification time
    pub fn get_file_modified_time<P: AsRef<Path>>(&self, path: P) -> Result<std::time::SystemTime> {
        let full_path = self.resolve_path(path.as_ref());
        let metadata = std::fs::metadata(&full_path)
            .map_err(|e| anyhow!("Failed to get file metadata for {:?}: {}", full_path, e))?;
        metadata
            .modified()
            .map_err(|e| anyhow!("Failed to get modification time: {}", e))
    }

    /// List files in directory
    pub fn list_files_in_directory<P: AsRef<Path>>(
        &self,
        path: P,
        pattern: Option<&str>,
    ) -> Result<Vec<PathBuf>> {
        let full_path = self.resolve_path(path.as_ref());

        if !full_path.exists() {
            return Err(anyhow!("Directory does not exist: {:?}", full_path));
        }

        if !full_path.is_dir() {
            return Err(anyhow!("Path is not a directory: {:?}", full_path));
        }

        let mut files = Vec::new();

        for entry in std::fs::read_dir(&full_path)
            .map_err(|e| anyhow!("Failed to read directory {:?}: {}", full_path, e))?
        {
            let entry = entry.map_err(|e| anyhow!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Apply pattern filter if specified
                if let Some(pattern) = pattern {
                    if !self.matches_pattern(file_name, pattern) {
                        continue;
                    }
                }

                files.push(path);
            }
        }

        files.sort();
        Ok(files)
    }

    /// List directories in directory
    pub fn list_directories<P: AsRef<Path>>(&self, path: P) -> Result<Vec<PathBuf>> {
        let full_path = self.resolve_path(path.as_ref());

        let mut directories = Vec::new();

        for entry in std::fs::read_dir(&full_path)
            .map_err(|e| anyhow!("Failed to read directory {:?}: {}", full_path, e))?
        {
            let entry = entry.map_err(|e| anyhow!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                directories.push(path);
            }
        }

        directories.sort();
        Ok(directories)
    }

    /// Create directory (and parents if needed)
    pub fn create_directory<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let full_path = self.resolve_path(path.as_ref());

        std::fs::create_dir_all(&full_path)
            .map_err(|e| anyhow!("Failed to create directory {:?}: {}", full_path, e))
    }

    /// Delete file
    pub fn delete_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let full_path = self.resolve_path(path.as_ref());

        if !full_path.exists() {
            return Err(anyhow!("File does not exist: {:?}", full_path));
        }

        std::fs::remove_file(&full_path)
            .map_err(|e| anyhow!("Failed to delete file {:?}: {}", full_path, e))
    }

    /// Delete directory (recursive)
    pub fn delete_directory<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let full_path = self.resolve_path(path.as_ref());

        if !full_path.exists() {
            return Err(anyhow!("Directory does not exist: {:?}", full_path));
        }

        std::fs::remove_dir_all(&full_path)
            .map_err(|e| anyhow!("Failed to delete directory {:?}: {}", full_path, e))
    }

    /// Copy file
    pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> Result<()> {
        let from_path = self.resolve_path(from.as_ref());
        let to_path = self.resolve_path(to.as_ref());

        // Create destination directory if needed
        if let Some(parent) = to_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create destination directory: {}", e))?;
            }
        }

        std::fs::copy(&from_path, &to_path)
            .map_err(|e| anyhow!("Failed to copy {:?} to {:?}: {}", from_path, to_path, e))?;

        Ok(())
    }

    /// Move/rename file
    pub fn move_file<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> Result<()> {
        let from_path = self.resolve_path(from.as_ref());
        let to_path = self.resolve_path(to.as_ref());

        // Create destination directory if needed
        if let Some(parent) = to_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create destination directory: {}", e))?;
            }
        }

        std::fs::rename(&from_path, &to_path)
            .map_err(|e| anyhow!("Failed to move {:?} to {:?}: {}", from_path, to_path, e))
    }

    /// Read lines from text file
    pub fn read_lines<P: AsRef<Path>>(&self, path: P) -> Result<Vec<String>> {
        let file = self.open_file_read(path)?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| anyhow!("Failed to read line: {}", e))?;
            lines.push(line);
        }

        Ok(lines)
    }

    /// Write lines to text file
    pub fn write_lines<P: AsRef<Path>, I, S>(&self, path: P, lines: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut file = self.open_file_write(path, true, true)?;

        for line in lines {
            writeln!(file, "{}", line.as_ref())
                .map_err(|e| anyhow!("Failed to write line: {}", e))?;
        }

        Ok(())
    }

    /// Resolve relative path against base path
    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        }
    }

    /// Check if filename matches pattern (basic wildcard support)
    fn matches_pattern(&self, filename: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // Handle simple wildcard patterns like "*.txt"
        if pattern.starts_with("*.") {
            let extension = &pattern[2..];
            let filename_lower = if self.case_sensitive {
                filename.to_string()
            } else {
                filename.to_lowercase()
            };
            let extension_lower = if self.case_sensitive {
                extension.to_string()
            } else {
                extension.to_lowercase()
            };

            return filename_lower.ends_with(&format!(".{}", extension_lower));
        }

        // Exact match (case-sensitive on Unix, case-insensitive on Windows)
        if self.case_sensitive {
            filename == pattern
        } else {
            filename.to_lowercase() == pattern.to_lowercase()
        }
    }
}

/// File information structure
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: std::time::SystemTime,
    pub is_directory: bool,
}

impl LocalFileSystem {
    /// Get detailed file information
    pub fn get_file_info<P: AsRef<Path>>(&self, path: P) -> Result<FileInfo> {
        let full_path = self.resolve_path(path.as_ref());
        let metadata = std::fs::metadata(&full_path)
            .map_err(|e| anyhow!("Failed to get file metadata for {:?}: {}", full_path, e))?;

        Ok(FileInfo {
            path: full_path,
            size: metadata.len(),
            modified: metadata
                .modified()
                .map_err(|e| anyhow!("Failed to get modification time: {}", e))?,
            is_directory: metadata.is_dir(),
        })
    }

    /// Find files recursively
    pub fn find_files_recursive<P: AsRef<Path>>(
        &self,
        path: P,
        pattern: Option<&str>,
    ) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.find_files_recursive_impl(path.as_ref(), pattern, &mut files)?;
        files.sort();
        Ok(files)
    }

    fn find_files_recursive_impl(
        &self,
        path: &Path,
        pattern: Option<&str>,
        files: &mut Vec<PathBuf>,
    ) -> Result<()> {
        let full_path = self.resolve_path(path);

        if !full_path.exists() {
            return Ok(());
        }

        if full_path.is_file() {
            if let Some(filename) = full_path.file_name().and_then(|n| n.to_str()) {
                if pattern.map_or(true, |p| self.matches_pattern(filename, p)) {
                    files.push(full_path);
                }
            }
            return Ok(());
        }

        if full_path.is_dir() {
            for entry in std::fs::read_dir(&full_path)
                .map_err(|e| anyhow!("Failed to read directory {:?}: {}", full_path, e))?
            {
                let entry = entry.map_err(|e| anyhow!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();

                if entry_path.is_file() {
                    if let Some(filename) = entry_path.file_name().and_then(|n| n.to_str()) {
                        if pattern.map_or(true, |p| self.matches_pattern(filename, p)) {
                            files.push(entry_path);
                        }
                    }
                } else if entry_path.is_dir() {
                    // Recursively search subdirectories
                    let relative_path = entry_path
                        .strip_prefix(&self.base_path)
                        .unwrap_or(&entry_path);
                    self.find_files_recursive_impl(relative_path, pattern, files)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_file_operations() {
        let temp_dir = tempdir().unwrap();
        let mut local_fs = LocalFileSystem::new();
        local_fs.set_base_path(temp_dir.path());

        let test_file = "test.txt";
        let test_content = "Hello, World!";

        // Test writing
        assert!(local_fs.write_file_text(test_file, test_content).is_ok());
        assert!(local_fs.does_file_exist(test_file));

        // Test reading
        let read_content = local_fs.read_file_text(test_file).unwrap();
        assert_eq!(read_content, test_content);

        // Test file size
        let size = local_fs.get_file_size(test_file).unwrap();
        assert_eq!(size, test_content.len() as u64);

        // Test deletion
        assert!(local_fs.delete_file(test_file).is_ok());
        assert!(!local_fs.does_file_exist(test_file));
    }

    #[test]
    fn test_directory_operations() {
        let temp_dir = tempdir().unwrap();
        let mut local_fs = LocalFileSystem::new();
        local_fs.set_base_path(temp_dir.path());

        let test_dir = "test_directory";

        // Test creation
        assert!(local_fs.create_directory(test_dir).is_ok());
        assert!(local_fs.does_directory_exist(test_dir));

        // Test listing
        let dirs = local_fs.list_directories(".").unwrap();
        assert!(dirs.iter().any(|d| d.file_name().unwrap() == test_dir));
    }

    #[test]
    fn test_pattern_matching() {
        let local_fs = LocalFileSystem::new();

        // Test wildcard patterns
        assert!(local_fs.matches_pattern("test.txt", "*.txt"));
        assert!(local_fs.matches_pattern("document.doc", "*.doc"));
        assert!(!local_fs.matches_pattern("image.png", "*.txt"));

        // Test exact match
        assert!(local_fs.matches_pattern("exact.txt", "exact.txt"));
        assert!(!local_fs.matches_pattern("other.txt", "exact.txt"));
    }
}
