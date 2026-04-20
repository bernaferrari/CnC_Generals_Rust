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
pub const SAVE_EXTENSION: &str = "gen";
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

    /// Get the default save directory
    pub fn default_save_directory() -> PathBuf {
        if let Ok(mut path) = std::env::current_exe() {
            path.pop(); // Remove executable name
            path.push("Save Games");
            path
        } else {
            PathBuf::from("Save Games")
        }
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

    /// Get save file information from file
    pub fn get_save_info_from_file(&self, path: &PathBuf) -> SaveLoadResult<SaveGameInfo> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Try to decompress if needed
        let data = if compression::is_compressed(&buffer)? {
            compression::decompress(&buffer)?
        } else {
            buffer
        };

        // Parse save header
        let save_info =
            bincode::deserialize::<SaveGameInfo>(&data[..std::cmp::min(1024, data.len())])
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(save_info)
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
