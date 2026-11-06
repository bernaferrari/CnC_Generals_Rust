//! Shared preconditions for the P0 retail executable gates (hq-x1j4u.9.x).
//!
//! Gates run with cwd = `Code/Main`, so every default location is anchored at
//! this crate's manifest. Missing retail data, Lone Eagle or the `generals`
//! binary is a gate FAILURE, not a silent pass; set
//! `GENERALS_GATES_ALLOW_SKIP=1` to explicitly opt out on machines without
//! retail assets.
#![allow(dead_code)]

use generals_main::executable_smoke::{ExecutableSmokeResult, lone_eagle_map_on_disk};
use std::path::{Path, PathBuf};

/// Explicit opt-out: unset / `0` / `false` means a missing precondition fails.
pub const ALLOW_SKIP_ENV: &str = "GENERALS_GATES_ALLOW_SKIP";

/// Repository root (holds `windows_game/`): `Code/Main` → `GeneralsRust` → root.
pub fn repo_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    root.canonicalize().unwrap_or(root)
}

fn skip_allowed() -> bool {
    std::env::var(ALLOW_SKIP_ENV).is_ok_and(|v| {
        let v = v.trim();
        !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false"))
    })
}

/// Fail the gate, unless the explicit opt-out is set (then the caller skips).
fn missing(gate: &str, what: &str) -> bool {
    assert!(
        skip_allowed(),
        "{gate}: {what}; set GENERALS_RETAIL_DIR / GENERALS_RUNTIME_EXE, or {ALLOW_SKIP_ENV}=1 to skip"
    );
    eprintln!("{gate}: SKIP ({ALLOW_SKIP_ENV}) — {what}");
    false
}

fn extracted_lone_eagle(root: &Path) -> bool {
    root.join("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
        .is_file()
}

fn retail_looks_valid(root: &Path) -> bool {
    let data = if root
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("Data"))
    {
        root.to_path_buf()
    } else {
        root.join("Data")
    };
    data.join("MapsZH.big").is_file()
        || data.join("EnglishZH.big").is_file()
        || extracted_lone_eagle(&repo_root())
}

fn retail_root() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Some(dir) = std::env::var_os(key) {
            let p = PathBuf::from(dir);
            if retail_looks_valid(&p) {
                return Some(p);
            }
        }
    }
    let windows_game = repo_root().join("windows_game");
    [
        windows_game.join("Command & Conquer Generals Zero Hour"),
        windows_game,
    ]
    .into_iter()
    .find(|c| retail_looks_valid(c))
    .map(|c| c.canonicalize().unwrap_or(c))
}

/// Resolve retail data and publish `GENERALS_ASSETS_DIR` for the smoke runner.
/// Returns false only when the explicit opt-out skips the gate.
pub fn require_retail(gate: &str) -> bool {
    let Some(retail) = retail_root() else {
        return missing(gate, "retail assets not found");
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        // SAFETY: set before the smoke runner spawns any thread or child; each
        // gate binary runs a single test.
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    true
}

/// [`require_retail`] plus the retail Lone Eagle map the skirmish gates load.
pub fn require_lone_eagle(gate: &str) -> bool {
    if !require_retail(gate) {
        return false;
    }
    if !lone_eagle_map_on_disk() {
        return missing(gate, "Lone Eagle.map not found");
    }
    true
}

/// The runner reports `binary_missing` when no `generals` executable resolved.
pub fn ran(gate: &str, r: &ExecutableSmokeResult) -> bool {
    if r.status == "binary_missing" {
        return missing(gate, &format!("generals binary missing — {}", r.detail));
    }
    true
}

/// Timeout from `key`, else `default_secs`.
pub fn timeout(key: &str, default_secs: u64) -> std::time::Duration {
    std::time::Duration::from_secs(
        std::env::var(key)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default_secs),
    )
}
