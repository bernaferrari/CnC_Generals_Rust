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
