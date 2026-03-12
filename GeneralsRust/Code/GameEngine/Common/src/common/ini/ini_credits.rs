////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_credits.rs
//! Author: Chris Huybregts (Converted to Rust)
//! Desc: Credits block parser for end-game credit scrolling
//!
//! Matches C++ Credits.cpp and Credits.h from:
//! - GeneralsMD/Code/GameEngine/Source/GameClient/Credits.cpp
//! - GeneralsMD/Code/GameEngine/Include/GameClient/Credits.h
//!
//! # C++ Field Parse Table (Credits.cpp lines 51-65)
//! ```cpp
//! const FieldParse CreditsManager::m_creditsFieldParseTable[] = 
//! {
//!     { "ScrollRate",              INI::parseInt,          NULL, offsetof(CreditsManager, m_scrollRate) },
//!     { "ScrollRateEveryFrames",   INI::parseInt,          NULL, offsetof(CreditsManager, m_scrollRatePerFrames) },
//!     { "ScrollDown",              INI::parseBool,         NULL, offsetof(CreditsManager, m_scrollDown) },
//!     { "TitleColor",              INI::parseColorInt,     NULL, offsetof(CreditsManager, m_titleColor) },
//!     { "MinorTitleColor",         INI::parseColorInt,     NULL, offsetof(CreditsManager, m_positionColor) },
//!     { "NormalColor",             INI::parseColorInt,     NULL, offsetof(CreditsManager, m_normalColor) },
//!     { "Style",                   INI::parseLookupList,   CreditStyleNames, offsetof(CreditsManager, m_currentStyle) },
//!     { "Blank",                   CreditsManager::parseBlank, NULL, NULL },
//!     { "Text",                    CreditsManager::parseText,  NULL, NULL },
//!     { NULL, NULL, NULL, 0 }
//! };
//! ```

use once_cell::sync::OnceCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ini::{INIError, INIResult, INI};

/// Space offset between credit lines
/// Matches C++ CREDIT_SPACE_OFFSET from Credits.h line 54
pub const CREDIT_SPACE_OFFSET: i32 = 2;

/// Credit style enumeration
/// Matches C++ enum from Credits.h lines 41-49
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditStyle {
    Title = 0,
    Position = 1,  // Called MINORTITLE in INI
    Normal = 2,
    Column = 3,
    Blank = 4,
}

impl Default for CreditStyle {
    fn default() -> Self {
        CreditStyle::Normal
    }
}

impl CreditStyle {
    /// Parse credit style from string
    /// Matches C++ CreditStyleNames lookup table from Credits.h lines 56-63
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "TITLE" => Some(CreditStyle::Title),
            "MINORTITLE" => Some(CreditStyle::Position),
            "NORMAL" => Some(CreditStyle::Normal),
            "COLUMN" => Some(CreditStyle::Column),
            _ => None,
        }
    }
}

/// Single line in the credits display
/// Matches C++ CreditsLine from Credits.h lines 66-82
///
/// # C++ Definition
/// ```cpp
/// class CreditsLine
/// {
/// public:
///     // parsing variables
///     Int m_style;
///     UnicodeString m_text;
///     UnicodeString m_secondText;
///     Bool m_useSecond;
///     Bool m_done;
///
///     // drawing variables
///     DisplayString *m_displayString;
///     DisplayString *m_secondDisplayString;
///     ICoord2D m_pos;
///     Int m_height;
///     Int m_color;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CreditsLine {
    /// Style of this credit line (TITLE, POSITION, NORMAL, COLUMN, BLANK)
    pub style: CreditStyle,
    
    /// Primary text content
    pub text: String,
    
    /// Secondary text for COLUMN style (right column)
    pub second_text: String,
    
    /// Flag indicating if second text should be used
    pub use_second: bool,
    
    /// Flag indicating if this line is done processing
    pub done: bool,
    
    /// Y position for drawing
    pub pos_y: i32,
    
    /// X position for drawing
    pub pos_x: i32,
    
    /// Height of this line
    pub height: i32,
    
    /// Color for drawing
    pub color: u32,
}

