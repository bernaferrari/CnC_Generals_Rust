use super::{EXTRACTED_DIR_NAMES, MARKER_BIGS, MAX_SCAN_DIRS, is_skip_dir_name, path_key};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct ScannedEntry {
    name: OsString,
    path: PathBuf,
    is_file: bool,
    is_dir: bool,
}

pub(super) fn read_scanned_entries(dir: &Path) -> Option<Vec<ScannedEntry>> {
    let entries = fs::read_dir(dir).ok()?;
    Some(
        entries
            .flatten()
            .map(|entry| {
                let path = entry.path();
                ScannedEntry {
                    name: entry.file_name(),
                    is_file: path.is_file(),
                    is_dir: path.is_dir(),
                    path,
                }
            })
            .collect(),
    )
}

pub(super) fn find_file_case_insensitive_in(
    dir: &Path,
    name: &str,
    entries: Option<&[ScannedEntry]>,
) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let want = name.to_ascii_lowercase();
    entries?
        .iter()
        .find(|entry| entry.name.to_string_lossy().eq_ignore_ascii_case(&want) && entry.is_file)
        .map(|entry| entry.path.clone())
}

pub(super) fn find_dir_case_insensitive_in(
    dir: &Path,
    name: &str,
    entries: Option<&[ScannedEntry]>,
) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_dir() {
        return Some(direct);
    }
    let want = name.to_ascii_lowercase();
    entries?
        .iter()
        .find(|entry| entry.name.to_string_lossy().eq_ignore_ascii_case(&want) && entry.is_dir)
        .map(|entry| entry.path.clone())
}

fn dir_looks_like_zh_install_in(dir: &Path, entries: Option<&[ScannedEntry]>) -> bool {
    MARKER_BIGS
        .iter()
        .any(|name| find_file_case_insensitive_in(dir, name, entries).is_some())
}

fn dir_looks_like_extracted_root_in(dir: &Path, entries: Option<&[ScannedEntry]>) -> bool {
    EXTRACTED_DIR_NAMES.iter().any(|name| {
        let child = dir.join(name);
        child.is_dir() || find_dir_case_insensitive_in(dir, name, entries).is_some()
    })
}

pub(super) fn scan_from(start: &Path, max_depth: usize) -> (Vec<PathBuf>, Vec<PathBuf>) {
    scan_from_with(start, max_depth, read_scanned_entries)
}

pub(super) fn scan_from_with(
    start: &Path,
    max_depth: usize,
    mut read_entries: impl FnMut(&Path) -> Option<Vec<ScannedEntry>>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
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

        let entries = read_entries(&dir);
        if dir_looks_like_zh_install_in(&dir, entries.as_deref()) {
            installs.push(dir.clone());
        }
        if dir_looks_like_extracted_root_in(&dir, entries.as_deref()) {
            extracted.push(dir.clone());
            for name in EXTRACTED_DIR_NAMES {
                if let Some(child) = find_dir_case_insensitive_in(&dir, name, entries.as_deref()) {
                    extracted.push(child);
                }
            }
        }

        if depth >= max_depth {
            continue;
        }
        let Some(entries) = entries else {
            continue;
        };
        for entry in entries {
            if !entry.is_dir {
                continue;
            }
            let name = entry.name.to_string_lossy();
            if is_skip_dir_name(&name) {
                continue;
            }
            queue.push_back((entry.path, depth + 1));
        }
    }

    (installs, extracted)
}
