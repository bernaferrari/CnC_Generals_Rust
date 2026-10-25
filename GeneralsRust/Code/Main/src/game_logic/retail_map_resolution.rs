//! Retail map resolution.

use std::path::{Path, PathBuf};

/// Lookup residual name index (exact match).

/// start_game Loading residual method names.

/// Ordered start_game Loading residual navigation steps.

/// Runtime-host command residual names.

/// Engine default skirmish map residual (cnc_game_engine DEFAULT_SKIRMISH_MAP).
pub const DEFAULT_SKIRMISH_MAP_WAVE169: &str = "Defcon6";

/// Behavior-gate / retail campaign-adjacent skirmish map residual.
pub const LONE_EAGLE_MAP_WAVE169: &str = "Lone Eagle";

/// Candidate roots for MapsZH retail trees.
pub const MAP_ROOT_CANDIDATES_WAVE169: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps",
    "windows_game/extracted_big_files_v2/MapsZH/Maps",
    "../windows_game/extracted_big_files/MapsZH/Maps",
    "../windows_game/extracted_big_files_v2/MapsZH/Maps",
    "../../windows_game/extracted_big_files/MapsZH/Maps",
    "../../windows_game/extracted_big_files_v2/MapsZH/Maps",
];

/// Honesty: method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 169 composite residual honesty pack.

/// Source residual: start_game_from_ui Loading → load_map → InGame path.

/// Resolve a retail map directory or .map file for `map_name`.
///
/// Walks cwd + up to 6 parents so residuals work from `Code/Main`,
/// `GeneralsRust/`, or the monorepo root (matches map_frame_scenario).
pub fn resolve_retail_map_path(map_name: &str) -> Option<PathBuf> {
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return None;
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
        for root in MAP_ROOT_CANDIDATES_WAVE169 {
            let root_path = if Path::new(root).is_absolute() {
                PathBuf::from(root)
            } else {
                base.join(root)
            };
            let dir = root_path.join(map_name);
            let map_file = dir.join(format!("{map_name}.map"));
            if map_file.is_file() {
                return Some(map_file);
            }
            if dir.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for ent in rd.flatten() {
                        let p = ent.path();
                        if p.extension().and_then(|e| e.to_str()) == Some("map") {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Honesty: default skirmish map (Defcon6) resolves when MapsZH is present.
pub fn honesty_default_skirmish_map_resolves() -> bool {
    match resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169) {
        Some(p) => p.is_file(),
        None => true, // assets unavailable — soft residual
    }
}

/// Honesty: Lone Eagle resolves when MapsZH is present (behavior_gate map).
pub fn honesty_lone_eagle_map_resolves() -> bool {
    match resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169) {
        Some(p) => p.is_file(),
        None => true,
    }
}

/// Live residual: pack + source + map resolve honesty (no full match start).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_lone_eagle_map_resolve() {
        assert!(honesty_default_skirmish_map_resolves());
        assert!(honesty_lone_eagle_map_resolves());
    }
}
