use crate::game_logic::GameLogic;
use crate::save_load::*;
use game_engine::common::system::save_game::GameState as CommonGameState;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Save file format header
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveFileHeader {
    pub magic: [u8; 4],         // "GZHS" (Generals Zero Hour Save)
    pub version: u32,           // Save format version
    pub flags: u32,             // Compression, encryption, etc.
    pub timestamp: u64,         // Unix timestamp
    pub checksum: u32,          // CRC32 of save data
    pub uncompressed_size: u64, // Original data size
    pub compressed_size: u64,   // Compressed data size
    pub game_version: [u8; 16], // Game version string
    pub reserved: [u8; 32],     // Reserved for future use
}

const SAVE_MAGIC: [u8; 4] = *b"GZHS";
const SAVE_HEADER_SIZE: usize = std::mem::size_of::<SaveFileHeader>();

impl SaveFileHeader {
    pub fn new() -> Self {
        Self {
            magic: SAVE_MAGIC,
            version: SAVE_FILE_VERSION,
            flags: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checksum: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            game_version: [0; 16],
            reserved: [0; 32],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == SAVE_MAGIC && self.version <= SAVE_FILE_VERSION
    }

    pub fn is_compressed(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    pub fn set_compressed(&mut self, compressed: bool) {
        if compressed {
            self.flags |= 0x01;
        } else {
            self.flags &= !0x01;
        }
    }
}

impl Default for SaveFileHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Save file section types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveFileSection {
    Header,
    GameInfo,
    WorldState,
    PlayerStates,
    AIStates,
    MapState,
    Scripts,
    EndMarker,
}

/// Save file manager
pub struct SaveFileManager {
    save_directory: PathBuf,
    temp_directory: PathBuf,
    auto_save_interval: std::time::Duration,
    max_save_files: usize,
    last_auto_save: SystemTime,
}

impl SaveFileManager {
    pub fn new() -> Self {
        let save_dir = SaveLoadManager::default_save_directory();
        Self::with_save_directory(save_dir)
    }

    pub fn with_save_directory(save_directory: impl Into<PathBuf>) -> Self {
        let save_dir = save_directory.into();
        let mut temp_dir = save_dir.clone();
        temp_dir.push("temp");

        Self {
            save_directory: save_dir,
            temp_directory: temp_dir,
            auto_save_interval: std::time::Duration::from_secs(300), // 5 minutes
            max_save_files: MAX_SAVE_SLOTS,
            last_auto_save: SystemTime::now(),
        }
    }

    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Create directories if they don't exist
        std::fs::create_dir_all(&self.save_directory)?;
        std::fs::create_dir_all(&self.temp_directory)?;

        // Clean up old temporary files
        self.cleanup_temp_files()?;

        Ok(())
    }

    /// Save game to file
    pub fn save_game(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        let save_path = self.get_save_path(filename);
        let temp_path = self.get_temp_path(&format!("{}_temp", filename));

        // Create snapshot of current game state
        let snapshot_builder = SnapshotBuilder::new();
        let world_snapshot = snapshot_builder.create_world_snapshot(game_logic)?;

        // Save to temporary file first
        self.save_to_file(&temp_path, &world_snapshot, save_info)?;

        // Atomically move temp file to final location
        std::fs::rename(&temp_path, &save_path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            SaveLoadError::Io(e)
        })?;

        self.enforce_save_limit()?;
        log::info!("Game saved successfully to: {}", save_path.display());
        Ok(())
    }

