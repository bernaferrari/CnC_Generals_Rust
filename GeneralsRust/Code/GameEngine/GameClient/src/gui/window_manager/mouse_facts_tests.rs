//! Actual WindowManager callbacks use receiving camera facts, never a peer's View.
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

fn packed(x: i32, y: i32) -> WindowMsgData {
    (((y as u32) << 16) | ((x as u32) & 0xFFFF)) as WindowMsgData
}

fn receiver() -> (
    WindowManager,
    Rc<RefCell<Vec<(WindowMessage, WindowMsgData)>>>,
) {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&calls);
    window
        .borrow_mut()
        .set_input_callback(move |_, msg, data, _| {
            recorded.borrow_mut().push((msg, data));
            WindowMsgHandled::Handled
        });
    manager.capture_mouse(&window).unwrap();
    (manager, calls)
}

fn deliver(
    manager: &mut WindowManager,
    facts: WindowMouseFacts,
    msg: WindowMessage,
) -> WindowInputReturnCode {
    dispatch_mouse_with_facts(
        msg,
        12,
        23,
        facts,
        |msg, x, y, data| manager.process_mouse_event(msg, x, y, data),
        || false,
    )
}

#[test]
fn receiving_managers_keep_scrolling_lmb_exception_and_peer_independence() {
    let (mut first, first_calls) = receiver();
    let (mut peer, peer_calls) = receiver();
    let scrolling = WindowMouseFacts {
        mouse_locked: true,
        scrolling: true,
    };
    let unlocked = WindowMouseFacts::default();
    assert_eq!(
        deliver(&mut first, scrolling, WindowMessage::RightDown),
        WindowInputReturnCode::NotUsed
    );
    assert_eq!(
        deliver(&mut peer, unlocked, WindowMessage::RightDown),
        WindowInputReturnCode::Used
    );
    assert!(first_calls.borrow().is_empty());
    let packed = ((23_u32 << 16) | 12) as WindowMsgData;
    assert_eq!(
        peer_calls.borrow().as_slice(),
        &[
            (WindowMessage::RightDown, packed),
            (WindowMessage::MouseEntering, packed),
        ]
    );
    for msg in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
        assert_eq!(
            deliver(&mut first, scrolling, msg),
            WindowInputReturnCode::Used
        );
    }
    assert_eq!(
        first_calls.borrow().as_slice(),
        &[
            (WindowMessage::LeftDown, packed),
            (WindowMessage::MouseEntering, packed),
            (WindowMessage::LeftUp, packed),
        ]
    );
    for (_, data) in first_calls.borrow().iter() {
        assert_eq!(*data, ((23_u32 << 16) | 12) as WindowMsgData);
    }
    for msg in [
        WindowMessage::MousePos,
        WindowMessage::MiddleDown,
        WindowMessage::WheelUp,
    ] {
        assert_eq!(
            deliver(&mut first, scrolling, msg),
            WindowInputReturnCode::NotUsed
        );
        assert_eq!(
            deliver(&mut peer, unlocked, msg),
            WindowInputReturnCode::Used
        );
    }
    assert_eq!(
        first_calls.borrow().as_slice(),
        &[
            (WindowMessage::LeftDown, packed),
            (WindowMessage::MouseEntering, packed),
            (WindowMessage::LeftUp, packed),
        ]
    );
    assert_eq!(
        peer_calls.borrow().as_slice(),
        &[
            (WindowMessage::RightDown, packed),
            (WindowMessage::MouseEntering, packed),
            (WindowMessage::MousePos, packed),
            (WindowMessage::MiddleDown, packed),
            (WindowMessage::WheelUp, packed),
        ]
    );
    assert_eq!(
        deliver(
            &mut first,
            WindowMouseFacts {
                mouse_locked: true,
                scrolling: false
            },
            WindowMessage::LeftDown
        ),
        WindowInputReturnCode::NotUsed
    );
}

