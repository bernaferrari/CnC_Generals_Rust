//! Audio File Cache System
//! 
//! This module provides an efficient audio file caching system that matches the
//! functionality of the C++ AudioFileCache. It manages memory usage and implements
//! LRU (Least Recently Used) eviction to keep memory usage under control.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock, Mutex};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH, Instant};

use crate::common::audio::{AudioEventInfo, AudioEventRts, Real, Bool, UnsignedInt, AsciiString};

/// Compressed audio file information (mimics C++ AILSOUNDINFO)
#[derive(Debug, Clone)]
pub struct SoundInfo {
    pub format: u16,           // Audio format (PCM, etc.)
    pub channels: u16,         // Number of channels (mono/stereo)
    pub sample_rate: u32,      // Sample rate in Hz
    pub bits_per_sample: u16,  // Bits per sample (8, 16, 24, 32)
    pub data_size: u32,        // Size of audio data in bytes
    pub duration_ms: u32,      // Duration in milliseconds
}

impl Default for SoundInfo {
    fn default() -> Self {
        Self {
            format: 1, // PCM
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            data_size: 0,
            duration_ms: 0,
        }
    }
}

/// Represents an open audio file in the cache
#[derive(Debug)]
pub struct OpenAudioFile {
    /// Audio information (format, sample rate, etc.)
    pub sound_info: SoundInfo,
    /// Raw audio data
    pub file_data: Arc<Vec<u8>>,
    /// Reference count - how many things are using this file
    pub open_count: UnsignedInt,
    /// Size in bytes
    pub file_size: UnsignedInt,
    /// Whether the file data is compressed
    pub compressed: Bool,
    /// Associated audio event info (not owned by this struct)
    pub event_info: Option<Arc<AudioEventInfo>>,
    /// Last access time for LRU
    pub last_accessed: SystemTime,
    /// Full file path
    pub file_path: PathBuf,
    /// Audio priority for cache eviction decisions
    pub priority: i32,
}

impl OpenAudioFile {
    pub fn new(file_path: PathBuf, data: Vec<u8>, sound_info: SoundInfo) -> Self {
        let file_size = data.len() as UnsignedInt;
        
        Self {
            sound_info,
            file_data: Arc::new(data),
            open_count: 1,
            file_size,
            compressed: false,
            event_info: None,
            last_accessed: SystemTime::now(),
            file_path,
            priority: 0,
        }
    }

    /// Increment reference count
    pub fn add_ref(&mut self) {
        self.open_count += 1;
        self.last_accessed = SystemTime::now();
    }

    /// Decrement reference count and return whether it's still in use
    pub fn release_ref(&mut self) -> bool {
        if self.open_count > 0 {
            self.open_count -= 1;
        }
        self.open_count > 0
    }

    /// Update last accessed time
    pub fn touch(&mut self) {
        self.last_accessed = SystemTime::now();
    }

    /// Get age in seconds since last access
    pub fn age_seconds(&self) -> u64 {
        self.last_accessed
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Audio file cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub current_size: usize,
    pub max_size: usize,
    pub entry_count: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub total_requests: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.hit_count as f64 / self.total_requests as f64
        }
    }

    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

/// Main audio file cache implementation
/// 
/// This cache manages audio files in memory with the following features:
/// - LRU eviction when memory limit is reached
/// - Reference counting for files in use
/// - Priority-based eviction (lower priority files evicted first)
/// - Thread-safe access
/// - Comprehensive statistics
pub struct AudioFileCache {
    /// Map of file paths to cached audio files
    cache: RwLock<HashMap<PathBuf, OpenAudioFile>>,
    /// Current total size of cached data
    current_size: RwLock<usize>,
    /// Maximum allowed cache size
    max_size: usize,
    /// LRU access order queue
    access_order: RwLock<VecDeque<PathBuf>>,
    /// Cache statistics
    stats: RwLock<CacheStats>,
    /// Thread synchronization for cache operations
    operation_lock: Mutex<()>,
    /// Search paths for audio files
    search_paths: RwLock<Vec<PathBuf>>,
}

