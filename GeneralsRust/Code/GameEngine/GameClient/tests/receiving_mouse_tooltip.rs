use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::WindowMessage;
fn client() -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.init_input_subsystems().unwrap();
    client
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
