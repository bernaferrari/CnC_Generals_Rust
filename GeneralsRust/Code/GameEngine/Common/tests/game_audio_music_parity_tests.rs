use game_engine::common::audio::AudioManager;

#[test]
fn music_track_navigation_updates_audio_manager_current_track() {
    let mut audio = AudioManager::new();
    audio.add_track_name("TrackA".to_string());
    audio.add_track_name("TrackB".to_string());
    audio.add_track_name("TrackC".to_string());

    audio.set_music_track_name("TrackA".to_string());
    assert_eq!(audio.next_music_track(), "TrackB");
    assert_eq!(audio.get_music_track_name(), "TrackB");

    assert_eq!(audio.prev_music_track(), "TrackA");
    assert_eq!(audio.get_music_track_name(), "TrackA");

    audio.set_music_track_name("Unknown".to_string());
    assert_eq!(audio.next_music_track(), "TrackA");
    assert_eq!(audio.prev_music_track(), "TrackC");
}
