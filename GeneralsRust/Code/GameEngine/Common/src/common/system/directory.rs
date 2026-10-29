////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Directory System Implementation
//!
//! Provides directory and file system operations for the game engine.
//! Handles path manipulation, directory traversal, and file enumeration.
//!
//! Rust conversion: 2025

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::common::ascii_string::AsciiString;

/// File information structure
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: AsciiString,
    pub path: PathBuf,
    pub size: u64,
    pub modified_time: SystemTime,
    pub is_directory: bool,
    pub is_readable: bool,
    pub is_writable: bool,
}

impl FileInfo {
    /// Create FileInfo from a path
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "directory entry name is not valid UTF-8",
                )
            })?
            .to_string();
        let metadata = fs::metadata(path)?;

        Ok(Self {
            name: AsciiString::from(&name),
            path: path.to_path_buf(),
            size: metadata.len(),
            modified_time: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            is_directory: metadata.is_dir(),
            is_readable: path_is_readable(path, &metadata),
            is_writable: !metadata.permissions().readonly(),
        })
    }

    /// Get file extension
    pub fn get_extension(&self) -> AsciiString {
        self.path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| AsciiString::from(s))
            .unwrap_or_else(|| AsciiString::new())
    }

    /// Check if file matches pattern
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        let name = self.name.as_str();
        match_pattern(name, pattern)
    }
}

/// Conservatively check this process's access to a regular file or directory.
/// A mode-000 entry is treated as unreadable even for a privileged process.
fn path_is_readable(path: &Path, metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Require both a granted permission bit and successful access by this
        // process. The first check keeps mode-000 entries unreadable even when
        // the test process is privileged; the open/read_dir probe observes the
        // effective owner class, ACLs, and other access-control decisions.
        if metadata.permissions().mode() & 0o444 == 0 {
            return false;
        }
        if metadata.is_dir() {
            fs::read_dir(path).is_ok()
        } else if metadata.file_type().is_file() {
            fs::File::open(path).is_ok()
        } else {
            // Never open FIFOs/devices/sockets just to infer readability; an
            // open may block or have side effects, so report conservatively.
            false
        }
    }
    #[cfg(not(unix))]
    {
        if metadata.is_dir() {
            fs::read_dir(path).is_ok()
        } else if metadata.file_type().is_file() {
            fs::File::open(path).is_ok()
        } else {
            false
        }
    }
}

/// Non-recursive snapshot matching the C++ `Directory` file/subdirectory
/// partition without changing the process working directory.
#[derive(Debug, Default)]
pub struct DirectorySnapshot {
    /// Non-directory entries, sorted by filename like C++ `FileInfoSet`.
    pub files: Vec<FileInfo>,
    /// Directory entries, sorted by filename like C++ `FileInfoSet`.
    pub subdirectories: Vec<FileInfo>,
}

impl DirectorySnapshot {
    /// Enumerate one directory level. Paths are resolved relative to the
    /// supplied path and the process current directory is never changed.
    /// Invalid UTF-8 names and metadata failures (including dangling symlinks,
    /// because metadata follows symlinks) fail the whole snapshot with an I/O
    /// error instead of silently omitting or misnaming an entry.
    pub fn read(path: &Path) -> io::Result<Self> {
        let mut snapshot = Self::default();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let info = FileInfo::from_path(&entry.path())?;
            if info.is_directory {
                snapshot.subdirectories.push(info);
            } else {
                snapshot.files.push(info);
            }
        }
        snapshot
            .files
            .sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
        snapshot
            .subdirectories
            .sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
        Ok(snapshot)
    }
}

/// Directory iterator for traversing file systems
pub struct DirectoryIterator {
    entries: Vec<FileInfo>,
    current_index: usize,
    _recursive: bool,
}

impl DirectoryIterator {
    /// Create a new directory iterator
    pub fn new(path: &Path, recursive: bool) -> io::Result<Self> {
        let mut entries = Vec::new();
        Self::collect_entries(path, recursive, &mut entries)?;

        Ok(Self {
            entries,
            current_index: 0,
            _recursive: recursive,
        })
    }

    /// Create iterator with pattern filter
    pub fn new_with_pattern(path: &Path, pattern: &str, recursive: bool) -> io::Result<Self> {
        let mut entries = Vec::new();
        Self::collect_entries_with_pattern(path, pattern, recursive, &mut entries)?;

        Ok(Self {
            entries,
            current_index: 0,
            _recursive: recursive,
        })
    }

