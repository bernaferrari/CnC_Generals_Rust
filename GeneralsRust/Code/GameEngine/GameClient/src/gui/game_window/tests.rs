//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use crate::gui::gadgets::RadioButtonGroup;
use crate::gui::gadgets::Rect;
use crate::gui::gadgets::tabcontrol;
use crate::gui::shell::Color as ShellColor;

use super::prelude::*;
use super::*;

fn combo_fixture() -> (
    GameWindow,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
) {
    let mut combo = GameWindow::new();
    combo.set_id(1);
    combo.set_size(120, 20).unwrap();
    combo.set_widget(WindowWidget::ComboBox(ComboBox::new(1, 0, 0, 120, 20)));

    let edit_box = Rc::new(RefCell::new(GameWindow::new()));
    {
        let mut edit_box = edit_box.borrow_mut();
        edit_box.set_id(2);
        edit_box.set_size(120, 20).unwrap();
        edit_box.set_widget(WindowWidget::TextEntry(TextEntry::new(2, 0, 0, 120, 20)));
    }

    let list_box = Rc::new(RefCell::new(GameWindow::new()));
    {
        let mut list_box = list_box.borrow_mut();
        list_box.set_id(3);
        list_box.set_size(120, 40).unwrap();
        list_box.set_widget(WindowWidget::ListBox(ListBox::new(3, 0, 20, 120, 40)));
        list_box.hide(true).unwrap();
    }

    let drop_down = Rc::new(RefCell::new(GameWindow::new()));
    drop_down.borrow_mut().set_id(4);

    combo.add_child(edit_box.clone());
    combo.add_child(list_box.clone());
    combo.add_child(drop_down.clone());
    combo.set_combobox_links(ComboBoxLinks {
        drop_down: 4,
        edit_box: 2,
        list_box: 3,
    });

    (combo, edit_box, list_box, drop_down)
}

#[test]
fn undefined_window_color_matches_cpp_transparent_white_sentinel() {
    assert_eq!(WIN_COLOR_UNDEFINED, 0x00FF_FFFF);
}

#[test]
fn window_instance_data_defaults_match_cpp_init() {
    let data = WindowInstanceData::default();
    assert_eq!(data.tooltip_delay, -1);
    assert_eq!(data.enabled_draw_data[0].color, WIN_COLOR_UNDEFINED);
    assert_eq!(data.enabled_draw_data[0].border_color, WIN_COLOR_UNDEFINED);
    assert_eq!(data.enabled_text.color, WIN_COLOR_UNDEFINED);
    assert_eq!(data.disabled_text.color, WIN_COLOR_UNDEFINED);
    assert_eq!(data.hilite_text.color, WIN_COLOR_UNDEFINED);
}

#[test]
fn legacy_virtual_keys_map_to_gadget_key_codes_like_cpp() {
    assert_eq!(map_keycode(0x21), KeyCode::PageUp);
    assert_eq!(map_keycode(0x22), KeyCode::PageDown);
    assert_eq!(map_keycode(0x23), KeyCode::End);
    assert_eq!(map_keycode(0x24), KeyCode::Home);
    assert_eq!(map_keycode(0x25), KeyCode::Left);
    assert_eq!(map_keycode(0x26), KeyCode::Up);
    assert_eq!(map_keycode(0x27), KeyCode::Right);
    assert_eq!(map_keycode(0x28), KeyCode::Down);
}

#[test]
fn static_text_label_system_messages_match_cpp() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::StaticText(StaticText::new(7, 0, 0, 120, 24)));

    assert_eq!(
        with_payload(
            WindowMsgPayload::Text("Mission Objective".to_string()),
            |token| { window.send_system_message(WindowMessage::User(GGM_SET_LABEL), token, 0) },
        ),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.get_text(), "Mission Objective");
    assert_eq!(
        window.widget().and_then(|widget| match widget {
            WindowWidget::StaticText(static_text) => Some(static_text.text()),
            _ => None,
        }),
        Some("Mission Objective")
    );

    let token = push_payload(WindowMsgPayload::Text(String::new()));
    assert_eq!(
        window.send_system_message(WindowMessage::User(GGM_GET_LABEL), 0, token),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        pop_payload(token),
        Some(WindowMsgPayload::Text("Mission Objective".to_string()))
    );
}

#[test]
fn garbage_window_msg_data_is_not_a_payload_and_does_not_panic() {
    for garbage in [
        0usize,
        1,
        KEY_STATE_DOWN,
        (-1i32) as WindowMsgData,
        i32::MAX as WindowMsgData,
        0xDEAD_BEEF,
        usize::MAX,
        usize::MAX - 7,
    ] {
        assert!(!is_window_msg_payload(garbage), "garbage={garbage:#x}");
        assert_eq!(payload(garbage), None);
        assert_eq!(pop_payload(garbage), None);
        assert!(!replace_payload(garbage, WindowMsgPayload::Bool(true)));
    }

    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::StaticText(StaticText::new(7, 0, 0, 120, 24)));
    assert_eq!(
        window.send_system_message(WindowMessage::User(GGM_SET_LABEL), 0xDEAD_BEEF, 0,),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.get_text(), "");
    assert_eq!(
        window.send_system_message(WindowMessage::User(GGM_GET_LABEL), 0, 0xDEAD_BEEF,),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 1, 0xDEAD_BEEF),
        WindowMsgHandled::Ignored
    );
    assert_eq!(
        write_input_focus_response(1, 0xDEAD_BEEF, true),
        WindowMsgHandled::Handled
    );
}

#[test]
fn typed_text_payload_roundtrip_works() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::StaticText(StaticText::new(8, 0, 0, 120, 24)));

    assert_eq!(
        with_payload(WindowMsgPayload::Text("Zero Hour".to_string()), |token| {
            window.send_system_message(WindowMessage::User(GGM_SET_LABEL), token, 0)
        }),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.get_text(), "Zero Hour");

    let (handled, out) = with_payload_mut(WindowMsgPayload::Text(String::new()), |token| {
        window.send_system_message(WindowMessage::User(GGM_GET_LABEL), 0, token)
    });
    assert_eq!(handled, WindowMsgHandled::Handled);
    assert_eq!(out, Some(WindowMsgPayload::Text("Zero Hour".to_string())));
}

#[test]
fn write_input_focus_response_uses_payload_token_not_raw_ptr() {
    let token = push_payload(WindowMsgPayload::Bool(false));
    assert_eq!(
        write_input_focus_response(1, token, true),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(token), Some(WindowMsgPayload::Bool(true)));

    let lose_token = push_payload(WindowMsgPayload::Bool(true));
    assert_eq!(
        write_input_focus_response(0, lose_token, false),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(lose_token), Some(WindowMsgPayload::Bool(true)));

    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(31, 0, 0, 16)));
    let focus_token = push_payload(WindowMsgPayload::Bool(false));
    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 1, focus_token),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(focus_token), Some(WindowMsgPayload::Bool(true)));
}

#[test]
fn static_text_create_destroy_are_consumed_like_cpp_system_callback() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::StaticText(StaticText::new(8, 0, 0, 120, 24)));

    assert_eq!(
        window.send_system_message(WindowMessage::Create, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_system_message(WindowMessage::Destroy, 0, 0),
        WindowMsgHandled::Handled
    );
}

#[test]
fn static_text_input_focus_is_ignored_like_cpp_system_callback() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let token = push_payload(WindowMsgPayload::Bool(true));
    let mut window = GameWindow::new();
    window.set_id(9);
    window.set_owner(Some(&owner));
    window.set_widget(WindowWidget::StaticText(StaticText::new(9, 0, 0, 120, 24)));

    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 1, token),
        WindowMsgHandled::Ignored
    );
    assert_eq!(pop_payload(token), Some(WindowMsgPayload::Bool(true)));
    assert!(!window.instance_data().state.contains(WindowState::HILITED));
    assert!(owner_seen.borrow().is_empty());
}

#[test]
fn test_window_creation() {
    let window = GameWindow::new();
    assert_eq!(window.get_id(), WINDOW_ID_INVALID);
    assert_eq!(window.get_size(), (0, 0));
    assert_eq!(window.get_position(), (0, 0));
    assert!(!window.is_enabled());
    assert!(!window.is_hidden());
}

#[test]
fn test_window_properties() {
    let mut window = GameWindow::new();

    window.set_id(123);
    assert_eq!(window.get_id(), 123);

    window.set_size(100, 200).unwrap();
    assert_eq!(window.get_size(), (100, 200));

    window.set_position(10, 20).unwrap();
    assert_eq!(window.get_position(), (10, 20));

    window.set_text("Test Window").unwrap();
    assert_eq!(window.get_text(), "Test Window");
    assert_eq!(window.get_text_length(), 11);

    window.enable(true).unwrap();
    assert!(window.is_enabled());

    window.hide(true).unwrap();
    assert!(window.is_hidden());
}

#[test]
fn is_enabled_fail_closed_when_parent_mutably_borrowed() {
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));

    parent.borrow_mut().enable(true).unwrap();
    {
        let mut child = child.borrow_mut();
        child.enable(true).unwrap();
        child.set_parent(Some(&parent));
    }

    let _parent_mut = parent.borrow_mut();
    assert!(
        !child.borrow().is_enabled(),
        "parent already mutably borrowed: fail-closed as not enabled, no RefCell alias"
    );
}

#[test]
fn text_length_counts_characters_like_cpp_unicode_string() {
    let mut window = GameWindow::new();

    window.set_text("Aé中").unwrap();

    assert_eq!(window.get_text_length(), 3);
    assert_eq!(window.get_text().len(), 6);
}

#[test]
fn gadget_messages_route_to_owner_not_parent_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let parent_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));

    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_window, msg, data1, _data2| {
                owner_seen.borrow_mut().push((msg, data1));
                WindowMsgHandled::Handled
            });
    }
    {
        let parent_seen = parent_seen.clone();
        parent
            .borrow_mut()
            .set_system_callback(move |_window, msg, data1, _data2| {
                parent_seen.borrow_mut().push((msg, data1));
                WindowMsgHandled::Handled
            });
    }

    let mut button = PushButton::new(7, 0, 0, 20, 20);
    button.set_triggers_on_mouse_down(true);

    {
        let mut child = child.borrow_mut();
        child.set_id(7);
        child.enable(true).unwrap();
        child.set_parent(Some(&parent));
        child.set_owner(Some(&owner));
        child.set_widget(WindowWidget::PushButton(button));
    }
    parent.borrow_mut().enable(true).unwrap();

    assert_eq!(
        child
            .borrow_mut()
            .send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Handled
    );

    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::GadgetSelected, 7)]
    );
    assert!(parent_seen.borrow().is_empty());
}

