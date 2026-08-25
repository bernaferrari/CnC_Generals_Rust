//! Boot Object INI sources matching C++ ThingFactory init.
//!
//! `GameEngine.cpp:458` loads `Data\\INI\\Default\\Object.ini` then
//! `INI::loadDirectory("Data\\INI\\Object", TRUE, INI_LOAD_OVERWRITE)`.
//! Directory files are listed through FileSystem (local + archive), current
//! directory first (sorted), then subdirectories.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DEFAULT_OBJECT_INI: &str = "Data/INI/Default/Object.ini";
const OBJECT_INI_DIR: &str = "Data/INI/Object";

/// Virtual paths in C++ ThingFactory order: Default/Object.ini, then
/// `Data/INI/Object` current-dir files, then nested files.
pub fn collect_object_ini_virtual_paths() -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();

    push_unique(&mut ordered, &mut seen, DEFAULT_OBJECT_INI);

    let (current_dir, nested) = collect_object_directory_files();
    for path in current_dir {
        push_unique(&mut ordered, &mut seen, &path);
    }
    for path in nested {
        push_unique(&mut ordered, &mut seen, &path);
    }

    ordered
}

fn push_unique(ordered: &mut Vec<String>, seen: &mut BTreeSet<String>, path: &str) {
    let key = path.replace('\\', "/").to_ascii_lowercase();
    if seen.insert(key) {
        ordered.push(path.replace('\\', "/"));
    }
}

fn collect_object_directory_files() -> (Vec<String>, Vec<String>) {
    let mut current = BTreeSet::new();
    let mut nested = BTreeSet::new();

    collect_from_file_system(&mut current, &mut nested);
    collect_from_disk_roots(&mut current, &mut nested);
    collect_from_live_archive(&mut current, &mut nested);

    (current.into_iter().collect(), nested.into_iter().collect())
}

fn collect_from_file_system(current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::system::file_system::{FilenameList, get_file_system};

    let fs_lock = get_file_system();
    let Ok(fs) = fs_lock.lock() else {
        return;
    };
    let mut list = FilenameList::new();
    fs.get_file_list_in_directory(
        &AsciiString::from(OBJECT_INI_DIR),
        &AsciiString::from("*.ini"),
        &mut list,
        true,
    );
    for name in list {
        classify_object_ini(name.as_str(), current, nested);
    }
}

fn collect_from_disk_roots(current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    for root in super::CnCGameEngine::startup_ini_disk_roots() {
        let dir = if root == "." {
            PathBuf::from(OBJECT_INI_DIR)
        } else {
            Path::new(root).join(OBJECT_INI_DIR)
        };
        walk_object_ini_dir(&dir, OBJECT_INI_DIR, current, nested);
    }
}

fn walk_object_ini_dir(
    dir: &Path,
    virtual_prefix: &str,
    current: &mut BTreeSet<String>,
    nested: &mut BTreeSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ini"))
        {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                files.push(name.to_string());
            }
        }
    }
    files.sort_by_key(|n| n.to_ascii_lowercase());
    for name in files {
        current.insert(format!("{virtual_prefix}/{name}"));
    }
    subdirs.sort();
    for sub in subdirs {
        let Some(name) = sub.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let child_virtual = format!("{virtual_prefix}/{name}");
        walk_nested_object_ini_dir(&sub, &child_virtual, nested);
    }
}

fn walk_nested_object_ini_dir(dir: &Path, virtual_prefix: &str, nested: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ini"))
        {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                files.push(name.to_string());
            }
        }
    }
    files.sort_by_key(|n| n.to_ascii_lowercase());
    for name in files {
        nested.insert(format!("{virtual_prefix}/{name}"));
    }
    subdirs.sort();
    for sub in subdirs {
        let Some(name) = sub.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        walk_nested_object_ini_dir(&sub, &format!("{virtual_prefix}/{name}"), nested);
    }
}

fn collect_from_live_archive(current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    let Some(manager_arc) = crate::assets::manager::get_asset_manager() else {
        return;
    };
    let Ok(mgr) = manager_arc.try_lock() else {
        return;
    };
    for path in mgr.list_all_files() {
        classify_object_ini(&path, current, nested);
    }
}

fn classify_object_ini(path: &str, current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    if !lower.starts_with("data/ini/object/") || !lower.ends_with(".ini") {
        return;
    }
    let rest = &normalized["Data/INI/Object/".len()..];
    if rest.contains('/') || rest.contains('\\') {
        nested.insert(normalized);
    } else {
        current.insert(normalized);
    }
}
