//! INI parsing for MapCache definitions
//!
//! This module handles parsing MapCache entries from INI files.
//! MapCache stores metadata about maps for quick access without loading the full map.
//!
//! Author: Matthew D. Campbell, February 2002
//! Rust port: 2025

use once_cell::sync::OnceCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use crate::common::system::quoted_printable::{
    quoted_printable_to_ascii_string as qp_to_ascii,
    quoted_printable_to_unicode_string as qp_to_unicode,
};

/// Maximum number of player slots supported
pub const MAX_SLOTS: usize = 8;

/// 3D coordinate representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }
}

impl Default for Coord3D {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<(f32, f32, f32)> for Coord3D {
    fn from(tuple: (f32, f32, f32)) -> Self {
        Self::new(tuple.0, tuple.1, tuple.2)
    }
}

/// 3D region representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }

    pub fn zero() -> Self {
        Self {
            lo: Coord3D::ZERO,
            hi: Coord3D::ZERO,
        }
    }

    pub fn get_size(&self) -> Coord3D {
        Coord3D::new(
            self.hi.x - self.lo.x,
            self.hi.y - self.lo.y,
            self.hi.z - self.lo.z,
        )
    }

    pub fn contains(&self, point: Coord3D) -> bool {
        point.x >= self.lo.x
            && point.x <= self.hi.x
            && point.y >= self.lo.y
            && point.y <= self.hi.y
            && point.z >= self.lo.z
            && point.z <= self.hi.z
    }
}

impl Default for Region3D {
    fn default() -> Self {
        Self::zero()
    }
}

/// Windows timestamp representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WinTimeStamp {
    pub low_time_stamp: i32,
    pub high_time_stamp: i32,
}

impl WinTimeStamp {
    pub fn new(low: i32, high: i32) -> Self {
        Self {
            low_time_stamp: low,
            high_time_stamp: high,
        }
    }

    pub fn zero() -> Self {
        Self {
            low_time_stamp: 0,
            high_time_stamp: 0,
        }
    }

    pub fn to_u64(&self) -> u64 {
        ((self.high_time_stamp as u64) << 32) | (self.low_time_stamp as u64)
    }

    pub fn from_u64(timestamp: u64) -> Self {
        Self {
            low_time_stamp: (timestamp & 0xFFFFFFFF) as i32,
            high_time_stamp: (timestamp >> 32) as i32,
        }
    }
}

impl Default for WinTimeStamp {
    fn default() -> Self {
        Self::zero()
    }
}

/// Unicode string representation
#[derive(Debug, Clone, PartialEq)]
pub struct UnicodeString {
    content: String,
}

impl UnicodeString {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }

    pub fn from_string(s: String) -> Self {
        Self { content: s }
    }

    pub fn translate(&mut self, ascii: &str) {
        self.content = ascii.to_string();
    }

    pub fn format(&mut self, format_str: &str, value: i32) {
        self.content = format_str.replace("%d", &value.to_string());
    }

    pub fn concat(&mut self, other: &UnicodeString) {
        self.content.push_str(&other.content);
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.content
    }
}

impl Default for UnicodeString {
    fn default() -> Self {
        Self::new()
    }
}

/// Map metadata reader - internal structure for parsing
#[derive(Debug, Clone)]
pub struct MapMetaDataReader {
    pub extent: Region3D,
    pub num_players: i32,
    pub is_multiplayer: bool,
    pub ascii_display_name: String,
    pub ascii_name_lookup_tag: String,
    pub is_official: bool,
    pub timestamp: WinTimeStamp,
    pub filesize: u32,
    pub crc: u32,
    pub waypoints: [Coord3D; MAX_SLOTS],
    pub initial_camera_position: Coord3D,
    pub supply_positions: Vec<Coord3D>,
    pub tech_positions: Vec<Coord3D>,
}

impl Default for MapMetaDataReader {
    fn default() -> Self {
        Self {
            extent: Region3D::default(),
            num_players: 0,
            is_multiplayer: false,
            ascii_display_name: String::new(),
            ascii_name_lookup_tag: String::new(),
            is_official: false,
            timestamp: WinTimeStamp::default(),
            filesize: 0,
            crc: 0,
            waypoints: [Coord3D::default(); MAX_SLOTS],
            initial_camera_position: Coord3D::default(),
            supply_positions: Vec::new(),
            tech_positions: Vec::new(),
        }
    }
}

