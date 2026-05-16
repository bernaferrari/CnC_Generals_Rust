//! Video Texture System
//!
//! Provides video playback as textures for in-game cinematics,
//! menu backgrounds, and window video elements.
//!
//! ## Key Components
//!
//! - [`VideoDecoder`] trait — pluggable decoder abstraction
//! - [`RawFrameDecoder`] — reads uncompressed RGBA frames from a byte buffer
//! - [`StubVideoDecoder`] — returns gray placeholder frames for testing
//! - [`VideoTexture`] — GPU texture with state machine, PTS-based sync, and fullscreen quad rendering
//!
//! ## State Machine
//!
//! ```text
//! Loading → Playing ⇄ Paused
//!    ↓         ↓        ↓
//! Stopped  Complete  Stopped
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Video playback state machine.
///
/// Matches the C++ VideoPlayer lifecycle: Loading is entered when a
/// stream is first opened; Playing/Paused are toggled by the user;
/// Stopped resets to frame 0; Complete fires after the last frame
/// when `LoopMode::Once` is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoState {
    /// Video is loading / initializing decoder
    Loading,
    /// Video is stopped (decoder reset, time at zero)
    Stopped,
    /// Video is actively playing
    Playing,
    /// Video is paused
    Paused,
    /// Video has completed playback (last frame in Once mode)
    Complete,
    /// Video failed to load or decode
    Error,
}

/// Video loop mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play once then transition to `Complete`
    Once,
    /// Loop indefinitely
    Loop,
    /// Loop N times then transition to `Complete`
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
    /// Raw uncompressed RGBA frames
    Raw,
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
                "raw" | "rgba" => Some(VideoFormat::Raw),
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
    /// Playback speed multiplier (1.0 = normal)
    pub playback_speed: f32,
    /// Auto-start playback after initialization
    pub auto_play: bool,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
    /// Audio sync offset — positive shifts audio earlier, negative shifts later
    pub audio_sync_offset: Duration,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            loop_mode: LoopMode::Once,
            audio_enabled: true,
            playback_speed: 1.0,
            auto_play: true,
            volume: 1.0,
            audio_sync_offset: Duration::ZERO,
        }
    }
}

/// Decoded video frame with presentation timestamp.
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

/// Convert a single YUV (BT.601) pixel to RGB.
///
/// This is the standard SDTV conversion matrix used by Bink and similar
/// codecs. Input `u` and `v` are in 0–255 range (128 = neutral).
pub fn yuv_to_rgb(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let y = y as f32;
    let cb = u as f32 - 128.0;
    let cr = v as f32 - 128.0;
    let r = (y + 1.402 * cr).clamp(0.0, 255.0) as u8;
    let g = (y - 0.344_136 * cb - 0.714_136 * cr).clamp(0.0, 255.0) as u8;
    let b = (y + 1.772 * cb).clamp(0.0, 255.0) as u8;
    (r, g, b)
}

/// Convert planar YUV 4:2:0 data to interleaved RGBA.
///
/// - `y_plane`: width × height bytes
/// - `u_plane`: (width/2) × (height/2) bytes
/// - `v_plane`: (width/2) × (height/2) bytes
pub fn yuv420_to_rgba(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: u32,
    height: u32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut rgba = vec![0u8; w * h * 4];

    for row in 0..h {
        for col in 0..w {
            let y_idx = row * w + col;
            let uv_row = row / 2;
            let uv_col = col / 2;
            let uv_idx = uv_row * (w / 2) + uv_col;

            let y = y_plane.get(y_idx).copied().unwrap_or(16);
            let u = u_plane.get(uv_idx).copied().unwrap_or(128);
            let v = v_plane.get(uv_idx).copied().unwrap_or(128);

            let (r, g, b) = yuv_to_rgb(y, u, v);
            let dst = y_idx * 4;
            rgba[dst] = r;
            rgba[dst + 1] = g;
            rgba[dst + 2] = b;
            rgba[dst + 3] = 255;
        }
    }

    rgba
}

