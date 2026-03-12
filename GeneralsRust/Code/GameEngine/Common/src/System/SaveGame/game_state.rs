// FILE: game_state.rs
// Author: Ported from C++ (Colin Day, September 2002)
// Desc: Game state singleton from which to load and save the game state

use super::super::xfer::*;
use super::super::xfer_load::XferLoad;
use super::super::xfer_save::XferSave;
use chrono::{Datelike, Local, Timelike};
use std::fs;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------------------------------------
// Constants
// ------------------------------------------------------------------------------------------------
const SAVE_FILE_EOF: &str = "SG_EOF";
const SAVE_GAME_EXTENSION: &str = ".sav";
const ZERO_NAME_ONLY: &str = "00000000";
const MAX_SAVE_FILE_NUMBER: i32 = 99999999;

const GAME_STATE_BLOCK_STRING: &str = "CHUNK_GameState";
const CAMPAIGN_BLOCK_STRING: &str = "CHUNK_Campaign";

// ------------------------------------------------------------------------------------------------
// Save/Load Layout Type
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLoadLayoutType {
    Invalid = 0,
    SaveAndLoad,
    LoadOnly,
    SaveOnly,
}

// ------------------------------------------------------------------------------------------------
// Save File Type
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFileType {
    Normal,  // Regular save game at any arbitrary point
    Mission, // Save game in between missions
}

impl Default for SaveFileType {
    fn default() -> Self {
        SaveFileType::Normal
    }
}

// ------------------------------------------------------------------------------------------------
// Save Date Structure
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, Default)]
pub struct SaveDate {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub day_of_week: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

impl SaveDate {
    /// Check if this date is newer than another
    pub fn is_newer_than(&self, other: &SaveDate) -> bool {
        // Year
        if self.year > other.year {
            return true;
        } else if self.year < other.year {
            return false;
        }

        // Month
        if self.month > other.month {
            return true;
        } else if self.month < other.month {
            return false;
        }

        // Day
        if self.day > other.day {
            return true;
        } else if self.day < other.day {
            return false;
        }

        // Hour
        if self.hour > other.hour {
            return true;
        } else if self.hour < other.hour {
            return false;
        }

        // Minute
        if self.minute > other.minute {
            return true;
        } else if self.minute < other.minute {
            return false;
        }

        // Second
        if self.second > other.second {
            return true;
        } else if self.second < other.second {
            return false;
        }

        // Millisecond
        self.milliseconds > other.milliseconds
    }
}

// ------------------------------------------------------------------------------------------------
// Save Game Info
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Default)]
pub struct SaveGameInfo {
    pub save_game_map_name: String,   // Map name of the "scratch pad" map
    pub pristine_map_name: String,    // Pristine map in the map directory
    pub map_label: String,            // Pretty name of this level
    pub date: SaveDate,               // Date of file save
    pub campaign_side: String,        // Which campaign side we're playing
    pub mission_number: i32,          // Mission number in campaign
    pub description: String,          // User description for save game file
    pub save_file_type: SaveFileType, // Type of save file
    pub mission_map_name: String,     // Used for mission saves
}

// ------------------------------------------------------------------------------------------------
// Available Game Info
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct AvailableGameInfo {
    pub filename: String,
    pub save_game_info: SaveGameInfo,
}

// ------------------------------------------------------------------------------------------------
// Save Code (return codes)
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveCode {
    Invalid = -1,
    Ok,
    NoFileAvailable,
    FileNotFound,
    UnableToOpenFile,
    InvalidXfer,
    UnknownBlock,
    InvalidData,
    Error,
}

// ------------------------------------------------------------------------------------------------
// Snapshot Type
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotType {
    SaveLoad,
    DeepCrcLogicOnly,
    DeepCrc,
}

const SNAPSHOT_MAX: usize = 3;

// ------------------------------------------------------------------------------------------------
// Snapshot Block - Associates a snapshot with its block name
// ------------------------------------------------------------------------------------------------
struct SnapshotBlock {
    snapshot: Box<dyn Snapshot>,
    block_name: String,
}

// ------------------------------------------------------------------------------------------------
// GameState - Main save/load management structure
// ------------------------------------------------------------------------------------------------
pub struct GameState {
    snapshot_block_lists: [Vec<SnapshotBlock>; SNAPSHOT_MAX],
    game_info: SaveGameInfo,
    snapshot_post_process_list: Vec<(SnapshotType, usize)>,
    available_games: Vec<AvailableGameInfo>,
    is_in_load_game: bool,
    save_directory: PathBuf,
}

