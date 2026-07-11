//! W3D Animation File Loader Integration
//!
//! This module provides animation playback infrastructure that works with
//! the rendering pipeline and AnimationFrameCoordinator.
//!
//! Supports:
//! - Animation file loading from W3D format
//! - Skeletal animation playback with frame-accurate evaluation
//! - Animation playback modes (Loop, Once, PingPong)
//! - Bone transform queries at current frame
//! - Animation caching for efficient asset management

use glam::{Mat4, Quat, Vec3};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use ww3d_animation::{
    load_w3d_animation, w3d_animation_to_hanim, HAnimClass, HCompressedAnimClass,
};

/// Result type for animation loader operations
pub type AnimationLoaderResult<T> = Result<T, AnimationLoaderError>;

/// Error types for animation loading
#[derive(Debug, Clone)]
pub enum AnimationLoaderError {
    /// File not found or inaccessible
    FileNotFound(String),
    /// Failed to parse W3D file
    ParseError(String),
    /// Unsupported animation format
    UnsupportedFormat(String),
    /// Invalid animation data
    InvalidAnimationData(String),
    /// Animation system error
    SystemError(String),
}

impl std::fmt::Display for AnimationLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationLoaderError::FileNotFound(msg) => write!(f, "File not found: {}", msg),
            AnimationLoaderError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            AnimationLoaderError::UnsupportedFormat(msg) => {
                write!(f, "Unsupported format: {}", msg)
            }
            AnimationLoaderError::InvalidAnimationData(msg) => {
                write!(f, "Invalid animation data: {}", msg)
            }
            AnimationLoaderError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for AnimationLoaderError {}

/// Loaded animation data ready for playback
///
/// This structure holds animation metadata and is designed to be
/// generic over the underlying animation representation.
#[derive(Clone)]
pub struct LoadedAnimation {
    /// Animation name from file
    pub name: String,
    /// Hierarchy name (skeleton) referenced by the animation
    pub hierarchy_name: String,
    /// Frame count in animation
    pub frame_count: u32,
    /// Animation duration in seconds (frame_count / frame_rate)
    pub duration_seconds: f32,
    /// Frame rate (frames per second)
    pub frame_rate: f32,
    /// Bone count in skeleton
    pub bone_count: u32,
    /// Uncompressed animation data (if present)
    pub hanim: Option<HAnimClass>,
    /// Compressed animation data (if present)
    pub compressed_anim: Option<Arc<Mutex<HCompressedAnimClass>>>,
    /// Custom data that animation systems can store
    /// (e.g., serialized animation data, file handles)
    pub metadata: std::collections::HashMap<String, String>,
}