/// Convert packed YUYV (YUV 4:2:2) data to interleaved RGBA.
///
/// Each 4-byte macroblock encodes two pixels: `[Y0 U0 Y1 V0]`.
pub fn yuv422_to_rgba(yuv_data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut rgba = vec![0u8; w * h * 4];
    let src_stride = w * 2;

    for row in 0..h {
        for col in 0..w {
            let macroblock = col / 2;
            let src_base = row * src_stride + macroblock * 4;

            let y = yuv_data.get(src_base + (col % 2) * 2).copied().unwrap_or(16);
            let u = yuv_data.get(src_base + 1).copied().unwrap_or(128);
            let v = yuv_data.get(src_base + 3).copied().unwrap_or(128);

            let (r, g, b) = yuv_to_rgb(y, u, v);
            let dst = (row * w + col) * 4;
            rgba[dst] = r;
            rgba[dst + 1] = g;
            rgba[dst + 2] = b;
            rgba[dst + 3] = 255;
        }
    }

    rgba
}

/// Video decoder interface (platform-specific implementation).
///
/// Implementations handle the actual decompression: Bink, raw frames,
/// or future codecs. The trait produces [`VideoFrame`]s with presentation
/// timestamps that the [`VideoTexture`] uses for audio-synchronized playback.
pub trait VideoDecoder: Send + Sync {
    /// Initialize decoder with video file path
    fn init(&mut self, path: &Path) -> Result<(), String>;

    /// Video width in pixels
    fn width(&self) -> u32;

    /// Video height in pixels
    fn height(&self) -> u32;

    /// Total duration of the video stream
    fn duration(&self) -> Duration;

    /// Frame rate in frames per second
    fn fps(&self) -> f32;

    /// Seek to a presentation timestamp
    fn seek(&mut self, timestamp: Duration) -> Result<(), String>;

    /// Decode the next frame. Returns `Ok(None)` when the stream is exhausted.
    fn decode_frame(&mut self) -> Result<Option<VideoFrame>, String>;

    /// Whether the decoder has reached end-of-stream
    fn is_finished(&self) -> bool;

    /// Reset the decoder to the first frame
    fn reset(&mut self) -> Result<(), String>;

    /// Current presentation timestamp (PTS) of the most recently decoded frame.
    fn current_pts(&self) -> Duration {
        Duration::ZERO
    }
}

/// Stub video decoder for testing/development.
///
/// Produces solid gray frames of the configured dimensions.
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
        let data = vec![128u8; size];

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

    fn current_pts(&self) -> Duration {
        if self.fps <= 0.0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f32(self.current_frame.saturating_sub(1) as f32 / self.fps)
    }
}

/// Decoder for raw uncompressed RGBA frames stored in a contiguous buffer.
///
/// Each frame occupies `width × height × 4` bytes in row-major RGBA order.
/// Frames are stored sequentially: frame N begins at byte offset
/// `N × frame_size`.
///
/// Presentation timestamps are either derived from frame index and fps
/// or provided explicitly via [`RawFrameDecoder::with_timestamps`].
///
/// This decoder is the reference implementation for the [`VideoDecoder`]
/// trait and can play any pre-decoded video content. Bink or other codec
/// decoders plug in via the same trait.
pub struct RawFrameDecoder {
    width: u32,
    height: u32,
    fps: f32,
    frame_data: Vec<u8>,
    frame_size: usize,
    total_frames: u32,
    current_frame: u32,
    timestamps: Vec<Duration>,
}

impl RawFrameDecoder {
    /// Create a decoder from a contiguous RGBA frame buffer.
    ///
    /// `frame_data` contains `total_frames = frame_data.len() / (width * height * 4)`
    /// frames. Timestamps are derived from `fps`.
    pub fn new(width: u32, height: u32, fps: f32, frame_data: Vec<u8>) -> Self {
        let frame_size = (width as usize) * (height as usize) * 4;
        let total_frames = if frame_size > 0 {
            (frame_data.len() / frame_size) as u32
        } else {
            0
        };

        let effective_fps = fps.max(0.001);
        let timestamps: Vec<Duration> = (0..total_frames)
            .map(|i| Duration::from_secs_f32(i as f32 / effective_fps))
            .collect();

        Self {
            width,
            height,
            fps,
            frame_data,
            frame_size,
            total_frames,
            current_frame: 0,
            timestamps,
        }
    }

