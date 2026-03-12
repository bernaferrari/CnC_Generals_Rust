//! INI parsing for OnlineChatColors block
//!
//! This module handles parsing the OnlineChatColors block from INI files.
//! OnlineChatColors define color values for various GameSpy online chat elements
//! including player names, chat text, game listings, and MOTD.
//!
//! C++ Reference: GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Chat.cpp
//! C++ Header: GeneralsMD/Code/GameEngine/Include/Common/INI.h
//!
//! Rust port: 2025

use crate::common::ini::ini::{INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;

// ============================================================================
// Constants
// ============================================================================

/// Number of color slots in the GameSpy color array
pub const GSCOLOR_MAX: usize = 27;

// ============================================================================
// Types
// ============================================================================

/// GameSpy color indices - each corresponds to a color slot
/// These are used as indices into the GameSpyColor array
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum GSColorIndex {
    Default = 0,
    CurrentRoom = 1,
    Room = 2,
    Game = 3,
    GameFull = 4,
    GameCRCMismatch = 5,
    PlayerNormal = 6,
    PlayerOwner = 7,
    PlayerBuddy = 8,
    PlayerSelf = 9,
    PlayerIgnored = 10,
    ChatNormal = 11,
    ChatEmote = 12,
    ChatOwner = 13,
    ChatOwnerEmote = 14,
    ChatPrivate = 15,
    ChatPrivateEmote = 16,
    ChatPrivateOwner = 17,
    ChatPrivateOwnerEmote = 18,
    ChatBuddy = 19,
    ChatSelf = 20,
    AcceptTrue = 21,
    AcceptFalse = 22,
    MapSelected = 23,
    MapUnselected = 24,
    MOTD = 25,
    MOTDHeading = 26,
}

impl GSColorIndex {
    /// Convert from usize to GSColorIndex
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            0 => Some(GSColorIndex::Default),
            1 => Some(GSColorIndex::CurrentRoom),
            2 => Some(GSColorIndex::Room),
            3 => Some(GSColorIndex::Game),
            4 => Some(GSColorIndex::GameFull),
            5 => Some(GSColorIndex::GameCRCMismatch),
            6 => Some(GSColorIndex::PlayerNormal),
            7 => Some(GSColorIndex::PlayerOwner),
            8 => Some(GSColorIndex::PlayerBuddy),
            9 => Some(GSColorIndex::PlayerSelf),
            10 => Some(GSColorIndex::PlayerIgnored),
            11 => Some(GSColorIndex::ChatNormal),
            12 => Some(GSColorIndex::ChatEmote),
            13 => Some(GSColorIndex::ChatOwner),
            14 => Some(GSColorIndex::ChatOwnerEmote),
            15 => Some(GSColorIndex::ChatPrivate),
            16 => Some(GSColorIndex::ChatPrivateEmote),
            17 => Some(GSColorIndex::ChatPrivateOwner),
            18 => Some(GSColorIndex::ChatPrivateOwnerEmote),
            19 => Some(GSColorIndex::ChatBuddy),
            20 => Some(GSColorIndex::ChatSelf),
            21 => Some(GSColorIndex::AcceptTrue),
            22 => Some(GSColorIndex::AcceptFalse),
            23 => Some(GSColorIndex::MapSelected),
            24 => Some(GSColorIndex::MapUnselected),
            25 => Some(GSColorIndex::MOTD),
            26 => Some(GSColorIndex::MOTDHeading),
            _ => None,
        }
    }
}

/// Online chat colors container
///
/// Contains all color values used for GameSpy online chat.
/// Matches the C++ GameSpyColor global array from Chat.cpp.
#[derive(Debug, Clone)]
pub struct OnlineChatColors {
    /// Color values stored as packed ARGB u32 values
    colors: [u32; GSCOLOR_MAX],
}

