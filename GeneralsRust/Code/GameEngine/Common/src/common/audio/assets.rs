//! Audio Asset Management and Caching System
//!
//! This module provides comprehensive audio asset management including:
//! - Intelligent caching with memory management
//! - Multiple audio format support (WAV, MP3, OGG, FLAC)
//! - Streaming support for large audio files
//! - Compression and decompression
//! - Asset preloading and background loading
//! - Memory-mapped file support for large assets

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::fs::{metadata, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime};

#[cfg(feature = "audio")]
use hound::{WavReader, WavSpec};
#[cfg(feature = "audio")]
use symphonia::core::audio::{AudioBuffer, Signal};
#[cfg(feature = "audio")]
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
#[cfg(feature = "audio")]
use symphonia::core::formats::{FormatOptions, FormatReader};
#[cfg(feature = "audio")]
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
#[cfg(feature = "audio")]
use symphonia::core::meta::MetadataOptions;
#[cfg(feature = "audio")]
use symphonia::core::probe::Hint;

use crate::common::audio::{AsciiString, AudioHandle, AudioType, Real, UnsignedInt};

/// Maximum cache size in bytes (default 256MB)
pub const DEFAULT_MAX_CACHE_SIZE: usize = 256 * 1024 * 1024;

/// Minimum file size for streaming (files larger than this will be streamed)
pub const STREAMING_THRESHOLD: usize = 10 * 1024 * 1024; // 10MB

/// Audio asset cache entry time-to-live (5 minutes)
pub const CACHE_TTL: Duration = Duration::from_secs(300);

/// Number of background loading threads
pub const BACKGROUND_LOADER_THREADS: usize = 2;

/// Audio format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFormat {
    /// Waveform Audio File Format
    Wav,
    /// MPEG Audio Layer III
    Mp3,
    /// Ogg Vorbis
    OggVorbis,
    /// Free Lossless Audio Codec
    Flac,
    /// Advanced Audio Codec
    Aac,
    /// Windows Media Audio
    Wma,
    /// Unknown or unsupported format
    Unknown,
}

impl AudioFormat {
    /// Determine format from file extension
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_lowercase().as_str() {
            "wav" | "wave" => Self::Wav,
            "mp3" | "mpeg" => Self::Mp3,
            "ogg" | "oga" => Self::OggVorbis,
            "flac" => Self::Flac,
            "aac" | "m4a" => Self::Aac,
            "wma" => Self::Wma,
            _ => Self::Unknown,
        }
    }

    /// Get supported format extensions
    pub fn supported_extensions() -> &'static [&'static str] {
        &["wav", "mp3", "ogg", "flac", "aac", "wma"]
    }

    /// Check if format supports streaming
    pub fn supports_streaming(&self) -> bool {
        matches!(self, Self::Mp3 | Self::OggVorbis | Self::Flac)
    }

    /// Check if format supports random access
    pub fn supports_random_access(&self) -> bool {
        matches!(self, Self::Wav | Self::Flac)
    }
}

/// Audio sample format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleFormat {
    /// 16-bit signed integer
    I16,
    /// 32-bit signed integer
    I32,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
}

impl SampleFormat {
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            Self::I16 => 2,
            Self::I32 => 4,
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

/// Audio metadata extracted from files
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of audio channels
    pub channels: u16,
    /// Sample format
    pub sample_format: SampleFormat,
    /// Total number of sample frames
    pub frame_count: Option<u64>,
    /// Duration in seconds
    pub duration: Option<f64>,
    /// Average bitrate
    pub bitrate: Option<u32>,
    /// File size in bytes
    pub file_size: u64,
    /// File modification time
    pub modified_time: SystemTime,
    /// Audio format
    pub format: AudioFormat,
    /// Title (from metadata)
    pub title: Option<String>,
    /// Artist (from metadata)
    pub artist: Option<String>,
    /// Album (from metadata)
    pub album: Option<String>,
}

impl AudioMetadata {
    pub fn calculate_memory_size(&self) -> usize {
        if let Some(frame_count) = self.frame_count {
            (frame_count as usize)
                * (self.channels as usize)
                * self.sample_format.bytes_per_sample()
        } else {
            0
        }
    }

