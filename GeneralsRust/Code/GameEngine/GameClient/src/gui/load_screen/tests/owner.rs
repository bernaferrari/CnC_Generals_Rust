// LoadScreen.cpp owner fixture cases; included in the original logical test module.

#[test]
fn peer_loading_state_and_retirement_are_independent() {
    crate::test_support::ensure_ambient_host();
    let mut owner = LoadScreenFixture::new();
    owner.with_runtime(|runtime, _, _| {
        runtime.state.single_player.movie_label = "owner-intro".into();
        runtime.state.multiplayer.player_lookup[3] = 5;
        {
            let mut peer = LoadScreenFixture::new();
            peer.with_runtime(|peer, _, _| {
                peer.state.single_player.movie_label = "peer-intro".into();
                peer.state.multiplayer.player_lookup[3] = 1;
                reset_multiplayer_load_screen_state(peer);
                assert_eq!(peer.state.multiplayer.player_lookup[3], -1);
            });
        }
        assert_eq!(runtime.state.single_player.movie_label, "owner-intro");
        assert_eq!(runtime.state.multiplayer.player_lookup[3], 5);
    });
}

#[test]
fn reset_visits_only_the_receiving_window_tree() {
    crate::test_support::ensure_ambient_host();
    let mut first = LoadScreenFixture::new();
    let mut second = LoadScreenFixture::new();
    let root_name = descriptor_for_kind(LoadScreenKind::Multiplayer).root;
    first.with_runtime(|first, _, _| {
        let windows = first.windows;
        windows.with_manager(|wm| {
            let root = wm
                .create_window(None, 0, 0, 100, 100, &mut first.effects)
                .unwrap();
            root.borrow_mut().set_name(root_name);
        });
        second.with_runtime(|second, _, _| {
            let windows = second.windows;
            windows.with_manager(|wm| {
                let root = wm
                    .create_window(None, 0, 0, 100, 100, &mut second.effects)
                    .unwrap();
                root.borrow_mut().set_name(root_name);
            });
            reset_load_screen(LoadScreenKind::Multiplayer, second);
            assert!(
                first
                    .windows
                    .with_manager_ref(|wm| wm.find_window_by_name(root_name).is_some())
            );
            assert!(
                second
                    .windows
                    .with_manager_ref(|wm| wm.find_window_by_name(root_name).is_none())
            );
        });
    });
}

#[test]
fn multiplayer_loading_screen_teardown_fades_receiving_music_once() {
    use game_engine::common::audio::game_audio::SoundPlaybackHook;
    use game_engine::common::audio::{AudioEventRts, AudioManager};
    use std::sync::{Arc, Mutex};

    struct FixtureOutput;
    impl SoundPlaybackHook for FixtureOutput {
        fn play(&self, _: &AudioEventRts) -> Result<(), String> {
            Ok(())
        }
        fn stop(&self, _: u32) {}
        fn pause(&self, _: u32) {}
        fn resume(&self, _: u32) {}
        fn is_playing(&self, _: u32) -> bool {
            true
        }
    }

    crate::test_support::ensure_ambient_host();
    let mixer = Arc::new(Mutex::new(AudioManager::with_playback_hook(Box::new(
        FixtureOutput,
    ))));
    let mixer_state = || {
        let mixer = mixer.lock().unwrap();
        (
            mixer.active_event_count(),
            mixer.fading_audio_count(),
            mixer.fading_frames(),
        )
    };
    let mut receiver = LoadScreenFixture::with_mixer(Arc::clone(&mixer));
    let mut peer = LoadScreenFixture::with_mixer(Arc::clone(&mixer));
    peer.with_runtime(|runtime, _, _| play_multiplayer_load_screen_music("Load_USA", runtime));
    receiver.with_runtime(|runtime, _, _| {
        let windows = runtime.windows;
        windows.with_manager(|wm| {
            for kind in [LoadScreenKind::Multiplayer, LoadScreenKind::GameSpy] {
                let root = wm
                    .create_window(None, 0, 0, 100, 100, &mut runtime.effects)
                    .unwrap();
                root.borrow_mut().set_name(descriptor_for_kind(kind).root);
            }
        });
        play_multiplayer_load_screen_music("Load_USA", runtime);
        assert_eq!(mixer_state(), (2, 0, 0));

        reset_load_screen(LoadScreenKind::GameSpy, runtime);
        runtime.effects.pump_menu_audio();
        assert_eq!(mixer_state(), (2, 0, 0));

        reset_load_screen(LoadScreenKind::Multiplayer, runtime);
        assert_eq!(mixer_state(), (2, 0, 0), "teardown doesn't pump audio");
        runtime.effects.play_menu_audio("Load_USA").unwrap();
        reset_load_screen(LoadScreenKind::Multiplayer, runtime);
        runtime.effects.pump_menu_audio();
        assert_eq!(
            mixer_state(),
            (2, 1, 1),
            "loading music must fade while peer and next track stay active"
        );
    });
}
