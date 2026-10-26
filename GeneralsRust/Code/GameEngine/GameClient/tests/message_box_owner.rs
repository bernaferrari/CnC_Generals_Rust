use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::callbacks::message_box::*;
use game_client_rust::gui::{WindowMessage, WindowMsgHandled};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;
fn client() -> GameClient {
    GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap()
}
#[test]
fn nested_confirmation_preserves_same_id_replacement_and_peer() {
    let mut owner = client();
    let mut peer = client();
    let events = Rc::new(RefCell::new(Vec::new()));
    let peer_window = peer.with_window_event_effects(None, |effects| {
        effects
            .with_manager(|m, e| message_box_yes_no("Peer", "Peer body", None, None, m, e).unwrap())
    });
    let trace = events.clone();
    let old = owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|m, e| {
            message_box_yes_no(
                "Original",
                "Original body",
                Some(Box::new(move |effects| {
                    trace.borrow_mut().push("callback");
                    effects.with_message_boxes(|o| {
                        assert!(o.get_standard().is_visible());
                        assert!(o.get_standard().get_result().is_none());
                    });
                    effects.with_manager(|m, e| {
                        message_box_yes_no("Replacement", "Replacement body", None, None, m, e)
                            .unwrap();
                    });
                    trace.borrow_mut().push("replacement created");
                })),
                None,
                m,
                e,
            )
            .unwrap()
        })
    });
    let id = old.borrow().get_id();
    owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|m, e| {
            assert_eq!(
                m.send_system_message(
                    &old,
                    WindowMessage::GadgetSelected,
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonYes") as usize,
                    0,
                    e
                ),
                WindowMsgHandled::Handled
            );
            assert_eq!(
                events.borrow().as_slice(),
                ["callback", "replacement created"]
            );
        })
    });
    let owner_tree = owner.window_manager_owner();
    owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|m, e| m.flush_destroy_queue(e))
    });
    let replacement = owner_tree.with_manager_ref(|m| m.get_window_by_id(id).unwrap());
    assert!(!Rc::ptr_eq(&old, &replacement));
    assert_eq!(
        owner.message_boxes().get_standard().get_title(),
        "Replacement"
    );
    assert!(owner.message_boxes().get_standard().is_visible());
    assert_eq!(owner.message_boxes().get_standard().get_result(), None);
    assert_eq!(peer.message_boxes().get_standard().get_title(), "Peer");
    assert!(
        peer.window_manager_owner()
            .with_manager_ref(|m| m.get_window_by_id(peer_window.borrow().get_id()).is_some())
    );
    drop(peer);
    assert_eq!(
        owner.message_boxes().get_standard().get_title(),
        "Replacement"
    );
}
#[test]
fn extended_keep_open_runs_before_dispatch_returns() {
    let mut owner = client();
    let calls = Rc::new(RefCell::new(0));
    let seen = calls.clone();
    let window = owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|m, e| {
            ex_message_box_ok(
                "Extended",
                "Body",
                None,
                Some(Box::new(move |_, _| {
                    *seen.borrow_mut() += 1;
                    MessageBoxReturnType::KeepOpen
                })),
                m,
                e,
            )
            .unwrap()
        })
    });
    owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|m, e| {
            m.send_system_message(
                &window,
                WindowMessage::GadgetSelected,
                NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonOk") as usize,
                0,
                e,
            );
            assert_eq!(*calls.borrow(), 1);
        })
    });
    assert!(owner.message_boxes().get_extended().is_visible());
    assert_eq!(
        owner.message_boxes().get_extended().get_result(),
        Some(MessageBoxResult::Ok)
    );
    owner.with_window_event_effects(None, |effects| {
        effects.with_manager(|m, e| m.flush_destroy_queue(e))
    });
    assert!(
        owner
            .window_manager_owner()
            .with_manager_ref(|m| m.get_window_by_id(window.borrow().get_id()).is_some())
    );
}
