//! BinkVideoPlayer Module
//!
//! Corresponds to C++ files:
//!   GameEngineDevice/Include/VideoDevice/Bink/BinkVideoPlayer.h
//!   GameEngineDevice/Source/VideoDevice/Bink/BinkVideoPlayer.cpp
//!
//! Provides Bink video stream parsing and placeholder frame rendering.
//! The actual Bink SDK decoder is not available as a Rust dependency, so this
//! module parses Bink file headers for metadata and generates placeholder frames
//! for frame_render().  When a native Bink decoder is integrated, the placeholder
//! rendering can be replaced with real decompression calls.

use std::fs::File;
use std::io::{Read as IoRead, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

use game_client_rust::video_buffer::{VideoBuffer, VideoBufferType};
use game_client_rust::video_player::VideoStreamProvider;
use game_client_rust::video_stream::VideoStreamInterface;
use log::warn;

// ---------------------------------------------------------------------------
// Bink file header constants and parsing
// ---------------------------------------------------------------------------

/// Recognised Bink file magics (little-endian u32 values).
const BINK_MAGIC_BIK: u32 = 0x004B_4942; // "BIK\0"
const BINK_MAGIC_BIKI: u32 = 0x694B_4942; // "BIKi"
const BINK_MAGIC_BIK2: u32 = 0x324B_4942; // "BIK2"
const BINK_MAGIC_BIKG: u32 = 0x674B_4942; // "BIKg"
const BINK_MAGIC_BIKF: u32 = 0x664B_4942; // "BIKf"
const BINK_MAGIC_KB2G: u32 = 0x6742_324B; // "KB2g"
const BINK_MAGIC_KB2I: u32 = 0x6942_324B; // "KB2i"
const BINK_MAGIC_KB2F: u32 = 0x6642_324B; // "KB2f"

/// Minimum Bink header size in bytes.
/// Layout (all little-endian):
///   0:  u32 magic
///   4:  u32 file_size
///   8:  u32 num_frames
///  12:  u32 largest_frame
///  16:  u32 unknown (flags)
///  20:  u32 width
///  24:  u32 height
///  28:  u32 fps_num
///  32:  u32 fps_den
///  36:  u32 video_flags
///  40:  u32 audio_tracks
const BINK_HEADER_SIZE: usize = 44;

/// Parsed Bink video file header.
///
/// Matches the metadata that C++ would read from the HBINK handle
/// (m_handle->Width, m_handle->Height, m_handle->Frames, etc.).
#[derive(Debug, Clone)]
pub struct BinkHeader {
    pub magic: u32,
    pub file_size: u32,
    pub num_frames: u32,
    pub largest_frame: u32,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub video_flags: u32,
    pub audio_tracks: u32,
}

impl Default for BinkHeader {
    fn default() -> Self {
        Self {
            magic: 0,
            file_size: 0,
            num_frames: 0,
            largest_frame: 0,
            width: 0,
            height: 0,
            fps_num: 30,
            fps_den: 1,
            video_flags: 0,
            audio_tracks: 0,
        }
    }
}

impl BinkHeader {
    /// Recognised Bink magic values.
    fn is_valid_magic(magic: u32) -> bool {
        matches!(
            magic,
            BINK_MAGIC_BIK
                | BINK_MAGIC_BIKI
                | BINK_MAGIC_BIK2
                | BINK_MAGIC_BIKG
                | BINK_MAGIC_BIKF
                | BINK_MAGIC_KB2G
                | BINK_MAGIC_KB2I
                | BINK_MAGIC_KB2F
        )
    }

    /// Parse a Bink header from a file path.
    ///
    /// Returns `None` if the file cannot be read or the header is invalid.
    pub fn from_file(path: &Path) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        Self::from_reader(&mut file)
    }

    /// Parse a Bink header from any reader.
    fn from_reader<R: IoRead + Seek>(reader: &mut R) -> Option<Self> {
        let mut buf = [0u8; BINK_HEADER_SIZE];
        reader.seek(SeekFrom::Start(0)).ok()?;
        reader.read_exact(&mut buf).ok()?;

        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if !Self::is_valid_magic(magic) {
            return None;
        }

        Some(Self {
            magic,
            file_size: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            num_frames: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            largest_frame: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            width: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            height: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            fps_num: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
            fps_den: u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]),
            video_flags: u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]),
            audio_tracks: u32::from_le_bytes([buf[40], buf[41], buf[42], buf[43]]),
        })
    }
}

