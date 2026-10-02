//! Operation-owned lookup of loose INI files, before the archive fallback.

use super::{
    discovery_roots, extracted_asset_roots_from, ini_loose_override_is_authoritative, path_key,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Candidate roots for one ordered load. Cache directory discovery only;
/// existence and validity of each file are checked at the time of lookup.
#[derive(Default)]
pub(crate) struct DataIniResolver {
    roots: Option<Vec<PathBuf>>,
    extracted: Option<Vec<PathBuf>>,
}

impl DataIniResolver {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn resolve(&mut self, virtual_path: &str) -> Option<PathBuf> {
        self.resolve_with_discovery(virtual_path, discovery_roots, extracted_asset_roots_from)
    }

    fn resolve_with_discovery(
        &mut self,
        virtual_path: &str,
        discover: impl FnOnce() -> Vec<PathBuf>,
        extract: impl FnOnce(&[PathBuf]) -> Vec<PathBuf>,
    ) -> Option<PathBuf> {
        let normalized = virtual_path.replace('\\', "/");
        let rel = Path::new(&normalized);
        // Keep the existing explicit-path behavior: a direct file wins before
        // extracted-override validation, including absolute paths.
        if rel.is_file() {
            return Some(rel.to_path_buf());
        }

        let mut seen = HashSet::new();
        let mut consider = |candidate: PathBuf| -> Option<PathBuf> {
            if !seen.insert(path_key(&candidate))
                || !candidate.is_file()
                || !ini_loose_override_is_authoritative(&candidate)
            {
                return None;
            }
            Some(candidate)
        };
        let roots = self.roots.get_or_insert_with(discover);
        for root in roots.iter() {
            if let Some(found) = consider(root.join(rel)) {
                return Some(found);
            }
            if let Some(found) = consider(root.join("INIZH").join(rel)) {
                return Some(found);
            }
        }
        for extracted in self.extracted.get_or_insert_with(|| extract(roots)).iter() {
            if let Some(found) = consider(extracted.join(rel)) {
                return Some(found);
            }
            if let Some(found) = consider(extracted.join("INIZH").join(rel)) {
                return Some(found);
            }
        }
        None
    }
}

/// Resolve a C++ `Data\\INI\\...` virtual path against cwd, install, and
/// extracted trees. Use a fresh operation for standalone queries.
pub fn resolve_data_ini_file(virtual_path: &str) -> Option<PathBuf> {
    DataIniResolver::new().resolve(virtual_path)
}

#[cfg(test)]
#[path = "ini_resolver/tests.rs"]
mod tests;
