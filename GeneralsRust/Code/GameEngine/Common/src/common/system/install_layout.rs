//! Retail/install layout discovery.
//!
//! Do not assume a repo-specific folder name. Users can put the ZH
//! install (or extracted archives) anywhere. Find it by:
//! 1. `GENERALS_INSTALL_PATH` / `GENERALS_ASSETS_DIR` / `GENERALS_BASE_INSTALL_PATH`
//! 2. cwd / exe / crate manifest ancestors
//! 3. a bounded scan for directories that contain marker `.big` files
//!    (`INIZH.big`, `GensecZH.big`, …) or extracted `INIZH`/`WindowZH` trees
//! 4. official retail directory names (`Command & Conquer Generals Zero Hour`)
//! 5. optical-drive roots (C++ `TheCDManager`)

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

/// C++ `areMusicFilesOnCD` archive name (Windows is case-insensitive).
pub const GENSECZH_BIG: &str = "genseczh.big";

const MARKER_BIGS: &[&str] = &[
    "inizh.big",
    "genseczh.big",
    "musiczh.big",
    "w3dzh.big",
    "textureszh.big",
    "audiozh.big",
    "englishzh.big",
];

const RETAIL_DIR_NAMES: &[&str] = &[
    "Command & Conquer Generals Zero Hour",
    "Command and Conquer Generals Zero Hour",
    "Command & Conquer Generals",
    "Command and Conquer Generals",
];

const EXTRACTED_DIR_NAMES: &[&str] = &[
    "INIZH",
    "WindowZH",
    "MapsZH",
    "EnglishZH",
    "TexturesZH",
    "W3DZH",
    "AudioZH",
];

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    "Library",
    "System Volume Information",
    "$recycle.bin",
];

const MAX_SCAN_DEPTH: usize = 3;
const MAX_SCAN_DIRS: usize = 96;

fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn push_unique(out: &mut Vec<PathBuf>, seen: &mut HashSet<String>, path: PathBuf) {
    if !path.exists() {
        return;
    }
    if seen.insert(path_key(&path)) {
        out.push(path);
    }
}

fn is_skip_dir_name(name: &str) -> bool {
    SKIP_DIR_NAMES
        .iter()
        .any(|skip| name.eq_ignore_ascii_case(skip))
        || name.starts_with('.')
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn is_filesystem_root_or_home(path: &Path) -> bool {
    if path.parent().is_none() {
        return true;
    }
    if let Some(home) = home_dir() {
        if path == home {
            return true;
        }
    }
    matches!(
        path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        "Users" | "home" | "Volumes"
    )
}

/// Seed directories: env overrides, cwd, exe, crate manifest.
pub fn discovery_roots() -> Vec<PathBuf> {
    let mut seeds = Vec::new();
    for env_name in [
        "GENERALS_INSTALL_PATH",
        "GENERALS_ASSETS_DIR",
        "GENERALS_BASE_INSTALL_PATH",
    ] {
        if let Ok(value) = std::env::var(env_name) {
            seeds.push(PathBuf::from(value));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            seeds.push(parent.to_path_buf());
        }
    }
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        seeds.push(PathBuf::from(manifest));
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for seed in seeds {
        push_unique(&mut out, &mut seen, seed.clone());
        for ancestor in seed.ancestors().take(8) {
            if is_filesystem_root_or_home(ancestor) {
                continue;
            }
            push_unique(&mut out, &mut seen, ancestor.to_path_buf());
        }
    }
    out
}

fn dir_looks_like_zh_install(dir: &Path) -> bool {
    MARKER_BIGS
        .iter()
        .any(|name| find_file_case_insensitive(dir, name).is_some())
}

fn dir_looks_like_extracted_root(dir: &Path) -> bool {
    EXTRACTED_DIR_NAMES.iter().any(|name| {
        let child = dir.join(name);
        child.is_dir() || find_dir_case_insensitive(dir, name).is_some()
    })
}

fn find_dir_case_insensitive(dir: &Path, name: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_dir() {
        return Some(direct);
    }
    let want = name.to_ascii_lowercase();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let file_name = entry.file_name();
        if file_name.to_string_lossy().eq_ignore_ascii_case(&want) && entry.path().is_dir() {
            return Some(entry.path());
        }
    }
    None
}

fn scan_from(start: &Path, max_depth: usize) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut installs = Vec::new();
    let mut extracted = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start.to_path_buf(), 0usize));
    let mut visited = 0usize;

    while let Some((dir, depth)) = queue.pop_front() {
        if visited >= MAX_SCAN_DIRS {
            break;
        }
        if !dir.is_dir() || !seen.insert(path_key(&dir)) {
            continue;
        }
        visited += 1;

        if dir_looks_like_zh_install(&dir) {
            installs.push(dir.clone());
        }
        if dir_looks_like_extracted_root(&dir) {
            extracted.push(dir.clone());
            for name in EXTRACTED_DIR_NAMES {
                if let Some(child) = find_dir_case_insensitive(&dir, name) {
                    extracted.push(child);
                }
            }
        }

        if depth >= max_depth {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_skip_dir_name(&name) {
                continue;
            }
            queue.push_back((path, depth + 1));
        }
    }

    (installs, extracted)
}

