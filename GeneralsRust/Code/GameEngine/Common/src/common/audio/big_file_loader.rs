//! BIG File Audio Loader
//! 
//! This module provides support for loading audio files from Command & Conquer
//! BIG archive files, specifically AudioEnglishZH.big and AudioZH.big.
//! It handles the BIG file format parsing and audio asset extraction.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::common::audio::{AudioEventRts, Real, Bool, UnsignedInt, AsciiString};

/// Magic bytes for BIG file format identification
const BIG_MAGIC: &[u8] = b"BIGF";
const BIG_VERSION: u32 = 4;

/// BIG file header structure
#[derive(Debug, Clone)]
struct BigHeader {
    magic: [u8; 4],
    total_size: u32,
    num_entries: u32,
    first_file_offset: u32,
}

impl BigHeader {
    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 16 {
            return Err("BIG header too short".to_string());
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[0..4]);
        
        if &magic != BIG_MAGIC {
            return Err(format!("Invalid BIG magic: {:?}, expected: {:?}", magic, BIG_MAGIC));
        }

        Ok(Self {
            magic,
            total_size: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            num_entries: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            first_file_offset: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        })
    }
}

/// BIG file entry structure
#[derive(Debug, Clone)]
pub struct BigEntry {
    pub offset: u32,
    pub size: u32,
    pub name: String,
    pub name_hash: u32,
}

impl BigEntry {
    fn calculate_hash(name: &str) -> u32 {
        // BIG files use a specific hash algorithm for file names
        let mut hash = 0u32;
        for byte in name.to_uppercase().bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
        }
        hash
    }
}

/// BIG file reader for audio assets
pub struct BigFileReader {
    file_path: PathBuf,
    file_size: u64,
    header: BigHeader,
    entries: HashMap<String, BigEntry>,
    name_to_entry: HashMap<String, BigEntry>,
    hash_to_entry: HashMap<u32, BigEntry>,
}

impl BigFileReader {
    /// Open and parse a BIG file
    pub fn open<P: AsRef<Path>>(file_path: P) -> Result<Self, String> {
        let file_path = file_path.as_ref().to_path_buf();
        let mut file = File::open(&file_path)
            .map_err(|e| format!("Failed to open BIG file {}: {}", file_path.display(), e))?;

        // Get file size
        let file_size = file.metadata()
            .map_err(|e| format!("Failed to get BIG file metadata: {}", e))?
            .len();

        // Read header
        let mut header_data = [0u8; 16];
        file.read_exact(&mut header_data)
            .map_err(|e| format!("Failed to read BIG header: {}", e))?;

        let header = BigHeader::from_bytes(&header_data)?;

        if header.total_size as u64 != file_size {
            eprintln!("Warning: BIG file size mismatch. Header: {}, Actual: {}", header.total_size, file_size);
        }

        // Read directory entries
        let entries = Self::read_directory(&mut file, &header)?;
        
        // Build lookup maps
        let mut name_to_entry = HashMap::new();
        let mut hash_to_entry = HashMap::new();
        
        for entry in entries.values() {
            name_to_entry.insert(entry.name.to_uppercase(), entry.clone());
            hash_to_entry.insert(entry.name_hash, entry.clone());
        }

        Ok(Self {
            file_path,
            file_size,
            header,
            entries,
            name_to_entry,
            hash_to_entry,
        })
    }

