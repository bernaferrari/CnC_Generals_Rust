use std::sync::Arc;

use game_engine::common::audio::Coord3D;
use game_engine::common::audio::audio_event_rts::AudioEventRts;

#[test]
fn retired_audio_owner_stops_resolving_without_retiring_peer_identity() {
    let owner = Arc::new(());
    let peer = Arc::new(());
    let mut first = AudioEventRts::new();
    let mut second = AudioEventRts::new();
    let first_position = Coord3D {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let second_position = Coord3D {
        x: 4.0,
        y: 5.0,
        z: 6.0,
    };
    first.set_receiving_object_spatial_owner(7, &owner, first_position);
    second.set_receiving_object_spatial_owner(7, &peer, second_position);
    drop(owner);

    assert_eq!(first.get_current_position().copied(), Some(first_position));
    assert!(first.is_dead());
    assert_eq!(first.get_current_position().copied(), None);
    assert_eq!(
        second.get_current_position().copied(),
        Some(second_position)
    );
    assert!(!second.is_dead());
}

#[test]
fn retiring_one_worlds_audio_does_not_stop_the_peer_world() {
    use game_engine::common::audio::{AudioEventInfo, AudioManager, AudioType, ST_WORLD};

    let mut manager = AudioManager::new();
    manager.init();
    let owner = Arc::new(());
    let peer = Arc::new(());
    let info = Arc::new(AudioEventInfo {
        sound_type: AudioType::SoundEffect,
        sound_type_field: AudioType::SoundEffect,
        type_field: ST_WORLD,
        volume: 1.0,
        min_distance: 25.0,
        max_distance: 100.0,
        ..Default::default()
    });
    for (handle, identity) in [(1001, &owner), (1002, &peer)] {
        let mut event = AudioEventRts::new();
        event.set_playing_handle(handle);
        event.set_audio_event_info(Arc::clone(&info));
        event.set_volume(1.0);
        event.set_volume_shift(1.0);
        event.set_receiving_object_spatial_owner(7, identity, Coord3D::ZERO);
        manager.insert_playing_event_for_test(event);
    }
    drop(owner);
    manager.update();
    manager.update();
    assert_eq!(manager.active_event_count(), 1);
    assert!(manager.active_event_portion(1001).is_none());
    assert!(manager.active_event_portion(1002).is_some());
}