impl LoadedAnimation {
    /// Create a new loaded animation
    pub fn new(name: String, frame_count: u32, frame_rate: f32, bone_count: u32) -> Self {
        Self {
            name,
            hierarchy_name: String::new(),
            frame_count,
            duration_seconds: frame_count as f32 / frame_rate,
            frame_rate,
            bone_count,
            hanim: None,
            compressed_anim: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Check if animation is valid for playback
    pub fn is_valid(&self) -> bool {
        self.frame_count > 0 && self.frame_rate > 0.0
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Animation loader for W3D files
pub struct W3DAnimationLoader;

impl W3DAnimationLoader {
    /// Load animation from a W3D file
    ///
    /// # Arguments
    /// * `file_path` - Path to the W3D file
    ///
    /// # Returns
    /// Loaded animation data ready for playback
    pub fn load_animation(file_path: &str) -> AnimationLoaderResult<LoadedAnimation> {
        // Verify file exists
        if !std::path::Path::new(file_path).exists() {
            return Err(AnimationLoaderError::FileNotFound(file_path.to_string()));
        }

        // Load the file data
        let file_data = std::fs::read(file_path)
            .map_err(|e| AnimationLoaderError::FileNotFound(format!("{}: {}", file_path, e)))?;

        // Parse as W3D animation
        Self::parse_animation_data(&file_data, file_path)
    }

    /// Parse animation data from binary W3D content
    fn parse_animation_data(
        data: &[u8],
        file_path: &str,
    ) -> AnimationLoaderResult<LoadedAnimation> {
        let mut cursor = Cursor::new(data);
        let anim_data = load_w3d_animation(&mut cursor)
            .map_err(|e| AnimationLoaderError::ParseError(format!("{}: {}", file_path, e)))?;

        let has_channels = !anim_data.channels.is_empty();
        let hanim = if has_channels {
            Some(w3d_animation_to_hanim(anim_data.clone()))
        } else {
            None
        };
        let compression_flavor = anim_data.compression_flavor;
        let name = anim_data.name.clone();
        let hierarchy_name = anim_data.hierarchy_name.clone();
        let num_frames = anim_data.num_frames;
        let frame_rate = anim_data.frame_rate;
        let compressed_anim = anim_data.compressed_anim;

        let mut loaded = LoadedAnimation::new(name, num_frames, frame_rate, 0);
        loaded.hierarchy_name = hierarchy_name;
        loaded.hanim = hanim;

        if let Some(compressed) = compressed_anim {
            loaded.compressed_anim = Some(Arc::new(Mutex::new(compressed)));
        }

        loaded.bone_count = if let Some(hanim) = &loaded.hanim {
            hanim.num_pivots() as u32
        } else if let Some(compressed) = &loaded.compressed_anim {
            compressed.lock().unwrap().get_num_pivots() as u32
        } else {
            0
        };

        loaded.set_metadata("source_file".to_string(), file_path.to_string());
        if let Some(flavor) = compression_flavor {
            loaded.set_metadata("compression_flavor".to_string(), flavor.to_string());
        }

        Ok(loaded)
    }

    /// Load animation from memory buffer
    pub fn load_animation_from_buffer(
        data: &[u8],
        name: &str,
    ) -> AnimationLoaderResult<LoadedAnimation> {
        Self::parse_animation_data(data, name)
    }
}

/// Animation playback controller
///
/// Manages animation playback state, including position, mode, and control.
/// Works with AnimationFrameCoordinator to synchronize animation time.
pub struct AnimationPlayback {
    /// Current animation being played
    pub animation: Arc<LoadedAnimation>,
    /// Current playback position (frame number)
    pub current_frame: f32,
    /// Playback mode
    pub mode: PlaybackMode,
    /// Is playback active
    pub is_playing: bool,
    /// Direction used by ping-pong playback.
    pub playback_direction: f32,
}

/// Animation playback modes matching W3D specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackMode {
    /// Loop: Restart from frame 0 when finished
    #[default]
    Loop,
    /// Once: Play to end and stop
    Once,
    /// PingPong: Play forward, reverse, repeat
    PingPong,
}

impl AnimationPlayback {
    /// Create a new animation playback controller
    pub fn new(animation: LoadedAnimation) -> Self {
        Self {
            animation: Arc::new(animation),
            current_frame: 0.0,
            mode: PlaybackMode::default(),
            is_playing: true,
            playback_direction: 1.0,
        }
    }

    /// Update playback position based on delta time
    pub fn update(&mut self, delta_time: f32) {
        if !self.is_playing {
            return;
        }

        let frame_rate = self.animation.frame_rate;
        let frame_delta = frame_rate * delta_time;
        let max_frame = self.animation.frame_count.saturating_sub(1) as f32;

        if max_frame <= 0.0 {
            self.current_frame = 0.0;
            return;
        }

        let direction = if matches!(self.mode, PlaybackMode::PingPong) {
            self.playback_direction
        } else {
            1.0
        };
        self.current_frame += frame_delta * direction;

        // Handle playback mode
        match self.mode {
            PlaybackMode::Loop => {
                if self.current_frame >= max_frame {
                    self.current_frame -= max_frame;
                }
                if self.current_frame >= max_frame {
                    self.current_frame = 0.0;
                }
            }
            PlaybackMode::Once => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame;
                    self.is_playing = false;
                }
            }
            PlaybackMode::PingPong => {
                if self.playback_direction >= 1.0 {
                    if self.current_frame >= max_frame {
                        self.current_frame = max_frame * 2.0 - self.current_frame;
                        if self.current_frame >= max_frame {
                            self.current_frame = max_frame;
                        }
                        self.playback_direction = -1.0;
                    }
                } else if self.current_frame < 0.0 {
                    self.current_frame = -self.current_frame;
                    if self.current_frame >= max_frame {
                        self.current_frame = 0.0;
                    }
                    self.playback_direction = 1.0;
                }
            }
        }
    }

    /// Get current frame number (clamped to valid range)
    pub fn get_current_frame(&self) -> u32 {
        let max_frame = self.animation.frame_count.saturating_sub(1) as f32;
        self.current_frame.clamp(0.0, max_frame) as u32
    }

    /// Set playback position to specific frame
    pub fn seek_to_frame(&mut self, frame: u32) {
        let max_frame = self.animation.frame_count.saturating_sub(1) as f32;
        self.current_frame = (frame as f32).clamp(0.0, max_frame);
        self.playback_direction = 1.0;
    }

    /// Reset playback to start
    pub fn reset(&mut self) {
        self.current_frame = 0.0;
        self.is_playing = true;
        self.playback_direction = 1.0;
    }

    /// Pause playback without changing position
    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    /// Resume playback
    pub fn resume(&mut self) {
        self.is_playing = true;
    }

    /// Get bone transform at current frame.
    pub fn get_bone_transform(&self, bone_index: u32) -> Mat4 {
        let frame = self.get_current_frame() as f32;
        let bone_index = bone_index as usize;

        if let Some(compressed) = &self.animation.compressed_anim {
            if let Ok(mut anim) = compressed.lock() {
                return anim.get_transform(bone_index, frame);
            }
        }

        if let Some(hanim) = &self.animation.hanim {
            return hanim.get_transform(bone_index, frame);
        }

        Mat4::IDENTITY
    }

    /// Get bone translation at current frame.
    pub fn get_bone_translation(&self, bone_index: u32) -> Vec3 {
        let frame = self.get_current_frame() as f32;
        let bone_index = bone_index as usize;

        if let Some(compressed) = &self.animation.compressed_anim {
            if let Ok(mut anim) = compressed.lock() {
                return anim.get_translation(bone_index, frame);
            }
        }

        if let Some(hanim) = &self.animation.hanim {
            return hanim.get_translation(bone_index, frame);
        }

        Vec3::ZERO
    }

    /// Get bone rotation at current frame.
    pub fn get_bone_rotation(&self, bone_index: u32) -> Quat {
        let frame = self.get_current_frame() as f32;
        let bone_index = bone_index as usize;

        if let Some(compressed) = &self.animation.compressed_anim {
            if let Ok(mut anim) = compressed.lock() {
                return anim.get_orientation(bone_index, frame);
            }
        }

        if let Some(hanim) = &self.animation.hanim {
            return hanim.get_orientation(bone_index, frame);
        }

        Quat::IDENTITY
    }

    /// Get bone visibility at current frame.
    pub fn get_bone_visibility(&self, bone_index: u32) -> bool {
        let frame = self.get_current_frame() as f32;
        let bone_index = bone_index as usize;

        if let Some(compressed) = &self.animation.compressed_anim {
            if let Ok(mut anim) = compressed.lock() {
                return anim.get_visibility(bone_index, frame);
            }
        }

        if let Some(hanim) = &self.animation.hanim {
            return hanim.get_visibility(bone_index, frame);
        }

        true
    }

    /// Check if animation has finished playing (for Once mode)
    pub fn is_finished(&self) -> bool {
        !self.is_playing && self.current_frame >= self.animation.frame_count as f32 - 1.0
    }

    /// Get playback progress (0.0 = start, 1.0 = end)
    pub fn get_progress(&self) -> f32 {
        (self.current_frame / self.animation.frame_count as f32).clamp(0.0, 1.0)
    }
}

/// Cache for loaded animations to avoid reloading
pub struct AnimationCache {
    animations: std::collections::HashMap<String, LoadedAnimation>,
}

impl AnimationCache {
    /// Create a new animation cache
    pub fn new() -> Self {
        Self {
            animations: std::collections::HashMap::new(),
        }
    }