    /// Read directory entries from BIG file
    fn read_directory(file: &mut File, header: &BigHeader) -> Result<HashMap<String, BigEntry>, String> {
        let mut entries = HashMap::new();
        
        // Each directory entry is 8 bytes (offset + size) followed by null-terminated filename
        for i in 0..header.num_entries {
            // Read offset and size
            let mut entry_data = [0u8; 8];
            file.read_exact(&mut entry_data)
                .map_err(|e| format!("Failed to read directory entry {}: {}", i, e))?;

            let offset = u32::from_le_bytes([entry_data[0], entry_data[1], entry_data[2], entry_data[3]]);
            let size = u32::from_le_bytes([entry_data[4], entry_data[5], entry_data[6], entry_data[7]]);

            // Read null-terminated filename
            let mut name_bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                file.read_exact(&mut byte)
                    .map_err(|e| format!("Failed to read filename byte: {}", e))?;
                
                if byte[0] == 0 {
                    break;
                }
                name_bytes.push(byte[0]);
            }

            let name = String::from_utf8(name_bytes)
                .map_err(|e| format!("Invalid filename in BIG file: {}", e))?;

            let name_hash = BigEntry::calculate_hash(&name);

            let entry = BigEntry {
                offset,
                size,
                name: name.clone(),
                name_hash,
            };

            entries.insert(name.to_uppercase(), entry);
        }

        Ok(entries)
    }

    /// Get list of all files in the BIG archive
    pub fn list_files(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Get list of audio files (by extension)
    pub fn list_audio_files(&self) -> Vec<String> {
        self.entries.keys()
            .filter(|name| {
                let name_lower = name.to_lowercase();
                name_lower.ends_with(".wav") || 
                name_lower.ends_with(".mp3") || 
                name_lower.ends_with(".ogg") ||
                name_lower.ends_with(".wma") ||
                name_lower.ends_with(".aif") ||
                name_lower.ends_with(".au")
            })
            .cloned()
            .collect()
    }

    /// Check if a file exists in the archive
    pub fn contains_file(&self, filename: &str) -> bool {
        self.name_to_entry.contains_key(&filename.to_uppercase())
    }

    /// Get file entry information
    pub fn get_entry(&self, filename: &str) -> Option<&BigEntry> {
        self.name_to_entry.get(&filename.to_uppercase())
    }

    /// Extract a file from the BIG archive
    pub fn extract_file(&self, filename: &str) -> Result<Vec<u8>, String> {
        let entry = self.get_entry(filename)
            .ok_or_else(|| format!("File '{}' not found in BIG archive", filename))?;

        let mut file = File::open(&self.file_path)
            .map_err(|e| format!("Failed to reopen BIG file: {}", e))?;

        file.seek(SeekFrom::Start(entry.offset as u64))
            .map_err(|e| format!("Failed to seek to file data: {}", e))?;

        let mut data = vec![0u8; entry.size as usize];
        file.read_exact(&mut data)
            .map_err(|e| format!("Failed to read file data: {}", e))?;

        Ok(data)
    }

    /// Extract a file and return as cursor for audio decoding
    pub fn extract_file_as_cursor(&self, filename: &str) -> Result<Cursor<Vec<u8>>, String> {
        let data = self.extract_file(filename)?;
        Ok(Cursor::new(data))
    }

    /// Get file size
    pub fn get_file_size(&self, filename: &str) -> Option<u32> {
        self.get_entry(filename).map(|entry| entry.size)
    }

    /// Get total number of files in archive
    pub fn get_file_count(&self) -> usize {
        self.entries.len()
    }

    /// Get total archive size
    pub fn get_archive_size(&self) -> u64 {
        self.file_size
    }

    /// Search for files matching a pattern
    pub fn search_files(&self, pattern: &str) -> Vec<String> {
        let pattern_upper = pattern.to_uppercase();
        self.entries.keys()
            .filter(|name| {
                if pattern_upper.contains('*') {
                    // Simple wildcard matching
                    let pattern_parts: Vec<&str> = pattern_upper.split('*').collect();
                    if pattern_parts.len() == 2 {
                        name.starts_with(pattern_parts[0]) && name.ends_with(pattern_parts[1])
                    } else {
                        name.contains(&pattern_upper.replace('*', ""))
                    }
                } else {
                    name.contains(&pattern_upper)
                }
            })
            .cloned()
            .collect()
    }
}