#[test]
fn public_native_dispatch_ignores_stale_foreign_tactical_mouse_lock() {
    let owner = crate::gui::window_manager::WindowManagerOwner::new();
    owner.with_scope(|| {
        // Existing TLS services are restored on unwind; no new registry or test mutex.
        struct Restore {
            manager: Option<WindowManager>,
            locked: bool,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                crate::display::view::with_tactical_view(|view| view.set_mouse_lock(self.locked));
                if let Some(previous) = self.manager.take() {
                    with_window_manager(|manager| *manager = previous);
                }
            }
        }
        let (manager, calls) = receiver();
        let mut previous = None;
        with_window_manager(|current| previous = Some(std::mem::replace(current, manager)));
        let previous = previous.expect("receiving manager is available outside reentrant dispatch");
        let locked = crate::display::view::with_tactical_view_ref(|view| view.is_mouse_locked());
        let _restore = Restore {
            manager: Some(previous),
            locked,
        };
        crate::display::view::with_tactical_view(|view| view.set_mouse_lock(true));
        // OLD global-discovery route blocks this event regardless of scrolling.
        assert_eq!(
            dispatch_os_mouse_to_window_manager(WindowMessage::RightDown, 12, 23),
            WindowInputReturnCode::NotUsed
        );
        assert!(calls.borrow().is_empty());
        for msg in [
            WindowMessage::RightDown,
            WindowMessage::MousePos,
            WindowMessage::WheelUp,
        ] {
            assert_eq!(
                dispatch_os_mouse_to_window_manager_with_facts(
                    msg,
                    12,
                    23,
                    WindowMouseFacts::default()
                ),
                WindowInputReturnCode::Used
            );
        }
        let packed = ((23_u32 << 16) | 12) as WindowMsgData;
        assert_eq!(
            calls.borrow().as_slice(),
            &[
                (WindowMessage::RightDown, packed),
                (WindowMessage::MouseEntering, packed),
                (WindowMessage::MousePos, packed),
                (WindowMessage::WheelUp, packed),
            ]
        );
        crate::display::view::with_tactical_view(|view| view.set_mouse_lock(false));
        assert_eq!(
            dispatch_os_mouse_to_window_manager_with_facts(
                WindowMessage::RightDown,
                12,
                23,
                WindowMouseFacts {
                    mouse_locked: true,
                    scrolling: true
                }
            ),
            WindowInputReturnCode::NotUsed
        );
        let packed = ((23_u32 << 16) | 12) as WindowMsgData;
        assert_eq!(
            calls.borrow().as_slice(),
            &[
                (WindowMessage::RightDown, packed),
                (WindowMessage::MouseEntering, packed),
                (WindowMessage::MousePos, packed),
                (WindowMessage::WheelUp, packed),
            ]
        );
    });
}

#[test]
fn shell_override_occurs_after_dispatch_and_never_after_lock_rejection() {
    let calls = std::cell::Cell::new(0);
    let result = dispatch_mouse_with_facts(
        WindowMessage::MousePos,
        1,
        2,
        WindowMouseFacts::default(),
        |_, _, _, _| {
            calls.set(calls.get() + 1);
            WindowInputReturnCode::NotUsed
        },
        || true,
    );
    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(calls.get(), 1);
    let result = dispatch_mouse_with_facts(
        WindowMessage::MousePos,
        1,
        2,
        WindowMouseFacts {
            mouse_locked: true,
            scrolling: true,
        },
        |_, _, _, _| panic!("blocked event dispatched"),
        || panic!("blocked event queried shell"),
    );
    assert_eq!(result, WindowInputReturnCode::NotUsed);
}

