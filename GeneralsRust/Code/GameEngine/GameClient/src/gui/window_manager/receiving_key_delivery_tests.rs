use super::*;

#[test]
fn receiving_key_sender_finishes_before_next_ancestor_and_peer_keeps_same_ids() {
    fn world() -> (
        WindowManager,
        Rc<RefCell<GameWindow>>,
        Rc<RefCell<GameWindow>>,
    ) {
        let mut manager = WindowManager::new();
        let parent = manager
            .create_window_with_id(None, 0, 0, 100, 100, 101)
            .unwrap();
        let child = manager
            .create_window_with_id(Some(&parent), 0, 0, 40, 40, 102)
            .unwrap();
        for window in [&parent, &child] {
            window
                .borrow_mut()
                .set_system_callback(|_, message, data1, data2| {
                    if message == WindowMessage::InputFocus {
                        return write_input_focus_response(data1, data2, data1 != 0);
                    }
                    WindowMsgHandled::Ignored
                });
        }
        manager.set_focus(Some(&child)).unwrap();
        assert!(
            manager
                .get_focus()
                .is_some_and(|focus| Rc::ptr_eq(&focus, &child)),
            "fixture must acquire real accepted focus before key dispatch"
        );
        (manager, parent, child)
    }
    let (mut a, parent_a, child_a) = world();
    let (mut b, parent_b, child_b) = world();
    assert!(Rc::ptr_eq(&a.get_focus().unwrap(), &child_a));
    assert!(Rc::ptr_eq(&b.get_focus().unwrap(), &child_b));
    let mut visits_a = Vec::new();
    let mut visits_b = Vec::new();
    let result = a.process_key_event_with_receiving_sender(
        13,
        1,
        &mut |manager, sender, message, _, _, _| {
            assert_eq!(message, WindowMessage::Char);
            let id = sender.borrow().get_id();
            visits_a.push(id);
            if id == 102 {
                parent_a
                    .borrow_mut()
                    .set_text("child continuation complete")
                    .unwrap();
                manager.set_focus(Some(&parent_a)).unwrap();
                let peer = b.process_key_event_with_receiving_sender(
                    13,
                    1,
                    &mut |_, sender, _, _, _, _| {
                        visits_b.push(sender.borrow().get_id());
                        assert_ne!(parent_b.borrow().get_text(), "child continuation complete");
                        WindowMsgHandled::Handled
                    },
                );
                assert_eq!(peer, WindowInputReturnCode::Used);
                WindowMsgHandled::Ignored
            } else {
                assert_eq!(id, 101);
                assert_eq!(parent_a.borrow().get_text(), "child continuation complete");
                WindowMsgHandled::Handled
            }
        },
    );
    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(visits_a, vec![102, 101]);
    assert_eq!(visits_b, vec![102]);
    assert!(Rc::ptr_eq(&a.get_focus().unwrap(), &parent_a));
    assert!(!Rc::ptr_eq(&a.get_focus().unwrap(), &child_a));
    assert_eq!(b.get_focus().unwrap().borrow().get_id(), 102);
}