impl GameState {
    /// Create a new GameState instance
    pub fn new(save_directory: PathBuf) -> Self {
        Self {
            snapshot_block_lists: [Vec::new(), Vec::new(), Vec::new()],
            game_info: SaveGameInfo::default(),
            snapshot_post_process_list: Vec::new(),
            available_games: Vec::new(),
            is_in_load_game: false,
            save_directory,
        }
    }

    /// Initialize the game state system
    pub fn init(&mut self) {
        // Snapshot blocks are registered externally via add_snapshot_block.
        self.is_in_load_game = false;
    }

    /// Reset the game state
    pub fn reset(&mut self) {
        self.snapshot_post_process_list.clear();
        self.available_games.clear();
        self.is_in_load_game = false;
    }

    /// Clear cached available games list.
    pub fn clear_available_games(&mut self) {
        self.available_games.clear();
    }

    /// Access the cached available games list.
    pub fn available_games(&self) -> &[AvailableGameInfo] {
        &self.available_games
    }

    /// Refresh the cached available games list from disk.
    pub fn refresh_available_games(&mut self) {
        self.available_games = self.collect_available_games();
        self.available_games.sort_by(|a, b| {
            if a.save_game_info.date.is_newer_than(&b.save_game_info.date) {
                std::cmp::Ordering::Less
            } else if b.save_game_info.date.is_newer_than(&a.save_game_info.date) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }

    /// Add a snapshot block to the system
    pub fn add_snapshot_block(
        &mut self,
        block_name: String,
        snapshot: Box<dyn Snapshot>,
        which: SnapshotType,
    ) {
        if block_name.is_empty() {
            eprintln!("addSnapshotBlock: Invalid parameters");
            return;
        }

        let block_info = SnapshotBlock {
            snapshot,
            block_name,
        };

        let index = which as usize;
        self.snapshot_block_lists[index].push(block_info);
    }

    /// Find a snapshot block by token
    fn find_block_info_by_token(
        &mut self,
        token: &str,
        which: SnapshotType,
    ) -> Option<&mut SnapshotBlock> {
        if token.is_empty() {
            return None;
        }

        let index = which as usize;
        self.snapshot_block_lists[index]
            .iter_mut()
            .find(|block| block.block_name == token)
    }

    /// Get save game info
    pub fn get_save_game_info(&self) -> &SaveGameInfo {
        &self.game_info
    }

    /// Get mutable save game info
    pub fn get_save_game_info_mut(&mut self) -> &mut SaveGameInfo {
        &mut self.game_info
    }

    /// Check if currently in load game
    pub fn is_in_load_game(&self) -> bool {
        self.is_in_load_game
    }

    /// Set pristine map name
    pub fn set_pristine_map_name(&mut self, name: String) {
        self.game_info.pristine_map_name = name;
    }

    /// Get pristine map name
    pub fn get_pristine_map_name(&self) -> &str {
        &self.game_info.pristine_map_name
    }

    /// Get save directory
    pub fn get_save_directory(&self) -> &Path {
        &self.save_directory
    }

    /// Get file path in save directory
    pub fn get_file_path_in_save_directory(&self, leaf: &str) -> PathBuf {
        let mut path = self.save_directory.clone();
        path.push(leaf);
        path
    }

    /// Check if path is in save directory
    pub fn is_in_save_directory(&self, path: &Path) -> bool {
        path.starts_with(&self.save_directory)
    }

    /// Get map leaf name (filename without path)
    pub fn get_map_leaf_name(&self, path: &str) -> String {
        let trimmed = path.trim_end_matches(['\\', '/']);
        let mut last_slash = trimmed.rfind('\\');
        let last_fwd = trimmed.rfind('/');
        if last_slash.is_none() || last_fwd > last_slash {
            last_slash = last_fwd;
        }
        if let Some(pos) = last_slash {
            trimmed[pos + 1..].to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Convert real map path to portable map path
    pub fn real_map_path_to_portable_map_path(&self, path: &str) -> String {
        let path_lower = path.to_lowercase();
        let save_dir = self.save_directory.to_string_lossy().to_lowercase();
        if path_lower.starts_with(&save_dir) {
            let mut out = String::from(super::game_state_map::PORTABLE_SAVE);
            out.push_str(&self.get_map_leaf_name(path));
            return out.to_lowercase();
        }

        let map_dir = "maps\\";
        let user_map_dir = "userdata\\maps\\";
        if let Some(idx) = path_lower.find(map_dir) {
            let mut out = String::from(super::game_state_map::PORTABLE_MAPS);
            out.push_str(&path[idx + map_dir.len()..]);
            return out.to_lowercase();
        }
        if let Some(idx) = path_lower.find(user_map_dir) {
            let mut out = String::from(super::game_state_map::PORTABLE_USER_MAPS);
            out.push_str(&path[idx + user_map_dir.len()..]);
            return out.to_lowercase();
        }

        path_lower
    }

    /// Convert portable map path to real map path
    pub fn portable_map_path_to_real_map_path(&self, path: &str) -> String {
        let lower = path.to_lowercase();
        if lower.starts_with(super::game_state_map::PORTABLE_SAVE) {
            let leaf = self.get_map_leaf_name(path);
            return self
                .get_file_path_in_save_directory(&leaf)
                .to_string_lossy()
                .to_string();
        }
        if lower.starts_with(super::game_state_map::PORTABLE_MAPS) {
            let mut out = String::from("Maps\\");
            out.push_str(&path[super::game_state_map::PORTABLE_MAPS.len()..]);
            return out.to_lowercase();
        }
        if lower.starts_with(super::game_state_map::PORTABLE_USER_MAPS) {
            let mut out = String::from("UserData\\Maps\\");
            out.push_str(&path[super::game_state_map::PORTABLE_USER_MAPS.len()..]);
            return out.to_lowercase();
        }

        lower
    }

    /// Save the game
    pub fn save_game(
        &mut self,
        filename: String,
        desc: String,
        save_type: SaveFileType,
        which: SnapshotType,
    ) -> Result<SaveCode, XferStatus> {
        // Find filename if not provided
        let filename = if filename.is_empty() {
            match self.find_next_save_filename(&desc) {
                Some(name) => name,
                None => {
                    eprintln!("GameState::save_game - Unable to find valid filename");
                    return Ok(SaveCode::NoFileAvailable);
                }
            }
        } else {
            filename
        };

        // Ensure save directory exists
        std::fs::create_dir_all(&self.save_directory).ok();

        // Construct file path
        let filepath = self.get_file_path_in_save_directory(&filename);

        // Save description
        self.game_info.description = desc;

        // Open save file
        let mut xfer_save = XferSave::new();
        if let Err(_) = xfer_save.open(filepath.to_str().unwrap_or("").to_string()) {
            eprintln!("Error opening file '{:?}'", filepath);
            return Ok(SaveCode::Error);
        }

        // Set save file type
        self.game_info.save_file_type = save_type;
        if save_type == SaveFileType::Mission {
            if self.game_info.mission_map_name.is_empty() {
                self.game_info.mission_map_name = self.game_info.pristine_map_name.clone();
            }
        } else {
            self.game_info.mission_map_name.clear();
        }

        // Write save file
        match self.xfer_save_data(&mut xfer_save, which) {
            Ok(_) => {
                xfer_save.close()?;
                println!("Game saved successfully");
                Ok(SaveCode::Ok)
            }
            Err(e) => {
                xfer_save.close()?;
                eprintln!("Error saving game: {:?}", e);
                Ok(SaveCode::Error)
            }
        }
    }

    /// Load a game
    pub fn load_game(&mut self, game_info: AvailableGameInfo) -> Result<SaveCode, XferStatus> {
        // Check if file exists
        let filepath = self.get_file_path_in_save_directory(&game_info.filename);
        if !filepath.exists() {
            return Ok(SaveCode::FileNotFound);
        }

        // Open load file
        let mut xfer_load = XferLoad::new();
        xfer_load.open(filepath.to_str().unwrap_or("").to_string())?;

        // Set load flag
        self.is_in_load_game = true;

        // Load save data
        let result = match self.xfer_save_data(&mut xfer_load, SnapshotType::SaveLoad) {
            Ok(_) => {
                xfer_load.close()?;

                // Post process
                self.game_state_post_process_load()?;

                Ok(SaveCode::Ok)
            }
            Err(e) => {
                xfer_load.close()?;
                eprintln!("Error loading game: {:?}", e);
                Ok(SaveCode::InvalidData)
            }
        };

        self.is_in_load_game = false;
        result
    }

    /// Create a mission save (best-effort without campaign integration).
    pub fn mission_save(&mut self) -> Result<SaveCode, XferStatus> {
        let mission_number = self.game_info.mission_number.saturating_add(1);
        let description = if self.game_info.campaign_side.is_empty() {
            format!("Mission Save {}", mission_number)
        } else {
            format!(
                "Mission Save {} {}",
                self.game_info.campaign_side, mission_number
            )
        };
        self.save_game(
            String::new(),
            description,
            SaveFileType::Mission,
            SnapshotType::SaveLoad,
        )
    }

    /// Transfer save data (used for both save and load)
    fn xfer_save_data(
        &mut self,
        xfer: &mut dyn Xfer,
        which: SnapshotType,
    ) -> Result<(), XferStatus> {
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                // Save all blocks
                let index = which as usize;
                for block_info in &mut self.snapshot_block_lists[index] {
                    let mut block_name = block_info.block_name.clone();

                    if self.game_info.save_file_type == SaveFileType::Mission {
                        if !block_name.eq_ignore_ascii_case(GAME_STATE_BLOCK_STRING)
                            && !block_name.eq_ignore_ascii_case(CAMPAIGN_BLOCK_STRING)
                        {
                            continue;
                        }
                    }

                    // Transfer block name
                    xfer.xfer_ascii_string(&mut block_name)?;

                    // Begin block
                    xfer.begin_block()?;

                    // Transfer block data
                    xfer.xfer_snapshot(block_info.snapshot.as_mut())?;

                    // End block
                    xfer.end_block()?;
                }

                // Write EOF token
                let mut eof_token = SAVE_FILE_EOF.to_string();
                xfer.xfer_ascii_string(&mut eof_token)?;
            }
            XferMode::Load => {
                // Read all data blocks
                loop {
                    // Read next token
                    let mut token = String::new();
                    xfer.xfer_ascii_string(&mut token)?;

                    // Check for EOF
                    if token.eq_ignore_ascii_case(SAVE_FILE_EOF) {
                        break;
                    }

                    // Find matching block
                    let index = which as usize;
                    let block_pos = self.snapshot_block_lists[index]
                        .iter()
                        .position(|block| block.block_name.eq_ignore_ascii_case(&token));
                    if let Some(pos) = block_pos {
                        let _block_size = xfer.begin_block()?;
                        let snapshot = self.snapshot_block_lists[index][pos].snapshot.as_mut();
                        xfer.xfer_snapshot(snapshot)?;
                        if !bit_test(xfer.get_options(), xfer_options::NO_POST_PROCESSING) {
                            self.snapshot_post_process_list.push((which, pos));
                        }
                        xfer.end_block()?;
                    } else {
                        // Unknown block - skip it
                        eprintln!("Skipping unknown block '{}'", token);
                        let data_size = xfer.begin_block()?;
                        xfer.skip(data_size)?;
                    }
                }
            }
            _ => {
                return Err(XferStatus::ModeUnknown);
            }
        }

        Ok(())
    }

    /// Post process after loading
    fn game_state_post_process_load(&mut self) -> Result<(), XferStatus> {
        // Post process each snapshot
        for (which, index) in &self.snapshot_post_process_list {
            let list_index = *which as usize;
            if let Some(block) = self.snapshot_block_lists[list_index].get_mut(*index) {
                block.snapshot.load_post_process()?;
            }
        }

        // Clear post process list
        self.snapshot_post_process_list.clear();

        Ok(())
    }

    /// Add snapshot for post processing
    pub fn add_post_process_snapshot(&mut self, which: SnapshotType, index: usize) {
        self.snapshot_post_process_list.push((which, index));
    }

    /// Find next available save filename
    fn find_next_save_filename(&self, _desc: &str) -> Option<String> {
        // Search for lowest available number
        for i in 0..=MAX_SAVE_FILE_NUMBER {
            let filename = format!("{:08}{}", i, SAVE_GAME_EXTENSION);
            let filepath = self.get_file_path_in_save_directory(&filename);

            if !filepath.exists() {
                return Some(filename);
            }
        }

        None
    }

    /// Check if save game exists
    pub fn does_save_game_exist(&self, filename: &str) -> bool {
        let filepath = self.get_file_path_in_save_directory(filename);
        filepath.exists()
    }

    /// Extract save game info from a file on disk.
    pub fn get_save_game_info_from_file(
        &mut self,
        filename: &str,
    ) -> Result<SaveGameInfo, XferStatus> {
        let path = if Path::new(filename).is_absolute() {
            PathBuf::from(filename)
        } else {
            self.get_file_path_in_save_directory(filename)
        };

        let mut xfer_load = XferLoad::new();
        xfer_load.open(path.to_str().unwrap_or("").to_string())?;
        xfer_load.set_options(xfer_options::NO_POST_PROCESSING);

        let mut token = String::new();
        loop {
            xfer_load.xfer_ascii_string(&mut token)?;
            if token.eq_ignore_ascii_case(SAVE_FILE_EOF) {
                break;
            }

            let block_size = xfer_load.begin_block()?;
            if token.eq_ignore_ascii_case(GAME_STATE_BLOCK_STRING) {
                let mut temp_state = GameState::new(self.save_directory.clone());
                xfer_load.xfer_snapshot(&mut temp_state)?;
                xfer_load.end_block()?;
                xfer_load.close()?;
                return Ok(temp_state.game_info);
            }

            xfer_load.skip(block_size)?;
            xfer_load.end_block()?;
        }

        xfer_load.close()?;
        Err(XferStatus::UnknownBlock)
    }

    fn collect_available_games(&mut self) -> Vec<AvailableGameInfo> {
        let mut games = Vec::new();
        for filename in self.iterate_save_files() {
            if let Ok(info) = self.get_save_game_info_from_file(&filename) {
                games.push(AvailableGameInfo {
                    filename,
                    save_game_info: info,
                });
            }
        }
        games.sort_by(|a, b| {
            let a_date = &a.save_game_info.date;
            let b_date = &b.save_game_info.date;
            if a_date.is_newer_than(b_date) {
                std::cmp::Ordering::Less
            } else if b_date.is_newer_than(a_date) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        games
    }

    fn iterate_save_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.save_directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                    continue;
                };
                if !ext.eq_ignore_ascii_case("sav") {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    files.push(name.to_string());
                }
            }
        }
        files
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshot implementation for GameState
// ------------------------------------------------------------------------------------------------
impl Snapshot for GameState {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        // Empty implementation
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        // Version
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        // Version 2+
        if version >= 2 {
            // File type
            let mut file_type = self.game_info.save_file_type as i32;
            xfer.xfer_int(&mut file_type)?;
            self.game_info.save_file_type = if file_type == SaveFileType::Mission as i32 {
                SaveFileType::Mission
            } else {
                SaveFileType::Normal
            };

            // Mission map name
            xfer.xfer_ascii_string(&mut self.game_info.mission_map_name)?;
        }

        // Date and time
        if xfer.get_xfer_mode() == XferMode::Save {
            let now = Local::now();
            self.game_info.date.year = now.year() as u16;
            self.game_info.date.month = now.month() as u16;
            self.game_info.date.day = now.day() as u16;
            self.game_info.date.day_of_week = now.weekday().num_days_from_sunday() as u16;
            self.game_info.date.hour = now.hour() as u16;
            self.game_info.date.minute = now.minute() as u16;
            self.game_info.date.second = now.second() as u16;
            self.game_info.date.milliseconds = (now.timestamp_subsec_millis()) as u16;
        }
        xfer.xfer_unsigned_short(&mut self.game_info.date.year)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.month)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.day)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.day_of_week)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.hour)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.minute)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.second)?;
        xfer.xfer_unsigned_short(&mut self.game_info.date.milliseconds)?;

        // User description
        xfer.xfer_unicode_string(&mut self.game_info.description)?;

        if xfer.get_xfer_mode() == XferMode::Save && self.game_info.map_label.is_empty() {
            if let Some(global) = crate::common::ini::ini_game_data::get_global_data() {
                let global = global.read();
                if !global.map_name.is_empty() {
                    self.game_info.map_label = self.get_map_leaf_name(&global.map_name);
                }
            }
        }
        xfer.xfer_ascii_string(&mut self.game_info.map_label)?;

        // Campaign info
        xfer.xfer_ascii_string(&mut self.game_info.campaign_side)?;
        xfer.xfer_int(&mut self.game_info.mission_number)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        // Empty implementation
        Ok(())
    }
}

impl GameState {
    /// Transfer save data for CRC (clears volatile fields before transfer).
    pub fn friend_xfer_save_data_for_crc(
        &mut self,
        xfer: &mut dyn Xfer,
        which: SnapshotType,
    ) -> Result<(), XferStatus> {
        self.game_info.description.clear();
        self.game_info.save_file_type = SaveFileType::Normal;
        self.game_info.mission_map_name.clear();
        self.game_info.pristine_map_name.clear();
        self.xfer_save_data(xfer, which)
    }
}