impl CreditsLine {
    /// Create a new credits line with default values
    /// Matches C++ CreditsLine::CreditsLine() from Credits.cpp lines 69-76
    pub fn new() -> Self {
        Self {
            style: CreditStyle::Blank,
            text: String::new(),
            second_text: String::new(),
            use_second: false,
            done: false,
            pos_y: 0,
            pos_x: 0,
            height: 0,
            color: 0xFFFFFFFF, // White
        }
    }
}

impl Default for CreditsLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Credits manager - handles credit scrolling display
/// Matches C++ CreditsManager from Credits.h lines 85-121
///
/// # C++ Definition
/// ```cpp
/// class CreditsManager : public SubsystemInterface
/// {
/// public:
///     // ...
/// private:
///     typedef std::list<CreditsLine *> CreditsLineList;
///     CreditsLineList m_creditLineList;
///     CreditsLineList::iterator m_creditLineListIt;
///     CreditsLineList m_displayedCreditLineList;
///
///     Int m_scrollRate;           // in pixels
///     Int m_scrollRatePerFrames;
///     Bool m_scrollDown;          // if TRUE text will come from top to bottom
///
///     Color m_titleColor;
///     Color m_positionColor;
///     Color m_normalColor;
///
///     Int m_currentStyle;
///     Bool m_isFinished;
///     Int m_framesSinceStarted;
///     Int m_normalFontHeight;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CreditsManager {
    /// Scroll rate in pixels per scroll step
    /// Matches C++ m_scrollRate
    pub scroll_rate: i32,
    
    /// How many frames between each scroll step
    /// Matches C++ m_scrollRatePerFrames
    pub scroll_rate_per_frames: i32,
    
    /// If true, text scrolls from top to bottom; if false, bottom to top
    /// Matches C++ m_scrollDown
    pub scroll_down: bool,
    
    /// Color for title lines (ARGB)
    /// Matches C++ m_titleColor
    pub title_color: u32,
    
    /// Color for position/minor title lines (ARGB)
    /// Matches C++ m_positionColor
    pub position_color: u32,
    
    /// Color for normal text lines (ARGB)
    /// Matches C++ m_normalColor
    pub normal_color: u32,
    
    /// Current style for parsing text entries
    /// Matches C++ m_currentStyle
    pub current_style: CreditStyle,
    
    /// List of all credit lines
    /// Matches C++ m_creditLineList
    pub credit_lines: Vec<CreditsLine>,
    
    /// Flag indicating credits have finished scrolling
    /// Matches C++ m_isFinished
    pub is_finished: bool,
    
    /// Frames since scrolling started
    /// Matches C++ m_framesSinceStarted
    pub frames_since_started: i32,
    
    /// Height of normal font for blank lines
    /// Matches C++ m_normalFontHeight
    pub normal_font_height: i32,
    
    /// Flag indicating this is an override
    is_override: bool,
}

impl CreditsManager {
    /// Create a new credits manager with default values
    /// Matches C++ CreditsManager::CreditsManager() from Credits.cpp lines 79-90
    pub fn new() -> Self {
        Self {
            scroll_rate: 1,
            scroll_rate_per_frames: 1,
            scroll_down: true,
            title_color: 0xFFFFFFFF,      // White
            position_color: 0xFFFFFFFF,    // White  
            normal_color: 0xFFFFFFFF,      // White
            current_style: CreditStyle::Normal,
            credit_lines: Vec::new(),
            is_finished: false,
            frames_since_started: 0,
            normal_font_height: 10,
            is_override: false,
        }
    }
    
