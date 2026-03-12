//! Window video manager (matches WindowVideoManager.cpp).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::video_buffer::{SoftwareVideoBuffer, VideoBufferHandle, VideoBufferType};
use crate::video_player::{get_video_player, VideoPlayerInterface};
use crate::video_stream::VideoStreamInterface;

use super::game_window::GameWindow;

thread_local! {
    static WINDOW_VIDEO_MANAGER: RefCell<WindowVideoManager> =
        RefCell::new(WindowVideoManager::new());
}

pub fn with_window_video_manager<R>(f: impl FnOnce(&mut WindowVideoManager) -> R) -> R {
    WINDOW_VIDEO_MANAGER.with(|manager| f(&mut manager.borrow_mut()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVideoPlayType {
    Once,
    Loop,
    ShowLastFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVideoState {
    Start,
    Stop,
    Pause,
    Play,
    Hidden,
}

pub struct WindowVideo {
    play_type: WindowVideoPlayType,
    window: Option<Weak<RefCell<GameWindow>>>,
    video_buffer: Option<VideoBufferHandle>,
    video_stream: Option<Box<dyn VideoStreamInterface>>,
    movie_name: String,
    state: WindowVideoState,
}

impl WindowVideo {
    pub fn new() -> Self {
        Self {
            play_type: WindowVideoPlayType::Once,
            window: None,
            video_buffer: None,
            video_stream: None,
            movie_name: String::new(),
            state: WindowVideoState::Stop,
        }
    }

    pub fn init(
        &mut self,
        window: Rc<RefCell<GameWindow>>,
        movie_name: String,
        play_type: WindowVideoPlayType,
        video_buffer: VideoBufferHandle,
        video_stream: Box<dyn VideoStreamInterface>,
    ) {
        self.window = Some(Rc::downgrade(&window));
        self.movie_name = movie_name;
        self.play_type = play_type;
        self.video_buffer = Some(video_buffer.clone());
        self.video_stream = Some(video_stream);
        self.state = WindowVideoState::Play;
        window.borrow_mut().set_video_buffer(Some(video_buffer));
    }

    pub fn set_window_state(&mut self, state: WindowVideoState) {
        self.state = state;
        if let Some(window) = self.window.as_ref().and_then(|w| w.upgrade()) {
            match self.state {
                WindowVideoState::Stop => window.borrow_mut().set_video_buffer(None),
                WindowVideoState::Play | WindowVideoState::Pause => window
                    .borrow_mut()
                    .set_video_buffer(self.video_buffer.clone()),
                WindowVideoState::Start | WindowVideoState::Hidden => {}
            }
        }
    }

    pub fn play_type(&self) -> WindowVideoPlayType {
        self.play_type
    }

    pub fn state(&self) -> WindowVideoState {
        self.state
    }

    pub fn movie_name(&self) -> &str {
        &self.movie_name
    }

    pub fn window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.window.as_ref().and_then(|w| w.upgrade())
    }

    pub fn video_stream_mut(&mut self) -> Option<&mut (dyn VideoStreamInterface + 'static)> {
        self.video_stream.as_deref_mut()
    }

    pub fn video_buffer(&self) -> Option<VideoBufferHandle> {
        self.video_buffer.clone()
    }
}

impl Drop for WindowVideo {
    fn drop(&mut self) {
        if let Some(window) = self.window.as_ref().and_then(|w| w.upgrade()) {
            window.borrow_mut().set_video_buffer(None);
        }
        if let Some(stream) = self.video_stream.take() {
            stream.close();
        }
    }
}

pub struct WindowVideoManager {
    playing_videos: HashMap<usize, WindowVideo>,
    stop_all_movies: bool,
    pause_all_movies: bool,
}

impl WindowVideoManager {
    pub fn new() -> Self {
        Self {
            playing_videos: HashMap::new(),
            stop_all_movies: false,
            pause_all_movies: false,
        }
    }

    pub fn init(&mut self) {
        self.playing_videos.clear();
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    pub fn reset(&mut self) {
        self.playing_videos.clear();
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    pub fn update(&mut self) {
        // C++ parity: when global stop/pause toggles are active, update short-circuits.
        if self.stop_all_movies || self.pause_all_movies {
            return;
        }

        let keys: Vec<usize> = self.playing_videos.keys().cloned().collect();
        let mut remove_keys = Vec::new();
        for key in keys {
            let Some(win_video) = self.playing_videos.get_mut(&key) else {
                continue;
            };
            let Some(window) = win_video.window() else {
                win_video.set_window_state(WindowVideoState::Stop);
                remove_keys.push(key);
                continue;
            };

            let window_hidden = window.borrow().is_hidden();
            if win_video.state() == WindowVideoState::Hidden && !window_hidden {
                win_video.set_window_state(WindowVideoState::Play);
            }

            if win_video.state() == WindowVideoState::Play && window_hidden {
                win_video.set_window_state(WindowVideoState::Hidden);
            }

            if win_video.state() != WindowVideoState::Play {
                continue;
            }

            let Some(buffer_handle) = win_video.video_buffer() else {
                win_video.set_window_state(WindowVideoState::Stop);
                remove_keys.push(key);
                continue;
            };
            let play_type = win_video.play_type();
            let Some(stream) = win_video.video_stream_mut() else {
                win_video.set_window_state(WindowVideoState::Stop);
                remove_keys.push(key);
                continue;
            };

            if stream.is_frame_ready() {
                stream.frame_decompress();
                let mut buffer = buffer_handle.lock();
                stream.frame_render(&mut *buffer);
                stream.frame_next();

                if stream.frame_index() == 0 {
                    match play_type {
                        WindowVideoPlayType::Once => {
                            win_video.set_window_state(WindowVideoState::Stop);
                            remove_keys.push(key);
                        }
                        WindowVideoPlayType::ShowLastFrame => {
                            win_video.set_window_state(WindowVideoState::Pause)
                        }
                        WindowVideoPlayType::Loop => {}
                    }
                }
            }
        }

        if !remove_keys.is_empty() {
            remove_keys.sort_unstable();
            remove_keys.dedup();
            for key in remove_keys {
                self.playing_videos.remove(&key);
            }
        }
    }

    pub fn play_movie(
        &mut self,
        window: Rc<RefCell<GameWindow>>,
        movie_name: String,
        play_type: WindowVideoPlayType,
    ) -> bool {
        self.stop_and_remove_movie(&window);

        let mut stream = open_stream(movie_name.clone());
        let Some(stream_ref) = stream.as_ref() else {
            return false;
        };
        let (width, height) = (stream_ref.width().max(1), stream_ref.height().max(1));

        let buffer = VideoBufferHandle::new(SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8));
        if !buffer.lock().allocate(width as u32, height as u32) {
            stream.take();
            return false;
        }

        let mut win_video = WindowVideo::new();
        if let Some(stream) = stream.take() {
            win_video.init(window.clone(), movie_name, play_type, buffer, stream);
            self.playing_videos.insert(window_key(&window), win_video);
        } else {
            return false;
        }

        self.pause_all_movies = false;
        self.stop_all_movies = false;
        true
    }

    pub fn pause_movie(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if let Some(win_video) = self.playing_videos.get_mut(&window_key(window)) {
            win_video.set_window_state(WindowVideoState::Pause);
        }
    }

    pub fn hide_movie(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if let Some(win_video) = self.playing_videos.get_mut(&window_key(window)) {
            win_video.set_window_state(WindowVideoState::Hidden);
        }
    }

    pub fn resume_movie(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if let Some(win_video) = self.playing_videos.get_mut(&window_key(window)) {
            win_video.set_window_state(WindowVideoState::Play);
        }
        self.pause_all_movies = false;
        self.stop_all_movies = false;
    }

    pub fn stop_movie(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if let Some(win_video) = self.playing_videos.get_mut(&window_key(window)) {
            win_video.set_window_state(WindowVideoState::Stop);
        }
    }

    pub fn stop_and_remove_movie(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if let Some(mut win_video) = self.playing_videos.remove(&window_key(window)) {
            win_video.set_window_state(WindowVideoState::Stop);
        }
    }

    pub fn stop_all_movies(&mut self) {
        for win_video in self.playing_videos.values_mut() {
            win_video.set_window_state(WindowVideoState::Stop);
        }
        self.stop_all_movies = true;
        self.pause_all_movies = false;
    }

    pub fn pause_all_movies(&mut self) {
        for win_video in self.playing_videos.values_mut() {
            win_video.set_window_state(WindowVideoState::Pause);
        }
        self.pause_all_movies = true;
        self.stop_all_movies = false;
    }

    pub fn resume_all_movies(&mut self) {
        for win_video in self.playing_videos.values_mut() {
            win_video.set_window_state(WindowVideoState::Play);
        }
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    pub fn get_win_state(&self, window: &Rc<RefCell<GameWindow>>) -> WindowVideoState {
        self.playing_videos
            .get(&window_key(window))
            .map(|video| video.state())
            .unwrap_or(WindowVideoState::Stop)
    }

    pub fn is_movie_playing(&self, movie_name: &str) -> bool {
        let key = movie_name.trim();
        if key.is_empty() {
            return false;
        }

        self.playing_videos.values().any(|video| {
            video.movie_name().eq_ignore_ascii_case(key)
                && !matches!(video.state(), WindowVideoState::Stop)
        })
    }
}

fn window_key(window: &Rc<RefCell<GameWindow>>) -> usize {
    Rc::as_ptr(window) as usize
}

fn open_stream(movie_name: String) -> Option<Box<dyn VideoStreamInterface>> {
    if let Some(player) = get_video_player() {
        if let Ok(mut guard) = player.lock() {
            if let Some(ref mut player) = *guard {
                if let Some(stream) = player.open(movie_name.clone()) {
                    return Some(stream);
                }
            }
        }
    }
    let _ = movie_name;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window() -> Rc<RefCell<GameWindow>> {
        Rc::new(RefCell::new(GameWindow::new()))
    }

    #[test]
    fn stop_movie_keeps_entry_and_sets_stop_state() {
        let window = make_window();
        let mut manager = WindowVideoManager::new();
        let mut video = WindowVideo::new();
        video.movie_name = "TestMovie".to_string();
        video.state = WindowVideoState::Play;
        manager.playing_videos.insert(window_key(&window), video);

        assert!(manager.is_movie_playing("testmovie"));

        manager.stop_movie(&window);

        assert_eq!(manager.playing_videos.len(), 1);
        assert_eq!(manager.get_win_state(&window), WindowVideoState::Stop);
        assert!(!manager.is_movie_playing("TestMovie"));
    }

    #[test]
    fn stop_all_movies_marks_entries_stopped_without_removal() {
        let window_a = make_window();
        let window_b = make_window();
        let mut manager = WindowVideoManager::new();
        let mut video_a = WindowVideo::new();
        video_a.state = WindowVideoState::Play;
        video_a.movie_name = "A".to_string();
        manager
            .playing_videos
            .insert(window_key(&window_a), video_a);
        let mut video_b = WindowVideo::new();
        video_b.state = WindowVideoState::Hidden;
        video_b.movie_name = "B".to_string();
        manager
            .playing_videos
            .insert(window_key(&window_b), video_b);

        manager.stop_all_movies();

        assert_eq!(manager.playing_videos.len(), 2);
        assert!(manager.stop_all_movies);
        assert!(!manager.pause_all_movies);
        assert_eq!(manager.get_win_state(&window_a), WindowVideoState::Stop);
        assert_eq!(manager.get_win_state(&window_b), WindowVideoState::Stop);
    }

    #[test]
    fn resume_all_movies_restores_play_state_for_stopped_entries() {
        let window = make_window();
        let mut manager = WindowVideoManager::new();
        let mut video = WindowVideo::new();
        video.state = WindowVideoState::Stop;
        manager.playing_videos.insert(window_key(&window), video);
        manager.stop_all_movies = true;

        manager.resume_all_movies();

        assert_eq!(manager.get_win_state(&window), WindowVideoState::Play);
        assert!(!manager.stop_all_movies);
        assert!(!manager.pause_all_movies);
    }

    #[test]
    fn update_returns_early_when_pause_all_movies_is_set() {
        let window = make_window();
        let mut manager = WindowVideoManager::new();
        manager.pause_all_movies = true;

        let mut video = WindowVideo::new();
        video.window = Some(Rc::downgrade(&window));
        video.state = WindowVideoState::Hidden;
        manager.playing_videos.insert(window_key(&window), video);

        manager.update();

        assert_eq!(manager.get_win_state(&window), WindowVideoState::Hidden);
        assert_eq!(manager.playing_videos.len(), 1);
    }

    #[test]
    fn update_does_not_hide_paused_window_when_window_is_hidden() {
        let window = make_window();
        window
            .borrow_mut()
            .hide(true)
            .expect("window hidden flag should be set");

        let mut manager = WindowVideoManager::new();
        let mut video = WindowVideo::new();
        video.window = Some(Rc::downgrade(&window));
        video.state = WindowVideoState::Pause;
        manager.playing_videos.insert(window_key(&window), video);

        manager.update();

        assert_eq!(manager.get_win_state(&window), WindowVideoState::Pause);
    }

    #[test]
    fn update_resumes_hidden_video_when_window_becomes_visible() {
        let window = make_window();

        let mut manager = WindowVideoManager::new();
        let mut video = WindowVideo::new();
        video.window = Some(Rc::downgrade(&window));
        video.state = WindowVideoState::Hidden;
        video.play_type = WindowVideoPlayType::Loop;
        let buffer = VideoBufferHandle::new(SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8));
        assert!(buffer.lock().allocate(2, 2));
        video.video_buffer = Some(buffer);
        video.video_stream = Some(Box::new(crate::video_stream::VideoStream::new()));
        manager.playing_videos.insert(window_key(&window), video);

        manager.update();

        assert_eq!(manager.get_win_state(&window), WindowVideoState::Play);
    }
}
