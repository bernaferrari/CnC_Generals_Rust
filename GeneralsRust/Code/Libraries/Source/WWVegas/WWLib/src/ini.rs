//! INI file handling utilities
//!
//! This module provides functionality for parsing and writing INI files,
//! compatible with the Command & Conquer Generals WWLib INI format.
//!
//! # Features
//!
//! - Parse INI files with sections, keys, values, and comments
//! - Support for various data types (int, float, bool, string, hex)
//! - Case-insensitive key/section lookup
//! - File I/O operations with proper error handling
//! - Memory-safe implementation in Rust
//!
//! # Example
//!
//! ```
//! use wwlib_rust::ini::INIClass;
//! use std::io::Cursor;
//!
//! let ini_content = r#"
//! ; This is a comment
//! [Section1]
//! Key1=Value1
//! Number=42
//! Flag=true
//!
//! [Section2]
//! Float=3.14
//! Hex=0xFF
//! "#;
//!
//! let mut ini = INIClass::new();
//! ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();
//!
//! // Read values
//! assert_eq!(ini.get_string("Section1", "Key1", "default"), "Value1");
//! assert_eq!(ini.get_int("Section1", "Number", 0), 42);
//! assert_eq!(ini.get_bool("Section1", "Flag", false), true);
//! assert_eq!(ini.get_float("Section2", "Float", 0.0), 3.14);
//! assert_eq!(ini.get_hex("Section2", "Hex", 0), 255);
//!
//! // Write values
//! ini.put_string("Section3", "NewKey", "NewValue");
//! ini.put_int("Section3", "NewNumber", 100);
//! ```

use crate::base64::{base64_decode, base64_encode};
use crate::crc::Crc32;
use crate::pk::PKey;
use crate::trim::strtrim;
use ::base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use ::base64::Engine;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

/// Maximum line length for INI parsing (matches C++ implementation)
const MAX_LINE_LENGTH: usize = 4096;

/// Error types for INI operations
#[derive(Debug, Clone, PartialEq)]
pub enum INIError {
    /// I/O error occurred
    IoError(String),
    /// Parse error with line number and description
    ParseError(usize, String),
    /// Section not found
    SectionNotFound(String),
    /// Entry not found in section
    EntryNotFound(String, String),
    /// Invalid data type conversion
    InvalidDataType(String),
    /// Line too long
    LineTooLong(usize),
}

impl std::fmt::Display for INIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            INIError::IoError(msg) => write!(f, "I/O error: {}", msg),
            INIError::ParseError(line, msg) => write!(f, "Parse error at line {}: {}", line, msg),
            INIError::SectionNotFound(section) => write!(f, "Section not found: {}", section),
            INIError::EntryNotFound(section, entry) => {
                write!(f, "Entry '{}' not found in section '{}'", entry, section)
            }
            INIError::InvalidDataType(msg) => write!(f, "Invalid data type: {}", msg),
            INIError::LineTooLong(line) => {
                write!(
                    f,
                    "Line {} exceeds maximum length of {} characters",
                    line, MAX_LINE_LENGTH
                )
            }
        }
    }
}

impl std::error::Error for INIError {}

impl From<io::Error> for INIError {
    fn from(error: io::Error) -> Self {
        INIError::IoError(error.to_string())
    }
}

/// Result type for INI operations
pub type INIResult<T> = Result<T, INIError>;

/// Represents a single entry (key-value pair) in an INI section
#[derive(Debug, Clone, PartialEq)]
pub struct INIEntry {
    /// The entry key name
    pub key: String,
    /// The entry value
    pub value: String,
}

impl INIEntry {
    /// Create a new INI entry
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }

    /// Get the CRC hash for this entry (for indexing)
    pub fn index_id(&self) -> u32 {
        Crc32::string(&self.key.to_lowercase())
    }
}

/// Represents a section in an INI file
#[derive(Debug, Clone, PartialEq)]
pub struct INISection {
    /// The section name
    pub name: String,
    /// List of entries in this section
    pub entries: Vec<INIEntry>,
    /// Index for fast lookup by CRC
    entry_index: HashMap<u32, usize>,
}

impl INISection {
    /// Create a new INI section
    pub fn new(name: String) -> Self {
        Self {
            name,
            entries: Vec::new(),
            entry_index: HashMap::new(),
        }
    }

    /// Get the CRC hash for this section (for indexing)
    pub fn index_id(&self) -> u32 {
        Crc32::string(&self.name.to_lowercase())
    }

