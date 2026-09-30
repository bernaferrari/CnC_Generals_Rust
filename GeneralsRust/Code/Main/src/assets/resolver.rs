//! Owned filesystem lookup state for live game assets.
//!
//! Each live `ArchiveFileSystem` owns one resolver alongside its BIG mounts.
//! Search roots and negative lookups therefore share the lifetime of the live
//! asset set instead of being process-global mutable state.

#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LiveAssetResolver {
    roots: Vec<PathBuf>,
    missing_keys: HashSet<String>,
    #[cfg(test)]
    miss_probes: HashMap<String, usize>,
}

impl LiveAssetResolver {
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut resolver = Self::default();
        resolver.set_roots(roots);
        resolver
    }

    /// Replace ordered local roots after language and mod configuration is read.
    pub fn set_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let mut seen = HashSet::new();
        self.roots = roots
            .into_iter()
            .filter(|path| seen.insert(path.to_string_lossy().to_ascii_lowercase()))
            .collect();
        // A changed search topology invalidates the instance's old misses.
        self.missing_keys.clear();
        #[cfg(test)]
        self.miss_probes.clear();
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Return the first existing candidate. Positive lookups are deliberately
    /// uncached; negative lookups retain the legacy per-key/session behavior.
    pub fn resolve_file(&mut self, key: &str, candidates: &[PathBuf]) -> Option<PathBuf> {
        let cache_key = key.to_ascii_lowercase();
        if self.missing_keys.contains(&cache_key) {
            return None;
        }
        let found = candidates
            .iter()
            .find_map(|path| resolve_existing_path_case_insensitive(path).filter(|p| p.is_file()));
        if found.is_none() {
            self.missing_keys.insert(cache_key.clone());
            #[cfg(test)]
            {
                *self.miss_probes.entry(cache_key).or_insert(0) += 1;
            }
        }
        found
    }

    #[cfg(test)]
    pub fn miss_cached(&self, key: &str) -> bool {
        self.missing_keys.contains(&key.to_ascii_lowercase())
    }

    #[cfg(test)]
    pub fn miss_probe_count(&self, key: &str) -> usize {
        self.miss_probes
            .get(&key.to_ascii_lowercase())
            .copied()
            .unwrap_or(0)
    }

    /// Build ordered candidate paths under these owned roots, including W3D's
    /// legacy nested `Art/W3D` directory variants.
    pub fn candidates(&self, names: &[String]) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        for root in &self.roots {
            for name in names {
                let path = root.join(name);
                if seen.insert(path.to_string_lossy().to_ascii_lowercase()) {
                    candidates.push(path);
                }
            }
            for subdir in ["Art/W3D", "art/w3d"] {
                for name in names {
                    let path = root.join(subdir).join(name);
                    if seen.insert(path.to_string_lossy().to_ascii_lowercase()) {
                        candidates.push(path);
                    }
                }
            }
        }
        candidates
    }

    pub fn resolve_under_roots(&mut self, key: &str, names: &[String]) -> Option<PathBuf> {
        let candidates = self.candidates(names);
        self.resolve_file(key, &candidates)
    }

    /// Compatibility selection for readers that retain their own I/O error
    /// behavior and historically retried a path after a miss.
    pub fn first_existing_candidate(&self, candidates: &[PathBuf]) -> Option<PathBuf> {
        candidates
            .iter()
            .find_map(|path| resolve_existing_path_case_insensitive(path))
    }

    pub fn first_existing_file_candidate(&self, candidates: &[PathBuf]) -> Option<PathBuf> {
        candidates.iter().find_map(|path| {
            resolve_existing_path_case_insensitive(path).filter(|resolved| resolved.is_file())
        })
    }
}

fn resolve_existing_path_case_insensitive(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
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
                        entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&part)
                            .then(|| entry.path())
                    })?;
                resolved = matched;
            }
        }
    }
    resolved.exists().then_some(resolved)
}