    /// Load game from file
    pub fn load_game(
        &mut self,
        filename: &str,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<SaveGameInfo> {
        let save_path = self.get_save_path(filename);

        if !save_path.exists() {
            return Err(SaveLoadError::FileNotFound(filename.to_string()));
        }

        // Load from file
        let (world_snapshot, save_info) = self.load_from_file(&save_path)?;

        // Restore game state
        let snapshot_builder = SnapshotBuilder::new();
        snapshot_builder.restore_from_snapshot(&world_snapshot, game_logic)?;

        log::info!("Game loaded successfully from: {}", save_path.display());
        Ok(save_info)
    }

    /// Quick save to slot 0
    pub fn quick_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<()> {
        let save_info = SaveGameInfo {
            filename: "quicksave".to_string(),
            display_name: "Quick Save".to_string(),
            description: "Quick save".to_string(),
            map_name: "Unknown".to_string(), // Would get from game state
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs(0), // Would track actual play time
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::QuickSave,
        };

        self.save_game("quicksave", game_logic, &save_info)
    }

    /// Auto save if enough time has passed
    pub fn try_auto_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<bool> {
        let now = SystemTime::now();
        if now.duration_since(self.last_auto_save).unwrap_or_default() >= self.auto_save_interval {
            self.auto_save(game_logic)?;
            self.last_auto_save = now;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Auto save
    pub fn auto_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let save_info = SaveGameInfo {
            filename: format!("autosave_{}", timestamp),
            display_name: "Auto Save".to_string(),
            description: format!("Automatic save at {}", timestamp),
            map_name: "Unknown".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::AutoSave,
        };

        let filename = &save_info.filename;
        self.save_game(filename, game_logic, &save_info)?;

        // Clean up old auto saves
        self.cleanup_old_auto_saves()?;

        Ok(())
    }

    /// Delete save file
    pub fn delete_save(&self, filename: &str) -> SaveLoadResult<()> {
        let save_path = self.get_save_path(filename);

        if save_path.exists() {
            std::fs::remove_file(&save_path)?;
            log::info!("Deleted save file: {}", save_path.display());
        }

        Ok(())
    }

    /// Check if save file exists
    pub fn save_exists(&self, filename: &str) -> bool {
        self.get_save_path(filename).exists()
    }

    /// Get save file info without loading the entire file
    pub fn get_save_info(&self, filename: &str) -> SaveLoadResult<SaveGameInfo> {
        let save_path = self.get_save_path(filename);
        let file = File::open(&save_path)?;
        let mut reader = BufReader::new(file);

        // Read and validate header
        let header = self.read_header(&mut reader)?;
        if !header.is_valid() {
            return Err(SaveLoadError::InvalidFormat);
        }

        // Read save info section
        self.read_save_info(&mut reader, &header)
    }

    /// List all available save files
    pub fn list_saves(&self) -> SaveLoadResult<Vec<AvailableGameInfo>> {
        let mut saves = Vec::new();

        let entries = std::fs::read_dir(&self.save_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == SAVE_EXTENSION {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        match self.get_save_info(filename) {
                            Ok(save_info) => {
                                saves.push(AvailableGameInfo {
                                    filename: filename.to_string(),
                                    save_info,
                                });
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to read save info from {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        // Sort by save date, newest first
        saves.sort_by(|a, b| b.save_info.save_date.cmp(&a.save_info.save_date));

        Ok(saves)
    }

    /// Save data to file with compression
    fn save_to_file(
        &self,
        path: &Path,
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);

        // Canonical save payload now goes through Common SaveGame + Xfer.
        let encoded_state = Self::encode_common_game_state(world_snapshot, save_info)?;

        // Compress data
        let compressed = compression::compress(&encoded_state)?;
        let is_compressed = compressed.len() < encoded_state.len();

        // Create header
        let mut header = SaveFileHeader::new();
        header.set_compressed(is_compressed);
        header.uncompressed_size = encoded_state.len() as u64;
        header.compressed_size = if is_compressed {
            compressed.len()
        } else {
            encoded_state.len()
        } as u64;
        header.checksum = crc32fast::hash(if is_compressed {
            &compressed
        } else {
            &encoded_state
        });

        // Write header
        let header_bytes =
            bincode::serialize(&header).map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        writer.write_all(&header_bytes)?;

        // Write save info
        let save_info_bytes = bincode::serialize(save_info)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        let save_info_size = save_info_bytes.len() as u32;
        writer.write_all(&save_info_size.to_le_bytes())?;
        writer.write_all(&save_info_bytes)?;

        // Write world data
        writer.write_all(if is_compressed {
            &compressed
        } else {
            &encoded_state
        })?;

        writer.flush()?;

        Ok(())
    }

    /// Load data from file with decompression
    fn load_from_file(&self, path: &Path) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read and validate header
        let header = self.read_header(&mut reader)?;
        if !header.is_valid() {
            return Err(SaveLoadError::InvalidFormat);
        }

        if header.version > SAVE_FILE_VERSION {
            return Err(SaveLoadError::VersionMismatch {
                expected: SAVE_FILE_VERSION,
                actual: header.version,
            });
        }

        // Read save info
        let save_info = self.read_save_info(&mut reader, &header)?;

        // Read world data
        let mut world_data = Vec::with_capacity(header.compressed_size as usize);
        reader.read_to_end(&mut world_data)?;

        // Verify checksum
        let actual_checksum = crc32fast::hash(&world_data);
        if actual_checksum != header.checksum {
            return Err(SaveLoadError::Corrupted(format!(
                "Checksum mismatch: expected {}, got {}",
                header.checksum, actual_checksum
            )));
        }

        // Decompress if needed
        let decompressed = if header.is_compressed() {
            compression::decompress(&world_data)?
        } else {
            world_data
        };

        // Prefer canonical Common SaveGame payload; fall back to legacy payload.
        let world_snapshot = match Self::decode_common_game_state(&decompressed) {
            Ok(common_state) => bincode::deserialize::<WorldSnapshot>(&common_state.data)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?,
            Err(common_err) => {
                log::warn!(
                    "Common SaveGame payload decode failed ({}), falling back to legacy snapshot payload",
                    common_err
                );
                bincode::deserialize::<WorldSnapshot>(&decompressed)
                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))?
            }
        };

        Ok((world_snapshot, save_info))
    }

    fn encode_common_game_state(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<Vec<u8>> {
        let mut state = CommonGameState::new(SAVE_FILE_VERSION);
        state.timestamp = save_info
            .save_date
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        state.map_name = save_info.map_name.clone();
        state.game_mode = format!("{:?}", save_info.save_type);
        state.player_count = world_snapshot.players.len() as u32;
        state.current_frame = u32::try_from(world_snapshot.frame_number).unwrap_or(u32::MAX);
        state.elapsed_time = save_info.play_time.as_secs_f32();
        state.set_metadata("display_name".to_string(), save_info.display_name.clone());
        state.set_metadata("description".to_string(), save_info.description.clone());
        state.set_metadata("game_version".to_string(), save_info.game_version.clone());
        state.set_metadata(
            "difficulty".to_string(),
            format!("{:?}", save_info.difficulty),
        );
        if let Some(side) = &save_info.campaign_side {
            state.set_metadata("campaign_side".to_string(), side.clone());
        }
        if let Some(mission_number) = save_info.mission_number {
            state.set_metadata("mission_number".to_string(), mission_number.to_string());
        }

        state.data = bincode::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        let mut cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            state
                .xfer(&mut xfer)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }
        Ok(cursor.into_inner())
    }

    fn decode_common_game_state(data: &[u8]) -> SaveLoadResult<CommonGameState> {
        let mut state = CommonGameState::default();
        let mut xfer = CommonXferLoad::new(Cursor::new(data), SAVE_FILE_VERSION);
        state
            .xfer(&mut xfer)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Ok(state)
    }

    /// Read file header
    fn read_header<R: Read>(&self, reader: &mut R) -> SaveLoadResult<SaveFileHeader> {
        let mut header_bytes = vec![0u8; SAVE_HEADER_SIZE];
        reader.read_exact(&mut header_bytes)?;

        let header: SaveFileHeader = bincode::deserialize(&header_bytes)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(header)
    }

    /// Read save info section
    fn read_save_info<R: Read>(
        &self,
        reader: &mut R,
        _header: &SaveFileHeader,
    ) -> SaveLoadResult<SaveGameInfo> {
        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        let size = u32::from_le_bytes(size_bytes) as usize;

        let mut info_bytes = vec![0u8; size];
        reader.read_exact(&mut info_bytes)?;

        let save_info: SaveGameInfo = bincode::deserialize(&info_bytes)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(save_info)
    }

    /// Get full path for save file
    pub fn get_save_path(&self, filename: &str) -> PathBuf {
        let mut path = self.save_directory.clone();
        path.push(format!("{}.{}", filename, SAVE_EXTENSION));
        path
    }

    /// Get temporary file path
    fn get_temp_path(&self, filename: &str) -> PathBuf {
        let mut path = self.temp_directory.clone();
        path.push(format!("{}.tmp", filename));
        path
    }

    /// Clean up temporary files
    fn cleanup_temp_files(&self) -> SaveLoadResult<()> {
        if !self.temp_directory.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.temp_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == "tmp" {
                    if let Err(e) = std::fs::remove_file(&path) {
                        log::warn!("Failed to remove temp file {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up old auto save files
    fn cleanup_old_auto_saves(&self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        let auto_saves: Vec<_> = saves
            .into_iter()
            .filter(|s| s.save_info.save_type == SaveFileType::AutoSave)
            .collect();

        // Keep only the 5 most recent auto saves
        if auto_saves.len() > 5 {
            for old_save in &auto_saves[5..] {
                if let Err(e) = self.delete_save(&old_save.filename) {
                    log::warn!(
                        "Failed to delete old auto save {}: {}",
                        old_save.filename,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    fn enforce_save_limit(&self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        if saves.len() <= self.max_save_files {
            return Ok(());
        }

        for old_save in saves.iter().skip(self.max_save_files) {
            if let Err(e) = self.delete_save(&old_save.filename) {
                log::warn!(
                    "Failed to delete excess save {} while enforcing limit: {}",
                    old_save.filename,
                    e
                );
            }
        }

        Ok(())
    }
}

impl Default for SaveFileManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global save file manager instance
lazy_static::lazy_static! {
    pub static ref SAVE_FILE_MANAGER: std::sync::Mutex<SaveFileManager> =
        std::sync::Mutex::new(SaveFileManager::new());
}

/// Initialize the global save file system
pub fn init_save_file_system() -> SaveLoadResult<()> {
    let mut manager = SAVE_FILE_MANAGER.lock().unwrap_or_else(|e| e.into_inner());
    manager.init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_header_serialization() {
        let mut header = SaveFileHeader::new();
        header.set_compressed(true);
        header.uncompressed_size = 12345;
        header.compressed_size = 6789;

        let serialized = bincode::serialize(&header).unwrap();
        let deserialized: SaveFileHeader = bincode::deserialize(&serialized).unwrap();

        assert_eq!(header.magic, deserialized.magic);
        assert_eq!(header.version, deserialized.version);
        assert_eq!(header.uncompressed_size, deserialized.uncompressed_size);
        assert_eq!(header.compressed_size, deserialized.compressed_size);
        assert!(deserialized.is_compressed());
        assert!(deserialized.is_valid());
    }

    #[test]
    fn test_save_file_paths() {
        let manager = SaveFileManager::new();

        let save_path = manager.get_save_path("test_save");
        assert!(save_path.to_string_lossy().contains("test_save"));
        assert!(save_path
            .to_string_lossy()
            .ends_with(&format!(".{}", SAVE_EXTENSION)));

        let temp_path = manager.get_temp_path("test_temp");
        assert!(temp_path.to_string_lossy().contains("test_temp"));
        assert!(temp_path.to_string_lossy().ends_with(".tmp"));
    }
}
