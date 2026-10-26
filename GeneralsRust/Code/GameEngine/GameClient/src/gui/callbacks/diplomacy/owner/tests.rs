use super::*;
use crate::gui::callbacks::menu_callbacks::MenuManager;

#[test]
fn same_ids_retire_only_the_receiving_layout_and_keep_briefing() {
    let mut first = MenuManager::new();
    let mut peer = MenuManager::new();
    let mut first_manager = WindowManager::new();
    let mut peer_manager = WindowManager::new();
    let mut a_messages = Vec::new();
    let mut b_messages = Vec::new();
    let mut a = WindowEventEffects::new(&mut a_messages);
    let mut b = WindowEventEffects::new(&mut b_messages);
    a.menus = Some(first.receiving_menus(ClientInputAuthority::NativeHosted));
    b.menus = Some(peer.receiving_menus(ClientInputAuthority::NativeHosted));
    let id = NameKeyGenerator::name_to_key("Diplomacy.wnd:Root") as i32;
    let first_window = first_manager
        .create_window_with_id(None, 0, 0, 100, 100, id, &mut a)
        .unwrap();
    let peer_window = peer_manager
        .create_window_with_id(None, 0, 0, 100, 100, id, &mut b)
        .unwrap();
    for (effects, window, label) in [
        (&mut a, &first_window, "SCRIPT:First"),
        (&mut b, &peer_window, "SCRIPT:Peer"),
    ] {
        let layout = Rc::new(RefCell::new(WindowLayout::new("Diplomacy.wnd".into())));
        layout.borrow_mut().set_self_handle(&layout);
        layout.borrow_mut().add_window(window.clone());
        effects.with_diplomacy(|owner| {
            owner.ui.layout = Some(layout);
            owner.ui.window = Some(window.clone());
            owner.briefing_list.push(label.into());
            owner.active = true;
        });
    }
    a.manager = Some(&mut first_manager);
    b.manager = Some(&mut peer_manager);
    assert!(canonical(&first_window.borrow(), &mut a));
    assert!(!canonical(&peer_window.borrow(), &mut a));
    b.with_manager(|manager, effects| reset_diplomacy(manager, effects))
        .unwrap();
    assert!(!canonical(&peer_window.borrow(), &mut b));
    a.with_diplomacy(|owner| {
        assert!(is_diplomacy_active(owner));
        assert_eq!(get_briefing_text_list(owner), vec!["SCRIPT:First"]);
    });
    b.with_diplomacy(|owner| {
        assert!(!is_diplomacy_active(owner));
        assert_eq!(get_briefing_text_list(owner), vec!["SCRIPT:Peer"]);
    });
    a.with_manager(|manager, effects| hide_diplomacy(true, manager, effects))
        .unwrap();
    assert!(first_window.borrow().is_hidden());
    assert!(!first_window.borrow().is_enabled());
    assert_eq!(
        diplomacy_input_receiving(
            &mut first_window.borrow_mut(),
            WindowMessage::Char,
            b'A' as WindowMsgData,
            0,
            &mut a
        ),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        diplomacy_input_receiving(
            &mut peer_window.borrow_mut(),
            WindowMessage::Char,
            KEY_ESC,
            0,
            &mut a
        ),
        WindowMsgHandled::Ignored
    );
}
#[test]
fn native_missing_facts_does_not_discover_an_unrelated_core_world() {
    let mut menus = MenuManager::new();
    let mut messages = Vec::new();
    let mut effects = WindowEventEffects::new(&mut messages);
    effects.menus = Some(menus.receiving_menus(ClientInputAuthority::NativeHosted));
    assert!(world(&mut effects).is_err());
}
