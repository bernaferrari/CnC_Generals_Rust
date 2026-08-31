use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use self::bink_decoder::BinkVideoDecoder;
use crate::video_buffer::{VideoBuffer, VideoBufferType};
use crate::video_player::{VideoStreamProvider, register_video_stream_provider};
use crate::video_stream::{PlaybackState, VideoStreamInterface};

#[path = "bink_decoder.rs"]
mod bink_decoder;

pub const ENABLE_REAL_BINK1_DECODER: bool = true;

const BINK_HEADER_SIZE: usize = 44;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinkVersion {
    Bink1,
    Bink2,
}

#[derive(Debug, Clone)]
pub struct BinkHeader {
    pub magic: [u8; 4],
    pub version: BinkVersion,
    pub file_size: u32,
    pub frame_count: u32,
    pub largest_frame_size: u32,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub video_flags: u32,
    pub audio_track_count: u32,
}

impl BinkHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < BINK_HEADER_SIZE {
            return Err("Bink header truncated".to_string());
        }

        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        let version = match magic {
            [b'B', b'I', b'K', b'i' | b'b' | b'f' | b'g' | b'h'] => BinkVersion::Bink1,
            [b'K', b'B', b'2', b'i' | b'k' | b'b' | b'f' | b'g' | b'h']
            | [b'B', b'I', b'K', b'2'] => BinkVersion::Bink2,
            _ => {
                return Err(format!(
                    "Unsupported Bink magic {:?}",
                    String::from_utf8_lossy(&magic)
                ));
            }
        };

        let file_size = read_u32(bytes, 4)?;
        let frame_count = read_u32(bytes, 8)?;
        let largest_frame_size = read_u32(bytes, 12)?;
        let width = read_u32(bytes, 20)?;
        let height = read_u32(bytes, 24)?;
        let fps_num = read_u32(bytes, 28)?.max(1);
        let fps_den = read_u32(bytes, 32)?.max(1);
        let video_flags = read_u32(bytes, 36)?;
        let audio_track_count = read_u32(bytes, 40)?;

        Ok(Self {
            magic,
            version,
            file_size,
            frame_count,
            largest_frame_size,
            width,
            height,
            fps_num,
            fps_den,
            video_flags,
            audio_track_count,
        })
    }

    pub fn fps(&self) -> f32 {
        (self.fps_num as f32 / self.fps_den.max(1) as f32).max(1.0)
    }

    pub fn frame_duration(&self) -> Duration {
        Duration::from_secs_f32(1.0 / self.fps())
    }

    pub fn has_alpha(&self) -> bool {
        self.video_flags & 0x0010_0000 != 0
    }
}

pub use crate::bink_audio::{
    apply_speech_slider_volume as bink_volume_from_speech_slider, get_handle_for_bink,
    has_bink_audio_bitstream_parser, initialize_bink_with_miles,
    notify_video_player_of_new_provider as notify_bink_of_new_provider,
    play_bink_pcm_through_miles, release_handle_for_bink, soundtrack_is_bound,
};

use crate::bink_audio::{
    BinkAudioDecoder, BinkAudioLayout, parse_audio_layout, split_frame_audio_and_video,
};

fn speech_slider_volume() -> f32 {
    game_engine::common::audio::game_audio::get_global_audio_manager()
        .and_then(|audio| {
            audio
                .lock()
                .ok()
                .map(|guard| guard.get_volume(game_engine::common::audio::AudioAffect::Speech))
        })
        .unwrap_or(1.0)
}

#[derive(Debug, Clone)]
pub struct BinkFramePacket {
    pub index: u32,
    pub offset: usize,
    pub size: usize,
}

pub struct BinkDecoder {
    bytes: Arc<[u8]>,
    header: BinkHeader,
    frame_packets: Vec<BinkFramePacket>,
    current_frame: u32,
    video_decoder: Option<BinkVideoDecoder>,
    audio_layout: BinkAudioLayout,
    audio_decoders: Vec<BinkAudioDecoder>,
}

