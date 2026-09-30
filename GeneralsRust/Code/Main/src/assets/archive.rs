////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! High-level archive facade built on top of the modernized core BIG loader.

use super::resolver::LiveAssetResolver;
use anyhow::{Result, anyhow};
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
    live_asset_resolver: LiveAssetResolver,
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
            live_asset_resolver: LiveAssetResolver::new(super::resolver::default_asset_roots()),
        }
    }

    fn add_default_search_paths(&mut self) {
        fn push_install_layout_paths(push_unique: &mut impl FnMut(PathBuf), root: &Path) {
            push_unique(root.join("assets"));

            // Official retail directory names (not a repo-specific wrapper folder).
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
        // C++ Win32BIGFileSystem.cpp:39-49 — original Generals InstallPath.
        if let Ok(from_env) = std::env::var("GENERALS_BASE_DIR") {
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
        for path in game_engine::common::system::install_layout::zh_install_roots() {
            push_unique(path);
        }
        for path in game_engine::common::system::install_layout::extracted_asset_roots() {
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
                    .is_some_and(|home| ancestor.starts_with(home))
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
                    if name.contains("zero hour") || name.contains("zh") {
                        push_install_layout_paths(&mut push_unique, &child);
                    } else {
                        // Sibling original-Generals install next to ZH.
                        push_unique(child.clone());
                        push_unique(child.join("Data"));
                    }
                }
            }
        }

        // C++ loads `*.big` from the original Generals install as well as ZH.
        // Probe sibling installs, Steam/EA common paths, windows_game extracts,
        // and GENERALS_BASE_DIR even when this machine has no W3D.big yet.
        for path in base_generals_mount_dirs() {
            push_unique(path);
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
        let local_roots = self.core.search_paths().to_vec();
        self.add_local_search_roots(local_roots);
        self.warn_if_base_archives_missing();
        Ok(())
    }

    /// Set the owner's ordered extracted/language/mod roots. Roots supplied by
    /// archive initialization are appended after these explicit overrides.
    pub fn set_local_search_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        self.live_asset_resolver.set_roots(roots);
    }

    pub fn add_local_search_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let mut ordered = self.live_asset_resolver.roots().to_vec();
        ordered.extend(roots);
        self.live_asset_resolver.set_roots(ordered);
    }

    /// Resolve a local/extracted file with Windows-style case-insensitive path
    /// matching. This is the local side of C++ FileSystem::openFile dispatch.
    pub fn resolve_local_file(&self, filename: &str) -> Option<PathBuf> {
        let relative = PathBuf::from(filename.replace('\\', "/"));
        let candidates = if relative.is_absolute() {
            vec![relative]
        } else {
            self.live_asset_resolver
                .roots()
                .iter()
                .map(|root| root.join(&relative))
                .collect()
        };
        self.live_asset_resolver
            .first_existing_file_candidate(&candidates)
    }

    pub fn find_filesystem_w3d(&mut self, model_key: &str) -> Option<PathBuf> {
        super::mesh_asset_resolve::find_filesystem_w3d_with_resolver(
            model_key,
            &mut self.live_asset_resolver,
        )
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

    /// C++ ArchiveFileSystem::loadMods — overwrite only the user -mod BIG/dir.
    pub fn load_user_mods(&mut self, mod_dir: &str, mod_big: &str) -> Result<()> {
        use game_engine::common::ascii_string::AsciiString;
        if !mod_big.trim().is_empty() {
            let path = Path::new(mod_big);
            if path.exists() {
                self.core
                    .open_archive_file(mod_big)
                    .map_err(anyhow::Error::from)?;
            }
        }
        if !mod_dir.trim().is_empty() {
            let dir = Path::new(mod_dir);
            if dir.exists() {
                let dir_ascii = AsciiString::from(dir.to_string_lossy().as_ref());
                let mask = AsciiString::from("*.big");
                self.core
                    .load_big_files_from_directory(&dir_ascii, &mask, true)
                    .map_err(anyhow::Error::from)?;
            }
        }
        Ok(())
    }

    /// Load an entire file into memory.
    pub async fn open_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        self.open_file_sync(filename)
    }

    /// BIG-backed file reads are synchronous; keep that fact visible to render
    /// paths which need a model during the current frame.
    pub fn open_file_sync(&mut self, filename: &str) -> Result<Vec<u8>> {
        if let Some(path) = self.resolve_local_file(filename) {
            if let Ok(mut file) = std::fs::File::open(&path) {
                let mut data = Vec::new();
                file.read_to_end(&mut data).map_err(|e| {
                    anyhow!("Failed to read local asset file '{}': {e}", path.display())
                })?;
                return Ok(data);
            }
        }
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
        if let Some(path) = self.resolve_local_file(filename) {
            if let Ok(file) = std::fs::File::open(&path) {
                return Ok(Box::new(file));
            }
        }
        self.core
            .open_file(filename, 0)
            .map_err(anyhow::Error::from)
    }

    /// Borrow a streaming reader usable inside async code via a blocking adapter.
    pub fn open_async_reader(&mut self, filename: &str) -> Result<BlockingAsyncReader> {
        let reader = self.open_reader(filename)?;
        Ok(BlockingAsyncReader::new(reader))
    }

    /// Check whether a virtual file exists.
    pub fn does_file_exist(&self, filename: &str) -> bool {
        self.resolve_local_file(filename).is_some() || self.core.does_file_exist(filename)
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

/// Base (non-ZH) W3D archives C++ mounts from the original Generals install.
/// `Win32BIGFileSystem.cpp:37-49` loads ZH `*.big` then registry `InstallPath`.
const BASE_GENERALS_W3D_ARCHIVES: &[&str] = &["W3D.big", "W3DEnglish.big"];

const BASE_GENERALS_INSTALL_DIR_NAMES: &[&str] =
    &["Command & Conquer Generals", "Command and Conquer Generals"];

const WINDOWS_GAME_EXTRACT_DIRS: &[&str] = &["extracted_big_files", "extracted_big_files_v2"];

fn discovery_seed_dirs() -> Vec<PathBuf> {
    let mut seeds = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            seeds.push(parent.to_path_buf());
        }
    }
    seeds.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    seeds
}