#[test]
fn hover_callbacks_keep_cpp_packed_screen_coordinates_and_order() {
    let mut manager = WindowManager::new();
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    for (name, window) in [("first", first), ("second", second)] {
        let seen = Rc::clone(&seen);
        window
            .borrow_mut()
            .set_input_callback(move |_, msg, data, _| {
                seen.borrow_mut().push((name, msg, data));
                WindowMsgHandled::Handled
            });
    }
    manager.process_mouse_event(WindowMessage::MousePos, 10, 20, packed(10, 20));
    manager.process_mouse_event(WindowMessage::MousePos, 210, 30, packed(210, 30));
    manager.process_mouse_event(WindowMessage::MousePos, -4, 12, packed(-4, 12));
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            ("first", WindowMessage::MousePos, packed(10, 20)),
            ("first", WindowMessage::MouseEntering, packed(10, 20)),
            ("second", WindowMessage::MousePos, packed(210, 30)),
            ("first", WindowMessage::MouseLeaving, packed(210, 30)),
            ("second", WindowMessage::MouseEntering, packed(210, 30)),
            ("second", WindowMessage::MouseLeaving, packed(-4, 12)),
        ],
        "CPP hover callbacks must preserve packed event coordinates, not zero"
    );
}

#[test]
fn native_dispatch_uses_explicit_receiving_game_client_for_same_id_windows() {
    fn receiving_client() -> (
        crate::gui::window_manager::WindowManagerOwner,
        Rc<RefCell<Vec<WindowMessage>>>,
        i32,
    ) {
        let client = crate::core::game_client::GameClient::new().unwrap();
        let owner = client.window_manager_owner();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&calls);
        let id = owner.with_manager(|manager| {
            let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
            window.borrow_mut().set_input_callback(move |_, msg, _, _| {
                recorded.borrow_mut().push(msg);
                WindowMsgHandled::Handled
            });
            manager.capture_mouse(&window).unwrap();
            window.borrow().id
        });
        (owner, calls, id)
    }

    let (first, first_calls, first_id) = receiving_client();
    let (peer, peer_calls, peer_id) = receiving_client();
    assert_eq!(first_id, peer_id, "each client starts with the same WND id");

    let empty_client = crate::core::game_client::GameClient::new().unwrap();
    let empty_owner = empty_client.window_manager_owner();
    peer.with_shell_mut(|shell| shell.set_shell_active(false));
    first.with_manager(|_| {
        assert_eq!(
            first.try_with_manager_ref(|_| ()),
            None,
            "owner query must report an active mutable borrow instead of reading a dummy tree"
        );
    });

    peer.with_scope(|| {
        assert_eq!(
            dispatch_os_mouse_to_window_manager_for_owner(
                &empty_owner,
                WindowMessage::MousePos,
                12,
                23,
                WindowMouseFacts::default(),
            ),
            WindowInputReturnCode::Used,
            "Shell-active override must come from the explicit receiver, not its selected peer"
        );
    });

    // Calling the first receiver while the peer is selected must temporarily
    // dispatch against the first tree and restore the peer's nested callback owner.
    peer.with_scope(|| {
        assert_eq!(
            dispatch_os_mouse_to_window_manager_for_owner(
                &first,
                WindowMessage::RightDown,
                12,
                23,
                WindowMouseFacts::default(),
            ),
            WindowInputReturnCode::Used
        );
        assert_eq!(
            with_window_manager(|manager| manager.window_count()),
            1,
            "explicit dispatch must restore the peer owner after callback"
        );
    });
    assert_eq!(
        dispatch_os_mouse_to_window_manager_for_owner(
            &peer,
            WindowMessage::LeftDown,
            12,
            23,
            WindowMouseFacts::default(),
        ),
        WindowInputReturnCode::Used
    );

    assert!(first_calls.borrow().contains(&WindowMessage::RightDown));
    assert!(!first_calls.borrow().contains(&WindowMessage::LeftDown));
    assert!(peer_calls.borrow().contains(&WindowMessage::LeftDown));
    assert!(!peer_calls.borrow().contains(&WindowMessage::RightDown));
}