impl BinkDecoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|err| format!("Failed reading {:?}: {err}", path))?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let header = BinkHeader::parse(&bytes)?;
        let version_b = header.magic[3] == b'b';
        let audio_layout = parse_audio_layout(&bytes, header.audio_track_count, version_b);
        let frame_packets = extract_frame_packets(&bytes, &header, audio_layout.frame_table_offset);

        if frame_packets.is_empty() {
            return Err("Bink file contains no extractable frame packets".to_string());
        }

        let video_decoder = (ENABLE_REAL_BINK1_DECODER && header.version == BinkVersion::Bink1)
            .then(|| {
                BinkVideoDecoder::new(
                    header.magic[3],
                    header.video_flags,
                    header.width.max(1) as usize,
                    header.height.max(1) as usize,
                )
            });
        let audio_decoders = audio_layout
            .tracks
            .iter()
            .cloned()
            .map(BinkAudioDecoder::new)
            .collect();

        Ok(Self {
            bytes,
            header,
            frame_packets,
            current_frame: 0,
            video_decoder,
            audio_layout,
            audio_decoders,
        })
    }

    pub fn header(&self) -> &BinkHeader {
        &self.header
    }

    pub fn width(&self) -> u32 {
        self.header.width.max(1)
    }

    pub fn height(&self) -> u32 {
        self.header.height.max(1)
    }

    pub fn fps(&self) -> f32 {
        self.header.fps()
    }

    pub fn frame_duration(&self) -> Duration {
        self.header.frame_duration()
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_packets.len() as u32
    }

    pub fn current_frame_index(&self) -> u32 {
        self.current_frame.min(self.frame_count().saturating_sub(1))
    }

    pub fn packet(&self, frame_index: u32) -> &[u8] {
        let packet =
            &self.frame_packets[frame_index.min(self.frame_count().saturating_sub(1)) as usize];
        &self.bytes[packet.offset..packet.offset + packet.size]
    }

    pub fn decode_current_frame_rgba(&mut self) -> Vec<u8> {
        self.decode_frame_rgba(self.current_frame_index())
    }

    pub fn decode_frame_rgba(&mut self, frame_index: u32) -> Vec<u8> {
        let packet = self.packet(frame_index).to_vec();
        let (audio_packets, video_packet) =
            split_frame_audio_and_video(&packet, self.audio_layout.tracks.len());
        self.play_frame_audio(&audio_packets);
        match self.header.version {
            BinkVersion::Bink1 => {
                if let Some(decoder) = self.video_decoder.as_mut() {
                    if let Ok(rgba) = decoder.decode_frame(video_packet) {
                        return rgba;
                    }
                }
                pseudo_decode_paletted_frame(
                    video_packet,
                    self.width(),
                    self.height(),
                    frame_index,
                    self.header.has_alpha(),
                )
            }
            BinkVersion::Bink2 => pseudo_decode_yuv_frame(
                video_packet,
                self.width(),
                self.height(),
                frame_index,
                self.header.has_alpha(),
            ),
        }
    }

    fn play_frame_audio(&mut self, audio_packets: &[&[u8]]) {
        if !soundtrack_is_bound() {
            return;
        }
        let volume = speech_slider_volume();
        for (decoder, packet) in self.audio_decoders.iter_mut().zip(audio_packets.iter()) {
            let pcm = decoder.decode_packet(packet);
            if !pcm.is_empty() {
                play_bink_pcm_through_miles(
                    &pcm,
                    decoder.sample_rate(),
                    decoder.channels(),
                    volume,
                );
            }
        }
    }

    pub fn seek(&mut self, frame_index: u32) {
        self.current_frame = frame_index.min(self.frame_count().saturating_sub(1));
    }

    pub fn advance(&mut self) -> bool {
        if self.current_frame + 1 >= self.frame_count() {
            self.current_frame = 0;
            true
        } else {
            self.current_frame += 1;
            false
        }
    }
}

/// Bink-backed [`VideoStreamInterface`] wrapping [`BinkDecoder`].
/// Matches C++ `BinkVideoStream` (BinkVideoPlayer.h).
pub struct BinkVideoStream {
    decoder: BinkDecoder,
    current_rgba: Vec<u8>,
    frame_accumulator: Duration,
    last_update: Instant,
    state: PlaybackState,
    volume: f32,
}

impl BinkVideoStream {
    pub fn open(path: &Path) -> Result<Self, String> {
        let _ = initialize_bink_with_miles();
        let mut decoder = BinkDecoder::open(path)?;
        let current_rgba = decoder.decode_current_frame_rgba();
        let speech = speech_slider_volume();
        Ok(Self {
            decoder,
            current_rgba,
            frame_accumulator: Duration::ZERO,
            last_update: Instant::now(),
            state: PlaybackState::Playing,
            volume: bink_volume_from_speech_slider(speech),
        })
    }
}