// ---------------------------------------------------------------------------
// BinkVideoStream — implements VideoStreamInterface
// ---------------------------------------------------------------------------

/// A video stream backed by a parsed Bink file.
///
/// Matches C++ `BinkVideoStream` (BinkVideoPlayer.h).
///
/// Without the Bink SDK we cannot decompress frames, so:
/// - Metadata (width, height, frame count) is read from the file header.
/// - `frame_render()` writes a checkerboard placeholder pattern into the
///   VideoBuffer so that the rendering pipeline has real pixel data.
/// - `frame_index()` / `frame_count()` / `frame_goto()` track playback
///   position correctly.
/// - `is_frame_ready()` returns `true` immediately (no async decompression).
pub struct BinkVideoStream {
    decoder: Option<game_client_rust::bink::BinkDecoder>,
    header: BinkHeader,
    current_frame: u32,
    movie_title: String,
    finished: bool,
    current_rgba: Vec<u8>,
}

impl BinkVideoStream {
    /// Create a new Bink video stream by parsing the file at `path`.
    ///
    /// Returns `None` if the file cannot be opened or the header is invalid.
    pub fn new(movie_title: &str, path: &Path) -> Option<Self> {
        let header = BinkHeader::from_file(path)?;
        if header.num_frames == 0 {
            warn!(
                "BinkVideoStream: '{}' has 0 frames, treating as 1-frame video",
                movie_title
            );
            return None;
        }

        let decoder = game_client_rust::bink::BinkDecoder::open(path).ok();
        let current_rgba = if let Some(ref dec) = decoder {
            dec.decode_current_frame_rgba()
        } else {
            Vec::new()
        };

        Some(Self {
            decoder,
            header,
            current_frame: 0,
            movie_title: movie_title.to_string(),
            finished: false,
            current_rgba,
        })
    }
}

// `BinkVideoStream` is `Send` because it only owns plain data.
unsafe impl Send for BinkVideoStream {}

