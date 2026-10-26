use super::*;
use crate::gui::callbacks::menu_callbacks::MenuManager;
use crate::gui::window_manager::WindowLayout;

fn admit_controls(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> ChatWindows {
    let mut create = |name: &str| {
        manager
            .create_window_with_id(
                None,
                0,
                0,
                120,
                20,
                NameKeyGenerator::name_to_key(name) as i32,
                effects,
            )
            .unwrap()
    };
    let parent = create("InGameChat.wnd:ParentInGameChat");
    let text_entry = create("InGameChat.wnd:TextEntryChat");
    let chat_type_text = create("InGameChat.wnd:StaticTextChatType");
    let layout = Rc::new(RefCell::new(WindowLayout::new("InGameChat.wnd".into())));
    layout.borrow_mut().set_self_handle(&layout);
    for window in [&parent, &text_entry, &chat_type_text] {
        layout.borrow_mut().add_window(window.clone());
    }
    let windows = ChatWindows {
        layout,
        parent,
        text_entry,
        chat_type_text,
    };
    effects.with_ingame_ui(|owner| owner.chat.ui.phase = ChatLayoutPhase::Live(windows.clone()));
    windows
}

#[test]
fn same_window_ids_do_not_admit_a_peer_callback_or_reset_peer_state() {
    let mut first = MenuManager::new();
    let mut second = MenuManager::new();
    let mut first_manager = WindowManager::new();
    let mut second_manager = WindowManager::new();
    let mut first_messages = Vec::new();
    let mut second_messages = Vec::new();
    let mut first_effects = WindowEventEffects::new(&mut first_messages);
    let mut second_effects = WindowEventEffects::new(&mut second_messages);
    first_effects.menus = Some(first.receiving_menus(ClientInputAuthority::NativeHosted));
    second_effects.menus = Some(second.receiving_menus(ClientInputAuthority::NativeHosted));
    let first_windows = admit_controls(&mut first_manager, &mut first_effects);
    let second_windows = admit_controls(&mut second_manager, &mut second_effects);
    first_effects.manager = Some(&mut first_manager);
    second_effects.manager = Some(&mut second_manager);
    first_effects.with_ingame_ui(|owner| {
        owner.chat.ui.saved_text = "first draft".into();
        owner.chat.chat_type = InGameChatType::Players;
    });
    assert_eq!(
        first_windows.text_entry.borrow().get_id(),
        second_windows.text_entry.borrow().get_id()
    );
    assert!(live_callback(
        &first_windows.text_entry.borrow(),
        &mut first_effects
    ));
    assert!(!live_callback(
        &second_windows.text_entry.borrow(),
        &mut first_effects
    ));
    second_effects
        .with_manager(|manager, effects| reset_in_game_chat(manager, effects))
        .unwrap();
    first_effects.with_ingame_ui(|owner| {
        assert_eq!(owner.chat.ui.saved_text, "first draft");
        assert_eq!(owner.chat.chat_type, InGameChatType::Players);
        assert!(matches!(owner.chat.ui.phase, ChatLayoutPhase::Live(_)));
    });
    assert!(live_callback(
        &first_windows.text_entry.borrow(),
        &mut first_effects
    ));
    assert!(!live_callback(
        &second_windows.text_entry.borrow(),
        &mut first_effects
    ));
}

#[test]
fn native_facts_are_required_and_never_fall_back_to_a_core_roster() {
    let mut owner = MenuManager::new();
    let mut messages = Vec::new();
    let mut effects = WindowEventEffects::new(&mut messages);
    effects.menus = Some(owner.receiving_menus(ClientInputAuthority::NativeHosted));
    assert!(chat_world(&mut effects).is_err());
    assert!(should_block_chat(&effects).is_err());
    effects.with_ingame_ui(|owner| assert!(matches!(owner.chat.ui.phase, ChatLayoutPhase::Absent)));
}