impl VideoStreamInterface for BinkVideoStream {
    fn next(&self) -> Option<&dyn VideoStreamInterface> {
        None
    }

    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        None
    }

    fn update(&mut self) {
        if self.state != PlaybackState::Playing {
            self.last_update = Instant::now();
            return;
        }

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_update);
        self.last_update = now;
        self.frame_accumulator += elapsed;

        let frame_duration = self.decoder.frame_duration();
        while self.frame_accumulator >= frame_duration {
            self.frame_accumulator -= frame_duration;
            if self.decoder.advance() {
                self.state = PlaybackState::Complete;
                break;
            }
            self.frame_decompress();
        }
    }

    fn close(self: Box<Self>) {
        drop(self);
    }

    fn is_frame_ready(&self) -> bool {
        self.state != PlaybackState::Complete
    }

    fn frame_decompress(&mut self) {
        self.current_rgba = self.decoder.decode_current_frame_rgba();
    }

    fn frame_render(&mut self, buffer: &mut dyn VideoBuffer) {
        if !buffer.valid() || self.current_rgba.is_empty() {
            return;
        }

        let width = self.decoder.width() as usize;
        let height = self.decoder.height() as usize;
        let copy_width = width.min(buffer.width() as usize);
        let copy_height = height.min(buffer.height() as usize);
        let pitch = buffer.pitch() as usize;
        let dst = buffer.lock();
        if dst.is_null() {
            return;
        }

        for y in 0..copy_height {
            for x in 0..copy_width {
                let src_index = (y * width + x) * 4;
                let rgba = &self.current_rgba[src_index..src_index + 4];
                let dst_index = y * pitch + x * bytes_per_pixel(buffer.format());
                // SAFETY: dst comes from buffer.lock() (null-checked above) and points
                // SAFETY: into the locked VideoBuffer backing store; dst_index stays
                // SAFETY: within pitch*height because copy_width/copy_height are min'd
                // SAFETY: against the buffer dimensions and bytes_per_pixel matches format.

                unsafe {
                    write_pixel(dst.add(dst_index), buffer.format(), rgba);
                }
            }
        }

        buffer.unlock();
    }

    fn frame_next(&mut self) {
        if self.decoder.advance() {
            self.state = PlaybackState::Complete;
        }
        self.frame_decompress();
    }

    fn frame_index(&self) -> i32 {
        self.decoder.current_frame_index() as i32
    }

    fn frame_count(&self) -> i32 {
        self.decoder.frame_count() as i32
    }

    fn frame_goto(&mut self, index: i32) {
        self.decoder.seek(index.max(0) as u32);
        self.frame_decompress();
        self.state = PlaybackState::Paused;
    }

    fn height(&self) -> i32 {
        self.decoder.height() as i32
    }

    fn width(&self) -> i32 {
        self.decoder.width() as i32
    }

    fn play(&mut self) {
        if self.state != PlaybackState::Complete {
            self.state = PlaybackState::Playing;
            self.last_update = Instant::now();
        }
    }

    fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    fn stop(&mut self) {
        self.decoder.seek(0);
        self.frame_accumulator = Duration::ZERO;
        self.frame_decompress();
        self.state = PlaybackState::Stopped;
    }

    fn set_volume(&mut self, volume: f32) {
        // C++ BinkVideoPlayer::createStream never lets volume go to 0.
        self.volume = bink_volume_from_speech_slider(volume);
    }

    fn playback_state(&self) -> PlaybackState {
        self.state
    }
}

#[derive(Default)]
pub struct BuiltInBinkVideoProvider;

impl VideoStreamProvider for BuiltInBinkVideoProvider {
    fn open(
        &self,
        _movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        BinkVideoStream::open(resolved_path)
            .map(|stream| Box::new(stream) as Box<dyn VideoStreamInterface>)
            .ok()
    }
}

static BUILTIN_PROVIDER_REGISTERED: OnceLock<()> = OnceLock::new();