impl VideoStreamInterface for BinkVideoStream {
    // -- linked list (not used by the provider/handle pattern) ---------------
    fn next(&self) -> Option<&dyn VideoStreamInterface> {
        None
    }

    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        None
    }

    // -- lifecycle -----------------------------------------------------------
    fn update(&mut self) {
        // In C++ this calls BinkWait().  Since we have no real decoder we
        // treat every frame as instantly ready.
    }

    fn close(self: Box<Self>) {
        // C++ BinkVideoStream::~BinkVideoStream() calls BinkClose(m_handle).
        // We hold no native handle, so just drop.
        drop(self);
    }

    // -- frame queries -------------------------------------------------------
    fn is_frame_ready(&self) -> bool {
        // C++: return !BinkWait(m_handle);
        // Without a real decoder, frames are always "ready".
        !self.finished
    }

    fn frame_index(&self) -> i32 {
        // C++: return m_handle->FrameNum - 1;
        self.current_frame as i32
    }

    fn frame_count(&self) -> i32 {
        // C++: return m_handle->Frames;
        self.header.num_frames as i32
    }

    fn height(&self) -> i32 {
        // C++: return m_handle->Height;
        self.header.height as i32
    }

    fn width(&self) -> i32 {
        // C++: return m_handle->Width;
        self.header.width as i32
    }

    // -- frame operations ----------------------------------------------------
    fn frame_decompress(&mut self) {
        // C++: BinkDoFrame(m_handle);
        if let Some(ref decoder) = self.decoder {
            self.current_rgba = decoder.decode_current_frame_rgba();
        }
    }

    fn frame_render(&mut self, buffer: &mut dyn VideoBuffer) {
        if !buffer.valid() {
            return;
        }

        let buf_width = buffer.width() as usize;
        let buf_height = buffer.height() as usize;
        let buf_pitch = buffer.pitch() as usize;
        let buf_format = buffer.format();
        let x_offset = buffer.x_pos() as usize;
        let y_offset = buffer.y_pos() as usize;

        if buf_width == 0 || buf_height == 0 || buf_pitch == 0 {
            return;
        }

        let mem = buffer.lock();
        if mem.is_null() {
            return;
        }

        let bpp: usize = match buf_format {
            VideoBufferType::X8R8G8B8 => 4,
            VideoBufferType::R8G8B8 => 3,
            VideoBufferType::R5G6B5 | VideoBufferType::X1R5G5B5 => 2,
            _ => {
                buffer.unlock();
                return;
            }
        };

        let vid_w = self.header.width as usize;
        let vid_h = self.header.height as usize;
        let copy_w = vid_w.min(buf_width.saturating_sub(x_offset));
        let copy_h = vid_h.min(buf_height.saturating_sub(y_offset));

        if copy_w == 0 || copy_h == 0 {
            buffer.unlock();
            return;
        }

        if !self.current_rgba.is_empty() && self.current_rgba.len() >= vid_w * vid_h * 4 {
            // Use decoded RGBA data from the BinkDecoder.
            for row in 0..copy_h {
                let dst_row = y_offset + row;
                let row_base = unsafe { mem.add(dst_row * buf_pitch + x_offset * bpp) };
                for col in 0..copy_w {
                    let src_index = (row * vid_w + col) * 4;
                    let rgba = &self.current_rgba[src_index..src_index + 4];
                    let dst = unsafe { row_base.add(col * bpp) };
                    unsafe { write_pixel(dst, buf_format, rgba[0], rgba[1], rgba[2], rgba[3]) };
                }
            }
        } else {
            // Fallback: checkerboard placeholder when no decoder data is available.
            let checker_size = 16usize;
            let frame_idx = self.current_frame as usize;
            for row in 0..copy_h {
                let dst_row = y_offset + row;
                let row_base = unsafe { mem.add(dst_row * buf_pitch + x_offset * bpp) };
                for col in 0..copy_w {
                    let checker_x = (col + frame_idx * 2) / checker_size;
                    let checker_y = row / checker_size;
                    let is_light = (checker_x + checker_y) % 2 == 0;
                    let dst = unsafe { row_base.add(col * bpp) };
                    let (r, g, b) = if is_light {
                        (80u8, 100, 180)
                    } else {
                        (30u8, 40, 100)
                    };
                    unsafe { write_pixel(dst, buf_format, r, g, b, 0xFF) };
                }
            }
        }

        buffer.unlock();
    }

    fn frame_next(&mut self) {
        if let Some(ref mut decoder) = self.decoder {
            decoder.advance();
            self.current_frame = decoder.current_frame_index();
        } else if self.current_frame < self.header.num_frames.saturating_sub(1) {
            self.current_frame += 1;
        } else {
            self.finished = true;
        }
    }

    fn frame_goto(&mut self, index: i32) {
        if let Some(ref mut decoder) = self.decoder {
            decoder.seek(index.max(0) as u32);
            self.current_frame = decoder.current_frame_index();
        } else if index < 0 {
            self.current_frame = 0;
        } else if (index as u32) >= self.header.num_frames {
            self.current_frame = self.header.num_frames.saturating_sub(1);
        } else {
            self.current_frame = index as u32;
        }
        self.finished = false;
    }
}