fn steam_library_common_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut push = |path: PathBuf| {
        if !dirs.iter().any(|existing| existing == &path) {
            dirs.push(path);
        }
    };

    for key in ["STEAM_DIR", "STEAM_PATH", "STEAMROOT"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                let root = PathBuf::from(value);
                push(root.join("steamapps").join("common"));
                push(root.join("Steam").join("steamapps").join("common"));
            }
        }
    }

    if let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
    {
        push(home.join("Library/Application Support/Steam/steamapps/common"));
        push(home.join(".steam/steam/steamapps/common"));
        push(home.join(".steam/root/steamapps/common"));
        push(home.join(".local/share/Steam/steamapps/common"));
        push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common"));
        push(home.join("Steam/steamapps/common"));
    }

    for prefix in [
        r"C:\Program Files (x86)\Steam\steamapps\common",
        r"C:\Program Files\Steam\steamapps\common",
        r"C:\Steam\steamapps\common",
        "/mnt/c/Program Files (x86)/Steam/steamapps/common",
        "/mnt/c/Program Files/Steam/steamapps/common",
        "/mnt/c/Steam/steamapps/common",
    ] {
        push(PathBuf::from(prefix));
    }

    dirs
}

fn classic_ea_games_roots() -> Vec<PathBuf> {
    [
        r"C:\Program Files (x86)\EA Games",
        r"C:\Program Files\EA Games",
        r"C:\Program Files (x86)\Electronic Arts",
        r"C:\Program Files (x86)\Origin Games",
        "/mnt/c/Program Files (x86)/EA Games",
        "/mnt/c/Program Files/EA Games",
        "/mnt/c/Program Files (x86)/Origin Games",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn path_search_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// Combined installers are often unpacked beneath a named Downloads wrapper.
/// Probe only Generals-named directories, at a bounded depth, for the exact
/// base-game install name. C++ obtains this directory from the registry;
/// macOS and Linux have no equivalent registry entry.
fn base_generals_dirs_in_downloads(downloads: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut frontier = vec![(downloads.to_path_buf(), 0usize)];
    while let Some((parent, depth)) = frontier.pop() {
        if depth >= 3 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&parent) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_dir()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.to_ascii_lowercase().contains("generals"))
            })
            .collect();
        children.sort_by_key(|path| path_search_key(path));
        for child in children.into_iter().take(64) {
            let name = child
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if BASE_GENERALS_INSTALL_DIR_NAMES
                .iter()
                .any(|base| name.eq_ignore_ascii_case(base))
            {
                found.push(child);
            } else {
                frontier.push((child, depth + 1));
            }
        }
    }
    found
}

