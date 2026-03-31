//! Video player interface and implementation.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::video_stream::{VideoStream, VideoStreamInterface};
use game_engine::common::ini::get_global_data;
use game_engine::common::ini::ini_webpage_url::get_registry_language;
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
}

pub trait VideoStreamProvider: Send + Sync {
    fn open(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>>;

    fn load(
        &self,
        movie_title: &str,
        resolved_path: &Path,
    ) -> Option<Box<dyn VideoStreamInterface>> {
        self.open(movie_title, resolved_path)
    }
}

pub struct VideoPlayer {
    videos_available_for_play: Vec<Video>,
    first_stream: Option<Box<VideoStream>>,
    active_streams: Vec<Weak<Mutex<ManagedVideoStreamState>>>,
    provider_is_valid: Bool,
}

impl VideoPlayer {
    pub fn new() -> Self {
        VideoPlayer {
            videos_available_for_play: Vec::new(),
            first_stream: None,
            active_streams: Vec::new(),
            provider_is_valid: false,
        }
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

    fn load_known_video_registry_files(&mut self) {
        for path in resolved_video_ini_paths() {
            self.load_video_registry_file(&path);
        }
    }

    fn load_video_registry_file(&mut self, path: &Path) {
        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };

        for video in parse_video_ini_contents(&contents) {
            self.add_video(video);
        }
    }

    pub fn resolve_movie_path(&self, movie_title: &str) -> Option<PathBuf> {
        if let Some(video) = self.get_video_by_name(movie_title) {
            if let Some(path) = resolve_movie_asset_path(&video.filename) {
                return Some(path);
            }
        }
        resolve_movie_asset_path(movie_title)
    }

    fn track_stream(
        &mut self,
        inner: Box<dyn VideoStreamInterface>,
    ) -> Box<dyn VideoStreamInterface> {
        self.active_streams
            .retain(|stream| stream.upgrade().is_some());
        let state = Arc::new(Mutex::new(ManagedVideoStreamState::new(inner)));
        self.active_streams.push(Arc::downgrade(&state));
        Box::new(ManagedVideoStreamHandle::new(state))
    }

    fn update_tracked_streams(&mut self) {
        self.active_streams.retain(|stream| {
            let Some(stream) = stream.upgrade() else {
                return false;
            };

            let keep = if let Ok(mut guard) = stream.lock() {
                guard.update();
                !guard.is_closed()
            } else {
                false
            };
            keep
        });
    }

    fn close_tracked_streams(&mut self) {
        self.active_streams.retain(|stream| {
            let Some(stream) = stream.upgrade() else {
                return false;
            };

            if let Ok(mut guard) = stream.lock() {
                guard.close_inner();
            }
            false
        });
    }

    fn has_registered_provider() -> Bool {
        get_video_stream_provider().is_some()
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
        self.videos_available_for_play.clear();
        self.load_known_video_registry_files();
    }

    fn reset(&mut self) {
        self.close_all_streams();
    }

