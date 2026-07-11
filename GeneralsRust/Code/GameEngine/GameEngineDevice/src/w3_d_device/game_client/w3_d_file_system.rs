//! W3D file factory adapter.
//!
//! C++ source: `GameEngineDevice/Source/W3DDevice/GameClient/W3DFileSystem.cpp`.
//! The legacy WW3D file factory only serves read-only W3D and image assets through
//! the Generals `TheFileSystem` singleton.  This module keeps that compatibility
//! layer separate from the lower-level Rust file-system backends.

use game_engine::common::{
    global_data,
    ini::ini_webpage_url::get_registry_language,
    system::{
        file::{File, FileAccess, SeekMode},
        file_system::{get_file_system, paths},
    },
};

/// Return value used by C++ `GameFileClass::Seek`/`Size` when no file is open.
pub const GAME_FILE_INVALID_RESULT: i32 = -1;

/// C++ `FileClass::READ` access right accepted by `GameFileClass::Open`.
pub const GAME_FILE_READ: i32 = 1;

/// C runtime `SEEK_SET` value accepted by `GameFileClass::Seek`.
pub const GAME_FILE_SEEK_SET: i32 = 0;

/// C runtime `SEEK_CUR` value accepted by `GameFileClass::Seek`.
pub const GAME_FILE_SEEK_CUR: i32 = 1;

/// C runtime `SEEK_END` value accepted by `GameFileClass::Seek`.
pub const GAME_FILE_SEEK_END: i32 = 2;

/// File kinds recognized by the C++ W3D file factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameFileType {
    /// C++ `FILE_TYPE_COMPLETELY_UNKNOWN`.
    CompletelyUnknown = 0,
    /// `.w3d` model asset.
    W3d = 1,
    /// `.tga` texture/image asset.
    Tga = 2,
    /// `.dds` texture/image asset.
    Dds = 3,
}

impl GameFileType {
    /// Classify a filename using the same case-insensitive extension checks as C++.
    #[must_use]
    pub fn from_filename(filename: &str) -> Self {
        match extension_with_dot(filename).to_ascii_lowercase().as_str() {
            ".w3d" => Self::W3d,
            ".tga" => Self::Tga,
            ".dds" => Self::Dds,
            _ => Self::CompletelyUnknown,
        }
    }

    /// Whether this file type is a C++ image file type.
    #[must_use]
    pub fn is_image(self) -> bool {
        matches!(self, Self::Tga | Self::Dds)
    }
}

/// Rust equivalent of WW3D `GameFileClass`.
pub struct GameFileClass {
    file: Option<Box<dyn File>>,
    file_exists: bool,
    file_path: String,
    filename: String,
}

impl GameFileClass {
    /// Create a closed file object with no filename.
    #[must_use]
    pub fn new() -> Self {
        Self {
            file: None,
            file_exists: false,
            file_path: String::new(),
            filename: String::new(),
        }
    }

    /// Create a file object and immediately run C++ `Set_Name`.
    #[must_use]
    pub fn with_name(filename: &str) -> Self {
        let mut file = Self::new();
        file.set_name(filename);
        file
    }

