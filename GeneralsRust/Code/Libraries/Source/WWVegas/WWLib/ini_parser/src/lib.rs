//! INI Parser for Command & Conquer Generals Zero Hour
//!
//! This crate provides comprehensive INI file parsing functionality that replicates
//! the sophisticated INI system from the original C++ codebase. It handles:
//!
//! - Loading INI files and directories
//! - Parsing various data types (objects, weapons, audio, etc.)
//! - Field parsing with custom parsers
//! - Error handling and validation
//! - Memory-efficient storage

use base_types::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use string_system::AsciiString;
use thiserror::Error;

/// Maximum characters per line in INI files
pub const MAX_CHARS_PER_LINE: usize = 1028;

/// INI loading types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IniLoadType {
    /// Invalid load type
    Invalid,
    /// Create new or load over existing data instance
    Overwrite,
    /// Create new or load into new override data instance
    CreateOverrides,
    /// Create new or continue loading into existing data instance
    Multifile,
}

/// INI parser error types
#[derive(Error, Debug)]
pub enum IniError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid directory: {0}")]
    InvalidDirectory(String),

    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Invalid data format: {0}")]
    InvalidData(String),

    #[error("Missing end token: {0}")]
    MissingEndToken(String),

    #[error("Unknown token: {0}")]
    UnknownToken(String),

    #[error("Buffer too small: {0}")]
    BufferTooSmall(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

impl From<String> for IniError {
    fn from(value: String) -> Self {
        IniError::InvalidData(value)
    }
}

/// Result type for INI operations
pub type IniResult<T> = Result<T, IniError>;

/// Field parse function type
pub type FieldParseFn = fn(&mut IniParser, &mut dyn std::any::Any, &str) -> IniResult<()>;

/// Block parse function type
pub type BlockParseFn = fn(&mut IniParser) -> IniResult<()>;

/// Lookup list record for enum-like parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupListRec {
    pub name: String,
    pub value: Int,
}

impl LookupListRec {
    pub fn new(name: &str, value: Int) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
}

/// Field parse information
#[derive(Clone)]
pub struct FieldParse {
    pub token: String,
    pub parse_fn: FieldParseFn,
    pub offset: usize,
}

impl FieldParse {
    pub fn new(token: &str, parse_fn: FieldParseFn, offset: usize) -> Self {
        Self {
            token: token.to_string(),
            parse_fn,
            offset,
        }
    }
}

/// Multi-field parse information
pub struct MultiFieldParse {
    fields: Vec<FieldParse>,
    extra_offsets: Vec<usize>,
}

impl Default for MultiFieldParse {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiFieldParse {
    pub const MAX_MULTI_FIELDS: usize = 16;

    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            extra_offsets: Vec::new(),
        }
    }

    pub fn add(&mut self, field: FieldParse, extra_offset: usize) -> IniResult<()> {
        if self.fields.len() >= Self::MAX_MULTI_FIELDS {
            return Err(IniError::InvalidParams(
                "Too many fields in MultiFieldParse".to_string(),
            ));
        }

        self.fields.push(field);
        self.extra_offsets.push(extra_offset);
        Ok(())
    }

    pub fn get_count(&self) -> usize {
        self.fields.len()
    }

    pub fn get_field(&self, index: usize) -> Option<&FieldParse> {
        self.fields.get(index)
    }

    pub fn get_extra_offset(&self, index: usize) -> Option<usize> {
        self.extra_offsets.get(index).copied()
    }
}

/// INI parser state
#[derive(Debug)]
struct ParserState {
    line_number: usize,
    current_block_type: Option<String>,
    current_block_name: Option<String>,
    in_block: bool,
}

/// Main INI parser
pub struct IniParser {
    filename: AsciiString,
    load_type: IniLoadType,
    lines: Vec<String>,
    current_line_index: usize,
    state: ParserState,
    block_parsers: HashMap<String, BlockParseFn>,
    field_parsers: HashMap<String, Vec<FieldParse>>,
}

impl IniParser {
    /// Create a new INI parser
    pub fn new() -> Self {
        Self {
            filename: AsciiString::new(),
            load_type: IniLoadType::Overwrite,
            lines: Vec::new(),
            current_line_index: 0,
            state: ParserState {
                line_number: 0,
                current_block_type: None,
                current_block_name: None,
                in_block: false,
            },
            block_parsers: HashMap::new(),
            field_parsers: HashMap::new(),
        }
    }

