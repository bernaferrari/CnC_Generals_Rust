//! W3D file system (Rust port of W3DFileSystem.cpp)
//!
//! Mirrors C++ `GameFileClass::Set_Name` search order:
//! 1) `Data/<lang>/Art/...`
//! 2) core `Art/...`
//! 3) user overrides (`<UserData>/W3D` or `<UserData>/Textures`)
//! 4) map previews (`<UserData>/MapPreviews`) for `.tga`

use std::env;
use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameFileType {
    Unknown,
    W3d,
    Tga,
    Dds,
}

/// Simple read-only handle used by WW3D resource callers.
pub struct W3dFileHandle {
    path: PathBuf,
    cursor: Cursor<Vec<u8>>,
}

impl W3dFileHandle {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn size(&self) -> std::io::Result<u64> {
        Ok(self.cursor.get_ref().len() as u64)
    }

    pub fn seek(&mut self, offset: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(offset)
    }

    pub fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        self.cursor.read_exact(buf)
    }

    pub fn read_all(mut self) -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.cursor.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

/// Rust reimplementation of C++ `W3DFileSystem` + `GameFileClass` lookup behavior.
pub struct WthreeDFileSystem {
    language: String,
    user_data_path: Option<PathBuf>,
    extra_roots: Vec<PathBuf>,
}

impl WthreeDFileSystem {
    /// Create with language from `WW_LANGUAGE` (default `English`) and optional
    /// user-data root from `WW_USER_DATA_PATH`.
    pub fn new() -> Self {
        let language = env::var("WW_LANGUAGE").unwrap_or_else(|_| "English".to_string());
        let user_data_path = env::var("WW_USER_DATA_PATH")
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);

        Self {
            language,
            user_data_path,
            extra_roots: Vec::new(),
        }
    }

    /// Override language (primarily for tests).
    pub fn with_language(mut self, lang: &str) -> Self {
        self.language = lang.to_string();
        self
    }

    /// Override user-data root (primarily for tests).
    pub fn with_user_data_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.user_data_path = Some(path.into());
        self
    }

    /// Add additional search roots (checked before CWD).
    pub fn push_root<P: Into<PathBuf>>(&mut self, root: P) {
        self.extra_roots.push(root.into());
    }

    /// Return true if the asset can be resolved through C++ lookup order.
    pub fn is_available(&self, name: &str) -> bool {
        self.resolve(name).is_some()
    }

    /// Open a file for read-only use.
    pub fn open(&self, name: &str) -> std::io::Result<W3dFileHandle> {
        let path = self
            .resolve(name)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string()))?;

        let bytes = self
            .read_from_game_fs(&path)
            .or_else(|| fs::read(&path).ok())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string()))?;

        Ok(W3dFileHandle {
            path,
            cursor: Cursor::new(bytes),
        })
    }

    /// Resolve to an on-disk path without opening.
    pub fn resolve(&self, name: &str) -> Option<PathBuf> {
        let normalized = Self::normalize_name(name);
        if normalized.is_empty() {
            return None;
        }

        let file_type = Self::classify_file_type(&normalized);
        for candidate in self.build_lookup_candidates(&normalized, file_type) {
            if let Some(path) = self.resolve_candidate(&candidate) {
                return Some(path);
            }
        }
        None
    }

    fn normalize_name(name: &str) -> String {
        name.replace('\\', "/").trim_start_matches("./").to_string()
    }

    fn classify_file_type(name: &str) -> GameFileType {
        let Some(ext) = Path::new(name).extension().and_then(|ext| ext.to_str()) else {
            return GameFileType::Unknown;
        };

        if ext.eq_ignore_ascii_case("w3d") {
            GameFileType::W3d
        } else if ext.eq_ignore_ascii_case("tga") {
            GameFileType::Tga
        } else if ext.eq_ignore_ascii_case("dds") {
            GameFileType::Dds
        } else {
            GameFileType::Unknown
        }
    }

    fn build_lookup_candidates(&self, filename: &str, file_type: GameFileType) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        let mut push_unique = |path: PathBuf| {
            if !candidates.iter().any(|existing| existing == &path) {
                candidates.push(path);
            }
        };

        match file_type {
            GameFileType::W3d => {
                push_unique(
                    Path::new("Data")
                        .join(&self.language)
                        .join("Art/W3D")
                        .join(filename),
                );
            }
            GameFileType::Tga | GameFileType::Dds => {
                push_unique(
                    Path::new("Data")
                        .join(&self.language)
                        .join("Art/Textures")
                        .join(filename),
                );
            }
            GameFileType::Unknown => {}
        }

        match file_type {
            GameFileType::W3d => push_unique(Path::new("Art/W3D").join(filename)),
            GameFileType::Tga | GameFileType::Dds => {
                push_unique(Path::new("Art/Textures").join(filename))
            }
            GameFileType::Unknown => push_unique(PathBuf::from(filename)),
        }

        if let Some(user_data_path) = &self.user_data_path {
            match file_type {
                GameFileType::W3d => push_unique(user_data_path.join("W3D").join(filename)),
                GameFileType::Tga | GameFileType::Dds => {
                    push_unique(user_data_path.join("Textures").join(filename))
                }
                GameFileType::Unknown => {}
            }

            if file_type == GameFileType::Tga {
                push_unique(user_data_path.join("MapPreviews").join(filename));
            }
        }

        candidates
    }

    fn resolve_candidate(&self, candidate: &Path) -> Option<PathBuf> {
        if candidate.is_absolute() {
            return candidate.is_file().then_some(candidate.to_path_buf());
        }

        if self.exists_in_game_fs(candidate) {
            return Some(candidate.to_path_buf());
        }

        for root in self.search_roots() {
            let resolved = root.join(candidate);
            if resolved.is_file() {
                return Some(resolved);
            }
        }

        None
    }

    fn exists_in_game_fs(&self, candidate: &Path) -> bool {
        let candidate = candidate.to_string_lossy().replace('\\', "/");
        let fs = get_file_system();
        let Ok(guard) = fs.lock() else {
            return false;
        };
        guard.does_file_exist(&candidate)
    }

    fn read_from_game_fs(&self, path: &Path) -> Option<Vec<u8>> {
        let path = path.to_string_lossy().replace('\\', "/");
        let fs = get_file_system();
        let Ok(mut guard) = fs.lock() else {
            return None;
        };
        let mut file = guard.open_file(&path, FileAccess::READ.combine(FileAccess::BINARY))?;
        file.read_entire_and_close().ok()
    }

    fn search_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for extra in &self.extra_roots {
            if !roots.iter().any(|existing| existing == extra) {
                roots.push(extra.clone());
            }
        }
        if let Ok(cwd) = env::current_dir() {
            if !roots.iter().any(|existing| existing == &cwd) {
                roots.push(cwd);
            }
        }
        roots
    }
}