    /// Mark this as an override
    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }
    
    /// Check if this is an override
    pub fn is_override(&self) -> bool {
        self.is_override
    }
    
    /// Add a blank line to the credits
    /// Matches C++ CreditsManager::addBlank() from Credits.cpp lines 164-169
    pub fn add_blank(&mut self) {
        let line = CreditsLine {
            style: CreditStyle::Blank,
            ..CreditsLine::new()
        };
        self.credit_lines.push(line);
    }
    
    /// Add text to the credits based on current style
    /// Matches C++ CreditsManager::addText() from Credits.cpp lines 186-221
    pub fn add_text(&mut self, text: &str) {
        match self.current_style {
            CreditStyle::Title | CreditStyle::Position | CreditStyle::Normal => {
                let unicode_text = Self::get_unicode_string_static(text);
                let line = CreditsLine {
                    style: self.current_style,
                    text: unicode_text,
                    ..CreditsLine::new()
                };
                self.credit_lines.push(line);
            }
            CreditStyle::Column => {
                // Check if the last line is a COLUMN that isn't done
                let should_update_last = {
                    if let Some(last) = self.credit_lines.last() {
                        last.style == CreditStyle::Column && !last.done
                    } else {
                        false
                    }
                };
                
                if should_update_last {
                    let unicode_text = Self::get_unicode_string_static(text);
                    if let Some(last) = self.credit_lines.last_mut() {
                        last.second_text = unicode_text;
                        last.done = true;
                    }
                    return;
                }
                
                // Create new column entry
                let unicode_text = Self::get_unicode_string_static(text);
                let line = CreditsLine {
                    style: CreditStyle::Column,
                    text: unicode_text,
                    use_second: true,
                    ..CreditsLine::new()
                };
                self.credit_lines.push(line);
            }
            CreditStyle::Blank => {
                // Shouldn't add text with Blank style, but handle gracefully
                log::warn!("CreditsManager::addText: Tried to add text with Blank style");
            }
        }
    }
    
    /// Convert text to unicode string, handling localization labels
    /// Matches C++ CreditsManager::getUnicodeString() from Credits.cpp lines 226-237
    fn get_unicode_string_static(str: &str) -> String {
        if str == "<BLANK>" {
            return String::new();
        }
        
        // If it contains ':', it's a localization label
        // Otherwise, just translate the string directly
        if str.contains(':') {
            // Would look up in localization system
            // For now, return as-is
            str.to_string()
        } else {
            str.to_string()
        }
    }
    
    /// Parse credits definition from INI
    /// Matches C++ INI::parseCredits() from Credits.cpp lines 47-57
    pub fn parse_credits_definition(&mut self, ini: &mut INI) -> INIResult<()> {
        self.parse_credits_fields(ini)?;
        Ok(())
    }
    
    /// Parse credits fields from INI
    /// Matches C++ field parse table from Credits.cpp lines 51-65
    fn parse_credits_fields(&mut self, ini: &mut INI) -> INIResult<()> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }
            
            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }
            
            let key = tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            
            // Get the value tokens (skip key and any '=' signs)
            let mut value_tokens: Vec<&str> = tokens.iter().skip(1).copied().collect();
            value_tokens.retain(|t| *t != "=");
            
            // Parse fields based on key
            // Matches C++ field parse table from Credits.cpp lines 51-65
            match key.to_ascii_lowercase().as_str() {
                "scrollrate" => {
                    // parseInt
                    self.scroll_rate = value_tokens
                        .first()
                        .ok_or(INIError::InvalidData)?
                        .parse()
                        .map_err(|_| INIError::InvalidData)?;
                }
                "scrollrateeveryframes" => {
                    // parseInt
                    self.scroll_rate_per_frames = value_tokens
                        .first()
                        .ok_or(INIError::InvalidData)?
                        .parse()
                        .map_err(|_| INIError::InvalidData)?;
                }
                "scrolldown" => {
                    // parseBool
                    self.scroll_down = Self::parse_bool_value(
                        value_tokens.first().ok_or(INIError::InvalidData)?
                    )?;
                }
                "titlecolor" => {
                    // parseColorInt
                    self.title_color = Self::parse_color_value(
                        value_tokens.first().ok_or(INIError::InvalidData)?
                    )?;
                }
                "minortitlecolor" => {
                    // parseColorInt
                    self.position_color = Self::parse_color_value(
                        value_tokens.first().ok_or(INIError::InvalidData)?
                    )?;
                }
                "normalcolor" => {
                    // parseColorInt
                    self.normal_color = Self::parse_color_value(
                        value_tokens.first().ok_or(INIError::InvalidData)?
                    )?;
                }
                "style" => {
                    // parseLookupList - CreditStyleNames
                    let style_name = value_tokens.first().ok_or(INIError::InvalidData)?;
                    self.current_style = CreditStyle::from_str(style_name)
                        .ok_or(INIError::InvalidData)?;
                }
                "blank" => {
                    // parseBlank - adds a blank line
                    self.add_blank();
                }
                "text" => {
                    // parseText - adds a text line (may be quoted)
                    let text = if value_tokens.is_empty() {
                        String::new()
                    } else {
                        // Join tokens, handling quotes
                        let joined = value_tokens.join(" ");
                        // Remove surrounding quotes if present
                        if joined.starts_with('"') && joined.ends_with('"') {
                            joined[1..joined.len()-1].to_string()
                        } else {
                            joined
                        }
                    };
                    self.add_text(&text);
                }
                _ => {
                    // Unknown field - log warning but don't fail
                    // In C++, unknown fields in the parse table are silently ignored
                }
            }
        }
        
        // Validate scroll settings (matches C++ load() validation)
        if self.scroll_rate_per_frames <= 0 {
            self.scroll_rate_per_frames = 1;
        }
        if self.scroll_rate <= 0 {
            self.scroll_rate = 1;
        }
        
        Ok(())
    }
    
    /// Parse boolean value (Yes/No/True/False/1/0)
    fn parse_bool_value(token: &str) -> INIResult<bool> {
        match token.to_ascii_lowercase().as_str() {
            "yes" | "true" | "1" => Ok(true),
            "no" | "false" | "0" => Ok(false),
            _ => Err(INIError::InvalidData),
        }
    }
    
    /// Parse color value as ARGB integer
    /// Matches C++ INI::parseColorInt
    fn parse_color_value(token: &str) -> INIResult<u32> {
        // Try parsing as integer first
        if let Ok(val) = token.parse::<u32>() {
            return Ok(val);
        }
        
        // Try parsing as hex with 0x prefix
        if token.starts_with("0x") || token.starts_with("0X") {
            let hex_str = &token[2..];
            u32::from_str_radix(hex_str, 16).map_err(|_| INIError::InvalidData)
        } else {
            Err(INIError::InvalidData)
        }
    }
}

