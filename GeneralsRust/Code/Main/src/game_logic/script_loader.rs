/*
** Command & Conquer Generals Zero Hour(tm) - Map Script Loader
** Copyright 2025 Electronic Arts
**
** Loads WW3D/SAGE mission scripts directly from .map files by decoding the
** chunky container and converting binary ScriptList data into the canonical
** rust structures under gamelogic::scripting::core.
*/

// Split by original C++ script-format ownership. The fragments are textual
// members of this module, so item visibility, field order and defaults,
// version handling, malformed-input behavior, and the public API remain
// unchanged. Map path discovery stays in this root because workspace-root
// search residuals pin its source text here.

use std::cell::Cell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use game_engine::common::dict::{Dict, DictType};
use game_engine::common::ini::{INI, INILoadType, try_get_terrain_roads};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{
    DataChunkInfo, DataChunkInput, file::FileAccess, file_system::get_file_system,
};
use gamelogic::GameLogicError;
use gamelogic::common::MAP_XY_FACTOR;
use gamelogic::common::{AsciiString, ICoord3D};
use gamelogic::polygon_trigger::PolygonTrigger;
use gamelogic::scripting::core::{
    Condition, ConditionType, Coord3D, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptGroup, ScriptList,
};
use gamelogic::scripting::{ScriptListReadInfo, parse_player_scripts_list_chunk};
use gamelogic::system::Coord3D as SystemCoord3D;
use gamelogic::system::map_loader::BridgeData;
use log::{debug, info, trace, warn};

type LoaderResult<T> = Result<T, GameLogicError>;

include!("script_loader/map_types.rs");
include!("script_loader/file_resolution.rs");
include!("script_loader/chunk_decoding.rs");
include!("script_loader/map_settings.rs");
include!("script_loader/map_terrain.rs");
include!("script_loader/map_objects.rs");
include!("script_loader/script_records.rs");
include!("script_loader/tests.rs");

// -------------------------------------------------------------------------------------------------
// Map path discovery
// -------------------------------------------------------------------------------------------------

fn locate_map_file(map_name: &str) -> Option<PathBuf> {
    let trimmed = map_name.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }

    let direct = Path::new(trimmed);
    if direct.is_file() {
        return Some(direct.to_path_buf());
    }
    if direct.extension().is_none() {
        let mut with_ext = direct.to_path_buf();
        with_ext.set_extension("map");
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }

    // Workspace-relative residual: binaries often run with cwd=GeneralsRust/ while
    // retail extracts live at repo_root/windows_game/... Accept ../windows_game and
    // walk parents so absolute-looking relative paths still resolve.
    let mut search_roots: Vec<PathBuf> = vec![PathBuf::from(".")];
    if let Ok(cwd) = std::env::current_dir() {
        search_roots.push(cwd.clone());
        let mut parent = cwd.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(p) = parent {
                search_roots.push(p.clone());
                parent = p.parent().map(|x| x.to_path_buf());
            } else {
                break;
            }
        }
    }
    // Code/Main manifest → repo root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    search_roots.push(manifest.clone());
    search_roots.push(manifest.join(".."));
    search_roots.push(manifest.join("../.."));
    search_roots.push(manifest.join("../../.."));

    // Retail ZH maps live under windows_game/extracted_big_files{,_v2}/MapsZH.
    // C++ virtual FS roots Maps/ at MapsZH.big; join INI "Maps/..." against
    // those extract trees so ShellMapMD resolves from cwd=GeneralsRust/.
    // NOTE: no bare "/Maps" suffixes here — the VFS local backend treats any
    // existing path (directories included) as a hit, so a Maps-suffixed root
    // makes bare names resolve to the map DIRECTORY, which then fails chunky
    // decode. Bare names resolve file-exact via resolve_retail_map_path below.
    let extract_suffixes = [
        "windows_game/extracted_big_files/MapsZH",
        "windows_game/extracted_big_files_v2/MapsZH",
        "extracted_big_files/MapsZH",
        "extracted_big_files_v2/MapsZH",
        "MapsZH",
    ];
    let existing = search_roots.clone();
    for root in &existing {
        for suffix in extract_suffixes {
            search_roots.push(root.join(suffix));
        }
    }

    let normalized = trimmed.replace('\\', "/");
    for root in &search_roots {
        let candidate = root.join(&normalized);
        if candidate.is_file() {
            trace!(
                "Resolved map '{}' via root '{}' -> '{}'",
                map_name,
                root.display(),
                candidate.display()
            );
            return Some(candidate);
        }
        if direct.extension().is_none() {
            let mut with_ext = candidate.clone();
            with_ext.set_extension("map");
            if with_ext.is_file() {
                return Some(with_ext);
            }
        }
    }

    let candidates = build_relative_candidates(trimmed);
    for candidate in candidates {
        if let Some(path) = resolve_path_candidate(&candidate) {
            trace!("Resolved map '{}' to '{}'", map_name, path.display());
            return Some(path);
        }
        // Also try each candidate under workspace roots.
        for root in &search_roots {
            let rooted = root.join(&candidate);
            if let Some(path) = resolve_path_candidate(&rooted) {
                return Some(path);
            }
        }
    }

    // The generic asset resolver covers mounted filesystems and common map
    // paths.  A retail install extracted under `windows_game/MapsZH/Maps`
    // stores a map in a same-named directory (including spaces), so use the
    // shared offline retail resolver as the final exact-name lookup.  This is
    // deliberately not a gameplay fallback: `None` still means the requested
    // map cannot be loaded.
    super::resolve_retail_map_path(trimmed)
}

fn build_relative_candidates(input: &str) -> Vec<PathBuf> {
    let sanitized = input
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if sanitized.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let base = PathBuf::from(&sanitized);
    push_unique(&mut results, base.clone());
    if let Some(stripped) = sanitized
        .strip_prefix("Maps/")
        .or_else(|| sanitized.strip_prefix("maps/"))
    {
        push_unique(&mut results, PathBuf::from(stripped));
    }
    if let Some(stripped) = sanitized
        .strip_prefix("Data/Maps/")
        .or_else(|| sanitized.strip_prefix("data/maps/"))
    {
        push_unique(&mut results, PathBuf::from(stripped));
    }

    if base.extension().is_none() {
        let mut with_ext = base.clone();
        with_ext.set_extension("map");
        push_unique(&mut results, with_ext.clone());
        if let Some(stripped) = sanitized
            .strip_prefix("Maps/")
            .or_else(|| sanitized.strip_prefix("maps/"))
        {
            let mut stripped_with_ext = PathBuf::from(stripped);
            stripped_with_ext.set_extension("map");
            push_unique(&mut results, stripped_with_ext);
        }
        if let Some(stripped) = sanitized
            .strip_prefix("Data/Maps/")
            .or_else(|| sanitized.strip_prefix("data/maps/"))
        {
            let mut stripped_with_ext = PathBuf::from(stripped);
            stripped_with_ext.set_extension("map");
            push_unique(&mut results, stripped_with_ext);
        }

        if base.components().count() == 1 {
            if let Some(file_name) = base.file_name() {
                let leaf = file_name.to_string_lossy();
                let mut nested = PathBuf::from(&sanitized);
                nested.push(format!("{leaf}.map"));
                push_unique(&mut results, nested);
            }
        }
    }

    results
}

fn push_unique(vec: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !vec.iter().any(|existing| existing == &candidate) {
        vec.push(candidate);
    }
}