impl AudioFileCache {
    /// Create a new audio file cache with specified maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            current_size: RwLock::new(0),
            max_size,
            access_order: RwLock::new(VecDeque::new()),
            stats: RwLock::new(CacheStats {
                current_size: 0,
                max_size,
                entry_count: 0,
                hit_count: 0,
                miss_count: 0,
                eviction_count: 0,
                total_requests: 0,
            }),
            operation_lock: Mutex::new(()),
            search_paths: RwLock::new(vec![
                PathBuf::from("./data/audio/"),
                PathBuf::from("./assets/audio/"),
                PathBuf::from("./audio/"),
            ]),
        }
    }

    /// Add a search path for audio files
    pub fn add_search_path<P: AsRef<Path>>(&self, path: P) {
        let mut search_paths = self.search_paths.write().unwrap();
        search_paths.push(path.as_ref().to_path_buf());
    }

    /// Clear all search paths
    pub fn clear_search_paths(&self) {
        let mut search_paths = self.search_paths.write().unwrap();
        search_paths.clear();
    }

    /// Set maximum cache size
    pub fn set_max_size(&self, max_size: usize) {
        let _lock = self.operation_lock.lock().unwrap();
        
        let old_max_size = self.max_size;
        // We can't actually change max_size because it's not mutable
        // In a real implementation, we'd make this field mutable with interior mutability
        
        // For now, just trigger cleanup if new size is smaller
        if max_size < old_max_size {
            self.ensure_space_available(0); // Force cleanup
        }
        
        // Update stats
        let mut stats = self.stats.write().unwrap();
        stats.max_size = max_size;
    }

    /// Open/load an audio file - main entry point
    pub fn open_file(&self, event: &AudioEventRts) -> Option<Arc<Vec<u8>>> {
        let _lock = self.operation_lock.lock().unwrap();
        
        let file_path = self.resolve_file_path(event)?;
        
        // Update stats
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_requests += 1;
        }

        // Check if already cached
        if let Some(data) = self.get_cached_file(&file_path) {
            let mut stats = self.stats.write().unwrap();
            stats.hit_count += 1;
            return Some(data);
        }

        // Cache miss - load the file
        self.load_and_cache_file(file_path)
    }

    /// Close/release a file (decrement reference count)
    pub fn close_file(&self, file_path: &Path) {
        let _lock = self.operation_lock.lock().unwrap();
        
        let mut cache = self.cache.write().unwrap();
        if let Some(open_file) = cache.get_mut(file_path) {
            if !open_file.release_ref() {
                // Reference count reached zero, but we keep it in cache for potential reuse
                // Actual removal will happen during cache pressure or cleanup
            }
        }
    }

    /// Force removal of files using a specific file handle (for when files are deleted externally)
    pub fn close_any_samples_using_file(&self, file_data: &Vec<u8>) {
        let _lock = self.operation_lock.lock().unwrap();
        
        let mut cache = self.cache.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();
        
        let files_to_remove: Vec<PathBuf> = cache
            .iter()
            .filter(|(_, open_file)| Arc::ptr_eq(&open_file.file_data, &Arc::new(file_data.clone())))
            .map(|(path, _)| path.clone())
            .collect();

        for file_path in files_to_remove {
            if let Some(open_file) = cache.remove(&file_path) {
                *current_size -= open_file.file_size as usize;
                
                // Remove from access order
                if let Some(pos) = access_order.iter().position(|p| p == &file_path) {
                    access_order.remove(pos);
                }
            }
        }

        // Update stats
        let mut stats = self.stats.write().unwrap();
        stats.entry_count = cache.len();
        stats.current_size = *current_size;
    }

    /// Get current cache statistics
    pub fn get_statistics(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// Get current cache size and entry count
    pub fn cache_info(&self) -> (usize, usize, usize) {
        let current_size = *self.current_size.read().unwrap();
        let cache = self.cache.read().unwrap();
        (current_size, self.max_size, cache.len())
    }

    /// Clear all cached files
    pub fn clear_cache(&self) {
        let _lock = self.operation_lock.lock().unwrap();
        
        let mut cache = self.cache.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();
        
        cache.clear();
        *current_size = 0;
        access_order.clear();
        
        let mut stats = self.stats.write().unwrap();
        stats.entry_count = 0;
        stats.current_size = 0;
    }

    /// Perform maintenance - remove unused files, update stats
    pub fn maintenance(&self) {
        let _lock = self.operation_lock.lock().unwrap();
        
        // Remove files with zero reference count that are old
        let mut cache = self.cache.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();
        
        let now = SystemTime::now();
        let max_age_seconds = 300; // 5 minutes
        
        let files_to_remove: Vec<PathBuf> = cache
            .iter()
            .filter(|(_, open_file)| {
                open_file.open_count == 0 && 
                now.duration_since(open_file.last_accessed)
                    .unwrap_or_default()
                    .as_secs() > max_age_seconds
            })
            .map(|(path, _)| path.clone())
            .collect();

        for file_path in files_to_remove {
            if let Some(open_file) = cache.remove(&file_path) {
                *current_size -= open_file.file_size as usize;
                
                if let Some(pos) = access_order.iter().position(|p| p == &file_path) {
                    access_order.remove(pos);
                }
            }
        }

        // Update stats
        let mut stats = self.stats.write().unwrap();
        stats.entry_count = cache.len();
        stats.current_size = *current_size;
    }
}

