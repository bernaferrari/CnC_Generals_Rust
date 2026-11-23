//! Video player interface and implementation.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use crate::bink::BuiltInBinkVideoProvider;
use crate::video_stream::{PlaybackState, VideoStream, VideoStreamInterface};
use game_engine::common::audio::{AudioManager, game_audio::MoviePcmVoice};
use log::warn;

type Bool = bool;
type Int = i32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    pub filename: String,
    pub internal_name: String,
    pub comment_for_wb: String,
}

impl Video {
    pub fn new(filename: String, internal_name: String, comment_for_wb: String) -> Self {
        Video {
            filename,
            internal_name,
            comment_for_wb,
        }
    }

    pub fn empty() -> Self {
        Video {
            filename: String::new(),
            internal_name: String::new(),
            comment_for_wb: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldParse {
    pub field_name: &'static str,
    pub parse_type: &'static str,
    pub data: usize,
    pub offset: usize,
}

pub const VIDEO_FIELD_PARSE_TABLE: &[FieldParse] = &[
    FieldParse {
        field_name: "Filename",
        parse_type: "AsciiString",
        data: 0,
        offset: 0,
    },
    FieldParse {
        field_name: "Comment",
        parse_type: "AsciiString",
        data: 0,
        offset: 0,
    },
];

pub trait VideoPlayerInterface {
    fn init(&mut self);
    fn reset(&mut self);
    fn update(&mut self);
    fn deinit(&mut self);
    fn lose_focus(&mut self);
    fn regain_focus(&mut self);
    fn open(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>>;
    fn load(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>>;
    fn first_stream(&self) -> Option<&dyn VideoStreamInterface>;
    fn first_stream_mut(&mut self) -> Option<&mut dyn VideoStreamInterface>;
    fn close_all_streams(&mut self);
    fn add_video(&mut self, video: Video);
    fn remove_video(&mut self, internal_name: &str);
    fn get_num_videos(&self) -> Int;
    fn get_video_by_name(&self, movie_title: &str) -> Option<&Video>;
    fn get_video_by_index(&self, index: Int) -> Option<&Video>;
    fn get_field_parse(&self) -> &'static [FieldParse] {
        VIDEO_FIELD_PARSE_TABLE
    }
    fn notify_video_player_of_new_provider(&mut self, now_has_valid: Bool);
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn set_volume(&mut self, _volume: f32) {}
    fn playback_state(&self) -> PlaybackState {
        PlaybackState::Stopped
    }
}

pub trait VideoStreamProvider: Send + Sync {
    /// File extension used for an authored Video.ini filename.
    fn file_extension(&self) -> &str;

    fn open(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>>;

    fn open_with_audio(
        &self,
        movie_title: &str,
        resolved_path: &Path,
        _audio: Option<&Mutex<AudioManager>>,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        self.open(movie_title, resolved_path)
    }

    fn load(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        self.open(movie_title, resolved_path)
    }
}

pub type VideoPlayerHandle = Arc<Mutex<Option<VideoPlayer>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSearchPaths {
    pub mod_dir: Option<PathBuf>,
    pub game_root: PathBuf,
    pub language: String,
}

impl VideoSearchPaths {
    pub fn new(mod_dir: Option<PathBuf>, game_root: PathBuf, language: impl Into<String>) -> Self {
        Self {
            mod_dir,
            game_root,
            language: language.into(),
        }
    }

    fn movie_candidates<'a>(
        &'a self,
        filename: &'a str,
        extension: &'a str,
    ) -> impl Iterator<Item = PathBuf> + 'a {
        let language = self.language.trim();
        let game_root = self.game_root.as_path();
        let mod_root = self.mod_dir.as_deref().filter(|path| !path.as_os_str().is_empty());
        [
            mod_root.map(|root| (root, None)),
            (!language.is_empty()).then_some((game_root, Some(language))),
            Some((game_root, None)),
        ]
        .into_iter()
        .flatten()
        .filter_map(move |(root, language)| {
            let dir = match language {
                Some(language) => resolve_case_insensitive_dir(root, &["Data", language, "Movies"]),
                None => resolve_case_insensitive_dir(root, &["Data", "Movies"]),
            }?;
            find_movie_file_in_dir(&dir, filename, extension)
        })
    }
}

/// The two exact virtual Video.ini sources consumed by C++ VideoPlayer::init.
/// Main's FileSystemSubsystem and stand-alone GameClient both feed this existing
/// Common FileSystem owner; each virtual path selects one local/archive source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoIniCatalogSources {
    pub default_catalog: Option<Vec<u8>>,
    pub overlay_catalog: Option<Vec<u8>>,
}

impl VideoIniCatalogSources {
    /// Read one selected file for each of the original catalog layers.
    pub fn read_from(mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> Self {
        Self {
            default_catalog: read("Data/INI/Default/Video.ini"),
            overlay_catalog: read("Data/INI/Video.ini"),
        }
    }

    pub(crate) fn from_game_file_system() -> Self {
        Self::read_from(crate::asset_file_system::read_game_fs_bytes)
    }
}

pub struct VideoPlayer {
    videos_available_for_play: Vec<Video>,
    first_stream: Option<Box<VideoStream>>,
    active_streams: Vec<Arc<Mutex<ManagedVideoStreamState>>>,
    movie_audio_device: Option<Weak<Mutex<AudioManager>>>,
    provider: Option<Box<dyn VideoStreamProvider>>,
    provider_is_valid: Bool,
    has_focus: Bool,
    search_paths: Option<VideoSearchPaths>,
}

impl VideoPlayer {
    pub fn new() -> Self {
        VideoPlayer {
            videos_available_for_play: Vec::new(),
            first_stream: None,
            active_streams: Vec::new(),
            movie_audio_device: None,
            provider: None,
            provider_is_valid: false,
            has_focus: true,
            search_paths: None,
        }
    }

    /// Replace this player's provider without affecting any other player.
    pub fn register_video_stream_provider(&mut self, provider: Box<dyn VideoStreamProvider>) {
        self.provider = Some(provider);
        self.notify_video_player_of_new_provider(true);
    }

    pub fn clear_video_stream_provider(&mut self) {
        self.provider = None;
        self.notify_video_player_of_new_provider(false);
    }

    pub fn remove(&mut self, stream_to_remove: *const VideoStream) {
        if self.first_stream.is_none() {
            return;
        }

        if let Some(ref first) = self.first_stream {
            let first_ptr = first.as_ref() as *const VideoStream;
            if first_ptr == stream_to_remove {
                let mut old_first = self.first_stream.take().unwrap();
                self.first_stream = old_first.take_next();
                return;
            }
        }

        let mut current = self.first_stream.as_deref_mut();
        while let Some(stream) = current {
            if let Some(next) = stream.get_next() {
                let next_ptr = next as *const VideoStream;
                if next_ptr == stream_to_remove {
                    if let Some(mut removed) = stream.take_next() {
                        stream.set_next(removed.take_next());
                    }
                    return;
                }
            }
            current = stream.get_next_mut();
        }
    }

    fn load_video_catalog_sources(&mut self, sources: &VideoIniCatalogSources) {
        for bytes in [&sources.default_catalog, &sources.overlay_catalog].into_iter().flatten() {
            if !looks_like_video_ini_bytes(bytes) { continue; }
            let Ok(contents) = std::str::from_utf8(bytes) else { continue; };
            for video in parse_video_ini_contents(contents) {
                self.add_video(video);
            }
        }
    }

    pub(crate) fn init_with_video_ini_sources(&mut self, sources: &VideoIniCatalogSources) {
        self.videos_available_for_play.clear();
        self.load_video_catalog_sources(sources);
        if self.provider.is_none() {
            self.register_video_stream_provider(Box::new(BuiltInBinkVideoProvider));
        }
        self.provider_is_valid = self.provider.is_some();
        self.has_focus = true;
    }

    pub fn set_search_paths(&mut self, paths: VideoSearchPaths) {
        self.search_paths = Some(paths);
    }

    pub(crate) fn bind_movie_audio_device(&mut self, device: &Arc<Mutex<AudioManager>>) {
        self.movie_audio_device = Some(Arc::downgrade(device));
    }

    fn track_stream(
        &mut self,
        inner: Box<dyn VideoStreamInterface>,
    ) -> Box<dyn VideoStreamInterface> {
        self.active_streams
            .retain(|stream| stream.lock().is_ok_and(|state| !state.is_closed()));
        let state = Arc::new(Mutex::new(ManagedVideoStreamState::new(inner)));
        self.active_streams.push(Arc::clone(&state));
        Box::new(ManagedVideoStreamHandle::new(state))
    }

    fn update_tracked_streams(&mut self) {
        self.active_streams.retain(|stream| {
            stream.lock().is_ok_and(|mut state| {
                state.update();
                !state.is_closed()
            })
        });
    }

    fn close_tracked_streams(&mut self) {
        for stream in self.active_streams.drain(..) {
            let mut state = stream.lock().unwrap_or_else(|error| error.into_inner());
            state.cancel();
        }
    }

    fn provider_is_valid(&self) -> Bool {
        self.provider_is_valid
    }
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoPlayerInterface for VideoPlayer {
    fn init(&mut self) {
        let sources = VideoIniCatalogSources::from_game_file_system();
        self.init_with_video_ini_sources(&sources);
    }

    fn reset(&mut self) {
        self.close_all_streams();
    }

    fn update(&mut self) {
        if !self.has_focus {
            return;
        }
        let mut current = self.first_stream.as_deref_mut();
        while let Some(stream) = current {
            stream.update();
            current = stream.get_next_mut();
        }
        self.update_tracked_streams();
    }

    fn deinit(&mut self) {
        self.close_all_streams();
        self.provider_is_valid = false;
    }

    fn lose_focus(&mut self) {
        self.has_focus = false;
    }

    fn regain_focus(&mut self) {
        self.has_focus = true;
    }

    fn open(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        let Some(video) = self.get_video_by_name(&movie_title) else {
            warn!("cutscene skipped (unknown registered video title): {movie_title}");
            return None;
        };
        let provider = self.provider.as_ref()?;
        let audio = self.movie_audio_device.as_ref().and_then(Weak::upgrade);
        let stream = self
            .search_paths
            .as_ref()?
            .movie_candidates(&video.filename, provider.file_extension())
            .find_map(|path| provider.open_with_audio(&movie_title, &path, audio.as_deref()))?;
        Some(self.track_stream(stream))
    }

    fn load(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        self.open(movie_title)
    }

    fn first_stream(&self) -> Option<&dyn VideoStreamInterface> {
        self.first_stream
            .as_deref()
            .map(|s| s as &dyn VideoStreamInterface)
    }

    fn first_stream_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        self.first_stream
            .as_deref_mut()
            .map(|s| s as &mut dyn VideoStreamInterface)
    }

    fn close_all_streams(&mut self) {
        while let Some(mut stream) = self.first_stream.take() {
            self.first_stream = stream.take_next();
        }
        self.close_tracked_streams();
    }

    fn add_video(&mut self, video: Video) {
        for existing in &mut self.videos_available_for_play {
            if existing.internal_name == video.internal_name {
                *existing = video;
                return;
            }
        }
        self.videos_available_for_play.push(video);
    }

    fn remove_video(&mut self, internal_name: &str) {
        self.videos_available_for_play
            .retain(|v| v.internal_name != internal_name);
    }

    fn get_num_videos(&self) -> Int {
        self.videos_available_for_play.len() as Int
    }

    fn get_video_by_name(&self, movie_title: &str) -> Option<&Video> {
        self.videos_available_for_play
            .iter()
            .find(|v| v.internal_name == movie_title)
    }

    fn get_video_by_index(&self, index: Int) -> Option<&Video> {
        if index < 0 || index >= self.videos_available_for_play.len() as Int {
            return None;
        }
        Some(&self.videos_available_for_play[index as usize])
    }

    fn notify_video_player_of_new_provider(&mut self, now_has_valid: Bool) {
        if !now_has_valid {
            self.close_tracked_streams();
        }
        self.provider_is_valid = now_has_valid;
    }

    fn play(&mut self) {
        if let Some(stream) = self.first_stream.as_deref_mut() {
            stream.play();
        }
        self.update_tracked_streams();
    }

    fn pause(&mut self) {
        if let Some(stream) = self.first_stream.as_deref_mut() {
            stream.pause();
        }
    }

    fn stop(&mut self) {
        if let Some(stream) = self.first_stream.as_deref_mut() {
            stream.stop();
        }
        self.update_tracked_streams();
    }

    fn set_volume(&mut self, volume: f32) {
        if let Some(stream) = self.first_stream.as_deref_mut() {
            stream.set_volume(volume);
        }
        self.update_tracked_streams();
    }

    fn playback_state(&self) -> PlaybackState {
        self.first_stream
            .as_deref()
            .map(|s| s.playback_state())
            .unwrap_or(PlaybackState::Stopped)
    }
}

#[cfg(test)]
thread_local! {
    static TEST_VIDEO_PLAYER: std::cell::RefCell<Option<Arc<Mutex<Option<VideoPlayer>>>>> =
        const { std::cell::RefCell::new(None) };
}

fn video_player_handle() -> Option<Arc<Mutex<Option<VideoPlayer>>>> {
    #[cfg(test)]
    {
        return Some(TEST_VIDEO_PLAYER.with(|slot| {
            slot.borrow_mut()
                .get_or_insert_with(|| Arc::new(Mutex::new(None)))
                .clone()
        }));
    }
    #[cfg(not(test))]
    {
        None
    }
}


pub fn init_video_player_slot(
    slot: &VideoPlayerHandle,
    paths: &VideoSearchPaths,
    catalogs: &VideoIniCatalogSources,
) {
    if let Ok(mut guard) = slot.lock() {
        if guard.is_none() {
            let mut player = VideoPlayer::new();
            player.set_search_paths(paths.clone());
            player.init_with_video_ini_sources(catalogs);
            *guard = Some(player);
        } else if let Some(player) = guard.as_mut() {
            player.set_search_paths(paths.clone());
        }
    }
}

pub(crate) fn bind_video_slot_movie_audio_device(
    slot: &VideoPlayerHandle,
    device: &Arc<Mutex<AudioManager>>,
) {
    if let Ok(mut guard) = slot.lock() {
        if let Some(player) = guard.as_mut() {
            player.bind_movie_audio_device(device);
        }
    }
}

pub fn init_video_player() {
    let Some(player) = video_player_handle() else { return };
    if let Ok(mut guard) = player.lock() {
        if guard.is_none() {
            let player = VideoPlayer::new();
            *guard = Some(player);
        } else if let Some(player) = guard.as_mut() {
            player.provider_is_valid = player.provider.is_some();
        }
    }
}

pub fn get_video_player() -> Option<Arc<Mutex<Option<VideoPlayer>>>> {
    #[cfg(test)]
    {
        return TEST_VIDEO_PLAYER.with(|slot| slot.borrow().clone());
    }
    #[cfg(not(test))]
    {
        None
    }
}


pub fn shutdown_video_player() {
    if let Some(player) = get_video_player() {
        let mut guard = player.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(player) = guard.as_mut() {
            player.deinit();
        }
    }
}
impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.deinit();
    }
}


struct ManagedVideoStreamState {
    inner: Option<Box<dyn VideoStreamInterface>>,
    audio_tail: Option<MoviePcmVoice>,
}

impl ManagedVideoStreamState {
    fn new(inner: Box<dyn VideoStreamInterface>) -> Self {
        Self {
            inner: Some(inner),
            audio_tail: None,
        }
    }