impl Default for OnlineChatColors {
    fn default() -> Self {
        Self {
            // Default values from C++ GameSpyColor array in Chat.cpp
            colors: [
                0xFFFFFFFF, // GSCOLOR_DEFAULT (255,255,255,255)
                0xFFFFFF00, // GSCOLOR_CURRENTROOM (255,255,0,255)
                0xFFFFFFFF, // GSCOLOR_ROOM (255,255,255,255)
                0xFF808000, // GSCOLOR_GAME (128,128,0,255)
                0xFF808080, // GSCOLOR_GAME_FULL (128,128,128,255)
                0xFF808080, // GSCOLOR_GAME_CRCMISMATCH (128,128,128,255)
                0xFFFFFFFF, // GSCOLOR_PLAYER_NORMAL (255,255,255,255)
                0xFFFF00FF, // GSCOLOR_PLAYER_OWNER (255,0,255,255)
                0xFFFF0080, // GSCOLOR_PLAYER_BUDDY (255,0,128,255)
                0xFFFF0000, // GSCOLOR_PLAYER_SELF (255,0,0,255)
                0xFF808080, // GSCOLOR_PLAYER_IGNORED (128,128,128,255)
                0xFFFFFFFF, // GSCOLOR_CHAT_NORMAL (255,255,255,255)
                0xFF80FF00, // GSCOLOR_CHAT_EMOTE (255,128,0,255)
                0xFFFFFF00, // GSCOLOR_CHAT_OWNER (255,255,0,255)
                0xFF00FF00, // GSCOLOR_CHAT_OWNER_EMOTE (128,255,0,255)
                0xFFFF0000, // GSCOLOR_CHAT_PRIVATE (0,0,255,255)
                0xFFFFFF00, // GSCOLOR_CHAT_PRIVATE_EMOTE (0,255,255,255)
                0xFFFF00FF, // GSCOLOR_CHAT_PRIVATE_OWNER (255,0,255,255)
                0xFF80FF80, // GSCOLOR_CHAT_PRIVATE_OWNER_EMOTE (255,128,255,255)
                0xFFFF00FF, // GSCOLOR_CHAT_BUDDY (255,0,255,255)
                0xFFFF0080, // GSCOLOR_CHAT_SELF (255,0,128,255)
                0xFF00FF00, // GSCOLOR_ACCEPT_TRUE (0,255,0,255)
                0xFFFF0000, // GSCOLOR_ACCEPT_FALSE (255,0,0,255)
                0xFFFFFF00, // GSCOLOR_MAP_SELECTED (255,255,0,255)
                0xFFFFFFFF, // GSCOLOR_MAP_UNSELECTED (255,255,255,255)
                0xFFFFFFFF, // GSCOLOR_MOTD (255,255,255,255)
                0xFFFFFF00, // GSCOLOR_MOTD_HEADING (255,255,0,255)
            ],
        }
    }
}

impl OnlineChatColors {
    /// Create a new OnlineChatColors with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a color by index
    pub fn get_color(&self, index: GSColorIndex) -> u32 {
        self.colors[index as usize]
    }

    /// Get a color by raw index
    pub fn get_color_by_index(&self, index: usize) -> Option<u32> {
        self.colors.get(index).copied()
    }

    /// Set a color by index
    pub fn set_color(&mut self, index: usize, color: u32) -> INIResult<()> {
        if index >= GSCOLOR_MAX {
            return Err(INIError::InvalidData);
        }
        self.colors[index] = color;
        Ok(())
    }

    /// Make a color from RGBA components (matches C++ GameMakeColor)
    /// Returns packed ARGB format: (A << 24) | (R << 16) | (G << 8) | B
    pub fn make_color(r: u8, g: u8, b: u8, a: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Extract RGBA components from a packed color
    pub fn extract_color(color: u32) -> (u8, u8, u8, u8) {
        let a = ((color >> 24) & 0xFF) as u8;
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        (r, g, b, a)
    }
}

// ============================================================================
// Global State
// ============================================================================

/// Global online chat colors instance
static ONLINE_CHAT_COLORS: OnceCell<RwLock<OnlineChatColors>> = OnceCell::new();

/// Get or initialize the global OnlineChatColors
fn online_chat_colors_cell() -> &'static RwLock<OnlineChatColors> {
    ONLINE_CHAT_COLORS.get_or_init(|| RwLock::new(OnlineChatColors::new()))
}

/// Get read access to the global OnlineChatColors
pub fn get_online_chat_colors() -> parking_lot::RwLockReadGuard<'static, OnlineChatColors> {
    online_chat_colors_cell().read()
}

/// Get write access to the global OnlineChatColors
pub fn get_online_chat_colors_mut() -> parking_lot::RwLockWriteGuard<'static, OnlineChatColors> {
    online_chat_colors_cell().write()
}

/// Initialize the global OnlineChatColors
pub fn init_online_chat_colors() {
    let _ = online_chat_colors_cell();
}

// ============================================================================
// INI Parsing
// ============================================================================

