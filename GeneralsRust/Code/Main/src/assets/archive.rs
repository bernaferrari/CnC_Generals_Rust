////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! High-level archive facade built on top of the modernized core BIG loader.

use anyhow::{anyhow, Result};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::archive_file_system as core;
use log::warn;
use std::future::Future;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::task::JoinHandle;
use ww3d_renderer_3d::rendering::texture_system::ArchiveFileReader;

/// Unity wrapper around the core archive system.
pub struct ArchiveFileSystem {
    core: core::ArchiveFileSystem,
}

impl Default for ArchiveFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveFileSystem {
    /// Construct a new archive facade.
    pub fn new() -> Self {
        Self {
            core: core::ArchiveFileSystem::new(),
        }
    }

    fn add_default_search_paths(&mut self) {
        fn push_install_layout_paths(push_unique: &mut impl FnMut(PathBuf), root: &Path) {
            push_unique(root.join("assets"));

            // Zero Hour directories.
            push_unique(root.join("windows_game/Command & Conquer Generals Zero Hour"));
            push_unique(root.join("windows_game/Command & Conquer Generals Zero Hour/Data"));
            push_unique(root.join("windows_game/Command and Conquer Generals Zero Hour"));
            push_unique(root.join("windows_game/Command and Conquer Generals Zero Hour/Data"));
            push_unique(root.join("Command & Conquer Generals Zero Hour"));
            push_unique(root.join("Command & Conquer Generals Zero Hour/Data"));
            push_unique(root.join("Command and Conquer Generals Zero Hour"));
            push_unique(root.join("Command and Conquer Generals Zero Hour/Data"));

            // Combined installer layout observed in legacy installs.
            push_unique(root.join(
                "Command and Conquer Generals + Zero Hour/Command & Conquer Generals Zero Hour",
            ));
            push_unique(root.join(
                "Command and Conquer Generals + Zero Hour/Command & Conquer Generals Zero Hour/Data",
            ));
            push_unique(root.join(
                "Command and Conquer Generals + Zero Hour/Command and Conquer Generals Zero Hour",
            ));
            push_unique(root.join(
                "Command and Conquer Generals + Zero Hour/Command and Conquer Generals Zero Hour/Data",
            ));

            // Base Generals directories (needed by ZH in C++).
            push_unique(root.join("windows_game/Command & Conquer Generals"));
            push_unique(root.join("windows_game/Command & Conquer Generals/Data"));
            push_unique(root.join("windows_game/Command and Conquer Generals"));
            push_unique(root.join("windows_game/Command and Conquer Generals/Data"));
            push_unique(root.join("Command & Conquer Generals"));
            push_unique(root.join("Command & Conquer Generals/Data"));
            push_unique(root.join("Command and Conquer Generals"));
            push_unique(root.join("Command and Conquer Generals/Data"));
            push_unique(
                root.join("Command and Conquer Generals + Zero Hour/Command & Conquer Generals"),
            );
            push_unique(
                root.join(
                    "Command and Conquer Generals + Zero Hour/Command & Conquer Generals/Data",
                ),
            );
            push_unique(
                root.join("Command and Conquer Generals + Zero Hour/Command and Conquer Generals"),
            );
            push_unique(root.join(
                "Command and Conquer Generals + Zero Hour/Command and Conquer Generals/Data",
            ));
        }

        let mut root_candidates: Vec<PathBuf> = Vec::new();
        let mut direct_install_candidates: Vec<PathBuf> = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            root_candidates.push(cwd);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                root_candidates.push(parent.to_path_buf());
            }
        }
        root_candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

        if let Ok(from_env) = std::env::var("GENERALS_ASSETS_DIR") {
            let path = PathBuf::from(from_env);
            direct_install_candidates.push(path.clone());
            root_candidates.push(path);
        }
        if let Ok(from_env) = std::env::var("GENERALS_INSTALL_PATH") {
            let path = PathBuf::from(from_env);
            direct_install_candidates.push(path.clone());
            root_candidates.push(path);
        }
        if let Ok(from_env) = std::env::var("GENERALS_BASE_INSTALL_PATH") {
            let path = PathBuf::from(from_env);
            direct_install_candidates.push(path.clone());
            root_candidates.push(path);
        }

        let mut ordered = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push_unique = |path: PathBuf| {
            if !path.exists() {
                return;
            }
            let key = path.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                ordered.push(path);
            }
        };

        let home_dir = std::env::var("HOME").ok().map(PathBuf::from);

        for path in direct_install_candidates {
            push_unique(path);
        }

        for root in root_candidates {
            for ancestor in root.ancestors().take(8) {
                let ancestor = ancestor.to_path_buf();
                push_install_layout_paths(&mut push_unique, &ancestor);

                // Non-Windows parity substitute for registry install path lookup:
                // probe one directory level for sibling install bundles.
                let should_scan_siblings = home_dir
                    .as_ref()
                    .map_or(false, |home| ancestor.starts_with(home))
                    || ancestor.starts_with("/Users/Shared");
                if !should_scan_siblings {
                    continue;
                }

                let Ok(entries) = std::fs::read_dir(&ancestor) else {
                    continue;
                };
                for entry in entries.flatten().take(256) {
                    let child = entry.path();
                    if !child.is_dir() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                    if !name.contains("generals") {
                        continue;
                    }
                    if !(name.contains("zero hour") || name.contains("zh")) {
                        continue;
                    }
                    push_install_layout_paths(&mut push_unique, &child);
                }
            }
        }

        for path in ordered {
            self.core.add_search_path(path);
        }
    }

    /// Register an additional search path to be processed on the next init.
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        self.core.add_search_path(path);
    }

    /// Initialize the archive system (async for compatibility with existing call sites).
    pub async fn init(&mut self) -> Result<()> {
        self.add_default_search_paths();
        self.core.init().map_err(anyhow::Error::from)?;
        self.warn_if_base_archives_missing();
        Ok(())
    }

    fn warn_if_base_archives_missing(&self) {
        let loaded = self.core.get_loaded_big_files();
        let has_textures_big = loaded
            .iter()
            .map(|name| name.as_str().to_ascii_lowercase())
            .any(|name| name.ends_with("textures.big"));
        let has_w3d_big = loaded
            .iter()
            .map(|name| name.as_str().to_ascii_lowercase())
            .any(|name| name.ends_with("w3d.big"));

        if has_textures_big && has_w3d_big {
            return;
        }

        let mut missing = Vec::new();
        if !has_textures_big {
            missing.push("Textures.big");
        }
        if !has_w3d_big {
            missing.push("W3D.big");
        }

        warn!(
            "Base Generals archives not loaded (missing: {}). Zero Hour models may reference textures unavailable in ZH-only archives.",
            missing.join(", ")
        );
    }

    /// Load a single BIG archive from disk.
    pub async fn load_big_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path_string = path.as_ref().to_string_lossy().into_owned();
        self.core
            .open_archive_file(path_string.as_str())
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    /// Load all BIG archives in the provided directory.
    pub async fn load_big_files_from_directory<P: AsRef<Path>>(
        &mut self,
        dir: P,
        file_mask: &str,
    ) -> Result<bool> {
        let dir_ascii = AsciiString::from(dir.as_ref().to_string_lossy().as_ref());
        let mask_ascii = AsciiString::from(file_mask);
        let loaded = self
            .core
            .load_big_files_from_directory(&dir_ascii, &mask_ascii, true)
            .map_err(anyhow::Error::from)?;
        Ok(loaded)
    }

    /// Load an entire file into memory.
    pub async fn open_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        let mut reader = self
            .core
            .open_file(filename, 0)
            .map_err(anyhow::Error::from)?;

        // C++ parity: perform direct synchronous stream reads from BIG-backed handles.
        // Per-request task dispatch here adds measurable overhead during texture bursts.
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|e| anyhow!("Failed to read archive file: {}", e))?;
        Ok(data)
    }

    /// Borrow a streaming reader for the specified archive entry.
    pub fn open_reader(&mut self, filename: &str) -> Result<Box<dyn Read + Send>> {
        self.core
            .open_file(filename, 0)
            .map_err(anyhow::Error::from)
    }

    /// Borrow a streaming reader usable inside async code via a blocking adapter.
    pub fn open_async_reader(&mut self, filename: &str) -> Result<BlockingAsyncReader> {
        let reader = self
            .core
            .open_file(filename, 0)
            .map_err(anyhow::Error::from)?;
        Ok(BlockingAsyncReader::new(reader))
    }

    /// Check whether a virtual file exists.
    pub fn does_file_exist(&self, filename: &str) -> bool {
        self.core.does_file_exist(filename)
    }

    /// Resolve the archive that currently owns the provided file.
    pub fn get_archive_filename_for_file(&self, filename: &str) -> Option<String> {
        let archive = self
            .core
            .get_archive_filename_for_file(&AsciiString::from(filename));
        if archive.is_empty() {
            None
        } else {
            Some(archive.as_str().to_string())
        }
    }

    /// Find an archive by name across registered search paths.
    pub fn find_archive(&self, name: &str) -> Option<PathBuf> {
        let requested = PathBuf::from(name);
        if let Some(path) = resolve_existing_path_case_insensitive(&requested) {
            if path.is_file() {
                return Some(path);
            }
        }
        if requested.is_file() {
            return Some(requested);
        }

        let search_paths = self.core.search_paths();
        for base in &search_paths {
            // Fast path: exact join with caller-provided casing.
            let direct = base.join(name);
            if let Some(path) = resolve_existing_path_case_insensitive(&direct) {
                if path.is_file() {
                    return Some(path);
                }
            }
            if direct.is_file() {
                return Some(direct);
            }

            // Portable path lookup: BIG archive names are case-insensitive in C++.
            let Ok(entries) = std::fs::read_dir(base) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if file_name.eq_ignore_ascii_case(name) {
                    return Some(path);
                }
            }
        }

        // Last fallback: we may only have loaded archive names, not absolute paths.
        // Try to resolve those names against known search paths.
        let target = name.to_ascii_lowercase();
        for loaded in self.core.get_loaded_big_files() {
            let loaded_name = loaded.as_str();
            if !loaded_name.eq_ignore_ascii_case(name) {
                continue;
            }

            let loaded_path = PathBuf::from(loaded_name);
            if loaded_path.is_file() {
                return Some(loaded_path);
            }

            for base in &search_paths {
                let candidate = base.join(loaded_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }

        // Best-effort containment check for callers that pass suffixes.
        for base in &search_paths {
            let Ok(entries) = std::fs::read_dir(base) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if file_name.to_ascii_lowercase().ends_with(&target) {
                    return Some(path);
                }
            }
        }

        None
    }

    /// Enumerate every known virtual path across loaded archives.
    pub fn list_all_files(&self) -> Vec<String> {
        let mut files = self.core.virtual_paths();
        files.sort();
        files.dedup();
        files
    }

    /// Enumerate loaded archives (sorted).
    pub fn get_loaded_archives(&self) -> Vec<String> {
        self.core
            .get_loaded_big_files()
            .into_iter()
            .map(|s| s.as_str().to_string())
            .collect()
    }

    /// Close a single archive and remove its contributions.
    pub fn close_archive_file(&mut self, filename: &str) {
        self.core.close_archive_file(filename);
    }

    /// Close all archived BIG files.
    pub fn close_all_archive_files(&mut self) {
        self.core.close_all_archive_files();
    }

    /// Reset the archive system to an empty state.
    pub fn reset(&mut self) {
        self.core.close_all_archive_files();
    }

    /// Collect aggregate statistics about the currently loaded archive set.
    pub fn get_statistics(&self) -> ArchiveStatistics {
        ArchiveStatistics {
            total_archives: self.core.get_loaded_big_files().len(),
            total_files: self.core.total_physical_files(),
            unique_files: self.core.total_virtual_files(),
        }
    }
}