    /// Load animation, caching for future requests
    pub fn load_or_cache(
        &mut self,
        file_path: &str,
    ) -> AnimationLoaderResult<Arc<LoadedAnimation>> {
        // Check if already cached
        if let Some(anim) = self.animations.get(file_path) {
            return Ok(Arc::new(anim.clone()));
        }

        // Load from file
        let anim = W3DAnimationLoader::load_animation(file_path)?;
        let anim_arc = Arc::new(anim.clone());
        self.animations.insert(file_path.to_string(), anim);

        Ok(anim_arc)
    }

    /// Get cached animation if available
    pub fn get(&self, file_path: &str) -> Option<Arc<LoadedAnimation>> {
        self.animations.get(file_path).map(|a| Arc::new(a.clone()))
    }

    /// Clear animation cache
    pub fn clear(&mut self) {
        self.animations.clear();
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let total_size: usize = self
            .animations
            .values()
            .map(|a| std::mem::size_of_val(a) + 256) // Estimate + frame data
            .sum();

        CacheStats {
            animation_count: self.animations.len(),
            total_memory_bytes: total_size,
        }
    }
}

impl Default for AnimationCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub animation_count: usize,
    pub total_memory_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_playback(frame_count: u32, frame_rate: f32, mode: PlaybackMode) -> AnimationPlayback {
        let mut playback = AnimationPlayback::new(LoadedAnimation::new(
            "test".to_string(),
            frame_count,
            frame_rate,
            0,
        ));
        playback.mode = mode;
        playback
    }

