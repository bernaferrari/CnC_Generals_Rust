use game_client_rust::input::keyboard::{KeyCode, Keyboard};

#[test]
fn key_repeat_uses_cpp_frame_delay_order_and_receiving_clock() {
    let mut first = Keyboard::new();
    let mut second = Keyboard::new();
    assert!(first.handle_key_simple(KeyCode::A, true));
    assert!(first.handle_key_simple(KeyCode::Q, true));
    assert!(second.handle_key_simple(KeyCode::B, true));

    for _ in 0..10 {
        assert!(first.update().is_empty(), "first repeat waits 10 frames");
        assert!(second.update().is_empty(), "peer repeat has its own clock");
    }

    // C++ Keyboard::checkKeyRepeat scans DIK codes numerically, so Q (0x10)
    // precedes A (0x1e), regardless of the order received by the Rust map.
    assert_eq!(first.update(), vec![KeyCode::Q]);
    assert_eq!(second.update(), vec![KeyCode::B]);

    // C++ backdates the selected sequence by KEY_REPEAT_DELAY + 2; because it
    // tests `> KEY_REPEAT_DELAY`, the next repeat is eligible on the next tick.
    assert_eq!(first.update(), vec![KeyCode::Q]);
    assert_eq!(second.update(), vec![KeyCode::B]);

    assert!(first.handle_key_simple(KeyCode::Q, false));
    for _ in 0..10 {
        assert!(first.update().is_empty());
    }
    assert_eq!(first.update(), vec![KeyCode::A]);
}
