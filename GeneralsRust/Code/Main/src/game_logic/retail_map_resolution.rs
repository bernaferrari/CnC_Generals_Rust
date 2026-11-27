//! Retail map resolution.

use std::path::{Path, PathBuf};

#[cfg(test)]
pub(super) const DEFAULT_SKIRMISH_MAP_NAME: &str = "Defcon6";

#[cfg(test)]
pub(super) const LONE_EAGLE_MAP_NAME: &str = "Lone Eagle";

/// Candidate roots for MapsZH retail trees.
const RETAIL_MAP_ROOTS: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps",
    "windows_game/extracted_big_files_v2/MapsZH/Maps",
    "../windows_game/extracted_big_files/MapsZH/Maps",
    "../windows_game/extracted_big_files_v2/MapsZH/Maps",
    "../../windows_game/extracted_big_files/MapsZH/Maps",
    "../../windows_game/extracted_big_files_v2/MapsZH/Maps",
];

/// Resolve a retail map directory or .map file for `map_name`.
///
/// Walks the current directory and up to six parents to support `Code/Main`,
/// `GeneralsRust/`, or the monorepo root (matches map_frame_scenario).
pub fn resolve_retail_map_path(map_name: &str) -> Option<PathBuf> {
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return None;
    }

    // Detached checkouts and independent test worlds can use the same
    // explicitly supplied Maps directory as the native instance harness.
    if let Some(root) = std::env::var_os("GENERALS_MAPS_DIR") {
        if let Some(map) = find_map_in(&PathBuf::from(root).join("Maps"), map_name) {
            return Some(map);
        }
    }

    let mut bases: Vec<PathBuf> = vec![PathBuf::from(".")];
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        let mut b = cwd;
        for _ in 0..6 {
            if let Some(parent) = b.parent() {
                b = parent.to_path_buf();
                bases.push(b.clone());
            } else {
                break;
            }
        }
    }

    // Compile-time install anchors (repo root via Code/Main manifest): C++
    // resolves map names through TheFileSystem's engine-fixed search paths
    // (FileSystem.cpp:164-178), never the transient process CWD — bare names
    // must resolve the same way from any launch directory.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    bases.push(manifest);
    for ancestor in PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .skip(1)
    {
        bases.push(ancestor.to_path_buf());
    }
    for base in bases {
        for root in RETAIL_MAP_ROOTS {
            let root_path = if Path::new(root).is_absolute() {
                PathBuf::from(root)
            } else {
                base.join(root)
            };
            if let Some(map) = find_map_in(&root_path, map_name) {
                return Some(map);
            }
        }
    }
    None
}

fn find_map_in(root: &Path, map_name: &str) -> Option<PathBuf> {
    let directory = root.join(map_name);
    let exact = directory.join(format!("{map_name}.map"));
    if exact.is_file() {
        return Some(exact);
    }
    std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("map"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_lone_eagle_map_resolve() {
        let paths = [DEFAULT_SKIRMISH_MAP_NAME, LONE_EAGLE_MAP_NAME].map(resolve_retail_map_path);
        if paths.iter().all(Option::is_none) {
            eprintln!("Retail maps are unavailable");
            return;
        }
        for (name, path) in [DEFAULT_SKIRMISH_MAP_NAME, LONE_EAGLE_MAP_NAME]
            .into_iter()
            .zip(paths)
        {
            assert!(
                path.is_some_and(|path| path.is_file()),
                "missing retail map {name}"
            );
        }
    }
}