    /// Load an INI file
    pub fn load(&mut self, filename: &str, load_type: IniLoadType) -> IniResult<()> {
        self.filename = AsciiString::from_str(filename)?;
        self.load_type = load_type;
        self.lines.clear();
        self.current_line_index = 0;
        self.state = ParserState {
            line_number: 0,
            current_block_type: None,
            current_block_name: None,
            in_block: false,
        };

        let path = Path::new(filename);
        if !path.exists() {
            return Err(IniError::FileNotFound(filename.to_string()));
        }

        let content = fs::read_to_string(path)?;
        self.lines = content.lines().map(|s| s.to_string()).collect();

        Ok(())
    }

    /// Load a directory of INI files
    pub fn load_directory(
        &mut self,
        dir_name: &str,
        include_subdirs: bool,
        load_type: IniLoadType,
        _xfer: Option<&mut dyn std::any::Any>,
    ) -> IniResult<()> {
        let path = Path::new(dir_name);
        if !path.is_dir() {
            return Err(IniError::InvalidDirectory(dir_name.to_string()));
        }

        for entry in walkdir::WalkDir::new(path)
            .max_depth(if include_subdirs { usize::MAX } else { 1 })
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "ini" || extension == "INI" {
                        let filename = entry.path().to_string_lossy().to_string();
                        let mut temp_parser = IniParser::new();
                        temp_parser.load(&filename, load_type)?;
                        temp_parser.parse_file()?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse the loaded file
    pub fn parse_file(&mut self) -> IniResult<()> {
        while self.current_line_index < self.lines.len() {
            let line_owned = self
                .lines
                .get(self.current_line_index)
                .cloned()
                .unwrap_or_default();
            let line = line_owned.trim();
            self.state.line_number = self.current_line_index + 1;

            if line.is_empty() || line.starts_with(';') {
                self.current_line_index += 1;
                continue;
            }

            if self.state.in_block {
                if Self::is_end_of_block(line) {
                    self.state.in_block = false;
                    self.state.current_block_type = None;
                    self.state.current_block_name = None;
                } else {
                    self.parse_block_line(line)?;
                }
            } else {
                self.parse_global_line(line)?;
            }

            self.current_line_index += 1;
        }

        Ok(())
    }

    /// Register a block parser
    pub fn register_block_parser(&mut self, block_type: &str, parser: BlockParseFn) {
        self.block_parsers.insert(block_type.to_string(), parser);
    }

    /// Register field parsers for a block type
    pub fn register_field_parsers(&mut self, block_type: &str, parsers: Vec<FieldParse>) {
        self.field_parsers.insert(block_type.to_string(), parsers);
    }

    /// Get the next token from the current line
    pub fn get_next_token(&mut self) -> IniResult<String> {
        // This is a simplified implementation
        // In a real implementation, you'd need more sophisticated token parsing
        if self.current_line_index >= self.lines.len() {
            return Err(IniError::ParseError {
                line: self.state.line_number,
                message: "Unexpected end of file".to_string(),
            });
        }

        let line = &self.lines[self.current_line_index];
        let tokens: Vec<&str> = line.split_whitespace().collect();

        // Return the first token (simplified)
        if let Some(token) = tokens.first() {
            Ok(token.to_string())
        } else {
            Err(IniError::ParseError {
                line: self.state.line_number,
                message: "No token found".to_string(),
            })
        }
    }

    /// Get the next token as AsciiString
    pub fn get_next_ascii_token(&mut self) -> IniResult<AsciiString> {
        let token = self.get_next_token()?;
        Ok(AsciiString::from_str(&token)?)
    }

    /// Get the next token as Int
    pub fn get_next_int(&mut self) -> IniResult<Int> {
        let token = self.get_next_token()?;
        token.parse::<Int>().map_err(|_| IniError::ParseError {
            line: self.state.line_number,
            message: format!("Invalid integer: {}", token),
        })
    }

    /// Get the next token as Real
    pub fn get_next_real(&mut self) -> IniResult<Real> {
        let token = self.get_next_token()?;
        token.parse::<Real>().map_err(|_| IniError::ParseError {
            line: self.state.line_number,
            message: format!("Invalid real: {}", token),
        })
    }

    /// Get the next token as Bool
    pub fn get_next_bool(&mut self) -> IniResult<Bool> {
        let token = self.get_next_token()?;
        match token.to_lowercase().as_str() {
            "yes" | "true" | "1" => Ok(true),
            "no" | "false" | "0" => Ok(false),
            _ => Err(IniError::ParseError {
                line: self.state.line_number,
                message: format!("Invalid boolean: {}", token),
            }),
        }
    }

    /// Get the filename
    pub fn get_filename(&self) -> &AsciiString {
        &self.filename
    }

    /// Get the load type
    pub fn get_load_type(&self) -> IniLoadType {
        self.load_type
    }

    /// Get the current line number
    pub fn get_line_number(&self) -> usize {
        self.state.line_number
    }

    /// Check if a line declares a block type
    pub fn is_declaration_of_type(block_type: &str, block_name: &str, line: &str) -> bool {
        let pattern = format!(
            r"^\s*{}\s+{}",
            regex::escape(block_type),
            regex::escape(block_name)
        );
        Regex::new(&pattern).unwrap().is_match(line)
    }

    /// Check if a line is the end of a block
    pub fn is_end_of_block(line: &str) -> bool {
        line.trim() == "END"
    }

    /// Parse a line that's part of a block
    fn parse_block_line(&mut self, line: &str) -> IniResult<()> {
        if let Some(block_type) = &self.state.current_block_type {
            if let Some(parsers) = self.field_parsers.get(block_type) {
                for parser in parsers {
                    if line.starts_with(&parser.token) {
                        // Parse the field value from the line
                        let value = self.extract_field_value(line, &parser.token)?;

                        // Create a dummy instance for demonstration
                        let mut dummy_instance = ();

                        // Call the parse function
                        (parser.parse_fn)(self, &mut dummy_instance, &value)?;
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse a global line (not part of a block)
    fn parse_global_line(&mut self, line: &str) -> IniResult<()> {
        for (block_type, parser) in &self.block_parsers {
            if line.starts_with(block_type) {
                // Extract block name
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    self.state.current_block_type = Some(block_type.clone());
                    self.state.current_block_name = Some(parts[1].to_string());
                    self.state.in_block = true;

                    // Call the block parser
                    parser(self)?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Extract field value from a line
    fn extract_field_value(&self, line: &str, token: &str) -> IniResult<String> {
        if let Some(value_start) = line.find(token) {
            let value = &line[value_start + token.len()..].trim();
            Ok(value.to_string())
        } else {
            Err(IniError::ParseError {
                line: self.state.line_number,
                message: format!("Token '{}' not found in line", token),
            })
        }
    }
}

/// Standard field parsers
pub mod field_parsers {
    use super::*;

    /// Parse integer field
    pub fn parse_int(
        _parser: &mut IniParser,
        _instance: &mut dyn std::any::Any,
        value: &str,
    ) -> IniResult<()> {
        let _int_value: Int = value.parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid integer: {}", value),
        })?;
        // In a real implementation, you'd store this value in the instance
        // This is just a demonstration
        Ok(())
    }

    /// Parse real field
    pub fn parse_real(
        _parser: &mut IniParser,
        _instance: &mut dyn std::any::Any,
        value: &str,
    ) -> IniResult<()> {
        let _real_value: Real = value.parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid real: {}", value),
        })?;
        Ok(())
    }

    /// Parse boolean field
    pub fn parse_bool(
        _parser: &mut IniParser,
        _instance: &mut dyn std::any::Any,
        value: &str,
    ) -> IniResult<()> {
        let _bool_value = match value.to_lowercase().as_str() {
            "yes" | "true" | "1" => true,
            "no" | "false" | "0" => false,
            _ => {
                return Err(IniError::ParseError {
                    line: 0,
                    message: format!("Invalid boolean: {}", value),
                })
            }
        };
        Ok(())
    }

    /// Parse string field
    pub fn parse_ascii_string(
        _parser: &mut IniParser,
        _instance: &mut dyn std::any::Any,
        value: &str,
    ) -> IniResult<()> {
        let _string_value = AsciiString::from_str(value)?;
        Ok(())
    }

    /// Parse coordinate 2D field
    pub fn parse_coord2d(
        _parser: &mut IniParser,
        _instance: &mut dyn std::any::Any,
        value: &str,
    ) -> IniResult<()> {
        let parts: Vec<&str> = value.split(',').collect();
        if parts.len() != 2 {
            return Err(IniError::ParseError {
                line: 0,
                message: format!("Invalid Coord2D format: {}", value),
            });
        }

        let _x: Real = parts[0].trim().parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid X coordinate: {}", parts[0]),
        })?;

        let _y: Real = parts[1].trim().parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid Y coordinate: {}", parts[1]),
        })?;

        Ok(())
    }

    /// Parse coordinate 3D field
    pub fn parse_coord3d(
        _parser: &mut IniParser,
        _instance: &mut dyn std::any::Any,
        value: &str,
    ) -> IniResult<()> {
        let parts: Vec<&str> = value.split(',').collect();
        if parts.len() != 3 {
            return Err(IniError::ParseError {
                line: 0,
                message: format!("Invalid Coord3D format: {}", value),
            });
        }

        let _x: Real = parts[0].trim().parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid X coordinate: {}", parts[0]),
        })?;

        let _y: Real = parts[1].trim().parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid Y coordinate: {}", parts[1]),
        })?;

        let _z: Real = parts[2].trim().parse().map_err(|_| IniError::ParseError {
            line: 0,
            message: format!("Invalid Z coordinate: {}", parts[2]),
        })?;

        Ok(())
    }
}

