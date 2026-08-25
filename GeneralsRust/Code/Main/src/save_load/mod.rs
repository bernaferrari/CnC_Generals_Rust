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

/// C++ `UnicodeString::format(TheGameText->fetch("GUI:MissionSave"), label, n)`.
pub fn format_mission_save_description(
    format: &str,
    campaign_label: &str,
    mission_number: i32,
) -> String {
    game_engine::System::SaveGame::format_mission_save_description(
        format,
        campaign_label,
        mission_number,
    )
}

/// C++ `GameState::xfer` v2 header refresh (`GameState.cpp:1614-1640`).
/// Campaign side is `TheCampaignManager` campaign `m_name`; mission number is
/// 0-based `getCurrentMissionNumber()`. No campaign → empty + INVALID.
pub fn campaign_header_from_campaign_manager() -> (Option<String>, Option<u32>) {
    #[cfg(feature = "game_client")]
    {
        let manager = game_client::gui::campaign_manager::get_campaign_manager();
        let Some(campaign) = manager.get_current_campaign() else {
            return (None, None);
        };
        let side = campaign.name.clone();
        let number = manager.get_current_mission_number();
        (
            if side.is_empty() { None } else { Some(side) },
            number.filter(|&n| n >= 0).map(|n| n as u32),
        )
    }
    #[cfg(not(feature = "game_client"))]
    {
        (None, None)
    }
}

/// C++ `PopupSaveLoad::setEditDescription` map-leaf strip (`PopupSaveLoad.cpp:438-452`).
pub fn normalize_default_save_description_from_map_name(mut default_desc: String) -> String {
    if let Some(pos) = default_desc.rfind('\\') {
        default_desc = default_desc[pos + 1..].to_string();
    }
    let char_len = default_desc.chars().count();
    if char_len >= 4 && default_desc.chars().nth(char_len - 4) == Some('.') {
        for _ in 0..4 {
            let _ = default_desc.pop();
        }
    }
    default_desc
}

/// Localized between-mission save description (`GameState.cpp:616-618`).
pub fn current_mission_save_description() -> String {
    #[cfg(feature = "game_client")]
    {
        let manager = game_client::gui::campaign_manager::get_campaign_manager();
        let Some(campaign) = manager.get_current_campaign() else {
            return String::new();
        };
        let mission_number = manager.get_current_mission_number().unwrap_or(0) + 1;
        let (text, exists) =
            game_client::game_text::GameText::fetch_with_exists(&campaign.campaign_name_label);
        let campaign_label = if exists && !text.is_empty() {
            text
        } else if !campaign.campaign_name_label.is_empty() {
            campaign.campaign_name_label.clone()
        } else {
            campaign.name.clone()
        };
        let (format, exists) =
            game_client::game_text::GameText::fetch_with_exists("GUI:MissionSave");
        // Retail English token when the CSF label is missing.
        let format = if exists && format.contains('%') {
            format
        } else {
            "%s Mission %d".to_string()
        };
        return format_mission_save_description(&format, &campaign_label, mission_number);
    }
    #[cfg(not(feature = "game_client"))]
    String::new()
}

/// C++ `setEditDescription` (`PopupSaveLoad.cpp:422-457`): campaign +
/// `missionNumber+1`, else the map leaf without a four-char extension.
pub fn default_save_edit_description(map_name: &str) -> String {
    #[cfg(feature = "game_client")]
    {
        let manager = game_client::gui::campaign_manager::get_campaign_manager();
        if let (Some(campaign), Some(mission_number)) = (
            manager.get_current_campaign(),
            manager.get_current_mission_number(),
        ) {
            let campaign_label =
                game_client::game_text::GameText::fetch(&campaign.campaign_name_label);
            let label = if campaign_label.is_empty() {
                if !campaign.campaign_name_label.is_empty() {
                    campaign.campaign_name_label.clone()
                } else {
                    campaign.name.clone()
                }
            } else {
                campaign_label
            };
            return format!("{} {}", label, mission_number + 1);
        }
    }
    let map = map_name.trim();
    if map.is_empty() {
        #[cfg(feature = "game_client")]
        {
            if let Some(data) = game_engine::common::ini::ini_game_data::get_global_data() {
                let name = data.read().map_name.clone();
                if !name.trim().is_empty() {
                    return normalize_default_save_description_from_map_name(name);
                }
            }
        }
        return String::new();
    }
    normalize_default_save_description_from_map_name(map.to_string())
}

/// C++ listbox columns 1/2 (`GameState.cpp:1176-1206`) from local save date.
pub fn format_save_list_date_time(time: SystemTime) -> (String, String) {
    let date = game_engine::System::SaveDate::from_local_time(time);
    (
        format!("{:02}:{:02}", date.hour, date.minute),
        format!("{:04}-{:02}-{:02}", date.year, date.month, date.day),
    )
}