/// BIG File Audio Manager for handling multiple BIG archives
pub struct BigFileAudioManager {
    big_files: RwLock<HashMap<String, Arc<BigFileReader>>>,
    search_order: RwLock<Vec<String>>, // Order in which to search BIG files
    file_cache: RwLock<HashMap<String, (String, Vec<u8>)>>, // filename -> (big_file_name, data)
    cache_size: RwLock<usize>,
    max_cache_size: usize,
}

impl BigFileAudioManager {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            big_files: RwLock::new(HashMap::new()),
            search_order: RwLock::new(Vec::new()),
            file_cache: RwLock::new(HashMap::new()),
            cache_size: RwLock::new(0),
            max_cache_size,
        }
    }

    /// Load a BIG file and add it to the manager
    pub fn load_big_file<P: AsRef<Path>>(&self, name: String, file_path: P) -> Result<(), String> {
        let reader = Arc::new(BigFileReader::open(file_path)?);
        
        {
            let mut big_files = self.big_files.write().unwrap();
            big_files.insert(name.clone(), reader);
        }

        {
            let mut search_order = self.search_order.write().unwrap();
            if !search_order.contains(&name) {
                search_order.push(name);
            }
        }

        Ok(())
    }

    /// Load common C&C audio BIG files
    pub fn load_default_audio_big_files<P: AsRef<Path>>(&self, game_directory: P) -> Result<(), String> {
        let game_dir = game_directory.as_ref();
        
        // Try to load AudioEnglishZH.big (English language sounds)
        let english_big = game_dir.join("AudioEnglishZH.big");
        if english_big.exists() {
            match self.load_big_file("AudioEnglishZH".to_string(), english_big) {
                Ok(()) => println!("Loaded AudioEnglishZH.big successfully"),
                Err(e) => eprintln!("Failed to load AudioEnglishZH.big: {}", e),
            }
        }

        // Try to load AudioZH.big (general audio files)
        let audio_big = game_dir.join("AudioZH.big");
        if audio_big.exists() {
            match self.load_big_file("AudioZH".to_string(), audio_big) {
                Ok(()) => println!("Loaded AudioZH.big successfully"),
                Err(e) => eprintln!("Failed to load AudioZH.big: {}", e),
            }
        }

        // Also try original Generals audio files if they exist
        let generals_audio = game_dir.join("Audio.big");
        if generals_audio.exists() {
            match self.load_big_file("Audio".to_string(), generals_audio) {
                Ok(()) => println!("Loaded Audio.big successfully"),
                Err(e) => eprintln!("Failed to load Audio.big: {}", e),
            }
        }

        let generals_english_audio = game_dir.join("AudioEnglish.big");
        if generals_english_audio.exists() {
            match self.load_big_file("AudioEnglish".to_string(), generals_english_audio) {
                Ok(()) => println!("Loaded AudioEnglish.big successfully"),
                Err(e) => eprintln!("Failed to load AudioEnglish.big: {}", e),
            }
        }

        Ok(())
    }

    /// Search for an audio file across all loaded BIG files
    pub fn find_audio_file(&self, filename: &str) -> Option<String> {
        let big_files = self.big_files.read().unwrap();
        let search_order = self.search_order.read().unwrap();

        for big_name in search_order.iter() {
            if let Some(big_file) = big_files.get(big_name) {
                if big_file.contains_file(filename) {
                    return Some(big_name.clone());
                }
            }
        }

        None
    }

    /// Load an audio file from BIG archives
    pub fn load_audio_file(&self, filename: &str) -> Result<Vec<u8>, String> {
        // Check cache first
        {
            let file_cache = self.file_cache.read().unwrap();
            if let Some((_, data)) = file_cache.get(filename) {
                return Ok(data.clone());
            }
        }

        // Find which BIG file contains this audio
        let big_file_name = self.find_audio_file(filename)
            .ok_or_else(|| format!("Audio file '{}' not found in any BIG archive", filename))?;

        // Load from the appropriate BIG file
        let data = {
            let big_files = self.big_files.read().unwrap();
            let big_file = big_files.get(&big_file_name)
                .ok_or_else(|| format!("BIG file '{}' not loaded", big_file_name))?;
            
            big_file.extract_file(filename)?
        };

        // Cache the loaded data (if there's room)
        self.cache_file(filename.to_string(), big_file_name, data.clone());

        Ok(data)
    }

    /// Load audio file as cursor for streaming
    pub fn load_audio_file_as_cursor(&self, filename: &str) -> Result<Cursor<Vec<u8>>, String> {
        let data = self.load_audio_file(filename)?;
        Ok(Cursor::new(data))
    }

    /// Get list of all audio files across all BIG archives
    pub fn list_all_audio_files(&self) -> Vec<String> {
        let big_files = self.big_files.read().unwrap();
        let mut all_files = Vec::new();

        for big_file in big_files.values() {
            let audio_files = big_file.list_audio_files();
            for file in audio_files {
                if !all_files.contains(&file) {
                    all_files.push(file);
                }
            }
        }

        all_files.sort();
        all_files
    }

    /// Search for audio files matching a pattern
    pub fn search_audio_files(&self, pattern: &str) -> Vec<String> {
        let big_files = self.big_files.read().unwrap();
        let mut matching_files = Vec::new();

        for big_file in big_files.values() {
            let matches = big_file.search_files(pattern);
            for file in matches {
                let file_lower = file.to_lowercase();
                if (file_lower.ends_with(".wav") || 
                    file_lower.ends_with(".mp3") || 
                    file_lower.ends_with(".ogg") ||
                    file_lower.ends_with(".wma")) && 
                   !matching_files.contains(&file) {
                    matching_files.push(file);
                }
            }
        }

        matching_files.sort();
        matching_files
    }

    /// Get file size without loading the entire file
    pub fn get_file_size(&self, filename: &str) -> Option<u32> {
        if let Some(big_file_name) = self.find_audio_file(filename) {
            let big_files = self.big_files.read().unwrap();
            if let Some(big_file) = big_files.get(&big_file_name) {
                return big_file.get_file_size(filename);
            }
        }
        None
    }

    /// Clear the file cache
    pub fn clear_cache(&self) {
        let mut file_cache = self.file_cache.write().unwrap();
        file_cache.clear();
        *self.cache_size.write().unwrap() = 0;
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize, usize) {
        let file_cache = self.file_cache.read().unwrap();
        let cache_size = *self.cache_size.read().unwrap();
        (cache_size, self.max_cache_size, file_cache.len())
    }

    /// Get information about loaded BIG files
    pub fn get_big_file_info(&self) -> Vec<(String, usize, u64)> {
        let big_files = self.big_files.read().unwrap();
        big_files.iter()
            .map(|(name, reader)| {
                (name.clone(), reader.get_file_count(), reader.get_archive_size())
            })
            .collect()
    }

    /// Cache a loaded file
    fn cache_file(&self, filename: String, big_file_name: String, data: Vec<u8>) {
        let data_size = data.len();
        
        // Don't cache files that are too large
        if data_size > self.max_cache_size / 4 {
            return;
        }

        let mut file_cache = self.file_cache.write().unwrap();
        let mut cache_size = self.cache_size.write().unwrap();

        // Make room if necessary
        while *cache_size + data_size > self.max_cache_size && !file_cache.is_empty() {
            // Remove oldest entry (simple FIFO for now)
            if let Some((old_filename, (_, old_data))) = file_cache.iter().next() {
                let old_size = old_data.len();
                let old_filename = old_filename.clone();
                drop(old_data); // Make sure we don't hold references
                file_cache.remove(&old_filename);
                *cache_size -= old_size;
            } else {
                break;
            }
        }

        // Add new file to cache
        if *cache_size + data_size <= self.max_cache_size {
            file_cache.insert(filename, (big_file_name, data));
            *cache_size += data_size;
        }
    }
}

