use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use self::bink_decoder::BinkVideoDecoder;
use crate::video_buffer::{VideoBuffer, VideoBufferType};
use crate::video_player::VideoStreamProvider;
use crate::video_stream::{PlaybackState, VideoStreamInterface};

#[path = "bink_decoder.rs"]
mod bink_decoder;

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
    apply_speech_slider_volume as bink_volume_from_speech_slider, has_bink_audio_bitstream_parser,
    BinkAudioState,
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
    video_decoder: BinkVideoDecoder,
    audio_layout: BinkAudioLayout,
    audio_decoders: Vec<BinkAudioDecoder>,
    audio: BinkAudioState,
}

impl BinkDecoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|err| format!("Failed reading {:?}: {err}", path))?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let header = BinkHeader::parse(&bytes)?;
        if header.version == BinkVersion::Bink2 {
            return Err("Bink2 video decoding is unsupported".to_owned());
        }
        if header.has_alpha() {
            return Err("Bink1 alpha-plane decoding is unsupported".to_owned());
        }
        if !(1..=7680).contains(&header.width) || !(1..=4800).contains(&header.height) {
            return Err(format!(
                "Invalid Bink dimensions {}x{}",
                header.width, header.height
            ));
        }
        if header.audio_track_count > 256 {
            return Err(format!("Too many Bink audio tracks: {}", header.audio_track_count));
        }
        let version_b = header.magic[3] == b'b';
        let audio_layout = parse_audio_layout(&bytes, header.audio_track_count, version_b);
        let frame_packets = extract_frame_packets(&bytes, &header, audio_layout.frame_table_offset)?;
        let video_decoder = BinkVideoDecoder::new(
            header.magic[3],
            header.video_flags,
            header.width as usize,
            header.height as usize,
        );
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
            audio: BinkAudioState::default(),
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

    pub fn decode_current_frame_rgba(&mut self) -> Result<Vec<u8>, String> {
        self.decode_frame_rgba(self.current_frame_index())
    }

    pub fn decode_frame_rgba(&mut self, frame_index: u32) -> Result<Vec<u8>, String> {
        let packet = self.packet(frame_index).to_vec();
        let (audio_packets, video_packet) =
            split_frame_audio_and_video(&packet, self.audio_layout.tracks.len());
        let rgba = self
            .video_decoder
            .decode_frame(video_packet)
            .map_err(|error| format!("Bink frame {frame_index}: {error}"))?;
        self.play_frame_audio(&audio_packets);
        Ok(rgba)
    }

    fn play_frame_audio(&mut self, audio_packets: &[&[u8]]) {
        if !self.audio.soundtrack_is_bound() {
            return;
        }
        let volume = speech_slider_volume();
        for (decoder, packet) in self.audio_decoders.iter_mut().zip(audio_packets.iter()) {
            let pcm = decoder.decode_packet(packet);
            if !pcm.is_empty() {
                self.audio.play_bink_pcm_through_miles(
                    &pcm,
                    decoder.sample_rate(),
                    decoder.channels(),
                    volume,
                );
            }
        }
    }

    pub fn audio_mut(&mut self) -> &mut BinkAudioState {
        &mut self.audio
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
    decoded_frame: Option<u32>,
    next_frame_ready: Instant,
    state: PlaybackState,
    volume: f32,
}