#[test]
fn gadget_messages_to_self_owner_do_not_reborrow_window() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));

    {
        let seen = seen.clone();
        child
            .borrow_mut()
            .set_system_callback(move |_window, msg, data1, _data2| {
                seen.borrow_mut().push((msg, data1));
                WindowMsgHandled::Handled
            });
    }

    let mut button = PushButton::new(11, 0, 0, 20, 20);
    button.set_triggers_on_mouse_down(true);

    {
        let mut child_mut = child.borrow_mut();
        child_mut.set_id(11);
        child_mut.enable(true).unwrap();
        child_mut.set_parent(Some(&parent));
        child_mut.set_owner_self(&child);
        child_mut.set_widget(WindowWidget::PushButton(button));
    }
    parent.borrow_mut().enable(true).unwrap();

    assert_eq!(
        child
            .borrow_mut()
            .send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        seen.borrow().as_slice(),
        &[(WindowMessage::GadgetSelected, 11)]
    );
}

#[test]
fn set_size_sends_resized_system_message_like_cpp() {
    let mut window = GameWindow::new();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        window.set_system_callback(move |_, msg, data1, data2| {
            seen.borrow_mut().push((msg, data1, data2));
            WindowMsgHandled::Handled
        });
    }

    window.set_size(123, 45).unwrap();

    assert_eq!(
        seen.borrow().as_slice(),
        &[(WindowMessage::User(GGM_RESIZED), 123, 45)]
    );
}

#[test]
fn text_color_getters_return_all_state_colors_like_cpp() {
    let mut window = GameWindow::new();

    window.set_enabled_text_colors(0x01020304, 0x05060708);
    window.set_disabled_text_colors(0x11121314, 0x15161718);
    window.set_ime_composite_text_colors(0x21222324, 0x25262728);
    window.set_hilite_text_colors(0x31323334, 0x35363738);

    assert_eq!(window.get_enabled_text_color(), 0x01020304);
    assert_eq!(window.get_enabled_text_border_color(), 0x05060708);
    assert_eq!(window.get_disabled_text_color(), 0x11121314);
    assert_eq!(window.get_disabled_text_border_color(), 0x15161718);
    assert_eq!(window.get_ime_composite_text_color(), 0x21222324);
    assert_eq!(window.get_ime_composite_text_border_color(), 0x25262728);
    assert_eq!(window.get_hilite_text_color(), 0x31323334);
    assert_eq!(window.get_hilite_text_border_color(), 0x35363738);
}

#[test]
fn combo_box_text_color_setters_propagate_to_sub_gadgets_like_cpp() {
    let mut combo = GameWindow::new();
    combo.set_id(1);

    let edit_box = Rc::new(RefCell::new(GameWindow::new()));
    edit_box.borrow_mut().set_id(2);
    let list_box = Rc::new(RefCell::new(GameWindow::new()));
    list_box.borrow_mut().set_id(3);
    let drop_down = Rc::new(RefCell::new(GameWindow::new()));
    drop_down.borrow_mut().set_id(4);

    combo.add_child(edit_box.clone());
    combo.add_child(list_box.clone());
    combo.add_child(drop_down.clone());
    combo.set_combobox_links(ComboBoxLinks {
        drop_down: 4,
        edit_box: 2,
        list_box: 3,
    });

    combo.set_enabled_text_colors(0x11223344, 0x55667788);
    combo.set_disabled_text_colors(0x01020304, 0x05060708);
    combo.set_hilite_text_colors(0xaabbccdd, 0x12345678);
    combo.set_ime_composite_text_colors(0x87654321, 0xddccbbaa);
    combo.set_font(GameFont {
        name: "Arial".to_string(),
        size: 18,
        bold: true,
    });

    for child in [edit_box, list_box] {
        let child = child.borrow();
        assert_eq!(child.inst_data.enabled_text.color, 0x11223344);
        assert_eq!(child.inst_data.enabled_text.border_color, 0x55667788);
        assert_eq!(child.inst_data.disabled_text.color, 0x01020304);
        assert_eq!(child.inst_data.disabled_text.border_color, 0x05060708);
        assert_eq!(child.inst_data.hilite_text.color, 0xaabbccdd);
        assert_eq!(child.inst_data.hilite_text.border_color, 0x12345678);
        assert_eq!(child.inst_data.ime_composite_text.color, 0x87654321);
        assert_eq!(child.inst_data.ime_composite_text.border_color, 0xddccbbaa);
        let font = child.get_font().unwrap();
        assert_eq!(font.name, "Arial");
        assert_eq!(font.size, 18);
        assert!(font.bold);
    }

    let drop_down = drop_down.borrow();
    assert_eq!(drop_down.inst_data.enabled_text.color, 0);
    assert_eq!(drop_down.inst_data.disabled_text.color, 0);
    assert_eq!(drop_down.inst_data.hilite_text.color, 0);
    assert_eq!(drop_down.inst_data.ime_composite_text.color, 0);
    assert!(drop_down.get_font().is_none());
}

#[test]
fn combo_box_set_colors_propagates_to_sub_gadgets_like_cpp() {
    let (mut combo, edit_box, list_box, drop_down) = combo_fixture();
    fn assert_combo_button_colors(window: &GameWindow) {
        assert_eq!(window.get_enabled_draw_data(0).unwrap().color, 0x01020304);
        assert_eq!(
            window.get_enabled_draw_data(0).unwrap().border_color,
            0x05060708
        );
        assert_eq!(window.get_enabled_draw_data(1).unwrap().color, 0x11121314);
        assert_eq!(
            window.get_enabled_draw_data(1).unwrap().border_color,
            0x15161718
        );
        assert_eq!(window.get_disabled_draw_data(0).unwrap().color, 0x21222324);
        assert_eq!(
            window.get_disabled_draw_data(1).unwrap().border_color,
            0x35363738
        );
        assert_eq!(window.get_hilite_draw_data(0).unwrap().color, 0x41424344);
        assert_eq!(
            window.get_hilite_draw_data(1).unwrap().border_color,
            0x55565758
        );
    }

    gadget_combo_box_set_colors(
        &mut combo, 0x01020304, 0x05060708, 0x11121314, 0x15161718, 0x21222324, 0x25262728,
        0x31323334, 0x35363738, 0x41424344, 0x45464748, 0x51525354, 0x55565758,
    );

    assert_combo_button_colors(&combo);
    assert_combo_button_colors(&edit_box.borrow());
    assert_combo_button_colors(&drop_down.borrow());

    let list_box = list_box.borrow();
    assert_eq!(list_box.get_enabled_draw_data(0).unwrap().color, 0x01020304);
    assert_eq!(list_box.get_enabled_draw_data(1).unwrap().color, 0x11121314);
    assert_eq!(
        list_box.get_disabled_draw_data(0).unwrap().color,
        0x21222324
    );
    assert_eq!(
        list_box.get_disabled_draw_data(1).unwrap().color,
        0x31323334
    );
    assert_eq!(list_box.get_hilite_draw_data(0).unwrap().color, 0x41424344);
    assert_eq!(list_box.get_hilite_draw_data(1).unwrap().color, 0x51525354);
}

#[test]
fn combo_box_set_text_uses_edit_child_without_selecting_matching_item_like_cpp() {
    let (mut combo, edit_box, _list_box, _drop_down) = combo_fixture();
    {
        let combo_widget = combo.combo_box_mut().unwrap();
        combo_widget.add_item(ComboBoxItem::new(0, "Alpha"));
        combo_widget.add_item(ComboBoxItem::new(1, "Bravo"));
    }

    assert_eq!(
        with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
            combo.send_system_message(WindowMessage::User(GCM_SET_TEXT), token, 0)
        }),
        WindowMsgHandled::Handled
    );

    assert_eq!(
        edit_box
            .borrow()
            .widget()
            .and_then(|widget| match widget {
                WindowWidget::TextEntry(entry) => Some(entry.text().to_string()),
                _ => None,
            })
            .as_deref(),
        Some("Alpha")
    );
    assert_eq!(combo.combo_box_mut().unwrap().selected_index(), None);

    let token = push_payload(WindowMsgPayload::Text("stale".to_string()));
    let _ = combo.send_system_message(WindowMessage::User(GCM_GET_TEXT), 0, token);
    assert_eq!(
        pop_payload(token),
        Some(WindowMsgPayload::Text("Alpha".to_string()))
    );
}

#[test]
fn combo_box_item_data_round_trips_pointer_sized_values_like_cpp() {
    let (mut combo, _edit_box, _list_box, _drop_down) = combo_fixture();
    let _ = with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
        combo.send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0xFF11_2233)
    });

    let raw_data = usize::MAX - 7;
    assert_eq!(
        combo.send_system_message(WindowMessage::User(GCM_SET_ITEM_DATA), 0, raw_data,),
        WindowMsgHandled::Handled
    );

    let token = push_payload(WindowMsgPayload::UInt(0));
    let _ = combo.send_system_message(WindowMessage::User(GCM_GET_ITEM_DATA), 0, token);
    assert_eq!(pop_payload(token), Some(WindowMsgPayload::UInt(raw_data)));
}

#[test]
fn combo_box_selection_hides_list_updates_edit_and_notifies_owner_like_cpp() {
    let (mut combo, edit_box, list_box, _drop_down) = combo_fixture();
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_window, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }
    combo.set_owner(Some(&owner));

    let _ = with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
        combo.send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0xFF11_2233)
    });
    let _ = with_payload(WindowMsgPayload::Text("Bravo".to_string()), |token| {
        combo.send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0xFF44_5566)
    });
    list_box.borrow_mut().hide(false).unwrap();
    combo.set_size(120, 80).unwrap();

    assert_eq!(
        combo.send_system_message(WindowMessage::User(GCM_SET_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );

    assert!(list_box.borrow().is_hidden());
    assert_eq!(combo.get_size(), (120, 20));
    assert_eq!(
        edit_box
            .borrow()
            .widget()
            .and_then(|widget| match widget {
                WindowWidget::TextEntry(entry) => Some(entry.text().to_string()),
                _ => None,
            })
            .as_deref(),
        Some("Bravo")
    );
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::User(GCM_SELECTED), 1, 0)]
    );
}