/// Field names and their corresponding indices in the color array
/// Matches C++ GameSpyColorFieldParse table from Chat.cpp
const COLOR_FIELD_NAMES: &[(&str, usize)] = &[
    ("Default", 0),                       // GSCOLOR_DEFAULT
    ("CurrentRoom", 1),                   // GSCOLOR_CURRENTROOM
    ("ChatRoom", 2),                      // GSCOLOR_ROOM
    ("Game", 3),                          // GSCOLOR_GAME
    ("GameFull", 4),                      // GSCOLOR_GAME_FULL
    ("GameCRCMismatch", 5),               // GSCOLOR_GAME_CRCMISMATCH
    ("PlayerNormal", 6),                  // GSCOLOR_PLAYER_NORMAL
    ("PlayerOwner", 7),                   // GSCOLOR_PLAYER_OWNER
    ("PlayerBuddy", 8),                   // GSCOLOR_PLAYER_BUDDY
    ("PlayerSelf", 9),                    // GSCOLOR_PLAYER_SELF
    ("PlayerIgnored", 10),                // GSCOLOR_PLAYER_IGNORED
    ("ChatNormal", 11),                   // GSCOLOR_CHAT_NORMAL
    ("ChatEmote", 12),                    // GSCOLOR_CHAT_EMOTE
    ("ChatOwner", 13),                    // GSCOLOR_CHAT_OWNER
    ("ChatOwnerEmote", 14),               // GSCOLOR_CHAT_OWNER_EMOTE
    ("ChatPriv", 15),                     // GSCOLOR_CHAT_PRIVATE
    ("ChatPrivEmote", 16),                // GSCOLOR_CHAT_PRIVATE_EMOTE
    ("ChatPrivOwner", 17),                // GSCOLOR_CHAT_PRIVATE_OWNER
    ("ChatPrivOwnerEmote", 18),           // GSCOLOR_CHAT_PRIVATE_OWNER_EMOTE
    ("ChatBuddy", 19),                    // GSCOLOR_CHAT_BUDDY
    ("ChatSelf", 20),                     // GSCOLOR_CHAT_SELF
    ("AcceptTrue", 21),                   // GSCOLOR_ACCEPT_TRUE
    ("AcceptFalse", 22),                  // GSCOLOR_ACCEPT_FALSE
    ("MapSelected", 23),                  // GSCOLOR_MAP_SELECTED
    ("MapUnselected", 24),                // GSCOLOR_MAP_UNSELECTED
    ("MOTD", 25),                         // GSCOLOR_MOTD
    ("MOTDHeading", 26),                  // GSCOLOR_MOTD_HEADING
];

/// Find the field index for a given field name
fn find_field_index(field_name: &str) -> Option<usize> {
    for &(name, index) in COLOR_FIELD_NAMES {
        if name.eq_ignore_ascii_case(field_name) {
            return Some(index);
        }
    }
    None
}

/// Parse a color in R:val G:val B:val [A:val] format (matches C++ INI::parseColorInt)
/// Returns packed ARGB format
fn parse_color_int(tokens: &[&str]) -> INIResult<u32> {
    let mut r: Option<i32> = None;
    let mut g: Option<i32> = None;
    let mut b: Option<i32> = None;
    let mut a: Option<i32> = None;

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];

        // Handle both "R:" and "R" formats
        let (key, value) = if let Some((left, right)) = token.split_once(':') {
            if right.is_empty() {
                // Format: R: 255 (value is next token)
                i += 1;
                if i >= tokens.len() {
                    return Err(INIError::InvalidData);
                }
                (left, tokens[i])
            } else {
                // Format: R:255
                (left, right)
            }
        } else {
            // Format: R 255 (no colon, value is next token)
            i += 1;
            if i >= tokens.len() {
                return Err(INIError::InvalidData);
            }
            (token, tokens[i])
        };

        // Parse the value as integer
        let value: i32 = value.parse().map_err(|_| INIError::InvalidData)?;

        // Validate range
        if value < 0 || value > 255 {
            return Err(INIError::InvalidData);
        }

        // Assign to the appropriate component
        match key.to_ascii_uppercase().as_str() {
            "R" => r = Some(value),
            "G" => g = Some(value),
            "B" => b = Some(value),
            "A" => a = Some(value),
            _ => {}
        }

        i += 1;
    }

    // R, G, B are required; A defaults to 255
    let r = r.ok_or(INIError::InvalidData)?;
    let g = g.ok_or(INIError::InvalidData)?;
    let b = b.ok_or(INIError::InvalidData)?;
    let a = a.unwrap_or(255);

    // Return packed ARGB color (matches C++ GameMakeColor)
    Ok(OnlineChatColors::make_color(r as u8, g as u8, b as u8, a as u8))
}