impl BinkVideoStream {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut decoder = BinkDecoder::open(path)?;
        let _ = decoder.audio_mut().initialize_bink_with_miles();
        let current_rgba = decoder
            .decode_current_frame_rgba()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let speech = speech_slider_volume();
        Ok(Self {
            decoder,
            current_rgba,
            decoded_frame: Some(0),
            next_frame_ready: Instant::now(),
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
        // BinkVideoPlayer.cpp:293 only services BinkWait here. The Display or
        // window that presents the movie owns the frameNext transition.
    }

    fn close(self: Box<Self>) {
        drop(self);
    }

    fn is_frame_ready(&self) -> bool {
        self.state != PlaybackState::Complete && Instant::now() >= self.next_frame_ready
    }

    fn has_decode_failed(&self) -> bool {
        self.current_rgba.is_empty()
    }

    fn frame_decompress(&mut self) {
        let index = self.decoder.current_frame_index();
        if self.has_decode_failed() || self.decoded_frame == Some(index) {
            return;
        }
        match self.decoder.decode_current_frame_rgba() {
            Ok(rgba) => {
                self.current_rgba = rgba;
                self.decoded_frame = Some(index);
            }
            Err(error) => {
                log::warn!("Skipping unreadable movie: {error}");
                self.current_rgba.clear();
                self.state = PlaybackState::Complete;
            }
        }
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
        let format = buffer.format();
        let bpp = bytes_per_pixel(format);
        let Some(dst) = buffer.lock() else {
            return;
        };

        if pitch != 0 && bpp != 0 {
            let copy_width = copy_width.min(pitch / bpp);
            for (y, row) in dst.chunks_exact_mut(pitch).take(copy_height).enumerate() {
                for (x, pixel) in row.chunks_exact_mut(bpp).take(copy_width).enumerate() {
                    let src_index = (y * width + x) * 4;
                    let rgba = &self.current_rgba[src_index..src_index + 4];
                    write_pixel(pixel, format, rgba);
                }
            }
        }

        buffer.unlock();
    }

    fn frame_next(&mut self) {
        // A caller may advance a hidden frame without presenting it. Decode
        // the current packet first so its prediction is available to the next.
        self.frame_decompress();
        if self.has_decode_failed() {
            return;
        }
        let now = Instant::now();
        let frame_duration = self.decoder.frame_duration();
        let following_frame = self.next_frame_ready + frame_duration;
        self.next_frame_ready = if following_frame <= now {
            now + frame_duration
        } else {
            following_frame
        };
        if self.decoder.advance() {
            self.decoder.video_decoder.clear_reference_frames();
            self.decoded_frame = None;
        }
        // Decompress when the presenter is ready. At wrap the window can hold
        // the last frame without decoding or playing the next iteration's audio.
    }

    fn frame_index(&self) -> i32 {
        self.decoder.current_frame_index() as i32
    }

    fn frame_count(&self) -> i32 {
        self.decoder.frame_count() as i32
    }

    fn frame_goto(&mut self, index: i32) {
        if self.has_decode_failed() {
            return;
        }
        self.decoder.seek(index.max(0) as u32);
        if self.decoder.current_frame_index() == 0 && self.decoded_frame != Some(0) {
            self.decoder.video_decoder.clear_reference_frames();
        }
        self.next_frame_ready = Instant::now();
        self.state = PlaybackState::Paused;
        self.frame_decompress();
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
            self.next_frame_ready = Instant::now();
        }
    }

    fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    fn stop(&mut self) {
        if self.has_decode_failed() {
            return;
        }
        self.frame_goto(0);
        if !self.has_decode_failed() {
            self.state = PlaybackState::Stopped;
        }
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
    fn file_extension(&self) -> &str {
        "bik"
    }

    fn open(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        match BinkVideoStream::open(resolved_path) {
            Ok(stream) => Some(Box::new(stream)),
            Err(error) => {
                log::warn!(
                    "Skipping movie {movie_title:?} at {}: {error}",
                    resolved_path.display()
                );
                None
            }
        }
    }
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
) -> Result<Vec<BinkFramePacket>, String> {
    let frame_count = header.frame_count as usize;
    if frame_count == 0 {
        return Err("Bink file declares no video frames".to_owned());
    }
    let table_end = frame_count
        .checked_mul(4)
        .and_then(|length| table_offset.checked_add(length))
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| "Bink frame index is truncated".to_owned())?;
    let offsets = bytes[table_offset..table_end]
        .chunks_exact(4)
        // Bit zero indicates a keyframe, independently of its file offset.
        .map(|chunk| (u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) & !1) as usize)
        .chain(std::iter::once(bytes.len()));

    let mut packets = Vec::with_capacity(frame_count);
    for (index, (offset, end)) in offsets.clone().zip(offsets.skip(1)).enumerate() {
        if offset < table_end || end <= offset || end > bytes.len() {
            return Err(format!("Invalid Bink packet bounds for frame {index}"));
        }
        packets.push(BinkFramePacket {
            index: index as u32,
            offset,
            size: end - offset,
        });
    }
    Ok(packets)
}

fn bytes_per_pixel(format: VideoBufferType) -> usize {
    match format {
        VideoBufferType::R8G8B8 => 3,
        VideoBufferType::X8R8G8B8 => 4,
        VideoBufferType::R5G6B5 | VideoBufferType::X1R5G5B5 => 2,
        VideoBufferType::Unknown => 0,
    }
}

fn write_pixel(dst: &mut [u8], format: VideoBufferType, rgba: &[u8]) {
    let r = rgba[0];
    let g = rgba[1];
    let b = rgba[2];
    let a = rgba[3];
    match format {
        VideoBufferType::X8R8G8B8 => {
            dst.copy_from_slice(&[b, g, r, a]);
        }
        VideoBufferType::R8G8B8 => {
            dst.copy_from_slice(&[r, g, b]);
        }
        VideoBufferType::R5G6B5 => {
            let packed = (((r as u16 >> 3) & 0x1F) << 11)
                | (((g as u16 >> 2) & 0x3F) << 5)
                | ((b as u16 >> 3) & 0x1F);
            dst.copy_from_slice(&packed.to_le_bytes());
        }
        VideoBufferType::X1R5G5B5 => {
            let alpha_bit = if a >= 128 { 1u16 } else { 0u16 };
            let packed = (alpha_bit << 15)
                | (((r as u16 >> 3) & 0x1F) << 10)
                | (((g as u16 >> 3) & 0x1F) << 5)
                | ((b as u16 >> 3) & 0x1F);
            dst.copy_from_slice(&packed.to_le_bytes());
        }
        VideoBufferType::Unknown => {}
    }
}