impl MapMetaDataReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse from INI file using the field parse table.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), String> {
        ini.init_from_ini_with_fields(self, FIELD_PARSE_TABLE)
            .map_err(|error| error.to_string())
    }
}

/// Map metadata - final processed metadata for a map
#[derive(Debug, Clone)]
pub struct MapMetaData {
    pub extent: Region3D,
    pub is_official: bool,
    pub is_multiplayer: bool,
    pub num_players: i32,
    pub filesize: u32,
    pub crc: u32,
    pub timestamp: WinTimeStamp,
    pub display_name: UnicodeString,
    pub name_lookup_tag: String,
    pub file_name: String,
    pub waypoints: HashMap<String, Coord3D>,
    pub supply_positions: Vec<Coord3D>,
    pub tech_positions: Vec<Coord3D>,
}

impl Default for MapMetaData {
    fn default() -> Self {
        Self {
            extent: Region3D::default(),
            is_official: false,
            is_multiplayer: false,
            num_players: 0,
            filesize: 0,
            crc: 0,
            timestamp: WinTimeStamp::default(),
            display_name: UnicodeString::default(),
            name_lookup_tag: String::new(),
            file_name: String::new(),
            waypoints: HashMap::new(),
            supply_positions: Vec::new(),
            tech_positions: Vec::new(),
        }
    }
}

impl MapMetaData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get waypoint by name
    pub fn get_waypoint(&self, name: &str) -> Option<Coord3D> {
        self.waypoints.get(name).copied()
    }

    /// Add or update a waypoint
    pub fn set_waypoint(&mut self, name: String, position: Coord3D) {
        self.waypoints.insert(name, position);
    }

    /// Get number of supply positions
    pub fn get_supply_position_count(&self) -> usize {
        self.supply_positions.len()
    }

    /// Get number of tech positions
    pub fn get_tech_position_count(&self) -> usize {
        self.tech_positions.len()
    }

    /// Check if this is a skirmish map
    pub fn is_skirmish_map(&self) -> bool {
        self.is_multiplayer && self.num_players >= 2
    }
}

/// Field parsing functions for MapMetaDataReader

/// Parse supply position coordinate
pub fn parse_supply_position_coord3d(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.supply_positions.push(coord.into());
    Ok(())
}

/// Parse tech position coordinate  
pub fn parse_tech_positions_coord3d(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.tech_positions.push(coord.into());
    Ok(())
}

/// Field parser definition
pub type FieldParser = FieldParse<MapMetaDataReader>;

/// Field parse table for MapMetaDataReader (matches C++ table)
pub const FIELD_PARSE_TABLE: &[FieldParser] = &[
    FieldParser {
        token: "isOfficial",
        parse: parse_is_official,
    },
    FieldParser {
        token: "isMultiplayer",
        parse: parse_is_multiplayer,
    },
    FieldParser {
        token: "extentMin",
        parse: parse_extent_min,
    },
    FieldParser {
        token: "extentMax",
        parse: parse_extent_max,
    },
    FieldParser {
        token: "numPlayers",
        parse: parse_num_players,
    },
    FieldParser {
        token: "fileSize",
        parse: parse_file_size,
    },
    FieldParser {
        token: "fileCRC",
        parse: parse_file_crc,
    },
    FieldParser {
        token: "timestampLo",
        parse: parse_timestamp_lo,
    },
    FieldParser {
        token: "timestampHi",
        parse: parse_timestamp_hi,
    },
    FieldParser {
        token: "displayName",
        parse: parse_display_name,
    },
    FieldParser {
        token: "nameLookupTag",
        parse: parse_name_lookup_tag,
    },
    FieldParser {
        token: "supplyPosition",
        parse: parse_supply_position_coord3d,
    },
    FieldParser {
        token: "techPosition",
        parse: parse_tech_positions_coord3d,
    },
    FieldParser {
        token: "Player_1_Start",
        parse: parse_player_1_start,
    },
    FieldParser {
        token: "Player_2_Start",
        parse: parse_player_2_start,
    },
    FieldParser {
        token: "Player_3_Start",
        parse: parse_player_3_start,
    },
    FieldParser {
        token: "Player_4_Start",
        parse: parse_player_4_start,
    },
    FieldParser {
        token: "Player_5_Start",
        parse: parse_player_5_start,
    },
    FieldParser {
        token: "Player_6_Start",
        parse: parse_player_6_start,
    },
    FieldParser {
        token: "Player_7_Start",
        parse: parse_player_7_start,
    },
    FieldParser {
        token: "Player_8_Start",
        parse: parse_player_8_start,
    },
    FieldParser {
        token: "InitialCameraPosition",
        parse: parse_initial_camera_position,
    },
];