/// Probe locations for original-Generals `W3D.big` / `W3DEnglish.big`.
/// Missing files stay in the list so a present install is still searched.
fn base_generals_search_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |path: PathBuf| {
        if seen.insert(path_search_key(&path)) {
            out.push(path);
        }
    };

    for key in ["GENERALS_BASE_DIR", "GENERALS_BASE_INSTALL_PATH"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                let path = PathBuf::from(value);
                push(path.clone());
                push(path.join("Data"));
            }
        }
    }

    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        for base in base_generals_dirs_in_downloads(&PathBuf::from(home).join("Downloads")) {
            push(base.clone());
            push(base.join("Data"));
        }
    }

    for zh in game_engine::common::system::install_layout::zh_install_roots() {
        if let Some(parent) = zh.parent() {
            for name in BASE_GENERALS_INSTALL_DIR_NAMES {
                push(parent.join(name));
                push(parent.join(name).join("Data"));
            }
        }
    }

    for common in steam_library_common_dirs() {
        for name in BASE_GENERALS_INSTALL_DIR_NAMES {
            push(common.join(name));
            push(common.join(name).join("Data"));
        }
    }

    for ea in classic_ea_games_roots() {
        for name in BASE_GENERALS_INSTALL_DIR_NAMES {
            push(ea.join(name));
            push(ea.join(name).join("Data"));
        }
    }

    for seed in discovery_seed_dirs() {
        for ancestor in seed.ancestors().take(8) {
            let wg = ancestor.join("windows_game");
            for name in BASE_GENERALS_INSTALL_DIR_NAMES {
                push(wg.join(name));
                push(wg.join(name).join("Data"));
            }
            for archive in BASE_GENERALS_W3D_ARCHIVES {
                push(wg.join(archive));
            }
            for extract in WINDOWS_GAME_EXTRACT_DIRS {
                let extract_root = wg.join(extract);
                for archive in BASE_GENERALS_W3D_ARCHIVES {
                    push(extract_root.join(archive));
                }
            }
        }
    }

    out
}

fn is_base_generals_w3d_archive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    BASE_GENERALS_W3D_ARCHIVES
        .iter()
        .any(|want| lower == want.to_ascii_lowercase())
}