    /// Return the original filename passed to `Set_Name`.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.filename
    }

    /// Return the currently selected relative path.
    #[must_use]
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// C++ `Set_Name`: close any open file, remember the filename, and resolve it.
    pub fn set_name(&mut self, filename: &str) -> &str {
        let language = get_registry_language().to_string();
        let user_data_dir = {
            let user_data = global_data::read().get_user_data_dir().trim().to_string();
            if user_data.is_empty() {
                None
            } else {
                Some(user_data)
            }
        };

        let file_system = get_file_system();
        let exists = |path: &str| {
            file_system
                .lock()
                .is_ok_and(|fs| fs.does_file_exist(path))
        };
        self.set_name_with_context(filename, &language, user_data_dir.as_deref(), exists);
        &self.filename
    }

    /// C++ `Is_Available`.
    #[must_use]
    pub fn is_available(&self, _forced: bool) -> bool {
        self.file_exists
    }

    /// C++ `Is_Open`.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.file.is_some()
    }

    /// C++ `Open(filename, rights)`.
    pub fn open_named(&mut self, filename: &str, rights: i32) -> bool {
        self.set_name(filename);
        if self.is_available(false) {
            self.open(rights)
        } else {
            false
        }
    }

    /// C++ `Open(rights)`.
    pub fn open(&mut self, rights: i32) -> bool {
        if rights != GAME_FILE_READ {
            return false;
        }

        let file_system = get_file_system();
        self.file = file_system.lock().ok().and_then(|mut fs| {
            fs.open_file(
                &self.file_path,
                FileAccess::READ.combine(FileAccess::BINARY),
            )
        });
        self.file.is_some()
    }

    /// C++ `Read`.
    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.file
            .as_mut()
            .and_then(|file| file.read(buffer).ok())
            .unwrap_or(0)
    }

    /// C++ `Seek`.
    pub fn seek(&mut self, pos: i32, dir: i32) -> i32 {
        let mode = match dir {
            GAME_FILE_SEEK_SET => SeekMode::Start,
            GAME_FILE_SEEK_END => SeekMode::End,
            GAME_FILE_SEEK_CUR => SeekMode::Current,
            _ => SeekMode::Current,
        };

        self.file
            .as_mut()
            .and_then(|file| file.seek(pos, mode).ok())
            .unwrap_or(GAME_FILE_INVALID_RESULT)
    }

    /// C++ `Size`.
    #[must_use]
    pub fn size(&self) -> i32 {
        self.file
            .as_ref()
            .map_or(GAME_FILE_INVALID_RESULT, |file| file.size())
    }

    /// C++ `Write`; the W3D file factory is read-only.
    pub fn write(&mut self, _buffer: &[u8]) -> usize {
        0
    }

    /// C++ `Close`.
    pub fn close(&mut self) {
        if let Some(mut file) = self.file.take() {
            file.close();
        }
    }

    /// C++ `Create` placeholder returns success after asserting in debug builds.
    #[must_use]
    pub fn create(&self) -> bool {
        true
    }

    /// C++ `Delete` placeholder returns success after asserting in debug builds.
    #[must_use]
    pub fn delete(&self) -> bool {
        true
    }

    fn set_name_with_context<F>(
        &mut self,
        filename: &str,
        language: &str,
        user_data_dir: Option<&str>,
        mut exists: F,
    ) where
        F: FnMut(&str) -> bool,
    {
        if self.is_open() {
            self.close();
        }

        self.filename = filename.to_string();
        let file_type = GameFileType::from_filename(filename);

        self.file_exists = false;
        for candidate in cpp_lookup_candidates(
            filename,
            file_type,
            language,
            user_data_dir,
            &self.file_path,
        ) {
            self.file_path = candidate;
            self.file_exists = exists(&self.file_path);
            if self.file_exists {
                break;
            }
        }
    }
}

impl Default for GameFileClass {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GameFileClass {
    fn drop(&mut self) {
        self.close();
    }
}

/// Rust equivalent of the WW3D `W3DFileSystem` file factory.
#[derive(Debug, Default, Clone, Copy)]
pub struct W3DFileSystem;

impl W3DFileSystem {
    /// Construct the W3D file factory adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// C++ `Get_File`.
    #[must_use]
    pub fn get_file(&self, filename: &str) -> GameFileClass {
        GameFileClass::with_name(filename)
    }

    /// C++ `Return_File`; Rust drops the file object.
    pub fn return_file(&self, _file: GameFileClass) {}
}

/// Return a process-wide W3D file factory adapter.
#[must_use]
pub fn get_w3d_file_system() -> &'static W3DFileSystem {
    static FILE_SYSTEM: W3DFileSystem = W3DFileSystem;
    &FILE_SYSTEM
}

fn extension_with_dot(filename: &str) -> &str {
    let Some(index) = filename.rfind('.') else {
        return "";
    };
    &filename[index..]
}

fn cpp_lookup_candidates(
    filename: &str,
    file_type: GameFileType,
    language: &str,
    user_data_dir: Option<&str>,
    previous_path: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();

    match file_type {
        GameFileType::W3d => candidates.push(format!("Data/{language}/Art/W3D/{filename}")),
        file_type if file_type.is_image() => {
            candidates.push(format!("Data/{language}/Art/Textures/{filename}"));
        }
        GameFileType::CompletelyUnknown => candidates.push(previous_path.to_string()),
        _ => unreachable!(),
    }

    match file_type {
        GameFileType::W3d => candidates.push(format!("{}{filename}", paths::W3D_DIR_PATH)),
        file_type if file_type.is_image() => {
            candidates.push(format!("{}{filename}", paths::TGA_DIR_PATH));
        }
        GameFileType::CompletelyUnknown => candidates.push(filename.to_string()),
        _ => unreachable!(),
    }

    if let Some(user_data_dir) = user_data_dir {
        match file_type {
            GameFileType::W3d => {
                candidates.push(format_percent_s(
                    paths::USER_W3D_DIR_PATH,
                    user_data_dir,
                    filename,
                ));
            }
            file_type if file_type.is_image() => {
                candidates.push(format_percent_s(
                    paths::USER_TGA_DIR_PATH,
                    user_data_dir,
                    filename,
                ));
            }
            GameFileType::CompletelyUnknown => candidates.push(filename.to_string()),
            _ => unreachable!(),
        }

        match file_type {
            GameFileType::Tga => {
                candidates.push(format_percent_s(
                    paths::MAP_PREVIEW_DIR_PATH,
                    user_data_dir,
                    filename,
                ));
            }
            _ => {
                if let Some(last) = candidates.last().cloned() {
                    candidates.push(last);
                }
            }
        }
    }

    candidates
}