    fn update(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.update();
        } else if self.audio_tail.as_ref().is_some_and(|tail| !tail.has_pending_samples()) {
            self.audio_tail = None;
        }
    }

    fn finish_picture(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            self.audio_tail = inner.take_audio_tail();
            inner.close();
        }
    }

    fn cancel(&mut self) {
        self.audio_tail = None;
        self.close_inner();
    }

    fn close_inner(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.close();
        }
    }

    fn is_closed(&self) -> bool {
        self.inner.is_none() && self.audio_tail.is_none()
    }
}

struct ManagedVideoStreamHandle {
    state: Arc<Mutex<ManagedVideoStreamState>>,
}

impl ManagedVideoStreamHandle {
    fn new(state: Arc<Mutex<ManagedVideoStreamState>>) -> Self {
        Self { state }
    }
}

impl Drop for ManagedVideoStreamHandle {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.close_inner();
        }
    }
}

impl VideoStreamInterface for ManagedVideoStreamHandle {
    fn next(&self) -> Option<&dyn VideoStreamInterface> {
        None
    }

    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        None
    }

    fn update(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.update();
        }
    }

    fn close(self: Box<Self>) {
        if let Ok(mut state) = self.state.lock() {
            state.cancel();
        }
    }

    fn finish(self: Box<Self>) {
        if let Ok(mut state) = self.state.lock() {
            state.finish_picture();
        }
    }

    fn cancel_audio(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.cancel_audio();
            }
        }
    }

    fn is_frame_ready(&self) -> Bool {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.is_frame_ready()))
            .unwrap_or(false)
    }

    fn has_decode_failed(&self) -> bool {
        self.state.lock().ok().is_some_and(|state| {
            state.inner.as_ref().is_some_and(|inner| inner.has_decode_failed())
        })
    }

    fn frame_decompress(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.frame_decompress();
            }
        }
    }

    fn frame_render(&mut self, buffer: &mut dyn crate::video_buffer::VideoBuffer) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.frame_render(buffer);
            }
        }
    }

    fn frame_next(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.frame_next();
            }
        }
    }

    fn frame_index(&self) -> Int {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.frame_index()))
            .unwrap_or(0)
    }

    fn frame_count(&self) -> Int {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.frame_count()))
            .unwrap_or(1)
    }

    fn frame_goto(&mut self, index: Int) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.frame_goto(index);
            }
        }
    }

    fn height(&self) -> Int {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.height()))
            .unwrap_or(0)
    }

    fn width(&self) -> Int {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.width()))
            .unwrap_or(0)
    }

    fn play(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.play();
            }
        }
    }

    fn pause(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.pause();
            }
        }
    }

    fn stop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.stop();
            }
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(inner) = state.inner.as_mut() {
                inner.set_volume(volume);
            }
        }
    }

    fn playback_state(&self) -> PlaybackState {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.playback_state()))
            .unwrap_or(PlaybackState::Stopped)
    }
}