#[test]
fn combo_box_edit_update_clears_selection_hides_list_and_notifies_owner_like_cpp() {
    let (mut combo, _edit_box, list_box, _drop_down) = combo_fixture();
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_window, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }
    combo.set_owner(Some(&owner));

    let _ = with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
        combo.send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0xFF11_2233)
    });
    let _ = combo.send_system_message(WindowMessage::User(GCM_SET_SELECTION), 0, 0);
    list_box.borrow_mut().hide(false).unwrap();
    combo.set_size(120, 80).unwrap();
    owner_seen.borrow_mut().clear();

    assert_eq!(
        combo.send_system_message(WindowMessage::GadgetValueChanged, 2, 0),
        WindowMsgHandled::Handled
    );

    assert!(list_box.borrow().is_hidden());
    assert_eq!(combo.combo_box_mut().unwrap().selected_index(), None);
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::User(GCM_UPDATE_TEXT), 1, 0)]
    );
}

#[test]
fn combo_box_dropdown_height_matches_cpp_font_formula() {
    assert_eq!(GameWindow::combo_box_dropdown_height(3, 5, 14), 52);
    assert_eq!(GameWindow::combo_box_dropdown_height(7, 5, 14), 84);
    assert_eq!(GameWindow::combo_box_dropdown_height(0, 5, 14), 4);
}

#[test]
fn test_window_status_flags() {
    let mut window = GameWindow::new();

    window.set_status(WindowStatus::ENABLED | WindowStatus::ACTIVE);
    assert!(window.get_status().contains(WindowStatus::ENABLED));
    assert!(window.get_status().contains(WindowStatus::ACTIVE));

    window.clear_status(WindowStatus::ENABLED);
    assert!(!window.get_status().contains(WindowStatus::ENABLED));
    assert!(window.get_status().contains(WindowStatus::ACTIVE));
}

#[test]
fn win_is_hidden_checks_only_own_status_like_cpp() {
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));
    child.borrow_mut().set_parent(Some(&parent));
    parent.borrow_mut().add_child(child.clone());

    parent.borrow_mut().hide(true).unwrap();
    assert!(parent.borrow().is_hidden());
    assert!(!child.borrow().is_hidden());

    parent.borrow_mut().hide(false).unwrap();
    assert!(!parent.borrow().is_hidden());
    assert!(!child.borrow().is_hidden());
}

#[test]
fn win_is_child_checks_full_parent_chain_like_cpp() {
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));
    let grandchild = Rc::new(RefCell::new(GameWindow::new()));
    let sibling = Rc::new(RefCell::new(GameWindow::new()));

    child.borrow_mut().set_parent(Some(&parent));
    parent.borrow_mut().add_child(child.clone());
    grandchild.borrow_mut().set_parent(Some(&child));
    child.borrow_mut().add_child(grandchild.clone());

    assert!(parent.borrow().is_child(&child.borrow()));
    assert!(parent.borrow().is_child(&grandchild.borrow()));
    assert!(child.borrow().is_child(&grandchild.borrow()));
    assert!(!parent.borrow().is_child(&parent.borrow()));
    assert!(!parent.borrow().is_child(&sibling.borrow()));
}

#[test]
fn leaf_helpers_walk_window_tree_like_cpp() {
    let root = Rc::new(RefCell::new(GameWindow::new()));
    let trailing_leaf = Rc::new(RefCell::new(GameWindow::new()));
    let branch = Rc::new(RefCell::new(GameWindow::new()));
    let branch_leaf = Rc::new(RefCell::new(GameWindow::new()));

    trailing_leaf.borrow_mut().set_parent(Some(&root));
    root.borrow_mut().add_child(trailing_leaf.clone());
    branch.borrow_mut().set_parent(Some(&root));
    root.borrow_mut().add_child(branch.clone());
    branch_leaf.borrow_mut().set_parent(Some(&branch));
    branch.borrow_mut().add_child(branch_leaf.clone());

    assert!(Rc::ptr_eq(
        &GameWindow::find_first_leaf(&trailing_leaf),
        &branch_leaf
    ));
    assert!(Rc::ptr_eq(
        &GameWindow::find_last_leaf(&branch_leaf),
        &trailing_leaf
    ));
    assert!(Rc::ptr_eq(
        &GameWindow::find_next_leaf(&branch_leaf).unwrap(),
        &trailing_leaf
    ));
    assert!(Rc::ptr_eq(
        &GameWindow::find_prev_leaf(&trailing_leaf).unwrap(),
        &branch_leaf
    ));
    assert!(Rc::ptr_eq(
        &GameWindow::find_next_leaf(&trailing_leaf).unwrap(),
        &branch_leaf
    ));
    assert!(Rc::ptr_eq(
        &GameWindow::find_prev_leaf(&branch_leaf).unwrap(),
        &trailing_leaf
    ));
}

#[test]
fn leaf_helpers_stop_descent_at_tab_stop_like_cpp() {
    let root = Rc::new(RefCell::new(GameWindow::new()));
    let tab_branch = Rc::new(RefCell::new(GameWindow::new()));
    let leading_leaf = Rc::new(RefCell::new(GameWindow::new()));
    let child_under_tab = Rc::new(RefCell::new(GameWindow::new()));

    tab_branch.borrow_mut().set_status(WindowStatus::TAB_STOP);
    tab_branch.borrow_mut().set_parent(Some(&root));
    root.borrow_mut().add_child(tab_branch.clone());
    leading_leaf.borrow_mut().set_parent(Some(&root));
    root.borrow_mut().add_child(leading_leaf.clone());
    child_under_tab.borrow_mut().set_parent(Some(&tab_branch));
    tab_branch.borrow_mut().add_child(child_under_tab.clone());

    assert!(Rc::ptr_eq(
        &GameWindow::find_next_leaf(&leading_leaf).unwrap(),
        &tab_branch
    ));
    assert!(Rc::ptr_eq(
        &GameWindow::find_prev_leaf(&leading_leaf).unwrap(),
        &child_under_tab
    ));
}

#[test]
fn show_tab_pane_falls_back_and_updates_active_tab_like_cpp() {
    let mut tab_window = GameWindow::new();
    let mut tab_control = TabControl::new(7, 0, 0, 100, 80);
    tab_control.set_tab_data(TabControlData {
        tab_count: 2,
        ..Default::default()
    });
    tab_window.set_widget(WindowWidget::TabControl(tab_control));

    let first_pane = Rc::new(RefCell::new(GameWindow::new()));
    first_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
    let second_pane = Rc::new(RefCell::new(GameWindow::new()));
    second_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;

    tab_window.add_child(first_pane.clone());
    tab_window.add_child(second_pane.clone());

    tab_window.show_tab_pane(1);
    assert!(first_pane.borrow().is_hidden());
    assert!(!second_pane.borrow().is_hidden());

    tab_window.show_tab_pane(7);
    assert!(!first_pane.borrow().is_hidden());
    assert!(second_pane.borrow().is_hidden());

    let Some(WindowWidget::TabControl(tab_control)) = tab_window.widget() else {
        panic!("expected tab control widget");
    };
    assert_eq!(tab_control.active_tab_index(), 0);
}

#[test]
fn resizing_tab_control_resizes_panes_like_cpp() {
    let mut tab_window = GameWindow::new();
    let mut tab_control = TabControl::new(7, 0, 0, 100, 80);
    tab_control.set_tab_data(TabControlData {
        tab_edge: tabcontrol::TP_TOP_SIDE,
        tab_height: 20,
        tab_count: 2,
        pane_border: 3,
        ..Default::default()
    });
    tab_window.set_widget(WindowWidget::TabControl(tab_control));

    let first_pane = Rc::new(RefCell::new(GameWindow::new()));
    first_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
    let second_pane = Rc::new(RefCell::new(GameWindow::new()));
    second_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
    tab_window.add_child(first_pane.clone());
    tab_window.add_child(second_pane.clone());

    tab_window.set_size(200, 100).unwrap();

    assert_eq!(first_pane.borrow().get_position(), (3, 23));
    assert_eq!(first_pane.borrow().get_size(), (194, 74));
    assert_eq!(second_pane.borrow().get_position(), (3, 23));
    assert_eq!(second_pane.borrow().get_size(), (194, 74));

    let Some(WindowWidget::TabControl(tab_control)) = tab_window.widget() else {
        panic!("expected tab control widget");
    };
    assert_eq!(tab_control.content_bounds(), Rect::new(3, 23, 194, 74));
}

#[test]
fn progress_bar_system_message_matches_cpp_range_rules() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
        42, 0, 0, 100, 10,
    )));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GPM_SET_PROGRESS), 37, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.progress_bar_mut().unwrap().percentage(), 37.0);
    assert_eq!(window.get_user_data::<i32>(), Some(&37));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GPM_SET_PROGRESS), 101, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.progress_bar_mut().unwrap().percentage(), 37.0);
    assert_eq!(window.get_user_data::<i32>(), Some(&37));

    assert_eq!(
        window.send_system_message(
            WindowMessage::User(GPM_SET_PROGRESS),
            (-1i32) as WindowMsgData,
            0,
        ),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.progress_bar_mut().unwrap().percentage(), 37.0);
    assert_eq!(window.get_user_data::<i32>(), Some(&37));
}

#[test]
fn slider_system_messages_match_cpp_numeric_rules() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::HorizontalSlider(
        HorizontalSlider::new(7, 0, 0, 100, 20).with_range(0, 100),
    ));
    let thumb = Rc::new(RefCell::new(GameWindow::new()));
    thumb.borrow_mut().set_id(77);
    thumb.borrow_mut().set_size(13, 16).unwrap();
    window.add_child(thumb.clone());
    window.set_slider_thumb(77);

    window.set_size(100, 24).unwrap();
    assert_eq!(thumb.borrow().get_size(), (GADGET_SIZE, 24));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GSM_SET_MIN_MAX), 10, 20),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.horizontal_slider_mut().unwrap().range(), (10, 20));
    assert_eq!(window.horizontal_slider_mut().unwrap().value(), 10);
    assert_eq!(
        thumb.borrow().get_position(),
        (0, HORIZONTAL_SLIDER_THUMB_POSITION)
    );

    assert_eq!(
        window.send_system_message(WindowMessage::User(GSM_SET_SLIDER), 15, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.horizontal_slider_mut().unwrap().value(), 15);
    let position_after_valid_set = thumb.borrow().get_position();

    assert_eq!(
        window.send_system_message(WindowMessage::User(GSM_SET_SLIDER), 21, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.horizontal_slider_mut().unwrap().value(), 15);
    assert_eq!(thumb.borrow().get_position(), position_after_valid_set);
}

#[test]
fn slider_mouse_enter_leave_hilites_thumb_child_like_cpp() {
    let mut window = GameWindow::new();
    window.set_id(7);
    window.enable(true).unwrap();
    window.set_widget(WindowWidget::HorizontalSlider(
        HorizontalSlider::new(7, 0, 0, 100, 20).with_range(0, 100),
    ));

    let thumb = Rc::new(RefCell::new(GameWindow::new()));
    {
        let mut thumb = thumb.borrow_mut();
        thumb.set_id(77);
        thumb.set_widget(WindowWidget::PushButton(PushButton::new(77, 0, 0, 16, 16)));
    }
    window.add_child(thumb.clone());
    window.set_slider_thumb(77);

    assert!(
        !thumb
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::HILITED)
    );
    assert_eq!(
        window.send_input_message(WindowMessage::MouseEntering, 0, 0),
        WindowMsgHandled::Handled
    );
    assert!(
        thumb
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::HILITED)
    );
    assert_eq!(
        window.send_input_message(WindowMessage::MouseLeaving, 0, 0),
        WindowMsgHandled::Handled
    );
    assert!(
        !thumb
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::HILITED)
    );
}