    fn collect_entries(
        path: &Path,
        recursive: bool,
        entries: &mut Vec<FileInfo>,
    ) -> io::Result<()> {
        if !path.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_info = FileInfo::from_path(&entry.path())?;

            if file_info.is_directory && recursive {
                Self::collect_entries(&entry.path(), recursive, entries)?;
            }

            entries.push(file_info);
        }

        Ok(())
    }

    fn collect_entries_with_pattern(
        path: &Path,
        pattern: &str,
        recursive: bool,
        entries: &mut Vec<FileInfo>,
    ) -> io::Result<()> {
        if !path.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_info = FileInfo::from_path(&entry.path())?;

            if file_info.is_directory && recursive {
                Self::collect_entries_with_pattern(&entry.path(), pattern, recursive, entries)?;
            }

            if !file_info.is_directory && file_info.matches_pattern(pattern) {
                entries.push(file_info);
            } else if file_info.is_directory {
                entries.push(file_info);
            }
        }

        Ok(())
    }

    /// Get next file info
    pub fn next(&mut self) -> Option<&FileInfo> {
        if self.current_index < self.entries.len() {
            let result = &self.entries[self.current_index];
            self.current_index += 1;
            Some(result)
        } else {
            None
        }
    }

    /// Reset iterator to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    /// Get total number of entries
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Get all entries
    pub fn get_all_entries(&self) -> &[FileInfo] {
        &self.entries
    }
}

/// Directory utility functions
pub struct DirectoryUtils;

impl DirectoryUtils {
    /// Check if directory exists
    pub fn exists(path: &str) -> bool {
        Path::new(path).exists() && Path::new(path).is_dir()
    }

    /// Create directory (including parent directories)
    pub fn create_directory(path: &str) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    /// Remove directory and all contents
    pub fn remove_directory(path: &str) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    /// Get current working directory
    pub fn get_current_directory() -> io::Result<PathBuf> {
        std::env::current_dir()
    }

    /// Set current working directory
    pub fn set_current_directory(path: &str) -> io::Result<()> {
        std::env::set_current_dir(path)
    }

    /// Get directory size (recursive)
    pub fn get_directory_size(path: &str) -> io::Result<u64> {
        let mut total_size = 0;
        let iterator = DirectoryIterator::new(Path::new(path), true)?;

        for entry in iterator.get_all_entries() {
            if !entry.is_directory {
                total_size += entry.size;
            }
        }

        Ok(total_size)
    }