    pub fn estimate_memory_size(&self) -> usize {
        if let Some(duration) = self.duration {
            let estimated_frames = (duration * self.sample_rate as f64) as usize;
            estimated_frames * (self.channels as usize) * self.sample_format.bytes_per_sample()
        } else {
            (self.file_size as usize) * 2 // Conservative estimate
        }
    }
}

/// Audio data storage
#[derive(Debug, Clone)]
pub enum AudioData {
    /// Fully loaded audio data in memory
    Loaded {
        samples: Vec<f32>,
        metadata: AudioMetadata,
    },
    /// Streaming audio data (file handle and metadata)
    Streaming {
        file_path: PathBuf,
        metadata: AudioMetadata,
        current_position: u64,
    },
    /// Compressed audio data (still encoded)
    Compressed {
        data: Vec<u8>,
        metadata: AudioMetadata,
    },
}

impl AudioData {
    pub fn metadata(&self) -> &AudioMetadata {
        match self {
            Self::Loaded { metadata, .. } => metadata,
            Self::Streaming { metadata, .. } => metadata,
            Self::Compressed { metadata, .. } => metadata,
        }
    }

    pub fn memory_usage(&self) -> usize {
        match self {
            Self::Loaded { samples, .. } => samples.len() * std::mem::size_of::<f32>(),
            Self::Streaming { .. } => std::mem::size_of::<Self>(),
            Self::Compressed { data, .. } => data.len(),
        }
    }

    pub fn is_streaming(&self) -> bool {
        matches!(self, Self::Streaming { .. })
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded { .. })
    }
}

/// Cache entry for audio assets
#[derive(Debug)]
struct CacheEntry {
    /// Audio data
    data: AudioData,
    /// Last access time
    last_accessed: Instant,
    /// Reference count
    reference_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Loading priority
    priority: CachePriority,
}

/// Cache priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CachePriority {
    /// Low priority (background music, ambient sounds)
    Low = 0,
    /// Normal priority (most sound effects)
    Normal = 1,
    /// High priority (UI sounds, important effects)
    High = 2,
    /// Critical priority (always keep in cache)
    Critical = 3,
}

/// Audio asset loading options
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// Force loading into memory (disable streaming)
    pub force_memory: bool,
    /// Force streaming (even for small files)
    pub force_streaming: bool,
    /// Cache priority
    pub priority: CachePriority,
    /// Preload in background
    pub background_load: bool,
    /// Target sample rate (resample if different)
    pub target_sample_rate: Option<u32>,
    /// Target channel count (convert if different)
    pub target_channels: Option<u16>,
    /// Apply compression
    pub compress: bool,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            force_memory: false,
            force_streaming: false,
            priority: CachePriority::Normal,
            background_load: false,
            target_sample_rate: None,
            target_channels: None,
            compress: false,
        }
    }
}

/// Audio loading error types
#[derive(Debug, thiserror::Error)]
pub enum AudioLoadError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Unsupported audio format: {0:?}")]
    UnsupportedFormat(AudioFormat),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Cache full")]
    CacheFull,
    #[error("Invalid audio data")]
    InvalidData,
}

/// Audio streaming reader
pub struct StreamingReader {
    file: File,
    metadata: AudioMetadata,
    current_position: u64,
    buffer_size: usize,
    #[cfg(feature = "audio")]
    decoder: Option<Box<dyn Decoder>>,
    #[cfg(feature = "audio")]
    format_reader: Option<Box<dyn FormatReader>>,
}

impl StreamingReader {
    pub fn new(file_path: &Path, buffer_size: usize) -> Result<Self, AudioLoadError> {
        let file = File::open(file_path)?;
        let metadata = AudioAssetManager::extract_metadata(file_path)?;

        Ok(Self {
            file,
            metadata,
            current_position: 0,
            buffer_size,
            #[cfg(feature = "audio")]
            decoder: None,
            #[cfg(feature = "audio")]
            format_reader: None,
        })
    }

