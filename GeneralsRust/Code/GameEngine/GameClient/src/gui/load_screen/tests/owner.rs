// LoadScreen.cpp owner fixture cases; included in the original logical test module.

#[test]
fn peer_loading_state_and_retirement_are_independent() {
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