#[test]
fn vertical_slider_system_messages_use_cpp_inverted_axis() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::VerticalSlider(
        VerticalSlider::new(7, 0, 0, 20, 120).with_range(0, 100),
    ));
    let thumb = Rc::new(RefCell::new(GameWindow::new()));
    thumb.borrow_mut().set_id(77);
    thumb.borrow_mut().set_size(20, 20).unwrap();
    window.add_child(thumb.clone());
    window.set_slider_thumb(77);
    window.set_size(20, 120).unwrap();

    assert_eq!(
        window.send_system_message(WindowMessage::User(GSM_SET_SLIDER), 100, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.vertical_slider_mut().unwrap().value(), 100);
    assert_eq!(thumb.borrow().get_position(), (0, 0));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GSM_SET_SLIDER), 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.vertical_slider_mut().unwrap().value(), 0);
    assert_eq!(thumb.borrow().get_position(), (0, 104));
}

#[test]
fn radio_set_selection_system_message_matches_cpp_notify_rules() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, _| {
                owner_seen.borrow_mut().push((msg, data1));
                WindowMsgHandled::Handled
            });
    }

    let mut silent_window = GameWindow::new();
    silent_window.set_id(17);
    silent_window.set_owner(Some(&owner));
    silent_window.set_widget(WindowWidget::RadioButton(RadioButton::new(
        17,
        0,
        0,
        16,
        RadioButtonGroup::new(2),
    )));

    assert_eq!(
        silent_window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 0, 0),
        WindowMsgHandled::Handled
    );
    assert!(matches!(
        silent_window.widget(),
        Some(WindowWidget::RadioButton(radio)) if radio.is_selected()
    ));
    assert!(
        silent_window
            .instance_data()
            .state
            .contains(WindowState::SELECTED)
    );
    assert!(owner_seen.borrow().is_empty());

    let mut notifying_window = GameWindow::new();
    notifying_window.set_id(18);
    notifying_window.set_owner(Some(&owner));
    notifying_window.set_widget(WindowWidget::RadioButton(RadioButton::new(
        18,
        0,
        0,
        16,
        RadioButtonGroup::new(3),
    )));

    assert_eq!(
        notifying_window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::GadgetSelected, 18)]
    );
    assert!(
        notifying_window
            .instance_data()
            .state
            .contains(WindowState::SELECTED)
    );

    assert_eq!(
        notifying_window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(owner_seen.borrow().len(), 1);
}

#[test]
fn radio_set_selection_clears_group_peers_like_cpp() {
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let first = Rc::new(RefCell::new(GameWindow::new()));
    let second = Rc::new(RefCell::new(GameWindow::new()));
    let group = RadioButtonGroup::new(44);

    {
        let mut first = first.borrow_mut();
        first.set_id(41);
        first.set_parent(Some(&parent));
        first.set_widget(WindowWidget::RadioButton(RadioButton::new(
            41,
            0,
            0,
            16,
            group.clone(),
        )));
        if let Some(WindowWidget::RadioButton(radio)) = first.widget.as_mut() {
            radio.select();
        }
        first.sync_state_from_widget();
    }
    {
        let mut second = second.borrow_mut();
        second.set_id(42);
        second.set_parent(Some(&parent));
        second.set_widget(WindowWidget::RadioButton(RadioButton::new(
            42,
            0,
            0,
            16,
            group.clone(),
        )));
    }
    parent.borrow_mut().add_child(first.clone());
    parent.borrow_mut().add_child(second.clone());

    assert!(
        first
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::SELECTED)
    );

    assert_eq!(
        second
            .borrow_mut()
            .send_system_message(WindowMessage::User(GBM_SET_SELECTION), 0, 0),
        WindowMsgHandled::Handled
    );

    assert!(
        !first
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::SELECTED)
    );
    assert!(matches!(
        first.borrow().widget(),
        Some(WindowWidget::RadioButton(radio)) if !radio.is_selected()
    ));
    assert!(
        second
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::SELECTED)
    );
}

#[test]
fn radio_create_destroy_are_consumed_like_cpp_system_callback() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::RadioButton(RadioButton::new(
        43,
        0,
        0,
        16,
        RadioButtonGroup::new(5),
    )));

    assert_eq!(
        window.send_system_message(WindowMessage::Create, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_system_message(WindowMessage::Destroy, 0, 0),
        WindowMsgHandled::Handled
    );
}

#[test]
fn radio_mouse_track_gate_and_payloads_match_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(44);
    window.set_owner(Some(&owner));
    window.enable(true).unwrap();
    window.set_widget(WindowWidget::RadioButton(RadioButton::new(
        44,
        0,
        0,
        16,
        RadioButtonGroup::new(6),
    )));

    assert_eq!(
        window.send_input_message(WindowMessage::MouseEntering, 77, 0),
        WindowMsgHandled::Ignored
    );
    assert!(owner_seen.borrow().is_empty());

    window.instance_data_mut().style |= GWS_MOUSE_TRACK;
    assert_eq!(
        window.send_input_message(WindowMessage::MouseEntering, 77, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::LeftUp, 88, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::LeftDrag, 99, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::MouseLeaving, 111, 0),
        WindowMsgHandled::Handled
    );

    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::GadgetMouseEntering, 44, 77),
            (WindowMessage::GadgetSelected, 44, 88),
            (WindowMessage::User(GGM_LEFT_DRAG), 44, 99),
            (WindowMessage::GadgetMouseLeaving, 44, 111),
        ]
    );
}

#[test]
fn checkbox_widget_sync_sets_selected_state_like_cpp_instance_data() {
    let mut window = GameWindow::new();
    window.set_id(19);
    window.enable(true).unwrap();
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(19, 0, 0, 16)));
    window.check_box_mut().unwrap().set_checked(true);
    window.sync_state_from_widget();

    assert!(matches!(
        window.widget(),
        Some(WindowWidget::CheckBox(checkbox)) if checkbox.is_checked()
    ));
    assert!(window.instance_data().state.contains(WindowState::SELECTED));
}

#[test]
fn checkbox_set_selection_notifies_owner_even_when_unchanged_like_cpp_helper() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(20);
    window.set_owner(Some(&owner));
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(20, 0, 0, 16)));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert!(matches!(
        window.widget(),
        Some(WindowWidget::CheckBox(checkbox)) if checkbox.is_checked()
    ));
    assert!(window.instance_data().state.contains(WindowState::SELECTED));
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::GadgetSelected, 20, 0),
            (WindowMessage::GadgetSelected, 20, 0),
        ]
    );
}

#[test]
fn checkbox_create_destroy_are_consumed_like_cpp_system_callback() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(21, 0, 0, 16)));

    assert_eq!(
        window.send_system_message(WindowMessage::Create, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_system_message(WindowMessage::Destroy, 0, 0),
        WindowMsgHandled::Handled
    );
}

#[test]
fn checkbox_mouse_track_gate_and_payloads_match_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(22);
    window.set_owner(Some(&owner));
    window.enable(true).unwrap();
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(22, 0, 0, 16)));

    assert_eq!(
        window.send_input_message(WindowMessage::MouseEntering, 77, 0),
        WindowMsgHandled::Ignored
    );
    assert!(owner_seen.borrow().is_empty());

    window.instance_data_mut().style |= GWS_MOUSE_TRACK;
    assert_eq!(
        window.send_input_message(WindowMessage::MouseEntering, 77, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::LeftUp, 88, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::LeftDrag, 99, 0),
        WindowMsgHandled::Handled
    );

    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::GadgetMouseEntering, 22, 77),
            (WindowMessage::GadgetSelected, 22, 88),
            (WindowMessage::User(GGM_LEFT_DRAG), 22, 99),
        ]
    );

    owner_seen.borrow_mut().clear();
    window.set_status(WindowStatus::RIGHT_CLICK);
    window.check_box_mut().unwrap().set_checked(true);
    window.sync_state_from_widget();

    assert_eq!(
        window.send_input_message(WindowMessage::RightUp, 111, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::GadgetRightClick, 22, 111)]
    );
    assert!(matches!(
        window.widget(),
        Some(WindowWidget::CheckBox(checkbox)) if !checkbox.is_checked()
    ));
}

#[test]
fn checkbox_right_up_notifies_without_right_click_status_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(22);
    window.set_owner(Some(&owner));
    window.enable(true).unwrap();
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(22, 0, 0, 16)));
    window.check_box_mut().unwrap().set_checked(true);
    window.sync_state_from_widget();

    assert!(
        !window.get_status().contains(WindowStatus::RIGHT_CLICK),
        "C++ GadgetCheckBox does not require WIN_STATUS_RIGHT_CLICK"
    );
    assert_eq!(
        window.send_input_message(WindowMessage::RightUp, 111, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::GadgetRightClick, 22, 111)]
    );
}

#[test]
fn input_focus_notifies_owner_and_updates_hilite_like_cpp_gadgets() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(31);
    window.set_owner(Some(&owner));
    window.set_widget(WindowWidget::CheckBox(CheckBox::new(31, 0, 0, 16)));

    let gain_token = push_payload(WindowMsgPayload::Bool(false));
    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 1, gain_token),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(gain_token), Some(WindowMsgPayload::Bool(true)));
    assert!(window.instance_data().state.contains(WindowState::HILITED));

    let lose_token = push_payload(WindowMsgPayload::Bool(true));
    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 0, lose_token),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(lose_token), Some(WindowMsgPayload::Bool(false)));
    assert!(!window.instance_data().state.contains(WindowState::HILITED));

    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::User(GGM_FOCUS_CHANGE), 1, 31),
            (WindowMessage::User(GGM_FOCUS_CHANGE), 0, 31),
        ]
    );
}