    /// Find an entry by key (case-insensitive)
    pub fn find_entry(&self, key: &str) -> Option<&INIEntry> {
        let key_crc = Crc32::string(&key.to_lowercase());
        if let Some(&index) = self.entry_index.get(&key_crc) {
            self.entries.get(index)
        } else {
            None
        }
    }

    /// Find an entry by key (case-insensitive, mutable)
    pub fn find_entry_mut(&mut self, key: &str) -> Option<&mut INIEntry> {
        let key_crc = Crc32::string(&key.to_lowercase());
        if let Some(&index) = self.entry_index.get(&key_crc) {
            self.entries.get_mut(index)
        } else {
            None
        }
    }

    /// Add or update an entry
    pub fn put_entry(&mut self, key: String, value: String) {
        let key_crc = Crc32::string(&key.to_lowercase());

        if let Some(&index) = self.entry_index.get(&key_crc) {
            // Update existing entry
            self.entries[index].value = value;
        } else {
            // Add new entry
            let entry = INIEntry::new(key, value);
            let index = self.entries.len();
            self.entries.push(entry);
            self.entry_index.insert(key_crc, index);
        }
    }

    /// Remove an entry by key
    pub fn remove_entry(&mut self, key: &str) -> bool {
        let key_crc = Crc32::string(&key.to_lowercase());

        if let Some(&index) = self.entry_index.get(&key_crc) {
            self.entries.remove(index);
            self.entry_index.remove(&key_crc);

            // Update indices for entries after the removed one
            for (_, idx) in self.entry_index.iter_mut() {
                if *idx > index {
                    *idx -= 1;
                }
            }

            true
        } else {
            false
        }
    }

    /// Get the number of entries in this section
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get an entry by index
    pub fn get_entry(&self, index: usize) -> Option<&INIEntry> {
        self.entries.get(index)
    }
}

/// Main INI file handler class
#[derive(Debug, Clone)]
pub struct INIClass {
    /// List of sections
    sections: Vec<INISection>,
    /// Index for fast section lookup by CRC
    section_index: HashMap<u32, usize>,
    /// Filename (if loaded from file)
    filename: Option<String>,
    /// Whether to keep blank entries (default: false)
    keep_blank_entries: bool,
}

impl Default for INIClass {
    fn default() -> Self {
        Self::new()
    }
}