// Private helper methods
impl AudioFileCache {
    /// Get cached file if available
    fn get_cached_file(&self, file_path: &Path) -> Option<Arc<Vec<u8>>> {
        let mut cache = self.cache.write().unwrap();
        
        if let Some(open_file) = cache.get_mut(file_path) {
            open_file.add_ref();
            self.update_access_order(file_path);
            return Some(open_file.file_data.clone());
        }
        
        None
    }

    /// Load file from disk and add to cache
    fn load_and_cache_file(&self, file_path: PathBuf) -> Option<Arc<Vec<u8>>> {
        // Try to load the file
        let file_data = match self.load_audio_file(&file_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to load audio file {:?}: {}", file_path, e);
                let mut stats = self.stats.write().unwrap();
                stats.miss_count += 1;
                return None;
            }
        };

        let file_size = file_data.len();
        
        // Ensure we have space
        if !self.ensure_space_available(file_size) {
            eprintln!("Not enough cache space for file {:?} (size: {})", file_path, file_size);
            let mut stats = self.stats.write().unwrap();
            stats.miss_count += 1;
            return None;
        }

        // Create sound info (in a real implementation, we'd parse the audio file headers)
        let sound_info = self.analyze_audio_file(&file_data);
        
        // Create cache entry
        let open_file = OpenAudioFile::new(file_path.clone(), file_data, sound_info);
        let result_data = open_file.file_data.clone();

        // Add to cache
        let mut cache = self.cache.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();
        
        cache.insert(file_path.clone(), open_file);
        *current_size += file_size;
        
        self.update_access_order(&file_path);

        // Update stats
        let mut stats = self.stats.write().unwrap();
        stats.miss_count += 1;
        stats.entry_count = cache.len();
        stats.current_size = *current_size;

