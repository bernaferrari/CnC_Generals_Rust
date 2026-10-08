//! Audio File Cache System
//!
//! This module provides an efficient audio file caching system that matches the
//! functionality of the C++ AudioFileCache. It manages memory usage and implements
//! LRU (Least Recently Used) eviction to keep memory usage under control.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::common::audio::audio_event_rts::{AudioEventInfo, AudioEventRts};
use crate::common::audio::{AsciiString, Bool, Real, UnsignedInt};

/// Compressed audio file information (mimics C++ AILSOUNDINFO)
#[derive(Debug, Clone)]
pub struct SoundInfo {
    pub format: u16,          // Audio format (PCM, etc.)
    pub channels: u16,        // Number of channels (mono/stereo)
    pub sample_rate: u32,     // Sample rate in Hz
    pub bits_per_sample: u16, // Bits per sample (8, 16, 24, 32)
    pub data_size: u32,       // Size of audio data in bytes
    pub duration_ms: u32,     // Duration in milliseconds
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

/// Mutable fields that participate in one cache transaction.
///
/// C++ AudioFileCache protects open/close/setMaxSize with one mutex. Rust keeps
/// that transaction mutex and groups its related state behind one RwLock so
/// diagnostics can read during filesystem/loader work while internal helpers
/// update cache/accounting/LRU/tombstones together without nested locks.
struct AudioCacheState {
    cache: HashMap<PathBuf, OpenAudioFile>,
    current_size: usize,
    max_size: usize,
    access_order: VecDeque<PathBuf>,
    stats: CacheStats,
    search_paths: Vec<PathBuf>,
    /// Session-lifetime tombstones for keys whose loader returned `None`.
    /// C++ AudioFileCache::openFile checks its filename table before VFS I/O;
    /// the Rust live host requeues failed ambient loops, so remembering misses
    /// avoids repeating path probes on every frame. clear_cache rearms them.
    negative_cache: HashSet<PathBuf>,
}

/// Main audio file cache implementation
///
/// This cache retains the operation-wide transaction mutex from C++ while
/// storing its interrelated Rust bookkeeping behind one read/write lock.
pub struct AudioFileCache {
    /// Serializes cache transactions across file I/O and loader callbacks.
    operation_lock: Mutex<()>,
    /// Cache state is locked only for in-memory phases; never across I/O.
    state: RwLock<AudioCacheState>,
}

impl AudioFileCache {
    /// Create a new audio file cache with specified maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            operation_lock: Mutex::new(()),
            state: RwLock::new(AudioCacheState {
                cache: HashMap::new(),
                current_size: 0,
                max_size,
                access_order: VecDeque::new(),
                stats: CacheStats {
                    current_size: 0,
                    max_size,
                    entry_count: 0,
                    hit_count: 0,
                    miss_count: 0,
                    eviction_count: 0,
                    total_requests: 0,
                },
                search_paths: vec![
                    PathBuf::from("./data/audio/"),
                    PathBuf::from("./assets/audio/"),
                    PathBuf::from("./audio/"),
                ],
                negative_cache: HashSet::new(),
            }),
        }
    }

    /// Add a search path for audio files
    pub fn add_search_path<P: AsRef<Path>>(&self, path: P) {
        self.state
            .write()
            .unwrap()
            .search_paths
            .push(path.as_ref().to_path_buf());
    }

    /// Clear all search paths
    pub fn clear_search_paths(&self) {
        self.state.write().unwrap().search_paths.clear();
    }

    /// Set maximum cache size
    pub fn set_max_size(&self, max_size: usize) {
        let _lock = self.operation_lock.lock().unwrap();
        let mut state = self.state.write().unwrap();
        // C++ setMaxSize only updates the limit; later misses handle pressure.
        state.max_size = max_size;
        state.stats.max_size = max_size;
    }

    /// Open/load an audio file - main entry point
    pub fn open_file(&self, event: &AudioEventRts) -> Option<Arc<Vec<u8>>> {
        let _lock = self.operation_lock.lock().unwrap();
        let search_paths = self.state.read().unwrap().search_paths.clone();
        // Filesystem existence checks happen after the state read guard drops.
        let file_path = self.resolve_file_path(&search_paths, event)?;

        {
            let mut state = self.state.write().unwrap();
            state.stats.total_requests += 1;
            if let Some(data) = Self::get_cached_file(&mut state, &file_path) {
                state.stats.hit_count += 1;
                return Some(data);
            }
        }

        // Disk I/O occurs without a state guard. operation_lock still prevents
        // another cache transaction from racing this miss.
        let file_data = match self.load_audio_file(&file_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to load audio file {:?}: {}", file_path, e);
                self.state.write().unwrap().stats.miss_count += 1;
                return None;
            }
        };
        let mut state = self.state.write().unwrap();
        self.cache_loaded_file(&mut state, file_path, file_data)
    }

    /// C++ `AudioFileCache::openFile` hit path: refcount an already-loaded buffer
    /// keyed by filename. Used by the live `RodioPlaybackHook` so VFS/fs reads
    /// are not repeated per play.
    pub fn get_or_insert_named(
        &self,
        key: &str,
        loader: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<Vec<u8>>> {
        let path = PathBuf::from(key);
        let _lock = self.operation_lock.lock().unwrap();

        {
            let mut state = self.state.write().unwrap();
            state.stats.total_requests += 1;
            if let Some(data) = Self::get_cached_file(&mut state, &path) {
                state.stats.hit_count += 1;
                return Some(data);
            }
            if state.negative_cache.contains(&path) {
                state.stats.miss_count += 1;
                return None;
            }
        }

        // Do not hold the state lock across the caller-supplied loader. The
        // transaction mutex remains held, matching the existing serialization.
        let Some(data) = loader() else {
            let mut state = self.state.write().unwrap();
            state.negative_cache.insert(path);
            state.stats.miss_count += 1;
            return None;
        };
        let file_size = data.len();
        let mut state = self.state.write().unwrap();
        if !Self::ensure_space_available(&mut state, file_size) {
            state.stats.miss_count += 1;
            return None;
        }
        let sound_info = self.analyze_audio_file(&data);
        let open_file = OpenAudioFile::new(path.clone(), data, sound_info);
        let result_data = open_file.file_data.clone();
        state.cache.insert(path.clone(), open_file);
        state.current_size += file_size;
        Self::update_access_order(&mut state, &path);
        state.stats.miss_count += 1;
        state.stats.entry_count = state.cache.len();
        state.stats.current_size = state.current_size;
        Some(result_data)
    }

    /// C++ `AudioFileCache::closeFile` by filename key.
    pub fn close_named(&self, key: &str) {
        self.close_file(Path::new(key));
    }

    /// Close/release a file (decrement reference count)
    pub fn close_file(&self, file_path: &Path) {
        let _lock = self.operation_lock.lock().unwrap();
        let mut state = self.state.write().unwrap();
        if let Some(open_file) = state.cache.get_mut(file_path) {
            if !open_file.release_ref() {
                // Keep the zero-reference entry for reuse until pressure/maintenance.
            }
        }
    }

    /// Force removal of files using a specific file handle (for when files are deleted externally)
    pub fn close_any_samples_using_file(&self, file_data: &Vec<u8>) {
        let _lock = self.operation_lock.lock().unwrap();
        let mut state = self.state.write().unwrap();

        // Preserve the current Rust pointer-comparison behavior exactly; this
        // consolidation does not reinterpret the file handle semantics.
        let files_to_remove: Vec<PathBuf> = state
            .cache
            .iter()
            .filter(|(_, open_file)| {
                Arc::ptr_eq(&open_file.file_data, &Arc::new(file_data.clone()))
            })
            .map(|(path, _)| path.clone())
            .collect();

        for file_path in files_to_remove {
            if let Some(open_file) = state.cache.remove(&file_path) {
                state.current_size -= open_file.file_size as usize;
                if let Some(pos) = state.access_order.iter().position(|p| p == &file_path) {
                    state.access_order.remove(pos);
                }
            }
        }
        state.stats.entry_count = state.cache.len();
        state.stats.current_size = state.current_size;
    }

    /// Get current cache statistics
    pub fn get_statistics(&self) -> CacheStats {
        self.state.read().unwrap().stats.clone()
    }

    /// Get current cache size and entry count
    pub fn cache_info(&self) -> (usize, usize, usize) {
        let state = self.state.read().unwrap();
        (state.current_size, state.max_size, state.cache.len())
    }

    /// Clear all cached files
    pub fn clear_cache(&self) {
        let _lock = self.operation_lock.lock().unwrap();
        let mut state = self.state.write().unwrap();
        state.cache.clear();
        state.current_size = 0;
        state.access_order.clear();
        // Miss tombstones are session state keyed to cache contents.
        state.negative_cache.clear();
        state.stats.entry_count = 0;
        state.stats.current_size = 0;
    }

    /// Perform maintenance - remove unused files, update stats
    pub fn maintenance(&self) {
        let _lock = self.operation_lock.lock().unwrap();
        let mut state = self.state.write().unwrap();
        let now = SystemTime::now();
        let max_age_seconds = 300; // 5 minutes

        let files_to_remove: Vec<PathBuf> = state
            .cache
            .iter()
            .filter(|(_, open_file)| {
                open_file.open_count == 0
                    && now
                        .duration_since(open_file.last_accessed)
                        .unwrap_or_default()
                        .as_secs()
                        > max_age_seconds
            })
            .map(|(path, _)| path.clone())
            .collect();

        for file_path in files_to_remove {
            if let Some(open_file) = state.cache.remove(&file_path) {
                state.current_size -= open_file.file_size as usize;
                if let Some(pos) = state.access_order.iter().position(|p| p == &file_path) {
                    state.access_order.remove(pos);
                }
            }
        }
        state.stats.entry_count = state.cache.len();
        state.stats.current_size = state.current_size;
    }

    /// Get memory usage information
    pub fn memory_info(&self) -> (usize, usize, f64) {
        let state = self.state.read().unwrap();
        let usage_percent = (state.current_size as f64 / state.max_size as f64) * 100.0;
        (state.current_size, state.max_size, usage_percent)
    }

    /// Get list of currently cached files
    pub fn get_cached_files(&self) -> Vec<(PathBuf, usize, u32)> {
        self.state
            .read()
            .unwrap()
            .cache
            .iter()
            .map(|(path, open_file)| {
                (
                    path.clone(),
                    open_file.file_size as usize,
                    open_file.open_count,
                )
            })
            .collect()
    }

    /// Check if a specific file is cached
    pub fn is_cached(&self, file_path: &Path) -> bool {
        self.state.read().unwrap().cache.contains_key(file_path)
    }

    /// Preload a file into cache (if space available)
    pub fn preload_file(&self, file_path: &Path) -> bool {
        let _lock = self.operation_lock.lock().unwrap();
        if self.state.read().unwrap().cache.contains_key(file_path) {
            return true;
        }

        // File I/O happens without a state guard, but inside the transaction.
        match self.load_audio_file(file_path) {
            Ok(data) => {
                let file_size = data.len();
                let mut state = self.state.write().unwrap();
                if !Self::ensure_space_available(&mut state, file_size) {
                    return false;
                }
                let sound_info = self.analyze_audio_file(&data);
                let open_file = OpenAudioFile::new(file_path.to_path_buf(), data, sound_info);
                state.cache.insert(file_path.to_path_buf(), open_file);
                state.current_size += file_size;
                Self::update_access_order(&mut state, file_path);
                state.stats.entry_count = state.cache.len();
                state.stats.current_size = state.current_size;
                true
            }
            Err(_) => false,
        }
    }

    /// Remove a specific file from cache
    pub fn remove_file(&self, file_path: &Path) -> bool {
        let _lock = self.operation_lock.lock().unwrap();
        let mut state = self.state.write().unwrap();
        if let Some(open_file) = state.cache.remove(file_path) {
            state.current_size -= open_file.file_size as usize;
            if let Some(pos) = state.access_order.iter().position(|p| p == file_path) {
                state.access_order.remove(pos);
            }
            state.stats.entry_count = state.cache.len();
            state.stats.current_size = state.current_size;
            state.stats.eviction_count += 1;
            true
        } else {
            false
        }
    }
}