    #[test]
    fn test_playback_mode_default() {
        assert_eq!(PlaybackMode::default(), PlaybackMode::Loop);
    }

    #[test]
    fn loop_playback_wraps_at_last_valid_frame() {
        let mut playback = test_playback(5, 1.0, PlaybackMode::Loop);

        playback.update(4.5);

        assert_eq!(playback.current_frame, 0.5);
        assert_eq!(playback.get_current_frame(), 0);
    }

    #[test]
    fn pingpong_playback_reflects_and_preserves_direction() {
        let mut playback = test_playback(5, 1.0, PlaybackMode::PingPong);

        playback.update(4.5);
        assert_eq!(playback.current_frame, 3.5);
        assert_eq!(playback.playback_direction, -1.0);

        playback.update(4.0);
        assert_eq!(playback.current_frame, 0.5);
        assert_eq!(playback.playback_direction, 1.0);
    }

    #[test]
    fn reset_and_seek_restore_forward_pingpong_direction() {
        let mut playback = test_playback(5, 1.0, PlaybackMode::PingPong);
        playback.update(4.5);
        assert_eq!(playback.playback_direction, -1.0);

        playback.seek_to_frame(2);
        assert_eq!(playback.current_frame, 2.0);
        assert_eq!(playback.playback_direction, 1.0);

        playback.update(4.0);
        assert_eq!(playback.playback_direction, -1.0);

        playback.reset();
        assert_eq!(playback.current_frame, 0.0);
        assert_eq!(playback.playback_direction, 1.0);
    }

    #[test]
    fn test_animation_cache_creation() {
        let cache = AnimationCache::new();
        assert_eq!(cache.animations.len(), 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = AnimationCache::new();
        let stats = cache.get_stats();
        assert_eq!(stats.animation_count, 0);
    }

    #[test]
    fn test_error_display() {
        let err = AnimationLoaderError::FileNotFound("test.w3d".to_string());
        assert_eq!(format!("{}", err), "File not found: test.w3d");
    }
}