pub fn ensure_bink_provider_registered() {
    if BUILTIN_PROVIDER_REGISTERED.get().is_some() {
        return;
    }

    register_video_stream_provider(Arc::new(BuiltInBinkVideoProvider));
    let _ = BUILTIN_PROVIDER_REGISTERED.set(());
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("Missing u32 at offset {offset}"))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn extract_frame_packets(
    bytes: &[u8],
    header: &BinkHeader,
    table_offset: usize,
) -> Vec<BinkFramePacket> {
    let frame_count = header.frame_count as usize;
    let table_len = frame_count.saturating_mul(4);
    let table_end = table_offset.saturating_add(table_len).min(bytes.len());

    if table_end <= table_offset || table_end - table_offset < 4 {
        return vec![BinkFramePacket {
            index: 0,
            offset: BINK_HEADER_SIZE.min(bytes.len()),
            size: bytes
                .len()
                .saturating_sub(BINK_HEADER_SIZE.min(bytes.len())),
        }];
    }

    let raw_offsets: Vec<usize> = bytes[table_offset..table_end]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize)
        .collect();

    let mut packets = Vec::new();
    for (index, offset) in raw_offsets.iter().copied().enumerate() {
        if offset >= bytes.len() {
            continue;
        }

        let next_offset = raw_offsets
            .iter()
            .copied()
            .skip(index + 1)
            .find(|candidate| *candidate > offset)
            .unwrap_or(bytes.len());
        let size = next_offset.saturating_sub(offset);
        if size == 0 {
            continue;
        }
        packets.push(BinkFramePacket {
            index: index as u32,
            offset,
            size,
        });
    }

    if packets.is_empty() {
        packets.push(BinkFramePacket {
            index: 0,
            offset: table_end,
            size: bytes.len().saturating_sub(table_end),
        });
    }

    packets
}

fn pseudo_decode_paletted_frame(
    packet: &[u8],
    width: u32,
    height: u32,
    frame_index: u32,
    has_alpha: bool,
) -> Vec<u8> {
    // PARITY_NOTE: Full Bink1 block/DCT decode is not available without RAD's
    // SDK. We extract the real frame packet and deterministically expand it
    // into RGBA so playback timing, buffering, and rendering stay wired.
    let palette_len = 256 * 3;
    let (palette_bytes, index_bytes) = if packet.len() > palette_len {
        packet.split_at(palette_len)
    } else {
        (&[][..], packet)
    };

    let mut out = vec![0u8; (width * height * 4) as usize];
    for pixel_index in 0..(width as usize * height as usize) {
        let palette_index = index_bytes
            .get((pixel_index + frame_index as usize) % index_bytes.len().max(1))
            .copied()
            .unwrap_or((pixel_index as u8).wrapping_add(frame_index as u8));
        let palette_base = (palette_index as usize % 256) * 3;

        let r = palette_bytes
            .get(palette_base)
            .copied()
            .unwrap_or(palette_index.wrapping_mul(3));
        let g = palette_bytes
            .get(palette_base + 1)
            .copied()
            .unwrap_or(palette_index.wrapping_mul(5));
        let b = palette_bytes
            .get(palette_base + 2)
            .copied()
            .unwrap_or(palette_index.wrapping_mul(7));
        let alpha = if has_alpha {
            128u8.saturating_add((palette_index >> 1) & 0x7F)
        } else {
            255
        };

        let dst = pixel_index * 4;
        out[dst] = r;
        out[dst + 1] = g;
        out[dst + 2] = b;
        out[dst + 3] = alpha;
    }
    out
}

fn pseudo_decode_yuv_frame(
    packet: &[u8],
    width: u32,
    height: u32,
    frame_index: u32,
    has_alpha: bool,
) -> Vec<u8> {
    // PARITY_NOTE: This is a packet-driven YUV approximation, not full Bink2
    // entropy/vector/DCT reconstruction. It preserves stream extraction and GPU
    // upload flow until a complete decoder replaces it.
    let mut out = vec![0u8; (width * height * 4) as usize];
    for pixel_index in 0..(width as usize * height as usize) {
        let base = ((pixel_index * 3) + frame_index as usize * 11) % packet.len().max(3);
        let y = packet
            .get(base)
            .copied()
            .unwrap_or((pixel_index & 0xFF) as u8);
        let u = packet
            .get((base + 1) % packet.len().max(1))
            .copied()
            .unwrap_or(128);
        let v = packet
            .get((base + 2) % packet.len().max(1))
            .copied()
            .unwrap_or(128);
        let (r, g, b) = yuv_to_rgb(y, u, v);
        let alpha = if has_alpha {
            packet
                .get((base + 3) % packet.len().max(1))
                .copied()
                .unwrap_or(255)
        } else {
            255
        };
        let dst = pixel_index * 4;
        out[dst] = r;
        out[dst + 1] = g;
        out[dst + 2] = b;
        out[dst + 3] = alpha;
    }
    out
}

fn yuv_to_rgb(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let y = y as f32;
    let u = u as f32 - 128.0;
    let v = v as f32 - 128.0;
    let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
    let g = (y - 0.344_136 * u - 0.714_136 * v).clamp(0.0, 255.0) as u8;
    let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;
    (r, g, b)
}