fn resolve_case_insensitive_dir(root: &Path, components: &[&str]) -> Option<PathBuf> {
    let mut current = if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() };
    for component in components {
        let requested = component.trim();
        let exact = current.join(requested);
        if exact.is_dir() {
            current = exact;
            continue;
        }
        current = fs::read_dir(&current)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(requested))
                    && path.is_dir()
            })
            .min()?;
    }
    Some(current)
}

fn find_movie_file_in_dir(dir: &Path, movie_filename: &str, extension: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    let target_name = Path::new(movie_filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(movie_filename.trim());
    let target_stem = Path::new(target_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(target_name)
        .trim();

    for entry in fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !file_extension.eq_ignore_ascii_case(extension) {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let file_name_matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case(target_name))
            .unwrap_or(false);
        if stem.eq_ignore_ascii_case(target_stem) || file_name_matches {
            return Some(path);
        }
    }

    None
}

fn looks_like_video_ini_bytes(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    text.lines().any(|line| {
        let trimmed = line.split(';').next().unwrap_or("").trim();
        trimmed
            .get(..6)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("video "))
    })
}

fn parse_video_ini_contents(contents: &str) -> Vec<Video> {
    let mut parsed = Vec::new();
    let mut current: Option<Video> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if let Some(name) = line.strip_prefix("Video ") {
            if let Some(video) = current.take() {
                parsed.push(video);
            }

            let internal_name = name.trim();
            if internal_name.is_empty() {
                continue;
            }

            current = Some(Video::new(
                String::new(),
                internal_name.to_string(),
                String::new(),
            ));
            continue;
        }

        if line.eq_ignore_ascii_case("End") {
            if let Some(video) = current.take() {
                parsed.push(video);
            }
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let Some(video) = current.as_mut() else {
            continue;
        };

        let key = raw_key.trim();
        let value = normalize_ini_value(raw_value);
        match key {
            "Filename" => video.filename = value,
            "Comment" => video.comment_for_wb = value,
            _ => {}
        }
    }

    if let Some(video) = current.take() {
        parsed.push(video);
    }

    parsed
}