#[test]
fn radio_input_focus_does_not_set_hilite_on_gain_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(32);
    window.set_owner(Some(&owner));
    window.set_widget(WindowWidget::RadioButton(RadioButton::new(
        32,
        0,
        0,
        16,
        RadioButtonGroup::new(4),
    )));

    let gain_token = push_payload(WindowMsgPayload::Bool(false));
    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 1, gain_token),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(gain_token), Some(WindowMsgPayload::Bool(true)));
    assert!(!window.instance_data().state.contains(WindowState::HILITED));

    window.set_hilite_state(true);
    let lose_token = push_payload(WindowMsgPayload::Bool(false));
    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 0, lose_token),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(lose_token), Some(WindowMsgPayload::Bool(true)));
    assert!(!window.instance_data().state.contains(WindowState::HILITED));

    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::User(GGM_FOCUS_CHANGE), 1, 32),
            (WindowMessage::User(GGM_FOCUS_CHANGE), 0, 32),
        ]
    );
}

#[test]
fn text_entry_input_focus_sets_selected_and_hilite_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(33);
    window.set_owner(Some(&owner));
    window.set_widget(WindowWidget::TextEntry(TextEntry::new(33, 0, 0, 100, 20)));

    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 1, 0),
        WindowMsgHandled::Handled
    );
    assert!(
        window
            .instance_data()
            .state
            .contains(WindowState::SELECTED | WindowState::HILITED)
    );

    assert_eq!(
        window.send_system_message(WindowMessage::InputFocus, 0, 0),
        WindowMsgHandled::Handled
    );
    assert!(
        !window
            .instance_data()
            .state
            .intersects(WindowState::SELECTED | WindowState::HILITED)
    );

    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::User(GGM_FOCUS_CHANGE), 1, 33),
            (WindowMessage::User(GGM_FOCUS_CHANGE), 0, 33),
        ]
    );
}

#[test]
fn listbox_delete_system_messages_match_cpp_state_rules() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 100, 60);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    listbox.add_item("charlie");
    assert!(listbox.select_index(2, KeyModifiers::none()));
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_DEL_ENTRY), 1, 0),
        WindowMsgHandled::Handled
    );
    let listbox = window.list_box_mut().unwrap();
    assert_eq!(listbox.items().len(), 2);
    assert_eq!(listbox.items()[1].text, "charlie");
    assert_eq!(listbox.selected_indices(), &[1]);

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_DEL_ENTRY), 99, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().items().len(), 2);

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_DEL_ALL), 0, 0),
        WindowMsgHandled::Handled
    );
    let listbox = window.list_box_mut().unwrap();
    assert!(listbox.items().is_empty());
    assert!(listbox.selected_indices().is_empty());
    assert_eq!(listbox.scroll_offset(), 0);
}

#[test]
fn listbox_selection_system_message_notifies_owner_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(42);
    window.set_owner(Some(&owner));
    let mut listbox = ListBox::new(42, 0, 0, 100, 60);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_SET_SELECTION), 1, 1),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().selected_indices(), &[1]);
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::User(GLM_SELECTED), 42, 1)]
    );

    assert_eq!(
        window.send_system_message(
            WindowMessage::User(GLM_SET_SELECTION),
            (-1i32) as WindowMsgData,
            1,
        ),
        WindowMsgHandled::Handled
    );
    assert!(window.list_box_mut().unwrap().selected_indices().is_empty());
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::User(GLM_SELECTED), 42, 1),
            (
                WindowMessage::User(GLM_SELECTED),
                42,
                (-1i32) as WindowMsgData,
            ),
        ]
    );
}

#[test]
fn titled_listbox_input_uses_title_content_inset_like_cpp() {
    let mut window = GameWindow::new();
    window.set_id(42);
    window.set_size(100, 60).unwrap();
    window.enable(true).unwrap();
    window.set_text("Title").unwrap();

    let mut listbox = ListBox::new(42, 0, 0, 100, 60).with_item_height(10);
    listbox.add_item_with_id(100, "alpha");
    listbox.add_item_with_id(200, "bravo");
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(window.list_box_mut().unwrap().content_top_inset(), 13);

    window.set_cursor_position(1, 12).unwrap();
    let _ = window.send_routed_input_message(WindowMessage::LeftUp, 0, 0);
    assert!(window.list_box_mut().unwrap().selected_indices().is_empty());

    window.set_cursor_position(1, 13).unwrap();
    let _ = window.send_routed_input_message(WindowMessage::LeftUp, 0, 0);
    assert_eq!(window.list_box_mut().unwrap().selected_indices(), &[0]);
}

#[test]
fn listbox_input_sends_glm_selected_row_to_owner_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(42);
    window.set_parent(Some(&owner));
    window.set_owner(Some(&owner));
    window.set_size(100, 40).unwrap();
    window.enable(true).unwrap();
    let mut listbox = ListBox::new(42, 0, 0, 100, 40).with_item_height(10);
    listbox.add_item_with_id(100, "alpha");
    listbox.add_item_with_id(200, "bravo");
    window.set_widget(WindowWidget::ListBox(listbox));

    window.set_cursor_position(1, 11).unwrap();
    assert_eq!(
        window.send_routed_input_message(WindowMessage::LeftUp, 0, 0),
        WindowMsgHandled::Handled
    );

    assert_eq!(window.list_box_mut().unwrap().selected_indices(), &[1]);
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::User(GLM_SELECTED), 42, 1)]
    );
}

#[test]
fn listbox_double_click_sends_glm_double_clicked_before_selection_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(42);
    window.set_parent(Some(&owner));
    window.set_owner(Some(&owner));
    window.set_size(100, 40).unwrap();
    window.enable(true).unwrap();
    let mut listbox = ListBox::new(42, 0, 0, 100, 40).with_item_height(10);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    window.set_widget(WindowWidget::ListBox(listbox));

    window.set_cursor_position(1, 1).unwrap();
    assert_eq!(
        window.send_routed_input_message(WindowMessage::LeftUp, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_routed_input_message(WindowMessage::LeftUp, 0, 0),
        WindowMsgHandled::Handled
    );

    assert!(window.list_box_mut().unwrap().selected_indices().is_empty());
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[
            (WindowMessage::User(GLM_SELECTED), 42, 0),
            (WindowMessage::User(GLM_DOUBLE_CLICKED), 42, 0),
            (
                WindowMessage::User(GLM_SELECTED),
                42,
                (-1i32) as WindowMsgData,
            ),
        ]
    );
}

#[test]
fn listbox_scroll_buffer_and_update_display_system_messages_match_cpp() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 100, 40);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    listbox.add_item("charlie");
    listbox.add_item("delta");
    assert!(listbox.select_index(0, KeyModifiers::none()));
    listbox.set_top_visible_entry(2);
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_SCROLL_BUFFER), 1, 0),
        WindowMsgHandled::Handled
    );
    let listbox = window.list_box_mut().unwrap();
    assert_eq!(
        listbox
            .items()
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["bravo", "charlie", "delta"]
    );
    assert_eq!(listbox.selected_indices(), &[0]);
    assert_eq!(listbox.scroll_offset(), 1);

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_UPDATE_DISPLAY), 99, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().scroll_offset(), 1);

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_UPDATE_DISPLAY), 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().scroll_offset(), 0);
}

#[test]
fn listbox_visible_entry_helpers_match_cpp_public_api() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 100, 30).with_item_height(10);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    listbox.add_item("charlie");
    listbox.add_item("delta");
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(gadget_list_box_get_top_visible_entry(&window), 0);
    assert_eq!(gadget_list_box_get_bottom_visible_entry(&window), 2);
    assert!(gadget_list_box_is_full(&window));

    gadget_list_box_set_bottom_visible_entry(&mut window, 3);
    assert_eq!(gadget_list_box_get_top_visible_entry(&window), 1);
    assert_eq!(gadget_list_box_get_bottom_visible_entry(&window), 3);

    gadget_list_box_set_top_visible_entry(&mut window, 0);
    assert_eq!(gadget_list_box_get_top_visible_entry(&window), 0);
    assert_eq!(gadget_list_box_get_bottom_visible_entry(&window), 2);

    let non_listbox = GameWindow::new();
    assert_eq!(gadget_list_box_get_top_visible_entry(&non_listbox), 0);
    assert_eq!(gadget_list_box_get_bottom_visible_entry(&non_listbox), 0);
    assert!(!gadget_list_box_is_full(&non_listbox));
}

#[test]
fn listbox_column_helpers_match_cpp_public_api() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 120, 30);
    listbox.set_columns(3);
    listbox.set_column_width_percentages(vec![50, 25, 25]);
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(gadget_list_box_get_num_columns(&window), 3);
    assert_eq!(gadget_list_box_get_column_width(&window, 0), 60);
    assert_eq!(gadget_list_box_get_column_width(&window, 1), 30);
    assert_eq!(gadget_list_box_get_column_width(&window, 2), 30);
    assert_eq!(gadget_list_box_get_column_width(&window, -1), 0);
    assert_eq!(gadget_list_box_get_column_width(&window, 3), 0);

    let non_listbox = GameWindow::new();
    assert_eq!(gadget_list_box_get_num_columns(&non_listbox), 0);
    assert_eq!(gadget_list_box_get_column_width(&non_listbox, 0), 0);
}