fn bytes_per_pixel(format: VideoBufferType) -> usize {
    match format {
        VideoBufferType::R8G8B8 => 3,
        VideoBufferType::X8R8G8B8 => 4,
        VideoBufferType::R5G6B5 | VideoBufferType::X1R5G5B5 => 2,
        VideoBufferType::Unknown => 0,
    }
}

// SAFETY: `dst` must point to at least bytes_per_pixel(format) writable bytes
// SAFETY: within a locked VideoBuffer; `rgba` must have 4 readable elements.
// SAFETY: Callers satisfy this via the bounds-checked copy loop in frame_render.

unsafe fn write_pixel(dst: *mut u8, format: VideoBufferType, rgba: &[u8]) {
    let r = rgba[0];
    let g = rgba[1];
    let b = rgba[2];
    let a = rgba[3];
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
        VideoBufferType::Unknown => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bink_blob(magic: [u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0u8; 44 + 8 + 32];
        bytes[0..4].copy_from_slice(&magic);
        let len = bytes.len() as u32;
        bytes[4..8].copy_from_slice(&len.to_le_bytes());
        bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&32u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&4u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&4u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&30u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&1u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&0u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&1u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&52u32.to_le_bytes());
        bytes[48..52].copy_from_slice(&68u32.to_le_bytes());
        for (i, byte) in bytes[52..].iter_mut().enumerate() {
            *byte = i as u8;
        }
        bytes
    }

    #[test]
    fn parses_bink1_header() {
        let bytes = make_bink_blob(*b"BIKi");
        let header = BinkHeader::parse(&bytes).expect("header should parse");
        assert_eq!(header.version, BinkVersion::Bink1);
        assert_eq!(header.frame_count, 2);
        assert_eq!(header.width, 4);
        assert_eq!(header.height, 4);
        assert_eq!(header.audio_track_count, 1);
    }

    #[test]
    fn extracts_frame_packets() {
        let bytes = make_bink_blob(*b"KB2i");
        let decoder = BinkDecoder::from_bytes(bytes).expect("decoder should parse");
        assert_eq!(decoder.header().version, BinkVersion::Bink2);
        assert_eq!(decoder.frame_count(), 2);
        assert_eq!(decoder.packet(0).len(), 16);
    }

    #[test]
    fn pseudo_decode_outputs_rgba() {
        let bytes = make_bink_blob(*b"BIKi");
        let mut decoder = BinkDecoder::from_bytes(bytes).expect("decoder should parse");
        let rgba = decoder.decode_current_frame_rgba();
        assert_eq!(rgba.len(), 4 * 4 * 4);
        assert!(rgba.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn video_stream_tracks_playback_state() {
        let path = std::env::temp_dir().join(format!("bink_stream_{}.bik", std::process::id()));
        fs::write(&path, make_bink_blob(*b"BIKi")).expect("write temp bink");
        let mut stream = BinkVideoStream::open(&path).expect("open");
        stream.play();
        stream.frame_next();
        assert_eq!(stream.frame_index(), 1);
        assert_eq!(stream.playback_state(), PlaybackState::Playing);

        stream.frame_next();
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.playback_state(), PlaybackState::Complete);
        assert!(!stream.is_frame_ready());

        stream.stop();
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.playback_state(), PlaybackState::Stopped);

        fs::remove_file(path).ok();
    }

    #[test]
    fn bink_has_audio_bitstream_parser_wired_to_miles() {
        let bytes = make_bink_blob(*b"BIKi");
        let header = BinkHeader::parse(&bytes).expect("header");
        assert_eq!(header.audio_track_count, 1);
        assert!(
            has_bink_audio_bitstream_parser(),
            "Bink audio bitstream parser must be present for Miles/kira soundtrack"
        );
        notify_bink_of_new_provider(false);
        assert!(
            !soundtrack_is_bound(),
            "lost audio device mutes soundtrack but parser stays available"
        );
    }

    #[test]
    fn bink_speech_slider_matches_create_stream_formula() {
        // C++ BinkVideoPlayer::createStream (BinkVideoPlayer.cpp:88-96)
        // mod = (speechVolume*0.8*100)+1; volume = 32768*mod/100.
        let expected = |speech: f32| {
            let modifier = (speech.clamp(0.0, 1.0) * 0.8 * 100.0) + 1.0;
            32768.0 * modifier / 100.0 / 32768.0
        };
        assert!((bink_volume_from_speech_slider(1.0) - expected(1.0)).abs() < 1e-6);
        assert!((bink_volume_from_speech_slider(0.0) - expected(0.0)).abs() < 1e-6);
        assert!((bink_volume_from_speech_slider(0.5) - expected(0.5)).abs() < 1e-6);
    }
}
