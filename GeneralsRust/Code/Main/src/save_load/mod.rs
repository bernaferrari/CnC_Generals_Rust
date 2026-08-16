pub mod campaign;
pub mod compression;
pub mod game_state;
pub mod replay;
pub mod save_file;
pub mod snapshot;
pub mod xfer;

// Re-export core functionality
pub use campaign::*;
pub use compression::*;
pub use game_state::*;
pub use replay::*;
pub use save_file::*;
pub use snapshot::*;
pub use xfer::*;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Save/Load error types
#[derive(Debug, thiserror::Error)]
pub enum SaveLoadError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid save file format")]
    InvalidFormat,

    #[error("Save file version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Corrupted save file: {0}")]
    Corrupted(String),

    #[error("Insufficient disk space")]
    InsufficientSpace,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type for save/load operations
pub type SaveLoadResult<T> = Result<T, SaveLoadError>;

/// Save file version for compatibility checking
pub const SAVE_FILE_VERSION: u32 = 1;

/// Maximum save file slots
pub const MAX_SAVE_SLOTS: usize = 10;

/// Save file extensions
/// Popup and host both write Common CHUNK_*.sav (C++ TheGameState container).
pub const SAVE_EXTENSION: &str = "sav";
/// Legacy host GZHS wrapper. Load still accepts it.
pub const LEGACY_SAVE_EXTENSION: &str = "gen";
pub const REPLAY_EXTENSION: &str = "rep";
pub const CAMPAIGN_EXTENSION: &str = "cam";

/// Save file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveGameInfo {
    pub filename: String,
    pub display_name: String,
    pub description: String,
    pub map_name: String,
    pub campaign_side: Option<String>,
    pub mission_number: Option<u32>,
    pub save_date: SystemTime,
    pub game_version: String,
    pub play_time: std::time::Duration,
    pub difficulty: GameDifficulty,
    pub save_type: SaveFileType,
}

/// Save file types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveFileType {
    Normal,    // Regular in-game save
    Mission,   // Mission transition save
    QuickSave, // Quick save slot
    AutoSave,  // Auto-save
}

/// Game difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameDifficulty {
    Easy,
    Medium,
    Hard,
}

/// File layout types for save/load UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLoadLayoutType {
    SaveAndLoad,
    LoadOnly,
    SaveOnly,
}

/// Available save game information
#[derive(Debug, Clone)]
pub struct AvailableGameInfo {
    pub filename: String,
    pub save_info: SaveGameInfo,
}

/// Main save/load manager singleton
pub struct SaveLoadManager {
    save_directory: PathBuf,
    available_saves: Vec<AvailableGameInfo>,
    current_save_info: Option<SaveGameInfo>,
    in_load_operation: bool,
}

impl Default for SaveLoadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveLoadManager {
    pub fn new() -> Self {
        let save_directory = Self::default_save_directory();

        Self {
            save_directory,
            available_saves: Vec::new(),
            current_save_info: None,
            in_load_operation: false,
        }
    }

    /// Shared host + Popup save directory (`UserData/Save`, same as Common TheGameState).
    ///
    /// Previously the host listed `Save Games` next to the exe while Popup wrote
    /// `UserData/Save`. One directory so `SaveFileManager::list_saves` sees both.
    pub fn default_save_directory() -> PathBuf {
        crate::subsystem_manager::resolve_save_directory()
    }

    /// Initialize save directory
    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Create save directory if it doesn't exist
        if !self.save_directory.exists() {
            std::fs::create_dir_all(&self.save_directory)?;
        }

        // Scan for available save games
        self.refresh_save_list()?;

