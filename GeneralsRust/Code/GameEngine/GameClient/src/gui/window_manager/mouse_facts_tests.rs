//! Actual WindowManager callbacks use receiving camera facts, never a peer's View.
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

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
    assert_eq!(peer_calls.borrow().len(), 1);
    for msg in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
        assert_eq!(
            deliver(&mut first, scrolling, msg),
            WindowInputReturnCode::Used
        );
    }
    assert_eq!(first_calls.borrow().len(), 2);
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
    assert_eq!(first_calls.borrow().len(), 2);
    assert_eq!(peer_calls.borrow().len(), 4);
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
    assert_eq!(calls.borrow().len(), 3);
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
    assert_eq!(calls.borrow().len(), 3);
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