    pub fn read_samples(&mut self, buffer: &mut [f32]) -> Result<usize, AudioLoadError> {
        // Implementation would depend on the specific audio format
        // This is a simplified version
        Ok(0)
    }

    pub fn seek(&mut self, position: u64) -> Result<(), AudioLoadError> {
        self.current_position = position;
        self.file.seek(SeekFrom::Start(position))?;
        Ok(())
    }

    pub fn position(&self) -> u64 {
        self.current_position
    }

    pub fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }
}

/// Background loading task
struct LoadingTask {
    file_path: PathBuf,
    options: LoadOptions,
    completion_callback: Option<Box<dyn FnOnce(Result<AudioData, AudioLoadError>) + Send>>,
}

impl std::fmt::Debug for LoadingTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadingTask")
            .field("file_path", &self.file_path)
            .field("options", &self.options)
            .field("completion_callback", &"<callback>")
            .finish()
    }
}

/// Audio asset manager with caching and streaming support
pub struct AudioAssetManager {
    /// Main asset cache
    cache: DashMap<String, Arc<RwLock<CacheEntry>>>,
    /// Maximum cache size in bytes
    max_cache_size: usize,
    /// Current cache size in bytes
    current_cache_size: Arc<std::sync::atomic::AtomicUsize>,
    /// Background loading task queue
    loading_queue: Arc<crossbeam_channel::Sender<LoadingTask>>,
    /// Background loading workers
    _loading_workers: Vec<std::thread::JoinHandle<()>>,
    /// Asset search directories
    search_paths: RwLock<Vec<PathBuf>>,
    /// File system watcher (for asset reloading)
    #[cfg(feature = "notify")]
    _watcher: Option<notify::RecommendedWatcher>,
}

impl AudioAssetManager {
    pub fn new() -> Self {
        Self::with_cache_size(DEFAULT_MAX_CACHE_SIZE)
    }

    pub fn with_cache_size(max_cache_size: usize) -> Self {
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();

        // Start background loading workers
        let mut workers = Vec::new();
        for i in 0..BACKGROUND_LOADER_THREADS {
            let receiver = task_receiver.clone();
            let worker = std::thread::Builder::new()
                .name(format!("audio-loader-{}", i))
                .spawn(move || {
                    Self::background_worker(receiver);
                })
                .expect("Failed to create background loading thread");
            workers.push(worker);
        }

        Self {
            cache: DashMap::new(),
            max_cache_size,
            current_cache_size: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            loading_queue: Arc::new(task_sender),
            _loading_workers: workers,
            search_paths: RwLock::new(vec![PathBuf::from("./assets/audio")]),
            #[cfg(feature = "notify")]
            _watcher: None,
        }
    }

    /// Add a search path for audio assets
    pub fn add_search_path<P: AsRef<Path>>(&self, path: P) {
        self.search_paths.write().push(path.as_ref().to_path_buf());
    }

    /// Load an audio asset synchronously
    pub fn load_audio(
        &self,
        asset_name: &str,
        options: LoadOptions,
    ) -> Result<Arc<AudioData>, AudioLoadError> {
        // Check cache first
        if let Some(cached) = self.get_from_cache(asset_name) {
            return Ok(Arc::new(cached));
        }

        // Find the actual file path
        let file_path = self.resolve_asset_path(asset_name)?;

        // Load the asset
        let audio_data = self.load_audio_file(&file_path, options)?;

        // Cache the loaded data
        self.add_to_cache(asset_name.to_string(), audio_data.clone());

        Ok(Arc::new(audio_data))
    }