    fn update(&mut self) {
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

    fn lose_focus(&mut self) {}

    fn regain_focus(&mut self) {}

    fn open(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        let resolved_path = self.resolve_movie_path(&movie_title)?;
        let provider = match get_video_stream_provider() {
            Some(p) => p,
            None => {
                warn!("cutscene skipped (no video stream provider registered): {}", movie_title);
                return None;
            }
        };
        provider
            .open(&movie_title, &resolved_path)
            .map(|stream| self.track_stream(stream))
    }

    fn load(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        let resolved_path = self.resolve_movie_path(&movie_title)?;
        let provider = match get_video_stream_provider() {
            Some(p) => p,
            None => {
                warn!("video load skipped (no video stream provider registered): {}", movie_title);
                return None;
            }
        };
        provider
            .load(&movie_title, &resolved_path)
            .map(|stream| self.track_stream(stream))
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
        self.provider_is_valid = now_has_valid;
    }
}

static THE_VIDEO_PLAYER: OnceLock<Arc<Mutex<Option<VideoPlayer>>>> = OnceLock::new();
static VIDEO_STREAM_PROVIDER: OnceLock<Mutex<Option<Arc<dyn VideoStreamProvider>>>> =
    OnceLock::new();

pub fn init_video_player() {
    let player = THE_VIDEO_PLAYER.get_or_init(|| Arc::new(Mutex::new(Some(VideoPlayer::new()))));
    if let Ok(mut guard) = player.lock() {
        if guard.is_none() {
            let mut player = VideoPlayer::new();
            if VideoPlayer::has_registered_provider() {
                player.notify_video_player_of_new_provider(true);
            }
            *guard = Some(player);
        }
    }
}

pub fn get_video_player() -> Option<Arc<Mutex<Option<VideoPlayer>>>> {
    THE_VIDEO_PLAYER.get().cloned()
}

pub fn register_video_stream_provider(provider: Arc<dyn VideoStreamProvider>) {
    let mut slot = video_stream_provider_slot()
        .lock()
        .expect("video stream provider lock poisoned");
    *slot = Some(provider);
    drop(slot);
    notify_video_player_provider_state(true);
}

pub fn clear_video_stream_provider() {
    notify_video_player_provider_state(false);
    let mut slot = video_stream_provider_slot()
        .lock()
        .expect("video stream provider lock poisoned");
    *slot = None;
}

pub fn shutdown_video_player() {
    if let Some(player) = THE_VIDEO_PLAYER.get() {
        let mut guard = player.lock().unwrap();
        if let Some(player) = guard.as_mut() {
            player.deinit();
        }
        *guard = None;
    }
}

fn video_stream_provider_slot() -> &'static Mutex<Option<Arc<dyn VideoStreamProvider>>> {
    VIDEO_STREAM_PROVIDER.get_or_init(|| Mutex::new(None))
}

fn get_video_stream_provider() -> Option<Arc<dyn VideoStreamProvider>> {
    video_stream_provider_slot()
        .lock()
        .expect("video stream provider lock poisoned")
        .clone()
}

fn notify_video_player_provider_state(now_has_valid: Bool) {
    let Some(player) = get_video_player() else {
        return;
    };
    let Ok(mut guard) = player.lock() else {
        return;
    };
    let Some(player) = guard.as_mut() else {
        return;
    };
    player.notify_video_player_of_new_provider(now_has_valid);
}

struct ManagedVideoStreamState {
    inner: Option<Box<dyn VideoStreamInterface>>,
}

impl ManagedVideoStreamState {
    fn new(inner: Box<dyn VideoStreamInterface>) -> Self {
        Self { inner: Some(inner) }
    }

    fn update(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.update();
        }
    }

    fn close_inner(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.close();
        }
    }

    fn is_closed(&self) -> bool {
        self.inner.is_none()
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
            state.close_inner();
        }
    }

    fn is_frame_ready(&self) -> Bool {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.inner.as_ref().map(|inner| inner.is_frame_ready()))
            .unwrap_or(false)
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
}

fn resolved_video_ini_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("Data/INI/Default/Video.ini"),
        PathBuf::from("Data/INI/Video.ini"),
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/Default/Video.ini"),
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/Video.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/Video.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INIZH/Data/INI/Video.ini"),
    ];

    if let Some(global_data) = get_global_data() {
        let guard = global_data.read();
        let mod_dir = guard.mod_dir.trim();
        if !mod_dir.is_empty() {
            let mod_root = PathBuf::from(mod_dir);
            paths.push(mod_root.join("Data/INI/Default/Video.ini"));
            paths.push(mod_root.join("Data/INI/Video.ini"));
        }
    }

    paths.into_iter().filter(|path| path.is_file()).collect()
}

fn resolve_movie_asset_path(movie_filename: &str) -> Option<PathBuf> {
    let movie_filename = movie_filename.trim();
    if movie_filename.is_empty() {
        return None;
    }

    let direct_path = PathBuf::from(movie_filename);
    if direct_path.is_file() {
        return Some(direct_path);
    }

    let language = get_registry_language().as_str().trim().to_string();
    let mut candidate_roots = Vec::new();

    if let Some(global_data) = get_global_data() {
        let guard = global_data.read();
        let mod_dir = guard.mod_dir.trim();
        if !mod_dir.is_empty() {
            candidate_roots.push(PathBuf::from(mod_dir));
        }
    }

    candidate_roots.push(PathBuf::new());
    candidate_roots.push(PathBuf::from(
        "windows_game/Command & Conquer Generals Zero Hour",
    ));

    for root in candidate_roots {
        let localized_dir = if language.is_empty() {
            None
        } else {
            Some(root.join("Data").join(&language).join("Movies"))
        };

        if let Some(dir) = localized_dir {
            if let Some(path) = find_movie_file_in_dir(&dir, movie_filename) {
                return Some(path);
            }
        }

        let shared_dir = root.join("Data").join("Movies");
        if let Some(path) = find_movie_file_in_dir(&shared_dir, movie_filename) {
            return Some(path);
        }
    }

    None
}