/// C++ `MessageBoxOk(GUI:Error, format(GUI:ErrorLoadingGame, filepath))`.
pub fn format_error_loading_game(filepath: &str) -> (String, String) {
    #[cfg(feature = "game_client")]
    {
        let template = game_client::game_text::GameText::fetch("GUI:ErrorLoadingGame");
        let body = if template.contains("%s") {
            template.replacen("%s", filepath, 1)
        } else if template.is_empty() {
            filepath.to_string()
        } else {
            format!("{template} {filepath}")
        };
        let title = game_client::game_text::GameText::fetch("GUI:Error");
        return (title, body);
    }
    #[cfg(not(feature = "game_client"))]
    (
        "GUI:Error".to_string(),
        format!("GUI:ErrorLoadingGame {filepath}"),
    )
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
    // C++ `GameState::getSaveGameInfoFromFile` xfers only CHUNK_GameState
    // with `GameState::xfer` v2. Host writes that header (version byte 2).
    crate::save_load::save_file::parse_named_chunk_save_info(data)
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
    fn save_load_menu_lists_host_cpp_v2_game_state() {
        use crate::save_load::save_file::SaveFileManager;
        let dir = std::env::temp_dir().join(format!(
            "menu_v2_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut files = SaveFileManager::with_save_directory(&dir);
        let save_info = SaveGameInfo {
            filename: "menu_v2".into(),
            display_name: "Host V2".into(),
            description: "Host V2".into(),
            map_name: "Maps\\Alpine Assault.map".into(),
            campaign_side: Some("America".into()),
            mission_number: Some(2),
            save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
            game_version: String::new(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        files
            .save_game("menu_v2", &crate::game_logic::GameLogic::new(), &save_info)
            .expect("write host v2 sav");
        let path = dir.join("menu_v2.sav");
        let manager = SaveLoadManager::new();
        let info = manager
            .get_save_info_from_file(&path)
            .expect("SaveLoadMenu must list host v2 CHUNK_GameState");
        assert_eq!(info.description, "Host V2");
        assert_eq!(info.save_type, SaveFileType::Normal);
        assert_eq!(info.campaign_side.as_deref(), Some("America"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn campaign_header_reads_campaign_manager_or_invalid() {
        // C++ GameState.cpp:1614-1640 refreshes v2 header from TheCampaignManager.
        let previous = {
            let manager = game_client::gui::campaign_manager::get_campaign_manager();
            (
                manager.get_current_campaign().map(|c| c.name.clone()),
                manager.get_current_mission().map(|m| m.name.clone()),
            )
        };

        {
            let mut manager = game_client::gui::campaign_manager::get_campaign_manager();
            manager.set_campaign("");
        }
        assert_eq!(
            campaign_header_from_campaign_manager(),
            (None, None),
            "no campaign must write empty side + INVALID_MISSION_NUMBER"
        );

        {
            let mut manager = game_client::gui::campaign_manager::get_campaign_manager();
            let campaign = manager.new_campaign("W31SaveDAmerica".to_string());
            campaign.first_mission = "m1".to_string();
            let first = campaign.new_mission("M1".to_string());
            first.next_mission = "m2".to_string();
            campaign.new_mission("M2".to_string());
            manager.set_campaign("W31SaveDAmerica");
            manager.goto_next_mission();
        }
        let (side, number) = campaign_header_from_campaign_manager();
        assert_eq!(side.as_deref(), Some("w31savedamerica"));
        assert_eq!(number, Some(1), "mission index is 0-based like C++");

        {
            let mut manager = game_client::gui::campaign_manager::get_campaign_manager();
            match previous {
                (Some(campaign), Some(mission)) => {
                    manager.set_campaign_and_mission(&campaign, &mission);
                }
                (Some(campaign), None) => manager.set_campaign(&campaign),
                _ => manager.set_campaign(""),
            }
        }
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
        assert_eq!(
            bytes,
            vec![1u8],
            "placeholder snapshot must write version 1"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mission_save_description_formats_gui_mission_save_like_cpp() {
        assert_eq!(
            format_mission_save_description("%s Mission %d", "USA Campaign", 2),
            "USA Campaign Mission 2"
        );
        assert_eq!(
            normalize_default_save_description_from_map_name(
                "Maps\\USA\\Mission01.map".to_string()
            ),
            "Mission01"
        );
        let (time, date) = format_save_list_date_time(UNIX_EPOCH);
        assert!(time.contains(':'), "list time column: {time}");
        assert!(date.contains('-'), "list date column: {date}");
        let (title, body) = format_error_loading_game("Save\\00000001.sav");
        assert!(title.contains("Error") || title == "GUI:Error", "{title}");
        assert!(
            body.contains("00000001.sav") || body.contains("ErrorLoadingGame"),
            "{body}"
        );
    }
}