pub(crate) fn default_asset_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.clone());
        for rel in super::mesh_asset_resolve::W3D_SEARCH_ROOT_RESIDUALS {
            roots.push(cwd.join(rel));
        }
        roots.push(cwd.join("../../../windows_game/extracted_big_files/W3DZH/Art/W3D"));
        roots.push(cwd.join("../../../windows_game/extracted_big_files/W3DEnglishZH/Art/W3D"));
        roots.push(cwd.join("../../Tools/w3d_to_gltf/W3D"));
        roots.push(cwd.join("../Tools/w3d_to_gltf/W3D"));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    roots.push(manifest.join("assets"));
    roots.push(manifest.join("../Tools/w3d_to_gltf/W3D"));
    roots.push(manifest.join("../../windows_game/extracted_big_files/W3DZH/Art/W3D"));
    roots.push(manifest.join("../../../windows_game/extracted_big_files/W3DZH/Art/W3D"));
    roots.push(manifest.join("../../../windows_game/extracted_big_files/W3DEnglishZH/Art/W3D"));
    for rel in super::mesh_asset_resolve::W3D_SEARCH_ROOT_RESIDUALS {
        roots.push(manifest.join("../../../").join(rel));
    }
    roots
}

pub(crate) fn prepend_roots(base: &[PathBuf], overrides: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for path in overrides.iter().chain(base) {
        if seen.insert(path.to_string_lossy().to_ascii_lowercase()) {
            roots.push(path.clone());
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn interleaved_owned_resolvers_keep_root_and_negative_cache_state_separate() {
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        fs::write(first.path().join("Shared.W3D"), b"first").unwrap();
        fs::write(second.path().join("Shared.W3D"), b"second").unwrap();
        let names = vec!["Shared.W3D".to_string()];
        let absent = vec!["Missing.W3D".to_string()];
        let mut resolver_a = LiveAssetResolver::new(vec![first.path().to_path_buf()]);
        let mut resolver_b = LiveAssetResolver::new(vec![second.path().to_path_buf()]);

        assert_eq!(
            resolver_a.resolve_under_roots("Shared", &names).unwrap(),
            first.path().join("Shared.W3D")
        );
        assert_eq!(
            resolver_b.resolve_under_roots("Shared", &names).unwrap(),
            second.path().join("Shared.W3D")
        );
        assert_eq!(resolver_a.resolve_under_roots("Missing", &absent), None);
        fs::write(second.path().join("Missing.W3D"), b"now present").unwrap();
        assert!(resolver_a.miss_cached("Missing"));
        assert_eq!(resolver_a.resolve_under_roots("Missing", &absent), None);
        assert_eq!(
            resolver_b.resolve_under_roots("Missing", &absent),
            Some(second.path().join("Missing.W3D"))
        );
        assert_eq!(resolver_a.miss_probe_count("missing"), 1);
        assert_eq!(resolver_b.miss_probe_count("missing"), 0);
    }

    #[test]
    fn ordered_roots_and_case_insensitive_components_match_live_lookup() {
        let mod_root = tempdir().unwrap();
        let base_root = tempdir().unwrap();
        fs::create_dir_all(mod_root.path().join("art/w3d")).unwrap();
        fs::create_dir_all(base_root.path().join("ART/W3D")).unwrap();
        fs::write(mod_root.path().join("art/w3d/Unit.W3D"), b"mod").unwrap();
        fs::write(base_root.path().join("ART/W3D/Unit.W3D"), b"base").unwrap();
        let mut resolver = LiveAssetResolver::new(vec![
            mod_root.path().to_path_buf(),
            base_root.path().to_path_buf(),
        ]);
        let names = vec!["UNIT.w3d".to_string()];

        let resolved = resolver
            .resolve_under_roots("Unit", &names)
            .expect("case-insensitive lookup should find the asset");
        let resolved_identity = fs::canonicalize(resolved).unwrap();
        let mod_root_identity = fs::canonicalize(mod_root.path()).unwrap();
        assert!(
            resolved_identity.starts_with(&mod_root_identity),
            "the first override root wins even if the filesystem preserves another spelling"
        );
        assert_eq!(fs::read(resolved_identity).unwrap(), b"mod");
    }
}