/// Integration with AudioEventRts for BIG file loading
impl BigFileAudioManager {
    /// Load audio data for an AudioEventRts from BIG files
    pub fn load_for_audio_event(&self, event: &AudioEventRts) -> Result<Vec<u8>, String> {
        if let Some(event_info) = event.get_audio_event_info() {
            // Try each sound file in the event info
            for sound_file in &event_info.sounds {
                if let Ok(data) = self.load_audio_file(sound_file) {
                    return Ok(data);
                }
            }
            
            // Try the main filename if no sounds worked
            if !event_info.filename.is_empty() {
                return self.load_audio_file(&event_info.filename);
            }
        }
        
        // Fallback: try the event name itself
        self.load_audio_file(event.get_event_name())
    }

    /// Find best matching audio file for an event
    pub fn find_audio_for_event(&self, event: &AudioEventRts) -> Option<String> {
        if let Some(event_info) = event.get_audio_event_info() {
            // Try each sound file in the event info
            for sound_file in &event_info.sounds {
                if self.find_audio_file(sound_file).is_some() {
                    return Some(sound_file.clone());
                }
            }
            
            // Try the main filename
            if !event_info.filename.is_empty() {
                if self.find_audio_file(&event_info.filename).is_some() {
                    return Some(event_info.filename.clone());
                }
            }
        }
        
        // Fallback: try the event name itself
        let event_name = event.get_event_name();
        if self.find_audio_file(event_name).is_some() {
            Some(event_name.to_string())
        } else {
            None
        }
    }
}