        Ok(())
    }

    /// Refresh list of available save games
    pub fn refresh_save_list(&mut self) -> SaveLoadResult<()> {
        self.available_saves.clear();

        let entries = std::fs::read_dir(&self.save_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == SAVE_EXTENSION) {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.get_save_info_from_file(&path) {
                        Ok(save_info) => {
                            self.available_saves.push(AvailableGameInfo {
                                filename: filename.to_string(),
                                save_info,
                            });
                        }
                        Err(e) => {
                            log::warn!("Failed to read save info from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        // Sort by save date, newest first
        self.available_saves
            .sort_by(|a, b| b.save_info.save_date.cmp(&a.save_info.save_date));

        Ok(())
    }

    /// Get save file information from file.
    ///
    /// C++ `GameState::getSaveGameInfoFromFile` (GameState.cpp:948-1048) walks
    /// named chunks (`xferAsciiString` token + `beginBlock` i32 size) and
    /// xfers only `CHUNK_GameState`. Do not bincode the first 1024 bytes —
    /// host/Popup `.sav` tokens are not a `SaveGameInfo` record.
    pub fn get_save_info_from_file(&self, path: &PathBuf) -> SaveLoadResult<SaveGameInfo> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let data = if compression::is_compressed(&buffer)? {
            compression::decompress(&buffer)?
        } else {
            buffer
        };

        parse_save_info_from_named_chunks(&data)
    }

    /// Check if save file exists
    pub fn save_exists(&self, filename: &str) -> bool {
        self.get_save_path(filename).exists()
    }

    /// Get full path for save file
    pub fn get_save_path(&self, filename: &str) -> PathBuf {
        let mut path = self.save_directory.clone();
        path.push(format!("{}.{}", filename, SAVE_EXTENSION));
        path
    }

    /// Get available save games
    pub fn get_available_saves(&self) -> &[AvailableGameInfo] {
        &self.available_saves
    }

    /// Set current save info
    pub fn set_current_save_info(&mut self, info: SaveGameInfo) {
        self.current_save_info = Some(info);
    }

    /// Get current save info
    pub fn get_current_save_info(&self) -> Option<&SaveGameInfo> {
        self.current_save_info.as_ref()
    }

    /// Check if currently in load operation
    pub fn is_in_load(&self) -> bool {
        self.in_load_operation
    }

    /// Set load operation state
    pub fn set_load_state(&mut self, loading: bool) {
        self.in_load_operation = loading;
    }
}

/// Global save/load manager instance
use std::sync::{Arc, Mutex, OnceLock};

static SAVE_LOAD_MANAGER: OnceLock<Arc<Mutex<SaveLoadManager>>> = OnceLock::new();

/// Initialize the global save/load system
pub fn init_save_load_system() -> SaveLoadResult<()> {
    let manager_arc =
        SAVE_LOAD_MANAGER.get_or_init(|| Arc::new(Mutex::new(SaveLoadManager::new())));
    let mut manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
    manager.init()
}

/// Get the global save/load manager
pub fn get_save_load_manager() -> Option<Arc<Mutex<SaveLoadManager>>> {
    SAVE_LOAD_MANAGER.get().cloned()
}

const CHUNK_GAME_STATE_TOKEN: &str = "CHUNK_GameState";

/// Walk C++ TheGameState named chunks: `u8` length + token + `i32` size.
fn parse_save_info_from_named_chunks(data: &[u8]) -> SaveLoadResult<SaveGameInfo> {
    use game_engine::common::system::save_game::GameState as CommonGameState;
    use game_engine::common::system::xfer::Xfer as CommonXfer;
    use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
    use std::io::Cursor;
    use std::time::{Duration, UNIX_EPOCH};

    let mut pos = 0usize;
    while pos < data.len() {
        let token_len = data[pos] as usize;
        pos += 1;
        if pos + token_len > data.len() {
            return Err(SaveLoadError::Corrupted(
                "truncated named-chunk token".to_string(),
            ));
        }
        let token = std::str::from_utf8(&data[pos..pos + token_len])
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        pos += token_len;
        if token.eq_ignore_ascii_case("SG_EOF") {
            break;
        }
        if pos + 4 > data.len() {
            return Err(SaveLoadError::Corrupted(
                "truncated named-chunk size".to_string(),
            ));
        }
        let block_size = i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        if block_size < 0 {
            return Err(SaveLoadError::Corrupted(
                "negative named-chunk size".to_string(),
            ));
        }
        let end = pos
            .checked_add(block_size as usize)
            .ok_or_else(|| SaveLoadError::Corrupted("named-chunk size overflow".to_string()))?;
        if end > data.len() {
            return Err(SaveLoadError::Corrupted(
                "named-chunk payload overruns file".to_string(),
            ));
        }
        if token.eq_ignore_ascii_case(CHUNK_GAME_STATE_TOKEN) {
            let mut header = CommonGameState::default();
            let mut xfer = CommonXferLoad::new(Cursor::new(&data[pos..end]), SAVE_FILE_VERSION);
            header
                .xfer(&mut xfer)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            let difficulty = match header
                .get_metadata("difficulty")
                .map(|s| s.as_str())
                .unwrap_or("Medium")
            {
                "Easy" => GameDifficulty::Easy,
                "Hard" => GameDifficulty::Hard,
                _ => GameDifficulty::Medium,
            };
            let save_type = match header.game_mode.as_str() {
                "Mission" => SaveFileType::Mission,
                "QuickSave" => SaveFileType::QuickSave,
                "AutoSave" => SaveFileType::AutoSave,
                _ => SaveFileType::Normal,
            };
            return Ok(SaveGameInfo {
                filename: String::new(),
                display_name: header
                    .get_metadata("display_name")
                    .cloned()
                    .unwrap_or_default(),
                description: header
                    .get_metadata("description")
                    .cloned()
                    .unwrap_or_default(),
                map_name: header.map_name.clone(),
                campaign_side: header.get_metadata("campaign_side").cloned(),
                mission_number: header
                    .get_metadata("mission_number")
                    .and_then(|s| s.parse().ok()),
                save_date: UNIX_EPOCH + Duration::from_secs(header.timestamp),
                game_version: header
                    .get_metadata("game_version")
                    .cloned()
                    .unwrap_or_default(),
                play_time: Duration::from_secs_f32(header.elapsed_time.max(0.0)),
                difficulty,
                save_type,
            });
        }
        pos = end;
    }

    Err(SaveLoadError::Corrupted(
        "CHUNK_GameState not found in named-chunk save".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::save_game::GameState as CommonGameState;
    use game_engine::common::system::xfer::Xfer as CommonXfer;
    use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
    use game_engine::{Snapshot, Xfer, XferSave, XferStatus};
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_ascii_token(out: &mut Vec<u8>, token: &str) {
        out.push(token.len() as u8);
        out.extend_from_slice(token.as_bytes());
    }

    #[test]
    fn get_save_info_from_file_walks_synthetic_named_chunks() {
        // C++ GameState::getSaveGameInfoFromFile (GameState.cpp:975-1038) walks
        // u8-len tokens + i32 block sizes. Pre-fix Rust bincode'd the first 1024
        // bytes and dropped every CHUNK_*.sav from the save list.
        let mut header = CommonGameState::new(SAVE_FILE_VERSION);
        header.map_name = "SyntheticMap".to_string();
        header.game_mode = "Normal".to_string();
        header.timestamp = 1_700_000_000;
        header.set_metadata("display_name".to_string(), "Chunk Walk".to_string());

        let mut payload = Vec::new();
        {
            let mut xfer = CommonXferSave::new(Cursor::new(&mut payload), SAVE_FILE_VERSION);
            header.xfer(&mut xfer).expect("encode CHUNK_GameState");
        }

        let mut bytes = Vec::new();
        write_ascii_token(&mut bytes, "CHUNK_Campaign");
        bytes.extend_from_slice(&4i32.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        write_ascii_token(&mut bytes, CHUNK_GAME_STATE_TOKEN);
        bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        write_ascii_token(&mut bytes, "SG_EOF");

        let dir = std::env::temp_dir().join(format!(
            "save_info_chunks_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("00000001.sav");
        std::fs::write(&path, &bytes).expect("write synthetic sav");

        let manager = SaveLoadManager::new();
        let info = manager
            .get_save_info_from_file(&path)
            .expect("parse named chunks");
        assert_eq!(info.map_name, "SyntheticMap");
        assert_eq!(info.display_name, "Chunk Walk");
        assert_eq!(info.save_type, SaveFileType::Normal);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn snapshot_version_starts_at_one_like_cpp_placeholder() {
        // C++ empty Snapshot::xfer (StateMachine.h:388) starts `XferVersion v = cv`
        // with cv=1 so save writes 1. Live NullSnapshot now does the same.
        struct VersionSnapshot;
        impl Snapshot for VersionSnapshot {
            fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
                Ok(())
            }
            fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
                let mut version: u8 = 1;
                xfer.xfer_version(&mut version, 1)
            }
            fn load_post_process(&mut self) -> Result<(), XferStatus> {
                Ok(())
            }
        }

        let dir = std::env::temp_dir().join(format!(
            "null_snapshot_ver_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("version.bin");
        let mut snapshot = VersionSnapshot;
        let mut xfer = XferSave::new();
        xfer.open(path.to_string_lossy().into_owned())
            .expect("open");
        snapshot.xfer(&mut xfer).expect("xfer version");
        xfer.close().expect("close");
        let bytes = std::fs::read(&path).expect("read");
        assert_eq!(bytes, vec![1u8], "placeholder snapshot must write version 1");
        let _ = std::fs::remove_dir_all(dir);
    }
}