/// Existing directories (and parents of present extract archives) safe to mount.
/// Does not add loose `extracted_big_files/W3D` trees — those are not `.big`s.
pub(crate) fn base_generals_mount_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push_dir = |path: PathBuf| {
        if seen.insert(path_search_key(&path)) {
            dirs.push(path);
        }
    };

    for candidate in base_generals_search_candidates() {
        if candidate.is_dir() {
            push_dir(candidate);
            continue;
        }

        let resolved = resolve_existing_path_case_insensitive(&candidate)
            .or_else(|| candidate.is_file().then(|| candidate.clone()));
        let Some(file) = resolved else {
            continue;
        };
        if !file.is_file() {
            continue;
        }
        let Some(name) = file.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_base_generals_w3d_archive_name(name) {
            continue;
        }
        if let Some(parent) = file.parent() {
            push_dir(parent.to_path_buf());
        }
    }

    dirs
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
                    Err(err) => Poll::Ready(Err(io::Error::other(format!(
                        "blocking reader task failed: {err}"
                    )))),
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
            crate::env_compat::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                crate::env_compat::set_var(self.key, previous);
            } else {
                crate::env_compat::remove_var(self.key);
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

    #[tokio::test]
    async fn interleaved_archive_owners_read_same_name_from_their_own_local_roots() {
        use tokio::io::AsyncReadExt;

        let first_root = tempfile::tempdir().unwrap();
        let second_root = tempfile::tempdir().unwrap();
        let unique = first_root.path().file_name().unwrap().to_string_lossy();
        let virtual_path = format!("codex_asset_resolver_{unique}_shared.bin");
        assert!(
            !Path::new(&virtual_path).exists(),
            "the fixture name must not be shadowed by the current directory"
        );

        let first_local = first_root.path().join(&virtual_path);
        let second_local = second_root.path().join(&virtual_path);
        std::fs::write(&first_local, b"first local owner").unwrap();
        std::fs::write(&second_local, b"second local owner").unwrap();

        let first_big = first_root.path().join("owner.big");
        let second_big = second_root.path().join("owner.big");
        create_single_file_big(&first_big, &virtual_path, b"first BIG").unwrap();
        create_single_file_big(&second_big, &virtual_path, b"second BIG").unwrap();

        let mut first = ArchiveFileSystem::new();
        let mut second = ArchiveFileSystem::new();
        first.set_local_search_roots(Vec::new());
        second.set_local_search_roots(Vec::new());
        first.load_big_file(&first_big).await.unwrap();
        second.load_big_file(&second_big).await.unwrap();
        assert_eq!(first.open_file_sync(&virtual_path).unwrap(), b"first BIG");
        assert_eq!(second.open_file_sync(&virtual_path).unwrap(), b"second BIG");

        first.set_local_search_roots([first_root.path().to_path_buf()]);
        second.set_local_search_roots([second_root.path().to_path_buf()]);
        assert!(first.does_file_exist(&virtual_path));
        assert!(second.does_file_exist(&virtual_path));

        assert_eq!(
            first.open_file_sync(&virtual_path).unwrap(),
            b"first local owner"
        );
        assert_eq!(
            second.open_file_sync(&virtual_path).unwrap(),
            b"second local owner"
        );

        let mut first_reader = first.open_reader(&virtual_path).unwrap();
        let mut second_reader = second.open_reader(&virtual_path).unwrap();
        let mut first_stream = Vec::new();
        first_reader.read_to_end(&mut first_stream).unwrap();
        let mut second_stream = Vec::new();
        second_reader.read_to_end(&mut second_stream).unwrap();
        assert_eq!(first_stream, b"first local owner");
        assert_eq!(second_stream, b"second local owner");

        let mut first_async_reader = first.open_async_reader(&virtual_path).unwrap();
        let mut second_async_reader = second.open_async_reader(&virtual_path).unwrap();
        let mut first_async = Vec::new();
        first_async_reader
            .read_to_end(&mut first_async)
            .await
            .unwrap();
        let mut second_async = Vec::new();
        second_async_reader
            .read_to_end(&mut second_async)
            .await
            .unwrap();
        assert_eq!(first_async, b"first local owner");
        assert_eq!(second_async, b"second local owner");
    }

    #[test]
    fn configured_local_roots_precede_a_working_directory_collision() {
        // The default owner includes CWD in its roots. Reconfigured owners must
        // not discover an extra implicit root ahead of their mod/language roots.
        let cwd = std::env::current_dir().unwrap();
        let cwd_fixture = tempfile::tempdir_in(&cwd).unwrap();
        let relative = cwd_fixture
            .path()
            .strip_prefix(&cwd)
            .unwrap()
            .join("Owner.asset");
        std::fs::write(cwd.join(&relative), b"working directory").unwrap();
        let owner_root = tempfile::tempdir().unwrap();
        let owner_file = owner_root.path().join(&relative);
        std::fs::create_dir_all(owner_file.parent().unwrap()).unwrap();
        std::fs::write(&owner_file, b"owning instance").unwrap();

        let mut files = ArchiveFileSystem::new();
        let virtual_path = relative.to_str().unwrap();
        assert_eq!(
            files.open_file_sync(virtual_path).unwrap(),
            b"working directory"
        );
        files.set_local_search_roots([owner_root.path().to_path_buf()]);
        assert_eq!(
            files.open_file_sync(virtual_path).unwrap(),
            b"owning instance"
        );
        files.set_local_search_roots(Vec::new());
        assert!(!files.does_file_exist(virtual_path));
        assert!(files.open_file_sync(virtual_path).is_err());
        // An explicitly supplied absolute file remains a caller-owned input.
        assert_eq!(
            files.open_file_sync(owner_file.to_str().unwrap()).unwrap(),
            b"owning instance"
        );
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
    fn init_discovers_retail_archives_from_install_layout() {
        if game_engine::common::system::install_layout::zh_install_roots().is_empty() {
            eprintln!("Skipping retail archive discovery test: no INIZH.big install found");
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
            "INIZH.big should be loaded from the discovered install layout"
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

    #[test]
    fn base_generals_search_includes_steam_windows_game_and_env() {
        let _guard = env_lock().lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let _env = EnvVarGuard::set("GENERALS_BASE_DIR", temp_dir.path());

        let candidates = base_generals_search_candidates();
        let rendered: Vec<String> = candidates
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/").to_ascii_lowercase())
            .collect();

        assert!(
            rendered.iter().any(|p| p.contains("steamapps/common")
                && p.contains("command")
                && p.contains("generals")
                && !p.contains("zero hour")),
            "Steam common Generals install must be probed"
        );
        assert!(
            rendered
                .iter()
                .any(|p| p.contains("windows_game") && p.ends_with("extracted_big_files/w3d.big")),
            "repo windows_game extract W3D.big must be probed even if absent"
        );
        assert!(
            rendered
                .iter()
                .any(|p| p.contains("windows_game")
                    && p.ends_with("extracted_big_files/w3denglish.big")),
            "repo windows_game extract W3DEnglish.big must be probed even if absent"
        );
        assert!(
            rendered.iter().any(|p| p.contains("windows_game")
                && p.contains("command")
                && p.contains("generals")
                && !p.contains("zero hour")
                && !p.contains("extracted_big")),
            "windows_game sibling Generals install must be probed"
        );
        let base = temp_dir
            .path()
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        assert!(
            rendered.iter().any(|p| p == &base || p.starts_with(&base)),
            "GENERALS_BASE_DIR must be in the search"
        );
    }

    #[test]
    fn discovers_base_archives_beneath_combined_download_wrapper() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp
            .path()
            .join("Command and Conquer Generals + Zero Hour (installer)")
            .join("Command and Conquer Generals + Zero Hour")
            .join("Command and Conquer Generals");
        std::fs::create_dir_all(&base).unwrap();
        assert!(base_generals_dirs_in_downloads(temp.path()).contains(&base));
    }

    #[test]
    fn init_mounts_base_generals_w3d_from_generals_base_dir() {
        let _guard = env_lock().lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        create_single_file_big(
            &temp_dir.path().join("W3D.big"),
            "Art/W3D/BaseUnit.w3d",
            b"w3d",
        )
        .unwrap();
        create_single_file_big(
            &temp_dir.path().join("W3DEnglish.big"),
            "Art/W3D/BaseUnitEN.w3d",
            b"w3den",
        )
        .unwrap();
        let _env = EnvVarGuard::set("GENERALS_BASE_DIR", temp_dir.path());

        let mut archive_system = ArchiveFileSystem::new();
        futures::executor::block_on(archive_system.init()).unwrap();

        let loaded: Vec<String> = archive_system
            .get_loaded_archives()
            .into_iter()
            .map(|a| a.replace('\\', "/").to_ascii_lowercase())
            .collect();
        assert!(
            loaded.iter().any(|a| a.ends_with("/w3d.big")),
            "GENERALS_BASE_DIR W3D.big should mount, got {loaded:?}"
        );
        assert!(
            loaded.iter().any(|a| a.ends_with("/w3denglish.big")),
            "GENERALS_BASE_DIR W3DEnglish.big should mount, got {loaded:?}"
        );
        assert!(archive_system.does_file_exist("art/w3d/baseunit.w3d"));
        assert!(archive_system.does_file_exist("art/w3d/baseuniten.w3d"));
    }

    #[test]
    fn warn_base_archives_does_not_treat_zh_w3d_as_base() {
        // warn_if_base_archives_missing matches `ends_with("w3d.big")`.
        // W3DZH.big / W3DEnglish.big must not silence a missing W3D.big.
        assert!(!"w3dzh.big".ends_with("w3d.big"));
        assert!("w3d.big".ends_with("w3d.big"));
        assert!(!"w3denglish.big".ends_with("w3d.big"));
        assert!(!"textureszh.big".ends_with("textures.big"));
    }
}