#[test]
fn listbox_set_colors_propagates_to_scrollbar_like_cpp() {
    let mut listbox = GameWindow::new();
    listbox.set_widget(WindowWidget::ListBox(ListBox::new(42, 0, 0, 120, 60)));

    let up = Rc::new(RefCell::new(GameWindow::new()));
    up.borrow_mut().set_id(10);
    let down = Rc::new(RefCell::new(GameWindow::new()));
    down.borrow_mut().set_id(11);
    let slider = Rc::new(RefCell::new(GameWindow::new()));
    slider.borrow_mut().set_id(12);
    let thumb = Rc::new(RefCell::new(GameWindow::new()));
    thumb.borrow_mut().set_id(13);

    thumb
        .borrow_mut()
        .set_enabled_draw_colors(1, 0x10101010, 0x20202020)
        .unwrap();
    thumb
        .borrow_mut()
        .set_disabled_draw_colors(1, 0x30303030, 0x40404040)
        .unwrap();
    thumb
        .borrow_mut()
        .set_hilite_draw_colors(1, 0x50505050, 0x60606060)
        .unwrap();

    slider.borrow_mut().add_child(thumb.clone());
    listbox.add_child(up.clone());
    listbox.add_child(down.clone());
    listbox.add_child(slider.clone());
    listbox.set_listbox_links(ListBoxLinks {
        up_button: 10,
        down_button: 11,
        slider: 12,
        thumb: Some(13),
    });

    gadget_list_box_set_colors(
        &mut listbox,
        0x01010101,
        0x02020202,
        0x03030303,
        0x04040404,
        0x05050505,
        0x06060606,
        0x07070707,
        0x08080808,
        0x09090909,
        0x0a0a0a0a,
        0x0b0b0b0b,
        0x0c0c0c0c,
    );

    assert_eq!(listbox.get_enabled_draw_data(0).unwrap().color, 0x01010101);
    assert_eq!(
        listbox.get_enabled_draw_data(0).unwrap().border_color,
        0x02020202
    );
    assert_eq!(listbox.get_enabled_draw_data(1).unwrap().color, 0x03030303);
    assert_eq!(
        listbox.get_disabled_draw_data(1).unwrap().border_color,
        0x08080808
    );
    assert_eq!(listbox.get_hilite_draw_data(1).unwrap().color, 0x0b0b0b0b);

    assert_eq!(
        slider.borrow().get_enabled_draw_data(0).unwrap().color,
        0x01010101
    );
    assert_eq!(
        slider
            .borrow()
            .get_disabled_draw_data(0)
            .unwrap()
            .border_color,
        0x06060606
    );
    assert_eq!(
        slider.borrow().get_hilite_draw_data(0).unwrap().color,
        0x09090909
    );

    for button in [up, down] {
        let button = button.borrow();
        assert_eq!(button.get_enabled_draw_data(0).unwrap().color, 0x01010101);
        assert_eq!(
            button.get_enabled_draw_data(1).unwrap().border_color,
            0x20202020
        );
        assert_eq!(button.get_disabled_draw_data(1).unwrap().color, 0x30303030);
        assert_eq!(
            button.get_hilite_draw_data(1).unwrap().border_color,
            0x60606060
        );
    }
}

#[test]
fn listbox_linked_slider_tracks_cpp_inverted_position() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 100, 40);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    listbox.add_item("charlie");
    listbox.add_item("delta");
    window.set_widget(WindowWidget::ListBox(listbox));

    let slider_window = Rc::new(RefCell::new(GameWindow::new()));
    slider_window.borrow_mut().set_id(99);
    slider_window
        .borrow_mut()
        .set_widget(WindowWidget::VerticalSlider(
            VerticalSlider::new(99, 0, 0, 20, 40).with_range(0, 1),
        ));
    window.add_child(slider_window.clone());

    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_SET_SLIDER), 99, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        slider_window
            .borrow_mut()
            .vertical_slider_mut()
            .unwrap()
            .range(),
        (0, 2)
    );
    assert_eq!(
        slider_window
            .borrow_mut()
            .vertical_slider_mut()
            .unwrap()
            .value(),
        2
    );

    window.list_box_mut().unwrap().set_scroll_offset(1);
    window.update_listbox_scrollbar();
    assert_eq!(
        slider_window
            .borrow_mut()
            .vertical_slider_mut()
            .unwrap()
            .value(),
        1
    );

    assert_eq!(
        window.send_system_message(WindowMessage::User(GSM_SLIDER_TRACK), 99, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().scroll_offset(), 2);

    slider_window
        .borrow_mut()
        .vertical_slider_mut()
        .unwrap()
        .set_value(1);
    assert_eq!(
        window.send_system_message(WindowMessage::GadgetValueChanged, 99, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().scroll_offset(), 1);
}

#[test]
fn listbox_toggle_multi_selection_system_message_matches_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut single_window = GameWindow::new();
    single_window.set_owner(Some(&owner));
    let mut single = ListBox::new(42, 0, 0, 100, 40);
    single.add_item("alpha");
    single.add_item("bravo");
    single_window.set_widget(WindowWidget::ListBox(single));

    assert_eq!(
        single_window.send_system_message(WindowMessage::User(GLM_TOGGLE_MULTI_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert!(
        single_window
            .list_box_mut()
            .unwrap()
            .selected_indices()
            .is_empty()
    );

    let mut multi_window = GameWindow::new();
    multi_window.set_owner(Some(&owner));
    let mut multi = ListBox::new(43, 0, 0, 100, 40).with_selection_mode(SelectionMode::Multiple);
    multi.add_item("alpha");
    multi.add_item("bravo");
    multi.add_item("charlie");
    multi_window.set_widget(WindowWidget::ListBox(multi));

    assert_eq!(
        multi_window.send_system_message(WindowMessage::User(GLM_TOGGLE_MULTI_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        multi_window.list_box_mut().unwrap().selected_indices(),
        &[1]
    );

    assert_eq!(
        multi_window.send_system_message(WindowMessage::User(GLM_TOGGLE_MULTI_SELECTION), 2, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        multi_window.list_box_mut().unwrap().selected_indices(),
        &[1, 2]
    );

    assert_eq!(
        multi_window.send_system_message(WindowMessage::User(GLM_TOGGLE_MULTI_SELECTION), 1, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        multi_window.list_box_mut().unwrap().selected_indices(),
        &[2]
    );

    assert_eq!(
        multi_window.send_system_message(
            WindowMessage::User(GLM_TOGGLE_MULTI_SELECTION),
            (-1i32) as WindowMsgData,
            0,
        ),
        WindowMsgHandled::Handled
    );
    assert!(
        multi_window
            .list_box_mut()
            .unwrap()
            .selected_indices()
            .is_empty()
    );
    assert!(owner_seen.borrow().is_empty());
}

#[test]
fn listbox_pointer_system_messages_bridge_cpp_payloads() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 100, 60);
    listbox.set_columns(2);
    window.set_widget(WindowWidget::ListBox(listbox));

    let entry = ListBoxAddEntry {
        row: -1,
        column: 0,
        overwrite: true,
        data: ListBoxItemData::Text("Alpha".to_string()),
        color: Some(ShellColor::new(10, 20, 30, 40)),
    };
    let result = with_payload(WindowMsgPayload::AddEntry(entry), |token| {
        window.send_system_message(WindowMessage::User(GLM_ADD_ENTRY), token, 0)
    });
    assert_eq!(result.value_i32(), Some(0));
    assert!(result.is_handled());

    let entry = ListBoxAddEntry {
        row: 0,
        column: 1,
        overwrite: true,
        data: ListBoxItemData::Text("Bravo".to_string()),
        color: Some(ShellColor::new(50, 60, 70, 80)),
    };
    let result = with_payload(WindowMsgPayload::AddEntry(entry), |token| {
        window.send_system_message(WindowMessage::User(GLM_ADD_ENTRY), token, 0)
    });
    assert_eq!(result.value_i32(), Some(0));

    let pos_token = push_payload(WindowMsgPayload::CellPosition(ListBoxCellPosition {
        x: 1,
        y: 0,
    }));
    let text_token = push_payload(WindowMsgPayload::TextAndColor(ListBoxTextAndColor {
        text: String::new(),
        color: ShellColor::new(0, 0, 0, 0),
    }));
    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_GET_TEXT), pos_token, text_token,),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        pop_payload(text_token),
        Some(WindowMsgPayload::TextAndColor(ListBoxTextAndColor {
            text: "Bravo".to_string(),
            color: ShellColor::new(50, 60, 70, 80),
        }))
    );

    let item_token = push_payload(WindowMsgPayload::ItemData(ListBoxItemData::Integer(99)));
    assert_eq!(
        window.send_system_message(
            WindowMessage::User(GLM_SET_ITEM_DATA),
            pos_token,
            item_token,
        ),
        WindowMsgHandled::Handled
    );
    let _ = pop_payload(item_token);
    let out_token = push_payload(WindowMsgPayload::ItemDataOpt(None));
    assert_eq!(
        window.send_system_message(WindowMessage::User(GLM_GET_ITEM_DATA), pos_token, out_token,),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        pop_payload(out_token),
        Some(WindowMsgPayload::ItemDataOpt(Some(
            ListBoxItemData::Integer(99)
        )))
    );
    let _ = pop_payload(pos_token);

    let mut full_window = GameWindow::new();
    let mut full_listbox = ListBox::new(43, 0, 0, 100, 60);
    full_listbox.set_max_length(1);
    full_listbox.add_item("existing");
    full_window.set_widget(WindowWidget::ListBox(full_listbox));
    let entry = ListBoxAddEntry {
        row: -1,
        column: 0,
        overwrite: true,
        data: ListBoxItemData::Text("overflow".to_string()),
        color: Some(ShellColor::new(90, 91, 92, 93)),
    };
    let result = with_payload(WindowMsgPayload::AddEntry(entry), |token| {
        full_window.send_system_message(WindowMessage::User(GLM_ADD_ENTRY), token, 0)
    });
    assert_eq!(result.value_i32(), Some(-1));
    assert!(result.is_handled());
}

#[test]
fn listbox_selection_system_messages_match_cpp_out_pointer_shape() {
    let mut window = GameWindow::new();
    let mut listbox = ListBox::new(42, 0, 0, 100, 60).with_selection_mode(SelectionMode::Multiple);
    listbox.add_item("alpha");
    listbox.add_item("bravo");
    listbox.add_item("charlie");
    window.set_widget(WindowWidget::ListBox(listbox));

    assert_eq!(
        with_payload(WindowMsgPayload::IntList(vec![0, 2]), |token| {
            window.send_system_message(WindowMessage::User(GLM_SET_SELECTION), token, 2)
        }),
        WindowMsgHandled::Handled
    );
    assert_eq!(window.list_box_mut().unwrap().selected_indices(), &[0, 2]);

    let result = window.list_box_selection_result().unwrap();
    assert_eq!(result.single, -1);
    assert_eq!(result.multiple, vec![0, 2]);

    let mut selected_out: WindowMsgData = 0;
    gadget_list_box_get_selected(&mut window, &mut selected_out);
    assert_eq!(
        pop_payload(selected_out),
        Some(WindowMsgPayload::IntList(vec![0, 2]))
    );

    let mut single_window = GameWindow::new();
    let mut single = ListBox::new(43, 0, 0, 100, 60);
    single.add_item("alpha");
    single.add_item("bravo");
    assert!(single.select_index(1, KeyModifiers::none()));
    single_window.set_widget(WindowWidget::ListBox(single));
    let single_token = push_payload(WindowMsgPayload::None);
    assert_eq!(
        single_window.send_system_message(WindowMessage::User(GLM_GET_SELECTION), 0, single_token,),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(single_token), Some(WindowMsgPayload::Int(1)));
}

#[test]
fn test_window_region() {
    let region = WindowRegion::new(10, 20, 100, 200);
    assert_eq!(region.low.x, 10);
    assert_eq!(region.low.y, 20);
    assert_eq!(region.high.x, 110);
    assert_eq!(region.high.y, 220);
    assert_eq!(region.width(), 100);
    assert_eq!(region.height(), 200);

    assert!(region.contains_point(50, 100));
    assert!(!region.contains_point(5, 100));
    assert!(!region.contains_point(50, 250));
}

#[test]
fn test_point_in_window() {
    let mut window = GameWindow::new();
    window.set_position(10, 10).unwrap();
    window.set_size(100, 100).unwrap();

    assert!(window.point_in_window(50, 50));
    assert!(!window.point_in_window(5, 50));
    assert!(!window.point_in_window(150, 50));
}

#[test]
fn point_in_child_returns_deepest_enabled_visible_child_like_cpp() {
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));
    let grandchild = Rc::new(RefCell::new(GameWindow::new()));

    parent.borrow_mut().set_position(10, 10).unwrap();
    parent.borrow_mut().set_size(100, 100).unwrap();
    parent.borrow_mut().enable(true).unwrap();

    child.borrow_mut().set_position(5, 5).unwrap();
    child.borrow_mut().set_size(40, 40).unwrap();
    child.borrow_mut().enable(true).unwrap();
    child.borrow_mut().set_parent(Some(&parent));
    parent.borrow_mut().add_child(child.clone());

    grandchild.borrow_mut().set_position(4, 4).unwrap();
    grandchild.borrow_mut().set_size(10, 10).unwrap();
    grandchild.borrow_mut().enable(true).unwrap();
    grandchild.borrow_mut().set_parent(Some(&child));
    child.borrow_mut().add_child(grandchild.clone());

    let found = GameWindow::point_in_child(&parent, 20, 20, false);
    assert!(Rc::ptr_eq(&found, &grandchild));

    grandchild.borrow_mut().enable(false).unwrap();
    let found = GameWindow::point_in_child(&parent, 20, 20, false);
    assert!(Rc::ptr_eq(&found, &child));

    let found = GameWindow::point_in_child(&parent, 20, 20, true);
    assert!(Rc::ptr_eq(&found, &grandchild));
}