/// Zero Hour install dirs that actually contain `.big` archives.
pub fn zh_install_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for root in discovery_roots() {
        if dir_looks_like_zh_install(&root) {
            push_unique(&mut out, &mut seen, root.clone());
        }
        for name in RETAIL_DIR_NAMES {
            let candidate = root.join(name);
            if dir_looks_like_zh_install(&candidate) {
                push_unique(&mut out, &mut seen, candidate.clone());
            }
            let data = candidate.join("Data");
            if dir_looks_like_zh_install(&data) {
                push_unique(&mut out, &mut seen, data);
            }
        }
        let data = root.join("Data");
        if dir_looks_like_zh_install(&data) {
            push_unique(&mut out, &mut seen, data);
        }

        let (installs, _) = scan_from(&root, MAX_SCAN_DEPTH);
        for install in installs {
            push_unique(&mut out, &mut seen, install);
        }
    }

    out
}

/// Extracted archive trees (loose files after `.big` unpack).
pub fn extracted_asset_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in discovery_roots() {
        let (_, extracted) = scan_from(&root, MAX_SCAN_DEPTH);
        for path in extracted {
            push_unique(&mut out, &mut seen, path);
        }
        for name in EXTRACTED_DIR_NAMES {
            if let Some(child) = find_dir_case_insensitive(&root, name) {
                push_unique(&mut out, &mut seen, child);
            }
        }
    }
    out
}

/// Optical-drive style roots (C++ `TheCDManager` drive paths).
pub fn optical_drive_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    #[cfg(windows)]
    {
        for letter in b'D'..=b'Z' {
            push_unique(
                &mut out,
                &mut seen,
                PathBuf::from(format!("{}:\\", letter as char)),
            );
        }
    }

    #[cfg(not(windows))]
    {
        for mount_root in ["/Volumes", "/media", "/mnt", "/run/media"] {
            let mount_root = Path::new(mount_root);
            if !mount_root.is_dir() {
                continue;
            }
            push_unique(&mut out, &mut seen, mount_root.to_path_buf());
            if let Ok(entries) = fs::read_dir(mount_root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        push_unique(&mut out, &mut seen, path);
                    }
                }
            }
        }
    }

    out
}

/// Case-insensitive file lookup in one directory (C++ Win32 local FS).
pub fn find_file_case_insensitive(dir: &Path, name: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let want = name.to_ascii_lowercase();
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if file_name.to_string_lossy().eq_ignore_ascii_case(&want) {
            let path = entry.path();
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// Resolve a C++ `Data\\INI\\...` virtual path against cwd, install, and extracted trees.
///
/// C++ `AudioManager::init` (GameAudio.cpp:187-202) loads `Data\\INI\\Music.ini` etc.
/// through the virtual file system. Live Rust cargo tests / extracted BIG trees
/// often keep those files under `INIZH/Data/INI` rather than `cwd/Data/INI`.
pub fn resolve_data_ini_file(virtual_path: &str) -> Option<PathBuf> {
    let normalized = virtual_path.replace('\\', "/");
    let rel = Path::new(&normalized);
    if rel.is_file() {
        return Some(rel.to_path_buf());
    }

    let mut seen = HashSet::new();
    let mut consider = |candidate: PathBuf| -> Option<PathBuf> {
        if !seen.insert(path_key(&candidate)) {
            return None;
        }
        candidate.is_file().then_some(candidate)
    };

    if let Some(found) = consider(rel.to_path_buf()) {
        return Some(found);
    }

    for root in discovery_roots() {
        if let Some(found) = consider(root.join(rel)) {
            return Some(found);
        }
        if let Some(found) = consider(root.join("INIZH").join(rel)) {
            return Some(found);
        }
    }

    for extracted in extracted_asset_roots() {
        if let Some(found) = consider(extracted.join(rel)) {
            return Some(found);
        }
        if let Some(found) = consider(extracted.join("INIZH").join(rel)) {
            return Some(found);
        }
        if extracted
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("INIZH"))
        {
            if let Some(found) = consider(extracted.join(rel)) {
                return Some(found);
            }
        }
    }

    None
}

/// Locate `genseczh.big` / `GensecZH.big` on CD roots or a discovered install.
pub fn find_genseczh_big() -> Option<PathBuf> {
    for root in zh_install_roots() {
        if let Some(path) = find_file_case_insensitive(&root, GENSECZH_BIG) {
            return Some(path);
        }
    }
    for drive in optical_drive_roots() {
        if let Some(path) = find_file_case_insensitive(&drive, GENSECZH_BIG) {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_genseczh_big_by_archive_marker_not_folder_name() {
        let found =
            find_genseczh_big().expect("GensecZH.big must be discoverable from cwd/exe/env");
        let name = found
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        assert_eq!(name, "genseczh.big");
        assert!(found.is_file());
    }

    #[test]
    fn zh_install_roots_contain_inizh_big() {
        let roots = zh_install_roots();
        assert!(
            roots
                .iter()
                .any(|root| find_file_case_insensitive(root, "inizh.big").is_some()),
            "install discovery must locate a directory that contains INIZH.big"
        );
    }

    #[test]
    fn resolve_data_ini_file_finds_sound_effects_from_extracted_tree() {
        // C++ AudioManager::init loads Data\\INI\\SoundEffects.ini (GameAudio.cpp:192-193).
        let found = resolve_data_ini_file("Data/INI/SoundEffects.ini")
            .expect("SoundEffects.ini must resolve from cwd/extracted INIZH");
        assert!(found.is_file());
        assert!(
            found
                .file_name()
                .unwrap()
                .to_string_lossy()
                .eq_ignore_ascii_case("SoundEffects.ini")
        );
    }
    #[test]
    fn install_layout_source_does_not_hardcode_a_repo_folder_name() {
        let production = include_str!("install_layout.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("");
        assert!(
            !production.contains("windows_game"),
            "do not hardcode a repo folder name; users may put .big files anywhere"
        );
    }
}
