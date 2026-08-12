use crate::game_logic::GameLogic;
use crate::save_load::*;
use game_engine::common::system::save_game::GameState as CommonGameState;
use game_engine::common::system::xfer::Xfer as CommonXfer;
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
/// Same tokens Popup / Common TheGameState write (`SG_EOF`, CHUNK_*).
///
/// Honest host path (Phase N5): this is **not** the C++ 17-block TheGameState
/// snapshot table and **not** crate `GameLogic::xfer`. Host pause-save writes
/// `CHUNK_GameState` (header metadata) + `CHUNK_GameLogic` + `SG_EOF`.
/// `CHUNK_GameLogic` payload bytes are `bincode::serialize(WorldSnapshot)`.
const CHUNK_GAME_STATE: &str = "CHUNK_GameState";
const CHUNK_GAME_LOGIC: &str = "CHUNK_GameLogic";
const SAVE_FILE_EOF: &str = "SG_EOF";

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

    /// Directory used for list/save/load. Default is `UserData/Save` (Popup + host).
    pub fn save_directory(&self) -> &Path {
        &self.save_directory
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
        let (world_snapshot, save_info) = self.load_game_snapshot(filename)?;
        self.restore_game_snapshot(&world_snapshot, game_logic)?;

        log::info!("Game loaded successfully from save slot: {}", filename);
        Ok(save_info)
    }

    /// Decode a save without mutating a live `GameLogic` instance.
    ///
    /// The runtime host uses this to load the saved map into a staging world
    /// before it restores the snapshot.  Keeping the decode separate from the
    /// restore means a bad/missing map or corrupted snapshot cannot partially
    /// overwrite the currently playable match.
    pub fn load_game_snapshot(
        &self,
        filename: &str,
    ) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
        let mut save_path = self.get_save_path(filename);

        if !save_path.exists() {
            let mut legacy = self.save_directory.clone();
            legacy.push(format!("{}.{}", filename, LEGACY_SAVE_EXTENSION));
            if legacy.exists() {
                save_path = legacy;
            } else {
                return Err(SaveLoadError::FileNotFound(filename.to_string()));
            }
        }

        self.load_from_file(&save_path)
    }

    /// Restore a previously decoded snapshot into the supplied world.
    ///
    /// Callers that need transactional map identity handling should restore
    /// into a fresh staging world and install it only after this succeeds.
    pub fn restore_game_snapshot(
        &self,
        world_snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let snapshot_builder = SnapshotBuilder::new();
        snapshot_builder.restore_from_snapshot(world_snapshot, game_logic)
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
        if !save_path.exists() {
            let mut legacy = self.save_directory.clone();
            legacy.push(format!("{}.{}", filename, LEGACY_SAVE_EXTENSION));
            if legacy.exists() {
                return self.get_save_info_from_path(&legacy);
            }
        }
        self.get_save_info_from_path(&save_path)
    }

    fn get_save_info_from_path(&self, save_path: &Path) -> SaveLoadResult<SaveGameInfo> {
        let mut file = File::open(save_path)?;
        let mut all = Vec::new();
        file.read_to_end(&mut all)?;
        if Self::looks_like_common_sav_chunks(&all) {
            return Self::read_common_sav_chunks(&all).map(|(_, info)| info);
        }
        let mut reader = Cursor::new(all);
        let header = self.read_header(&mut reader)?;
        if !header.is_valid() {
            return Err(SaveLoadError::InvalidFormat);
        }
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
                if extension == SAVE_EXTENSION || extension == LEGACY_SAVE_EXTENSION {
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

    /// Save data to file as Common `.sav` chunks (same tokens as Popup).
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
        let encoded = Self::write_common_sav_chunks(world_snapshot, save_info)?;
        writer.write_all(&encoded)?;
        writer.flush()?;
        Ok(())
    }

    /// Load data from file. Prefers Common `.sav` chunks; falls back to GZHS `.gen`.
    fn load_from_file(&self, path: &Path) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
        let mut file = File::open(path)?;
        let mut all = Vec::new();
        file.read_to_end(&mut all)?;

        if Self::looks_like_common_sav_chunks(&all) {
            return Self::read_common_sav_chunks(&all);
        }

        let mut reader = Cursor::new(all);
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

        let save_info = self.read_save_info(&mut reader, &header)?;
        let mut world_data = Vec::with_capacity(header.compressed_size as usize);
        reader.read_to_end(&mut world_data)?;

        let actual_checksum = crc32fast::hash(&world_data);
        if actual_checksum != header.checksum {
            return Err(SaveLoadError::Corrupted(format!(
                "Checksum mismatch: expected {}, got {}",
                header.checksum, actual_checksum
            )));
        }

        let decompressed = if header.is_compressed() {
            compression::decompress(&world_data)?
        } else {
            world_data
        };

        let world_snapshot = match Self::decode_common_game_state(&decompressed) {
            Ok(common_state) => match Self::decode_world_snapshot_payload(&common_state.data) {
                Ok(snapshot) => snapshot,
                Err(common_payload_err) => {
                    // Raw legacy `.gen` bincode can happen to satisfy enough
                    // CommonXfer framing to produce an empty/invalid
                    // GameState.  Only commit to the wrapper route after its
                    // nested WorldSnapshot has decoded; otherwise retry the
                    // original raw payload.
                    log::warn!(
                        "GZHS Common SaveGame payload did not contain a valid WorldSnapshot ({}); falling back to raw legacy snapshot payload",
                        common_payload_err
                    );
                    Self::decode_world_snapshot_payload(&decompressed)?
                }
            },
            Err(common_err) => {
                log::warn!(
                    "Common SaveGame payload decode failed ({}), falling back to legacy snapshot payload",
                    common_err
                );
                Self::decode_world_snapshot_payload(&decompressed)?
            }
        };

        Ok((world_snapshot, save_info))
    }

    fn looks_like_common_sav_chunks(data: &[u8]) -> bool {
        data.windows(CHUNK_GAME_STATE.len())
            .any(|w| w == CHUNK_GAME_STATE.as_bytes())
            || data
                .windows(SAVE_FILE_EOF.len())
                .any(|w| w == SAVE_FILE_EOF.as_bytes())
    }

    /// Popup + host container: CHUNK_GameState / CHUNK_GameLogic / SG_EOF.
    ///
    /// `CHUNK_GameLogic` is a Common `GameState` block whose `data` field is a
    /// **bincode `WorldSnapshot`**, not crate `GameLogic::xfer` and not the
    /// full C++ 17 named-chunk TheGameState table.
    fn write_common_sav_chunks(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<Vec<u8>> {
        let logic_payload = bincode::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Self::write_common_sav_chunks_with_payload(world_snapshot, save_info, logic_payload)
    }

    /// Shared Common container writer.  Keeping the logic payload separate
    /// makes the outer format independent of the positional WorldSnapshot
    /// schema and lets the regression fixture exercise a historical payload.
    fn write_common_sav_chunks_with_payload(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
    ) -> SaveLoadResult<Vec<u8>> {
        let mut header = Self::common_state_from_save_info(save_info, world_snapshot);
        header.data.clear();
        let mut logic = CommonGameState::new(SAVE_FILE_VERSION);
        logic.data = logic_payload;

        let mut cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            let mut name = CHUNK_GAME_STATE.to_string();
            xfer.xfer_ascii_string(&mut name)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            xfer.begin_block()
                .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;
            header
                .xfer(&mut xfer)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            xfer.end_block()
                .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;

            let mut name = CHUNK_GAME_LOGIC.to_string();
            xfer.xfer_ascii_string(&mut name)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            xfer.begin_block()
                .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;
            logic
                .xfer(&mut xfer)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            xfer.end_block()
                .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;

            let mut eof = SAVE_FILE_EOF.to_string();
            xfer.xfer_ascii_string(&mut eof)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }
        Ok(cursor.into_inner())
    }

    fn read_common_sav_chunks(data: &[u8]) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
        let mut cursor = Cursor::new(data);
        let mut xfer = CommonXferLoad::new(&mut cursor, SAVE_FILE_VERSION);
        let mut header_state = CommonGameState::default();
        let mut logic_data: Option<Vec<u8>> = None;
        loop {
            let mut token = String::new();
            xfer.xfer_ascii_string(&mut token)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            if token.eq_ignore_ascii_case(SAVE_FILE_EOF) {
                break;
            }
            let _ = xfer
                .begin_block()
                .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;
            let mut block = CommonGameState::default();
            block
                .xfer(&mut xfer)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
            let _ = xfer.end_block();
            if token.eq_ignore_ascii_case(CHUNK_GAME_STATE) {
                header_state = block;
            } else if token.eq_ignore_ascii_case(CHUNK_GAME_LOGIC) {
                logic_data = Some(block.data);
            }
        }
        let payload = logic_data.ok_or_else(|| {
            SaveLoadError::Corrupted("CHUNK_GameLogic missing from Common .sav".to_string())
        })?;
        let world_snapshot = Self::decode_world_snapshot_payload(&payload)?;
        let save_info = Self::save_info_from_common_state(&header_state, &world_snapshot);
        Ok((world_snapshot, save_info))
    }

    /// Decode the positional bincode payload shared by Common `.sav` chunks,
    /// GZHS-wrapped Common state, and the original raw `.gen` fallback.
    ///
    /// Production snapshot fields were appended inside nested records, so this
    /// must go through the exact v1 mirror instead of relying on serde defaults
    /// at each outer container call site.
    fn decode_world_snapshot_payload(payload: &[u8]) -> SaveLoadResult<WorldSnapshot> {
        let (snapshot, path) = decode_bincode_world_snapshot(payload)?;
        if matches!(path, BincodeWorldSnapshotDecodePath::LegacyProductionV1) {
            log::info!(
                "Migrated legacy production bincode snapshot into schema v{}",
                WORLD_SNAPSHOT_BINCODE_VERSION
            );
        }
        Ok(snapshot)
    }

    fn common_state_from_save_info(
        save_info: &SaveGameInfo,
        world_snapshot: &WorldSnapshot,
    ) -> CommonGameState {
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
        state
    }

    fn save_info_from_common_state(
        state: &CommonGameState,
        _world_snapshot: &WorldSnapshot,
    ) -> SaveGameInfo {
        let difficulty = match state
            .get_metadata("difficulty")
            .map(|s| s.as_str())
            .unwrap_or("Medium")
        {
            "Easy" => GameDifficulty::Easy,
            "Hard" => GameDifficulty::Hard,
            _ => GameDifficulty::Medium,
        };
        let save_type = match state.game_mode.as_str() {
            "Mission" => SaveFileType::Mission,
            "QuickSave" => SaveFileType::QuickSave,
            "AutoSave" => SaveFileType::AutoSave,
            _ => SaveFileType::Normal,
        };
        SaveGameInfo {
            filename: String::new(),
            display_name: state
                .get_metadata("display_name")
                .cloned()
                .unwrap_or_default(),
            description: state
                .get_metadata("description")
                .cloned()
                .unwrap_or_default(),
            map_name: state.map_name.clone(),
            campaign_side: state.get_metadata("campaign_side").cloned(),
            mission_number: state
                .get_metadata("mission_number")
                .and_then(|s| s.parse().ok()),
            save_date: UNIX_EPOCH + std::time::Duration::from_secs(state.timestamp),
            game_version: state
                .get_metadata("game_version")
                .cloned()
                .unwrap_or_default(),
            play_time: std::time::Duration::from_secs_f32(state.elapsed_time.max(0.0)),
            difficulty,
            save_type,
        }
    }

    fn encode_common_game_state(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<Vec<u8>> {
        let logic_payload = bincode::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Self::encode_common_game_state_with_payload(world_snapshot, save_info, logic_payload)
    }

    /// Encode the older GZHS wrapper's Common GameState body around an already
    /// serialized WorldSnapshot payload.
    fn encode_common_game_state_with_payload(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
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

        state.data = logic_payload;

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
    use crate::game_logic::{
        HackerDisableChannelPhase, HackerDisableChannelState, KindOf, ObjectId, Player, Team,
        ThingTemplate,
    };
    use glam::Vec3;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn legacy_production_payload_fixture() -> (
        Vec<u8>,
        ObjectId,
        std::collections::HashMap<String, ThingTemplate>,
    ) {
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "Legacy Player", true));

        let mut barracks = ThingTemplate::new("LegacyBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable);
        source
            .templates
            .insert("LegacyBarracks".to_string(), barracks);

        let mut ranger = ThingTemplate::new("LegacyRanger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_cost(225, 0);
        ranger.build_time = 12.0;
        source.templates.insert("LegacyRanger".to_string(), ranger);

        let barracks_id = source
            .create_object("LegacyBarracks", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("legacy fixture barracks");
        assert!(source.enqueue_production(barracks_id, "LegacyRanger".to_string()));
        {
            let building = source
                .host_object_mut(barracks_id)
                .expect("legacy fixture barracks must remain live");
            let building_data = building
                .building_data
                .as_mut()
                .expect("legacy fixture needs production data");
            // These post-v1 values make an accidental current bincode decode
            // observably wrong while the historical serializer below omits
            // them exactly as an old saved game did.
            building_data.production_queue[0].progress = 4.5;
            building_data.production_queue[0].construction_frames = 135;
            building_data.production_queue[0].quantity_total = 2;
            building_data.production_queue[0].quantity_produced = 1;
            building_data.exit_delay_remaining = 0.3;
            building_data.exit_delay_remaining_frames = 9;
            building_data.exit_burst_remaining = 1;
            building_data.queue_exit_state_initialized = true;
        }

        let snapshot = SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .expect("legacy fixture snapshot");
        let templates = source.templates.clone();
        (
            serialize_legacy_production_v1_fixture(snapshot)
                .expect("serialize exact v1 production fixture"),
            barracks_id,
            templates,
        )
    }

    fn fixture_save_info() -> SaveGameInfo {
        SaveGameInfo {
            filename: "legacy_fixture".to_string(),
            display_name: "Legacy production fixture".to_string(),
            description: "v1 bincode production payload".to_string(),
            map_name: "LegacyMap".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH,
            game_version: "test".to_string(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        }
    }

    fn assert_legacy_production_migrated(snapshot: &WorldSnapshot, barracks_id: ObjectId) {
        assert_eq!(snapshot.version, WORLD_SNAPSHOT_BINCODE_VERSION);
        let object = snapshot
            .objects
            .get(&barracks_id)
            .expect("migrated snapshot must retain its producer");
        let ModuleSnapshot::Production(production) = object
            .modules
            .get("Production")
            .expect("migrated snapshot must retain production module")
        else {
            panic!("migrated producer must use Production module");
        };
        let entry = production
            .production_queue
            .first()
            .expect("migrated producer must retain queue entry");
        assert_eq!(entry.template_name, "LegacyRanger");
        assert!((entry.progress - 4.5).abs() < f32::EPSILON);
        assert_eq!(entry.cost, 225);
        // These values never existed in the historical bincode record.  The
        // first live production tick reconstructs the integer frame counter
        // from `progress`; batch/exit state receives C++ legacy defaults.
        assert_eq!(entry.construction_frames, 0);
        assert_eq!(entry.quantity_total, 1);
        assert_eq!(entry.quantity_produced, 0);
        assert!(!entry.is_upgrade);
        assert_eq!(production.exit_delay_remaining, 0.0);
        assert_eq!(production.exit_delay_remaining_frames, 0);
        assert_eq!(production.exit_burst_remaining, 0);
        assert!(!production.queue_exit_state_initialized);
        assert!(object.hacker_disable_channel.is_none());
    }

    fn gzhs_fixture_bytes(save_info: &SaveGameInfo, decompressed_payload: &[u8]) -> Vec<u8> {
        let mut header = SaveFileHeader::new();
        header.set_compressed(false);
        header.checksum = crc32fast::hash(decompressed_payload);
        header.uncompressed_size = decompressed_payload.len() as u64;
        header.compressed_size = decompressed_payload.len() as u64;

        let mut bytes = bincode::serialize(&header).expect("serialize GZHS header");
        assert!(bytes.len() <= SAVE_HEADER_SIZE);
        // `read_header` reserves the native header footprint before bincode
        // consumes its compact fields; mirror the legacy writer's padding.
        bytes.resize(SAVE_HEADER_SIZE, 0);

        let save_info = bincode::serialize(save_info).expect("serialize GZHS save info");
        bytes.extend_from_slice(&(save_info.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&save_info);
        bytes.extend_from_slice(decompressed_payload);
        bytes
    }

    fn unique_fixture_directory() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "generalsrust-legacy-production-{}-{nonce}",
            std::process::id()
        ))
    }

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

    #[test]
    fn default_save_directory_is_user_data_save_like_popup() {
        let host = SaveLoadManager::default_save_directory();
        let popup = crate::subsystem_manager::resolve_save_directory();
        assert_eq!(
            host, popup,
            "host SaveFileManager and Popup TheGameState must share UserData/Save"
        );
        assert_eq!(host.file_name().and_then(|s| s.to_str()), Some("Save"));
        let manager = SaveFileManager::new();
        assert_eq!(manager.save_directory(), host.as_path());
    }

    #[test]
    fn legacy_bincode_production_payload_migrates_through_every_save_container() {
        let (legacy_payload, barracks_id, templates) = legacy_production_payload_fixture();

        // This is the regression: bincode's positional reader cannot use the
        // current nested serde defaults to safely consume an actual v1 record.
        assert!(
            bincode::deserialize::<WorldSnapshot>(&legacy_payload).is_err(),
            "the current positional record must not be trusted for a v1 production payload"
        );

        let (mut migrated, path) = decode_bincode_world_snapshot(&legacy_payload)
            .expect("exact legacy production payload should migrate");
        assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyProductionV1);
        assert_legacy_production_migrated(&migrated, barracks_id);

        // Schema v2 already had production frames and Queue exit state, but
        // its ObjectSnapshot predates the appended Hacker Disable channel.
        // Decode it via its exact mirror instead of allowing the current v3
        // object decoder to consume the following world fields as an Option.
        let (v2_source, v2_source_path) =
            decode_bincode_world_snapshot(&legacy_payload).expect("rebuild source for v2 fixture");
        assert_eq!(
            v2_source_path,
            BincodeWorldSnapshotDecodePath::LegacyProductionV1
        );
        let v2_payload = serialize_pre_hacker_disable_v2_fixture(v2_source)
            .expect("serialize exact pre-HDB v2 fixture");
        assert!(
            bincode::deserialize::<WorldSnapshot>(&v2_payload).is_err(),
            "the current v3 object record must not consume a pre-HDB v2 payload"
        );
        let (v2_migrated, v2_path) = decode_bincode_world_snapshot(&v2_payload)
            .expect("pre-HDB v2 payload should migrate through its exact mirror");
        assert_eq!(
            v2_path,
            BincodeWorldSnapshotDecodePath::LegacyPreHackerDisableV2
        );
        assert_legacy_production_migrated(&v2_migrated, barracks_id);

        // The old float is converted at the first real production update,
        // where the restored template and live power factor are available.
        let mut restored = GameLogic::new();
        restored.templates = templates;
        SnapshotBuilder::new()
            .restore_from_snapshot(&migrated, &mut restored)
            .expect("restore migrated production snapshot");
        let restored_production = restored
            .host_object_mut(barracks_id)
            .expect("restored legacy producer")
            .building_data
            .as_mut()
            .expect("restored legacy production data");
        restored_production.production_queue[0].progress = 4.5;
        // 12 seconds at 30 FPS: floor(4.5 / 12 * 360) = 135, then this
        // update advances exactly one C++ logic frame.
        restored_production.production_queue[0].construction_frames = 0;
        restored_production.advance_production_progress(1.0, 1.0);
        assert_eq!(
            restored_production.production_queue[0].construction_frames,
            136
        );

        // A re-save is tagged v3 and uses the current record directly.  Its
        // HDB channel must survive independently of older production layouts.
        migrated
            .objects
            .get_mut(&barracks_id)
            .expect("migrated producer for current HDB channel")
            .hacker_disable_channel = Some(HackerDisableChannelState::new(
            ObjectId(77),
            HackerDisableChannelPhase::Preparing,
            1_500,
        ));
        let current_payload = bincode::serialize(&migrated).expect("serialize current snapshot");
        let (current_round_trip, current_path) = decode_bincode_world_snapshot(&current_payload)
            .expect("current production snapshot should remain readable");
        assert_eq!(current_path, BincodeWorldSnapshotDecodePath::Current);
        assert_eq!(current_round_trip.version, WORLD_SNAPSHOT_BINCODE_VERSION);
        assert_eq!(
            current_round_trip
                .objects
                .get(&barracks_id)
                .and_then(|object| object.hacker_disable_channel),
            Some(HackerDisableChannelState::new(
                ObjectId(77),
                HackerDisableChannelPhase::Preparing,
                1_500,
            ))
        );

        let mut future_payload = current_payload.clone();
        future_payload[..std::mem::size_of::<u32>()]
            .copy_from_slice(&(WORLD_SNAPSHOT_BINCODE_VERSION + 1).to_le_bytes());
        assert!(matches!(
            decode_bincode_world_snapshot(&future_payload),
            Err(SaveLoadError::VersionMismatch {
                expected: WORLD_SNAPSHOT_BINCODE_VERSION,
                actual
            }) if actual == WORLD_SNAPSHOT_BINCODE_VERSION + 1
        ));

        let save_info = fixture_save_info();

        // Native Common `.sav` CHUNK_GameLogic route.
        let common_chunks = SaveFileManager::write_common_sav_chunks_with_payload(
            &migrated,
            &save_info,
            legacy_payload.clone(),
        )
        .expect("encode Common fixture");
        let (common_snapshot, _) = SaveFileManager::read_common_sav_chunks(&common_chunks)
            .expect("Common fixture should migrate legacy payload");
        assert_legacy_production_migrated(&common_snapshot, barracks_id);

        // The GZHS wrapper can contain a Common GameState body or the original
        // raw WorldSnapshot body; both call the same migration seam.
        let wrapped_common = SaveFileManager::encode_common_game_state_with_payload(
            &migrated,
            &save_info,
            legacy_payload.clone(),
        )
        .expect("encode GZHS Common payload");
        let fixture_directory = unique_fixture_directory();
        std::fs::create_dir_all(&fixture_directory).expect("create fixture directory");
        let manager = SaveFileManager::with_save_directory(&fixture_directory);

        let wrapped_path = fixture_directory.join("legacy_common.gen");
        std::fs::write(
            &wrapped_path,
            gzhs_fixture_bytes(&save_info, &wrapped_common),
        )
        .expect("write GZHS Common fixture");
        let (wrapped_snapshot, _) = manager
            .load_from_file(&wrapped_path)
            .expect("GZHS Common fixture should migrate legacy payload");
        assert_legacy_production_migrated(&wrapped_snapshot, barracks_id);

        let raw_path = fixture_directory.join("legacy_raw.gen");
        std::fs::write(&raw_path, gzhs_fixture_bytes(&save_info, &legacy_payload))
            .expect("write raw GZHS fixture");
        let (raw_snapshot, _) = manager
            .load_from_file(&raw_path)
            .expect("raw GZHS fixture should migrate legacy payload");
        assert_legacy_production_migrated(&raw_snapshot, barracks_id);

        let _ = std::fs::remove_file(wrapped_path);
        let _ = std::fs::remove_file(raw_path);
        let _ = std::fs::remove_dir(fixture_directory);
    }
}