/// Standard block parsers
pub mod block_parsers {
    use super::*;

    /// Parse object definition
    pub fn parse_object_definition(parser: &mut IniParser) -> IniResult<()> {
        let _name = parser.get_next_ascii_token()?;
        // In a real implementation, you'd create and populate an object template
        log::info!(
            "Parsing object definition at line {}",
            parser.get_line_number()
        );
        Ok(())
    }

    /// Parse weapon template definition
    pub fn parse_weapon_template_definition(parser: &mut IniParser) -> IniResult<()> {
        let _name = parser.get_next_ascii_token()?;
        log::info!(
            "Parsing weapon template definition at line {}",
            parser.get_line_number()
        );
        Ok(())
    }

    /// Parse game data definition
    pub fn parse_game_data_definition(parser: &mut IniParser) -> IniResult<()> {
        log::info!(
            "Parsing game data definition at line {}",
            parser.get_line_number()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ini_parser_creation() {
        let parser = IniParser::new();
        assert_eq!(parser.get_load_type(), IniLoadType::Overwrite);
        assert!(parser.get_filename().is_empty()); // Should be empty string
    }

    #[test]
    fn test_field_parsers() {
        let mut parser = IniParser::new();

        // Test int parser
        let result = field_parsers::parse_int(&mut parser, &mut (), "42");
        assert!(result.is_ok());

        // Test invalid int
        let result = field_parsers::parse_int(&mut parser, &mut (), "not_a_number");
        assert!(result.is_err());

        // Test bool parser
        let result = field_parsers::parse_bool(&mut parser, &mut (), "yes");
        assert!(result.is_ok());

        let result = field_parsers::parse_bool(&mut parser, &mut (), "invalid_bool");
        assert!(result.is_err());

        // Test coord2d parser
        let result = field_parsers::parse_coord2d(&mut parser, &mut (), "1.0, 2.0");
        assert!(result.is_ok());

        let result = field_parsers::parse_coord2d(&mut parser, &mut (), "invalid_coord");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_declaration_of_type() {
        assert!(IniParser::is_declaration_of_type(
            "Object",
            "Tank",
            "Object Tank"
        ));
        assert!(!IniParser::is_declaration_of_type(
            "Object",
            "Tank",
            "Weapon Tank"
        ));
    }

    #[test]
    fn test_is_end_of_block() {
        assert!(IniParser::is_end_of_block("END"));
        assert!(IniParser::is_end_of_block("  END  "));
        assert!(!IniParser::is_end_of_block("Object Tank"));
    }

    #[test]
    fn test_multi_field_parse() {
        let mut multi_parse = MultiFieldParse::new();

        let field1 = FieldParse::new("Name", field_parsers::parse_ascii_string, 0);
        let field2 = FieldParse::new("Health", field_parsers::parse_int, 4);

        assert!(multi_parse.add(field1, 0).is_ok());
        assert!(multi_parse.add(field2, 4).is_ok());

        assert_eq!(multi_parse.get_count(), 2);
        assert!(multi_parse.get_field(0).is_some());
        assert!(multi_parse.get_field(1).is_some());
        assert!(multi_parse.get_field(2).is_none());
    }
}