    /// Copy directory and all contents
    pub fn copy_directory(source: &str, destination: &str) -> io::Result<()> {
        let src_path = Path::new(source);
        let dst_path = Path::new(destination);

        if !src_path.exists() || !src_path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Source directory not found",
            ));
        }

        fs::create_dir_all(dst_path)?;

        let iterator = DirectoryIterator::new(src_path, true)?;
        for entry in iterator.get_all_entries() {
            let relative_path = entry
                .path
                .strip_prefix(src_path)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to get relative path"))?;
            let dest_path = dst_path.join(relative_path);

            if entry.is_directory {
                fs::create_dir_all(&dest_path)?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&entry.path, &dest_path)?;
            }
        }

        Ok(())
    }

    /// Find files matching pattern
    pub fn find_files(
        directory: &str,
        pattern: &str,
        recursive: bool,
    ) -> io::Result<Vec<FileInfo>> {
        let iterator =
            DirectoryIterator::new_with_pattern(Path::new(directory), pattern, recursive)?;
        Ok(iterator
            .get_all_entries()
            .iter()
            .filter(|entry| !entry.is_directory)
            .cloned()
            .collect())
    }

    /// Get subdirectories
    pub fn get_subdirectories(directory: &str) -> io::Result<Vec<FileInfo>> {
        let iterator = DirectoryIterator::new(Path::new(directory), false)?;
        Ok(iterator
            .get_all_entries()
            .iter()
            .filter(|entry| entry.is_directory)
            .cloned()
            .collect())
    }

    /// Clean up temporary files
    pub fn cleanup_temp_files(directory: &str, max_age_seconds: u64) -> io::Result<usize> {
        let now = SystemTime::now();
        let max_age = std::time::Duration::from_secs(max_age_seconds);
        let mut removed_count = 0;

        let iterator = DirectoryIterator::new(Path::new(directory), true)?;
        for entry in iterator.get_all_entries() {
            if !entry.is_directory {
                if let Ok(age) = now.duration_since(entry.modified_time) {
                    if age > max_age {
                        if fs::remove_file(&entry.path).is_ok() {
                            removed_count += 1;
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }
}

/// Path utility functions
pub struct PathUtils;

impl PathUtils {
    /// Join path components
    pub fn join(base: &str, component: &str) -> AsciiString {
        let path = Path::new(base).join(component);
        AsciiString::from(path.to_string_lossy().as_ref())
    }

    /// Get parent directory
    pub fn get_parent(path: &str) -> Option<AsciiString> {
        Path::new(path)
            .parent()
            .map(|p| AsciiString::from(p.to_string_lossy().as_ref()))
    }

    /// Get filename from path
    pub fn get_filename(path: &str) -> AsciiString {
        Path::new(path)
            .file_name()
            .map(|name| AsciiString::from(name.to_string_lossy().as_ref()))
            .unwrap_or_else(|| AsciiString::new())
    }

    /// Get file extension
    pub fn get_extension(path: &str) -> AsciiString {
        Path::new(path)
            .extension()
            .map(|ext| AsciiString::from(ext.to_string_lossy().as_ref()))
            .unwrap_or_else(|| AsciiString::new())
    }

    /// Remove extension from path
    pub fn remove_extension(path: &str) -> AsciiString {
        let path_obj = Path::new(path);
        let without_ext = path_obj.with_extension("");
        AsciiString::from(without_ext.to_string_lossy().as_ref())
    }

    /// Check if path is absolute
    pub fn is_absolute(path: &str) -> bool {
        Path::new(path).is_absolute()
    }

    /// Convert to absolute path
    pub fn to_absolute(path: &str) -> io::Result<AsciiString> {
        let abs_path = fs::canonicalize(path)?;
        Ok(AsciiString::from(abs_path.to_string_lossy().as_ref()))
    }

    /// Normalize path separators
    pub fn normalize_separators(path: &str) -> AsciiString {
        let normalized = path.replace('\\', "/");
        AsciiString::from(&normalized)
    }
}

/// Simple wildcard pattern matching
fn match_pattern(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    match_pattern_recursive(&text_chars, &pattern_chars, 0, 0)
}

fn match_pattern_recursive(
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
            if match_pattern_recursive(text, pattern, text_idx, pattern_idx + 1) {
                return true;
            }

            // Try matching one or more characters
            for i in text_idx..text.len() {
                if match_pattern_recursive(text, pattern, i + 1, pattern_idx + 1) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // Match any single character
            match_pattern_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
        }
        c if c == text[text_idx] => {
            // Exact character match
            match_pattern_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
        }
        _ => false,
    }
}

/// Directory watcher for monitoring file system changes
pub struct DirectoryWatcher {
    watched_paths: HashMap<PathBuf, SystemTime>,
}

impl DirectoryWatcher {
    pub fn new() -> Self {
        Self {
            watched_paths: HashMap::new(),
        }
    }

    pub fn add_watch(&mut self, path: &Path) -> io::Result<()> {
        let metadata = fs::metadata(path)?;
        let modified_time = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        self.watched_paths.insert(path.to_path_buf(), modified_time);
        Ok(())
    }

    pub fn remove_watch(&mut self, path: &Path) {
        self.watched_paths.remove(path);
    }

    pub fn check_changes(&mut self) -> io::Result<Vec<PathBuf>> {
        let mut changed_paths = Vec::new();

        for (path, last_modified) in &mut self.watched_paths {
            if let Ok(metadata) = fs::metadata(path) {
                let current_modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                if current_modified > *last_modified {
                    changed_paths.push(path.clone());
                    *last_modified = current_modified;
                }
            }
        }

        Ok(changed_paths)
    }
}

impl Default for DirectoryWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_pattern_matching() {
        assert!(match_pattern("test.txt", "*.txt"));
        assert!(match_pattern("test.txt", "test.*"));
        assert!(match_pattern("test.txt", "t?st.txt"));
        assert!(!match_pattern("test.txt", "*.exe"));
        assert!(match_pattern("anything", "*"));
        assert!(match_pattern("a", "?"));
        assert!(!match_pattern("ab", "?"));
    }

    #[test]
    fn test_path_utils() {
        let joined = PathUtils::join("/home/user", "documents");
        assert!(joined.contains("documents"));

        let filename = PathUtils::get_filename("/home/user/test.txt");
        assert_eq!(filename.as_str(), "test.txt");

        let ext = PathUtils::get_extension("test.txt");
        assert_eq!(ext.as_str(), "txt");

        let without_ext = PathUtils::remove_extension("test.txt");
        assert_eq!(without_ext.as_str(), "test");
    }

    #[test]
    fn test_directory_utils() {
        assert!(DirectoryUtils::exists("."));

        // Test current directory operations
        let current = DirectoryUtils::get_current_directory().unwrap();
        assert!(current.exists());
    }

    #[test]
    fn test_directory_iterator() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create some test files
        File::create(temp_path.join("test1.txt")).unwrap();
        File::create(temp_path.join("test2.log")).unwrap();

        let iterator = DirectoryIterator::new(temp_path, false).unwrap();
        assert!(iterator.count() >= 2);

        // Test pattern matching
        let iterator = DirectoryIterator::new_with_pattern(temp_path, "*.txt", false).unwrap();
        let txt_files: Vec<_> = iterator
            .get_all_entries()
            .iter()
            .filter(|e| !e.is_directory)
            .collect();
        assert_eq!(txt_files.len(), 1);
    }

    #[test]
    fn directory_snapshot_partitions_entries_without_changing_cwd() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        fs::create_dir(root.join("z-subdir")).unwrap();
        fs::create_dir(root.join("a-subdir")).unwrap();
        File::create(root.join("z-file.txt")).unwrap();
        let unreadable_path = root.join("a-unreadable.txt");
        File::create(&unreadable_path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&unreadable_path, fs::Permissions::from_mode(0o000)).unwrap();
        }

        let cwd_before = std::env::current_dir().unwrap();
        let snapshot = DirectorySnapshot::read(root).unwrap();
        let cwd_after = std::env::current_dir().unwrap();

        assert_eq!(cwd_before, cwd_after);
        assert_eq!(
            snapshot
                .files
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["a-unreadable.txt", "z-file.txt"]
        );
        assert_eq!(
            snapshot
                .subdirectories
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["a-subdir", "z-subdir"]
        );
        assert!(
            !snapshot
                .files
                .iter()
                .any(|entry| entry.name.as_str() == ".")
        );
        assert!(
            !snapshot
                .files
                .iter()
                .any(|entry| entry.name.as_str() == "..")
        );
        assert!(
            !snapshot
                .subdirectories
                .iter()
                .any(|entry| { entry.name.as_str() == "." || entry.name.as_str() == ".." })
        );
        #[cfg(unix)]
        assert!(
            !snapshot
                .files
                .iter()
                .find(|entry| entry.name.as_str() == "a-unreadable.txt")
                .unwrap()
                .is_readable
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_info_uses_effective_open_access_when_read_bits_exist() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("other-class-read-only.txt");
        File::create(&path).unwrap();
        // The owner has no read bit, while the other class does. For a
        // nonprivileged owner, effective access should still be denied.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o004)).unwrap();
        let can_open_as_this_process = fs::File::open(&path).is_ok();
        let info = FileInfo::from_path(&path).unwrap();
        assert_eq!(info.is_readable, can_open_as_this_process);
        if !can_open_as_this_process {
            assert_ne!(fs::metadata(&path).unwrap().permissions().mode() & 0o444, 0);
            assert!(!info.is_readable);
        }
    }

    #[cfg(unix)]
    #[test]
    fn file_info_rejects_non_utf8_names() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let name = OsStr::from_bytes(b"invalid-\xff-name");
        // Some filesystems (including the macOS test filesystem) reject such
        // names at creation. Test FileInfo's explicit failure policy directly
        // so the behavior remains covered on both permissive and strict FSes.
        let error = FileInfo::from_path(Path::new(name)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn directory_snapshot_propagates_dangling_symlink_metadata_failure() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        symlink(
            temp_dir.path().join("missing-target"),
            temp_dir.path().join("dangling-link"),
        )
        .unwrap();
        let error = DirectorySnapshot::read(temp_dir.path()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_directory_watcher() {
        let mut watcher = DirectoryWatcher::new();
        let temp_dir = TempDir::new().unwrap();

        watcher.add_watch(temp_dir.path()).unwrap();
        let changes = watcher.check_changes().unwrap();
        assert_eq!(changes.len(), 0); // No changes initially
    }
}