// Private helper methods over the locked state. They never acquire another lock.
impl AudioFileCache {
    fn get_cached_file(state: &mut AudioCacheState, file_path: &Path) -> Option<Arc<Vec<u8>>> {
        let data = if let Some(open_file) = state.cache.get_mut(file_path) {
            open_file.add_ref();
            Some(open_file.file_data.clone())
        } else {
            None
        }?;
        Self::update_access_order(state, file_path);
        Some(data)
    }

    fn cache_loaded_file(
        &self,
        state: &mut AudioCacheState,
        file_path: PathBuf,
        file_data: Vec<u8>,
    ) -> Option<Arc<Vec<u8>>> {
        let file_size = file_data.len();
        if !Self::ensure_space_available(state, file_size) {
            eprintln!(
                "Not enough cache space for file {:?} (size: {})",
                file_path, file_size
            );
            state.stats.miss_count += 1;
            return None;
        }
        let sound_info = self.analyze_audio_file(&file_data);
        let open_file = OpenAudioFile::new(file_path.clone(), file_data, sound_info);
        let result_data = open_file.file_data.clone();
        state.cache.insert(file_path.clone(), open_file);
        state.current_size += file_size;
        Self::update_access_order(state, &file_path);
        state.stats.miss_count += 1;
        state.stats.entry_count = state.cache.len();
        state.stats.current_size = state.current_size;
        Some(result_data)
    }

