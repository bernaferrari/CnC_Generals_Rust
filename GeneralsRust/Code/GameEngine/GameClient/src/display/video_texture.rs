//! Video Texture System
//!
//! Provides video playback as textures for in-game cinematics,
//! menu backgrounds, and window video elements.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::{Device, Queue, Texture, TextureView};

/// Video playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoState {
    /// Video is stopped
    Stopped,
    /// Video is playing
    Playing,
    /// Video is paused
    Paused,
    /// Video has finished
    Finished,
    /// Video failed to load or play
    Error,
}

/// Video loop mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play once and stop
    Once,
    /// Loop indefinitely
    Loop,
    /// Loop a specific number of times
    Count(u32),
}

/// Video format support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    /// BIK video format (common in C&C games)
    Bik,
    /// MP4 format
    Mp4,
    /// WebM format
    WebM,
    /// AVI format
    Avi,
}

impl VideoFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext.to_lowercase().as_str() {
                "bik" => Some(VideoFormat::Bik),
                "mp4" => Some(VideoFormat::Mp4),
                "webm" => Some(VideoFormat::WebM),
                "avi" => Some(VideoFormat::Avi),
                _ => None,
            })
    }
}

/// Video texture configuration
#[derive(Debug, Clone)]
pub struct VideoConfig {
    /// Loop mode
    pub loop_mode: LoopMode,
    /// Audio enabled
    pub audio_enabled: bool,
    /// Playback speed multiplier
    pub playback_speed: f32,
    /// Auto-start playback
    pub auto_play: bool,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            loop_mode: LoopMode::Once,
            audio_enabled: true,
            playback_speed: 1.0,
            auto_play: true,
            volume: 1.0,
        }
    }
}

/// Video frame data
pub struct VideoFrame {
    width: u32,
    height: u32,
    data: Vec<u8>,
    timestamp: Duration,
}