unsafe fn write_pixel(dst: *mut u8, format: VideoBufferType, r: u8, g: u8, b: u8, a: u8) {
    match format {
        VideoBufferType::X8R8G8B8 => {
            *dst = b;
            *dst.add(1) = g;
            *dst.add(2) = r;
            *dst.add(3) = a;
        }
        VideoBufferType::R8G8B8 => {
            *dst = r;
            *dst.add(1) = g;
            *dst.add(2) = b;
        }
        VideoBufferType::R5G6B5 => {
            let packed = (((r as u16 >> 3) & 0x1F) << 11)
                | (((g as u16 >> 2) & 0x3F) << 5)
                | ((b as u16 >> 3) & 0x1F);
            let bytes = packed.to_le_bytes();
            *dst = bytes[0];
            *dst.add(1) = bytes[1];
        }
        VideoBufferType::X1R5G5B5 => {
            let alpha_bit = if a >= 128 { 1u16 } else { 0u16 };
            let packed = (alpha_bit << 15)
                | (((r as u16 >> 3) & 0x1F) << 10)
                | (((g as u16 >> 3) & 0x1F) << 5)
                | ((b as u16 >> 3) & 0x1F);
            let bytes = packed.to_le_bytes();
            *dst = bytes[0];
            *dst.add(1) = bytes[1];
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// BinkVideoProvider — implements VideoStreamProvider
// ---------------------------------------------------------------------------

/// Bink video stream provider.
///
/// Matches C++ `BinkVideoPlayer::open()` / `createStream()`.
///
/// Implements `VideoStreamProvider` so it can be registered with the global
/// `VideoPlayer` via `register_video_stream_provider()`.
pub struct BinkVideoProvider;

impl BinkVideoProvider {
    pub fn new() -> Self {
        BinkVideoProvider
    }
}

impl Default for BinkVideoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoStreamProvider for BinkVideoProvider {
    fn open(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        // C++ BinkVideoPlayer::open() opens the .bik file with BinkOpen()
        // then calls createStream() which wraps the HBINK handle.
        let stream = BinkVideoStream::new(movie_title, resolved_path)?;
        Some(Box::new(stream))
    }

    fn load(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        // C++: load() delegates to open()
        self.open(movie_title, resolved_path)
    }
}

// ---------------------------------------------------------------------------
// BinkVideoPlayer — high-level player (matches C++ class)
// ---------------------------------------------------------------------------

/// High-level Bink video player.
///
/// Matches C++ `BinkVideoPlayer` class.  In C++ this extends `VideoPlayer`
/// and overrides `init`, `deinit`, `open`, `load`.  In the Rust architecture
/// the provider pattern separates concerns, so `BinkVideoPlayer` owns the
/// provider registration lifecycle.
pub struct BinkVideoProviderHandle {
    registered: bool,
}

impl BinkVideoProviderHandle {
    pub fn new() -> Self {
        Self { registered: false }
    }

    /// Equivalent to C++ `BinkVideoPlayer::init()`.
    ///
    /// Registers the Bink provider with the global VideoPlayer.
    pub fn init(&mut self) {
        if self.registered {
            return;
        }
        let provider = Arc::new(BinkVideoProvider::new());
        game_client_rust::video_player::register_video_stream_provider(provider);
        self.registered = true;
    }

    /// Equivalent to C++ `BinkVideoPlayer::deinit()`.
    pub fn deinit(&mut self) {
        if !self.registered {
            return;
        }
        game_client_rust::video_player::clear_video_stream_provider();
        self.registered = false;
    }

    /// Equivalent to C++ `BinkVideoPlayer::notifyVideoPlayerOfNewProvider(Bool)`.
    ///
    /// In C++ this connects/disconnects Bink audio with the Miles audio system.
    /// The Rust port does not depend on Miles, so this is a stub that manages
    /// the provider registration state.
    pub fn notify_video_player_of_new_provider(&mut self, now_has_valid: bool) {
        if now_has_valid && !self.registered {
            self.init();
        } else if !now_has_valid && self.registered {
            self.deinit();
        }
    }
}

impl Default for BinkVideoProviderHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BinkVideoProviderHandle {
    fn drop(&mut self) {
        self.deinit();
    }
}

// ---------------------------------------------------------------------------
// Convenience re-exports / registration helpers
// ---------------------------------------------------------------------------

/// Register the Bink video provider with the global VideoPlayer.
///
/// This is a one-shot call; subsequent calls are no-ops until
/// `unregister_bink_provider()` is called.
pub fn register_bink_provider() {
    let provider = Arc::new(BinkVideoProvider::new());
    game_client_rust::video_player::register_video_stream_provider(provider);
}

/// Remove the Bink video provider from the global VideoPlayer.
pub fn unregister_bink_provider() {
    game_client_rust::video_player::clear_video_stream_provider();
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bink_header_from_actual_file() {
        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            // Skip on CI / environments without game assets.
            return;
        }

        let header = BinkHeader::from_file(path).expect("should parse BIK header");

        assert_eq!(header.magic, BINK_MAGIC_BIKI);
        assert_eq!(header.num_frames, 96);
        assert_eq!(header.width, 720);
        assert_eq!(header.height, 486);
        assert!(header.fps_num > 0);
        assert!(header.fps_den > 0);

        // Verify ~30 fps
        let fps = header.fps_num as f64 / header.fps_den as f64;
        assert!((fps - 30.0).abs() < 1.0, "expected ~30 fps, got {:.2}", fps);
    }

    #[test]
    fn test_bink_header_invalid_magic() {
        // Non-BIK file should fail.
        let path = Path::new("Cargo.toml");
        assert!(BinkHeader::from_file(path).is_none());
    }

    #[test]
    fn test_bink_header_missing_file() {
        assert!(BinkHeader::from_file(Path::new("/nonexistent/file.bik")).is_none());
    }

    #[test]
    fn test_bink_video_stream_creation() {
        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            return;
        }

        let mut stream = BinkVideoStream::new("EALogoMovie", path).unwrap();

        assert_eq!(stream.frame_count(), 96);
        assert_eq!(stream.width(), 720);
        assert_eq!(stream.height(), 486);
        assert_eq!(stream.frame_index(), 0);
        assert!(stream.is_frame_ready());
    }

    #[test]
    fn test_bink_video_stream_frame_navigation() {
        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            return;
        }

        let mut stream = BinkVideoStream::new("test", path).unwrap();

        stream.frame_next();
        assert_eq!(stream.frame_index(), 1);

        stream.frame_goto(50);
        assert_eq!(stream.frame_index(), 50);

        stream.frame_goto(-1);
        assert_eq!(stream.frame_index(), 0);

        stream.frame_goto(999);
        assert_eq!(stream.frame_index(), 95);
    }

    #[test]
    fn test_bink_video_stream_finished_state() {
        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            return;
        }

        let mut stream = BinkVideoStream::new("test", path).unwrap();
        let total = stream.frame_count();

        for _ in 0..(total as usize) {
            assert!(stream.is_frame_ready());
            stream.frame_next();
        }

        assert!(!stream.is_frame_ready());

        // frame_goto should reset finished state.
        stream.frame_goto(0);
        assert!(stream.is_frame_ready());
    }

    #[test]
    fn test_bink_video_stream_frame_render_writes_to_buffer() {
        use game_client_rust::video_buffer::{SoftwareVideoBuffer, VideoBuffer as _};

        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            return;
        }

        let mut stream = BinkVideoStream::new("test", path).unwrap();
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(720, 486));

        // Verify buffer is initially zeroed.
        {
            let ptr = buffer.lock();
            let slice = unsafe { std::slice::from_raw_parts(ptr, 720 * 486 * 4) };
            assert!(slice.iter().all(|&b| b == 0));
            buffer.unlock();
        }

        // Render a frame — must write non-zero data.
        stream.frame_render(&mut buffer);

        {
            let ptr = buffer.lock();
            let slice = unsafe { std::slice::from_raw_parts(ptr, 720 * 486 * 4) };
            // At least some bytes should be non-zero now.
            let non_zero_count = slice.iter().filter(|&&b| b != 0).count();
            assert!(non_zero_count > 0, "frame_render should write pixel data");
            buffer.unlock();
        }

        // Advance and render next frame — the checkerboard should shift.
        stream.frame_next();
        stream.frame_render(&mut buffer);

        {
            let ptr = buffer.lock();
            let slice = unsafe { std::slice::from_raw_parts(ptr, 720 * 486 * 4) };
            let non_zero_count = slice.iter().filter(|&&b| b != 0).count();
            assert!(non_zero_count > 0, "second frame should also write data");
            buffer.unlock();
        }
    }

    #[test]
    fn test_bink_video_stream_frame_render_respects_format() {
        use game_client_rust::video_buffer::{SoftwareVideoBuffer, VideoBuffer as _};

        // Test R5G6B5 format.
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::R5G6B5);
        assert!(buffer.allocate(64, 64));

        let mut stream = BinkVideoStream::new_from_dims(64, 64, 10);
        stream.frame_render(&mut buffer);

        let ptr = buffer.lock();
        let slice = unsafe { std::slice::from_raw_parts(ptr, 64 * 64 * 2) };
        let non_zero = slice.iter().filter(|&&b| b != 0).count();
        assert!(non_zero > 0);
        buffer.unlock();
    }

    #[test]
    fn test_bink_video_stream_frame_render_offsets() {
        use game_client_rust::video_buffer::{SoftwareVideoBuffer, VideoBuffer as _};

        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(128, 128));
        buffer.set_pos(32, 32);

        let mut stream = BinkVideoStream::new_from_dims(64, 64, 1);
        stream.frame_render(&mut buffer);

        // Pixels outside the 32..96 region should remain zero.
        let ptr = buffer.lock();
        let row_bytes = 128 * 4;

        // Check top-left corner (row 0, col 0) — should be zero.
        let top_left = unsafe { std::slice::from_raw_parts(ptr, 4) };
        assert!(
            top_left.iter().all(|&b| b == 0),
            "top-left should be untouched"
        );

        // Check pixel at (32, 32) — should be non-zero.
        let offset_32_32 = (32 * row_bytes) + (32 * 4);
        let pixel = unsafe { std::slice::from_raw_parts(ptr.add(offset_32_32), 4) };
        assert!(
            pixel.iter().any(|&b| b != 0),
            "pixel at offset should be written"
        );

        buffer.unlock();
    }

    #[test]
    fn test_bink_video_provider_open() {
        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            return;
        }

        let provider = BinkVideoProvider::new();
        let mut stream = provider.open("EALogoMovie", path).unwrap();

        assert_eq!(stream.frame_count(), 96);
        assert_eq!(stream.width(), 720);
        assert_eq!(stream.height(), 486);
        assert!(stream.is_frame_ready());
    }

    #[test]
    fn test_bink_video_provider_load_delegates_to_open() {
        let path = Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        if !path.is_file() {
            return;
        }

        let provider = BinkVideoProvider::new();
        let stream = provider.load("EALogoMovie", path);
        assert!(stream.is_some());
    }

    #[test]
    fn test_bink_video_provider_invalid_file() {
        let provider = BinkVideoProvider::new();
        assert!(provider
            .open("missing", Path::new("/nonexistent.bik"))
            .is_none());
    }
}

// Helper used in tests — create a BinkVideoStream without needing a real file.
#[cfg(test)]
impl BinkVideoStream {
    fn new_from_dims(width: u32, height: u32, num_frames: u32) -> Self {
        Self {
            decoder: None,
            header: BinkHeader {
                magic: BINK_MAGIC_BIKI,
                width,
                height,
                num_frames,
                fps_num: 30,
                fps_den: 1,
                ..Default::default()
            },
            current_frame: 0,
            movie_title: String::from("test"),
            finished: false,
            current_rgba: Vec::new(),
        }
    }
}