    fn load_audio_file(&self, file_path: &Path) -> Result<Vec<u8>, std::io::Error> {
        let mut file = File::open(file_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    fn analyze_audio_file(&self, data: &[u8]) -> SoundInfo {
        let mut sound_info = SoundInfo::default();
        sound_info.data_size = data.len() as u32;
        if data.len() >= 4 {
            if &data[0..4] == b"RIFF" {
                sound_info.format = 1;
            } else if data.len() >= 3 && &data[0..3] == b"ID3" {
                sound_info.format = 0x55;
            } else if &data[0..4] == b"OggS" {
                sound_info.format = 0x674F;
            }
        }
        sound_info
    }

    fn resolve_file_path(
        &self,
        search_paths: &[PathBuf],
        event: &AudioEventRts,
    ) -> Option<PathBuf> {
        let event_info = event.get_audio_event_info()?;
        let filename = if !event_info.sounds.is_empty() {
            &event_info.sounds[0]
        } else if !event_info.filename.is_empty() {
            &event_info.filename
        } else {
            return None;
        };

        for base_path in search_paths {
            let full_path = base_path.join(filename);
            if full_path.exists() {
                return Some(full_path);
            }
            for ext in ["wav", "mp3", "ogg", "flac"] {
                let with_ext = full_path.with_extension(ext);
                if with_ext.exists() {
                    return Some(with_ext);
                }
            }
        }
        None
    }

    fn ensure_space_available(state: &mut AudioCacheState, needed_size: usize) -> bool {
        if needed_size > state.max_size {
            return false;
        }
        if state.current_size + needed_size <= state.max_size {
            return true;
        }

        let mut files_to_remove = Vec::new();
        for (path, open_file) in &state.cache {
            if open_file.open_count == 0 {
                files_to_remove.push((
                    path.clone(),
                    open_file.file_size as usize,
                    open_file.priority,
                ));
            }
        }
        files_to_remove.sort_by_key(|&(_, _, priority)| priority);
        for (path, file_size, _) in files_to_remove {
            if state.current_size + needed_size <= state.max_size {
                break;
            }
            state.cache.remove(&path);
            state.current_size -= file_size;
            state.stats.eviction_count += 1;
            if let Some(pos) = state.access_order.iter().position(|p| p == &path) {
                state.access_order.remove(pos);
            }
        }
        let result = state.current_size + needed_size <= state.max_size;
        state.stats.entry_count = state.cache.len();
        state.stats.current_size = state.current_size;
        result
    }

    fn update_access_order(state: &mut AudioCacheState, file_path: &Path) {
        if let Some(pos) = state.access_order.iter().position(|p| p == file_path) {
            state.access_order.remove(pos);
        }
        state.access_order.push_back(file_path.to_path_buf());
        const MAX_ACCESS_HISTORY: usize = 1000;
        while state.access_order.len() > MAX_ACCESS_HISTORY {
            state.access_order.pop_front();
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

    #[test]
    fn get_or_insert_named_refcounts_and_reuses_buffer() {
        // C++ AudioFileCache::openFile (MilesAudioManager.cpp:3123-3128):
        // a second open of the same name increments m_openCount and returns
        // the same buffer pointer.
        let cache = AudioFileCache::new(1024);
        let first = cache
            .get_or_insert_named("boom.wav", || Some(vec![1, 2, 3, 4]))
            .expect("first insert");
        let second = cache
            .get_or_insert_named("boom.wav", || panic!("loader must not run on cache hit"))
            .expect("cache hit");
        assert!(Arc::ptr_eq(&first, &second));
        let cached = cache.get_cached_files();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].2, 2);
        cache.close_named("boom.wav");
        let cached = cache.get_cached_files();
        assert_eq!(cached[0].2, 1);
    }

    #[test]
    fn get_or_insert_named_misses_probe_the_loader_once_per_key() {
        // C++ AudioFileCache::openFile (MilesAudioManager.cpp:3123-3136)
        // consults the filename hash before touching the file system and
        // logs "Missing Audio File" for the miss. The Rust live host
        // re-queues failing loops, so a miss must be tombstoned: exactly one
        // loader probe per key until clear_cache re-arms the session.
        let cache = AudioFileCache::new(1024);
        let missing = "van/missing_loop.wav";
        let mut probes = 0;
        assert!(
            cache
                .get_or_insert_named(missing, || {
                    probes += 1;
                    None
                })
                .is_none()
        );
        for _ in 0..5 {
            assert!(
                cache
                    .get_or_insert_named(missing, || {
                        probes += 1;
                        None
                    })
                    .is_none(),
                "tombstoned miss must stay a miss"
            );
        }
        assert_eq!(
            probes, 1,
            "loader must run exactly once per missing key, not per play"
        );

        // Distinct keys probe independently of each other's tombstones.
        let mut other_probes = 0;
        assert!(
            cache
                .get_or_insert_named("van/other_missing.wav", || {
                    other_probes += 1;
                    None
                })
                .is_none()
        );
        assert_eq!(other_probes, 1);

        // clear_cache drops tombstones: the next open probes again.
        cache.clear_cache();
        let mut probes_after_clear = 0;
        assert!(
            cache
                .get_or_insert_named(missing, || {
                    probes_after_clear += 1;
                    None
                })
                .is_none()
        );
        assert_eq!(probes_after_clear, 1);
    }
}