fn resolve_existing_path_case_insensitive(path: &Path) -> Option<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::Normal(part) => {
                let search_dir = if resolved.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    resolved.as_path()
                };
                let part = part.to_string_lossy();
                let matched = std::fs::read_dir(search_dir)
                    .ok()?
                    .filter_map(Result::ok)
                    .find_map(|entry| {
                        if entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&part)
                        {
                            Some(entry.path())
                        } else {
                            None
                        }
                    })?;
                resolved = matched;
            }
        }
    }

    resolved.exists().then_some(resolved)
}

/// Archive system statistics (mirrors legacy reporting).
#[derive(Debug, Default, Clone)]
pub struct ArchiveStatistics {
    pub total_archives: usize,
    pub total_files: usize,
    pub unique_files: usize,
}

/// Global archive file system instance - thread-safe version
static ARCHIVE_SYSTEM: OnceLock<Arc<Mutex<ArchiveFileSystem>>> = OnceLock::new();

/// Initialize the global archive file system
pub async fn init_archive_file_system() -> Result<()> {
    let archive_system = Arc::new(Mutex::new(ArchiveFileSystem::new()));

    {
        let mut system = archive_system.lock().unwrap_or_else(|e| e.into_inner());
        system.init().await?;
    }

    ARCHIVE_SYSTEM
        .set(archive_system.clone())
        .map_err(|_| anyhow!("Archive system already initialized"))?;

    Ok(())
}