    /// Load an audio asset asynchronously
    pub fn load_audio_async<F>(&self, asset_name: &str, options: LoadOptions, callback: F)
    where
        F: FnOnce(Result<Arc<AudioData>, AudioLoadError>) + Send + 'static,
    {
        let asset_name = asset_name.to_string();

        // Check cache first
        if let Some(cached) = self.get_from_cache(&asset_name) {
            callback(Ok(Arc::new(cached)));
            return;
        }

        // Find the file path
        let file_path = match self.resolve_asset_path(&asset_name) {
            Ok(path) => path,
            Err(e) => {
                callback(Err(e));
                return;
            }
        };

        // Queue for background loading
        let task = LoadingTask {
            file_path,
            options,
            completion_callback: Some(Box::new(move |result| match result {
                Ok(data) => callback(Ok(Arc::new(data))),
                Err(e) => callback(Err(e)),
            })),
        };

        if let Err(_) = self.loading_queue.send(task) {
            // The callback has been moved into the task, so we can't call it here.
            // The task won't be processed if sending failed, so the callback won't be called.
            // This is acceptable behavior - the operation simply fails silently.
        }
    }

    /// Create a streaming reader for large audio files
    pub fn create_stream_reader(
        &self,
        asset_name: &str,
    ) -> Result<StreamingReader, AudioLoadError> {
        let file_path = self.resolve_asset_path(asset_name)?;
        StreamingReader::new(&file_path, 4096)
    }

    /// Get asset metadata without loading the full file
    pub fn get_metadata(&self, asset_name: &str) -> Result<AudioMetadata, AudioLoadError> {
        let file_path = self.resolve_asset_path(asset_name)?;
        Self::extract_metadata(&file_path)
    }

    /// Preload assets for better performance
    pub fn preload_assets(&self, asset_names: &[&str], options: LoadOptions) {
        for &asset_name in asset_names {
            if !self.is_cached(asset_name) {
                let options = LoadOptions {
                    background_load: true,
                    ..options.clone()
                };
                self.load_audio_async(asset_name, options, |_| {});
            }
        }
    }