#[test]
fn point_in_any_child_matches_hidden_and_disabled_cpp_rules() {
    let parent = Rc::new(RefCell::new(GameWindow::new()));
    let child = Rc::new(RefCell::new(GameWindow::new()));

    parent.borrow_mut().set_position(0, 0).unwrap();
    parent.borrow_mut().set_size(100, 100).unwrap();
    parent.borrow_mut().enable(true).unwrap();

    child.borrow_mut().set_position(10, 10).unwrap();
    child.borrow_mut().set_size(20, 20).unwrap();
    child.borrow_mut().enable(false).unwrap();
    child.borrow_mut().set_parent(Some(&parent));
    parent.borrow_mut().add_child(child.clone());

    let found = GameWindow::point_in_child(&parent, 15, 15, false);
    assert!(Rc::ptr_eq(&found, &parent));

    let found = GameWindow::point_in_any_child(&parent, 15, 15, true, false);
    assert!(Rc::ptr_eq(&found, &child));

    child.borrow_mut().hide(true).unwrap();
    let found = GameWindow::point_in_any_child(&parent, 15, 15, true, false);
    assert!(Rc::ptr_eq(&found, &parent));

    let found = GameWindow::point_in_any_child(&parent, 15, 15, false, false);
    assert!(Rc::ptr_eq(&found, &child));
}

#[test]
fn test_user_data() {
    let mut window = GameWindow::new();

    window.set_user_data(42i32);
    assert_eq!(window.get_user_data::<i32>(), Some(&42));
    assert_eq!(window.get_user_data::<String>(), None);

    window.set_user_data("test".to_string());
    assert_eq!(window.get_user_data::<String>(), Some(&"test".to_string()));
    assert_eq!(window.get_user_data::<i32>(), None);
}

#[test]
fn edit_data_stores_gui_editor_callback_names_like_cpp() {
    let mut window = GameWindow::new();
    assert!(window.get_edit_data().is_none());

    let edit_data = GameWindowEditData {
        system_callback_string: "System".to_string(),
        input_callback_string: "Input".to_string(),
        tooltip_callback_string: "Tooltip".to_string(),
        draw_callback_string: "Draw".to_string(),
    };

    window.set_edit_data(Some(edit_data.clone()));
    assert_eq!(window.get_edit_data(), Some(&edit_data));

    window.set_edit_data(None);
    assert!(window.get_edit_data().is_none());
}

#[test]
fn test_callbacks() {
    let mut window = GameWindow::new();
    window.set_status(WindowStatus::ENABLED);

    // Test input callback
    window.set_input_callback(|_win, msg, _d1, _d2| match msg {
        WindowMessage::LeftDown => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    });

    assert_eq!(
        window.send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::RightDown, 0, 0),
        WindowMsgHandled::Ignored
    );
}

#[test]
fn callback_getters_return_installed_handlers_like_cpp() {
    let window = GameWindow::new();

    assert!(window.get_draw_callback().is_some());
    assert!(window.get_tooltip_callback().is_none());
    assert_eq!(
        window.get_input_callback().unwrap()(&window, WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Ignored
    );
    assert_eq!(
        window.get_system_callback().unwrap()(&window, WindowMessage::Create, 0, 0),
        WindowMsgHandled::Ignored
    );
}

#[test]
fn callback_resets_restore_default_handlers_like_cpp_null_setters() {
    let mut window = GameWindow::new();
    window.set_status(WindowStatus::ENABLED);
    let drawn = Rc::new(RefCell::new(0));

    {
        let drawn = drawn.clone();
        window.set_draw_callback(move |_, _| {
            *drawn.borrow_mut() += 1;
        });
    }
    window.set_input_callback(|_, _, _, _| WindowMsgHandled::Handled);
    window.set_system_callback(|_, _, _, _| WindowMsgHandled::Handled);
    window.set_tooltip_callback(|_, _, _| {});

    window.draw();
    assert_eq!(*drawn.borrow(), 1);
    assert_eq!(
        window.send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_system_message(WindowMessage::Create, 0, 0),
        WindowMsgHandled::Handled
    );

    window.reset_draw_callback();
    window.reset_input_callback();
    window.reset_system_callback();
    window.clear_tooltip_callback();

    window.draw();
    assert_eq!(*drawn.borrow(), 1);
    assert_eq!(
        window.send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Ignored
    );
    assert_eq!(
        window.send_system_message(WindowMessage::Create, 0, 0),
        WindowMsgHandled::Ignored
    );
    assert!(window.get_tooltip_callback().is_none());
}

#[test]
fn set_callbacks_updates_input_draw_and_tooltip_like_cpp() {
    let mut window = GameWindow::new();
    window.set_status(WindowStatus::ENABLED);
    let drawn = Rc::new(RefCell::new(0));
    let tooltip_seen = Rc::new(RefCell::new(0));

    let draw: DrawCallback = {
        let drawn = drawn.clone();
        Box::new(move |_, _| {
            *drawn.borrow_mut() += 1;
        })
    };
    let tooltip: TooltipCallback = {
        let tooltip_seen = tooltip_seen.clone();
        Box::new(move |_, _, mouse| {
            *tooltip_seen.borrow_mut() = mouse;
        })
    };

    window.set_callbacks(
        Some(Box::new(|_, _, _, _| WindowMsgHandled::Handled)),
        Some(draw),
        Some(tooltip),
    );

    assert_eq!(
        window.send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Handled
    );
    window.draw();
    assert_eq!(*drawn.borrow(), 1);
    window.get_tooltip_callback().unwrap()(&window, window.instance_data(), 42);
    assert_eq!(*tooltip_seen.borrow(), 42);

    window.set_callbacks(None, None, None);
    assert_eq!(
        window.send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Ignored
    );
    window.draw();
    assert_eq!(*drawn.borrow(), 1);
    assert!(window.get_tooltip_callback().is_none());
}

#[test]
fn destroyed_window_ignores_non_destroy_system_messages_like_cpp() {
    let mut window = GameWindow::new();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        window.set_system_callback(move |_, msg, _, _| {
            seen.borrow_mut().push(msg);
            WindowMsgHandled::Handled
        });
    }
    window.set_status_exact(WindowStatus::ENABLED | WindowStatus::DESTROYED);

    assert_eq!(
        window.send_system_message(WindowMessage::Create, 0, 0),
        WindowMsgHandled::Ignored
    );
    assert_eq!(
        window.send_system_message(WindowMessage::Destroy, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(seen.borrow().as_slice(), &[WindowMessage::Destroy]);
}

#[test]
fn destroyed_window_ignores_non_destroy_input_messages_like_cpp() {
    let mut window = GameWindow::new();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        window.set_input_callback(move |_, msg, _, _| {
            seen.borrow_mut().push(msg);
            WindowMsgHandled::Handled
        });
    }
    window.set_status_exact(WindowStatus::DESTROYED);

    assert_eq!(
        window.send_input_message(WindowMessage::LeftDown, 0, 0),
        WindowMsgHandled::Ignored
    );
    assert_eq!(
        window.send_input_message(WindowMessage::Destroy, 0, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(seen.borrow().as_slice(), &[WindowMessage::Destroy]);
}

#[test]
fn char_key_state_up_maps_to_widget_key_up() {
    let mut window = GameWindow::new();
    window.set_id(7);
    window.set_status(WindowStatus::ENABLED);
    window.set_widget(WindowWidget::PushButton(PushButton::new(7, 0, 0, 100, 30)));
    window.set_system_callback(|_, msg, data1, _| {
        if msg == WindowMessage::GadgetSelected && data1 == 7 {
            WindowMsgHandled::Handled
        } else {
            WindowMsgHandled::Ignored
        }
    });
    if let Some(WindowWidget::PushButton(button)) = window.widget_mut() {
        button.set_focus(true);
    }

    assert_eq!(
        window.send_input_message(WindowMessage::Char, 13, KEY_STATE_DOWN),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::Char, 13, KEY_STATE_UP),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        window.send_input_message(WindowMessage::Char, 13, 0),
        WindowMsgHandled::Ignored
    );
}

#[test]
fn image_status_with_defined_color_does_not_emit_draw_rect_fill() {
    // Regression windows: MainMenu.wnd:MainMenuParent, MainMenu.wnd:Logo,
    // MainMenu.wnd:MainMenuRuler are WIN_STATUS_IMAGE USER windows that
    // use default_draw_callback. IMAGE + defined color must not fill.
    for name in [
        "MainMenu.wnd:MainMenuParent",
        "MainMenu.wnd:Logo",
        "MainMenu.wnd:MainMenuRuler",
    ] {
        let ops = plan_w3d_game_win_default_draw(true, true, true, true);
        assert_eq!(
            ops,
            vec![W3dDefaultDrawOp::Image],
            "{name} IMAGE + defined color must not emit draw_rect fill"
        );
    }
    let ops = plan_w3d_game_win_default_draw(true, true, true, true);
    assert_eq!(ops, vec![W3dDefaultDrawOp::Image]);
    assert!(
        !ops.contains(&W3dDefaultDrawOp::ColorFill),
        "IMAGE + defined color must not emit draw_rect fill"
    );
    assert!(
        !ops.contains(&W3dDefaultDrawOp::ColorBorder),
        "IMAGE + defined border must not emit draw_rect_outline"
    );
}

#[test]
fn clamp_border_span_rejects_runaway_window_size() {
    use super::window_impl_draw::{MAX_BORDER_SPAN, clamp_border_span};
    assert_eq!(clamp_border_span(-20), 0);
    assert_eq!(clamp_border_span(800), 800);
    assert_eq!(clamp_border_span(i32::MAX), MAX_BORDER_SPAN);
    assert!(
        MAX_BORDER_SPAN <= 4096,
        "border tiling must stay on-screen-scale"
    );
}

#[test]
fn ui_renderer_caps_draw_commands_before_vertex_flood() {
    let src = include_str!("../ui_renderer.rs");
    assert!(src.contains("MAX_DRAW_COMMANDS_PER_FRAME"));
    assert!(src.contains("if self.draw_commands.len() >= Self::MAX_DRAW_COMMANDS_PER_FRAME"));
    assert!(
        src.contains("dropping remaining commands"),
        "render() must stop assembling verts before allocating hundreds of MB"
    );
}

#[test]
fn image_status_missing_image_draws_nothing() {
    let ops = plan_w3d_game_win_default_draw(true, true, true, false);
    assert!(
        ops.is_empty(),
        "IMAGE with missing mapped image must draw nothing, not a black rect"
    );
}

#[test]
fn non_image_status_emits_color_fill_and_border() {
    let ops = plan_w3d_game_win_default_draw(false, true, true, false);
    assert_eq!(
        ops,
        vec![W3dDefaultDrawOp::ColorFill, W3dDefaultDrawOp::ColorBorder]
    );
}

#[test]
fn default_draw_image_branch_source_does_not_call_draw_rect() {
    let src = include_str!("callbacks.rs");
    let start = src
        .find("pub fn default_draw_callback")
        .expect("default_draw_callback");
    let body = &src[start..];
    let plan_idx = body
        .find("plan_w3d_game_win_default_draw")
        .expect("default_draw must use the C++ IMAGE/color planner");
    let image_op = body
        .find("W3dDefaultDrawOp::Image")
        .expect("IMAGE op must be dispatched");
    let fill_op = body
        .find("W3dDefaultDrawOp::ColorFill")
        .expect("color fill must be gated on planner");
    assert!(
        plan_idx < image_op && plan_idx < fill_op,
        "draw_rect must be gated by plan_w3d_game_win_default_draw, not IMAGE+fill"
    );
    // Planner itself is the shipped IMAGE contract.
    assert!(
        src.contains("`WIN_STATUS_IMAGE` draws the mapped image only")
            || src.contains("WIN_STATUS_IMAGE draws the mapped image only"),
        "default draw must document C++ IMAGE-only contract"
    );
    let regression = include_str!("tests.rs");
    for name in [
        "MainMenu.wnd:MainMenuParent",
        "MainMenu.wnd:Logo",
        "MainMenu.wnd:MainMenuRuler",
    ] {
        assert!(
            regression.contains(name),
            "IMAGE regression must name {name}"
        );
    }
}

#[test]
fn combo_left_up_toggles_child_list_like_cpp() {
    let (mut combo, _edit_box, list_box, _drop_down) = combo_fixture();
    combo.enable(true).unwrap();
    let _ = with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
        combo.send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0)
    });
    assert!(list_box.borrow().is_hidden());

    assert_eq!(
        combo.send_input_message(WindowMessage::LeftUp, 0, 0),
        WindowMsgHandled::Handled
    );
    assert!(!list_box.borrow().is_hidden());
    assert!(combo.get_size().1 > 20);

    assert_eq!(
        combo.send_input_message(WindowMessage::LeftUp, 0, 0),
        WindowMsgHandled::Handled
    );
    assert!(list_box.borrow().is_hidden());
    assert_eq!(combo.get_size(), (120, 20));
}