// Individual field parsers

pub fn parse_is_official(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.is_official = INI::parse_bool(value)?;
    Ok(())
}

pub fn parse_is_multiplayer(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.is_multiplayer = INI::parse_bool(value)?;
    Ok(())
}

pub fn parse_extent_min(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.extent.lo = coord.into();
    Ok(())
}

pub fn parse_extent_max(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.extent.hi = coord.into();
    Ok(())
}

pub fn parse_num_players(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.num_players = INI::parse_int(value)?;
    Ok(())
}

pub fn parse_file_size(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.filesize = INI::parse_unsigned_int(value)?;
    Ok(())
}

pub fn parse_file_crc(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.crc = INI::parse_unsigned_int(value)?;
    Ok(())
}

pub fn parse_timestamp_lo(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.timestamp.low_time_stamp = INI::parse_int(value)?;
    Ok(())
}

pub fn parse_timestamp_hi(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let value = tokens.first().ok_or(INIError::InvalidData)?;
    reader.timestamp.high_time_stamp = INI::parse_int(value)?;
    Ok(())
}

pub fn parse_display_name(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    if tokens.is_empty() {
        reader.ascii_display_name.clear();
        return Ok(());
    }
    let joined = tokens.join(" ");
    reader.ascii_display_name = INI::parse_ascii_string(&joined)?;
    Ok(())
}

pub fn parse_name_lookup_tag(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    if tokens.is_empty() {
        reader.ascii_name_lookup_tag.clear();
        return Ok(());
    }
    let joined = tokens.join(" ");
    reader.ascii_name_lookup_tag = INI::parse_ascii_string(&joined)?;
    Ok(())
}

pub fn parse_player_1_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[0] = coord.into();
    Ok(())
}

pub fn parse_player_2_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[1] = coord.into();
    Ok(())
}

pub fn parse_player_3_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[2] = coord.into();
    Ok(())
}

pub fn parse_player_4_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[3] = coord.into();
    Ok(())
}

pub fn parse_player_5_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[4] = coord.into();
    Ok(())
}

pub fn parse_player_6_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[5] = coord.into();
    Ok(())
}

pub fn parse_player_7_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[6] = coord.into();
    Ok(())
}

pub fn parse_player_8_start(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.waypoints[7] = coord.into();
    Ok(())
}

pub fn parse_initial_camera_position(
    _ini: &mut INI,
    reader: &mut MapMetaDataReader,
    tokens: &[&str],
) -> INIResult<()> {
    let coord = INI::parse_coord_3d(tokens)?;
    reader.initial_camera_position = coord.into();
    Ok(())
}

/// Map cache storage
#[derive(Debug)]
pub struct MapCache {
    maps: BTreeMap<String, MapMetaData>,
}

impl Default for MapCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MapCache {
    pub fn new() -> Self {
        Self {
            maps: BTreeMap::new(),
        }
    }

    /// Add or update map metadata
    pub fn insert(&mut self, filename: String, metadata: MapMetaData) {
        self.maps.insert(filename.to_lowercase(), metadata);
    }

    /// Get map metadata by filename
    pub fn get(&self, filename: &str) -> Option<&MapMetaData> {
        self.maps.get(&filename.to_lowercase())
    }

    /// Remove map metadata by filename
    pub fn remove(&mut self, filename: &str) -> Option<MapMetaData> {
        self.maps.remove(&filename.to_lowercase())
    }