/// Create a BIG file audio manager with default C&C settings
pub fn create_big_file_audio_manager<P: AsRef<Path>>(game_directory: P) -> Result<Arc<BigFileAudioManager>, String> {
    let manager = Arc::new(BigFileAudioManager::new(64 * 1024 * 1024)); // 64 MB cache
    
    manager.load_default_audio_big_files(game_directory)?;
    
    Ok(manager)
}

/// Utility functions for BIG file handling
pub mod utils {
    use super::*;

    /// Extract all audio files from a BIG archive to a directory
    pub fn extract_all_audio<P: AsRef<Path>>(big_file_path: P, output_dir: P) -> Result<usize, String> {
        let reader = BigFileReader::open(big_file_path)?;
        let audio_files = reader.list_audio_files();
        let output_path = output_dir.as_ref();

        std::fs::create_dir_all(output_path)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;

        let mut extracted_count = 0;

        for filename in &audio_files {
            let data = reader.extract_file(filename)?;
            let output_file_path = output_path.join(filename);

            if let Some(parent) = output_file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory for {}: {}", filename, e))?;
            }

            std::fs::write(&output_file_path, data)
                .map_err(|e| format!("Failed to write {}: {}", filename, e))?;

            extracted_count += 1;
        }

        Ok(extracted_count)
    }

    /// Validate BIG file integrity
    pub fn validate_big_file<P: AsRef<Path>>(big_file_path: P) -> Result<bool, String> {
        let reader = BigFileReader::open(big_file_path)?;
        let filenames = reader.list_files();

        for filename in &filenames {
            // Try to extract each file to verify it's readable
            match reader.extract_file(filename) {
                Ok(_) => continue,
                Err(e) => {
                    eprintln!("Failed to extract {}: {}", filename, e);
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Get detailed information about a BIG file
    pub fn analyze_big_file<P: AsRef<Path>>(big_file_path: P) -> Result<BigFileInfo, String> {
        let reader = BigFileReader::open(big_file_path.as_ref())?;
        let all_files = reader.list_files();
        let audio_files = reader.list_audio_files();

        let mut total_uncompressed_size = 0u64;
        let mut audio_total_size = 0u64;
        let mut file_types = HashMap::new();

        for filename in &all_files {
            if let Some(entry) = reader.get_entry(filename) {
                total_uncompressed_size += entry.size as u64;

                // Count file types
                let extension = Path::new(filename)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("unknown")
                    .to_lowercase();
                
                *file_types.entry(extension).or_insert(0) += 1;

                if audio_files.contains(filename) {
                    audio_total_size += entry.size as u64;
                }
            }
        }

        Ok(BigFileInfo {
            file_path: big_file_path.as_ref().to_path_buf(),
            archive_size: reader.get_archive_size(),
            total_files: all_files.len(),
            audio_files: audio_files.len(),
            total_uncompressed_size,
            audio_total_size,
            file_types,
        })
    }

    #[derive(Debug)]
    pub struct BigFileInfo {
        pub file_path: PathBuf,
        pub archive_size: u64,
        pub total_files: usize,
        pub audio_files: usize,
        pub total_uncompressed_size: u64,
        pub audio_total_size: u64,
        pub file_types: HashMap<String, usize>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_simple_big_file(temp_dir: &TempDir) -> PathBuf {
        let big_file_path = temp_dir.path().join("test.big");
        let mut file = File::create(&big_file_path).unwrap();
        
        // Write a minimal BIG file header (this is simplified for testing)
        file.write_all(BIG_MAGIC).unwrap(); // Magic
        file.write_all(&100u32.to_le_bytes()).unwrap(); // Total size (placeholder)
        file.write_all(&1u32.to_le_bytes()).unwrap(); // Num entries
        file.write_all(&32u32.to_le_bytes()).unwrap(); // First file offset
        
        // Write directory entry
        file.write_all(&32u32.to_le_bytes()).unwrap(); // Offset
        file.write_all(&11u32.to_le_bytes()).unwrap(); // Size
        file.write_all(b"test.wav\0").unwrap(); // Filename with null terminator
        
        // Write file data
        file.write_all(b"RIFF\x07\x00\x00\x00WAVE").unwrap(); // Minimal WAV header
        
        big_file_path
    }

    #[test]
    fn test_big_header_parsing() {
        let header_data = [
            b'B', b'I', b'G', b'F',  // Magic
            0x64, 0x00, 0x00, 0x00,  // Total size (100)
            0x05, 0x00, 0x00, 0x00,  // Num entries (5)
            0x20, 0x00, 0x00, 0x00,  // First file offset (32)
        ];

        let header = BigHeader::from_bytes(&header_data).unwrap();
        assert_eq!(header.magic, *BIG_MAGIC);
        assert_eq!(header.total_size, 100);
        assert_eq!(header.num_entries, 5);
        assert_eq!(header.first_file_offset, 32);
    }

    #[test]
    fn test_big_entry_hash() {
        let hash1 = BigEntry::calculate_hash("test.wav");
        let hash2 = BigEntry::calculate_hash("TEST.WAV");
        assert_eq!(hash1, hash2); // Should be case-insensitive
    }

    #[test]
    fn test_big_file_audio_manager() {
        let manager = BigFileAudioManager::new(1024 * 1024);
        
        let (current_size, max_size, entry_count) = manager.get_cache_stats();
        assert_eq!(current_size, 0);
        assert_eq!(max_size, 1024 * 1024);
        assert_eq!(entry_count, 0);
        
        manager.clear_cache();
        let (current_size_after, _, entry_count_after) = manager.get_cache_stats();
        assert_eq!(current_size_after, 0);
        assert_eq!(entry_count_after, 0);
    }

    #[test]
    fn test_search_patterns() {
        let temp_dir = TempDir::new().unwrap();
        
        // This test would need a real BIG file to work properly
        // For now, just test the manager creation
        let manager = BigFileAudioManager::new(1024);
        let results = manager.search_audio_files("*.wav");
        assert_eq!(results.len(), 0); // No BIG files loaded
    }

    // Note: Full integration tests would require actual BIG files from the game,
    // which we can't include in the repository. These tests would be run against
    // real game installations in a testing environment.
}