#[test]
fn combo_drop_down_button_toggles_via_shared_open_path() {
    let (mut combo, _edit_box, list_box, _drop_down) = combo_fixture();
    combo.enable(true).unwrap();
    let _ = with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
        combo.send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0)
    });

    assert_eq!(
        combo.send_system_message(WindowMessage::GadgetSelected, 4, 0),
        WindowMsgHandled::Handled
    );
    assert!(!list_box.borrow().is_hidden());

    assert_eq!(
        combo.send_system_message(WindowMessage::GadgetSelected, 4, 0),
        WindowMsgHandled::Handled
    );
    assert!(list_box.borrow().is_hidden());
}

#[test]
fn combo_input_focus_forwards_to_edit_child_like_cpp() {
    let (mut combo, edit_box, _list_box, _drop_down) = combo_fixture();
    combo.enable(true).unwrap();
    edit_box.borrow_mut().enable(true).unwrap();

    let token = push_payload(WindowMsgPayload::Bool(false));
    assert_eq!(
        combo.send_system_message(WindowMessage::InputFocus, 1, token),
        WindowMsgHandled::Handled
    );
    assert_eq!(pop_payload(token), Some(WindowMsgPayload::Bool(true)));
    assert!(
        edit_box
            .borrow()
            .instance_data()
            .state
            .contains(WindowState::HILITED)
    );
}

#[test]
fn slider_left_drag_updates_value_and_sends_track() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut slider = GameWindow::new();
    slider.set_id(8);
    slider.enable(true).unwrap();
    slider.set_size(100, 16).unwrap();
    slider.set_owner(Some(&owner));
    slider.set_widget(WindowWidget::HorizontalSlider(
        HorizontalSlider::new(8, 0, 0, 100, 16)
            .with_range(0, 10)
            .with_value(5),
    ));

    let thumb = Rc::new(RefCell::new(GameWindow::new()));
    thumb.borrow_mut().set_id(9);
    thumb.borrow_mut().set_size(13, 16).unwrap();
    thumb.borrow_mut().set_position(40, 10).unwrap();
    slider.add_child(thumb);
    slider.set_slider_thumb(9);

    let packed = 200usize;
    assert_eq!(
        slider.send_system_message(WindowMessage::User(GGM_LEFT_DRAG), 9, packed),
        WindowMsgHandled::Handled
    );
    assert_eq!(slider.horizontal_slider_mut().unwrap().value(), 10);
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::User(GSM_SLIDER_TRACK), 8, 10)]
    );
}

#[test]
fn slider_keyboard_sends_gsm_slider_track_like_cpp() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut slider = GameWindow::new();
    slider.set_id(8);
    slider.enable(true).unwrap();
    slider.set_owner(Some(&owner));
    slider.set_widget(WindowWidget::HorizontalSlider(
        HorizontalSlider::new(8, 0, 0, 100, 16)
            .with_range(0, 10)
            .with_value(5)
            .with_step_size(1),
    ));
    slider.horizontal_slider_mut().unwrap().set_focus(true);

    assert_eq!(
        slider.send_input_message(WindowMessage::Char, 0x27, KEY_STATE_DOWN),
        WindowMsgHandled::Handled
    );
    assert_eq!(slider.horizontal_slider_mut().unwrap().value(), 3);
    assert_eq!(
        owner_seen.borrow().as_slice(),
        &[(WindowMessage::User(GSM_SLIDER_TRACK), 8, 3)]
    );
}

#[test]
fn listbox_right_click_sends_right_click_struct_payload() {
    let owner_seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(RefCell::new(GameWindow::new()));
    {
        let owner_seen = owner_seen.clone();
        owner
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                owner_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let mut window = GameWindow::new();
    window.set_id(3);
    window.enable(true).unwrap();
    window.set_owner(Some(&owner));
    window.set_position(10, 20).unwrap();
    window.set_size(120, 40).unwrap();
    let mut listbox = ListBox::new(3, 0, 0, 120, 40);
    listbox.add_item("Alpha");
    listbox.add_item("Bravo");
    window.set_widget(WindowWidget::ListBox(listbox));
    window.set_cursor_position(5, 8).unwrap();

    assert_eq!(
        window.send_input_message(WindowMessage::RightUp, 0, 0),
        WindowMsgHandled::Handled
    );
    let seen = owner_seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, WindowMessage::User(GLM_RIGHT_CLICKED));
    assert_eq!(seen[0].1, 3);
    match payload(seen[0].2) {
        Some(WindowMsgPayload::RightClick(rc)) => {
            assert_eq!(rc.mouse_x, 15);
            assert_eq!(rc.mouse_y, 28);
        }
        other => panic!("expected RightClick payload, got {other:?}"),
    }
}

#[test]
fn tab_pane_forwards_selected_to_tab_control_parent_like_cpp() {
    let root_seen = Rc::new(RefCell::new(Vec::new()));
    let root = Rc::new(RefCell::new(GameWindow::new()));
    {
        let root_seen = root_seen.clone();
        root.borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                root_seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let tab = Rc::new(RefCell::new(GameWindow::new()));
    tab.borrow_mut()
        .set_widget(WindowWidget::TabControl(TabControl::new(7, 0, 0, 100, 80)));
    tab.borrow_mut().set_parent(Some(&root));
    root.borrow_mut().add_child(tab.clone());

    let pane = Rc::new(RefCell::new(GameWindow::new()));
    pane.borrow_mut().set_widget(WindowWidget::TabPane);
    pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
    pane.borrow_mut().set_parent(Some(&tab));
    tab.borrow_mut().add_child(pane.clone());

    assert_eq!(
        pane.borrow_mut()
            .send_system_message(WindowMessage::GadgetSelected, 21, 0),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        root_seen.borrow().as_slice(),
        &[(WindowMessage::GadgetSelected, 21, 0)]
    );
}
