use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::WindowMessage;
fn client() -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.init_input_subsystems().unwrap();
    client
}
#[test]
fn interleaved_wnd_events_keep_cursor_and_tooltip_render_with_their_client() {
    use game_client_rust::input::MouseCursor;

    fn install(client: &mut GameClient, text: &'static str, cursor: MouseCursor) {
        client.input_mouse_mut().unwrap().set_visibility(true);
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let window = manager.create_window(None, 0, 0, 100, 100, effects).unwrap();
                let mut window = window.borrow_mut();
                window.set_id(42);
                window.set_tooltip_callback(move |_, _, _, mouse| {
                    mouse.set_cursor(cursor);
                    mouse.set_cursor_tooltip(text.into(), Some(0), None, None);
                });
            });
        });
    }
    fn move_and_dispatch(client: &mut GameClient, x: i32, y: i32) {
        client.input_mouse_mut().unwrap().handle_mouse_move(x as f32, y as f32);
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager.process_mouse_event(WindowMessage::MousePos, x, y, 0, effects);
            });
        });
    }
    fn draw(client: &mut GameClient) -> (MouseCursor, String, f32, f32) {
        let mouse = client.input_mouse_mut().unwrap();
        mouse.update();
        mouse.update();
        let info = mouse.compute_tooltip_draw_info(800.0, 600.0).unwrap();
        (mouse.get_cursor(), info.text, info.x, info.y)
    }

    let _common_services = game_engine::common::common_services::CommonServicesScope::new();
    let owner_world = gamelogic::system::engine_stores::new_world_services();
    let peer_world = gamelogic::system::engine_stores::new_world_services();
    use gamelogic::system::engine_stores::with_world_services;
    let mut owner = with_world_services(&owner_world, client);
    let mut peer = with_world_services(&peer_world, client);
    with_world_services(&owner_world, || install(&mut owner, "owner", MouseCursor::Selecting));
    with_world_services(&peer_world, || install(&mut peer, "peer", MouseCursor::AttackObject));
    with_world_services(&owner_world, || move_and_dispatch(&mut owner, 10, 15));
    with_world_services(&peer_world, || move_and_dispatch(&mut peer, 30, 35));
    with_world_services(&owner_world, || move_and_dispatch(&mut owner, 20, 25));
    assert_eq!(with_world_services(&owner_world, || draw(&mut owner)), (MouseCursor::Selecting, "owner".into(), 40.0, 25.0));
    assert_eq!(with_world_services(&peer_world, || draw(&mut peer)), (MouseCursor::AttackObject, "peer".into(), 50.0, 35.0));
    let moved_peer = Box::new(peer);
    with_world_services(&peer_world, || drop(moved_peer));
    with_world_services(&owner_world, || move_and_dispatch(&mut owner, 40, 45));
    assert_eq!(with_world_services(&owner_world, || draw(&mut owner)), (MouseCursor::Selecting, "owner".into(), 60.0, 45.0));
    with_world_services(&owner_world, || drop(owner));
}

#[test]
fn generic_and_callback_tooltips_target_only_the_receiving_mouse() {
    let mut owner = client();
    let mut peer = client();
    for (client, text) in [(&mut owner, "owner"), (&mut peer, "peer")] {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let window = manager
                    .create_window(None, 0, 0, 100, 100, effects)
                    .unwrap();
                window.borrow_mut().set_tooltip(text);
                manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0, effects);
            });
        });
    }
    assert_eq!(
        owner
            .input_mouse_mut()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "owner"
    );
    assert_eq!(
        peer.input_mouse_mut()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "peer"
    );
    peer.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager.destroy_all_windows(effects);
            let window = manager
                .create_window(None, 0, 0, 100, 100, effects)
                .unwrap();
            window.borrow_mut().set_tooltip_callback(|_, _, _, mouse| {
                mouse.set_cursor_tooltip("callback peer".into(), Some(7), None, None);
            });
            manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0, effects);
        });
    });
    assert_eq!(
        owner
            .input_mouse_mut()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "owner"
    );
    assert_eq!(
        peer.input_mouse_mut()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "callback peer"
    );
    drop(peer);
    assert_eq!(
        owner
            .input_mouse_mut()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "owner"
    );
}

