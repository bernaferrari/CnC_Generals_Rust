//! Resolve the retail map adjacent to the configured installation.
use std::path::{Path, PathBuf};

pub(super) fn lone_eagle_from_assets_root(assets_root: &Path) -> Option<PathBuf> {
    // GENERALS_ASSETS_DIR may point to the game installation or its Data dir.
    let installation = if assets_root
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Data"))
    {
        assets_root.parent().unwrap_or(assets_root)
    } else {
        assets_root
    };
    let relative = Path::new("Maps/Lone Eagle/Lone Eagle.map");
    let mut candidates = vec![installation.join(relative)];
    if let Some(parent) = installation.parent() {
        for extract in ["extracted_big_files", "extracted_big_files_v2"] {
            candidates.push(parent.join(extract).join("MapsZH").join(relative));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.canonicalize().unwrap_or(path))
}
