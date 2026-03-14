use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "WindowVideoManager.cpp",
    "crate::gui::window_video_manager",
    "Window Video Manager",
    "Coordinates legacy FMV and video-backed UI surfaces within GPUI-driven windows.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowVideoStatePort {
    Stop,
    Play,
    Pause,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowVideoPlayTypePort {
    Once,
    ShowLastFrame,
    Loop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowVideoPort {
    pub window_id: i32,
    pub movie_name: String,
    pub state: WindowVideoStatePort,
    pub play_type: WindowVideoPlayTypePort,
    pub frame_index: usize,
    pub total_frames: usize,
    pub hidden_by_window: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WindowVideoManagerPort {
    pub videos: Vec<WindowVideoPort>,
    pub stop_all_movies: bool,
    pub pause_all_movies: bool,
}

impl WindowVideoManagerPort {
    pub fn init(&mut self) {
        self.videos.clear();
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    pub fn play(
        &mut self,
        window_id: i32,
        movie_name: impl Into<String>,
        play_type: WindowVideoPlayTypePort,
    ) {
        let movie_name = movie_name.into();
        self.stop_and_remove(window_id);
        self.videos.push(WindowVideoPort {
            window_id,
            movie_name,
            state: WindowVideoStatePort::Play,
            play_type,
            frame_index: 0,
            total_frames: 120,
            hidden_by_window: false,
        });
        self.pause_all_movies = false;
        self.stop_all_movies = false;
    }

    pub fn pause(&mut self, window_id: i32) {
        if let Some(video) = self
            .videos
            .iter_mut()
            .find(|video| video.window_id == window_id)
        {
            video.state = WindowVideoStatePort::Pause;
        }
    }

    pub fn hide(&mut self, window_id: i32) {
        if let Some(video) = self
            .videos
            .iter_mut()
            .find(|video| video.window_id == window_id)
        {
            video.state = WindowVideoStatePort::Hidden;
            video.hidden_by_window = true;
        }
    }

    pub fn resume(&mut self, window_id: i32) {
        if let Some(video) = self
            .videos
            .iter_mut()
            .find(|video| video.window_id == window_id)
        {
            video.state = WindowVideoStatePort::Play;
            video.hidden_by_window = false;
        }
        self.pause_all_movies = false;
        self.stop_all_movies = false;
    }

    pub fn stop(&mut self, window_id: i32) {
        if let Some(video) = self
            .videos
            .iter_mut()
            .find(|video| video.window_id == window_id)
        {
            video.state = WindowVideoStatePort::Stop;
        }
    }

    pub fn stop_and_remove(&mut self, window_id: i32) {
        self.videos.retain(|video| video.window_id != window_id);
    }

    pub fn stop_all(&mut self) {
        for video in &mut self.videos {
            video.state = WindowVideoStatePort::Stop;
        }
        self.stop_all_movies = true;
        self.pause_all_movies = false;
    }

    pub fn pause_all(&mut self) {
        for video in &mut self.videos {
            video.state = WindowVideoStatePort::Pause;
        }
        self.pause_all_movies = true;
        self.stop_all_movies = false;
    }

    pub fn resume_all(&mut self) {
        for video in &mut self.videos {
            video.state = WindowVideoStatePort::Play;
            video.hidden_by_window = false;
        }
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    pub fn update(&mut self, hidden_windows: &[i32]) {
        if self.pause_all_movies || self.stop_all_movies {
            return;
        }

        for video in &mut self.videos {
            if video.state == WindowVideoStatePort::Hidden
                && !hidden_windows.contains(&video.window_id)
            {
                video.state = WindowVideoStatePort::Play;
                video.hidden_by_window = false;
            }

            if video.state == WindowVideoStatePort::Play
                && hidden_windows.contains(&video.window_id)
            {
                video.state = WindowVideoStatePort::Hidden;
                video.hidden_by_window = true;
            }

            if video.state != WindowVideoStatePort::Play {
                continue;
            }

            video.frame_index = (video.frame_index + 1) % video.total_frames.max(1);
            if video.frame_index == 0 {
                match video.play_type {
                    WindowVideoPlayTypePort::Once => video.state = WindowVideoStatePort::Stop,
                    WindowVideoPlayTypePort::ShowLastFrame => {
                        video.state = WindowVideoStatePort::Pause;
                        video.frame_index = video.total_frames.saturating_sub(1);
                    }
                    WindowVideoPlayTypePort::Loop => {}
                }
            }
        }
    }

    pub fn get_win_state(&self, window_id: i32) -> WindowVideoStatePort {
        self.videos
            .iter()
            .find(|video| video.window_id == window_id)
            .map(|video| video.state)
            .unwrap_or(WindowVideoStatePort::Stop)
    }

    pub fn reset(&mut self) {
        self.videos.clear();
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_replaces_existing_video_for_same_window() {
        let mut manager = WindowVideoManagerPort::default();
        manager.play(1, "A.bik", WindowVideoPlayTypePort::Loop);
        manager.play(1, "B.bik", WindowVideoPlayTypePort::Loop);

        assert_eq!(manager.videos.len(), 1);
        assert_eq!(manager.videos[0].movie_name, "B.bik");
    }

    #[test]
    fn hidden_window_moves_video_to_hidden_state() {
        let mut manager = WindowVideoManagerPort::default();
        manager.play(1, "A.bik", WindowVideoPlayTypePort::Loop);
        manager.update(&[1]);

        assert_eq!(manager.get_win_state(1), WindowVideoStatePort::Hidden);
    }

    #[test]
    fn once_play_type_stops_when_loop_wraps() {
        let mut manager = WindowVideoManagerPort::default();
        manager.play(1, "A.bik", WindowVideoPlayTypePort::Once);
        if let Some(video) = manager.videos.first_mut() {
            video.total_frames = 2;
            video.frame_index = 1;
        }
        manager.update(&[]);

        assert_eq!(manager.get_win_state(1), WindowVideoStatePort::Stop);
    }
}