    /// Iterate over map metadata entries
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, String, MapMetaData> {
        self.maps.iter()
    }

    /// Get all map filenames
    pub fn get_map_names(&self) -> Vec<&String> {
        self.maps.keys().collect()
    }

    /// Get maps matching criteria
    pub fn find_multiplayer_maps(
        &self,
        min_players: i32,
        max_players: i32,
    ) -> Vec<(&String, &MapMetaData)> {
        self.maps
            .iter()
            .filter(|(_, metadata)| {
                metadata.is_multiplayer
                    && metadata.num_players >= min_players
                    && metadata.num_players <= max_players
            })
            .collect()
    }

    /// Get official maps only
    pub fn get_official_maps(&self) -> Vec<(&String, &MapMetaData)> {
        self.maps
            .iter()
            .filter(|(_, metadata)| metadata.is_official)
            .collect()
    }

    /// Clear all cached maps
    pub fn clear(&mut self) {
        self.maps.clear();
    }

    /// Get number of cached maps
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

pub fn quoted_printable_to_ascii_string(input: &str) -> String {
    qp_to_ascii(input)
}

pub fn quoted_printable_to_unicode_string(input: &str) -> UnicodeString {
    UnicodeString::from_string(qp_to_unicode(input))
}

/// Global map cache instance
static MAP_CACHE: OnceCell<RwLock<MapCache>> = OnceCell::new();
type GameTextProvider = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;
static GAME_TEXT_PROVIDER: OnceCell<GameTextProvider> = OnceCell::new();

pub fn set_game_text_provider(provider: GameTextProvider) -> bool {
    GAME_TEXT_PROVIDER.set(provider).is_ok()
}

fn lookup_game_text(tag: &str) -> Option<String> {
    GAME_TEXT_PROVIDER
        .get()
        .and_then(|provider| (provider)(tag))
}

/// Initialize the global map cache
pub fn init_global_map_cache() {
    if MAP_CACHE.get().is_none() {
        let _ = MAP_CACHE.set(RwLock::new(MapCache::new()));
    }
}

/// Get reference to global map cache
pub fn get_map_cache() -> Option<RwLockReadGuard<'static, MapCache>> {
    MAP_CACHE
        .get()
        .map(|cache| cache.read().expect("MapCache poisoned"))
}

/// Get mutable reference to global map cache
pub fn get_map_cache_mut() -> Option<RwLockWriteGuard<'static, MapCache>> {
    MAP_CACHE
        .get()
        .map(|cache| cache.write().expect("MapCache poisoned"))
}