impl INIClass {
    /// Create a new empty INI handler
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            section_index: HashMap::new(),
            filename: None,
            keep_blank_entries: false,
        }
    }

    /// Create an INI handler and load from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> INIResult<Self> {
        let mut ini = Self::new();
        ini.load(path)?;
        Ok(ini)
    }

    /// Set whether to keep blank entries (entries with empty values)
    pub fn set_keep_blank_entries(&mut self, keep: bool) {
        self.keep_blank_entries = keep;
    }

    /// Check if the INI has been loaded with data
    pub fn is_loaded(&self) -> bool {
        !self.sections.is_empty()
    }

    /// Get the filename (if loaded from file)
    pub fn get_filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Clear all data or specific section/entry
    pub fn clear(&mut self, section: Option<&str>, entry: Option<&str>) -> bool {
        match (section, entry) {
            (None, None) => {
                // Clear all data
                self.sections.clear();
                self.section_index.clear();
                true
            }
            (Some(section_name), None) => {
                // Clear entire section
                let section_crc = Crc32::string(&section_name.to_lowercase());
                if let Some(&index) = self.section_index.get(&section_crc) {
                    self.sections.remove(index);
                    self.section_index.remove(&section_crc);

                    // Update indices for sections after the removed one
                    for (_, idx) in self.section_index.iter_mut() {
                        if *idx > index {
                            *idx -= 1;
                        }
                    }
                    true
                } else {
                    false
                }
            }
            (Some(section_name), Some(entry_name)) => {
                // Clear specific entry
                if let Some(section) = self.find_section_mut(section_name) {
                    section.remove_entry(entry_name)
                } else {
                    false
                }
            }
            (None, Some(_)) => {
                // Invalid: entry specified without section
                false
            }
        }
    }

    /// Get the number of sections
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Check if a section is present
    pub fn section_present(&self, section: &str) -> bool {
        self.find_section(section).is_some()
    }

    /// Get the number of entries in a section
    pub fn entry_count(&self, section: &str) -> usize {
        self.find_section(section)
            .map(|s| s.entry_count())
            .unwrap_or(0)
    }

    /// Get an entry key by index in a section
    pub fn get_entry(&self, section: &str, index: usize) -> Option<&str> {
        self.find_section(section)
            .and_then(|s| s.get_entry(index))
            .map(|e| e.key.as_str())
    }

    /// Check if section and/or entry is present
    pub fn is_present(&self, section: &str, entry: Option<&str>) -> bool {
        match entry {
            None => self.section_present(section),
            Some(entry_name) => self.find_entry(section, entry_name).is_some(),
        }
    }

    /// Find a section by name (case-insensitive)
    pub fn find_section(&self, name: &str) -> Option<&INISection> {
        let name_crc = Crc32::string(&name.to_lowercase());
        if let Some(&index) = self.section_index.get(&name_crc) {
            self.sections.get(index)
        } else {
            None
        }
    }

    /// Find a section by name (case-insensitive, mutable)
    pub fn find_section_mut(&mut self, name: &str) -> Option<&mut INISection> {
        let name_crc = Crc32::string(&name.to_lowercase());
        if let Some(&index) = self.section_index.get(&name_crc) {
            self.sections.get_mut(index)
        } else {
            None
        }
    }

    /// Find an entry by section and key name
    pub fn find_entry(&self, section: &str, key: &str) -> Option<&INIEntry> {
        self.find_section(section)?.find_entry(key)
    }

    /// Load INI data from a file
    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> INIResult<()> {
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        self.filename = Some(path.as_ref().to_string_lossy().into_owned());
        self.load_from_reader(&mut reader)
    }

    /// Load INI data from a reader
    pub fn load_from_reader<R: BufRead>(&mut self, reader: &mut R) -> INIResult<()> {
        let mut current_section: Option<INISection> = None;
        let mut line_number = 0;

        for line_result in reader.lines() {
            line_number += 1;
            let mut line = line_result?;

            // Check line length
            if line.len() > MAX_LINE_LENGTH {
                return Err(INIError::LineTooLong(line_number));
            }

            // Strip comments and trim whitespace
            self.strip_comments(&mut line);
            line = line.trim().to_string();

            // Skip empty lines and comment lines
            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            // Check for section header
            if line.starts_with('[') && line.ends_with(']') {
                // Save previous section if it exists
                if let Some(section) = current_section.take() {
                    if !section.entries.is_empty() {
                        self.add_section(section);
                    }
                }

                // Start new section
                let section_name = line[1..line.len() - 1].trim().to_string();
                if section_name.is_empty() {
                    return Err(INIError::ParseError(
                        line_number,
                        "Empty section name".to_string(),
                    ));
                }
                current_section = Some(INISection::new(section_name));
                continue;
            }

            // Parse key=value entries
            if let Some(eq_pos) = line.find('=') {
                if eq_pos == 0 {
                    // Skip lines starting with '='
                    continue;
                }

                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                if key.is_empty() {
                    continue;
                }

                // Handle blank entries
                let value_to_store = if value.is_empty() {
                    if self.keep_blank_entries {
                        " ".to_string()
                    } else {
                        continue;
                    }
                } else {
                    value.to_string()
                };

                // Add entry to current section
                match &mut current_section {
                    Some(section) => {
                        section.put_entry(key.to_string(), value_to_store);
                    }
                    None => {
                        return Err(INIError::ParseError(
                            line_number,
                            "Entry found outside of section".to_string(),
                        ));
                    }
                }
            }
        }

        // Save the last section
        if let Some(section) = current_section {
            if !section.entries.is_empty() {
                self.add_section(section);
            }
        }

        Ok(())
    }

    /// Add a section to the INI
    fn add_section(&mut self, section: INISection) {
        let section_crc = section.index_id();

        // Check for existing section with same CRC (merge or replace)
        if let Some(&index) = self.section_index.get(&section_crc) {
            // Replace existing section
            self.sections[index] = section;
        } else {
            // Add new section
            let index = self.sections.len();
            self.sections.push(section);
            self.section_index.insert(section_crc, index);
        }
    }

    /// Save INI data to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> INIResult<()> {
        let mut file = File::create(&path)?;
        self.save_to_writer(&mut file)?;
        Ok(())
    }

    /// Save INI data to a writer
    pub fn save_to_writer<W: Write>(&self, writer: &mut W) -> INIResult<()> {
        for (i, section) in self.sections.iter().enumerate() {
            // Add blank line between sections (except before first)
            if i > 0 {
                writeln!(writer)?;
            }

            // Write section header
            writeln!(writer, "[{}]", section.name)?;

            // Write entries
            for entry in &section.entries {
                writeln!(writer, "{}={}", entry.key, entry.value)?;
            }
        }
        Ok(())
    }

    /// Strip comments from a line (everything after ';')
    fn strip_comments(&self, line: &mut String) {
        if let Some(pos) = line.find(';') {
            line.truncate(pos);
            *line = line.trim_end().to_string();
        }
    }

    /// Get a string value from the INI
    pub fn get_string(&self, section: &str, key: &str, default: &str) -> String {
        self.find_entry(section, key)
            .map(|entry| entry.value.clone())
            .unwrap_or_else(|| default.to_string())
    }

    /// Get an integer value from the INI
    pub fn get_int(&self, section: &str, key: &str, default: i32) -> i32 {
        self.find_entry(section, key)
            .and_then(|entry| {
                let value = entry.value.trim();

                // Handle hex values starting with '$'
                if value.starts_with('$') {
                    i32::from_str_radix(&value[1..], 16).ok()
                } else if value
                    .chars()
                    .last()
                    .map(|c| c.eq_ignore_ascii_case(&'h'))
                    .unwrap_or(false)
                {
                    let trimmed = &value[..value.len().saturating_sub(1)];
                    i32::from_str_radix(trimmed, 16).ok()
                } else {
                    value.parse().ok()
                }
            })
            .unwrap_or(default)
    }

    /// Get a hexadecimal integer value from the INI
    pub fn get_hex(&self, section: &str, key: &str, default: i32) -> i32 {
        self.find_entry(section, key)
            .and_then(|entry| {
                let value = entry.value.trim();

                // Handle various hex formats
                if value.starts_with("0x") || value.starts_with("0X") {
                    i32::from_str_radix(&value[2..], 16).ok()
                } else if value.starts_with('$') {
                    i32::from_str_radix(&value[1..], 16).ok()
                } else {
                    // Try parsing as hex without prefix
                    i32::from_str_radix(value, 16).ok()
                }
            })
            .unwrap_or(default)
    }

    /// Get a floating point value from the INI
    pub fn get_float(&self, section: &str, key: &str, default: f32) -> f32 {
        self.find_entry(section, key)
            .and_then(|entry| entry.value.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Get a double precision floating point value from the INI
    pub fn get_double(&self, section: &str, key: &str, default: f64) -> f64 {
        self.find_entry(section, key)
            .and_then(|entry| entry.value.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Get a boolean value from the INI
    pub fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        self.find_entry(section, key)
            .and_then(|entry| {
                let value = entry.value.trim();
                if value.is_empty() {
                    return Some(default);
                }

                match value.chars().next().unwrap().to_ascii_uppercase() {
                    'Y' | 'T' | '1' => Some(true),
                    'N' | 'F' | '0' => Some(false),
                    _ => {
                        // Try parsing as integer
                        value.parse::<i32>().ok().map(|n| n != 0)
                    }
                }
            })
            .unwrap_or(default)
    }

    /// Store a binary encoded data block into the INI database (base64, multi-entry).
    pub fn put_uublock(&mut self, section: &str, block: &[u8]) -> bool {
        if section.is_empty() || block.is_empty() {
            return false;
        }

        self.clear(Some(section), None);

        let encoded = BASE64_STANDARD.encode(block);
        let mut counter = 1usize;
        let mut offset = 0usize;

        while offset < encoded.len() {
            let end = (offset + 70).min(encoded.len());
            let chunk = &encoded[offset..end];
            let entry = counter.to_string();
            self.put_string(section, &entry, chunk);
            counter += 1;
            offset = end;
        }

        true
    }

    /// Store a binary encoded data block into a specific INI entry (base64, single entry).
    pub fn put_uublock_entry(&mut self, section: &str, entry: &str, block: &[u8]) -> bool {
        if section.is_empty() || entry.is_empty() || block.is_empty() {
            return false;
        }

        let encoded = BASE64_STANDARD.encode(block);
        self.put_string(section, entry, &encoded)
    }

    /// Fetch a binary encoded data block from a section (base64, multi-entry).
    pub fn get_uublock(&self, section: &str, block: &mut [u8]) -> usize {
        if section.is_empty() {
            return 0;
        }

        let mut combined = String::new();
        let entry_count = self.entry_count(section);

        for index in 0..entry_count {
            if let Some(entry) = self.get_entry(section, index) {
                let value = self.get_string(section, entry, "=");
                combined.push_str(&value);
            }
        }

        if combined.is_empty() {
            return 0;
        }

        match BASE64_STANDARD.decode(combined.as_bytes()) {
            Ok(decoded) => {
                let len = decoded.len().min(block.len());
                block[..len].copy_from_slice(&decoded[..len]);
                len
            }
            Err(_) => 0,
        }
    }

    /// Fetch a binary encoded data block from a specific INI entry (base64).
    pub fn get_uublock_entry(&self, section: &str, entry: &str, block: &mut [u8]) -> usize {
        if section.is_empty() || entry.is_empty() {
            return 0;
        }

        let value = self.get_string(section, entry, "=");
        if value.is_empty() {
            return 0;
        }

        match BASE64_STANDARD.decode(value.as_bytes()) {
            Ok(decoded) => {
                let len = decoded.len().min(block.len());
                block[..len].copy_from_slice(&decoded[..len]);
                len
            }
            Err(_) => 0,
        }
    }

    /// Put a string value into the INI
    pub fn put_string(&mut self, section: &str, key: &str, value: &str) -> bool {
        self.ensure_section_exists(section);

        if let Some(sec) = self.find_section_mut(section) {
            sec.put_entry(key.to_string(), value.to_string());
            true
        } else {
            false
        }
    }

    /// Put an integer value into the INI
    pub fn put_int(&mut self, section: &str, key: &str, value: i32) -> bool {
        self.put_string(section, key, &value.to_string())
    }

    /// Put an integer value into the INI with a specific format.
    /// format: 0 = decimal, 1 = hex with trailing 'h', 2 = hex with leading '$'.
    pub fn put_int_format(&mut self, section: &str, key: &str, value: i32, format: i32) -> bool {
        let formatted = match format {
            1 => format!("{:X}h", value),
            2 => format!("${:X}", value),
            _ => format!("{}", value),
        };
        self.put_string(section, key, &formatted)
    }

    /// Put a hexadecimal integer value into the INI
    pub fn put_hex(&mut self, section: &str, key: &str, value: i32) -> bool {
        self.put_string(section, key, &format!("0x{:X}", value))
    }

    /// Put a floating point value into the INI
    pub fn put_float(&mut self, section: &str, key: &str, value: f32) -> bool {
        self.put_string(section, key, &value.to_string())
    }

    /// Put a double precision floating point value into the INI
    pub fn put_double(&mut self, section: &str, key: &str, value: f64) -> bool {
        self.put_string(section, key, &value.to_string())
    }

    /// Put a boolean value into the INI
    pub fn put_bool(&mut self, section: &str, key: &str, value: bool) -> bool {
        self.put_string(section, key, if value { "true" } else { "false" })
    }

    /// Ensure a section exists, create it if it doesn't
    fn ensure_section_exists(&mut self, section: &str) {
        if self.find_section(section).is_none() {
            let new_section = INISection::new(section.to_string());
            self.add_section(new_section);
        }
    }

    /// Count entries with a specific prefix followed by numbers in a range
    pub fn enumerate_entries(&self, section: &str, prefix: &str, start: u32, end: u32) -> u32 {
        let sec = match self.find_section(section) {
            Some(s) => s,
            None => return 0,
        };

        let mut count = 0;
        for num in start..=end {
            let key = format!("{}{}", prefix, num);
            if sec.find_entry(&key).is_some() {
                count += 1;
            }
        }
        count
    }

    /// Store a text block into a section, splitting into numbered entries.
    pub fn put_text_block(&mut self, section: &str, text: &str) -> bool {
        if section.is_empty() {
            return false;
        }

        self.clear(Some(section), None);
        let mut index = 1usize;
        let bytes = text.as_bytes();
        let mut offset = 0usize;

        while offset < bytes.len() {
            let mut chunk_len = (bytes.len() - offset).min(75);
            if chunk_len == 75 {
                let mut scan = chunk_len;
                while scan > 0 {
                    let c = bytes[offset + scan - 1];
                    if c.is_ascii_whitespace() {
                        chunk_len = scan - 1;
                        break;
                    }
                    scan -= 1;
                }
                if chunk_len == 75 {
                    break;
                }
            }

            let mut buffer = Vec::with_capacity(chunk_len + 1);
            buffer.extend_from_slice(&bytes[offset..offset + chunk_len]);
            buffer.push(0);
            strtrim(&mut buffer);
            let trimmed = String::from_utf8_lossy(
                &buffer[..buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len())],
            )
            .to_string();

            let entry = index.to_string();
            self.put_string(section, &entry, &trimmed);
            index += 1;

            offset += chunk_len.max(1);
        }

        true
    }

    /// Retrieve a text block from a section, concatenating numbered entries with spaces.
    pub fn get_text_block(&self, section: &str, max_len: usize) -> String {
        if max_len <= 1 {
            return String::new();
        }

        let mut output = String::new();
        let entry_count = self.entry_count(section);
        let mut remaining = max_len - 1;

        for index in 0..entry_count {
            if remaining == 0 {
                break;
            }
            if index > 0 {
                if remaining == 0 {
                    break;
                }
                output.push(' ');
                remaining = remaining.saturating_sub(1);
            }

            if let Some(entry_key) = self.get_entry(section, index) {
                let value = self.get_string(section, entry_key, "");
                let take_len = value.len().min(remaining);
                output.push_str(&value[..take_len]);
                remaining = remaining.saturating_sub(take_len);
            }
        }

        output
    }

    /// Store a UTF-16 wide string into the INI using base64 encoding.
    pub fn put_wide_string_utf16(&mut self, section: &str, entry: &str, string: &[u16]) -> bool {
        if section.is_empty() || entry.is_empty() {
            return false;
        }

        if string.is_empty() {
            return self.put_string(section, entry, "");
        }

        let mut bytes = Vec::with_capacity((string.len() + 1) * 2);
        for &ch in string {
            bytes.extend_from_slice(&ch.to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());

        let mut encoded = vec![0u8; (bytes.len() * 4 / 3) + 8];
        let out_len = base64_encode(&bytes, &mut encoded);
        let encoded_str = String::from_utf8_lossy(&encoded[..out_len]).to_string();
        self.put_string(section, entry, &encoded_str)
    }

    /// Retrieve a UTF-16 wide string from the INI (base64 encoded).
    pub fn get_wide_string_utf16(&self, section: &str, entry: &str, default: &[u16]) -> Vec<u16> {
        let value = self.get_string(section, entry, "");
        if value.is_empty() {
            return default.to_vec();
        }

        let mut decoded = vec![0u8; value.len()];
        let len = base64_decode(value.as_bytes(), &mut decoded);
        let mut out = Vec::new();
        let mut i = 0usize;
        while i + 1 < len {
            let ch = u16::from_le_bytes([decoded[i], decoded[i + 1]]);
            if ch == 0 {
                break;
            }
            out.push(ch);
            i += 2;
        }
        if out.is_empty() {
            default.to_vec()
        } else {
            out
        }
    }

    /// Store a public/private key pair into the INI database.
    pub fn put_pkey(&mut self, key: &PKey) -> bool {
        let mut buffer = vec![0u8; 512];
        let len = key.encode_modulus(&mut buffer);
        self.put_uublock("PublicKey", &buffer[..len]);

        let len = key.encode_exponent(&mut buffer);
        self.put_uublock("PrivateKey", &buffer[..len]);
        true
    }

    /// Retrieve a key from the INI database.
    pub fn get_pkey(&self, fast: bool) -> PKey {
        let mut key = PKey::new();
        let mut buffer = vec![0u8; 512];

        if fast {
            let exp = PKey::fast_exponent();
            let encoded = exp.der_encode();
            key.decode_exponent(&encoded);
        } else {
            let len = self.get_uublock("PrivateKey", &mut buffer);
            key.decode_exponent(&buffer[..len]);
        }

        let len = self.get_uublock("PublicKey", &mut buffer);
        key.decode_modulus(&buffer[..len]);
        key
    }

    /// Get all section names
    pub fn get_section_names(&self) -> Vec<&str> {
        self.sections.iter().map(|s| s.name.as_str()).collect()
    }

    /// Get all entry keys in a section
    pub fn get_entry_keys(&self, section: &str) -> Vec<&str> {
        self.find_section(section)
            .map(|s| s.entries.iter().map(|e| e.key.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get the total size (approximate) of the INI data in bytes
    pub fn size(&self) -> usize {
        let mut total = 0;
        for section in &self.sections {
            total += section.name.len() + 4; // "[]\r\n"
            for entry in &section.entries {
                total += entry.key.len() + entry.value.len() + 3; // "=\r\n"
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_empty_ini() {
        let ini = INIClass::new();
        assert!(!ini.is_loaded());
        assert_eq!(ini.section_count(), 0);
        assert_eq!(ini.get_string("test", "key", "default"), "default");
    }

    #[test]
    fn test_basic_parsing() {
        let ini_content = r#"
[Section1]
Key1=Value1
Number=42
Float=3.14

[Section2]
Flag=true
Hex=0xFF
"#;

        let mut ini = INIClass::new();
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();

        assert!(ini.is_loaded());
        assert_eq!(ini.section_count(), 2);
        assert!(ini.section_present("Section1"));
        assert!(ini.section_present("Section2"));
        assert!(!ini.section_present("NonExistent"));

        assert_eq!(ini.get_string("Section1", "Key1", ""), "Value1");
        assert_eq!(ini.get_int("Section1", "Number", 0), 42);
        assert_eq!(ini.get_float("Section1", "Float", 0.0), 3.14);
        assert_eq!(ini.get_bool("Section2", "Flag", false), true);
        assert_eq!(ini.get_hex("Section2", "Hex", 0), 255);
    }

    #[test]
    fn test_comments() {
        let ini_content = r#"
; This is a comment
[Section1]
Key1=Value1 ; This is also a comment
; Another comment
Key2=Value2
"#;

        let mut ini = INIClass::new();
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();

        assert_eq!(ini.get_string("Section1", "Key1", ""), "Value1");
        assert_eq!(ini.get_string("Section1", "Key2", ""), "Value2");
    }

    #[test]
    fn test_case_insensitive() {
        let ini_content = r#"
[Section1]
Key1=Value1
"#;

        let mut ini = INIClass::new();
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();

        // Test case insensitive section and key lookup
        assert_eq!(ini.get_string("section1", "key1", ""), "Value1");
        assert_eq!(ini.get_string("SECTION1", "KEY1", ""), "Value1");
        assert_eq!(ini.get_string("Section1", "Key1", ""), "Value1");
    }

    #[test]
    fn test_blank_entries() {
        let ini_content = r#"
[Section1]
BlankKey=
NotBlank=Value
"#;

        // Test with keep_blank_entries = false (default)
        let mut ini = INIClass::new();
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();
        assert_eq!(ini.get_string("Section1", "BlankKey", "default"), "default");
        assert_eq!(ini.get_string("Section1", "NotBlank", ""), "Value");

        // Test with keep_blank_entries = true
        let mut ini = INIClass::new();
        ini.set_keep_blank_entries(true);
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();
        assert_eq!(ini.get_string("Section1", "BlankKey", "default"), " ");
        assert_eq!(ini.get_string("Section1", "NotBlank", ""), "Value");
    }

    #[test]
    fn test_boolean_parsing() {
        let ini_content = r#"
[Booleans]
True1=true
True2=yes
True3=y
True4=1
True5=True
False1=false
False2=no
False3=n
False4=0
False5=False
"#;

        let mut ini = INIClass::new();
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();

        assert_eq!(ini.get_bool("Booleans", "True1", false), true);
        assert_eq!(ini.get_bool("Booleans", "True2", false), true);
        assert_eq!(ini.get_bool("Booleans", "True3", false), true);
        assert_eq!(ini.get_bool("Booleans", "True4", false), true);
        assert_eq!(ini.get_bool("Booleans", "True5", false), true);
        assert_eq!(ini.get_bool("Booleans", "False1", true), false);
        assert_eq!(ini.get_bool("Booleans", "False2", true), false);
        assert_eq!(ini.get_bool("Booleans", "False3", true), false);
        assert_eq!(ini.get_bool("Booleans", "False4", true), false);
        assert_eq!(ini.get_bool("Booleans", "False5", true), false);
    }

    #[test]
    fn test_hex_parsing() {
        let ini_content = r#"
[Hex]
Hex1=0xFF
Hex2=0x10
Hex3=$A0
Hex4=FF
"#;

        let mut ini = INIClass::new();
        ini.load_from_reader(&mut Cursor::new(ini_content)).unwrap();

        assert_eq!(ini.get_hex("Hex", "Hex1", 0), 255);
        assert_eq!(ini.get_hex("Hex", "Hex2", 0), 16);
        assert_eq!(ini.get_hex("Hex", "Hex3", 0), 160);
        assert_eq!(ini.get_hex("Hex", "Hex4", 0), 255);

        // Also test get_int with $ prefix
        assert_eq!(ini.get_int("Hex", "Hex3", 0), 160);
    }

    #[test]
    fn test_put_operations() {
        let mut ini = INIClass::new();

        ini.put_string("Section1", "Key1", "Value1");
        ini.put_int("Section1", "Number", 42);
        ini.put_float("Section1", "Float", 3.14);
        ini.put_bool("Section1", "Flag", true);
        ini.put_hex("Section1", "Hex", 255);

        assert_eq!(ini.get_string("Section1", "Key1", ""), "Value1");
        assert_eq!(ini.get_int("Section1", "Number", 0), 42);
        assert_eq!(ini.get_float("Section1", "Float", 0.0), 3.14);
        assert_eq!(ini.get_bool("Section1", "Flag", false), true);
        assert_eq!(ini.get_hex("Section1", "Hex", 0), 255);
    }

    #[test]
    fn test_save_and_load() {
        let mut ini1 = INIClass::new();
        ini1.put_string("Section1", "Key1", "Value1");
        ini1.put_int("Section1", "Number", 42);
        ini1.put_bool("Section2", "Flag", true);

        // Save to string
        let mut output = Vec::new();
        ini1.save_to_writer(&mut output).unwrap();
        let saved_content = String::from_utf8(output).unwrap();

        // Load into new INI
        let mut ini2 = INIClass::new();
        ini2.load_from_reader(&mut Cursor::new(saved_content))
            .unwrap();

        // Verify data matches
        assert_eq!(ini2.get_string("Section1", "Key1", ""), "Value1");
        assert_eq!(ini2.get_int("Section1", "Number", 0), 42);
        assert_eq!(ini2.get_bool("Section2", "Flag", false), true);
    }

    #[test]
    fn test_clear_operations() {
        let mut ini = INIClass::new();
        ini.put_string("Section1", "Key1", "Value1");
        ini.put_string("Section1", "Key2", "Value2");
        ini.put_string("Section2", "Key3", "Value3");

        // Clear specific entry
        assert!(ini.clear(Some("Section1"), Some("Key1")));
        assert_eq!(ini.get_string("Section1", "Key1", "default"), "default");
        assert_eq!(ini.get_string("Section1", "Key2", ""), "Value2");

        // Clear entire section
        assert!(ini.clear(Some("Section1"), None));
        assert!(!ini.section_present("Section1"));
        assert!(ini.section_present("Section2"));

        // Clear all
        ini.clear(None, None);
        assert_eq!(ini.section_count(), 0);
    }

    #[test]
    fn test_enumerate_entries() {
        let mut ini = INIClass::new();
        ini.put_string("Section1", "Item1", "Value1");
        ini.put_string("Section1", "Item3", "Value3");
        ini.put_string("Section1", "Item5", "Value5");
        ini.put_string("Section1", "Other", "Value");

        assert_eq!(ini.enumerate_entries("Section1", "Item", 1, 10), 3);
        assert_eq!(ini.enumerate_entries("Section1", "Item", 1, 3), 2);
        assert_eq!(ini.enumerate_entries("Section1", "NotFound", 1, 10), 0);
    }

    #[test]
    fn test_utility_methods() {
        let mut ini = INIClass::new();
        ini.put_string("Section1", "Key1", "Value1");
        ini.put_string("Section1", "Key2", "Value2");
        ini.put_string("Section2", "Key3", "Value3");

        // Test section names
        let sections = ini.get_section_names();
        assert_eq!(sections.len(), 2);
        assert!(sections.contains(&"Section1"));
        assert!(sections.contains(&"Section2"));

        // Test entry keys
        let keys = ini.get_entry_keys("Section1");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"Key1"));
        assert!(keys.contains(&"Key2"));

        // Test size
        assert!(ini.size() > 0);

        // Test entry count
        assert_eq!(ini.entry_count("Section1"), 2);
        assert_eq!(ini.entry_count("Section2"), 1);
        assert_eq!(ini.entry_count("NonExistent"), 0);

        // Test is_present
        assert!(ini.is_present("Section1", None));
        assert!(ini.is_present("Section1", Some("Key1")));
        assert!(!ini.is_present("Section1", Some("NonExistent")));
        assert!(!ini.is_present("NonExistent", None));
    }

    #[test]
    fn test_error_handling() {
        let mut ini = INIClass::new();

        // Test entry outside section
        let invalid_content = "Key=Value\n[Section1]\nKey2=Value2";
        let result = ini.load_from_reader(&mut Cursor::new(invalid_content));
        assert!(matches!(result, Err(INIError::ParseError(_, _))));

        // Test empty section name
        let invalid_content2 = "[]\nKey=Value";
        let result2 = ini.load_from_reader(&mut Cursor::new(invalid_content2));
        assert!(matches!(result2, Err(INIError::ParseError(_, _))));
    }
}