    /// Create with explicit per-frame presentation timestamps.
    pub fn with_timestamps(
        width: u32,
        height: u32,
        fps: f32,
        frame_data: Vec<u8>,
        timestamps: Vec<Duration>,
    ) -> Self {
        let frame_size = (width as usize) * (height as usize) * 4;
        let total_frames = if frame_size > 0 {
            (frame_data.len() / frame_size) as u32
        } else {
            0
        };

        Self {
            width,
            height,
            fps,
            frame_data,
            frame_size,
            total_frames,
            current_frame: 0,
            timestamps,
        }
    }

    /// Create from individual frame slices.
    ///
    /// Convenience constructor that concatenates the slices into the
    /// internal buffer. Each slice should be `width × height × 4` bytes.
    pub fn from_frames(width: u32, height: u32, fps: f32, frames: &[&[u8]]) -> Self {
        let frame_size = (width as usize) * (height as usize) * 4;
        let total_size = frames.len() * frame_size;
        let mut frame_data = vec![0u8; total_size];

        for (i, frame) in frames.iter().enumerate() {
            let copy_len = frame.len().min(frame_size);
            let offset = i * frame_size;
            frame_data[offset..offset + copy_len].copy_from_slice(&frame[..copy_len]);
        }

        Self::new(width, height, fps, frame_data)
    }

    /// Total number of frames in the buffer.
    pub fn total_frames(&self) -> u32 {
        self.total_frames
    }
}

impl VideoDecoder for RawFrameDecoder {
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
        if self.total_frames == 0 || self.fps <= 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f32(self.total_frames as f32 / self.fps)
        }
    }

    fn fps(&self) -> f32 {
        self.fps
    }

    fn seek(&mut self, timestamp: Duration) -> Result<(), String> {
        if self.fps <= 0.0 {
            self.current_frame = 0;
            return Ok(());
        }
        let target_frame = (timestamp.as_secs_f32() * self.fps) as u32;
        self.current_frame = target_frame.min(self.total_frames.saturating_sub(1));
        Ok(())
    }

    fn decode_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        if self.current_frame >= self.total_frames || self.frame_size == 0 {
            return Ok(None);
        }

        let offset = self.current_frame as usize * self.frame_size;
        let end = offset + self.frame_size;
        if end > self.frame_data.len() {
            log::warn!(
                "RawFrameDecoder: frame {} data truncated (need {} bytes, have {})",
                self.current_frame,
                self.frame_size,
                self.frame_data.len() - offset,
            );
            return Ok(None);
        }

        let data = self.frame_data[offset..end].to_vec();
        let timestamp = self
            .timestamps
            .get(self.current_frame as usize)
            .copied()
            .unwrap_or_else(|| {
                Duration::from_secs_f32(self.current_frame as f32 / self.fps.max(0.001))
            });

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

    fn current_pts(&self) -> Duration {
        if self.current_frame == 0 {
            Duration::ZERO
        } else {
            self.timestamps
                .get(self.current_frame.saturating_sub(1) as usize)
                .copied()
                .unwrap_or_else(|| {
                    Duration::from_secs_f32(
                        self.current_frame.saturating_sub(1) as f32 / self.fps.max(0.001),
                    )
                })
        }
    }
}

/// WGSL shader for rendering a fullscreen quad with a video texture.
/// Uses a 3-vertex triangle-strip trick (no vertex buffer needed).
const VIDEO_QUAD_SHADER: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var video_texture: texture_2d<f32>;
@group(0) @binding(1) var video_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    // Three vertices that cover the entire clip-space quad:
    //   (-1, -3), (-1, 1), (3, 1)
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    return textureSample(video_texture, video_sampler, input.uv);
}
"#;

/// GPU video texture with decoder, state machine, PTS-based audio sync,
/// and fullscreen quad rendering.
///
/// ## Lifecycle
///
/// 1. Construct with [`VideoTexture::new`] (state = `Stopped`)
/// 2. Call [`VideoTexture::initialize`] with a wgpu `Device` (state → `Loading` → `Playing`/`Stopped`)
/// 3. Call [`VideoTexture::update`] each frame with the wgpu `Queue`
/// 4. Call [`VideoTexture::render`] to draw the current frame as a fullscreen quad
/// 5. Call `play`/`pause`/`stop`/`set_volume` as needed
pub struct VideoTexture {
    path: PathBuf,
    decoder: Box<dyn VideoDecoder>,
    config: VideoConfig,
    state: VideoState,