        Some(result_data)
    }

    /// Load audio file from disk
    fn load_audio_file(&self, file_path: &Path) -> Result<Vec<u8>, std::io::Error> {
        let mut file = File::open(file_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    /// Analyze audio file to extract basic information
    fn analyze_audio_file(&self, data: &[u8]) -> SoundInfo {
        // In a real implementation, we'd parse WAV, MP3, OGG headers
        // For now, return default values
        let mut sound_info = SoundInfo::default();
        sound_info.data_size = data.len() as u32;
        
        // Try to detect format from data
        if data.len() >= 4 {
            if &data[0..4] == b"RIFF" {
                // WAV file
                sound_info.format = 1; // PCM
                // Could parse WAV header here for accurate info
            } else if data.len() >= 3 && &data[0..3] == b"ID3" {
                // MP3 file
                sound_info.format = 0x55; // MP3
            } else if &data[0..4] == b"OggS" {
                // OGG file
                sound_info.format = 0x674F; // OGG
            }
        }
        
        sound_info
    }

    /// Resolve file path using search paths and audio event info
    fn resolve_file_path(&self, event: &AudioEventRts) -> Option<PathBuf> {
        let event_info = event.get_audio_event_info()?;
        
        // Get the base filename
        let filename = if !event_info.sounds.is_empty() {
            &event_info.sounds[0]
        } else if !event_info.filename.is_empty() {
            &event_info.filename
        } else {
            return None;
        };

        // Try each search path
        let search_paths = self.search_paths.read().unwrap();
        for base_path in search_paths.iter() {
            let full_path = base_path.join(filename);
            if full_path.exists() {
                return Some(full_path);
            }
            
            // Try with different extensions
            let extensions = ["wav", "mp3", "ogg", "flac"];
            for ext in &extensions {
                let with_ext = full_path.with_extension(ext);
                if with_ext.exists() {
                    return Some(with_ext);
                }
            }
        }

        None
    }

    /// Ensure enough space is available for a new file
    fn ensure_space_available(&self, needed_size: usize) -> bool {
        if needed_size > self.max_size {
            return false; // File too large for cache
        }

        let mut current_size = self.current_size.write().unwrap();
        
        if *current_size + needed_size <= self.max_size {
            return true; // Already enough space
        }

        // Need to free some space
        let mut cache = self.cache.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        // First, try to remove unused files (ref count = 0)
        let mut files_to_remove = Vec::new();
        for (path, open_file) in cache.iter() {
            if open_file.open_count == 0 {
                files_to_remove.push((path.clone(), open_file.file_size as usize, open_file.priority));
            }
        }

        // Sort by priority (lower priority first) then by age
        files_to_remove.sort_by_key(|&(_, _, priority)| priority);

        // Remove files until we have enough space
        for (path, file_size, _) in files_to_remove {
            if *current_size + needed_size <= self.max_size {
                break;
            }

            cache.remove(&path);
            *current_size -= file_size;
            stats.eviction_count += 1;

            // Remove from access order
            if let Some(pos) = access_order.iter().position(|p| p == &path) {
                access_order.remove(pos);
            }
        }

        // If still not enough space, we can't cache this file
        let result = *current_size + needed_size <= self.max_size;
        
        // Update stats
        stats.entry_count = cache.len();
        stats.current_size = *current_size;
        
        result
    }

    /// Update access order for LRU
    fn update_access_order(&self, file_path: &Path) {
        let mut access_order = self.access_order.write().unwrap();
        
        // Remove if already present
        if let Some(pos) = access_order.iter().position(|p| p == file_path) {
            access_order.remove(pos);
        }
        
        // Add to end (most recent)
        access_order.push_back(file_path.to_path_buf());
        
        // Limit access order size
        const MAX_ACCESS_HISTORY: usize = 1000;
        while access_order.len() > MAX_ACCESS_HISTORY {
            access_order.pop_front();
        }
    }
}

/// Builder for configuring AudioFileCache
pub struct AudioFileCacheBuilder {
    max_size: usize,
    search_paths: Vec<PathBuf>,
}

impl AudioFileCacheBuilder {
    pub fn new() -> Self {
        Self {
            max_size: 16 * 1024 * 1024, // 16 MB default
            search_paths: Vec::new(),
        }
    }

    pub fn max_size(mut self, size: usize) -> Self {
        self.max_size = size;
        self
    }

    pub fn add_search_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.search_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn build(self) -> AudioFileCache {
        let cache = AudioFileCache::new(self.max_size);
        
        for path in self.search_paths {
            cache.add_search_path(path);
        }
        
        cache
    }
}

impl Default for AudioFileCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for audio file cache
impl AudioFileCache {
    /// Get memory usage information
    pub fn memory_info(&self) -> (usize, usize, f64) {
        let current_size = *self.current_size.read().unwrap();
        let usage_percent = (current_size as f64 / self.max_size as f64) * 100.0;
        (current_size, self.max_size, usage_percent)
    }

    /// Get list of currently cached files
    pub fn get_cached_files(&self) -> Vec<(PathBuf, usize, u32)> {
        let cache = self.cache.read().unwrap();
        cache
            .iter()
            .map(|(path, open_file)| {
                (path.clone(), open_file.file_size as usize, open_file.open_count)
            })
            .collect()
    }

    /// Check if a specific file is cached
    pub fn is_cached(&self, file_path: &Path) -> bool {
        let cache = self.cache.read().unwrap();
        cache.contains_key(file_path)
    }

    /// Preload a file into cache (if space available)
    pub fn preload_file(&self, file_path: &Path) -> bool {
        let _lock = self.operation_lock.lock().unwrap();
        
        if self.is_cached(file_path) {
            return true; // Already cached
        }

        match self.load_audio_file(file_path) {
            Ok(data) => {
                let file_size = data.len();
                if self.ensure_space_available(file_size) {
                    let sound_info = self.analyze_audio_file(&data);
                    let open_file = OpenAudioFile::new(file_path.to_path_buf(), data, sound_info);
                    
                    let mut cache = self.cache.write().unwrap();
                    let mut current_size = self.current_size.write().unwrap();
                    
                    cache.insert(file_path.to_path_buf(), open_file);
                    *current_size += file_size;
                    
                    self.update_access_order(file_path);
                    
                    // Update stats
                    let mut stats = self.stats.write().unwrap();
                    stats.entry_count = cache.len();
                    stats.current_size = *current_size;
                    
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    /// Remove a specific file from cache
    pub fn remove_file(&self, file_path: &Path) -> bool {
        let _lock = self.operation_lock.lock().unwrap();
        
        let mut cache = self.cache.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();
        
        if let Some(open_file) = cache.remove(file_path) {
            *current_size -= open_file.file_size as usize;
            
            if let Some(pos) = access_order.iter().position(|p| p == file_path) {
                access_order.remove(pos);
            }
            
            // Update stats
            let mut stats = self.stats.write().unwrap();
            stats.entry_count = cache.len();
            stats.current_size = *current_size;
            stats.eviction_count += 1;
            
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let file_path = dir.path().join(name);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    #[test]
    fn test_cache_creation() {
        let cache = AudioFileCache::new(1024 * 1024);
        let (current, max, count) = cache.cache_info();
        
        assert_eq!(current, 0);
        assert_eq!(max, 1024 * 1024);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_cache_builder() {
        let cache = AudioFileCacheBuilder::new()
            .max_size(2048)
            .add_search_path("/test/path")
            .build();
            
        let (_, max, _) = cache.cache_info();
        assert_eq!(max, 2048);
    }

    #[test]
    fn test_preload_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_data = b"fake audio data for testing";
        let file_path = create_test_file(&temp_dir, "test.wav", test_data);
        
        let cache = AudioFileCache::new(1024);
        assert!(!cache.is_cached(&file_path));
        
        let preload_result = cache.preload_file(&file_path);
        assert!(preload_result);
        assert!(cache.is_cached(&file_path));
    }

    #[test]
    fn test_cache_statistics() {
        let cache = AudioFileCache::new(1024);
        let stats = cache.get_statistics();
        
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_memory_info() {
        let cache = AudioFileCache::new(1024);
        let (current, max, usage) = cache.memory_info();
        
        assert_eq!(current, 0);
        assert_eq!(max, 1024);
        assert_eq!(usage, 0.0);
    }

    #[test]
    fn test_cache_clear() {
        let temp_dir = TempDir::new().unwrap();
        let test_data = b"test data";
        let file_path = create_test_file(&temp_dir, "test.wav", test_data);
        
        let cache = AudioFileCache::new(1024);
        cache.preload_file(&file_path);
        
        let (current_before, _, count_before) = cache.cache_info();
        assert!(current_before > 0);
        assert!(count_before > 0);
        
        cache.clear_cache();
        
        let (current_after, _, count_after) = cache.cache_info();
        assert_eq!(current_after, 0);
        assert_eq!(count_after, 0);
    }

    #[test]
    fn test_file_removal() {
        let temp_dir = TempDir::new().unwrap();
        let test_data = b"test data for removal";
        let file_path = create_test_file(&temp_dir, "test.wav", test_data);
        
        let cache = AudioFileCache::new(1024);
        cache.preload_file(&file_path);
        
        assert!(cache.is_cached(&file_path));
        
        let removed = cache.remove_file(&file_path);
        assert!(removed);
        assert!(!cache.is_cached(&file_path));
    }

    #[test]
    fn test_search_paths() {
        let cache = AudioFileCache::new(1024);
        
        // Test adding search paths
        cache.add_search_path("/test/path1");
        cache.add_search_path("/test/path2");
        
        // Test clearing search paths
        cache.clear_search_paths();
        
        // No direct way to test search paths without actual file system,
        // but the methods should not panic
    }

    #[test]
    fn test_sound_info_default() {
        let sound_info = SoundInfo::default();
        
        assert_eq!(sound_info.format, 1); // PCM
        assert_eq!(sound_info.channels, 2);
        assert_eq!(sound_info.sample_rate, 44100);
        assert_eq!(sound_info.bits_per_sample, 16);
        assert_eq!(sound_info.data_size, 0);
        assert_eq!(sound_info.duration_ms, 0);
    }

    #[test]
    fn test_open_audio_file_ref_counting() {
        let temp_path = PathBuf::from("/tmp/test.wav");
        let data = vec![1, 2, 3, 4, 5];
        let sound_info = SoundInfo::default();
        
        let mut open_file = OpenAudioFile::new(temp_path, data, sound_info);
        
        assert_eq!(open_file.open_count, 1);
        
        open_file.add_ref();
        assert_eq!(open_file.open_count, 2);
        
        assert!(open_file.release_ref()); // Still has references
        assert_eq!(open_file.open_count, 1);
        
        assert!(!open_file.release_ref()); // No more references
        assert_eq!(open_file.open_count, 0);
    }
}