impl Default for WthreeDFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = env::temp_dir();
        path.push(format!("w3dfs_test_{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn resolves_localized_before_core() {
        let root = unique_temp_root();
        let lang_path = root.join("Data/English/Art/Textures");
        let core_path = root.join("Art/Textures");
        fs::create_dir_all(&lang_path).unwrap();
        fs::create_dir_all(&core_path).unwrap();

        let localized = lang_path.join("grass.tga");
        let core = core_path.join("grass.tga");
        fs::write(&localized, b"lang").unwrap();
        fs::write(&core, b"core").unwrap();

        let mut fsys = WthreeDFileSystem::new().with_language("English");
        fsys.push_root(root);

        let handle = fsys.open("grass.tga").unwrap();
        assert_eq!(handle.path(), localized.as_path());
    }

    #[test]
    fn resolves_map_preview_from_user_data() {
        let root = unique_temp_root();
        let user = root.join("UserData");
        let map_dir = user.join("MapPreviews");
        fs::create_dir_all(&map_dir).unwrap();
        let preview = map_dir.join("preview.tga");
        fs::write(&preview, b"preview-bytes").unwrap();

        let mut fsys = WthreeDFileSystem::new()
            .with_language("English")
            .with_user_data_path(user);
        fsys.push_root(root);

        assert!(fsys.is_available("preview.tga"));
        let handle = fsys.open("preview.tga").unwrap();
        let buf = handle.read_all().unwrap();
        assert_eq!(buf, b"preview-bytes");
    }

    #[test]
    fn does_not_guess_extensions() {
        let root = unique_temp_root();
        let core_path = root.join("Art/W3D");
        fs::create_dir_all(&core_path).unwrap();
        fs::write(core_path.join("tank.w3d"), b"mesh").unwrap();

        let mut fsys = WthreeDFileSystem::new().with_language("English");
        fsys.push_root(root);

        assert!(fsys.is_available("tank.w3d"));
        assert!(!fsys.is_available("tank"));
    }
}
