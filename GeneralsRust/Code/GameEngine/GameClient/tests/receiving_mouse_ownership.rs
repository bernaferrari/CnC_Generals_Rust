use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::helpers::{is_mouse_cursor_visible, set_mouse_cursor_visibility};
#[test]
fn native_clients_receive_distinct_virtual_mouse_devices() {
    let mut owner =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    owner.init_input_subsystems().unwrap();
    owner
        .input_mouse_mut()
        .unwrap()
        .handle_mouse_move(73.0, 81.0);
    owner
        .input_mouse_mut()
        .unwrap()
        .set_cursor_tooltip("owner".into(), Some(0), None, None);
    set_mouse_cursor_visibility(owner.input_mouse_mut().unwrap(), false);
    let mut peer =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    peer.init_input_subsystems().unwrap();
    assert!(!std::ptr::eq(
        owner.input_mouse().unwrap(),
        peer.input_mouse().unwrap()
    ));
    peer.input_mouse_mut().unwrap().handle_mouse_move(5.0, 9.0);
    set_mouse_cursor_visibility(peer.input_mouse_mut().unwrap(), true);
    peer.reset().unwrap();
    assert!(is_mouse_cursor_visible(peer.input_mouse().unwrap()));
    drop(peer);
    assert_eq!(
        owner.input_mouse().unwrap().state().position(),
        (73.0, 81.0)
    );
    assert_eq!(
        owner
            .input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "owner"
    );
    assert!(!is_mouse_cursor_visible(owner.input_mouse().unwrap()));
}