/// INI parsing function for MapCache definition (matches C++ interface)
///
/// This is the main entry point for parsing MapCache definitions from INI files
pub fn parse_map_cache_definition(ini: &mut INI) -> Result<(), String> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or("Expected map name but found none")?
        .to_string();
    let decoded_name = quoted_printable_to_ascii_string(&name);

    // Create reader and metadata
    let mut reader = MapMetaDataReader::new();
    let mut metadata = MapMetaData::new();

    // Parse the reader fields using field table
    reader.parse_from_ini(ini)?;

    // Copy data from reader to metadata
    metadata.extent = reader.extent;
    metadata.is_official = reader.is_official;
    metadata.is_multiplayer = reader.is_multiplayer;
    metadata.num_players = reader.num_players;
    metadata.filesize = reader.filesize;
    metadata.crc = reader.crc;
    metadata.timestamp = reader.timestamp;

    // Set initial camera position waypoint
    metadata.waypoints.insert(
        "InitialCameraPosition".to_string(),
        reader.initial_camera_position,
    );

    // Handle display name
    metadata.name_lookup_tag = quoted_printable_to_ascii_string(&reader.ascii_name_lookup_tag);

    if metadata.name_lookup_tag.is_empty() {
        // Maps without localized name tags
        let temp_display_name = match decoded_name.rfind('\\') {
            Some(pos) => &decoded_name[pos + 1..],
            None => &decoded_name,
        };
        metadata.display_name.translate(temp_display_name);

        if metadata.num_players >= 2 {
            let mut extension = UnicodeString::new();
            extension.format(" (%d)", metadata.num_players);
            metadata.display_name.concat(&extension);
        }
    } else {
        // Official maps with name tags
        if let Some(localized) = lookup_game_text(&metadata.name_lookup_tag) {
            metadata.display_name.translate(&localized);
        } else {
            metadata.display_name.translate(&metadata.name_lookup_tag);
        }

        if metadata.num_players >= 2 {
            let mut extension = UnicodeString::new();
            extension.format(" (%d)", metadata.num_players);
            metadata.display_name.concat(&extension);
        }
    }

    // Add player start waypoints
    for i in 0..metadata.num_players.min(MAX_SLOTS as i32) {
        let starting_cam_name = format!("Player_{}_Start", i + 1); // Start pos waypoints are 1-based
        metadata
            .waypoints
            .insert(starting_cam_name, reader.waypoints[i as usize]);
    }

    // Copy supply positions
    for supply_pos in &reader.supply_positions {
        metadata.supply_positions.push(*supply_pos);
    }

    // Copy tech positions
    for tech_pos in &reader.tech_positions {
        metadata.tech_positions.push(*tech_pos);
    }

    // Add to global cache if available and display name is not empty
    if let Some(mut map_cache) = get_map_cache_mut() {
        if !metadata.display_name.is_empty() {
            let lower_name = decoded_name.to_lowercase();
            metadata.file_name = lower_name.clone();
            map_cache.insert(lower_name, metadata);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord3d() {
        let coord = Coord3D::new(10.0, 20.0, 30.0);
        assert_eq!(coord.x, 10.0);
        assert_eq!(coord.y, 20.0);
        assert_eq!(coord.z, 30.0);

        let zero = Coord3D::ZERO;
        assert_eq!(zero.x, 0.0);
        assert_eq!(zero.y, 0.0);
        assert_eq!(zero.z, 0.0);
    }

    #[test]
    fn test_region3d() {
        let region = Region3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(100.0, 100.0, 100.0),
        );

        let size = region.get_size();
        assert_eq!(size.x, 100.0);
        assert_eq!(size.y, 100.0);
        assert_eq!(size.z, 100.0);

        assert!(region.contains(Coord3D::new(50.0, 50.0, 50.0)));
        assert!(!region.contains(Coord3D::new(150.0, 50.0, 50.0)));
    }

    #[test]
    fn test_win_timestamp() {
        let timestamp = WinTimeStamp::new(1000, 2000);
        assert_eq!(timestamp.low_time_stamp, 1000);
        assert_eq!(timestamp.high_time_stamp, 2000);

        let as_u64 = timestamp.to_u64();
        let back_from_u64 = WinTimeStamp::from_u64(as_u64);
        assert_eq!(timestamp.low_time_stamp, back_from_u64.low_time_stamp);
        assert_eq!(timestamp.high_time_stamp, back_from_u64.high_time_stamp);
    }

    #[test]
    fn test_unicode_string() {
        let mut unicode_str = UnicodeString::new();
        assert!(unicode_str.is_empty());

        unicode_str.translate("Hello World");
        assert!(!unicode_str.is_empty());
        assert_eq!(unicode_str.as_str(), "Hello World");

        unicode_str.format(" (%d)", 5);
        assert_eq!(unicode_str.as_str(), " (5)");

        let mut other = UnicodeString::from_string(" players".to_string());
        unicode_str.concat(&other);
        assert_eq!(unicode_str.as_str(), " (5) players");
    }

    #[test]
    fn test_map_meta_data_reader() {
        let mut reader = MapMetaDataReader::new();
        assert_eq!(reader.num_players, 0);
        assert!(!reader.is_multiplayer);
        assert!(!reader.is_official);
        assert_eq!(reader.supply_positions.len(), 0);
        assert_eq!(reader.tech_positions.len(), 0);

        reader
            .supply_positions
            .push(Coord3D::new(100.0, 100.0, 0.0));
        reader.tech_positions.push(Coord3D::new(200.0, 200.0, 0.0));

        assert_eq!(reader.supply_positions.len(), 1);
        assert_eq!(reader.tech_positions.len(), 1);
    }

    #[test]
    fn test_map_meta_data() {
        let mut metadata = MapMetaData::new();
        assert_eq!(metadata.num_players, 0);
        assert!(metadata.waypoints.is_empty());

        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(50.0, 50.0, 0.0));
        assert_eq!(metadata.waypoints.len(), 1);

        let waypoint = metadata.get_waypoint("Player_1_Start");
        assert!(waypoint.is_some());
        assert_eq!(waypoint.unwrap().x, 50.0);
    }

    #[test]
    fn test_map_cache() {
        let mut cache = MapCache::new();
        assert!(cache.is_empty());

        let mut metadata = MapMetaData::new();
        metadata.num_players = 2;
        metadata.is_multiplayer = true;
        metadata.is_official = true;

        cache.insert("test_map.map".to_string(), metadata);
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        let retrieved = cache.get("test_map.map");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().num_players, 2);

        let multiplayer_maps = cache.find_multiplayer_maps(2, 4);
        assert_eq!(multiplayer_maps.len(), 1);

        let official_maps = cache.get_official_maps();
        assert_eq!(official_maps.len(), 1);
    }

    #[test]
    fn test_map_cache_iterates_in_cpp_std_map_key_order() {
        let mut cache = MapCache::new();

        cache.insert("Maps\\Zulu\\Zulu.map".to_string(), MapMetaData::new());
        cache.insert("Maps\\Alpha\\Alpha.map".to_string(), MapMetaData::new());
        cache.insert("Maps\\Middle\\Middle.map".to_string(), MapMetaData::new());

        assert_eq!(
            cache
                .get_map_names()
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec![
                "maps\\alpha\\alpha.map",
                "maps\\middle\\middle.map",
                "maps\\zulu\\zulu.map"
            ]
        );
        assert_eq!(
            cache
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "maps\\alpha\\alpha.map",
                "maps\\middle\\middle.map",
                "maps\\zulu\\zulu.map"
            ]
        );
    }

    #[test]
    fn test_map_cache_filtered_results_keep_cpp_std_map_key_order() {
        let mut cache = MapCache::new();

        let mut zulu = MapMetaData::new();
        zulu.is_multiplayer = true;
        zulu.is_official = true;
        zulu.num_players = 4;

        let mut alpha = MapMetaData::new();
        alpha.is_multiplayer = true;
        alpha.is_official = true;
        alpha.num_players = 4;

        let mut middle = MapMetaData::new();
        middle.is_multiplayer = true;
        middle.is_official = false;
        middle.num_players = 8;

        cache.insert("Maps\\Zulu\\Zulu.map".to_string(), zulu);
        cache.insert("Maps\\Alpha\\Alpha.map".to_string(), alpha);
        cache.insert("Maps\\Middle\\Middle.map".to_string(), middle);

        assert_eq!(
            cache
                .find_multiplayer_maps(2, 6)
                .into_iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["maps\\alpha\\alpha.map", "maps\\zulu\\zulu.map"]
        );
        assert_eq!(
            cache
                .get_official_maps()
                .into_iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["maps\\alpha\\alpha.map", "maps\\zulu\\zulu.map"]
        );
    }

    #[test]
    fn test_global_map_cache() {
        init_global_map_cache();

        assert!(get_map_cache().is_some());

        if let Some(mut cache) = get_map_cache_mut() {
            let mut metadata = MapMetaData::new();
            metadata.num_players = 4;
            cache.insert("global_test.map".to_string(), metadata);
        }

        if let Some(cache) = get_map_cache() {
            assert_eq!(cache.len(), 1);
            assert!(cache.get("global_test.map").is_some());
        }
    }

    #[test]
    fn test_field_parse_table() {
        assert!(!FIELD_PARSE_TABLE.is_empty());

        // Check that expected fields are present
        let field_names: Vec<&str> = FIELD_PARSE_TABLE.iter().map(|f| f.token).collect();
        assert!(field_names.contains(&"isOfficial"));
        assert!(field_names.contains(&"isMultiplayer"));
        assert!(field_names.contains(&"numPlayers"));
        assert!(field_names.contains(&"Player_1_Start"));
        assert!(field_names.contains(&"InitialCameraPosition"));
        assert!(field_names.contains(&"supplyPosition"));
        assert!(field_names.contains(&"techPosition"));
    }
}