#[cfg(test)]
#[path = "../tests/support/bink_fixture.rs"]
pub(crate) mod bink_fixture;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bink_blob(magic: [u8; 4]) -> Vec<u8> {
        let mut bytes = bink_fixture::two_frames();
        bytes[..4].copy_from_slice(&magic);
        bytes
    }

    #[test]
    fn packet_boundaries_exclude_keyframe_flag() {
        let mut bytes = make_bink_blob(*b"BIKi");
        bytes[44] |= 1;
        bytes[48] |= 1;
        let decoder = BinkDecoder::from_bytes(bytes).unwrap();
        let first = &decoder.frame_packets[0];
        let second = &decoder.frame_packets[1];
        let packet_size = bink_fixture::frame(96).len();
        assert_eq!((first.offset, first.size), (52, packet_size));
        assert_eq!((second.offset, second.size), (52 + packet_size, packet_size));
    }

    #[test]
    fn independently_initialized_players_open_movies_after_peer_reset_and_drop() {
        use crate::video_player::{Video, VideoPlayer, VideoPlayerInterface, VideoSearchPaths};

        crate::test_support::ensure_ambient_host();
        let root = std::env::temp_dir().join(format!("bink_two_players_{}", std::process::id()));
        let mod_root = root.join("mod");
        let path = mod_root.join("Data/Movies/CLIP.BIK");
        fs::create_dir_all(path.parent().unwrap()).expect("create mod movie path");
        fs::write(&path, make_bink_blob(*b"BIKi")).expect("write Bink fixture");
        let paths = VideoSearchPaths::new(Some(mod_root), root.join("install"), "English");
        for reverse in [false, true] {
            let mut first = VideoPlayer::new();
            let mut second = VideoPlayer::new();
            for player in [&mut first, &mut second] { player.set_search_paths(paths.clone()); }
            if reverse { second.init(); first.init(); } else { first.init(); second.init(); }
            first.add_video(Video::new("CLIP".into(), "FixtureMovie".into(), String::new()));
            second.add_video(Video::new("CLIP".into(), "FixtureMovie".into(), String::new()));
            let mut first_stream = first.open("FixtureMovie".into()).expect("first player opens Bink");
            let mut second_stream = second.open("FixtureMovie".into()).expect("second player opens Bink");
            first_stream.play();
            second_stream.play();
            first.reset();
            drop(first);
            second_stream.frame_next();
            assert_eq!(second_stream.frame_index(), 1);
            let _peer_stream = second.open("FixtureMovie".into()).expect("peer still opens movies");
        }
        fs::remove_dir_all(root).expect("remove Bink fixture root");
    }

    #[test]
    fn frame_render_converts_pixels_and_preserves_row_padding() {
        use crate::video_buffer::SoftwareVideoBuffer;

        let decoder = BinkDecoder::from_bytes(make_bink_blob(*b"BIKi")).unwrap();
        let mut stream = BinkVideoStream {
            decoder,
            current_rgba: (0..16)
                .flat_map(|i| [255, 128, 0, if i % 2 == 0 { 127 } else { 128 }])
                .collect(),
            decoded_frame: Some(0),
            next_frame_ready: Instant::now(),
            state: PlaybackState::Playing,
            volume: 1.0,
        };
        for (format, even, odd) in [
            (VideoBufferType::R8G8B8, &[255, 128, 0][..], &[255, 128, 0][..]),
            (VideoBufferType::X8R8G8B8, &[0, 128, 255, 127][..], &[0, 128, 255, 128][..]),
            (VideoBufferType::R5G6B5, &[0x00, 0xfc][..], &[0x00, 0xfc][..]),
            (VideoBufferType::X1R5G5B5, &[0x00, 0x7e][..], &[0x00, 0xfe][..]),
        ] {
            let mut buffer = SoftwareVideoBuffer::new(format);
            assert!(buffer.allocate(3, 2));
            let pitch = buffer.pitch() as usize;
            buffer.lock().unwrap().fill(0x55);
            buffer.unlock();
            stream.frame_render(&mut buffer);
            let pixels = buffer.lock().unwrap();
            let bpp = even.len();
            for row in pixels.chunks_exact(pitch).take(2) {
                assert_eq!(&row[..bpp], even);
                assert_eq!(&row[bpp..2 * bpp], odd);
                assert_eq!(&row[2 * bpp..3 * bpp], even);
                assert!(row[3 * bpp..].iter().all(|&byte| byte == 0x55));
            }
            buffer.unlock();
        }
    }

    #[test]
    fn parses_bink1_header() {
        crate::test_support::ensure_ambient_host();
        let bytes = make_bink_blob(*b"BIKi");
        let header = BinkHeader::parse(&bytes).expect("header should parse");
        assert_eq!(header.version, BinkVersion::Bink1);
        assert_eq!(header.frame_count, 2);
        assert_eq!(header.width, 4);
        assert_eq!(header.height, 4);
        assert_eq!(header.audio_track_count, 0);
    }

    #[test]
    fn rejects_unsupported_bink2_and_invalid_frame_index() {
        let bytes = make_bink_blob(*b"KB2i");
        assert_eq!(BinkHeader::parse(&bytes).unwrap().version, BinkVersion::Bink2);
        let error = BinkDecoder::from_bytes(bytes).err().expect("reject Bink2");
        assert!(error.contains("Bink2 video decoding is unsupported"), "{error}");

        let mut bytes = bink_fixture::two_frames();
        bytes[48..52].copy_from_slice(&u32::MAX.to_le_bytes());
        let error = BinkDecoder::from_bytes(bytes).err().expect("reject packet bounds");
        assert!(error.contains("Invalid Bink packet bounds for frame 0"), "{error}");
    }

    #[test]
    fn real_binki_decodes_exact_pixels_and_reports_read_errors() {
        let mut decoder = BinkDecoder::from_bytes(bink_fixture::two_frames()).unwrap();
        for (index, color) in [(0, 96), (1, 110)] {
            let rgba = decoder.decode_frame_rgba(index).unwrap();
            assert_eq!(rgba, [color, color, color, 255].repeat(16));
        }

        let bytes = bink_fixture::clip(&[vec![0; 4]]);
        let mut decoder = BinkDecoder::from_bytes(bytes).unwrap();
        let error = decoder.decode_current_frame_rgba().unwrap_err();
        assert!(error.contains("Bink frame 0"), "{error}");
    }

    #[test]
    fn video_stream_stops_on_bad_packet_without_rendering_a_replacement() {
        use crate::video_buffer::SoftwareVideoBuffer;

        let path = std::env::temp_dir().join(format!("bink_bad_packet_{}.bik", std::process::id()));
        fs::write(&path, bink_fixture::clip(&[bink_fixture::frame(96), vec![0; 4]])).unwrap();
        let mut stream = BinkVideoStream::open(&path).unwrap();
        fs::remove_file(path).unwrap();
        stream.play();
        stream.frame_next();
        stream.frame_decompress();
        assert!(stream.has_decode_failed());
        assert_eq!(stream.playback_state(), PlaybackState::Complete);
        assert!(!stream.is_frame_ready());

        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(4, 4));
        buffer.lock().unwrap().fill(0x55);
        buffer.unlock();
        stream.frame_render(&mut buffer);
        assert!(buffer.lock().unwrap().iter().all(|&byte| byte == 0x55));
        buffer.unlock();
    }

    #[test]
    fn video_stream_tracks_playback_state() {
        crate::test_support::ensure_ambient_host();
        let path = std::env::temp_dir().join(format!("bink_stream_{}.bik", std::process::id()));
        fs::write(&path, make_bink_blob(*b"BIKi")).expect("write temp bink");
        let mut stream = BinkVideoStream::open(&path).expect("open");
        stream.play();
        stream.frame_next();
        assert_eq!(stream.frame_index(), 1);
        assert_eq!(stream.playback_state(), PlaybackState::Playing);

        stream.frame_decompress();
        assert_eq!(stream.current_rgba, [110, 110, 110, 255].repeat(16));
        stream.frame_next();
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.playback_state(), PlaybackState::Playing);
        assert_eq!(stream.current_rgba, [110, 110, 110, 255].repeat(16));
        stream.frame_decompress();
        assert_eq!(stream.current_rgba, [96, 96, 96, 255].repeat(16));

        stream.stop();
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.playback_state(), PlaybackState::Stopped);

        fs::remove_file(path).ok();
    }

    #[test]
    fn bink_has_audio_bitstream_parser_wired_to_miles() {
        crate::test_support::ensure_ambient_host();
        let bytes = make_bink_blob(*b"BIKi");
        let header = BinkHeader::parse(&bytes).expect("header");
        assert_eq!(header.audio_track_count, 0);
        assert!(
            has_bink_audio_bitstream_parser(),
            "Bink audio bitstream parser must be present for Miles/kira soundtrack"
        );
        let mut audio = BinkAudioState::default();
        audio.notify_video_player_of_new_provider(false);
        assert!(
            !audio.soundtrack_is_bound(),
            "lost audio device mutes soundtrack but parser stays available"
        );
    }

    #[test]
    fn bink_speech_slider_matches_create_stream_formula() {
        crate::test_support::ensure_ambient_host();
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