fn normalize_ini_value(raw_value: &str) -> String {
    let without_comment = strip_ini_comment(raw_value).trim();
    let without_quotes = without_comment
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(without_comment);
    without_quotes.trim().to_string()
}

fn strip_ini_comment(value: &str) -> &str {
    let mut in_quotes = false;

    for (index, ch) in value.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => return &value[..index],
            _ => {}
        }
    }

    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_buffer::{SoftwareVideoBuffer, VideoBuffer as _, VideoBufferType};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockProvider {
        updates: Arc<AtomicUsize>,
        closes: Arc<AtomicUsize>,
    }

    impl VideoStreamProvider for MockProvider {
        fn file_extension(&self) -> &str {
            "bik"
        }

        fn open(
            &self,
            _movie_title: &str,
            _resolved_path: &Path,
        ) -> Option<Box<dyn VideoStreamInterface>> {
            Some(Box::new(MockStream::new(
                Arc::clone(&self.updates),
                Arc::clone(&self.closes),
            )))
        }
    }

    struct OrderedProvider {
        attempts: Arc<Mutex<Vec<PathBuf>>>,
        reject_before: usize,
        updates: Arc<AtomicUsize>,
        closes: Arc<AtomicUsize>,
    }
    impl VideoStreamProvider for OrderedProvider {
        fn file_extension(&self) -> &str {
            "testvideo"
        }

        fn open(&self, _title: &str, path: &Path) -> Option<Box<dyn VideoStreamInterface>> {
            let mut attempts = self.attempts.lock().unwrap();
            attempts.push(path.to_path_buf());
            if attempts.len() <= self.reject_before { return None; }
            Some(Box::new(MockStream::new(self.updates.clone(), self.closes.clone())))
        }
    }

    struct MockStream {
        updates: Arc<AtomicUsize>,
        closes: Arc<AtomicUsize>,
    }

    impl MockStream {
        fn new(updates: Arc<AtomicUsize>, closes: Arc<AtomicUsize>) -> Self {
            Self { updates, closes }
        }
    }

    impl Drop for MockStream {
        fn drop(&mut self) {
            self.closes.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl VideoStreamInterface for MockStream {
        fn next(&self) -> Option<&dyn VideoStreamInterface> {
            None
        }

        fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
            None
        }

        fn update(&mut self) {
            self.updates.fetch_add(1, Ordering::SeqCst);
        }

        fn close(self: Box<Self>) {
            drop(self);
        }

        fn is_frame_ready(&self) -> Bool {
            true
        }

        fn frame_decompress(&mut self) {}

        fn frame_render(&mut self, _buffer: &mut dyn crate::video_buffer::VideoBuffer) {}

        fn frame_next(&mut self) {}

        fn frame_index(&self) -> Int {
            0
        }

        fn frame_count(&self) -> Int {
            1
        }

        fn frame_goto(&mut self, _index: Int) {}

        fn height(&self) -> Int {
            16
        }

        fn width(&self) -> Int {
            16
        }
    }

    #[test]
    fn parse_video_ini_contents_extracts_filename_and_comment() {
        crate::test_support::ensure_ambient_host();
        let videos = parse_video_ini_contents(
            r#"
                Video EALogoMovie
                  Filename = EA_LOGO  ; comment
                  Comment = "This is the EA logo screen"
                End
            "#,
        );

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].internal_name, "EALogoMovie");
        assert_eq!(videos[0].filename, "EA_LOGO");
        assert_eq!(videos[0].comment_for_wb, "This is the EA logo screen");
    }

    #[test]
    fn stock_bink_provider_decodes_mounted_retail_movie_when_requested() {
        crate::test_support::ensure_ambient_host();
        let Some(root) = std::env::var_os("GENERALS_RETAIL_VIDEO_ROOT").map(PathBuf::from) else {
            return;
        };
        let movie_dir = resolve_case_insensitive_dir(&root, &["Data", "English", "Movies"])
            .expect("GENERALS_RETAIL_VIDEO_ROOT must contain Data/English/Movies");
        let path = find_movie_file_in_dir(&movie_dir, "EA_LOGO.BIK", "bik")
            .expect("GENERALS_RETAIL_VIDEO_ROOT requires mounted EA_LOGO.BIK");
        let mut player = VideoPlayer::new();
        player.set_search_paths(VideoSearchPaths::new(None, root, "English"));
        player.init();
        player.add_video(Video::new("EA_LOGO".into(), "EALogoMovie".into(), String::new()));
        let mut stream = player.open("EALogoMovie".to_string())
            .expect("registered stock title should open through Bink provider");
        assert!(stream.width() > 0 && stream.height() > 0 && stream.frame_count() > 0);
        stream.frame_decompress();
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(stream.width() as u32, stream.height() as u32));
        buffer.lock().unwrap().fill(0x55);
        buffer.unlock();
        stream.frame_render(&mut buffer);
        assert_ne!(&buffer.lock().unwrap()[..4], &[0x55; 4]);
        buffer.unlock();
        let first_frame = stream.frame_index();
        stream.frame_next();
        assert_eq!(stream.frame_index(), first_frame + 1);
        assert!(path.is_file());
    }

    #[test]
    fn video_player_init_loads_known_video_registry_entries() {
        crate::test_support::ensure_ambient_host();
        let mut player = VideoPlayer::new();
        player.init();

        assert!(
            player.get_video_by_name("EALogoMovie").is_some()
                || player.get_video_by_name("Sizzle").is_some()
        );
    }

    #[test]
    fn init_video_player_restores_singleton_after_shutdown() {
        crate::test_support::ensure_ambient_host();
        init_video_player();
        shutdown_video_player();
        init_video_player();

        let player = get_video_player().expect("video player singleton should exist");
        let guard = player.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.is_some());
    }


    #[test]
    fn player_open_rejects_unregistered_raw_names() {
        crate::test_support::ensure_ambient_host();
        let mut player = VideoPlayer::new();
        player.set_search_paths(VideoSearchPaths::new(None, PathBuf::from("."), "English"));
        player.init();
        assert!(player.open("EA_LOGO.BIK".to_string()).is_none());
    }

    #[test]
    fn two_receiving_video_catalogs_apply_default_then_overlay_independently() {
        crate::test_support::ensure_ambient_host();
        let catalogs = [
            VideoIniCatalogSources {
                default_catalog: Some(b"Video Shared\n Filename = A_Default\nEnd\nVideo DefaultOnly\n Filename = A_Only\nEnd\n".to_vec()),
                overlay_catalog: Some(b"Video Shared\n Filename = A_Overlay\nEnd\n".to_vec()),
            },
            VideoIniCatalogSources {
                default_catalog: Some(b"Video Shared\n Filename = B_Default\nEnd\nVideo DefaultOnly\n Filename = B_Only\nEnd\n".to_vec()),
                overlay_catalog: Some(b"Video Shared\n Filename = B_Overlay\nEnd\n".to_vec()),
            },
        ];
        let mut players = [VideoPlayer::new(), VideoPlayer::new()];
        for (player, sources) in players.iter_mut().zip(&catalogs) {
            player.init_with_video_ini_sources(sources);
        }
        assert_eq!(players[0].get_video_by_name("Shared").unwrap().filename, "A_Overlay");
        assert_eq!(players[0].get_video_by_name("DefaultOnly").unwrap().filename, "A_Only");
        assert_eq!(players[1].get_video_by_name("Shared").unwrap().filename, "B_Overlay");
        assert_eq!(players[1].get_video_by_name("DefaultOnly").unwrap().filename, "B_Only");
        assert_eq!(players[0].get_num_videos(), 2);
        assert_eq!(players[1].get_num_videos(), 2);
    }

    fn movie_fixture(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("generals_video_{}_{}", name, std::process::id()))
            .join("mod/Data/Movies/CLIP.BIK");
        fs::create_dir_all(path.parent().unwrap()).expect("create mod movie directory");
        fs::write(&path, b"fixture for custom provider").expect("write movie fixture");
        path
    }


    #[test]
    fn registered_title_tries_mod_then_language_then_shared_until_provider_accepts() {
        crate::test_support::ensure_ambient_host();
        let root = std::env::temp_dir().join(format!("video-order-{}", std::process::id()));
        let mod_root = root.join("mod");
        let mod_file = mod_root.join("Data/Movies/CLIP.TESTVIDEO");
        let localized = root.join("install/Data/English/Movies/CLIP.TESTVIDEO");
        let shared = root.join("install/Data/Movies/CLIP.TESTVIDEO");
        let different_codec = mod_file.with_extension("BIK");
        for file in [&mod_file, &localized, &shared, &different_codec] {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, b"fixture").unwrap();
        }
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let mut player = VideoPlayer::new();
        player.set_search_paths(VideoSearchPaths::new(Some(mod_root), root.join("install"), "English"));
        player.add_video(Video::new("CLIP".into(), "RegisteredClip".into(), String::new()));
        player.register_video_stream_provider(Box::new(OrderedProvider {
            attempts: attempts.clone(), reject_before: 2,
            updates: Arc::new(AtomicUsize::new(0)), closes: Arc::new(AtomicUsize::new(0)),
        }));
        let stream = player.open("RegisteredClip".into()).expect("shared candidate accepted");
        assert_eq!(*attempts.lock().unwrap(), vec![mod_file, localized, shared]);
        assert!(player.open("CLIP".into()).is_none(), "filename is not a registered title");
        drop(stream);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tracked_provider_streams_are_updated_and_closed_by_player() {
        crate::test_support::ensure_ambient_host();
        let path = movie_fixture("tracked");
        let updates = Arc::new(AtomicUsize::new(0));
        let closes = Arc::new(AtomicUsize::new(0));
        let mut player = VideoPlayer::new();
        let mod_root = path.parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
        player.set_search_paths(VideoSearchPaths::new(Some(mod_root), std::env::temp_dir().join("generals_video_tracked_install"), "English"));
        player.register_video_stream_provider(Box::new(MockProvider {
            updates: Arc::clone(&updates),
            closes: Arc::clone(&closes),
        }));
        player.init();
        player.add_video(Video::new("CLIP".into(), "TrackedClip".into(), String::new()));
        let mut stream = player.open("TrackedClip".to_string()).expect("open custom stream");
        player.update();
        assert_eq!(updates.load(Ordering::SeqCst), 1);
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(16, 16));
        stream.frame_render(&mut buffer);
        player.close_all_streams();
        assert_eq!(closes.load(Ordering::SeqCst), 1);
        fs::remove_dir_all(path.parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap())
            .expect("remove fixture tree");
    }

    #[test]
    fn custom_providers_and_streams_are_isolated_across_players() {
        crate::test_support::ensure_ambient_host();
        let path = movie_fixture("isolated");
        let first_updates = Arc::new(AtomicUsize::new(0));
        let second_updates = Arc::new(AtomicUsize::new(0));
        let first_closes = Arc::new(AtomicUsize::new(0));
        let second_closes = Arc::new(AtomicUsize::new(0));
        let mut first = VideoPlayer::new();
        let mut second = VideoPlayer::new();
        for player in [&mut first, &mut second] {
            let mod_root = path.parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
            player.set_search_paths(VideoSearchPaths::new(Some(mod_root), std::env::temp_dir().join("generals_video_isolated_install"), "English"));
        }
        first.register_video_stream_provider(Box::new(MockProvider {
            updates: Arc::clone(&first_updates), closes: Arc::clone(&first_closes),
        }));
        second.register_video_stream_provider(Box::new(MockProvider {
            updates: Arc::clone(&second_updates), closes: Arc::clone(&second_closes),
        }));
        second.init();
        first.init();
        first.add_video(Video::new("CLIP".into(), "FirstClip".into(), String::new()));
        second.add_video(Video::new("CLIP".into(), "SecondClip".into(), String::new()));
        let _first_stream = first.open("FirstClip".into()).expect("first provider opens");
        let _second_stream = second.open("SecondClip".into()).expect("second provider opens");
        first.update();
        assert_eq!(first_updates.load(Ordering::SeqCst), 1);
        assert_eq!(second_updates.load(Ordering::SeqCst), 0);
        first.reset();
        first.clear_video_stream_provider();
        drop(first);
        assert_eq!(first_closes.load(Ordering::SeqCst), 1);
        second.update();
        assert_eq!(second_updates.load(Ordering::SeqCst), 1);
        assert!(second.open("SecondClip".into()).is_some());
        second.deinit();
        assert!(second_closes.load(Ordering::SeqCst) >= 1);
        fs::remove_dir_all(path.parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap())
            .expect("remove fixture tree");
    }

    #[test]
    fn provider_registration_and_reinitialization_are_player_owned() {
        crate::test_support::ensure_ambient_host();
        let mut player = VideoPlayer::new();
        assert!(!player.provider_is_valid());
        player.register_video_stream_provider(Box::new(MockProvider {
            updates: Arc::new(AtomicUsize::new(0)),
            closes: Arc::new(AtomicUsize::new(0)),
        }));
        assert!(player.provider_is_valid());
        player.deinit();
        assert!(!player.provider_is_valid());
        player.init();
        assert!(player.provider_is_valid());
        player.clear_video_stream_provider();
        assert!(!player.provider_is_valid());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "video_movie_pcm_tests.rs"]
mod movie_pcm_tests;