/// Get reference to global archive file system
pub fn get_archive_file_system() -> Option<Arc<Mutex<ArchiveFileSystem>>> {
    ARCHIVE_SYSTEM.get().cloned()
}

/// Adapter that exposes a blocking reader as an `AsyncRead` using `block_in_place`.
pub struct BlockingAsyncReader {
    inner: Arc<Mutex<Box<dyn Read + Send>>>,
    in_flight: Option<JoinHandle<io::Result<Vec<u8>>>>,
}

impl BlockingAsyncReader {
    fn new(reader: Box<dyn Read + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(reader)),
            in_flight: None,
        }
    }
}

impl AsyncRead for BlockingAsyncReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        if this.in_flight.is_none() {
            let inner = this.inner.clone();
            let to_read = buf.remaining().min(64 * 1024);
            this.in_flight = Some(tokio::task::spawn_blocking(move || {
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                let mut tmp = vec![0u8; to_read];
                loop {
                    match guard.read(&mut tmp) {
                        Ok(0) => {
                            tmp.clear();
                            return Ok(tmp);
                        }
                        Ok(read) => {
                            tmp.truncate(read);
                            return Ok(tmp);
                        }
                        Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                        Err(err) => return Err(err),
                    }
                }
            }));
        }

        let Some(handle) = &mut this.in_flight else {
            return Poll::Ready(Ok(()));
        };

        match Pin::new(handle).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(join_result) => {
                this.in_flight = None;
                match join_result {
                    Ok(Ok(bytes)) => {
                        buf.put_slice(&bytes);
                        Poll::Ready(Ok(()))
                    }
                    Ok(Err(err)) => Poll::Ready(Err(err)),
                    Err(err) => Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("blocking reader task failed: {err}"),
                    ))),
                }
            }
        }
    }
}

