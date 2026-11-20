use super::{EXTRACTED_DIR_NAMES, MARKER_BIGS, MAX_SCAN_DIRS, is_skip_dir_name, path_key};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
                // Use the directory entry's type when it is available. A
                // symlink still follows its target, just as Path::is_dir did.
                let file_type = match entry.file_type() {
                    Ok(kind) if !kind.is_symlink() => Some(kind),
                    _ => path.metadata().ok().map(|meta| meta.file_type()),
                };
                ScannedEntry {
                    name: entry.file_name(),
                    is_file: file_type.is_some_and(|kind| kind.is_file()),
                    is_dir: file_type.is_some_and(|kind| kind.is_dir()),
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
    EXTRACTED_DIR_NAMES
        .iter()
        .any(|name| find_dir_case_insensitive_in(dir, name, entries).is_some())
}

/// Reuse listings while one discovery walks overlapping search roots. Each
/// root still gets its own original traversal order, depth and visit budget.
#[derive(Default)]
pub(super) struct DirectoryScan {
    listings: HashMap<PathBuf, Option<Rc<[ScannedEntry]>>>,
}

impl DirectoryScan {
    fn read(&mut self, dir: &Path) -> Option<Rc<[ScannedEntry]>> {
        if let Some(cached) = self.listings.get(dir) {
            return cached.clone();
        }
        let entries = read_scanned_entries(dir).map(Rc::from);
        self.listings.insert(dir.to_path_buf(), entries.clone());
        entries
    }

    pub(super) fn looks_like_install(&mut self, dir: &Path) -> bool {
        dir.is_dir() && dir_looks_like_zh_install_in(dir, self.read(dir).as_deref())
    }

    pub(super) fn scan_from(&mut self, start: &Path, max_depth: usize) -> (Vec<PathBuf>, Vec<PathBuf>) {
        scan_from_with(start, max_depth, |dir| self.read(dir))
    }
}

pub(super) fn scan_from_with<E: AsRef<[ScannedEntry]>>(
    start: &Path,
    max_depth: usize,
    mut read_entries: impl FnMut(&Path) -> Option<E>,
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

        let listing = read_entries(&dir);
        let entries = listing.as_ref().map(AsRef::as_ref);
        if dir_looks_like_zh_install_in(&dir, entries) {
            installs.push(dir.clone());
        }
        if dir_looks_like_extracted_root_in(&dir, entries) {
            extracted.push(dir.clone());
            for name in EXTRACTED_DIR_NAMES {
                if let Some(child) = find_dir_case_insensitive_in(&dir, name, entries) {
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
            queue.push_back((entry.path.clone(), depth + 1));
        }
    }

    (installs, extracted)
}