#[test]
fn native_key_dispatch_uses_explicit_receiving_game_client_focus() {
    fn receiving_client() -> (
        crate::gui::window_manager::WindowManagerOwner,
        Rc<RefCell<Vec<(WindowMessage, WindowMsgData, WindowMsgData)>>>,
        i32,
    ) {
        let client = crate::core::game_client::GameClient::new().unwrap();
        let owner = client.window_manager_owner();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&calls);
        let id = owner.with_manager(|manager| {
            let window = manager.create_window(None, 0, 0, 100, 40).unwrap();
            window
                .borrow_mut()
                .set_system_callback(|_, msg, data1, data2| {
                    if msg == WindowMessage::InputFocus && data1 != 0 {
                        write_input_focus_response(data1, data2, true)
                    } else {
                        WindowMsgHandled::Ignored
                    }
                });
            window
                .borrow_mut()
                .set_input_callback(move |_, msg, data1, data2| {
                    recorded.borrow_mut().push((msg, data1, data2));
                    WindowMsgHandled::Handled
                });
            manager.set_focus(Some(&window)).unwrap();
            window.borrow().id
        });
        (owner, calls, id)
    }

    let (first, first_calls, first_id) = receiving_client();
    let (peer, peer_calls, peer_id) = receiving_client();
    assert_eq!(first_id, peer_id, "clients have matching first WND ids");

    let empty_client = crate::core::game_client::GameClient::new().unwrap();
    let empty_owner = empty_client.window_manager_owner();
    assert_eq!(
        dispatch_os_key_to_window_manager_for_owner(&empty_owner, 0x1B, 0x01),
        WindowInputReturnCode::NotUsed,
        "CPP does not consume an unhandled key solely because Shell is active"
    );

    peer.with_scope(|| {
        assert_eq!(
            dispatch_os_key_to_window_manager_for_owner(&first, 13, 0x02),
            WindowInputReturnCode::Used
        );
        assert_eq!(
            with_window_manager(|manager| manager.window_count()),
            1,
            "explicit keyboard dispatch restores peer selection after focus delivery"
        );
    });
    assert_eq!(
        dispatch_os_key_to_window_manager_for_owner(&peer, 27, 0x01),
        WindowInputReturnCode::Used
    );

    assert!(
        first_calls
            .borrow()
            .contains(&(WindowMessage::Char, 13, 0x02))
    );
    assert!(
        !first_calls
            .borrow()
            .contains(&(WindowMessage::Char, 27, 0x01))
    );
    assert!(
        peer_calls
            .borrow()
            .contains(&(WindowMessage::Char, 27, 0x01))
    );
    assert!(
        !peer_calls
            .borrow()
            .contains(&(WindowMessage::Char, 13, 0x02))
    );
}

#[test]
fn native_wnd_queries_use_the_explicit_receiving_client_tree() {
    fn receiving_client(name: &str, x: i32) -> (WindowManagerOwner, i32) {
        let client = crate::core::game_client::GameClient::new().unwrap();
        let owner = client.window_manager_owner();
        let id = owner.with_manager(|manager| {
            let window = manager.create_window(None, x, 0, 100, 100).unwrap();
            window.borrow_mut().set_name(name);
            window.borrow().id
        });
        (owner, id)
    }

    let (first, first_id) = receiving_client("first-client-window", 0);
    let (peer, peer_id) = receiving_client("peer-client-window", 200);
    assert_eq!(first_id, peer_id, "clients use the same first WND id");

    peer.with_scope(|| {
        assert_eq!(
            crate::gui::shell::main_menu::os_wnd_widget_under_cursor_name_for_owner(&first, 10, 10)
                .flatten()
                .as_deref(),
            Some("first-client-window")
        );
        assert_eq!(
            crate::gui::shell::main_menu::os_wnd_widget_under_cursor_name_for_owner(&peer, 210, 10)
                .flatten()
                .as_deref(),
            Some("peer-client-window")
        );
        assert_eq!(
            crate::gui::shell::main_menu::os_wnd_widget_under_cursor_name_for_owner(&peer, 10, 10),
            Some(None),
            "peer query must not observe the first client's same-id window"
        );
    });
}