impl VideoFrame {
    pub fn new(width: u32, height: u32, data: Vec<u8>, timestamp: Duration) -> Self {
        Self {
            width,
            height,
            data,
            timestamp,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn timestamp(&self) -> Duration {
        self.timestamp
    }
}

/// Video decoder interface (platform-specific implementation)
pub trait VideoDecoder: Send + Sync {
    /// Initialize decoder with video file
    fn init(&mut self, path: &Path) -> Result<(), String>;

    /// Get video width
    fn width(&self) -> u32;

    /// Get video height
    fn height(&self) -> u32;

    /// Get video duration
    fn duration(&self) -> Duration;

    /// Get frame rate
    fn fps(&self) -> f32;

    /// Seek to timestamp
    fn seek(&mut self, timestamp: Duration) -> Result<(), String>;

    /// Decode next frame
    fn decode_frame(&mut self) -> Result<Option<VideoFrame>, String>;

    /// Check if end of video
    fn is_finished(&self) -> bool;

    /// Reset to beginning
    fn reset(&mut self) -> Result<(), String>;
}

/// Stub video decoder for testing/development
pub struct StubVideoDecoder {
    width: u32,
    height: u32,
    duration: Duration,
    fps: f32,
    current_frame: u32,
    total_frames: u32,
}

impl StubVideoDecoder {
    pub fn new(width: u32, height: u32, duration: Duration, fps: f32) -> Self {
        let total_frames = (duration.as_secs_f32() * fps) as u32;
        Self {
            width,
            height,
            duration,
            fps,
            current_frame: 0,
            total_frames,
        }
    }
}

impl VideoDecoder for StubVideoDecoder {
    fn init(&mut self, _path: &Path) -> Result<(), String> {
        Ok(())
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn duration(&self) -> Duration {
        self.duration
    }

    fn fps(&self) -> f32 {
        self.fps
    }

    fn seek(&mut self, timestamp: Duration) -> Result<(), String> {
        self.current_frame = (timestamp.as_secs_f32() * self.fps) as u32;
        Ok(())
    }

    fn decode_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        if self.current_frame >= self.total_frames {
            return Ok(None);
        }

        let timestamp = Duration::from_secs_f32(self.current_frame as f32 / self.fps);
        let size = (self.width * self.height * 4) as usize;
        let data = vec![128u8; size]; // Gray frame

        self.current_frame += 1;

        Ok(Some(VideoFrame::new(self.width, self.height, data, timestamp)))
    }

    fn is_finished(&self) -> bool {
        self.current_frame >= self.total_frames
    }

    fn reset(&mut self) -> Result<(), String> {
        self.current_frame = 0;
        Ok(())
    }
}

/// Video texture for rendering
pub struct VideoTexture {
    path: PathBuf,
    decoder: Box<dyn VideoDecoder>,
    config: VideoConfig,
    state: VideoState,
    texture: Option<Texture>,
    texture_view: Option<TextureView>,
    current_time: Duration,
    last_update: Instant,
    loop_count: u32,
}

impl VideoTexture {
    /// Create a new video texture
    pub fn new(path: PathBuf, decoder: Box<dyn VideoDecoder>, config: VideoConfig) -> Self {
        Self {
            path,
            decoder,
            config,
            state: VideoState::Stopped,
            texture: None,
            texture_view: None,
            current_time: Duration::ZERO,
            last_update: Instant::now(),
            loop_count: 0,
        }
    }

    /// Initialize video texture with GPU device
    pub fn initialize(&mut self, device: &Device) -> Result<(), String> {
        self.decoder.init(&self.path)?;

        // Create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Texture"),
            size: wgpu::Extent3d {
                width: self.decoder.width(),
                height: self.decoder.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.texture = Some(texture);
        self.texture_view = Some(texture_view);

        if self.config.auto_play {
            self.play();
        }

        Ok(())
    }

    /// Get texture view
    pub fn texture_view(&self) -> Option<&TextureView> {
        self.texture_view.as_ref()
    }

    /// Get video state
    pub fn state(&self) -> VideoState {
        self.state
    }

    /// Play video
    pub fn play(&mut self) {
        if self.state == VideoState::Finished {
            let _ = self.decoder.reset();
            self.current_time = Duration::ZERO;
        }
        self.state = VideoState::Playing;
        self.last_update = Instant::now();
    }

    /// Pause video
    pub fn pause(&mut self) {
        if self.state == VideoState::Playing {
            self.state = VideoState::Paused;
        }
    }

    /// Stop video
    pub fn stop(&mut self) {
        self.state = VideoState::Stopped;
        let _ = self.decoder.reset();
        self.current_time = Duration::ZERO;
    }

    /// Seek to timestamp
    pub fn seek(&mut self, timestamp: Duration) -> Result<(), String> {
        self.decoder.seek(timestamp)?;
        self.current_time = timestamp;
        Ok(())
    }

    /// Get current playback time
    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    /// Get video duration
    pub fn duration(&self) -> Duration {
        self.decoder.duration()
    }

    /// Get video dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.decoder.width(), self.decoder.height())
    }

    /// Update video (decode frames)
    pub fn update(&mut self, queue: &Queue) -> Result<(), String> {
        if self.state != VideoState::Playing {
            return Ok(());
        }

        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();

        // Update time with playback speed
        self.current_time += elapsed.mul_f32(self.config.playback_speed);

        // Decode and upload frame
        if let Some(frame) = self.decoder.decode_frame()? {
            if let Some(texture) = &self.texture {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    frame.data(),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(frame.width() * 4),
                        rows_per_image: Some(frame.height()),
                    },
                    wgpu::Extent3d {
                        width: frame.width(),
                        height: frame.height(),
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        // Check for end of video
        if self.decoder.is_finished() {
            match self.config.loop_mode {
                LoopMode::Once => {
                    self.state = VideoState::Finished;
                }
                LoopMode::Loop => {
                    self.decoder.reset()?;
                    self.current_time = Duration::ZERO;
                    self.loop_count += 1;
                }
                LoopMode::Count(max_loops) => {
                    self.loop_count += 1;
                    if self.loop_count < max_loops {
                        self.decoder.reset()?;
                        self.current_time = Duration::ZERO;
                    } else {
                        self.state = VideoState::Finished;
                    }
                }
            }
        }

        Ok(())
    }

    /// Get loop count
    pub fn loop_count(&self) -> u32 {
        self.loop_count
    }

    /// Set volume
    pub fn set_volume(&mut self, volume: f32) {
        self.config.volume = volume.clamp(0.0, 1.0);
    }

    /// Get volume
    pub fn volume(&self) -> f32 {
        self.config.volume
    }
}

/// Video texture manager
pub struct VideoTextureManager {
    videos: Vec<Arc<Mutex<VideoTexture>>>,
}

impl VideoTextureManager {
    /// Create a new video texture manager
    pub fn new() -> Self {
        Self { videos: Vec::new() }
    }

    /// Load a video
    pub fn load_video(
        &mut self,
        path: PathBuf,
        decoder: Box<dyn VideoDecoder>,
        config: VideoConfig,
    ) -> Arc<Mutex<VideoTexture>> {
        let video = Arc::new(Mutex::new(VideoTexture::new(path, decoder, config)));
        self.videos.push(video.clone());
        video
    }

    /// Update all videos
    pub fn update_all(&mut self, queue: &Queue) {
        for video in &self.videos {
            if let Ok(mut video) = video.lock() {
                let _ = video.update(queue);
            }
        }
    }

    /// Clear all videos
    pub fn clear(&mut self) {
        self.videos.clear();
    }
}

impl Default for VideoTextureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_format_detection() {
        assert_eq!(
            VideoFormat::from_path(Path::new("video.bik")),
            Some(VideoFormat::Bik)
        );
        assert_eq!(
            VideoFormat::from_path(Path::new("video.mp4")),
            Some(VideoFormat::Mp4)
        );
        assert_eq!(VideoFormat::from_path(Path::new("unknown.xyz")), None);
    }

    #[test]
    fn test_stub_decoder() {
        let mut decoder = StubVideoDecoder::new(640, 480, Duration::from_secs(10), 30.0);
        assert_eq!(decoder.width(), 640);
        assert_eq!(decoder.height(), 480);
        assert_eq!(decoder.duration(), Duration::from_secs(10));
        assert_eq!(decoder.fps(), 30.0);
    }

    #[test]
    fn test_video_state() {
        let decoder = Box::new(StubVideoDecoder::new(640, 480, Duration::from_secs(1), 30.0));
        let mut video = VideoTexture::new(
            PathBuf::from("test.bik"),
            decoder,
            VideoConfig::default(),
        );

        assert_eq!(video.state(), VideoState::Stopped);

        video.play();
        assert_eq!(video.state(), VideoState::Playing);

        video.pause();
        assert_eq!(video.state(), VideoState::Paused);

        video.stop();
        assert_eq!(video.state(), VideoState::Stopped);
    }
}