#[test]
fn receiving_device_mode_changes_only_its_own_mouse_limits() {
    let mut owner = client();
    let mut peer = client();
    owner.with_window_event_effects(None, |effects| {
        let (display, mouse) = effects.script_display_and_mouse();
        assert!(
            game_client_rust::core::script_action_handler::apply_script_display_mode(
                display, 800, 600, 32, true, mouse
            )
        );
    });
    peer.with_window_event_effects(None, |effects| {
        let (display, mouse) = effects.script_display_and_mouse();
        assert!(
            game_client_rust::core::script_action_handler::apply_script_display_mode(
                display, 1024, 768, 32, true, mouse
            )
        );
    });
    owner
        .input_mouse_mut()
        .unwrap()
        .handle_mouse_move(1000.0, 700.0);
    peer.input_mouse_mut()
        .unwrap()
        .handle_mouse_move(1000.0, 700.0);
    assert_eq!(
        owner.input_mouse_mut().unwrap().state().position(),
        (800.0, 600.0)
    );
    assert_eq!(
        peer.input_mouse_mut().unwrap().state().position(),
        (1000.0, 700.0)
    );
}

#[test]
fn native_hover_clear_updates_only_its_actual_client_mouse() {
    let mut owner = client();
    let mut peer = client();
    owner
        .set_in_game_ui_subsystem(game_client_rust::core::subsystems::InGameUISubsystem::default());
    peer.set_in_game_ui_subsystem(game_client_rust::core::subsystems::InGameUISubsystem::default());
    owner
        .input_mouse_mut()
        .unwrap()
        .set_cursor_tooltip("owner stale".into(), Some(0), None, None);
    peer.input_mouse_mut()
        .unwrap()
        .set_cursor_tooltip("peer retained".into(), Some(0), None, None);
    owner.create_mouseover_hint(None, true, 17, &Default::default());
    assert_eq!(
        owner
            .input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        ""
    );
    assert_eq!(
        peer.input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "peer retained"
    );
    assert_eq!(
        owner
            .in_game_ui_subsystem()
            .unwrap()
            .get_moused_over_drawable_id(),
        0
    );
}

#[test]
fn stationary_control_bar_tooltip_uses_receiving_callback_sink() {
    use game_client_rust::gui::callbacks::control_bar_callbacks::ControlBarViewport;
    use game_client_rust::gui::control_bar::{ControlBar, HostControlBarInputProvenance};
    use game_client_rust::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource;
    use game_client_rust::gui::window_manager::ControlBarInputContext;

    struct Viewport;
    impl ControlBarViewport for Viewport {
        fn display_size(&self) -> (u32, u32) {
            (800, 600)
        }
        fn set_height_px(&mut self, _: i32) {}
    }

    let mut owner = client();
    let mut peer = client();
    peer.input_mouse_mut()
        .unwrap()
        .set_cursor_tooltip("peer".into(), None, None, None);
    owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let window = manager
                .create_window(None, 0, 0, 100, 100, effects)
                .unwrap();
            window
                .borrow_mut()
                .set_control_bar_tooltip_callback(|_, _, _, _| {});
        });
    });
    let window_owner = owner.window_manager_owner();
    let mut bar = ControlBar::new();
    let mut viewport = Viewport;
    let mut messages = Vec::new();
    let mut context = ControlBarInputContext {
        control_bar_observation: None,
        menu_audio: None,
        diplomacy_world: None,
        input_provenance: HostControlBarInputProvenance::InjectedOrUnknown,
        client_frame: Some(1),
        logic_frame: Some(1),
        ui: None,
        menus: None,
        ime: None,
        upgrade_tooltip: UpgradeTooltipSource::Standalone,
        camera_options: None,
        replay_menu: None,
        replay_version: None,
        observer_roster: None,
        control_bar: &mut bar,
        viewport: &mut viewport,
        replay: false,
        game_ending: false,
        tooltips_disabled: false,
        cursor_mouse: owner.input_mouse_mut(),
        window_manager_owner: Some(&window_owner),
        campaign_manager: None,
        menu_world: None,
        menu_host: None,
        window_videos: None,
        in_game_movies: None,
        script_display: None,
        main_menu: None,
        emitted_messages: Some(&mut messages),
    };
    window_owner.with_manager(|manager| manager.refresh_control_bar_tooltip(10, 10, &mut context));
    owner.complete_window_event_messages(messages, None);
    assert_eq!(
        peer.input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "peer"
    );
}