fn find_movie_file_in_dir(dir: &Path, movie_filename: &str) -> Option<PathBuf> {
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

        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !extension.eq_ignore_ascii_case("bik") {
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
    use game_engine::common::ini::get_global_data;
    use game_engine::common::ini::ini_game_data::init_global_data;
    use std::fs::{self, File};
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockProvider {
        updates: Arc<AtomicUsize>,
        closes: Arc<AtomicUsize>,
    }

    impl VideoStreamProvider for MockProvider {
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
    fn video_player_init_loads_known_video_registry_entries() {
        let mut player = VideoPlayer::new();
        player.init();

        assert!(
            player.get_video_by_name("EALogoMovie").is_some()
                || player.get_video_by_name("Sizzle").is_some()
        );
    }

    #[test]
    fn init_video_player_restores_singleton_after_shutdown() {
        init_video_player();
        shutdown_video_player();
        init_video_player();

        let player = get_video_player().expect("video player singleton should exist");
        let guard = player.lock().expect("video player lock should succeed");
        assert!(guard.is_some());
    }

    #[test]
    fn resolve_movie_asset_path_finds_localized_movie_file() {
        let path = resolve_movie_asset_path("EA_LOGO").expect("expected movie file to resolve");
        let normalized = path.to_string_lossy().replace('\\', "/");

        assert!(normalized.ends_with("Data/English/Movies/EA_LOGO.BIK"));
    }

    #[test]
    fn resolve_movie_path_uses_video_registry_filename() {
        let mut player = VideoPlayer::new();
        player.init();

        let path = player
            .resolve_movie_path("EALogoMovie")
            .expect("expected registry-backed movie path");
        let normalized = path.to_string_lossy().replace('\\', "/");

        assert!(normalized.ends_with("Data/English/Movies/EA_LOGO.BIK"));
    }

    #[test]
    fn resolve_movie_asset_path_accepts_bik_extension() {
        let path =
            resolve_movie_asset_path("EA_LOGO.BIK").expect("expected .bik movie file to resolve");
        let normalized = path.to_string_lossy().replace('\\', "/");

        assert!(normalized.ends_with("Data/English/Movies/EA_LOGO.BIK"));
    }

    #[test]
    fn resolve_movie_path_falls_back_to_direct_movie_name() {
        let player = VideoPlayer::new();
        let path = player
            .resolve_movie_path("EA_LOGO.BIK")
            .expect("expected direct movie filename fallback");
        let normalized = path.to_string_lossy().replace('\\', "/");

        assert!(normalized.ends_with("Data/English/Movies/EA_LOGO.BIK"));
    }

    #[test]
    fn resolve_movie_asset_path_accepts_existing_direct_path() {
        let path = PathBuf::from(
            "windows_game/Command & Conquer Generals Zero Hour/Data/English/Movies/EA_LOGO.BIK",
        );
        assert!(path.is_file(), "expected extracted movie asset to exist");

        let resolved = resolve_movie_asset_path(path.to_str().expect("utf-8 path"))
            .expect("expected direct path to resolve");
        assert_eq!(resolved, path);
    }

    #[test]
    fn resolved_video_ini_paths_include_mod_overlay_files() {
        init_global_data();
        let temp_root = std::env::temp_dir().join("generalsrust_video_ini_overlay_test");
        let default_dir = temp_root.join("Data/INI/Default");
        let video_dir = temp_root.join("Data/INI");
        fs::create_dir_all(&default_dir).expect("default ini dir should be created");
        fs::create_dir_all(&video_dir).expect("video ini dir should be created");
        File::create(default_dir.join("Video.ini")).expect("default Video.ini should be created");
        File::create(video_dir.join("Video.ini")).expect("overlay Video.ini should be created");

        let global = get_global_data().expect("global data should be initialized");
        let old_mod_dir = {
            let mut guard = global.write();
            let old = guard.mod_dir.clone();
            guard.mod_dir = temp_root.to_string_lossy().to_string();
            old
        };

        let resolved = resolved_video_ini_paths();
        let normalized: Vec<String> = resolved
            .iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect();

        {
            let mut guard = global.write();
            guard.mod_dir = old_mod_dir;
        }
        let _ = fs::remove_dir_all(&temp_root);

        assert!(normalized
            .iter()
            .any(|path| path.ends_with("Data/INI/Default/Video.ini")));
        assert!(normalized
            .iter()
            .any(|path| path.ends_with("Data/INI/Video.ini")));
    }

    #[test]
    fn tracked_provider_streams_are_updated_by_video_player() {
        let updates = Arc::new(AtomicUsize::new(0));
        let closes = Arc::new(AtomicUsize::new(0));
        register_video_stream_provider(Arc::new(MockProvider {
            updates: Arc::clone(&updates),
            closes: Arc::clone(&closes),
        }));

        let mut player = VideoPlayer::new();
        player.init();
        let _stream = player
            .open("EALogoMovie".to_string())
            .expect("provider-backed stream should open");

        player.update();

        assert_eq!(updates.load(Ordering::SeqCst), 1);
        clear_video_stream_provider();
    }

    #[test]
    fn close_all_streams_closes_provider_streams() {
        let updates = Arc::new(AtomicUsize::new(0));
        let closes = Arc::new(AtomicUsize::new(0));
        register_video_stream_provider(Arc::new(MockProvider {
            updates,
            closes: Arc::clone(&closes),
        }));

        let mut player = VideoPlayer::new();
        player.init();
        let mut stream = player
            .open("EALogoMovie".to_string())
            .expect("provider-backed stream should open");

        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(16, 16));
        stream.frame_render(&mut buffer);

        player.close_all_streams();

        assert!(closes.load(Ordering::SeqCst) >= 1);
        clear_video_stream_provider();
    }

    #[test]
    fn register_video_stream_provider_notifies_existing_player() {
        clear_video_stream_provider();
        init_video_player();

        let player = get_video_player().expect("video player singleton should exist");
        {
            let mut guard = player.lock().expect("video player lock should succeed");
            let player = guard.as_mut().expect("video player should be initialized");
            assert!(!player.provider_is_valid());
        }

        register_video_stream_provider(Arc::new(MockProvider {
            updates: Arc::new(AtomicUsize::new(0)),
            closes: Arc::new(AtomicUsize::new(0)),
        }));

        {
            let mut guard = player.lock().expect("video player lock should succeed");
            let player = guard.as_mut().expect("video player should be initialized");
            assert!(player.provider_is_valid());
        }

        clear_video_stream_provider();
        shutdown_video_player();
    }

    #[test]
    fn clear_video_stream_provider_notifies_existing_player() {
        init_video_player();
        register_video_stream_provider(Arc::new(MockProvider {
            updates: Arc::new(AtomicUsize::new(0)),
            closes: Arc::new(AtomicUsize::new(0)),
        }));

        let player = get_video_player().expect("video player singleton should exist");
        {
            let mut guard = player.lock().expect("video player lock should succeed");
            let player = guard.as_mut().expect("video player should be initialized");
            assert!(player.provider_is_valid());
        }

        clear_video_stream_provider();

        {
            let mut guard = player.lock().expect("video player lock should succeed");
            let player = guard.as_mut().expect("video player should be initialized");
            assert!(!player.provider_is_valid());
        }

        shutdown_video_player();
    }

    #[test]
    fn init_video_player_restores_provider_validity_after_shutdown() {
        register_video_stream_provider(Arc::new(MockProvider {
            updates: Arc::new(AtomicUsize::new(0)),
            closes: Arc::new(AtomicUsize::new(0)),
        }));
        init_video_player();
        shutdown_video_player();
        init_video_player();

        let player = get_video_player().expect("video player singleton should exist");
        let mut guard = player.lock().expect("video player lock should succeed");
        let player = guard.as_mut().expect("video player should be initialized");
        assert!(player.provider_is_valid());

        clear_video_stream_provider();
        shutdown_video_player();
    }

    #[test]
    fn shutdown_video_player_deinitializes_active_streams() {
        let closes = Arc::new(AtomicUsize::new(0));
        register_video_stream_provider(Arc::new(MockProvider {
            updates: Arc::new(AtomicUsize::new(0)),
            closes: Arc::clone(&closes),
        }));

        init_video_player();
        let player = get_video_player().expect("video player singleton should exist");
        {
            let mut guard = player.lock().expect("video player lock should succeed");
            let player = guard.as_mut().expect("video player should be initialized");
            player.init();
            let _stream = player
                .open("EALogoMovie".to_string())
                .expect("provider-backed stream should open");
        }

        shutdown_video_player();
        assert_eq!(closes.load(Ordering::SeqCst), 1);

        clear_video_stream_provider();
    }
}