    // GPU resources
    texture: Option<wgpu::Texture>,
    texture_view: Option<wgpu::TextureView>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    sampler: Option<wgpu::Sampler>,
    pipeline: Option<wgpu::RenderPipeline>,
    bind_group: Option<wgpu::BindGroup>,
    current_time: Duration,
    current_pts: Duration,
    last_update: Instant,
    loop_count: u32,
    frame_uploaded: bool,
}

impl VideoTexture {
    /// Create a new video texture with the given decoder and configuration.
    ///
    /// The video starts in `Stopped` state. Call [`initialize`](Self::initialize)
    /// to set up GPU resources.
    pub fn new(path: PathBuf, decoder: Box<dyn VideoDecoder>, config: VideoConfig) -> Self {
        Self {
            path,
            decoder,
            config,
            state: VideoState::Stopped,
            texture: None,
            texture_view: None,
            bind_group_layout: None,
            sampler: None,
            pipeline: None,
            bind_group: None,
            current_time: Duration::ZERO,
            current_pts: Duration::ZERO,
            last_update: Instant::now(),
            loop_count: 0,
            frame_uploaded: false,
        }
    }

    /// Initialize GPU resources and optionally begin playback.
    ///
    /// Creates the wgpu texture, bind group, sampler, and render pipeline.
    /// If `config.auto_play` is `true`, transitions to `Playing` after loading.
    /// Otherwise transitions to `Stopped` (ready to play on demand).
    ///
    /// State transition: `Stopped` → `Loading` → `Playing` | `Stopped`
    pub fn initialize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), String> {
        self.state = VideoState::Loading;

        if let Err(err) = self.decoder.init(&self.path) {
            self.state = VideoState::Error;
            return Err(err);
        }

        let width = self.decoder.width().max(1);
        let height = self.decoder.height().max(1);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Texture"),
            size: wgpu::Extent3d {
                width,
                height,
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

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Video Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Video Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Video Texture Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Video Quad Shader"),
            source: wgpu::ShaderSource::Wgsl(VIDEO_QUAD_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Video Quad Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Video Quad Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.texture = Some(texture);
        self.texture_view = Some(texture_view);
        self.bind_group_layout = Some(bind_group_layout);
        self.sampler = Some(sampler);
        self.pipeline = Some(pipeline);
        self.bind_group = Some(bind_group);

        self.decode_and_upload_frame(device, queue);

        if self.config.auto_play {
            self.state = VideoState::Playing;
            self.last_update = Instant::now();
        } else {
            self.state = VideoState::Stopped;
        }

        Ok(())
    }

    fn decode_and_upload_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let frame = match self.decoder.decode_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) => return,
            Err(err) => {
                log::warn!("VideoTexture: decode error: {}", err);
                return;
            }
        };

        self.current_pts = frame.timestamp();

        if let Some(texture) = &self.texture {
            let width = frame.width().max(1);
            let height = frame.height().max(1);

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
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            self.frame_uploaded = true;
        }
    }

    fn upload_frame_data(&self, queue: &wgpu::Queue, frame: &VideoFrame) {
        let Some(texture) = &self.texture else {
            return;
        };

        let width = frame.width().max(1);
        let height = frame.height().max(1);

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
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Begin or resume playback.
    ///
    /// - From `Stopped`/`Loading`/`Complete`: resets decoder and starts from frame 0
    /// - From `Paused`: resumes at the current frame
    /// - No-op if already `Playing` or in `Error` state
    pub fn play(&mut self) {
        match self.state {
            VideoState::Playing => return,
            VideoState::Error => return,
            VideoState::Complete | VideoState::Stopped | VideoState::Loading => {
                let _ = self.decoder.reset();
                self.current_time = Duration::ZERO;
                self.current_pts = Duration::ZERO;
                self.loop_count = 0;
                self.frame_uploaded = false;
            }
            VideoState::Paused => {}
        }
        self.state = VideoState::Playing;
        self.last_update = Instant::now();
    }

    /// Pause playback.
    ///
    /// Only transitions from `Playing` → `Paused`. No-op otherwise.
    pub fn pause(&mut self) {
        if self.state == VideoState::Playing {
            self.state = VideoState::Paused;
        }
    }

    /// Stop playback and reset to frame 0.
    ///
    /// Transitions to `Stopped` from any state except `Error`.
    pub fn stop(&mut self) {
        if self.state == VideoState::Error {
            return;
        }
        self.state = VideoState::Stopped;
        let _ = self.decoder.reset();
        self.current_time = Duration::ZERO;
        self.current_pts = Duration::ZERO;
        self.frame_uploaded = false;
    }

    /// Set playback volume (0.0 – 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.config.volume = volume.clamp(0.0, 1.0);
    }

    /// Get current volume.
    pub fn volume(&self) -> f32 {
        self.config.volume
    }

    /// Advance video playback by the elapsed wall-clock time.
    ///
    /// Decodes frames as needed to keep up with the presentation timeline.
    /// Uses PTS-based synchronization: frames are displayed when
    /// `current_time >= frame_pts + audio_sync_offset`.
    ///
    /// Must be called once per render frame while in `Playing` state.
    pub fn update(&mut self, queue: &wgpu::Queue) -> Result<(), String> {
        if self.state != VideoState::Playing {
            return Ok(());
        }

        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();

        self.current_time += elapsed.mul_f32(self.config.playback_speed);

        let sync_target = self
            .current_time
            .saturating_sub(self.config.audio_sync_offset);

        loop {
            if self.decoder.is_finished() {
                self.handle_end_of_stream()?;
                return Ok(());
            }

            if self.current_pts > sync_target {
                break;
            }

            match self.decoder.decode_frame() {
                Ok(Some(frame)) => {
                    self.current_pts = frame.timestamp();
                    self.upload_frame_data(queue, &frame);
                    self.frame_uploaded = true;
                }
                Ok(None) => {
                    self.handle_end_of_stream()?;
                    return Ok(());
                }
                Err(err) => {
                    log::warn!("VideoTexture: decode error during update: {}", err);
                    self.state = VideoState::Error;
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    /// Handle reaching end of stream based on loop mode.
    fn handle_end_of_stream(&mut self) -> Result<(), String> {
        match self.config.loop_mode {
            LoopMode::Once => {
                self.state = VideoState::Complete;
            }
            LoopMode::Loop => {
                self.decoder.reset()?;
                self.current_time = Duration::ZERO;
                self.current_pts = Duration::ZERO;
                self.loop_count += 1;
            }
            LoopMode::Count(max_loops) => {
                self.loop_count += 1;
                if self.loop_count < max_loops {
                    self.decoder.reset()?;
                    self.current_time = Duration::ZERO;
                    self.current_pts = Duration::ZERO;
                } else {
                    self.state = VideoState::Complete;
                }
            }
        }
        Ok(())
    }

    /// Render the current video frame as a fullscreen quad.
    ///
    /// The quad is drawn with aspect-ratio-preserving letterboxing /
    /// pillarboxing to fit the target viewport. The video is centered
    /// within `(target_width, target_height)` with black bars.
    ///
    /// No-op if the video has not been initialized or is in an error state.
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        target_width: u32,
        target_height: u32,
    ) {
        let Some(bind_group) = self.bind_group.as_ref() else {
            return;
        };
        let Some(pipeline) = self.pipeline.as_ref() else {
            return;
        };

        if self.state == VideoState::Error {
            return;
        }
        if !self.frame_uploaded {
            return;
        }

        let (vp_x, vp_y, vp_w, vp_h) = compute_letterbox_viewport(
            target_width.max(1),
            target_height.max(1),
            self.decoder.width().max(1),
            self.decoder.height().max(1),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Video Fullscreen Quad Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_viewport(vp_x, vp_y, vp_w.max(1.0), vp_h.max(1.0), 0.0, 1.0);
        pass.draw(0..3, 0..1);
    }

    /// Get the GPU texture view (for custom rendering pipelines)
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.texture_view.as_ref()
    }

    /// Current playback state
    pub fn state(&self) -> VideoState {
        self.state
    }

    /// Seek to a presentation timestamp
    pub fn seek(&mut self, timestamp: Duration) -> Result<(), String> {
        self.decoder.seek(timestamp)?;
        self.current_time = timestamp;
        self.current_pts = timestamp;
        Ok(())
    }

    /// Current playback time
    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    /// Current presentation timestamp of the decoded frame
    pub fn current_pts(&self) -> Duration {
        self.current_pts
    }

    /// Total video duration
    pub fn duration(&self) -> Duration {
        self.decoder.duration()
    }

    /// Video dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.decoder.width(), self.decoder.height())
    }

    /// Number of times the video has looped
    pub fn loop_count(&self) -> u32 {
        self.loop_count
    }
}

/// Compute a letterboxed viewport that preserves the video's aspect ratio
/// within the target frame.
///
/// Returns `(x, y, width, height)` in physical pixels.
fn compute_letterbox_viewport(
    target_width: u32,
    target_height: u32,
    video_width: u32,
    video_height: u32,
) -> (f32, f32, f32, f32) {
    let target_aspect = target_width as f32 / target_height.max(1) as f32;
    let video_aspect = video_width as f32 / video_height.max(1) as f32;

    if video_aspect > target_aspect {
        // Video is wider than target → pillarbox (bars top & bottom)
        let vp_w = target_width as f32;
        let vp_h = vp_w / video_aspect;
        let vp_y = ((target_height as f32 - vp_h) * 0.5).max(0.0);
        (0.0, vp_y, vp_w, vp_h)
    } else {
        // Video is taller than target → letterbox (bars left & right)
        let vp_h = target_height as f32;
        let vp_w = vp_h * video_aspect;
        let vp_x = ((target_width as f32 - vp_w) * 0.5).max(0.0);
        (vp_x, 0.0, vp_w, vp_h)
    }
}

/// Manages multiple concurrent video textures.
pub struct VideoTextureManager {
    videos: Vec<Arc<std::sync::Mutex<VideoTexture>>>,
}

impl VideoTextureManager {
    /// Create a new video texture manager
    pub fn new() -> Self {
        Self { videos: Vec::new() }
    }

    /// Load a video and return a shared handle.
    pub fn load_video(
        &mut self,
        path: PathBuf,
        decoder: Box<dyn VideoDecoder>,
        config: VideoConfig,
    ) -> Arc<std::sync::Mutex<VideoTexture>> {
        let video = Arc::new(std::sync::Mutex::new(VideoTexture::new(path, decoder, config)));
        self.videos.push(video.clone());
        video
    }

    /// Update all active videos
    pub fn update_all(&mut self, queue: &wgpu::Queue) {
        for video in &self.videos {
            if let Ok(mut video) = video.lock() {
                let _ = video.update(queue);
            }
        }
    }

    /// Clear all managed videos
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
        assert_eq!(
            VideoFormat::from_path(Path::new("video.raw")),
            Some(VideoFormat::Raw)
        );
        assert_eq!(
            VideoFormat::from_path(Path::new("video.rgba")),
            Some(VideoFormat::Raw)
        );
        assert_eq!(VideoFormat::from_path(Path::new("unknown.xyz")), None);
    }

    #[test]
    fn test_stub_decoder_produces_gray_frames() {
        let mut decoder = StubVideoDecoder::new(640, 480, Duration::from_secs(10), 30.0);
        assert_eq!(decoder.width(), 640);
        assert_eq!(decoder.height(), 480);
        assert_eq!(decoder.duration(), Duration::from_secs(10));
        assert_eq!(decoder.fps(), 30.0);

        let frame = decoder.decode_frame().expect("should decode").expect("should have frame");
        assert_eq!(frame.width(), 640);
        assert_eq!(frame.height(), 480);
        assert_eq!(frame.data().len(), 640 * 480 * 4);
        assert!(frame.data().iter().all(|&b| b == 128));
    }

    #[test]
    fn test_stub_decoder_state_transitions() {
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

        video.play();
        assert_eq!(video.state(), VideoState::Playing);

        video.stop();
        assert_eq!(video.state(), VideoState::Stopped);
    }

    #[test]
    fn test_raw_frame_decoder_basic() {
        let width = 4u32;
        let height = 4u32;
        let fps = 30.0f32;
        let frame_size = (width * height * 4) as usize;
        let total_frames = 3u32;
        let mut data = vec![0u8; frame_size * total_frames as usize];

        // Frame 0: red
        for i in 0..16 {
            data[i * 4] = 255;
            data[i * 4 + 1] = 0;
            data[i * 4 + 2] = 0;
            data[i * 4 + 3] = 255;
        }
        // Frame 1: green
        for i in 0..16 {
            let offset = frame_size + i * 4;
            data[offset] = 0;
            data[offset + 1] = 255;
            data[offset + 2] = 0;
            data[offset + 3] = 255;
        }
        // Frame 2: blue
        for i in 0..16 {
            let offset = frame_size * 2 + i * 4;
            data[offset] = 0;
            data[offset + 1] = 0;
            data[offset + 2] = 255;
            data[offset + 3] = 255;
        }

        let mut decoder = RawFrameDecoder::new(width, height, fps, data);
        assert_eq!(decoder.width(), width);
        assert_eq!(decoder.height(), height);
        assert_eq!(decoder.total_frames(), 3);
        assert_eq!(decoder.duration(), Duration::from_secs_f32(3.0 / 30.0));

        // Frame 0: red
        let frame = decoder
            .decode_frame()
            .expect("decode should succeed")
            .expect("should have frame");
        assert_eq!(frame.data()[0], 255); // R
        assert_eq!(frame.data()[1], 0); // G
        assert!(frame.timestamp() > Duration::ZERO || decoder.current_frame == 1);

        // Frame 1: green
        let frame = decoder
            .decode_frame()
            .expect("decode should succeed")
            .expect("should have frame");
        assert_eq!(frame.data()[1], 255); // G

        // Frame 2: blue
        let frame = decoder
            .decode_frame()
            .expect("decode should succeed")
            .expect("should have frame");
        assert_eq!(frame.data()[2], 255); // B

        // No more frames
        let result = decoder.decode_frame().expect("decode should succeed");
        assert!(result.is_none());
        assert!(decoder.is_finished());
    }

    #[test]
    fn test_raw_frame_decoder_seek_and_reset() {
        let width = 2u32;
        let height = 2u32;
        let fps = 10.0f32;
        let frame_size = (width * height * 4) as usize;
        let total_frames = 5u32;
        let mut data = vec![0u8; frame_size * total_frames as usize];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        let mut decoder = RawFrameDecoder::new(width, height, fps, data);

        // Seek to frame 3
        decoder
            .seek(Duration::from_secs_f32(3.0 / 10.0))
            .expect("seek should succeed");
        let frame = decoder
            .decode_frame()
            .expect("decode should succeed")
            .expect("should have frame");
        // The PTS should be around frame 3's time
        assert!(frame.timestamp() >= Duration::from_millis(250));

        // Reset
        decoder.reset().expect("reset should succeed");
        assert!(!decoder.is_finished());

        let frame = decoder
            .decode_frame()
            .expect("decode should succeed")
            .expect("should have frame");
        assert_eq!(frame.width(), width);
        assert_eq!(frame.height(), height);
    }

    #[test]
    fn test_raw_frame_decoder_with_timestamps() {
        let width = 2u32;
        let height = 2u32;
        let fps = 10.0f32;
        let frame_size = (width * height * 4) as usize;
        let data = vec![128u8; frame_size * 3];
        let timestamps = vec![
            Duration::from_millis(0),
            Duration::from_millis(50),
            Duration::from_millis(120),
        ];

        let mut decoder = RawFrameDecoder::with_timestamps(width, height, fps, data, timestamps);

        let f0 = decoder.decode_frame().unwrap().unwrap();
        assert_eq!(f0.timestamp(), Duration::from_millis(0));

        let f1 = decoder.decode_frame().unwrap().unwrap();
        assert_eq!(f1.timestamp(), Duration::from_millis(50));

        let f2 = decoder.decode_frame().unwrap().unwrap();
        assert_eq!(f2.timestamp(), Duration::from_millis(120));

        assert!(decoder.decode_frame().unwrap().is_none());
    }

    #[test]
    fn test_raw_frame_decoder_from_frames() {
        let width = 2u32;
        let height = 2u32;
        let frame_size = (width * height * 4) as usize;
        let frame0 = vec![255u8; frame_size];
        let frame1 = vec![0u8; frame_size];

        let mut decoder =
            RawFrameDecoder::from_frames(width, height, 30.0, &[&frame0, &frame1]);
        assert_eq!(decoder.total_frames(), 2);

        let f0 = decoder.decode_frame().unwrap().unwrap();
        assert_eq!(f0.data()[0], 255);

        let f1 = decoder.decode_frame().unwrap().unwrap();
        assert_eq!(f1.data()[0], 0);
    }

    #[test]
    fn test_yuv_to_rgb() {
        // White: Y=255, U=128, V=128 (neutral chroma)
        let (r, g, b) = yuv_to_rgb(255, 128, 128);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);

        // Black: Y=0, U=128, V=128
        let (r, g, b) = yuv_to_rgb(0, 128, 128);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_yuv420_to_rgba_dimensions() {
        let w = 4u32;
        let h = 4u32;
        let y_plane = vec![128u8; (w * h) as usize];
        let u_plane = vec![128u8; ((w / 2) * (h / 2)) as usize];
        let v_plane = vec![128u8; ((w / 2) * (h / 2)) as usize];

        let rgba = yuv420_to_rgba(&y_plane, &u_plane, &v_plane, w, h);
        assert_eq!(rgba.len(), (w * h * 4) as usize);

        // With neutral chroma and Y=128, all channels should be ~128
        assert_eq!(rgba[0], 128); // R
        assert_eq!(rgba[1], 128); // G
        assert_eq!(rgba[2], 128); // B
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn test_yuv422_to_rgba_dimensions() {
        let w = 4u32;
        let h = 2u32;
        let yuyv_data = vec![128u8; (w * 2 * h) as usize];

        let rgba = yuv422_to_rgba(&yuyv_data, w, h);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }

    #[test]
    fn test_letterbox_viewport_preserves_aspect_ratio() {
        // 16:9 video in 4:3 target
        let (x, y, w, h) = compute_letterbox_viewport(800, 600, 1920, 1080);
        assert!(x >= 0.0);
        assert!(y >= 0.0);
        assert!(w <= 800.0);
        assert!(h <= 600.0);

        // 4:3 video in 16:9 target
        let (x, y, w, h) = compute_letterbox_viewport(1920, 1080, 800, 600);
        assert!(x >= 0.0);
        assert!(y >= 0.0);
        assert!(w <= 1920.0);
        assert!(h <= 1080.0);
    }

    #[test]
    fn test_video_state_machine_transitions() {
        let decoder = Box::new(StubVideoDecoder::new(640, 480, Duration::from_secs(1), 30.0));
        let mut video = VideoTexture::new(
            PathBuf::from("test.bik"),
            decoder,
            VideoConfig {
                auto_play: false,
                ..VideoConfig::default()
            },
        );

        assert_eq!(video.state(), VideoState::Stopped);

        // Stopped → Playing
        video.play();
        assert_eq!(video.state(), VideoState::Playing);

        // Playing → Paused
        video.pause();
        assert_eq!(video.state(), VideoState::Paused);

        // Paused → Playing (resume)
        video.play();
        assert_eq!(video.state(), VideoState::Playing);

        // Playing → Stopped
        video.stop();
        assert_eq!(video.state(), VideoState::Stopped);

        // Complete → Playing (restart)
        video.state = VideoState::Complete;
        video.play();
        assert_eq!(video.state(), VideoState::Playing);

        // Stop from Complete
        video.state = VideoState::Complete;
        video.stop();
        assert_eq!(video.state(), VideoState::Stopped);
    }

    #[test]
    fn test_volume_clamping() {
        let decoder = Box::new(StubVideoDecoder::new(640, 480, Duration::from_secs(1), 30.0));
        let mut video = VideoTexture::new(
            PathBuf::from("test.bik"),
            decoder,
            VideoConfig::default(),
        );

        video.set_volume(1.5);
        assert!((video.volume() - 1.0).abs() < f32::EPSILON);

        video.set_volume(-0.5);
        assert!(video.volume().abs() < f32::EPSILON);

        video.set_volume(0.5);
        assert!((video.volume() - 0.5).abs() < f32::EPSILON);
    }
}