fn format_percent_s(format: &str, value: &str, filename: &str) -> String {
    format!("{}{}", format.replacen("%s", value, 1), filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_cpp_file_types_case_insensitively() {
        assert_eq!(GameFileType::from_filename("Tank.W3D"), GameFileType::W3d);
        assert_eq!(GameFileType::from_filename("Button.TGA"), GameFileType::Tga);
        assert_eq!(GameFileType::from_filename("Skin.DdS"), GameFileType::Dds);
        assert_eq!(
            GameFileType::from_filename("Data/INI/Object.ini"),
            GameFileType::CompletelyUnknown
        );
    }

    #[test]
    fn w3d_candidates_match_cpp_lookup_order() {
        let candidates = cpp_lookup_candidates(
            "Tank.W3D",
            GameFileType::W3d,
            "German",
            Some("UserData/"),
            "",
        );

        assert_eq!(
            candidates,
            vec![
                "Data/German/Art/W3D/Tank.W3D",
                "Art/W3D/Tank.W3D",
                "UserData/W3D/Tank.W3D",
                "UserData/W3D/Tank.W3D",
            ]
        );
    }

    #[test]
    fn tga_candidates_include_map_preview_after_user_textures() {
        let candidates = cpp_lookup_candidates(
            "Preview.tga",
            GameFileType::Tga,
            "English",
            Some("UserData/"),
            "",
        );

        assert_eq!(
            candidates,
            vec![
                "Data/English/Art/Textures/Preview.tga",
                "Art/Textures/Preview.tga",
                "UserData/Textures/Preview.tga",
                "UserData/MapPreviews/Preview.tga",
            ]
        );
    }

    #[test]
    fn dds_candidates_do_not_use_map_preview() {
        let candidates = cpp_lookup_candidates(
            "Control.dds",
            GameFileType::Dds,
            "English",
            Some("UserData/"),
            "",
        );

        assert_eq!(
            candidates,
            vec![
                "Data/English/Art/Textures/Control.dds",
                "Art/Textures/Control.dds",
                "UserData/Textures/Control.dds",
                "UserData/Textures/Control.dds",
            ]
        );
    }

    #[test]
    fn unknown_candidates_preserve_previous_path_probe_before_raw_filename() {
        let candidates = cpp_lookup_candidates(
            "Config.ini",
            GameFileType::CompletelyUnknown,
            "English",
            Some("UserData/"),
            "Art/W3D/Old.W3D",
        );

        assert_eq!(
            candidates,
            vec!["Art/W3D/Old.W3D", "Config.ini", "Config.ini", "Config.ini",]
        );
    }

    #[test]
    fn set_name_keeps_original_filename_and_first_existing_candidate() {
        let mut file = GameFileClass::new();
        file.set_name_with_context("Tank.W3D", "German", Some("UserData/"), |candidate| {
            candidate == "Art/W3D/Tank.W3D"
        });

        assert_eq!(file.file_name(), "Tank.W3D");
        assert_eq!(file.file_path(), "Art/W3D/Tank.W3D");
        assert!(file.is_available(false));
    }

    #[test]
    fn rejected_open_rights_do_not_touch_file_system() {
        let mut file = GameFileClass::new();
        file.set_name_with_context("Tank.W3D", "German", None, |_| true);

        assert!(!file.open(0));
        assert!(!file.is_open());
    }

    #[test]
    fn closed_file_methods_match_cpp_sentinel_values() {
        let mut file = GameFileClass::new();
        let mut buffer = [0u8; 4];

        assert_eq!(file.read(&mut buffer), 0);
        assert_eq!(file.seek(0, GAME_FILE_SEEK_CUR), GAME_FILE_INVALID_RESULT);
        assert_eq!(file.size(), GAME_FILE_INVALID_RESULT);
        assert_eq!(file.write(&buffer), 0);
    }
}
