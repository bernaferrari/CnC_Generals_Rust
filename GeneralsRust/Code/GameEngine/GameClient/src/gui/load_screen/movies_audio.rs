// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn play_single_player_movie(wm: &mut WindowManager, window_name: &str, movie_name: &str) -> bool {
    if movie_name.is_empty() {
        return false;
    }
    #[cfg(test)]
    if let Some(hook) = single_player_movie_play_hook() {
        return hook(movie_name);
    }
    let Some(window) = wm.find_window_by_name(window_name) else {
        return false;
    };
    with_window_video_manager(|manager| {
        manager.play_movie(window, movie_name.to_string(), WindowVideoPlayType::Once)
    })
}

fn is_single_player_movie_playing(movie_name: &str) -> bool {
    #[cfg(test)]
    if let Some(hook) = single_player_movie_playing_hook() {
        return hook(movie_name);
    }
    with_window_video_manager(|manager| manager.is_movie_playing(movie_name))
}

fn play_challenge_background_movie(
    wm: &mut WindowManager,
    window_name: &str,
    movie_name: &str,
) -> bool {
    if movie_name.is_empty() {
        return false;
    }
    #[cfg(test)]
    if let Some(hook) = challenge_movie_play_hook() {
        return hook(movie_name);
    }
    let Some(window) = wm.find_window_by_name(window_name) else {
        return false;
    };
    with_window_video_manager(|manager| {
        manager.play_movie(window, movie_name.to_string(), WindowVideoPlayType::Once)
    })
}

fn play_challenge_movie(wm: &mut WindowManager, window_name: &str, movie_name: &str) -> bool {
    if movie_name.is_empty() {
        return false;
    }
    let Some(window) = wm.find_window_by_name(window_name) else {
        return false;
    };
    with_window_video_manager(|manager| {
        manager.play_movie(
            window,
            movie_name.to_string(),
            WindowVideoPlayType::ShowLastFrame,
        )
    })
}

/// Advance the Challenge background exactly once and retain its final authored
/// frame when a one-shot entry removes itself from WindowVideoManager. A frame
/// that is not decoder-ready remains pending, matching C++'s short Sleep(1)
/// retry instead of treating it as a rendered animation step.
fn advance_challenge_background_movie(movie_name: &str) -> Option<LoadScreenMovieAdvance> {
    #[cfg(test)]
    if let Some(hook) = challenge_movie_advance_hook() {
        return hook(movie_name);
    }

    with_window_video_manager(|manager| {
        let before = manager.movie_progress(movie_name);
        if let Some(before) = before {
            if !before.frame_ready {
                return Some(LoadScreenMovieAdvance {
                    frame_index: before.frame_index,
                    frame_count: before.frame_count,
                    completed: false,
                });
            }
        }
        manager.update();
        if let Some(after) = manager.movie_progress(movie_name) {
            return Some(LoadScreenMovieAdvance {
                frame_index: after.frame_index,
                frame_count: after.frame_count,
                completed: false,
            });
        }

        before.map(|before| LoadScreenMovieAdvance {
            frame_index: before.frame_count.saturating_sub(1).max(before.frame_index),
            frame_count: before.frame_count,
            completed: true,
        })
    })
}

#[cfg(test)]
fn register_single_player_movie_play_hook(
    hook: impl Fn(&str) -> bool + Send + Sync + 'static,
) -> Option<SinglePlayerMoviePlayHook> {
    let hook = Arc::new(hook);
    let state = SINGLE_PLAYER_MOVIE_PLAY_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(hook)
}

#[cfg(test)]
fn register_single_player_movie_playing_hook(
    hook: impl Fn(&str) -> bool + Send + Sync + 'static,
) -> Option<SinglePlayerMoviePlayHook> {
    let hook = Arc::new(hook);
    let state = SINGLE_PLAYER_MOVIE_PLAYING_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(hook)
}

#[cfg(test)]
fn clear_single_player_movie_play_hook() -> Option<SinglePlayerMoviePlayHook> {
    let state = SINGLE_PLAYER_MOVIE_PLAY_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

#[cfg(test)]
fn clear_single_player_movie_playing_hook() -> Option<SinglePlayerMoviePlayHook> {
    let state = SINGLE_PLAYER_MOVIE_PLAYING_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

#[cfg(test)]
fn single_player_movie_play_hook() -> Option<SinglePlayerMoviePlayHook> {
    let state = SINGLE_PLAYER_MOVIE_PLAY_HOOK.get_or_init(|| Mutex::new(None));
    let guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

#[cfg(test)]
fn single_player_movie_playing_hook() -> Option<SinglePlayerMoviePlayHook> {
    let state = SINGLE_PLAYER_MOVIE_PLAYING_HOOK.get_or_init(|| Mutex::new(None));
    let guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

#[cfg(test)]
fn register_challenge_movie_play_hook(
    hook: impl Fn(&str) -> bool + Send + Sync + 'static,
) -> Option<ChallengeMoviePlayHook> {
    let state = CHALLENGE_MOVIE_PLAY_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(Arc::new(hook))
}

#[cfg(test)]
fn clear_challenge_movie_play_hook() -> Option<ChallengeMoviePlayHook> {
    let state = CHALLENGE_MOVIE_PLAY_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

#[cfg(test)]
fn challenge_movie_play_hook() -> Option<ChallengeMoviePlayHook> {
    let state = CHALLENGE_MOVIE_PLAY_HOOK.get_or_init(|| Mutex::new(None));
    let guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

#[cfg(test)]
fn register_challenge_movie_advance_hook(
    hook: impl Fn(&str) -> Option<LoadScreenMovieAdvance> + Send + Sync + 'static,
) -> Option<ChallengeMovieAdvanceHook> {
    let state = CHALLENGE_MOVIE_ADVANCE_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(Arc::new(hook))
}

#[cfg(test)]
fn clear_challenge_movie_advance_hook() -> Option<ChallengeMovieAdvanceHook> {
    let state = CHALLENGE_MOVIE_ADVANCE_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

#[cfg(test)]
fn challenge_movie_advance_hook() -> Option<ChallengeMovieAdvanceHook> {
    let state = CHALLENGE_MOVIE_ADVANCE_HOOK.get_or_init(|| Mutex::new(None));
    let guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

fn play_audio_event(event_name: &str) {
    let _ = add_audio_event(event_name);
}

#[cfg(not(test))]
fn add_audio_event(event_name: &str) -> u32 {
    if event_name.is_empty() {
        return 0;
    }
    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new(event_name);
        audio.add_audio_event(&event)
    } else {
        0
    }
}

#[cfg(test)]
fn add_audio_event(event_name: &str) -> u32 {
    if event_name.is_empty() {
        0
    } else {
        event_name
            .bytes()
            .fold(1_u32, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(byte as u32)
            })
            .max(1)
    }
}