/// Parse the OnlineChatColors INI block
///
/// This function parses an OnlineChatColors block from an INI file.
/// The block contains color definitions for various GameSpy chat elements.
///
/// Example INI format:
/// ```ini
/// OnlineChatColors
///     Default = R:255 G:255 B:255
///     PlayerNormal = R:255 G:255 B:255
///     PlayerSelf = R:255 G:0 B:0
///     ChatNormal = R:255 G:255 B:255
/// End
/// ```
pub fn parse_online_chat_color_definition(ini: &mut INI) -> INIResult<()> {
    let mut colors = get_online_chat_colors_mut();

    // Parse fields until we hit END
    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }

        let tokens = ini.get_line_tokens();
        if tokens.is_empty() {
            continue;
        }

        let first = tokens[0];
        if first.eq_ignore_ascii_case("End") {
            break;
        }

        // Find the field index
        let field_name = first.trim_end_matches(':');
        
        if let Some(field_index) = find_field_index(field_name) {
            // Get the value tokens (everything after '=' or the field name)
            let value_tokens: Vec<&str> = tokens
                .iter()
                .skip(1)
                .copied()
                .filter(|t| *t != "=")
                .collect();

            let color = parse_color_int(&value_tokens)?;
            colors.set_color(field_index, color)?;
        } else {
            // Unknown field - in debug builds this would warn
            #[cfg(debug_assertions)]
            eprintln!(
                "Warning: Unknown OnlineChatColors field '{}' at line {}",
                field_name,
                ini.get_line_num()
            );
        }
    }

    Ok(())
}

/// Register the OnlineChatColors parser with the INI system
pub fn register_online_chat_colors_parser() {
    crate::common::ini::ini::register_block_parser("OnlineChatColors", |ini: &mut INI| {
        parse_online_chat_color_definition(ini)
    });
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_color() {
        // Test with white
        let white = OnlineChatColors::make_color(255, 255, 255, 255);
        assert_eq!(white, 0xFFFFFFFF);

        // Test with red
        let red = OnlineChatColors::make_color(255, 0, 0, 255);
        assert_eq!(red, 0xFFFF0000);

        // Test with semi-transparent blue
        let blue = OnlineChatColors::make_color(0, 0, 255, 128);
        assert_eq!(blue, 0x800000FF);
    }

    #[test]
    fn test_extract_color() {
        let (r, g, b, a) = OnlineChatColors::extract_color(0xFFFFFFFF);
        assert_eq!((r, g, b, a), (255, 255, 255, 255));

        let (r, g, b, a) = OnlineChatColors::extract_color(0xFFFF0000);
        assert_eq!((r, g, b, a), (0, 0, 255, 255));

        let (r, g, b, a) = OnlineChatColors::extract_color(0x80808040);
        assert_eq!((r, g, b, a), (128, 128, 128, 64));
    }

    #[test]
    fn test_parse_color_int() {
        // Standard format
        let color = parse_color_int(&["R:255", "G:255", "B:255"]).unwrap();
        assert_eq!(color, 0xFFFFFFFF);

        // With alpha
        let color = parse_color_int(&["R:255", "G:0", "B:128", "A:64"]).unwrap();
        assert_eq!(color, 0x40FF0080);

        // Space-separated format
        let color = parse_color_int(&["R", "100", "G", "200", "B", "50"]).unwrap();
        assert_eq!(color, 0xFF64C832);
    }

    #[test]
    fn test_online_chat_colors_default() {
        let colors = OnlineChatColors::new();

        // Verify default color values match C++ defaults
        assert_eq!(
            colors.get_color(GSColorIndex::Default),
            0xFFFFFFFF
        );
        assert_eq!(
            colors.get_color(GSColorIndex::CurrentRoom),
            0xFFFFFF00
        );
        assert_eq!(
            colors.get_color(GSColorIndex::PlayerSelf),
            0xFFFF0000
        );
    }

    #[test]
    fn test_online_chat_colors_set_get() {
        let mut colors = OnlineChatColors::new();

        colors.set_color(0, 0x12345678).unwrap();
        assert_eq!(colors.get_color_by_index(0), Some(0x12345678));

        // Out of bounds should fail
        assert!(colors.set_color(GSCOLOR_MAX, 0xFFFFFFFF).is_err());
    }

    #[test]
    fn test_gs_color_index() {
        assert_eq!(
            GSColorIndex::from_usize(0),
            Some(GSColorIndex::Default)
        );
        assert_eq!(
            GSColorIndex::from_usize(9),
            Some(GSColorIndex::PlayerSelf)
        );
        assert_eq!(GSColorIndex::from_usize(27), None);
    }
}