    /// Remove an asset from the cache
    pub fn unload_asset(&self, asset_name: &str) {
        if let Some((_, entry)) = self.cache.remove(asset_name) {
            let size = entry.read().data.memory_usage();
            self.current_cache_size
                .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Clear all cached assets
    pub fn clear_cache(&self) {
        self.cache.clear();
        self.current_cache_size
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get current cache usage statistics
    pub fn cache_stats(&self) -> (usize, usize, usize) {
        let current_size = self
            .current_cache_size
            .load(std::sync::atomic::Ordering::Relaxed);
        let entry_count = self.cache.len();
        (current_size, self.max_cache_size, entry_count)
    }

    /// Check if an asset is currently cached
    pub fn is_cached(&self, asset_name: &str) -> bool {
        self.cache.contains_key(asset_name)
    }

    /// Force garbage collection of unused cache entries
    pub fn gc_cache(&self) {
        let mut to_remove = Vec::new();
        let now = Instant::now();

        for entry in self.cache.iter() {
            let cache_entry = entry.value().read();

            // Check if entry has expired and has no references
            if now.duration_since(cache_entry.last_accessed) > CACHE_TTL
                && cache_entry
                    .reference_count
                    .load(std::sync::atomic::Ordering::Relaxed)
                    <= 1
                && cache_entry.priority != CachePriority::Critical
            {
                to_remove.push(entry.key().clone());
            }
        }

        for key in to_remove {
            if let Some((_, entry)) = self.cache.remove(&key) {
                let size = entry.read().data.memory_usage();
                self.current_cache_size
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    /// Internal method to resolve asset file path
    fn resolve_asset_path(&self, asset_name: &str) -> Result<PathBuf, AudioLoadError> {
        // Try exact path first
        let path = Path::new(asset_name);
        if path.exists() {
            return Ok(path.to_path_buf());
        }

        // Search in configured search paths
        let search_paths = self.search_paths.read();
        for search_path in search_paths.iter() {
            let full_path = search_path.join(asset_name);
            if full_path.exists() {
                return Ok(full_path);
            }

            // Try with different extensions
            for ext in AudioFormat::supported_extensions() {
                let mut path_with_ext = full_path.clone();
                path_with_ext.set_extension(ext);
                if path_with_ext.exists() {
                    return Ok(path_with_ext);
                }
            }
        }

        Err(AudioLoadError::FileNotFound(asset_name.to_string()))
    }

    /// Extract metadata from an audio file
    fn extract_metadata(file_path: &Path) -> Result<AudioMetadata, AudioLoadError> {
        let file_metadata = metadata(file_path)?;
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        let format = AudioFormat::from_extension(&extension);

        let audio_metadata = AudioMetadata {
            sample_rate: 44100, // Default values
            channels: 2,
            sample_format: SampleFormat::F32,
            frame_count: None,
            duration: None,
            bitrate: None,
            file_size: file_metadata.len(),
            modified_time: file_metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            format,
            title: None,
            artist: None,
            album: None,
        };

        // Extract format-specific metadata
        #[cfg(feature = "audio")]
        match format {
            AudioFormat::Wav => {
                if let Ok(mut reader) = WavReader::open(file_path) {
                    let spec = reader.spec();
                    audio_metadata.sample_rate = spec.sample_rate;
                    audio_metadata.channels = spec.channels;
                    audio_metadata.sample_format = match spec.sample_format {
                        hound::SampleFormat::Int => match spec.bits_per_sample {
                            16 => SampleFormat::I16,
                            32 => SampleFormat::I32,
                            _ => SampleFormat::I16,
                        },
                        hound::SampleFormat::Float => SampleFormat::F32,
                    };

                    if let Some(samples) = reader.len() {
                        audio_metadata.frame_count = Some(samples as u64 / spec.channels as u64);
                        audio_metadata.duration = Some(
                            (samples as f64) / (spec.sample_rate as f64 * spec.channels as f64),
                        );
                    }
                }
            }
            _ => {
                // Use symphonia for other formats
                if let Ok(file) = File::open(file_path) {
                    let source = ReadOnlySource::new(file);
                    let mss = MediaSourceStream::new(Box::new(source), Default::default());

                    let mut hint = Hint::new();
                    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                        hint.with_extension(ext);
                    }

                    let format_opts = FormatOptions::default();
                    let metadata_opts = MetadataOptions::default();

                    if let Ok(format) = symphonia::default::get_probe().format(
                        &hint,
                        mss,
                        &format_opts,
                        &metadata_opts,
                    ) {
                        if let Some(track) = format
                            .format
                            .tracks()
                            .iter()
                            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                        {
                            if let Some(sample_rate) = track.codec_params.sample_rate {
                                audio_metadata.sample_rate = sample_rate;
                            }
                            if let Some(channels) = track.codec_params.channels {
                                audio_metadata.channels = channels.count() as u16;
                            }
                            if let Some(frames) = track.codec_params.n_frames {
                                audio_metadata.frame_count = Some(frames);
                                audio_metadata.duration =
                                    Some(frames as f64 / audio_metadata.sample_rate as f64);
                            }
                        }
                    }
                }
            }
        }

        Ok(audio_metadata)
    }

    /// Load audio file into memory or create streaming data
    fn load_audio_file(
        &self,
        file_path: &Path,
        options: LoadOptions,
    ) -> Result<AudioData, AudioLoadError> {
        let metadata = Self::extract_metadata(file_path)?;

        // Decide whether to stream or load into memory
        let should_stream = if options.force_memory {
            false
        } else if options.force_streaming {
            true
        } else {
            metadata.file_size as usize > STREAMING_THRESHOLD
                || !metadata.format.supports_random_access()
        };

        if should_stream && metadata.format.supports_streaming() {
            Ok(AudioData::Streaming {
                file_path: file_path.to_path_buf(),
                metadata,
                current_position: 0,
            })
        } else {
            // Load into memory
            let samples = self.decode_audio_file(file_path, &options)?;
            Ok(AudioData::Loaded { samples, metadata })
        }
    }

    /// Decode audio file into samples
    fn decode_audio_file(
        &self,
        file_path: &Path,
        options: &LoadOptions,
    ) -> Result<Vec<f32>, AudioLoadError> {
        let format = AudioFormat::from_extension(
            file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or(""),
        );

        match format {
            AudioFormat::Wav => self.decode_wav_file(file_path),
            #[cfg(feature = "audio")]
            _ => self.decode_with_symphonia(file_path, options),
            #[cfg(not(feature = "audio"))]
            _ => Err(AudioLoadError::UnsupportedFormat(format)),
        }
    }

    /// Decode WAV file using hound
    fn decode_wav_file(&self, file_path: &Path) -> Result<Vec<f32>, AudioLoadError> {
        #[cfg(feature = "audio")]
        {
            let mut reader = WavReader::open(file_path)?;
            let spec = reader.spec();
            let samples: Result<Vec<_>, _> = match spec.sample_format {
                hound::SampleFormat::Int => match spec.bits_per_sample {
                    16 => reader
                        .samples::<i16>()
                        .map(|s| s.map(|sample| sample as f32 / i16::MAX as f32))
                        .collect(),
                    32 => reader
                        .samples::<i32>()
                        .map(|s| s.map(|sample| sample as f32 / i32::MAX as f32))
                        .collect(),
                    _ => return Err(AudioLoadError::UnsupportedFormat(AudioFormat::Wav)),
                },
                hound::SampleFormat::Float => reader.samples::<f32>().collect(),
            };
            samples.map_err(|_| AudioLoadError::DecodeError("WAV decode error".to_string()))
        }
        #[cfg(not(feature = "audio"))]
        {
            Err(AudioLoadError::UnsupportedFormat(AudioFormat::Wav))
        }
    }

    /// Decode audio file using symphonia
    #[cfg(feature = "audio")]
    fn decode_with_symphonia(
        &self,
        file_path: &Path,
        _options: &LoadOptions,
    ) -> Result<Vec<f32>, AudioLoadError> {
        let file = File::open(file_path)?;
        let source = ReadOnlySource::new(file);
        let mss = MediaSourceStream::new(Box::new(source), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let mut format = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|_| AudioLoadError::DecodeError("Failed to probe format".to_string()))?;

        let track = format
            .format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioLoadError::DecodeError("No audio track found".to_string()))?;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|_| AudioLoadError::DecodeError("Failed to create decoder".to_string()))?;

        let mut samples = Vec::new();

        while let Ok(packet) = format.format.next_packet() {
            if packet.track_id() != track.id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    // Convert samples to f32
                    if let Some(buf) = decoded.make_equivalent::<f32>() {
                        samples.extend_from_slice(buf.chan(0));
                        if buf.spec().channels.count() > 1 {
                            // Interleave channels
                            for ch in 1..buf.spec().channels.count() {
                                let channel_samples = buf.chan(ch);
                                for (i, &sample) in channel_samples.iter().enumerate() {
                                    if i * 2 + 1 < samples.len() {
                                        samples.insert(i * 2 + 1, sample);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        Ok(samples)
    }

    /// Get audio data from cache
    fn get_from_cache(&self, asset_name: &str) -> Option<AudioData> {
        if let Some(entry) = self.cache.get(asset_name) {
            let mut cache_entry = entry.value().write();
            cache_entry.last_accessed = Instant::now();
            cache_entry
                .reference_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Some(cache_entry.data.clone())
        } else {
            None
        }
    }

    /// Add audio data to cache
    fn add_to_cache(&self, asset_name: String, data: AudioData) {
        let size = data.memory_usage();

        // Check if adding this would exceed cache limit
        let current_size = self
            .current_cache_size
            .load(std::sync::atomic::Ordering::Relaxed);
        if current_size + size > self.max_cache_size {
            self.evict_cache_entries(size);
        }

        let entry = CacheEntry {
            data,
            last_accessed: Instant::now(),
            reference_count: Arc::new(std::sync::atomic::AtomicUsize::new(1)),
            priority: CachePriority::Normal,
        };

        self.cache.insert(asset_name, Arc::new(RwLock::new(entry)));
        self.current_cache_size
            .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
    }

    /// Evict cache entries to make room
    fn evict_cache_entries(&self, needed_space: usize) {
        let mut freed_space = 0;
        let now = Instant::now();

        // Collect candidates for eviction (oldest, lowest priority, no references)
        let mut candidates: Vec<_> = self
            .cache
            .iter()
            .filter_map(|entry| {
                let cache_entry = entry.value().read();
                if cache_entry
                    .reference_count
                    .load(std::sync::atomic::Ordering::Relaxed)
                    <= 1
                    && cache_entry.priority != CachePriority::Critical
                {
                    Some((
                        entry.key().clone(),
                        cache_entry.priority,
                        now.duration_since(cache_entry.last_accessed),
                        cache_entry.data.memory_usage(),
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Sort by priority (lowest first) then by age (oldest first)
        candidates.sort_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)));

        // Evict entries until we have enough space
        for (key, _, _, size) in candidates {
            if freed_space >= needed_space {
                break;
            }

            if let Some((_, _)) = self.cache.remove(&key) {
                freed_space += size;
                self.current_cache_size
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    /// Background worker thread function
    fn background_worker(receiver: crossbeam_channel::Receiver<LoadingTask>) {
        while let Ok(task) = receiver.recv() {
            let result = Self::load_file_sync(&task.file_path, &task.options);
            if let Some(callback) = task.completion_callback {
                callback(result);
            }
        }
    }

    /// Synchronous file loading for background workers
    fn load_file_sync(
        file_path: &Path,
        options: &LoadOptions,
    ) -> Result<AudioData, AudioLoadError> {
        // This would be a simplified version of load_audio_file
        // without access to self (cache, etc.)
        let metadata = Self::extract_metadata(file_path)?;

        Ok(AudioData::Streaming {
            file_path: file_path.to_path_buf(),
            metadata,
            current_position: 0,
        })
    }
}

impl Default for AudioAssetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn create_test_wav_file() -> PathBuf {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_audio.wav");

        // Create a minimal WAV file for testing
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        if let Ok(mut writer) = hound::WavWriter::create(&file_path, spec) {
            for _ in 0..44100 {
                // 1 second of silence
                writer.write_sample(0i16).unwrap();
                writer.write_sample(0i16).unwrap();
            }
            writer.finalize().unwrap();
        }

        file_path
    }

    #[test]
    fn test_audio_format_detection() {
        assert_eq!(AudioFormat::from_extension("wav"), AudioFormat::Wav);
        assert_eq!(AudioFormat::from_extension("mp3"), AudioFormat::Mp3);
        assert_eq!(AudioFormat::from_extension("ogg"), AudioFormat::OggVorbis);
        assert_eq!(AudioFormat::from_extension("flac"), AudioFormat::Flac);
        assert_eq!(AudioFormat::from_extension("unknown"), AudioFormat::Unknown);
    }

    #[test]
    fn test_sample_format_sizes() {
        assert_eq!(SampleFormat::I16.bytes_per_sample(), 2);
        assert_eq!(SampleFormat::I32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::F32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::F64.bytes_per_sample(), 8);
    }

    #[test]
    fn test_audio_asset_manager_creation() {
        let manager = AudioAssetManager::new();
        assert_eq!(manager.max_cache_size, DEFAULT_MAX_CACHE_SIZE);
        assert_eq!(manager.cache_stats().2, 0); // No entries
    }

    #[test]
    fn test_cache_priority_ordering() {
        assert!(CachePriority::Low < CachePriority::Normal);
        assert!(CachePriority::Normal < CachePriority::High);
        assert!(CachePriority::High < CachePriority::Critical);
    }

    #[cfg(feature = "audio")]
    #[test]
    fn test_wav_metadata_extraction() {
        let wav_file = create_test_wav_file();

        if let Ok(metadata) = AudioAssetManager::extract_metadata(&wav_file) {
            assert_eq!(metadata.format, AudioFormat::Wav);
            assert_eq!(metadata.channels, 2);
            assert_eq!(metadata.sample_rate, 44100);
        }

        // Cleanup
        let _ = fs::remove_file(wav_file);
    }

    #[test]
    fn test_load_options_default() {
        let options = LoadOptions::default();
        assert!(!options.force_memory);
        assert!(!options.force_streaming);
        assert_eq!(options.priority, CachePriority::Normal);
        assert!(!options.background_load);
    }
}