pub struct BigArchiveFileReader {
    archive_system: Arc<Mutex<ArchiveFileSystem>>,
}

impl BigArchiveFileReader {
    pub fn new(archive_system: Arc<Mutex<ArchiveFileSystem>>) -> Self {
        Self { archive_system }
    }
}

impl ArchiveFileReader for BigArchiveFileReader {
    fn read_from_archive(&self, path: &str) -> Option<Vec<u8>> {
        let mut guard = self.archive_system.lock().ok()?;
        let mut reader = guard.open_reader(path).ok()?;
        let mut data = Vec::new();
        reader.read_to_end(&mut data).ok()?;
        if data.is_empty() {
            return None;
        }
        Some(data)
    }
}

static BIG_ARCHIVE_READER: OnceLock<Arc<BigArchiveFileReader>> = OnceLock::new();

pub fn init_big_archive_file_reader() -> Result<()> {
    let archive_system =
        get_archive_file_system().ok_or_else(|| anyhow!("Archive file system not initialized"))?;
    let reader = Arc::new(BigArchiveFileReader::new(archive_system));
    BIG_ARCHIVE_READER
        .set(reader)
        .map_err(|_| anyhow!("Big archive reader already initialized"))?;
    Ok(())
}

pub fn get_big_archive_file_reader() -> Option<Arc<BigArchiveFileReader>> {
    BIG_ARCHIVE_READER.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Mutex as StdMutex, OnceLock as StdOnceLock};

    fn env_lock() -> &'static StdMutex<()> {
        static LOCK: StdOnceLock<StdMutex<()>> = StdOnceLock::new();
        LOCK.get_or_init(|| StdMutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn create_single_file_big(path: &Path, virtual_path: &str, data: &[u8]) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        let data_offset = 0x10 + 8 + virtual_path.len() + 1;
        let archive_size = data_offset + data.len();

        file.write_all(b"BIGF")?;
        file.write_all(&(archive_size as u32).to_le_bytes())?;
        file.write_all(&1u32.to_be_bytes())?;
        file.write_all(&(data_offset as u32).to_be_bytes())?;
        file.write_all(&(data_offset as u32).to_be_bytes())?;
        file.write_all(&(data.len() as u32).to_be_bytes())?;
        file.write_all(virtual_path.as_bytes())?;
        file.write_all(&[0])?;
        file.write_all(data)?;

        Ok(())
    }

    #[tokio::test]
    async fn archive_system_initializes() {
        let mut archive_system = ArchiveFileSystem::new();
        assert!(archive_system.init().await.is_ok());
    }

    #[test]
    fn async_reader_reports_missing_files() {
        let mut archive_system = ArchiveFileSystem::new();
        futures::executor::block_on(archive_system.init()).unwrap();
        let result = archive_system.open_async_reader("does/not/exist.txt");
        assert!(result.is_err());
    }

    #[test]
    fn blocking_async_reader_streams_bytes() {
        use tokio::io::AsyncReadExt;

        let reader = BlockingAsyncReader::new(Box::new(std::io::Cursor::new(b"abc".to_vec())));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async move {
            let mut reader = reader;
            let mut buf = [0u8; 3];
            reader.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"abc");
        });
    }

    #[test]
    fn archive_statistics_default() {
        let archive_system = ArchiveFileSystem::new();
        let stats = archive_system.get_statistics();
        assert_eq!(stats.total_archives, 0);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.unique_files, 0);
    }

    #[test]
    fn init_discovers_retail_windows_game_archives() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(repo_root) = manifest_dir.ancestors().nth(3) else {
            eprintln!("Skipping retail archive discovery test: cannot resolve repo root");
            return;
        };
        let retail_root = repo_root
            .join("windows_game")
            .join("Command & Conquer Generals Zero Hour");
        if !retail_root.join("INIZH.big").is_file() {
            eprintln!(
                "Skipping retail archive discovery test: {} is unavailable",
                retail_root.display()
            );
            return;
        }

        let mut archive_system = ArchiveFileSystem::new();
        futures::executor::block_on(archive_system.init()).unwrap();

        let loaded: Vec<String> = archive_system
            .get_loaded_archives()
            .into_iter()
            .map(|archive| archive.replace('\\', "/").to_ascii_lowercase())
            .collect();

        assert!(
            loaded.iter().any(|archive| archive.ends_with("/inizh.big")),
            "INIZH.big should be loaded from the retail windows_game layout"
        );
        assert!(
            loaded
                .iter()
                .any(|archive| archive.ends_with("/audioenglishzh.big")),
            "localized English audio archive should be loaded"
        );
        assert!(
            archive_system.does_file_exist("data/ini/gamedata.ini"),
            "virtual lookups should be case-insensitive"
        );
        assert!(
            archive_system.does_file_exist("Data\\Audio\\Sounds\\English\\aangr01a.wav"),
            "localized archive entries should accept C++ backslash paths"
        );

        let owner = archive_system
            .get_archive_filename_for_file("DATA/INI/GAMEDATA.INI")
            .expect("GameData.ini should resolve to its owning archive")
            .replace('\\', "/")
            .to_ascii_lowercase();
        assert!(
            owner.ends_with("/inizh.big"),
            "GameData.ini should be owned by INIZH.big, got {owner}"
        );
    }

    #[test]
    fn init_discovers_direct_install_path_from_env() {
        let _guard = env_lock().lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("INIZH.big");
        create_single_file_big(
            &archive_path,
            "Data/INI/EnvInstall.ini",
            b"env install data",
        )
        .unwrap();
        let _env = EnvVarGuard::set("GENERALS_INSTALL_PATH", temp_dir.path());

        let mut archive_system = ArchiveFileSystem::new();
        futures::executor::block_on(archive_system.init()).unwrap();

        assert!(
            archive_system.does_file_exist("data/ini/envinstall.ini"),
            "GENERALS_INSTALL_PATH should load BIG files directly in the install directory"
        );
        let data = futures::executor::block_on(archive_system.open_file("Data/INI/EnvInstall.ini"))
            .unwrap();
        assert_eq!(data, b"env install data");
    }

    #[test]
    fn find_archive_resolves_nested_case_insensitive_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let actual_dir = temp_dir.path().join("Data").join("English");
        std::fs::create_dir_all(&actual_dir).unwrap();
        let actual_archive = actual_dir.join("AudioEnglishZH.big");
        std::fs::write(&actual_archive, b"placeholder").unwrap();

        let mut archive_system = ArchiveFileSystem::new();
        archive_system.add_search_path(temp_dir.path());

        let resolved = archive_system
            .find_archive("data/english/audioenglishzh.big")
            .expect("nested archive path should resolve case-insensitively");

        assert_eq!(resolved, actual_archive);
    }

    #[test]
    fn find_archive_resolves_absolute_case_insensitive_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let actual_dir = temp_dir.path().join("Command & Conquer Generals Zero Hour");
        std::fs::create_dir_all(&actual_dir).unwrap();
        let actual_archive = actual_dir.join("INIZH.big");
        std::fs::write(&actual_archive, b"placeholder").unwrap();

        let requested = temp_dir
            .path()
            .join("command & conquer generals zero hour")
            .join("inizh.big");
        let archive_system = ArchiveFileSystem::new();

        let resolved = archive_system
            .find_archive(requested.to_str().unwrap())
            .expect("absolute archive path should resolve case-insensitively");

        assert_eq!(resolved, actual_archive);
    }
}