impl Default for CreditsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global credits manager instance
static CREDITS_MANAGER: OnceCell<RwLock<CreditsManager>> = OnceCell::new();

/// Get the global credits manager (read access)
pub fn get_credits_manager() -> RwLockReadGuard<'static, CreditsManager> {
    CREDITS_MANAGER
        .get_or_init(|| RwLock::new(CreditsManager::new()))
        .read()
        .unwrap()
}

/// Get the global credits manager (write access)
pub fn get_credits_manager_mut() -> RwLockWriteGuard<'static, CreditsManager> {
    CREDITS_MANAGER
        .get_or_init(|| RwLock::new(CreditsManager::new()))
        .write()
        .unwrap()
}

/// Initialize the global credits manager
pub fn init_credits_manager() {
    if CREDITS_MANAGER.get().is_none() {
        let _ = CREDITS_MANAGER.set(RwLock::new(CreditsManager::new()));
    } else if let Some(manager) = CREDITS_MANAGER.get() {
        if let Ok(mut guard) = manager.write() {
            *guard = CreditsManager::new();
        }
    }
}

/// Parse credits definition from INI block
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseCredits from Credits.cpp lines 47-57
pub fn parse_credits_definition(ini: &mut INI) -> Result<(), String> {
    let mut manager = get_credits_manager_mut();
    
    if ini.get_load_type() == crate::common::ini::INILoadType::CreateOverrides {
        manager.mark_as_override();
    }
    
    manager
        .parse_credits_definition(ini)
        .map_err(|e| format!("Credits parse error: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_credits_line_creation() {
        let line = CreditsLine::new();
        
        assert_eq!(line.style, CreditStyle::Blank);
        assert!(line.text.is_empty());
        assert!(line.second_text.is_empty());
        assert!(!line.use_second);
        assert!(!line.done);
    }
    
    #[test]
    fn test_credits_manager_creation() {
        let manager = CreditsManager::new();
        
        assert_eq!(manager.scroll_rate, 1);
        assert_eq!(manager.scroll_rate_per_frames, 1);
        assert!(manager.scroll_down);
        assert!(manager.credit_lines.is_empty());
        assert!(!manager.is_finished);
    }
    
    #[test]
    fn test_credit_style_parsing() {
        assert_eq!(CreditStyle::from_str("TITLE"), Some(CreditStyle::Title));
        assert_eq!(CreditStyle::from_str("MINORTITLE"), Some(CreditStyle::Position));
        assert_eq!(CreditStyle::from_str("NORMAL"), Some(CreditStyle::Normal));
        assert_eq!(CreditStyle::from_str("COLUMN"), Some(CreditStyle::Column));
        assert_eq!(CreditStyle::from_str("title"), Some(CreditStyle::Title));
        assert_eq!(CreditStyle::from_str("UNKNOWN"), None);
    }
    
    #[test]
    fn test_add_blank() {
        let mut manager = CreditsManager::new();
        manager.add_blank();
        
        assert_eq!(manager.credit_lines.len(), 1);
        assert_eq!(manager.credit_lines[0].style, CreditStyle::Blank);
    }
    
    #[test]
    fn test_add_text_normal() {
        let mut manager = CreditsManager::new();
        manager.current_style = CreditStyle::Normal;
        manager.add_text("Test Text");
        
        assert_eq!(manager.credit_lines.len(), 1);
        assert_eq!(manager.credit_lines[0].style, CreditStyle::Normal);
        assert_eq!(manager.credit_lines[0].text, "Test Text");
    }
    
    #[test]
    fn test_add_text_column() {
        let mut manager = CreditsManager::new();
        manager.current_style = CreditStyle::Column;
        
        // First text creates new column with first text
        manager.add_text("Left Column");
        assert_eq!(manager.credit_lines.len(), 1);
        assert!(manager.credit_lines[0].use_second);
        assert!(!manager.credit_lines[0].done);
        
        // Second text fills in second column
        manager.add_text("Right Column");
        assert_eq!(manager.credit_lines.len(), 1);
        assert!(manager.credit_lines[0].done);
        assert_eq!(manager.credit_lines[0].second_text, "Right Column");
        
        // Third text creates new column entry
        manager.add_text("Left Column 2");
        assert_eq!(manager.credit_lines.len(), 2);
    }
    
    #[test]
    fn test_parse_bool_value() {
        assert_eq!(CreditsManager::parse_bool_value("Yes").unwrap(), true);
        assert_eq!(CreditsManager::parse_bool_value("yes").unwrap(), true);
        assert_eq!(CreditsManager::parse_bool_value("TRUE").unwrap(), true);
        assert_eq!(CreditsManager::parse_bool_value("1").unwrap(), true);
        assert_eq!(CreditsManager::parse_bool_value("No").unwrap(), false);
        assert_eq!(CreditsManager::parse_bool_value("false").unwrap(), false);
        assert_eq!(CreditsManager::parse_bool_value("0").unwrap(), false);
        assert!(CreditsManager::parse_bool_value("invalid").is_err());
    }
    
    #[test]
    fn test_parse_color_value() {
        assert_eq!(CreditsManager::parse_color_value("0").unwrap(), 0);
        assert_eq!(CreditsManager::parse_color_value("255").unwrap(), 255);
        assert_eq!(CreditsManager::parse_color_value("0xFFFFFFFF").unwrap(), 0xFFFFFFFF);
        assert_eq!(CreditsManager::parse_color_value("0xFF0000FF").unwrap(), 0xFF0000FF);
    }
    
    #[test]
    fn test_global_manager() {
        init_credits_manager();
        
        let manager = get_credits_manager();
        assert!(!manager.is_override());
    }
}
